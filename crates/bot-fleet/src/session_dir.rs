// @story 11.1
// @implements AC-04 (Zero-Global-Pollution $COWOK_SESSION_DIR with O_NOFOLLOW, fchmod 0o600 & RAII Drop)
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::BotFleetError;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};

const SESSION_SUBDIRS: &[&str] = &[
    "xdg_config",
    "xdg_config/claude",
    "xdg_config/gemini",
    "xdg_config/codex",
    "xdg_cache",
    "mcp",
    "prompts",
];

pub fn validate_session_id(raw: &str) -> Result<String, BotFleetError> {
    validate_segment(raw, "session_id")
}

fn validate_segment(raw: &str, label: &str) -> Result<String, BotFleetError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    {
        return Err(BotFleetError::InvalidSessionSegment(format!(
            "Unsafe session segment for {label}"
        )));
    }
    Ok(trimmed.to_string())
}

fn ensure_not_symlink(path: &Path) -> Result<(), BotFleetError> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(BotFleetError::SymlinkViolation(format!(
                "Refusing symlink path '{}'",
                path.display()
            )));
        }
    }
    Ok(())
}

fn create_dir_0700(path: &Path, recursive: bool) -> Result<(), BotFleetError> {
    let mut builder = DirBuilder::new();
    builder.recursive(recursive);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(path).map_err(|e| {
        BotFleetError::Io(format!(
            "Failed creating 0o700 directory '{}': {e}",
            path.display()
        ))
    })
}

#[cfg(unix)]
fn verify_owned_by_current_euid(path: &Path) -> Result<(), BotFleetError> {
    let meta = fs::symlink_metadata(path).map_err(|e| {
        BotFleetError::Io(format!(
            "Failed inspecting directory ownership '{}': {e}",
            path.display()
        ))
    })?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(BotFleetError::SymlinkViolation(format!(
            "Session base path '{}' is not a real directory",
            path.display()
        )));
    }
    // SAFETY: geteuid() is always safe and side-effect free.
    let euid = unsafe { libc::geteuid() };
    if meta.uid() != euid {
        return Err(BotFleetError::PermissionViolation(format!(
            "Directory '{}' is owned by uid {}, expected current euid {}",
            path.display(),
            meta.uid(),
            euid
        )));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn open_dir_nofollow(dir: &Path) -> Result<OwnedFd, BotFleetError> {
    ensure_not_symlink(dir)?;
    let c_dir = CString::new(dir.as_os_str().as_bytes())
        .map_err(|_| BotFleetError::InvalidPath("Path contains NUL byte".to_string()))?;
    // SAFETY: c_dir is a valid NUL-terminated CString; open with O_NOFOLLOW | O_DIRECTORY | O_CLOEXEC.
    let fd = unsafe {
        libc::open(
            c_dir.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ELOOP) {
            return Err(BotFleetError::SymlinkViolation(format!(
                "O_NOFOLLOW rejected symlink directory '{}'",
                dir.display()
            )));
        }
        return Err(BotFleetError::Io(format!(
            "Failed opening directory '{}' with O_NOFOLLOW: {err}",
            dir.display()
        )));
    }
    // SAFETY: fd is a freshly opened valid file descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[cfg(unix)]
pub(crate) fn create_file_0600_nofollow(
    dir: &Path,
    filename: &str,
    bytes: &[u8],
) -> Result<PathBuf, BotFleetError> {
    let safe_name = validate_segment(filename, "filename")?;
    let target_path = dir.join(&safe_name);
    ensure_not_symlink(&target_path)?;

    let dir_fd = open_dir_nofollow(dir)?;
    let c_name = CString::new(safe_name.as_bytes())
        .map_err(|_| BotFleetError::InvalidPath("Filename contains NUL byte".to_string()))?;

    // SAFETY: dir_fd is a valid O_NOFOLLOW directory descriptor; openat enforces O_EXCL | O_NOFOLLOW | O_CLOEXEC | 0o600.
    let fd = unsafe {
        libc::openat(
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ELOOP) {
            return Err(BotFleetError::SymlinkViolation(format!(
                "O_NOFOLLOW rejected symlink target '{}'",
                target_path.display()
            )));
        }
        return Err(BotFleetError::Io(format!(
            "Failed creating 0o600 file '{}' via openat(O_NOFOLLOW): {err}",
            target_path.display()
        )));
    }

    // SAFETY: fd is a valid owned file descriptor from openat.
    let mut file = unsafe { File::from_raw_fd(fd) };
    // Enforce 0o600 directly on the open file descriptor (fchmod, zero path TOCTOU).
    if let Err(err) = file
        .set_permissions(fs::Permissions::from_mode(0o600))
        .and_then(|_| file.write_all(bytes))
        .and_then(|_| file.sync_all())
    {
        drop(file);
        // SAFETY: unlink freshly created temp file on permission/write failure.
        unsafe {
            libc::unlinkat(dir_fd.as_raw_fd(), c_name.as_ptr(), 0);
        }
        return Err(BotFleetError::Io(format!(
            "Failed writing/chmod 0o600 file '{}': {err}",
            target_path.display()
        )));
    }
    Ok(target_path)
}

#[cfg(not(unix))]
pub(crate) fn create_file_0600_nofollow(
    dir: &Path,
    filename: &str,
    bytes: &[u8],
) -> Result<PathBuf, BotFleetError> {
    let safe_name = validate_segment(filename, "filename")?;
    ensure_not_symlink(dir)?;
    let target_path = dir.join(&safe_name);
    ensure_not_symlink(&target_path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target_path)
        .map_err(|e| {
            BotFleetError::Io(format!(
                "Failed creating file '{}': {e}",
                target_path.display()
            ))
        })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&target_path);
            BotFleetError::Io(format!(
                "Failed writing file '{}': {e}",
                target_path.display()
            ))
        })?;
    Ok(target_path)
}

/// Zeroizes a regular file's contents in place (using `O_NOFOLLOW` + open-FD regular-file check)
/// before unlinking it.
pub(crate) fn zeroize_and_unlink(path: &Path) {
    let mut opts = OpenOptions::new();
    opts.write(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);

    if let Ok(mut file) = opts.open(path) {
        if let Ok(meta) = file.metadata() {
            if meta.file_type().is_file() {
                let len = meta.len() as usize;
                if len > 0 {
                    let zeros = vec![0u8; len.min(64 * 1024)];
                    let mut remaining = len;
                    while remaining > 0 {
                        let chunk = remaining.min(zeros.len());
                        if file.write_all(&zeros[..chunk]).is_err() {
                            break;
                        }
                        remaining -= chunk;
                    }
                    let _ = file.sync_all();
                }
            }
        }
        drop(file);
        let _ = fs::remove_file(path);
    }
}

/// Isolated `$COWOK_SESSION_DIR` (`0o700`) with `O_NOFOLLOW` traversal checks and RAII Drop cleanup.
/// Guarantees Zero-Global-Pollution: child CLI tools never read/write `~/.claude`, `~/.gemini`, or `~/.codex`.
#[derive(Debug)]
pub struct IsolatedSessionDir {
    root: PathBuf,
    session_id: String,
    tracked_secrets: Arc<Mutex<Vec<PathBuf>>>,
}

impl IsolatedSessionDir {
    /// Creates a new isolated `0o700` session directory under `base_root`.
    pub fn create(base_root: &Path, session_id: &str) -> Result<Self, BotFleetError> {
        let safe_session = validate_segment(session_id, "session_id")?;
        ensure_not_symlink(base_root)?;
        if !base_root.exists() {
            create_dir_0700(base_root, true)?;
        }
        ensure_not_symlink(base_root)?;

        #[cfg(unix)]
        {
            verify_owned_by_current_euid(base_root)?;
            let _ = open_dir_nofollow(base_root)?;
        }

        let dir_name = format!("sess-{safe_session}-{}", Uuid::new_v4().simple());
        let root = base_root.join(&dir_name);
        ensure_not_symlink(&root)?;
        create_dir_0700(&root, false)?;

        #[cfg(unix)]
        {
            let _ = open_dir_nofollow(&root)?;
        }

        for sub in SESSION_SUBDIRS {
            let sub_path = root.join(sub);
            ensure_not_symlink(&sub_path)?;
            create_dir_0700(&sub_path, true)?;
        }

        Ok(Self {
            root,
            session_id: safe_session,
            tracked_secrets: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Creates an isolated session directory inside a per-UID subdirectory of OS temp dir.
    pub fn create_ephemeral(session_id: &str) -> Result<Self, BotFleetError> {
        #[cfg(unix)]
        let uid_suffix = {
            // SAFETY: geteuid() is always safe.
            let euid = unsafe { libc::geteuid() };
            format!("cowok-bot-fleet-sessions-{euid}")
        };
        #[cfg(not(unix))]
        let uid_suffix = "cowok-bot-fleet-sessions".to_string();

        let base = std::env::temp_dir().join(uid_suffix);
        Self::create(&base, session_id)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn prompts_dir(&self) -> PathBuf {
        self.root.join("prompts")
    }

    pub fn mcp_dir(&self) -> PathBuf {
        self.root.join("mcp")
    }

    pub fn xdg_config_dir(&self) -> PathBuf {
        self.root.join("xdg_config")
    }

    pub fn xdg_cache_dir(&self) -> PathBuf {
        self.root.join("xdg_cache")
    }

    /// Validates every path component beneath `$COWOK_SESSION_DIR` with `O_NOFOLLOW` symlink rejection.
    pub fn validate_rel_path_nofollow(&self, rel: &Path) -> Result<PathBuf, BotFleetError> {
        let mut cursor = self.root.clone();
        #[cfg(unix)]
        {
            let _ = open_dir_nofollow(&cursor)?;
        }
        for comp in rel.components() {
            match comp {
                Component::Normal(seg) => {
                    cursor.push(seg);
                    ensure_not_symlink(&cursor)?;
                    #[cfg(unix)]
                    if cursor.is_dir() {
                        let _ = open_dir_nofollow(&cursor)?;
                    }
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(BotFleetError::PathTraversal(format!(
                        "Refusing path traversal outside $COWOK_SESSION_DIR: '{}'",
                        rel.display()
                    )));
                }
            }
        }
        Ok(cursor)
    }

    /// Writes an isolated `0o600` file inside a validated subdirectory (`mcp`, `prompts`, `xdg_config`).
    pub fn write_isolated_file_0600(
        &self,
        subdir: &str,
        filename: &str,
        content: &[u8],
    ) -> Result<PathBuf, BotFleetError> {
        let target_dir = self.validate_rel_path_nofollow(Path::new(subdir))?;
        let created = create_file_0600_nofollow(&target_dir, filename, content)?;
        if let Ok(mut list) = self.tracked_secrets.lock() {
            list.push(created.clone());
        }
        Ok(created)
    }

    /// Writes an isolated MCP config file inside `$COWOK_SESSION_DIR/mcp/mcp_config.json` (`0o600`).
    pub fn inject_mcp_config(
        &self,
        config_json: &serde_json::Value,
    ) -> Result<PathBuf, BotFleetError> {
        let bytes = serde_json::to_vec_pretty(config_json)
            .map_err(|e| BotFleetError::Serialization(e.to_string()))?;
        self.write_isolated_file_0600("mcp", "mcp_config.json", &bytes)
    }

    /// Returns isolated environment overrides ensuring zero writes to `~/.claude`, `~/.gemini`, or `~/.codex`.
    pub fn isolated_env(&self) -> Vec<(String, String)> {
        let xdg_cfg = self.xdg_config_dir();
        let xdg_cache = self.xdg_cache_dir();
        let mcp_cfg = self.mcp_dir().join("mcp_config.json");
        vec![
            (
                "COWOK_SESSION_DIR".to_string(),
                self.root.to_string_lossy().to_string(),
            ),
            (
                "XDG_CONFIG_HOME".to_string(),
                xdg_cfg.to_string_lossy().to_string(),
            ),
            (
                "XDG_CACHE_HOME".to_string(),
                xdg_cache.to_string_lossy().to_string(),
            ),
            (
                "CLAUDE_CONFIG_DIR".to_string(),
                xdg_cfg.join("claude").to_string_lossy().to_string(),
            ),
            (
                "GEMINI_CONFIG_DIR".to_string(),
                xdg_cfg.join("gemini").to_string_lossy().to_string(),
            ),
            (
                "CODEX_HOME".to_string(),
                xdg_cfg.join("codex").to_string_lossy().to_string(),
            ),
            (
                "COWOK_MCP_CONFIG".to_string(),
                mcp_cfg.to_string_lossy().to_string(),
            ),
            ("CI".to_string(), "1".to_string()),
            ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
            ("TERM".to_string(), "dumb".to_string()),
        ]
    }
}

impl Drop for IsolatedSessionDir {
    fn drop(&mut self) {
        if let Ok(mut list) = self.tracked_secrets.lock() {
            for file_path in list.drain(..) {
                zeroize_and_unlink(&file_path);
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
