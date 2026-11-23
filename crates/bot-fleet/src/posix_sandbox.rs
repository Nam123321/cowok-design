// @story 11.1
// @implements AC-01, AC-03 (POSIX 0o600 FD Staging + O_CLOEXEC + env_clear Allowlist + -PGID Zero-Zombie Reaper)
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::session_dir::{create_file_0600_nofollow, zeroize_and_unlink, IsolatedSessionDir};
use crate::BotFleetError;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::io::FromRawFd;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Strict parent environment allowlist forwarded to sandboxed child processes after `cmd.env_clear()`.
/// Ambient daemon secrets (`COWOK_SUPERADMIN_SECRET`, `COWOK_VAULT_KEY_DIR`, `COWOK_WORKBENCH_SECRET_DIR`,
/// host cloud API keys) are NEVER inherited.
pub const SANDBOX_PARENT_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TMPDIR",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "TERM",
];

const READER_DRAIN_GRACE_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptTransport {
    /// Streams staged `0o600` prompt file directly into child stdin file descriptor (zero `argv` exposure).
    StdinFd,
    /// Passes the `0o600` staged prompt file path via an isolated environment variable (zero `argv` exposure).
    StagedFileEnvVar(&'static str),
    /// Passes the `0o600` staged prompt file path via a CLI flag (never prompt bytes).
    StagedFileFlag(&'static str),
}

impl PromptTransport {
    pub fn label(&self) -> &'static str {
        match self {
            Self::StdinFd => "stdin_fd_0600",
            Self::StagedFileEnvVar(_) => "env_file_0600",
            Self::StagedFileFlag(_) => "flag_file_0600",
        }
    }
}

/// Ephemeral `0o600` prompt file staged inside `$COWOK_SESSION_DIR/prompts/`.
/// Automatically zeroizes and unlinks on `Drop` (normal exit, error, or panic).
#[derive(Debug)]
pub struct StagedPromptFile {
    path: PathBuf,
    byte_len: usize,
    sha256: String,
    cleaned: AtomicBool,
}

impl StagedPromptFile {
    pub fn stage_in_session(
        session: &IsolatedSessionDir,
        prompt: &str,
    ) -> Result<Self, BotFleetError> {
        let filename = format!("prompt-{}.txt", Uuid::new_v4().simple());
        let bytes = prompt.as_bytes();
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        let path = create_file_0600_nofollow(&session.prompts_dir(), &filename, bytes)?;
        Ok(Self {
            path,
            byte_len: bytes.len(),
            sha256,
            cleaned: AtomicBool::new(false),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Opens the staged `0o600` file read-only with `O_NOFOLLOW | O_CLOEXEC` to attach directly to child `Stdio`.
    /// Note: `Command::stdin(Stdio::from(file))` `dup2`s this FD to FD 0 (which clears CLOEXEC on FD 0)
    /// while preventing the original descriptor from leaking into the child process.
    pub fn open_readonly_stdio(&self) -> Result<Stdio, BotFleetError> {
        #[cfg(unix)]
        {
            let c_path = CString::new(self.path.as_os_str().as_bytes())
                .map_err(|_| BotFleetError::InvalidPath("Staged prompt path has NUL".into()))?;
            // SAFETY: c_path is NUL-terminated; O_RDONLY | O_NOFOLLOW | O_CLOEXEC prevents symlink swap and FD leaks.
            let fd = unsafe {
                libc::open(
                    c_path.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ELOOP) {
                    return Err(BotFleetError::SymlinkViolation(format!(
                        "O_NOFOLLOW rejected symlink staged prompt '{}'",
                        self.path.display()
                    )));
                }
                return Err(BotFleetError::Io(format!(
                    "Failed opening staged prompt '{}' read-only: {err}",
                    self.path.display()
                )));
            }
            // SAFETY: fd is a freshly opened valid descriptor owned by File -> Stdio.
            let file = unsafe { File::from_raw_fd(fd) };
            Ok(Stdio::from(file))
        }
        #[cfg(not(unix))]
        {
            let meta = std::fs::symlink_metadata(&self.path)
                .map_err(|e| BotFleetError::Io(format!("Failed inspecting staged prompt: {e}")))?;
            if meta.file_type().is_symlink() {
                return Err(BotFleetError::SymlinkViolation(format!(
                    "Refusing symlink staged prompt '{}'",
                    self.path.display()
                )));
            }
            let file = File::open(&self.path)
                .map_err(|e| BotFleetError::Io(format!("Failed opening staged prompt: {e}")))?;
            Ok(Stdio::from(file))
        }
    }

    pub fn cleanup_now(&self) {
        if !self.cleaned.swap(true, Ordering::SeqCst) {
            zeroize_and_unlink(&self.path);
        }
    }
}

impl Drop for StagedPromptFile {
    fn drop(&mut self) {
        self.cleanup_now();
    }
}

#[derive(Debug, Clone)]
pub struct SandboxExecutionSpec {
    pub binary: PathBuf,
    pub args: Vec<String>,
    pub prompt: Zeroizing<String>,
    pub prompt_transport: PromptTransport,
    pub working_dir: Option<PathBuf>,
    pub extra_env: Vec<(String, Zeroizing<String>)>,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub redact_secrets: Vec<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub struct SandboxExecutionOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub pgid_reaped: bool,
    pub duration_ms: u64,
    pub pid: u32,
    pub pgid: i32,
    pub prompt_sha256: String,
}

/// RAII process-group guard ensuring `kill(-pgid, SIGKILL)` + `waitpid` reaping on timeout or drop.
struct PgidProcessGuard {
    child: Option<Child>,
    pgid: i32,
    reaped_group: bool,
}

impl PgidProcessGuard {
    fn new(child: Child) -> Self {
        let pgid = child.id() as i32;
        Self {
            child: Some(child),
            pgid,
            reaped_group: false,
        }
    }

    fn pid(&self) -> u32 {
        self.child
            .as_ref()
            .map(Child::id)
            .unwrap_or(self.pgid.max(0) as u32)
    }

    fn kill_and_reap_group(&mut self) {
        if self.reaped_group {
            return;
        }
        #[cfg(unix)]
        {
            if self.pgid > 0 {
                // SAFETY: pgid is our child's dedicated process group created via setpgid(0, 0).
                unsafe {
                    libc::kill(-self.pgid, libc::SIGKILL);
                }
            }
        }
        if let Some(mut child) = self.child.take() {
            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        #[cfg(unix)]
        {
            if self.pgid > 0 {
                // Reap any remaining direct children in the process group so 0 zombies remain.
                let mut status: libc::c_int = 0;
                for _ in 0..8 {
                    // SAFETY: waitpid(-pgid, ..., WNOHANG) only reaps our own exited child processes.
                    let rc = unsafe { libc::waitpid(-self.pgid, &mut status, libc::WNOHANG) };
                    if rc <= 0 {
                        break;
                    }
                }
            }
        }
        self.reaped_group = true;
    }
}

impl Drop for PgidProcessGuard {
    fn drop(&mut self) {
        if !self.reaped_group {
            self.kill_and_reap_group();
        }
    }
}

fn spawn_bounded_reader<R: Read + Send + 'static>(
    mut reader: R,
    max_bytes: usize,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if out.len() < max_bytes {
                let take = (max_bytes - out.len()).min(n);
                out.extend_from_slice(&buf[..take]);
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&out).into_owned());
    });
    rx
}

/// Redacts secrets from text so keys/tokens never escape into logs or response envelopes.
pub fn redact_secrets_from_text(input: &str, secrets: &[Zeroizing<String>]) -> String {
    let mut sanitized = input.to_string();
    for secret in secrets {
        let trimmed = secret.trim();
        if trimmed.len() >= 6 && sanitized.contains(trimmed) {
            sanitized = sanitized.replace(trimmed, "[REDACTED]");
        }
    }
    sanitized
}

fn collect_effective_redact_secrets(spec: &SandboxExecutionSpec) -> Vec<Zeroizing<String>> {
    let mut all = spec.redact_secrets.clone();
    for (_, val) in &spec.extra_env {
        if val.trim().len() >= 6 {
            all.push(val.clone());
        }
    }
    all
}

fn verify_argv_has_no_secret_or_prompt(
    spec: &SandboxExecutionSpec,
    all_secrets: &[Zeroizing<String>],
) -> Result<(), BotFleetError> {
    let prompt_trimmed = spec.prompt.trim();
    for arg in &spec.args {
        if prompt_trimmed.len() >= 8 && arg.contains(prompt_trimmed) {
            return Err(BotFleetError::ArgvSecretLeakBlocked(
                "Prompt content MUST NOT be passed via process argv; use 0o600 FD staging".into(),
            ));
        }
        for secret in all_secrets {
            let s = secret.trim();
            if s.len() >= 6 && arg.contains(s) {
                return Err(BotFleetError::ArgvSecretLeakBlocked(
                    "API secret MUST NOT be passed via process argv".into(),
                ));
            }
        }
    }
    Ok(())
}

/// Supervises isolated POSIX BYOS CLI execution using `0o600` FD staging, `$COWOK_SESSION_DIR`,
/// strict `env_clear()` allowlist isolation, `setpgid(0, 0)`, and `kill(-pgid, SIGKILL)` + `waitpid` reaping.
pub struct PgidProcessSupervisor;

impl PgidProcessSupervisor {
    /// Executes a sandboxed child process synchronously (call inside `tokio::task::spawn_blocking` from async routes).
    pub fn execute_in_session(
        session: &IsolatedSessionDir,
        spec: &SandboxExecutionSpec,
    ) -> Result<SandboxExecutionOutput, BotFleetError> {
        let effective_secrets = collect_effective_redact_secrets(spec);
        verify_argv_has_no_secret_or_prompt(spec, &effective_secrets)?;

        let staged = StagedPromptFile::stage_in_session(session, &spec.prompt)?;
        let prompt_sha256 = staged.sha256().to_string();

        let mut cmd = Command::new(&spec.binary);
        cmd.args(&spec.args);

        // Zero-Trust Env Isolation: clear inherited daemon environment first, then re-inject ONLY strict allowlist.
        cmd.env_clear();
        for key in SANDBOX_PARENT_ENV_ALLOWLIST {
            if let Some(val) = std::env::var_os(key) {
                cmd.env(key, val);
            }
        }
        for (k, v) in session.isolated_env() {
            cmd.env(k, v);
        }
        for (k, v) in &spec.extra_env {
            cmd.env(k, v.as_str());
        }

        match spec.prompt_transport {
            PromptTransport::StdinFd => {
                cmd.stdin(staged.open_readonly_stdio()?);
            }
            PromptTransport::StagedFileEnvVar(var_name) => {
                cmd.stdin(Stdio::null());
                cmd.env(var_name, staged.path());
            }
            PromptTransport::StagedFileFlag(flag) => {
                cmd.arg(flag).arg(staged.path());
                cmd.stdin(Stdio::null());
            }
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(ref wd) = spec.working_dir {
            cmd.current_dir(wd);
        }

        #[cfg(unix)]
        {
            // SAFETY: pre_exec executes only async-signal-safe libc::setpgid(0, 0) prior to exec.
            unsafe {
                cmd.pre_exec(|| {
                    if libc::setpgid(0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        let start = Instant::now();
        let child = cmd.spawn().map_err(|e| {
            BotFleetError::SpawnFailed(format!("Failed spawning '{}': {e}", spec.binary.display()))
        })?;

        let mut guard = PgidProcessGuard::new(child);
        let pid = guard.pid();
        let pgid = guard.pgid;

        let max_bytes = spec.max_output_bytes.clamp(1024, 4 * 1024 * 1024);
        let stdout_rx = guard
            .child
            .as_mut()
            .and_then(|c| c.stdout.take())
            .map(|r| spawn_bounded_reader(r, max_bytes));
        let stderr_rx = guard
            .child
            .as_mut()
            .and_then(|c| c.stderr.take())
            .map(|r| spawn_bounded_reader(r, max_bytes));

        let deadline = Instant::now() + Duration::from_millis(spec.timeout_ms.max(50));
        let mut exit_code: Option<i32> = None;
        let mut timed_out = false;

        loop {
            let status_opt = match guard.child.as_mut() {
                Some(c) => c
                    .try_wait()
                    .map_err(|e| BotFleetError::Io(format!("Failed polling child status: {e}")))?,
                None => None,
            };

            if let Some(status) = status_opt {
                exit_code = status.code();
                // Even after leader exits, kill any lingering backgrounded descendants in -PGID.
                guard.kill_and_reap_group();
                break;
            }

            if Instant::now() >= deadline {
                timed_out = true;
                guard.kill_and_reap_group();
                break;
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        let pgid_reaped = guard.reaped_group;
        let drain_timeout = Duration::from_millis(READER_DRAIN_GRACE_MS);
        let raw_stdout = stdout_rx
            .and_then(|rx| rx.recv_timeout(drain_timeout).ok())
            .unwrap_or_default();
        let raw_stderr = stderr_rx
            .and_then(|rx| rx.recv_timeout(drain_timeout).ok())
            .unwrap_or_default();

        staged.cleanup_now();

        let stdout = redact_secrets_from_text(&raw_stdout, &effective_secrets);
        let stderr = redact_secrets_from_text(&raw_stderr, &effective_secrets);

        Ok(SandboxExecutionOutput {
            stdout,
            stderr,
            exit_code,
            timed_out,
            pgid_reaped,
            duration_ms: start.elapsed().as_millis() as u64,
            pid,
            pgid,
            prompt_sha256,
        })
    }

    /// Async wrapper executing inside `tokio::task::spawn_blocking`.
    pub async fn execute_ephemeral(
        session_id: String,
        spec: SandboxExecutionSpec,
    ) -> Result<SandboxExecutionOutput, BotFleetError> {
        tokio::task::spawn_blocking(move || {
            let session = IsolatedSessionDir::create_ephemeral(&session_id)?;
            Self::execute_in_session(&session, &spec)
        })
        .await
        .map_err(|e| BotFleetError::Io(format!("Sandbox task panicked or cancelled: {e}")))?
    }
}
