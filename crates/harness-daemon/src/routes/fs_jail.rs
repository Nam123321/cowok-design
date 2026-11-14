// @story 04.2
// @implements AC-04
use std::ffi::CString;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use chrono::Utc;
use domain::HarnessError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::legacy_specs::verify_anti_idor_with_caller;
use super::{
    constant_time_eq, decode_hex, extract_actor_id, hmac_sha256, map_error_to_response,
    parse_project_uuid, validate_and_normalize_relative_path, IssuePathTokenRequest,
    PathTokenPayload, PathTokenResponse, VerifyPathTokenRequest,
};
use crate::daemon_auth::VerifiedLegacyIngress;
use crate::routes::DaemonAppState;

#[derive(Debug, Clone, Deserialize)]
pub struct FsBrowseQuery {
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsBrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_git_repo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsBrowseResponse {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub managed_root: String,
    pub entries: Vec<FsBrowseEntry>,
}

#[derive(Debug, Clone)]
pub struct JailAnchor {
    pub canonical_path: PathBuf,
    pub ancestor_dev: u64,
    pub ancestor_ino: u64,
    pub ancestor_dev_ino: String,
    pub target_exists: bool,
    pub remaining_components: Vec<String>,
}

pub fn default_managed_root() -> Result<PathBuf, HarnessError> {
    if let Ok(override_root) = std::env::var("COWOK_PROJECTS_ROOT") {
        let trimmed = override_root.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let home = std::env::var("HOME").map_err(|_| HarnessError::BoundaryViolation {
        reason: "FS_JAIL_VIOLATION: HOME environment variable is unset".to_string(),
    })?;
    Ok(PathBuf::from(home).join("CowokProjects"))
}

pub fn validate_managed_slug(slug: &str) -> Result<(), HarnessError> {
    let bytes = slug.as_bytes();
    let valid_len = !bytes.is_empty() && bytes.len() <= 63;
    let valid_first = bytes
        .first()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let valid_rest = bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_');
    if !(valid_len && valid_first && valid_rest) {
        return Err(HarnessError::ValidationFailed {
            reason: "INVALID_MANAGED_SLUG: Slug must match ^[a-z0-9][a-z0-9_-]{0,62}$".to_string(),
        });
    }
    Ok(())
}

fn has_sensitive_dot_component(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(
            c,
            Component::Normal(s)
                if matches!(
                    s.to_string_lossy().to_lowercase().as_str(),
                    ".cowok-design" | ".ssh" | ".gnupg" | ".aws" | ".kube"
                )
        )
    })
}

pub fn is_sensitive_or_system_path(path: &Path) -> bool {
    if has_sensitive_dot_component(path) {
        return true;
    }
    let raw = path.to_string_lossy();
    let norm = raw.trim_end_matches('/');
    let norm = if norm.is_empty() { "/" } else { norm };
    let lower = norm.to_lowercase();
    if lower == "/private/tmp"
        || lower.starts_with("/private/tmp/")
        || lower == "/private/var/folders"
        || lower.starts_with("/private/var/folders/")
        || lower == "/tmp"
        || lower.starts_with("/tmp/")
        || lower == "/var/folders"
        || lower.starts_with("/var/folders/")
    {
        return false;
    }
    for blocked in [
        "/",
        "/etc",
        "/usr",
        "/bin",
        "/sbin",
        "/var",
        "/private",
        "/private/var",
        "/system",
        "/library",
        "/applications",
        "/volumes",
        "/boot",
        "/dev",
        "/proc",
        "/sys",
    ] {
        if lower == blocked {
            return true;
        }
    }
    for prefix in [
        "/etc/",
        "/bin/",
        "/sbin/",
        "/var/",
        "/private/etc",
        "/private/var/",
        "/system/",
        "/library/",
        "/applications/",
        "/volumes/",
        "/dev/",
        "/proc/",
        "/sys/",
    ] {
        let trimmed = prefix.trim_end_matches('/');
        if lower == trimmed || lower.starts_with(&format!("{trimmed}/")) {
            return true;
        }
    }
    if lower.starts_with("/usr/") && !lower.starts_with("/usr/local/") {
        return true;
    }
    if is_sensitive_home_subdir(&lower) {
        return true;
    }
    false
}

fn is_sensitive_home_subdir(lower: &str) -> bool {
    let Ok(home) = std::env::var("HOME") else {
        return false;
    };
    let trimmed = home.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return false;
    }
    let home_canon = fs::canonicalize(trimmed).unwrap_or_else(|_| PathBuf::from(trimmed));
    for candidate in [
        trimmed.to_lowercase(),
        home_canon.to_string_lossy().to_lowercase(),
    ] {
        for sub in ["/library", "/.ssh", "/.gnupg", "/.aws", "/.kube"] {
            let full = format!("{candidate}{sub}");
            if lower == full || lower.starts_with(&format!("{full}/")) {
                return true;
            }
        }
    }
    false
}

pub fn is_exact_home_dir(path: &Path) -> bool {
    if let Ok(home) = std::env::var("HOME") {
        let home_norm = home.trim_end_matches('/');
        if !home_norm.is_empty() {
            let path_norm = path.to_string_lossy();
            let path_trimmed = path_norm.trim_end_matches('/');
            if path_trimmed.eq_ignore_ascii_case(home_norm) {
                return true;
            }
            if let Ok(home_canon) = fs::canonicalize(&home) {
                let p_canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
                if p_canon == home_canon
                    || p_canon
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&home_canon.to_string_lossy())
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn ensure_writable_target_not_home(path: &Path) -> Result<(), HarnessError> {
    if is_exact_home_dir(path) {
        return Err(HarnessError::BoundaryViolation {
            reason: format!(
                "FS_JAIL_VIOLATION:SYSTEM_PATH_DENIED: Cannot target HOME root '{}'",
                path.display()
            ),
        });
    }
    Ok(())
}

fn normalize_macos_root_alias(abs_path: &Path) -> Result<PathBuf, HarnessError> {
    let mut comps = abs_path.components();
    let Some(Component::RootDir) = comps.next() else {
        return Err(HarnessError::PathTraversalDenied {
            path: abs_path.display().to_string(),
        });
    };
    let rest: Vec<&std::ffi::OsStr> = comps
        .map(|c| match c {
            Component::Normal(s) => Ok(s),
            _ => Err(HarnessError::PathTraversalDenied {
                path: abs_path.display().to_string(),
            }),
        })
        .collect::<Result<_, _>>()?;
    if let Some(first) = rest.first() {
        let root_child = Path::new("/").join(first);
        if fs::symlink_metadata(&root_child)
            .ok()
            .is_some_and(|m| m.file_type().is_symlink())
            && matches!(first.to_str(), Some("tmp" | "var" | "etc"))
        {
            if let Ok(canon_top) = fs::canonicalize(&root_child) {
                let mut rebuilt = canon_top;
                for seg in rest.iter().skip(1) {
                    rebuilt.push(seg);
                }
                return Ok(rebuilt);
            }
        }
    }
    Ok(abs_path.to_path_buf())
}

fn open_root_fd() -> Result<OwnedFd, HarnessError> {
    let root_c = CString::new("/").expect("static cstr");
    let fd = unsafe {
        libc::open(
            root_c.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(HarnessError::ValidationFailed {
            reason: "FS_ROOT_OPEN_FAILED".to_string(),
        });
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn is_regular_file_nofollow(parent_fd: i32, name: &std::ffi::OsStr) -> bool {
    let Ok(c_name) = CString::new(name.as_bytes()) else {
        return false;
    };
    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::fstatat(
            parent_fd,
            c_name.as_ptr(),
            &mut stat_buf,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    rc == 0 && (stat_buf.st_mode & libc::S_IFMT) == libc::S_IFREG
}

fn walk_nofollow_from_root(
    normalized: &Path,
    allow_leaf_file: bool,
) -> Result<(OwnedFd, PathBuf, Vec<String>, bool), HarnessError> {
    let mut cur_fd = open_root_fd()?;
    let mut existing = PathBuf::from("/");
    let mut remaining = Vec::new();
    let mut hit_missing = false;
    let mut leaf_is_file = false;
    let comps: Vec<&std::ffi::OsStr> = normalized
        .components()
        .filter_map(|c| match c {
            Component::Normal(seg) => Some(seg),
            _ => None,
        })
        .collect();
    for (idx, seg) in comps.iter().enumerate() {
        if hit_missing {
            remaining.push(seg.to_string_lossy().into_owned());
            continue;
        }
        let c_name =
            CString::new(seg.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
                path: seg.to_string_lossy().into_owned(),
            })?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = unsafe { libc::openat(cur_fd.as_raw_fd(), c_name.as_ptr(), flags) };
        if fd >= 0 {
            cur_fd = unsafe { OwnedFd::from_raw_fd(fd) };
            existing.push(seg);
            continue;
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::ENOENT) => {
                hit_missing = true;
                remaining.push(seg.to_string_lossy().into_owned());
            }
            Some(libc::ENOTDIR)
                if allow_leaf_file
                    && idx + 1 == comps.len()
                    && is_regular_file_nofollow(cur_fd.as_raw_fd(), seg) =>
            {
                leaf_is_file = true;
                existing.push(seg);
            }
            Some(libc::ELOOP) => {
                return Err(HarnessError::BoundaryViolation {
                    reason: format!(
                        "FS_JAIL_VIOLATION: Symlink traversal denied at '{}'",
                        seg.to_string_lossy()
                    ),
                });
            }
            _ => {
                return Err(HarnessError::BoundaryViolation {
                    reason: format!(
                        "FS_JAIL_VIOLATION: Cannot open directory '{}': {err}",
                        seg.to_string_lossy()
                    ),
                });
            }
        }
    }
    let canon_existing = if leaf_is_file {
        let parent = existing.parent().unwrap_or_else(|| Path::new("/"));
        let leaf = existing.file_name().map(PathBuf::from).unwrap_or_default();
        fs::canonicalize(parent)
            .unwrap_or_else(|_| parent.to_path_buf())
            .join(leaf)
    } else {
        fs::canonicalize(&existing).unwrap_or(existing)
    };
    let mut candidate = canon_existing;
    for seg in &remaining {
        candidate.push(seg);
    }
    Ok((cur_fd, candidate, remaining, leaf_is_file))
}

fn open_dir_fd_nofollow(
    parent_fd: i32,
    name: &std::ffi::OsStr,
) -> Result<Option<OwnedFd>, HarnessError> {
    let c_name = CString::new(name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
        path: name.to_string_lossy().into_owned(),
    })?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let fd = unsafe { libc::openat(parent_fd, c_name.as_ptr(), flags) };
    if fd >= 0 {
        return Ok(Some(unsafe { OwnedFd::from_raw_fd(fd) }));
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ENOENT) => Ok(None),
        Some(libc::ELOOP) => Err(HarnessError::BoundaryViolation {
            reason: format!(
                "FS_JAIL_VIOLATION: Symlink traversal denied at '{}'",
                name.to_string_lossy()
            ),
        }),
        _ => Err(HarnessError::BoundaryViolation {
            reason: format!(
                "FS_JAIL_VIOLATION: Cannot open directory '{}': {err}",
                name.to_string_lossy()
            ),
        }),
    }
}

fn fstat_dev_ino(fd: &OwnedFd) -> Result<(u64, u64), HarnessError> {
    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd.as_raw_fd(), &mut stat_buf) } != 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!("FS_STAT_FAILED: {}", std::io::Error::last_os_error()),
        });
    }
    Ok((stat_buf.st_dev as u64, stat_buf.st_ino as u64))
}

pub fn inspect_path_jail(raw_path: &Path) -> Result<JailAnchor, HarnessError> {
    let s = raw_path.to_string_lossy();
    if s.trim().is_empty() || s.contains('\0') {
        return Err(HarnessError::PathTraversalDenied {
            path: s.into_owned(),
        });
    }
    let normalized = normalize_macos_root_alias(raw_path)?;
    let (cur_fd, candidate, remaining, _) = walk_nofollow_from_root(&normalized, false)?;
    if is_sensitive_or_system_path(&candidate) {
        return Err(HarnessError::BoundaryViolation {
            reason: format!(
                "FS_JAIL_VIOLATION:SYSTEM_PATH_DENIED: '{}' is blocked",
                candidate.display()
            ),
        });
    }
    let (dev, ino) = fstat_dev_ino(&cur_fd)?;
    Ok(JailAnchor {
        canonical_path: candidate,
        ancestor_dev: dev,
        ancestor_ino: ino,
        ancestor_dev_ino: format!("{dev}:{ino}"),
        target_exists: remaining.is_empty(),
        remaining_components: remaining,
    })
}

pub fn inspect_workspace_relative_jail(
    ws_root: &Path,
    norm_rel: &str,
) -> Result<JailAnchor, HarnessError> {
    let rel_path = Path::new(norm_rel);
    if has_sensitive_dot_component(rel_path) {
        return Err(HarnessError::BoundaryViolation {
            reason: format!("FS_JAIL_VIOLATION:SYSTEM_PATH_DENIED: '{norm_rel}' is blocked"),
        });
    }
    let target = if norm_rel.is_empty() {
        ws_root.to_path_buf()
    } else {
        ws_root.join(rel_path)
    };
    let normalized = normalize_macos_root_alias(&target)?;
    let ws_norm = normalize_macos_root_alias(ws_root)?;
    let ws_canon = fs::canonicalize(&ws_norm).unwrap_or(ws_norm);
    let (cur_fd, candidate, remaining, _) = walk_nofollow_from_root(&normalized, true)?;
    if !candidate.starts_with(&ws_canon) {
        return Err(HarnessError::PathTraversalDenied {
            path: norm_rel.to_string(),
        });
    }
    let (dev, ino) = fstat_dev_ino(&cur_fd)?;
    Ok(JailAnchor {
        canonical_path: candidate,
        ancestor_dev: dev,
        ancestor_ino: ino,
        ancestor_dev_ino: format!("{dev}:{ino}"),
        target_exists: remaining.is_empty(),
        remaining_components: remaining,
    })
}

pub fn provision_dir_zero_toctou_checked(
    raw_path: &Path,
    expected_canonical: Option<&str>,
    expected_ancestor_dev_ino: Option<&str>,
) -> Result<JailAnchor, HarnessError> {
    let anchor = inspect_path_jail(raw_path)?;
    if let Some(exp_di) = expected_ancestor_dev_ino {
        if exp_di.is_empty() || anchor.ancestor_dev_ino != exp_di {
            return Err(HarnessError::BoundaryViolation {
                reason: "FS_JAIL_VIOLATION: Ancestor inode mismatch before directory creation (TOCTOU detected)".to_string(),
            });
        }
    }
    let normalized = normalize_macos_root_alias(raw_path)?;
    let mut cur_fd = open_root_fd()?;
    let mut walked = PathBuf::from("/");
    let mut verified_ancestor_fd = false;
    for comp in normalized.components() {
        if let Component::Normal(seg) = comp {
            walked.push(seg);
            if !verified_ancestor_fd {
                if let Some(next_fd) = open_dir_fd_nofollow(cur_fd.as_raw_fd(), seg)? {
                    cur_fd = next_fd;
                    continue;
                }
                if let Some(exp_di) = expected_ancestor_dev_ino {
                    let (dev, ino) = fstat_dev_ino(&cur_fd)?;
                    if format!("{dev}:{ino}") != exp_di {
                        return Err(HarnessError::BoundaryViolation {
                            reason: "FS_JAIL_VIOLATION: Ancestor inode changed during fd walk before mkdirat (TOCTOU detected)".to_string(),
                        });
                    }
                }
                verified_ancestor_fd = true;
            } else if let Some(next_fd) = open_dir_fd_nofollow(cur_fd.as_raw_fd(), seg)? {
                cur_fd = next_fd;
                continue;
            }
            let c_seg =
                CString::new(seg.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
                    path: walked.display().to_string(),
                })?;
            let rc = unsafe { libc::mkdirat(cur_fd.as_raw_fd(), c_seg.as_ptr(), 0o700) };
            if rc != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MKDIRAT_FAILED [{}]: {}",
                        walked.display(),
                        std::io::Error::last_os_error()
                    ),
                });
            }
            cur_fd = open_dir_fd_nofollow(cur_fd.as_raw_fd(), seg)?.ok_or_else(|| {
                HarnessError::ValidationFailed {
                    reason: format!("OPENAT_AFTER_MKDIR_FAILED [{}]", walked.display()),
                }
            })?;
            if unsafe { libc::fchmod(cur_fd.as_raw_fd(), 0o700) } != 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "FCHMOD_FAILED [{}]: {}",
                        walked.display(),
                        std::io::Error::last_os_error()
                    ),
                });
            }
        }
    }
    if !verified_ancestor_fd {
        if let Some(exp_di) = expected_ancestor_dev_ino {
            let (dev, ino) = fstat_dev_ino(&cur_fd)?;
            if format!("{dev}:{ino}") != exp_di {
                return Err(HarnessError::BoundaryViolation {
                    reason: "FS_JAIL_VIOLATION: Existing target inode changed during fd walk (TOCTOU detected)".to_string(),
                });
            }
        }
    }
    let _ = fstat_dev_ino(&cur_fd)?;
    let final_canon = fs::canonicalize(&walked).unwrap_or(walked);
    if is_sensitive_or_system_path(&final_canon) {
        return Err(HarnessError::BoundaryViolation {
            reason: format!("FS_JAIL_VIOLATION: '{}'", final_canon.display()),
        });
    }
    if let Some(exp_canon) = expected_canonical {
        if final_canon.display().to_string() != exp_canon {
            return Err(HarnessError::BoundaryViolation {
                reason: "FS_JAIL_VIOLATION: Canonical target mismatch after provisioning (TOCTOU detected)".to_string(),
            });
        }
    }
    Ok(JailAnchor {
        canonical_path: final_canon,
        target_exists: true,
        ..anchor
    })
}

pub fn provision_dir_zero_toctou(raw_path: &Path) -> Result<JailAnchor, HarnessError> {
    provision_dir_zero_toctou_checked(raw_path, None, None)
}

pub fn open_dir_abs_nofollow(abs_dir: &Path) -> Result<OwnedFd, HarnessError> {
    let normalized = normalize_macos_root_alias(abs_dir)?;
    let mut cur_fd = open_root_fd()?;
    for comp in normalized.components() {
        if let Component::Normal(seg) = comp {
            cur_fd = open_dir_fd_nofollow(cur_fd.as_raw_fd(), seg)?.ok_or_else(|| {
                HarnessError::ValidationFailed {
                    reason: format!("DIR_NOT_FOUND: '{}'", abs_dir.display()),
                }
            })?;
        }
    }
    Ok(cur_fd)
}

pub fn open_child_dir_fd_nofollow(
    parent_fd: &OwnedFd,
    name: &std::ffi::OsStr,
) -> Result<Option<OwnedFd>, HarnessError> {
    open_dir_fd_nofollow(parent_fd.as_raw_fd(), name)
}

pub fn ensure_child_dir_fd(
    parent_fd: &OwnedFd,
    name: &std::ffi::OsStr,
    mode: libc::mode_t,
) -> Result<OwnedFd, HarnessError> {
    if let Some(existing) = open_dir_fd_nofollow(parent_fd.as_raw_fd(), name)? {
        return Ok(existing);
    }
    let c_name = CString::new(name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
        path: name.to_string_lossy().into_owned(),
    })?;
    let rc = unsafe { libc::mkdirat(parent_fd.as_raw_fd(), c_name.as_ptr(), mode) };
    if rc != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "MKDIRAT_FAILED [{}]: {}",
                name.to_string_lossy(),
                std::io::Error::last_os_error()
            ),
        });
    }
    let fd = open_dir_fd_nofollow(parent_fd.as_raw_fd(), name)?.ok_or_else(|| {
        HarnessError::ValidationFailed {
            reason: format!("OPENAT_AFTER_MKDIR_FAILED [{}]", name.to_string_lossy()),
        }
    })?;
    if unsafe { libc::fchmod(fd.as_raw_fd(), mode) } != 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "FCHMOD_FAILED [{}]: {}",
                name.to_string_lossy(),
                std::io::Error::last_os_error()
            ),
        });
    }
    Ok(fd)
}

#[derive(Debug, Clone)]
pub struct FdDirEntry {
    pub name: std::ffi::OsString,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
}

fn verify_regular_stat(
    st: &libc::stat,
    label: &str,
    require_single_link: bool,
) -> Result<(), HarnessError> {
    let kind = st.st_mode & libc::S_IFMT;
    if kind == libc::S_IFLNK {
        return Err(HarnessError::BoundaryViolation {
            reason: format!("FS_JAIL_VIOLATION: Symlink rejected at '{label}'"),
        });
    }
    if kind != libc::S_IFREG {
        return Err(HarnessError::BoundaryViolation {
            reason: format!("FS_JAIL_VIOLATION: Non-regular file rejected at '{label}'"),
        });
    }
    if require_single_link && st.st_nlink > 1 {
        return Err(HarnessError::BoundaryViolation {
            reason: format!(
                "FS_JAIL_VIOLATION: Hardlink write/overwrite rejected at '{label}' (nlink={})",
                st.st_nlink
            ),
        });
    }
    Ok(())
}

fn ensure_dst_not_hardlink_or_special(
    dir_fd: &OwnedFd,
    c_name: &CString,
    label: &str,
) -> Result<(), HarnessError> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::fstatat(
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc == 0 {
        return verify_regular_stat(&st, label, true);
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ENOENT) {
        return Ok(());
    }
    Err(HarnessError::ValidationFailed {
        reason: format!("FSTATAT_DST_FAILED [{label}]: {err}"),
    })
}

fn write_temp_and_rename_in_dir_fd(
    dir_fd: &OwnedFd,
    c_name: &CString,
    label: &str,
    mode: libc::mode_t,
    mut write_fn: impl FnMut(&mut fs::File) -> Result<(), HarnessError>,
) -> Result<(), HarnessError> {
    ensure_dst_not_hardlink_or_special(dir_fd, c_name, label)?;

    let tmp_name = format!(".cowok-tmp-{}", Uuid::new_v4().simple());
    let c_tmp =
        CString::new(tmp_name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
            path: label.to_string(),
        })?;
    let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw_fd = unsafe {
        libc::openat(
            dir_fd.as_raw_fd(),
            c_tmp.as_ptr(),
            flags,
            mode as libc::c_uint,
        )
    };
    if raw_fd < 0 {
        let err = std::io::Error::last_os_error();
        return Err(HarnessError::ValidationFailed {
            reason: format!("OPENAT_TMP_FAILED [{label}]: {err}"),
        });
    }
    let tmp_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let cleanup_tmp = || {
        let _ = unsafe { libc::unlinkat(dir_fd.as_raw_fd(), c_tmp.as_ptr(), 0) };
    };

    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(tmp_fd.as_raw_fd(), &mut st) } != 0 {
        cleanup_tmp();
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "FSTAT_TMP_FAILED [{label}]: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    if let Err(e) = verify_regular_stat(&st, label, true) {
        cleanup_tmp();
        return Err(e);
    }

    let mut tmp_file = fs::File::from(tmp_fd);
    if let Err(e) = write_fn(&mut tmp_file) {
        cleanup_tmp();
        return Err(e);
    }
    drop(tmp_file);

    // Re-check destination immediately before atomic renameat to block concurrent hardlink/symlink swaps
    if let Err(e) = ensure_dst_not_hardlink_or_special(dir_fd, c_name, label) {
        cleanup_tmp();
        return Err(e);
    }
    if unsafe {
        libc::renameat(
            dir_fd.as_raw_fd(),
            c_tmp.as_ptr(),
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
        )
    } != 0
    {
        let err = std::io::Error::last_os_error();
        cleanup_tmp();
        return Err(HarnessError::ValidationFailed {
            reason: format!("RENAMEAT_FAILED [{label}]: {err}"),
        });
    }
    Ok(())
}

pub fn read_dir_fd_nofollow(
    dir_fd: &OwnedFd,
    label: &str,
) -> Result<Vec<FdDirEntry>, HarnessError> {
    use std::os::unix::ffi::OsStringExt;
    struct DirGuard(*mut libc::DIR);
    impl Drop for DirGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { libc::closedir(self.0) };
            }
        }
    }

    let dup_fd = unsafe { libc::fcntl(dir_fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if dup_fd < 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "DUP_DIR_FD_FAILED [{label}]: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    let dirp = unsafe { libc::fdopendir(dup_fd) };
    if dirp.is_null() {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(dup_fd) };
        return Err(HarnessError::ValidationFailed {
            reason: format!("FDOPENDIR_FAILED [{label}]: {err}"),
        });
    }
    let _guard = DirGuard(dirp);
    let mut entries = Vec::new();
    loop {
        let ent = unsafe { libc::readdir(dirp) };
        if ent.is_null() {
            break;
        }
        let c_name = unsafe { std::ffi::CStr::from_ptr((*ent).d_name.as_ptr()) };
        let bytes = c_name.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::fstatat(
                dir_fd.as_raw_fd(),
                c_name.as_ptr(),
                &mut st,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "FSTATAT_ENTRY_FAILED [{}]: {}",
                    c_name.to_string_lossy(),
                    std::io::Error::last_os_error()
                ),
            });
        }
        let kind = st.st_mode & libc::S_IFMT;
        entries.push(FdDirEntry {
            name: std::ffi::OsString::from_vec(bytes.to_vec()),
            is_dir: kind == libc::S_IFDIR,
            is_file: kind == libc::S_IFREG,
            is_symlink: kind == libc::S_IFLNK,
        });
    }
    Ok(entries)
}

pub fn write_file_in_dir_fd(
    dir_fd: &OwnedFd,
    name: &std::ffi::OsStr,
    bytes: &[u8],
    mode: libc::mode_t,
) -> Result<(), HarnessError> {
    use std::io::Write;
    let label = name.to_string_lossy().into_owned();
    let c_name = CString::new(name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
        path: label.clone(),
    })?;
    write_temp_and_rename_in_dir_fd(dir_fd, &c_name, &label, mode, |file| {
        file.write_all(bytes)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("WRITE_FILE_FAILED [{label}]: {e}"),
            })
    })
}

pub fn check_child_non_symlink_in_dir_fd(dir_fd: &OwnedFd, name: &std::ffi::OsStr) -> bool {
    let Ok(c_name) = CString::new(name.as_bytes()) else {
        return false;
    };
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::fstatat(
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    rc == 0 && (st.st_mode & libc::S_IFMT) != libc::S_IFLNK
}

pub fn remove_rel_file_in_dir_fd(base_fd: &OwnedFd, rel: &Path) -> Result<(), HarnessError> {
    let segs: Vec<&std::ffi::OsStr> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s),
            _ => None,
        })
        .collect();
    let Some((file_name, dirs)) = segs.split_last() else {
        return Ok(());
    };
    let mut cur_fd: Option<OwnedFd> = None;
    for seg in dirs {
        let parent_raw = cur_fd.as_ref().unwrap_or(base_fd).as_raw_fd();
        let Some(next) = open_dir_fd_nofollow(parent_raw, seg)? else {
            return Ok(());
        };
        cur_fd = Some(next);
    }
    let dir_raw = cur_fd.as_ref().unwrap_or(base_fd).as_raw_fd();
    let c_name =
        CString::new(file_name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
            path: rel.display().to_string(),
        })?;
    let rc = unsafe { libc::unlinkat(dir_raw, c_name.as_ptr(), 0) };
    if rc != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "UNLINKAT_FAILED [{}]: {}",
                rel.display(),
                std::io::Error::last_os_error()
            ),
        });
    }
    Ok(())
}

pub fn ensure_regular_file_or_init_in_dir_fd(
    dir_fd: &OwnedFd,
    name: &std::ffi::OsStr,
    mode: libc::mode_t,
    gen_bytes: impl FnOnce() -> Vec<u8>,
) -> Result<Vec<u8>, HarnessError> {
    use std::io::{Read, Write};
    let label = name.to_string_lossy().into_owned();
    let c_name = CString::new(name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
        path: label.clone(),
    })?;
    let rd_flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw = unsafe { libc::openat(dir_fd.as_raw_fd(), c_name.as_ptr(), rd_flags) };
    if raw >= 0 {
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd.as_raw_fd(), &mut st) } != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "FSTAT_FAILED [{label}]: {}",
                    std::io::Error::last_os_error()
                ),
            });
        }
        verify_regular_stat(&st, &label, true)?;
        if unsafe { libc::fchmod(fd.as_raw_fd(), mode) } != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "FCHMOD_FAILED [{label}]: {}",
                    std::io::Error::last_os_error()
                ),
            });
        }
        let mut f = fs::File::from(fd);
        let mut out = Vec::new();
        f.read_to_end(&mut out)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("READ_FILE_FAILED [{label}]: {e}"),
            })?;
        return Ok(out);
    }
    let open_err = std::io::Error::last_os_error();
    if open_err.raw_os_error() != Some(libc::ENOENT) {
        return Err(HarnessError::PathTraversalDenied {
            path: format!("{label} ({open_err})"),
        });
    }
    let secret = gen_bytes();
    let wr_flags =
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let wr_raw = unsafe {
        libc::openat(
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            wr_flags,
            mode as libc::c_uint,
        )
    };
    if wr_raw < 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "CREATE_FILE_FAILED [{label}]: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    let wr_fd = unsafe { OwnedFd::from_raw_fd(wr_raw) };
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(wr_fd.as_raw_fd(), &mut st) } != 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "FSTAT_NEW_FAILED [{label}]: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    verify_regular_stat(&st, &label, true)?;
    let _ = unsafe { libc::fchmod(wr_fd.as_raw_fd(), mode) };
    let mut wf = fs::File::from(wr_fd);
    wf.write_all(&secret)
        .and_then(|_| wf.sync_all())
        .map_err(|e| HarnessError::ValidationFailed {
            reason: format!("WRITE_NEW_FAILED [{label}]: {e}"),
        })?;
    Ok(secret)
}

pub fn copy_file_between_dir_fds(
    src_dir_fd: &OwnedFd,
    dst_dir_fd: &OwnedFd,
    name: &std::ffi::OsStr,
    mode: libc::mode_t,
) -> Result<(), HarnessError> {
    let label = name.to_string_lossy().into_owned();
    let c_name = CString::new(name.as_bytes()).map_err(|_| HarnessError::PathTraversalDenied {
        path: label.clone(),
    })?;
    let src_raw = unsafe {
        libc::openat(
            src_dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if src_raw < 0 {
        let err = std::io::Error::last_os_error();
        return Err(if err.raw_os_error() == Some(libc::ELOOP) {
            HarnessError::BoundaryViolation {
                reason: format!("FS_JAIL_VIOLATION: Symlink source rejected at '{label}'"),
            }
        } else {
            HarnessError::ValidationFailed {
                reason: format!("OPEN_SRC_FAILED [{label}]: {err}"),
            }
        });
    }
    let src_fd = unsafe { OwnedFd::from_raw_fd(src_raw) };
    let mut src_st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(src_fd.as_raw_fd(), &mut src_st) } != 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "FSTAT_SRC_FAILED [{label}]: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    // Allow legitimate workspace hardlinks (e.g. .git/objects, pnpm store) on src while blocking symlinks/FIFOs
    verify_regular_stat(&src_st, &label, false)?;
    let mut src_file = fs::File::from(src_fd);
    write_temp_and_rename_in_dir_fd(dst_dir_fd, &c_name, &label, mode, |dst_file| {
        std::io::copy(&mut src_file, dst_file)
            .map(|_| ())
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("COPY_FILE_FAILED [{label}]: {e}"),
            })
    })
}

pub fn build_signed_path_token(
    secret: &[u8],
    payload: &PathTokenPayload,
) -> Result<String, HarnessError> {
    let bytes = serde_json::to_vec(payload).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("SERIALIZE_PAYLOAD_FAILED: {e}"),
    })?;
    let payload_hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let mac_hex: String = hmac_sha256(secret, payload_hex.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(format!("{payload_hex}.{mac_hex}"))
}

pub fn decode_and_verify_path_token(
    secret: &[u8],
    token: &str,
    expected_project_id: Uuid,
    daemon_nonce: &str,
    target_mode: bool,
) -> Result<PathTokenPayload, HarnessError> {
    let fail = |msg: &str| {
        if target_mode {
            HarnessError::BoundaryViolation {
                reason: format!("FS_JAIL_VIOLATION: {msg}"),
            }
        } else {
            HarnessError::ValidationFailed {
                reason: format!("INVALID_PATH_TOKEN: {msg}"),
            }
        }
    };
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return Err(fail("Malformed token structure"));
    }
    let expected_mac: String = hmac_sha256(secret, parts[0].as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if !constant_time_eq(parts[1].as_bytes(), expected_mac.as_bytes()) {
        return Err(fail("Signature verification failed"));
    }
    let bytes = decode_hex(parts[0]).ok_or_else(|| fail("Payload hex decode failed"))?;
    let payload: PathTokenPayload =
        serde_json::from_slice(&bytes).map_err(|e| fail(&format!("Invalid payload json: {e}")))?;
    if payload.project_id != expected_project_id || payload.exp < Utc::now().timestamp() {
        return Err(fail("Token project mismatch or expired"));
    }
    if target_mode {
        if payload.daemon_nonce.is_empty() || payload.daemon_nonce != daemon_nonce {
            return Err(HarnessError::BoundaryViolation {
                reason: "FS_JAIL_VIOLATION: Daemon nonce missing or mismatch (stale path token)"
                    .to_string(),
            });
        }
        if payload.ancestor_dev_ino.is_empty() {
            return Err(HarnessError::BoundaryViolation {
                reason: "FS_JAIL_VIOLATION: PathToken missing ancestor_dev_ino".to_string(),
            });
        }
    } else if !payload.daemon_nonce.is_empty() && payload.daemon_nonce != daemon_nonce {
        return Err(HarnessError::BoundaryViolation {
            reason: "FS_JAIL_VIOLATION: Daemon nonce mismatch (stale path token)".to_string(),
        });
    }
    Ok(payload)
}

fn is_browse_path_allowed(canon: &Path, managed_root_canon: &Path) -> bool {
    if is_sensitive_or_system_path(canon) {
        return false;
    }
    if canon.starts_with(managed_root_canon) {
        return true;
    }
    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            let home_canon = fs::canonicalize(trimmed).unwrap_or_else(|_| PathBuf::from(trimmed));
            if canon.starts_with(&home_canon) {
                return true;
            }
        }
    }
    let lower = canon.to_string_lossy().to_lowercase();
    [
        "/private/tmp",
        "/tmp",
        "/private/var/folders",
        "/var/folders",
    ]
    .iter()
    .any(|prefix| lower == *prefix || lower.starts_with(&format!("{prefix}/")))
}

pub(super) async fn handle_fs_browse(
    State(_state): State<DaemonAppState>,
    Query(query): Query<FsBrowseQuery>,
) -> Result<Json<FsBrowseResponse>, Response> {
    let managed_root = default_managed_root().map_err(map_error_to_response)?;
    let managed_anchor = provision_dir_zero_toctou(&managed_root).map_err(map_error_to_response)?;
    let requested = query
        .path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let current = match requested {
        Some(raw) => {
            let anchor = inspect_path_jail(Path::new(raw)).map_err(map_error_to_response)?;
            if !is_browse_path_allowed(&anchor.canonical_path, &managed_anchor.canonical_path) {
                return Err(map_error_to_response(HarnessError::BoundaryViolation {
                    reason: format!(
                        "FS_JAIL_VIOLATION: Browse path '{}' is outside allowed roots",
                        anchor.canonical_path.display()
                    ),
                }));
            }
            if !anchor.target_exists {
                return Err(map_error_to_response(HarnessError::ValidationFailed {
                    reason: format!(
                        "DIR_NOT_FOUND: Directory '{}' does not exist",
                        anchor.canonical_path.display()
                    ),
                }));
            }
            anchor.canonical_path
        }
        None => managed_anchor.canonical_path.clone(),
    };
    let mut entries = Vec::new();
    let current_fd = open_dir_abs_nofollow(&current).map_err(map_error_to_response)?;
    let dir_entries =
        read_dir_fd_nofollow(&current_fd, "fs_browse").map_err(map_error_to_response)?;
    for item in dir_entries {
        let name = item.name.to_string_lossy().into_owned();
        if name.starts_with('.') || !item.is_dir || item.is_symlink {
            continue;
        }
        if let Ok(Some(child_fd)) = open_child_dir_fd_nofollow(&current_fd, &item.name) {
            let is_git_repo =
                check_child_non_symlink_in_dir_fd(&child_fd, std::ffi::OsStr::new(".git"));
            let p = current.join(&item.name);
            entries.push(FsBrowseEntry {
                name,
                is_git_repo,
                path: p.display().to_string(),
                is_dir: true,
            });
        }
    }
    entries.sort_by_key(|a| a.name.to_lowercase());
    let parent_path = current
        .parent()
        .filter(|p| is_browse_path_allowed(p, &managed_anchor.canonical_path))
        .map(|p| p.display().to_string());
    Ok(Json(FsBrowseResponse {
        current_path: current.display().to_string(),
        parent_path,
        managed_root: managed_anchor.canonical_path.display().to_string(),
        entries,
    }))
}

pub(super) async fn handle_issue_path_token_v2(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<IssuePathTokenRequest>,
) -> Result<Json<PathTokenResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, ws_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let (norm_path, anchor) = if let Some(tp) = req
        .target_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let a = inspect_path_jail(Path::new(tp)).map_err(map_error_to_response)?;
        ensure_writable_target_not_home(&a.canonical_path).map_err(map_error_to_response)?;
        (a.canonical_path.display().to_string(), a)
    } else {
        let rel = req.relative_path.as_deref().unwrap_or("");
        let n = validate_and_normalize_relative_path(rel).map_err(map_error_to_response)?;
        let a = inspect_workspace_relative_jail(&ws_root, &n).map_err(map_error_to_response)?;
        (n, a)
    };
    let ttl = req.ttl_seconds.unwrap_or(3600).max(1);
    let exp = Utc::now().timestamp().saturating_add(ttl);
    let canon_str = anchor.canonical_path.display().to_string();
    let payload = PathTokenPayload {
        v: 2,
        project_id,
        normalized_relative_path: norm_path.clone(),
        principal_id: req
            .principal_id
            .unwrap_or_else(|| extract_actor_id(&headers)),
        exp,
        nonce: Uuid::new_v4().to_string(),
        daemon_nonce: state.auth.daemon_nonce.clone(),
        canonical_path: canon_str.clone(),
        ancestor_dev_ino: anchor.ancestor_dev_ino.clone(),
        target_exists: anchor.target_exists,
    };
    let secret = crate::synthesis::ensure_project_token(&ws_root).map_err(map_error_to_response)?;
    let token =
        build_signed_path_token(secret.as_bytes(), &payload).map_err(map_error_to_response)?;
    Ok(Json(PathTokenResponse {
        path_token: token.clone(),
        token,
        expires_at: exp,
        normalized_relative_path: norm_path,
        canonical_path: canon_str,
        daemon_nonce: payload.daemon_nonce,
        dev_ino: anchor.ancestor_dev_ino.clone(),
        ancestor_dev_ino: anchor.ancestor_dev_ino,
        target_exists: anchor.target_exists,
    }))
}

pub(super) async fn handle_verify_path_token_v2(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<VerifyPathTokenRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, ws_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let target_opt = req
        .target_path
        .as_deref()
        .or(req.target_directory.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let secret = crate::synthesis::ensure_project_token(&ws_root).map_err(map_error_to_response)?;
    let payload = decode_and_verify_path_token(
        secret.as_bytes(),
        &req.token,
        project_id,
        &state.auth.daemon_nonce,
        target_opt.is_some(),
    )
    .map_err(map_error_to_response)?;
    if let Some(ref p) = req.principal_id {
        if &payload.principal_id != p {
            return Err(map_error_to_response(HarnessError::BoundaryViolation {
                reason: "UNAUTHORIZED: Principal ID mismatch".to_string(),
            }));
        }
    }
    if let Some(tp) = target_opt {
        let anchor = inspect_path_jail(Path::new(tp)).map_err(map_error_to_response)?;
        ensure_writable_target_not_home(&anchor.canonical_path).map_err(map_error_to_response)?;
        if anchor.canonical_path.display().to_string() != payload.canonical_path
            || payload.ancestor_dev_ino.is_empty()
            || anchor.ancestor_dev_ino != payload.ancestor_dev_ino
        {
            return Err(map_error_to_response(HarnessError::BoundaryViolation {
                reason: "FS_JAIL_VIOLATION: Path or ancestor inode mismatch (TOCTOU detected)"
                    .to_string(),
            }));
        }
    } else if let Some(ref rel) = req.expected_relative_path {
        let norm_expected =
            validate_and_normalize_relative_path(rel).map_err(map_error_to_response)?;
        if payload.normalized_relative_path != norm_expected {
            return Err(map_error_to_response(HarnessError::ValidationFailed {
                reason: format!(
                    "INVALID_PATH_TOKEN: Path mismatch (expected '{}', got '{}')",
                    norm_expected, payload.normalized_relative_path
                ),
            }));
        }
    }
    Ok(Json(serde_json::json!({
        "valid": true,
        "normalized_relative_path": payload.normalized_relative_path,
        "canonical_path": payload.canonical_path,
        "project_id": project_id,
        "principal_id": payload.principal_id,
        "daemon_nonce": payload.daemon_nonce,
        "ancestor_dev_ino": payload.ancestor_dev_ino,
        "target_exists": payload.target_exists,
    })))
}
