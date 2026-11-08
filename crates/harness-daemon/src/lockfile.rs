use std::fs;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};

use chrono::Utc;
use domain::{HarnessError, SafeJsInteger};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_DAEMON_PORT: u16 = 4170;

/// Configuration for the loopback Axum harness daemon (`AS-01.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonConfig {
    pub host: String,
    pub preferred_port: u16,
    pub workspace_root: PathBuf,
}

impl DaemonConfig {
    pub fn loopback_default(workspace_root: PathBuf) -> Self {
        let port = std::env::var("UXUI_OS_DAEMON_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(DEFAULT_DAEMON_PORT);
        Self {
            host: "127.0.0.1".to_string(),
            preferred_port: port,
            workspace_root,
        }
    }
}

/// Discovery metadata persisted atomically to `.cowok-design/daemon.lock` (`AS-01.1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonLockInfo {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    pub started_at: SafeJsInteger,
}

/// Validates that `host` resolves strictly to IPv4 loopback `127.0.0.1`.
pub fn validate_loopback_host(host: &str) -> Result<Ipv4Addr, HarnessError> {
    let trimmed = host.trim();
    if trimmed != "127.0.0.1" {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "LOOPBACK_ONLY_VIOLATION: daemon must bind strictly to 127.0.0.1, got '{trimmed}'"
            ),
        });
    }
    let parsed: IpAddr = trimmed.parse().map_err(|e| HarnessError::ValidationFailed {
        reason: format!("LOOPBACK_ONLY_VIOLATION: invalid IP '{trimmed}': {e}"),
    })?;
    match parsed {
        IpAddr::V4(v4) if v4 == Ipv4Addr::new(127, 0, 0, 1) => Ok(v4),
        _ => Err(HarnessError::ValidationFailed {
            reason: format!("LOOPBACK_ONLY_VIOLATION: non-loopback address '{trimmed}'"),
        }),
    }
}

/// Binds a loopback TCP listener on `127.0.0.1:preferred_port`, automatically falling back
/// to `127.0.0.1:0` if `preferred_port` is already in use (`AddrInUse`), and atomically writes
/// `.cowok-design/daemon.lock` using `openat` + `O_NOFOLLOW` (`AS-01.1`).
pub fn bind_loopback_with_lockfile(
    config: &DaemonConfig,
) -> Result<(TcpListener, DaemonLockInfo, PathBuf), HarnessError> {
    let ip = validate_loopback_host(&config.host)?;
    let canonical_root = validate_and_canonicalize_workspace_root(&config.workspace_root)?;

    let preferred_addr = SocketAddr::new(IpAddr::V4(ip), config.preferred_port);
    let listener = match TcpListener::bind(preferred_addr) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let fallback_addr = SocketAddr::new(IpAddr::V4(ip), 0);
            TcpListener::bind(fallback_addr).map_err(|fe| HarnessError::ValidationFailed {
                reason: format!("DAEMON_BIND_FAILED: fallback bind on 127.0.0.1:0 failed: {fe}"),
            })?
        }
        Err(e) => {
            return Err(HarnessError::ValidationFailed {
                reason: format!("DAEMON_BIND_FAILED on {preferred_addr}: {e}"),
            })
        }
    };

    listener
        .set_nonblocking(true)
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_NONBLOCKING_FAILED: {e}"),
        })?;

    let actual_port = listener
        .local_addr()
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCAL_ADDR_FAILED: {e}"),
        })?
        .port();

    let now_ms = Utc::now().timestamp_millis();
    let lock_info = DaemonLockInfo {
        pid: std::process::id(),
        host: "127.0.0.1".to_string(),
        port: actual_port,
        started_at: SafeJsInteger::new(now_ms)?,
    };

    let lock_path = write_daemon_lock_atomic(&canonical_root, &lock_info)?;
    Ok((listener, lock_info, lock_path))
}

/// Reads and validates `.cowok-design/daemon.lock` inside `workspace_root`.
pub fn read_daemon_lock(workspace_root: &Path) -> Result<DaemonLockInfo, HarnessError> {
    let canonical_root = validate_and_canonicalize_workspace_root(workspace_root)?;
    let uxui_dir = canonical_root.join(".cowok-design");
    reject_if_symlink(&uxui_dir)?;
    let lock_path = uxui_dir.join("daemon.lock");
    reject_if_symlink(&lock_path)?;

    let raw = fs::read_to_string(&lock_path).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("DAEMON_LOCK_READ_ERROR [{}]: {e}", lock_path.display()),
    })?;
    serde_json::from_str::<DaemonLockInfo>(&raw).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("DAEMON_LOCK_PARSE_ERROR [{}]: {e}", lock_path.display()),
    })
}

pub fn validate_and_canonicalize_workspace_root(root: &Path) -> Result<PathBuf, HarnessError> {
    for comp in root.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(HarnessError::PathTraversalDenied {
                path: root.display().to_string(),
            });
        }
    }
    reject_if_symlink(root)?;
    root.canonicalize().map_err(|_| HarnessError::PathTraversalDenied {
        path: root.display().to_string(),
    })
}

pub fn reject_if_symlink(path: &Path) -> Result<(), HarnessError> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(HarnessError::PathTraversalDenied {
                path: path.display().to_string(),
            });
        }
    }
    Ok(())
}

fn write_daemon_lock_atomic(
    canonical_root: &Path,
    lock_info: &DaemonLockInfo,
) -> Result<PathBuf, HarnessError> {
    let uxui_dir = canonical_root.join(".cowok-design");
    reject_if_symlink(&uxui_dir)?;
    fs::create_dir_all(&uxui_dir).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("DAEMON_LOCK_DIR_CREATE_FAILED [{}]: {e}", uxui_dir.display()),
    })?;
    reject_if_symlink(&uxui_dir)?;

    let json_bytes = serde_json::to_vec_pretty(lock_info).map_err(|e| {
        HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCK_SERIALIZE_FAILED: {e}"),
        }
    })?;

    #[cfg(unix)]
    {
        write_daemon_lock_unix_nofollow(canonical_root, &json_bytes)?;
    }

    #[cfg(not(unix))]
    {
        let tmp_path = uxui_dir.join(format!(".daemon.lock.tmp.{}", Uuid::new_v4()));
        let target_path = uxui_dir.join("daemon.lock");
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("DAEMON_LOCK_TMP_OPEN_FAILED: {e}"),
            })?;
        f.write_all(&json_bytes).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCK_TMP_WRITE_FAILED: {e}"),
        })?;
        f.sync_all().map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCK_TMP_SYNC_FAILED: {e}"),
        })?;
        drop(f);
        fs::rename(&tmp_path, &target_path).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCK_RENAME_FAILED: {e}"),
        })?;
    }

    Ok(uxui_dir.join("daemon.lock"))
}

#[cfg(unix)]
fn write_daemon_lock_unix_nofollow(
    canonical_root: &Path,
    json_bytes: &[u8],
) -> Result<(), HarnessError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::FromRawFd;

    struct FdGuard(libc::c_int);
    impl Drop for FdGuard {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe {
                    libc::close(self.0);
                }
            }
        }
    }

    let root_c = CString::new(canonical_root.as_os_str().as_bytes()).map_err(|_| {
        HarnessError::ValidationFailed {
            reason: "Invalid NUL byte in workspace_root".to_string(),
        }
    })?;
    let dir_flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let root_fd = unsafe { libc::open(root_c.as_ptr(), dir_flags) };
    if root_fd < 0 {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_root.display().to_string(),
        });
    }
    let root_guard = FdGuard(root_fd);

    let uxui_c = CString::new(".cowok-design").expect("static CString");
    let uxui_fd = unsafe { libc::openat(root_guard.0, uxui_c.as_ptr(), dir_flags) };
    if uxui_fd < 0 {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_root.join(".cowok-design").display().to_string(),
        });
    }
    let uxui_guard = FdGuard(uxui_fd);

    let tmp_name = format!(".daemon.lock.tmp.{}", Uuid::new_v4());
    let tmp_c = CString::new(tmp_name.as_bytes()).expect("valid tmp name");
    let target_c = CString::new("daemon.lock").expect("static target name");

    // Reject if daemon.lock is currently a symlink
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let lstat_rc = unsafe {
        libc::fstatat(
            uxui_guard.0,
            target_c.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if lstat_rc == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_root
                .join(".cowok-design/daemon.lock")
                .display()
                .to_string(),
        });
    }

    let file_flags =
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let tmp_fd = unsafe { libc::openat(uxui_guard.0, tmp_c.as_ptr(), file_flags, 0o600) };
    if tmp_fd < 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "DAEMON_LOCK_TMP_OPENAT_FAILED: {}",
                std::io::Error::last_os_error()
            ),
        });
    }

    let mut tmp_file = unsafe { fs::File::from_raw_fd(tmp_fd) };
    let write_res = (|| -> Result<(), HarnessError> {
        tmp_file
            .write_all(json_bytes)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("DAEMON_LOCK_WRITE_FAILED: {e}"),
            })?;
        tmp_file
            .flush()
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("DAEMON_LOCK_FLUSH_FAILED: {e}"),
            })?;
        tmp_file
            .sync_all()
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("DAEMON_LOCK_FSYNC_FAILED: {e}"),
            })?;
        Ok(())
    })();
    drop(tmp_file);

    if let Err(err) = write_res {
        unsafe {
            libc::unlinkat(uxui_guard.0, tmp_c.as_ptr(), 0);
        }
        return Err(err);
    }

    let rename_rc =
        unsafe { libc::renameat(uxui_guard.0, tmp_c.as_ptr(), uxui_guard.0, target_c.as_ptr()) };
    if rename_rc != 0 {
        let io_err = std::io::Error::last_os_error();
        unsafe {
            libc::unlinkat(uxui_guard.0, tmp_c.as_ptr(), 0);
        }
        return Err(HarnessError::ValidationFailed {
            reason: format!("DAEMON_LOCK_RENAMEAT_FAILED: {io_err}"),
        });
    }

    let dir_sync_rc = unsafe { libc::fsync(uxui_guard.0) };
    if dir_sync_rc != 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "DAEMON_LOCK_DIR_FSYNC_FAILED: {}",
                std::io::Error::last_os_error()
            ),
        });
    }

    Ok(())
}
