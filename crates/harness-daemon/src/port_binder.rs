// @story 12.1
// @implements AC-03 (Bound-FD Zero Port Collision, SIGINT/SIGTERM Graceful Child Reaper & Read-Only FS XDG Fallback)
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

use domain::HarnessError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::lockfile::{reject_if_symlink, DaemonLockInfo};
use crate::routes::execution_reaper::{
    spawn_pgid_child, terminate_pgid_checked, ActiveWorkerRegistry,
};

/// Live OS-bound loopback TCP listener held directly by file descriptor to eliminate
/// TOCTOU port-probe races across concurrent `cowok-design up` instances.
#[derive(Debug)]
pub struct BoundLoopbackListener {
    listener: TcpListener,
    host: String,
    port: u16,
}

impl BoundLoopbackListener {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn local_addr(&self) -> Result<SocketAddr, HarnessError> {
        self.listener
            .local_addr()
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("BOUND_LISTENER_LOCAL_ADDR_FAILED: {e}"),
            })
    }

    pub fn into_std(self) -> TcpListener {
        self.listener
    }

    pub fn into_tokio_listener(self) -> Result<tokio::net::TcpListener, HarnessError> {
        tokio::net::TcpListener::from_std(self.listener).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("BOUND_LISTENER_TOKIO_CONVERSION_FAILED: {e}"),
            }
        })
    }
}

/// Binds a non-blocking loopback (`127.0.0.1`) `TcpListener` without closing or rebinding
/// the underlying file descriptor. When `preferred_port` is `None`, `Some(0)`, or already
/// bound (`AddrInUse`), immediately falls back to `127.0.0.1:0` for an OS-assigned port.
pub fn bind_zero_collision_listener(
    preferred_port: Option<u16>,
) -> Result<BoundLoopbackListener, HarnessError> {
    // Default strictly to loopback (`127.0.0.1`). Inside an isolated OCI bridge container whose
    // host port publish is bound to `127.0.0.1:<port>:<port>`, both explicit opt-in flags allow
    // binding `0.0.0.0` in the container netns so Docker's host loopback forwarder can connect.
    let container_bind_all = std::env::var("COWOK_CONTAINER_MODE").as_deref() == Ok("1")
        && std::env::var("COWOK_CONTAINER_BIND_ALL").as_deref() == Ok("1");
    let bind_ip = if container_bind_all {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    };
    let requested = preferred_port.unwrap_or(0);

    let listener = if requested == 0 {
        TcpListener::bind(SocketAddr::new(bind_ip, 0)).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("DAEMON_EPHEMERAL_BIND_FAILED: {e}"),
            }
        })?
    } else {
        match TcpListener::bind(SocketAddr::new(bind_ip, requested)) {
            Ok(l) => l,
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::AddrInUse | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                TcpListener::bind(SocketAddr::new(bind_ip, 0)).map_err(|fe| {
                    HarnessError::ValidationFailed {
                        reason: format!(
                            "DAEMON_COLLISION_FALLBACK_BIND_FAILED (preferred={requested}): {fe}"
                        ),
                    }
                })?
            }
            Err(err) => {
                return Err(HarnessError::ValidationFailed {
                    reason: format!("DAEMON_BIND_FAILED (preferred={requested}): {err}"),
                });
            }
        }
    };

    listener
        .set_nonblocking(true)
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_SET_NONBLOCKING_FAILED: {e}"),
        })?;

    let actual_port = listener
        .local_addr()
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCAL_ADDR_FAILED: {e}"),
        })?
        .port();

    Ok(BoundLoopbackListener {
        listener,
        host: "127.0.0.1".to_string(),
        port: actual_port,
    })
}

/// Convenience wrapper matching `impl-plan.md` signature.
pub fn bind_zero_collision_loopback(
    preferred_port: u16,
) -> Result<BoundLoopbackListener, HarnessError> {
    bind_zero_collision_listener(Some(preferred_port))
}

/// Deterministic runtime storage paths with automatic XDG / macOS Application Support fallback
/// when the binary or workspace directory resides on a read-only filesystem (`/opt/homebrew`, `0555`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStoragePaths {
    pub workspace_root: PathBuf,
    pub state_root: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub db_path: PathBuf,
    pub lock_path: PathBuf,
    pub read_only_fallback: bool,
}

fn validate_no_parent_or_symlink(path: &Path) -> Result<(), HarnessError> {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(HarnessError::PathTraversalDenied {
            path: path.display().to_string(),
        });
    }
    reject_if_symlink(path)
}

#[cfg(unix)]
fn set_dir_mode_0700(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_dir_mode_0700(_dir: &Path) {}

/// Atomically probes whether `dir` is writable by creating and immediately unlinking a unique
/// probe file with `O_CREAT | O_EXCL | O_NOFOLLOW` (mode `0o600` on Unix).
pub fn probe_dir_writable_atomic(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    if fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return false;
    }
    let probe_path = dir.join(format!(".cowok-write-probe-{}", Uuid::new_v4().simple()));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    match opts.open(&probe_path) {
        Ok(mut f) => {
            let write_ok = f.write_all(b"1").is_ok();
            drop(f);
            let _ = fs::remove_file(&probe_path);
            write_ok
        }
        Err(_) => false,
    }
}

fn try_prepare_cowok_state_dir(base_dir: &Path) -> bool {
    if validate_no_parent_or_symlink(base_dir).is_err() {
        return false;
    }
    let cowok_dir = base_dir.join(".cowok-design");
    if reject_if_symlink(&cowok_dir).is_err() {
        return false;
    }
    if !cowok_dir.exists() && fs::create_dir_all(&cowok_dir).is_err() {
        return false;
    }
    if reject_if_symlink(&cowok_dir).is_err() {
        return false;
    }
    if !probe_dir_writable_atomic(&cowok_dir) {
        return false;
    }
    set_dir_mode_0700(&cowok_dir);
    true
}

fn resolve_xdg_dirs(fallback_home: Option<&Path>) -> Result<(PathBuf, PathBuf), HarnessError> {
    let home_dir = match fallback_home {
        Some(h) => h.to_path_buf(),
        None => std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            HarnessError::ValidationFailed {
                reason: "XDG_FALLBACK_HOME_UNAVAILABLE: neither XDG_* nor HOME is set".to_string(),
            }
        })?,
    };

    let data_base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                home_dir.join("Library").join("Application Support")
            } else {
                home_dir.join(".local").join("share")
            }
        });

    let cache_base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                home_dir.join("Library").join("Caches")
            } else {
                home_dir.join(".cache")
            }
        });

    let data_root = data_base.join("cowok-design");
    let cache_root = cache_base.join("cowok-design");

    for p in [&data_base, &data_root, &cache_base, &cache_root] {
        validate_no_parent_or_symlink(p)?;
    }
    fs::create_dir_all(&data_root).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("XDG_DATA_DIR_CREATE_FAILED: {e}"),
    })?;
    fs::create_dir_all(&cache_root).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("XDG_CACHE_DIR_CREATE_FAILED: {e}"),
    })?;
    reject_if_symlink(&data_root)?;
    reject_if_symlink(&cache_root)?;
    set_dir_mode_0700(&data_root);
    set_dir_mode_0700(&cache_root);

    let cowok_sub = data_root.join(".cowok-design");
    reject_if_symlink(&cowok_sub)?;
    fs::create_dir_all(&cowok_sub).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("XDG_STATE_SUBDIR_CREATE_FAILED: {e}"),
    })?;
    reject_if_symlink(&cowok_sub)?;
    set_dir_mode_0700(&cowok_sub);

    if !probe_dir_writable_atomic(&cowok_sub) {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "XDG_FALLBACK_NOT_WRITABLE: fallback state directory '{}' is not writable",
                cowok_sub.display()
            ),
        });
    }

    Ok((data_root, cache_root))
}

/// Resolves writable SQLite WAL, lockfile, and cache paths for `binary_or_workspace_dir`.
/// Falls back cleanly to XDG data/cache directories when `binary_or_workspace_dir` is read-only.
pub fn resolve_writable_state_dir(
    binary_or_workspace_dir: &Path,
) -> Result<RuntimeStoragePaths, HarnessError> {
    resolve_runtime_storage_paths(Some(binary_or_workspace_dir), None)
}

/// Resolves runtime storage paths for an optional workspace override and optional explicit home dir.
pub fn resolve_runtime_storage_paths(
    workspace_override: Option<&Path>,
    fallback_home: Option<&Path>,
) -> Result<RuntimeStoragePaths, HarnessError> {
    let raw_workspace = match workspace_override {
        Some(p) => p.to_path_buf(),
        None => std::env::var_os("UXUI_OS_WORKSPACE_ROOT")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
    };
    validate_no_parent_or_symlink(&raw_workspace)?;
    let canonical_workspace =
        raw_workspace
            .canonicalize()
            .map_err(|_| HarnessError::PathTraversalDenied {
                path: raw_workspace.display().to_string(),
            })?;

    if try_prepare_cowok_state_dir(&canonical_workspace) {
        let cowok_dir = canonical_workspace.join(".cowok-design");
        let cache_dir = cowok_dir.join("cache");
        let _ = fs::create_dir_all(&cache_dir);
        set_dir_mode_0700(&cache_dir);
        return Ok(RuntimeStoragePaths {
            workspace_root: canonical_workspace.clone(),
            state_root: canonical_workspace,
            data_dir: cowok_dir.clone(),
            cache_dir,
            db_path: cowok_dir.join("cowok-design.sqlite3"),
            lock_path: cowok_dir.join("daemon.lock"),
            read_only_fallback: false,
        });
    }

    let (xdg_data_root, xdg_cache_root) = resolve_xdg_dirs(fallback_home)?;
    let cowok_dir = xdg_data_root.join(".cowok-design");
    Ok(RuntimeStoragePaths {
        workspace_root: canonical_workspace,
        state_root: xdg_data_root,
        data_dir: cowok_dir.clone(),
        cache_dir: xdg_cache_root,
        db_path: cowok_dir.join("cowok-design.sqlite3"),
        lock_path: cowok_dir.join("daemon.lock"),
        read_only_fallback: true,
    })
}

/// Tracked child process handle owned by `ChildProcessReaper`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapedChildHandle {
    pub pid: u32,
    pub pgid: i32,
    pub start_time: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReapSummary {
    pub children_reaped: usize,
    pub active_workers_reaped: usize,
    pub lockfile_removed: bool,
}

#[derive(Default)]
struct ReaperInner {
    children: Mutex<Vec<ReapedChildHandle>>,
    owned_lock: Mutex<Option<(PathBuf, u32, String)>>,
    active_workers: Mutex<Option<ActiveWorkerRegistry>>,
}

/// Tracks spawned child process groups (Pooled Chromium CDP, BYOS CLI workers) and
/// guarantees deterministic `SIGINT`/`SIGTERM`/Drop cleanup with strict PGID safety guards.
#[derive(Clone, Default)]
pub struct ChildProcessReaper {
    inner: Arc<ReaperInner>,
}

impl ChildProcessReaper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_child(
        &self,
        pid: u32,
        pgid: i32,
        start_time: String,
        label: impl Into<String>,
    ) {
        if let Ok(mut guard) = self.inner.children.lock() {
            guard.push(ReapedChildHandle {
                pid,
                pgid,
                start_time,
                label: label.into(),
            });
        }
    }

    pub fn unregister_child(&self, pid: u32) {
        if let Ok(mut guard) = self.inner.children.lock() {
            guard.retain(|c| c.pid != pid);
        }
    }

    pub fn tracked_count(&self) -> usize {
        self.inner.children.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn set_owned_lockfile(&self, lock_path: PathBuf, pid: u32, daemon_nonce: String) {
        if let Ok(mut guard) = self.inner.owned_lock.lock() {
            *guard = Some((lock_path, pid, daemon_nonce));
        }
    }

    pub fn set_active_workers(&self, workers: ActiveWorkerRegistry) {
        if let Ok(mut guard) = self.inner.active_workers.lock() {
            *guard = Some(workers);
        }
    }

    /// Spawns `cmd` in its own process group (`setpgid(0, 0)`) and registers it for automatic reaping.
    pub fn spawn_managed_child(
        &self,
        cmd: Command,
        label: impl Into<String>,
    ) -> std::io::Result<(Child, ReapedChildHandle)> {
        let (child, pid, pgid, start_time) = spawn_pgid_child(cmd)?;
        let handle = ReapedChildHandle {
            pid,
            pgid,
            start_time,
            label: label.into(),
        };
        self.register_child(
            handle.pid,
            handle.pgid,
            handle.start_time.clone(),
            handle.label.clone(),
        );
        Ok((child, handle))
    }

    /// Safely terminates a tracked child PGID, refusing to signal PGID `<= 1`, PID `0`,
    /// the current daemon PID, or the current daemon process group.
    pub fn terminate_pgid_safe(
        pid: u32,
        pgid: i32,
        expected_start_time: &str,
    ) -> Result<bool, String> {
        let self_pid = std::process::id();
        if pid == 0 || pid == self_pid {
            return Err("REFUSED_SELF_OR_ZERO_PID".to_string());
        }
        if pgid <= 1 {
            return Err("REFUSED_INIT_OR_ZERO_PGID".to_string());
        }
        #[cfg(unix)]
        {
            // SAFETY: getpgrp() is a read-only POSIX query with no memory safety preconditions.
            let self_pgid = unsafe { libc::getpgrp() };
            if pgid == self_pgid {
                return Err("REFUSED_SELF_PGID".to_string());
            }
        }
        terminate_pgid_checked(pid, pgid, expected_start_time)
    }

    /// Terminates all tracked child process groups and removes the owned `daemon.lock`
    /// only when its on-disk PID and `daemon_nonce` match this daemon instance.
    pub fn reap_all(&self) -> ReapSummary {
        let mut summary = ReapSummary::default();
        let tracked: Vec<ReapedChildHandle> = self
            .inner
            .children
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();

        for child in tracked {
            if matches!(
                Self::terminate_pgid_safe(child.pid, child.pgid, &child.start_time),
                Ok(true)
            ) {
                summary.children_reaped += 1;
            }
        }

        if let Some(workers_registry) = self
            .inner
            .active_workers
            .lock()
            .ok()
            .and_then(|g| g.clone())
        {
            for worker in workers_registry.take_all() {
                if matches!(
                    Self::terminate_pgid_safe(worker.pid, worker.pgid, &worker.start_time),
                    Ok(true)
                ) {
                    summary.active_workers_reaped += 1;
                }
            }
        }

        let lock_entry = self.inner.owned_lock.lock().ok().and_then(|mut g| g.take());

        if let Some((lock_path, expected_pid, expected_nonce)) = lock_entry {
            if remove_owned_lockfile_if_matching(&lock_path, expected_pid, &expected_nonce) {
                summary.lockfile_removed = true;
            }
        }

        summary
    }
}

impl Drop for ChildProcessReaper {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            let _ = self.reap_all();
        }
    }
}

fn remove_owned_lockfile_if_matching(
    lock_path: &Path,
    expected_pid: u32,
    expected_nonce: &str,
) -> bool {
    if fs::symlink_metadata(lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return false;
    }
    let Ok(raw) = fs::read_to_string(lock_path) else {
        return false;
    };
    let Ok(info) = serde_json::from_str::<DaemonLockInfo>(&raw) else {
        return false;
    };
    if info.pid == expected_pid && info.daemon_nonce == expected_nonce {
        return fs::remove_file(lock_path).is_ok();
    }
    false
}

/// Waits asynchronously for either `SIGINT` (`Ctrl-C`) or Unix `SIGTERM`.
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = async {
                if let Some(ref mut s) = sigterm {
                    s.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
