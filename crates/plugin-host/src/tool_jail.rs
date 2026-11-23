// @story 08.5
// @implements AC-02, AC-05 (HUMAN_ONLY Controlled Ephemeral Tool Jail: AST scan, staged-file scan, env_clear, setpgid, RLIMIT, O_NOFOLLOW, <=5s kill -pgid, non-blocking bounded pipe readers)
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::lifecycle::ActorKind;
use crate::repackager::scan_external_script_security;

pub const TOOL_JAIL_MAX_TIMEOUT_MS: u64 = 5_000;
pub const TOOL_JAIL_DEFAULT_TIMEOUT_MS: u64 = 3_000;
pub const TOOL_JAIL_MAX_SCRIPT_BYTES: usize = 64 * 1024;
pub const TOOL_JAIL_MAX_STAGED_FILES: usize = 16;
pub const TOOL_JAIL_MAX_STAGED_FILE_BYTES: usize = 64 * 1024;
pub const TOOL_JAIL_MAX_OUTPUT_BYTES: usize = 64 * 1024;
pub const TOOL_JAIL_RLIMIT_AS_BYTES: u64 = 256 * 1024 * 1024;
pub const TOOL_JAIL_RLIMIT_NPROC: u64 = 16;
pub const TOOL_JAIL_RLIMIT_FSIZE_BYTES: u64 = 4 * 1024 * 1024;
const PIPE_COLLECT_TIMEOUT_MS: u64 = 500;

const FORBIDDEN_STAGED_EXTENSIONS: &[&str] = &[
    ".sh", ".bash", ".zsh", ".dash", ".py", ".pyw", ".js", ".mjs", ".cjs", ".ts", ".mts", ".cts",
    ".rb", ".pl", ".php", ".so", ".dylib", ".dll", ".exe",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolJailError {
    #[error("HUMAN_APPROVAL_REQUIRED: ephemeral tool execution strictly requires HumanWorkbenchSession (got {actor:?})")]
    HumanApprovalRequired { actor: ActorKind },
    #[error("AST_CAPABILITY_BLOCKED [{rule_id}]: {reason}")]
    CapabilityBlocked { rule_id: String, reason: String },
    #[error("JAIL_SYMLINK_OR_HARDLINK_REJECTED: {path}")]
    SymlinkOrHardlinkRejected { path: String },
    #[error("JAIL_PATH_TRAVERSAL: {path}")]
    PathTraversal { path: String },
    #[error("JAIL_TIMEOUT_KILLED: process group -{pgid} terminated after {timeout_ms}ms")]
    TimeoutKilled { pgid: i32, timeout_ms: u64 },
    #[error("JAIL_SCRIPT_TOO_LARGE: {actual} > {max} bytes")]
    ScriptTooLarge { actual: usize, max: usize },
    #[error("JAIL_RUNTIME_ERROR: {0}")]
    Runtime(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EphemeralRuntime {
    Sh,
    Python3,
    Node,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EphemeralToolRequest {
    pub tool_name: String,
    pub runtime: EphemeralRuntime,
    pub script_source: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub staged_files: Vec<StagedJailFile>,
}

fn default_timeout_ms() -> u64 {
    TOOL_JAIL_DEFAULT_TIMEOUT_MS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedJailFile {
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolJailAuditReceipt {
    pub execution_id: String,
    pub tool_name: String,
    pub runtime: EphemeralRuntime,
    pub actor_kind: String,
    pub script_sha256: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub killed_pgid: Option<i32>,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub env_cleared: bool,
    /// Reflects OS-level network namespace isolation (false unless running inside a Linux netns).
    pub network_isolated: bool,
    /// Reflects proxy sinkhole env enforcement (`HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY=127.0.0.1:0`, `NO_PROXY` cleared).
    #[serde(default)]
    pub network_proxy_sinkholed: bool,
    pub nofollow_verified: bool,
    pub rlimit_as_bytes: u64,
    pub rlimit_nproc: u64,
}

struct CapabilityDenyRule {
    rule_id: &'static str,
    reason: &'static str,
    regex: Regex,
}

fn fold_fullwidth_and_strip_invisible(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let cp = ch as u32;
        // Drop zero-width / bidi spoofing codepoints
        if matches!(
            cp,
            0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFEFF
        ) {
            continue;
        }
        // Fold fullwidth ASCII variants (FF01..=FF5E -> 0x21..=0x7E)
        if (0xFF01..=0xFF5E).contains(&cp) {
            if let Some(ascii) = char::from_u32(cp - 0xFEE0) {
                out.push(ascii);
                continue;
            }
        }
        if cp == 0x3000 {
            out.push(' ');
            continue;
        }
        out.push(ch);
    }
    out
}

/// Strips shell quote-splitting tricks (`c'u'rl`, `c"u"rl`, `c\url`) for evasion-resistant scanning.
fn strip_shell_evasion_quotes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' || c == '"' {
            continue;
        }
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next.is_ascii_alphanumeric() || next == '_' || next == '/' {
                    out.push(next);
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn capability_deny_rules() -> &'static [CapabilityDenyRule] {
    static RULES: OnceLock<Vec<CapabilityDenyRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let defs: &[(&str, &str, &str)] = &[
            (
                "JAIL-NET-001",
                "Outbound network CLI or socket utility forbidden in ephemeral jail",
                r"(?i)\b(curl|wget|nc|ncat|netcat|telnet|ssh|scp|sftp|ftp|dig|nslookup|ping)\b",
            ),
            (
                "JAIL-NET-002",
                "HTTP/network/filesystem library call forbidden in ephemeral jail",
                r"(?i)(https?://|urllib|requests\.|http\.client|socket\.|from\s+socket\s+import|import\s+socket|fetch\s*\(|XMLHttpRequest|WebSocket|node:(?:net|http|https|tls|dns|dgram|fs|child_process|worker_threads|vm)|require\(\s*['\x22](?:node:)?(?:http|https|net|tls|dns|dgram|fs|child_process|worker_threads|vm)['\x22]\s*\)|\bfrom\s+['\x22](?:node:)?(?:http|https|net|tls|dns|dgram|fs|child_process|worker_threads|vm)['\x22]|\bimport\s*\(\s*['\x22](?:node:)?(?:http|https|net|tls|dns|dgram|fs|child_process|worker_threads|vm)['\x22])",
            ),
            (
                "JAIL-RCE-001",
                "Dynamic code execution, OS/process import, or shell escape primitive forbidden",
                r"(?i)(\beval\b|\bexec\s*\(|os\.system|\bimport\s+os\b|\bfrom\s+os\s+import|\bopen\s*\(|subprocess|child_process|spawnSync|execSync|execFile|Function\s*\(|__import__|importlib|ctypes|popen\s*\(|/bin/(?:ba|z|da)?sh\b|bash\s+-c|sh\s+-c)",
            ),
            (
                "JAIL-SOURCE-001",
                "Sourcing or importing staged/external scripts forbidden in ephemeral jail",
                r"(?im)(?:^|[;&|]\s*)(?:\.\s+\.?/|\bsource\s+|\bimport\s+.*['\x22]\.?/)",
            ),
            (
                "JAIL-PRIV-001",
                "Privilege escalation or destructive filesystem command forbidden",
                r"(?i)(\bsudo\b|\bsu\b|\bchmod\s+\+s|\bchown\b|\bmkfifo\b|\bmknod\b|\brm\s+-rf\s+/|\bdd\s+if=|\bln\s+-s)",
            ),
            (
                "JAIL-SMUGGLE-001",
                "Access to sealed eval suites, host system paths, daemon lockfiles, or credentials forbidden",
                r"(?i)(tests/eval/sealed|eval/sealed|\.cowok-design|daemon\.lock|/etc/|/proc/|/var/run/|\.ssh/|AWS_SECRET|GITHUB_TOKEN|OPENAI_API_KEY|HUMAN_ONLY)",
            ),
            (
                "JAIL-TRAVERSAL-001",
                "Parent directory traversal ('..' or 'cd ..') forbidden inside ephemeral script",
                r"(?i)(?:\.\./|\.\.\\|\bcd\s+\.\.)",
            ),
        ];

        defs.iter()
            .map(|(rule_id, reason, pat)| CapabilityDenyRule {
                rule_id,
                reason,
                regex: Regex::new(pat).expect("static capability deny regex"),
            })
            .collect()
    })
}

/// Static AST/token capability scan blocking RCE, network egress, shell escapes, quote-splitting evasions,
/// ESM/Python imports, and sealed-eval smuggling.
pub fn scan_ephemeral_script_capabilities(script_source: &str) -> Result<(), ToolJailError> {
    if script_source.len() > TOOL_JAIL_MAX_SCRIPT_BYTES {
        return Err(ToolJailError::ScriptTooLarge {
            actual: script_source.len(),
            max: TOOL_JAIL_MAX_SCRIPT_BYTES,
        });
    }
    if script_source.contains('\0') {
        return Err(ToolJailError::CapabilityBlocked {
            rule_id: "JAIL-NUL-001".to_string(),
            reason: "NUL byte forbidden in ephemeral script source".to_string(),
        });
    }

    // Reuse Zero-Trust repackager scanner (handles NFKC + obfuscated shell patterns)
    if let Err(err) = scan_external_script_security("ephemeral_script", script_source) {
        return Err(ToolJailError::CapabilityBlocked {
            rule_id: "JAIL-EXT-SCAN".to_string(),
            reason: err.to_string(),
        });
    }

    let folded = fold_fullwidth_and_strip_invisible(script_source);
    let dequoted = strip_shell_evasion_quotes(&folded);

    for candidate in [script_source, folded.as_str(), dequoted.as_str()] {
        for rule in capability_deny_rules() {
            if rule.regex.is_match(candidate) {
                return Err(ToolJailError::CapabilityBlocked {
                    rule_id: rule.rule_id.to_string(),
                    reason: rule.reason.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Verifies a workspace directory contains zero symlinks or hardlinks (`nlink > 1`).
pub fn assert_workspace_no_symlinks_or_hardlinks(dir: &Path) -> Result<(), ToolJailError> {
    let root_meta = fs::symlink_metadata(dir)
        .map_err(|e| ToolJailError::Runtime(format!("stat workspace {}: {e}", dir.display())))?;
    if root_meta.file_type().is_symlink() {
        return Err(ToolJailError::SymlinkOrHardlinkRejected {
            path: dir.display().to_string(),
        });
    }

    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|e| ToolJailError::Runtime(format!("read_dir {}: {e}", current.display())))?;
        for entry in entries {
            let entry = entry.map_err(|e| ToolJailError::Runtime(format!("entry err: {e}")))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| {
                ToolJailError::Runtime(format!("symlink_metadata {}: {e}", path.display()))
            })?;
            if meta.file_type().is_symlink() {
                return Err(ToolJailError::SymlinkOrHardlinkRejected {
                    path: path.display().to_string(),
                });
            }
            #[cfg(unix)]
            if meta.file_type().is_file() {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() > 1 {
                    return Err(ToolJailError::SymlinkOrHardlinkRejected {
                        path: format!("{} (hardlink nlink={})", path.display(), meta.nlink()),
                    });
                }
            }
            if meta.file_type().is_dir() {
                stack.push(path);
            }
        }
    }
    Ok(())
}

fn validate_staged_rel_path(rel: &str) -> Result<PathBuf, ToolJailError> {
    let p = Path::new(rel);
    if p.is_absolute() || rel.trim().is_empty() {
        return Err(ToolJailError::PathTraversal {
            path: rel.to_string(),
        });
    }
    let lower = rel.trim().to_ascii_lowercase();
    if FORBIDDEN_STAGED_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(ext))
    {
        return Err(ToolJailError::CapabilityBlocked {
            rule_id: "JAIL-STAGED-EXT".to_string(),
            reason: format!("Executable/script extension forbidden in staged file '{rel}'"),
        });
    }
    let mut clean = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(seg) => clean.push(seg),
            _ => {
                return Err(ToolJailError::PathTraversal {
                    path: rel.to_string(),
                })
            }
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(ToolJailError::PathTraversal {
            path: rel.to_string(),
        });
    }
    Ok(clean)
}

fn validate_staged_files(staged_files: &[StagedJailFile]) -> Result<(), ToolJailError> {
    if staged_files.len() > TOOL_JAIL_MAX_STAGED_FILES {
        return Err(ToolJailError::CapabilityBlocked {
            rule_id: "JAIL-STAGED-CAP".to_string(),
            reason: format!(
                "Staged file count {} exceeds cap {}",
                staged_files.len(),
                TOOL_JAIL_MAX_STAGED_FILES
            ),
        });
    }
    for sf in staged_files {
        let _ = validate_staged_rel_path(&sf.relative_path)?;
        if sf.content.len() > TOOL_JAIL_MAX_STAGED_FILE_BYTES {
            return Err(ToolJailError::ScriptTooLarge {
                actual: sf.content.len(),
                max: TOOL_JAIL_MAX_STAGED_FILE_BYTES,
            });
        }
        // C-02: Scan EVERY staged file with both Zero-Trust external script scanner & capability scanner
        if let Err(err) = scan_external_script_security(&sf.relative_path, &sf.content) {
            return Err(ToolJailError::CapabilityBlocked {
                rule_id: "JAIL-STAGED-SCAN".to_string(),
                reason: format!("Staged file '{}': {err}", sf.relative_path),
            });
        }
        scan_ephemeral_script_capabilities(&sf.content)?;
    }
    Ok(())
}

fn write_file_nofollow(path: &Path, bytes: &[u8]) -> Result<(), ToolJailError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ToolJailError::Runtime(format!("create_dir_all: {e}")))?;
    }
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = opts.open(path).map_err(|e| {
        #[cfg(unix)]
        if e.raw_os_error() == Some(libc::ELOOP) {
            return ToolJailError::SymlinkOrHardlinkRejected {
                path: path.display().to_string(),
            };
        }
        ToolJailError::Runtime(format!("open {}: {e}", path.display()))
    })?;
    file.write_all(bytes)
        .map_err(|e| ToolJailError::Runtime(format!("write {}: {e}", path.display())))?;
    Ok(())
}

fn spawn_bounded_pipe_reader<R: Read + Send + 'static>(
    reader_opt: Option<R>,
    max_bytes: usize,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::sync_channel(1);
    if let Some(mut reader) = reader_opt {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut limited = reader.by_ref().take(max_bytes as u64);
            let _ = limited.read_to_end(&mut buf);
            // Drain any remaining bytes up to a cap so child doesn't SIGPIPE prematurely while still bounded
            let mut sink = [0u8; 4096];
            while let Ok(n) = reader.read(&mut sink) {
                if n == 0 {
                    break;
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
        });
    } else {
        let _ = tx.send(String::new());
    }
    rx
}

fn reap_process_group(pgid: i32) {
    #[cfg(unix)]
    if pgid > 0 {
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = pgid;
}

/// Executes an ephemeral script strictly under `HUMAN_ONLY` (`ActorKind::HumanWorkbenchSession`),
/// with AST + staged-file capability scan, `env_clear()`, `setpgid(0, 0)`, RLIMITs, `O_NOFOLLOW`
/// workspace staging, non-blocking bounded pipe readers, and hard `<= 5s` timeout reaping via `kill(-pgid, SIGKILL)`.
pub fn execute_in_ephemeral_tool_jail(
    actor_kind: ActorKind,
    jail_parent_dir: &Path,
    request: &EphemeralToolRequest,
) -> Result<ToolJailAuditReceipt, ToolJailError> {
    // 1. Strict HUMAN_ONLY enforcement (AC-02)
    if actor_kind != ActorKind::HumanWorkbenchSession {
        return Err(ToolJailError::HumanApprovalRequired { actor: actor_kind });
    }

    // 2. Static AST / token capability scan on main script AND all staged files
    scan_ephemeral_script_capabilities(&request.script_source)?;
    validate_staged_files(&request.staged_files)?;

    // 3. M-06: Check ONLY jail_parent_dir itself (never walk concurrent sibling jail-* dirs)
    fs::create_dir_all(jail_parent_dir)
        .map_err(|e| ToolJailError::Runtime(format!("create jail root: {e}")))?;
    let parent_meta = fs::symlink_metadata(jail_parent_dir)
        .map_err(|e| ToolJailError::Runtime(format!("stat jail root: {e}")))?;
    if parent_meta.file_type().is_symlink() || !parent_meta.file_type().is_dir() {
        return Err(ToolJailError::SymlinkOrHardlinkRejected {
            path: jail_parent_dir.display().to_string(),
        });
    }

    let execution_id = Uuid::new_v4().to_string();
    let run_dir = jail_parent_dir.join(format!("jail-{execution_id}"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&run_dir)
            .map_err(|e| ToolJailError::Runtime(format!("create run_dir: {e}")))?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(&run_dir)
            .map_err(|e| ToolJailError::Runtime(format!("create run_dir: {e}")))?;
    }

    let result = run_inside_staged_dir(&execution_id, &run_dir, request);
    let _ = fs::remove_dir_all(&run_dir);
    result
}

fn run_inside_staged_dir(
    execution_id: &str,
    run_dir: &Path,
    request: &EphemeralToolRequest,
) -> Result<ToolJailAuditReceipt, ToolJailError> {
    for sf in &request.staged_files {
        let rel = validate_staged_rel_path(&sf.relative_path)?;
        write_file_nofollow(&run_dir.join(rel), sf.content.as_bytes())?;
    }

    let script_filename = match request.runtime {
        EphemeralRuntime::Sh => "ephemeral.sh",
        EphemeralRuntime::Python3 => "ephemeral.py",
        EphemeralRuntime::Node => "ephemeral.mjs",
    };
    let script_path = run_dir.join(script_filename);
    write_file_nofollow(&script_path, request.script_source.as_bytes())?;
    assert_workspace_no_symlinks_or_hardlinks(run_dir)?;

    let timeout_ms = request.timeout_ms.clamp(100, TOOL_JAIL_MAX_TIMEOUT_MS);
    let timeout_dur = Duration::from_millis(timeout_ms);
    let cpu_limit_secs = (timeout_ms / 1000).max(1) + 1;

    let mut cmd = match request.runtime {
        EphemeralRuntime::Sh => {
            let mut c = Command::new("/bin/sh");
            c.arg(&script_path);
            c
        }
        EphemeralRuntime::Python3 => {
            let mut c = Command::new("python3");
            c.arg("-I").arg("-S").arg(&script_path);
            c
        }
        EphemeralRuntime::Node => {
            let mut c = Command::new("node");
            c.arg("--disallow-code-generation-from-strings")
                .arg(&script_path);
            c
        }
    };

    // C-02: env_clear(), sinkhole proxies, and explicitly clear NO_PROXY/no_proxy
    cmd.current_dir(run_dir)
        .env_clear()
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", run_dir)
        .env("TMPDIR", run_dir)
        .env("LC_ALL", "C")
        .env("HTTP_PROXY", "http://127.0.0.1:0")
        .env("HTTPS_PROXY", "http://127.0.0.1:0")
        .env("ALL_PROXY", "http://127.0.0.1:0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(move || {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                let cpu_lim = libc::rlimit {
                    rlim_cur: cpu_limit_secs as libc::rlim_t,
                    rlim_max: cpu_limit_secs as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_CPU, &cpu_lim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                let fsize_lim = libc::rlimit {
                    rlim_cur: TOOL_JAIL_RLIMIT_FSIZE_BYTES as libc::rlim_t,
                    rlim_max: TOOL_JAIL_RLIMIT_FSIZE_BYTES as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &fsize_lim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                #[cfg(target_os = "linux")]
                {
                    let as_lim = libc::rlimit {
                        rlim_cur: TOOL_JAIL_RLIMIT_AS_BYTES as libc::rlim_t,
                        rlim_max: TOOL_JAIL_RLIMIT_AS_BYTES as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_AS, &as_lim) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }

                    let nproc_lim = libc::rlimit {
                        rlim_cur: TOOL_JAIL_RLIMIT_NPROC as libc::rlim_t,
                        rlim_max: TOOL_JAIL_RLIMIT_NPROC as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_NPROC, &nproc_lim) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }

                #[cfg(not(target_os = "linux"))]
                {
                    let data_lim = libc::rlimit {
                        rlim_cur: TOOL_JAIL_RLIMIT_AS_BYTES as libc::rlim_t,
                        rlim_max: TOOL_JAIL_RLIMIT_AS_BYTES as libc::rlim_t,
                    };
                    let _ = libc::setrlimit(libc::RLIMIT_DATA, &data_lim);
                }
                Ok(())
            });
        }
    }

    let start = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| ToolJailError::Runtime(format!("spawn jailed process: {e}")))?;
    let pgid = i32::try_from(child.id()).unwrap_or(0);

    // C-03: Immediately spawn bounded non-blocking pipe reader threads to prevent pipe buffer deadlock
    let stdout_rx = spawn_bounded_pipe_reader(child.stdout.take(), TOOL_JAIL_MAX_OUTPUT_BYTES);
    let stderr_rx = spawn_bounded_pipe_reader(child.stderr.take(), TOOL_JAIL_MAX_OUTPUT_BYTES);

    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // C-03: Reap entire process group even on normal exit to kill detached background children (`sleep 3600 &`)
                reap_process_group(pgid);
                break status;
            }
            Ok(None) => {
                if start.elapsed() >= timeout_dur {
                    reap_process_group(pgid);
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ToolJailError::TimeoutKilled { pgid, timeout_ms });
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                reap_process_group(pgid);
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolJailError::Runtime(format!("try_wait failed: {e}")));
            }
        }
    };

    let collect_timeout = Duration::from_millis(PIPE_COLLECT_TIMEOUT_MS);
    let stdout = stdout_rx.recv_timeout(collect_timeout).unwrap_or_default();
    let stderr = stderr_rx.recv_timeout(collect_timeout).unwrap_or_default();
    let duration_ms = start.elapsed().as_millis() as u64;
    let script_sha256 = format!("{:x}", Sha256::digest(request.script_source.as_bytes()));

    #[cfg(target_os = "linux")]
    let reported_nproc = TOOL_JAIL_RLIMIT_NPROC;
    #[cfg(not(target_os = "linux"))]
    let reported_nproc = 0u64;

    Ok(ToolJailAuditReceipt {
        execution_id: execution_id.to_string(),
        tool_name: request.tool_name.clone(),
        runtime: request.runtime,
        actor_kind: "HUMAN_WORKBENCH_SESSION".to_string(),
        script_sha256,
        exit_code: exit_status.code().unwrap_or(-1),
        timed_out: false,
        killed_pgid: None,
        duration_ms,
        stdout,
        stderr,
        env_cleared: true,
        network_isolated: false,
        network_proxy_sinkholed: true,
        nofollow_verified: true,
        rlimit_as_bytes: TOOL_JAIL_RLIMIT_AS_BYTES,
        rlimit_nproc: reported_nproc,
    })
}
