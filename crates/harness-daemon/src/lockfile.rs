// @story 04.1
// @implements AC-01, AC-06
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};

use chrono::Utc;
use domain::{HarnessError, SafeJsInteger};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_DAEMON_PORT: u16 = 4170;
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonLockInfo {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    pub started_at: SafeJsInteger,
    #[serde(default = "generate_csprng_hex_32")]
    pub daemon_nonce: String,
    #[serde(default = "generate_csprng_hex_32")]
    pub hmac_key_hex: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

pub fn generate_csprng_hex_32() -> String {
    let mut buf = [0u8; 32];
    if fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_err()
    {
        buf[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        buf[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn validate_loopback_host(host: &str) -> Result<Ipv4Addr, HarnessError> {
    let trimmed = host.trim();
    if trimmed != "127.0.0.1" {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "LOOPBACK_ONLY_VIOLATION: daemon must bind strictly to 127.0.0.1, got '{trimmed}'"
            ),
        });
    }
    match trimmed.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) if v4 == Ipv4Addr::new(127, 0, 0, 1) => Ok(v4),
        _ => Err(HarnessError::ValidationFailed {
            reason: format!("LOOPBACK_ONLY_VIOLATION: '{trimmed}'"),
        }),
    }
}

pub fn bind_loopback_with_lockfile(
    config: &DaemonConfig,
) -> Result<(TcpListener, DaemonLockInfo, PathBuf), HarnessError> {
    let ip = validate_loopback_host(&config.host)?;
    let canonical_root = validate_and_canonicalize_workspace_root(&config.workspace_root)?;
    reap_dead_lockfile_if_stale(&canonical_root);

    let preferred_addr = SocketAddr::new(IpAddr::V4(ip), config.preferred_port);
    let listener = match TcpListener::bind(preferred_addr) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            TcpListener::bind(SocketAddr::new(IpAddr::V4(ip), 0)).map_err(|fe| {
                HarnessError::ValidationFailed {
                    reason: format!(
                        "DAEMON_BIND_FAILED: fallback bind on 127.0.0.1:0 failed: {fe}"
                    ),
                }
            })?
        }
        Err(e) => {
            return Err(HarnessError::ValidationFailed {
                reason: format!("DAEMON_BIND_FAILED: {e}"),
            })
        }
    };
    listener
        .set_nonblocking(true)
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("NONBLOCKING: {e}"),
        })?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("LOCAL_ADDR: {e}"),
        })?
        .port();
    let lock_info = DaemonLockInfo {
        pid: std::process::id(),
        host: "127.0.0.1".to_string(),
        port: actual_port,
        started_at: SafeJsInteger::new(Utc::now().timestamp_millis())?,
        daemon_nonce: generate_csprng_hex_32(),
        hmac_key_hex: generate_csprng_hex_32(),
        schema_version: CURRENT_SCHEMA_VERSION,
    };
    let lock_path = write_daemon_lock_atomic(&canonical_root, &lock_info)?;
    Ok((listener, lock_info, lock_path))
}

fn reap_dead_lockfile_if_stale(canonical_root: &Path) {
    let lock_path = canonical_root.join(".cowok-design/daemon.lock");
    if let Ok(raw) = fs::read_to_string(&lock_path) {
        if let Ok(info) = serde_json::from_str::<DaemonLockInfo>(&raw) {
            #[cfg(unix)]
            if unsafe { libc::kill(info.pid as libc::pid_t, 0) } != 0
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                let _ = fs::remove_file(&lock_path);
            }
        }
    }
}

pub fn read_daemon_lock(workspace_root: &Path) -> Result<DaemonLockInfo, HarnessError> {
    let canonical_root = validate_and_canonicalize_workspace_root(workspace_root)?;
    let uxui_dir = canonical_root.join(".cowok-design");
    reject_if_symlink(&uxui_dir)?;
    let lock_path = uxui_dir.join("daemon.lock");
    reject_if_symlink(&lock_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = fs::symlink_metadata(&lock_path) {
            if meta.uid() != unsafe { libc::geteuid() } {
                return Err(HarnessError::BoundaryViolation {
                    reason: "UNAUTHORIZED: daemon.lock uid mismatch".into(),
                });
            }
        }
    }
    let raw = fs::read_to_string(&lock_path).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("DAEMON_LOCK_READ_ERROR: {e}"),
    })?;
    serde_json::from_str::<DaemonLockInfo>(&raw).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("DAEMON_LOCK_PARSE_ERROR: {e}"),
    })
}

pub fn validate_and_canonicalize_workspace_root(root: &Path) -> Result<PathBuf, HarnessError> {
    if root
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(HarnessError::PathTraversalDenied {
            path: root.display().to_string(),
        });
    }
    reject_if_symlink(root)?;
    root.canonicalize()
        .map_err(|_| HarnessError::PathTraversalDenied {
            path: root.display().to_string(),
        })
}

pub fn reject_if_symlink(path: &Path) -> Result<(), HarnessError> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(HarnessError::PathTraversalDenied {
            path: path.display().to_string(),
        });
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
        reason: format!("DIR_CREATE: {e}"),
    })?;
    reject_if_symlink(&uxui_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&uxui_dir, fs::Permissions::from_mode(0o700));
    }
    let json_bytes =
        serde_json::to_vec_pretty(lock_info).map_err(|e| HarnessError::ValidationFailed {
            reason: e.to_string(),
        })?;
    #[cfg(unix)]
    write_daemon_lock_unix_nofollow(canonical_root, &json_bytes)?;
    Ok(uxui_dir.join("daemon.lock"))
}

#[cfg(unix)]
fn write_daemon_lock_unix_nofollow(
    canonical_root: &Path,
    json_bytes: &[u8],
) -> Result<(), HarnessError> {
    use std::ffi::CString;
    use std::os::unix::{ffi::OsStrExt, io::FromRawFd};
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
            reason: "NUL".into(),
        }
    })?;
    let dflags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let root_fd = unsafe { libc::open(root_c.as_ptr(), dflags) };
    if root_fd < 0 {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_root.display().to_string(),
        });
    }
    let root_g = FdGuard(root_fd);
    let uxui_c = CString::new(".cowok-design").expect("static");
    let uxui_fd = unsafe { libc::openat(root_g.0, uxui_c.as_ptr(), dflags) };
    if uxui_fd < 0 {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_root.join(".cowok-design").display().to_string(),
        });
    }
    let uxui_g = FdGuard(uxui_fd);

    let tmp_c =
        CString::new(format!(".daemon.lock.tmp.{}", Uuid::new_v4()).into_bytes()).expect("tmp");
    let target_c = CString::new("daemon.lock").expect("target");
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe {
        libc::fstatat(
            uxui_g.0,
            target_c.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
        && (st.st_mode & libc::S_IFMT) == libc::S_IFLNK
    {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_root
                .join(".cowok-design/daemon.lock")
                .display()
                .to_string(),
        });
    }
    let fflags = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let tmp_fd = unsafe { libc::openat(uxui_g.0, tmp_c.as_ptr(), fflags, 0o600) };
    if tmp_fd < 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!("TMP_OPENAT: {}", std::io::Error::last_os_error()),
        });
    }
    let mut tmp_file = unsafe { fs::File::from_raw_fd(tmp_fd) };
    let res = tmp_file
        .write_all(json_bytes)
        .and_then(|_| tmp_file.flush())
        .and_then(|_| tmp_file.sync_all());
    drop(tmp_file);
    if let Err(e) = res {
        unsafe {
            libc::unlinkat(uxui_g.0, tmp_c.as_ptr(), 0);
        }
        return Err(HarnessError::ValidationFailed {
            reason: e.to_string(),
        });
    }
    if unsafe { libc::renameat(uxui_g.0, tmp_c.as_ptr(), uxui_g.0, target_c.as_ptr()) } != 0 {
        unsafe {
            libc::unlinkat(uxui_g.0, tmp_c.as_ptr(), 0);
        }
        return Err(HarnessError::ValidationFailed {
            reason: "RENAMEAT_FAILED".into(),
        });
    }
    let _ = unsafe { libc::fsync(uxui_g.0) };
    Ok(())
}
