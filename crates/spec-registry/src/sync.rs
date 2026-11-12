use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use domain::{CanonicalSpecKey, HarnessError};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, RawFd};

/// Computes the lowercase 64-character hex SHA-256 digest of raw bytes.
pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Computes the deterministic `summary_digest` SHA-256 hex of the canonical metadata fields.
pub fn compute_summary_digest(
    spec_key: CanonicalSpecKey,
    content_sha256: &str,
    actor_id: &str,
    change_summary: &str,
    source_project_id: Option<Uuid>,
    source_revision_sha256: Option<&str>,
) -> String {
    let canonical = serde_json::json!({
        "actor": actor_id,
        "change_summary": change_summary,
        "content_sha256": content_sha256,
        "source_project_id": source_project_id.map(|id| id.to_string()),
        "source_revision_sha256": source_revision_sha256,
        "spec_key": spec_key.as_str(),
    });
    compute_sha256_hex(canonical.to_string().as_bytes())
}

/// Validates and resolves `<workspace_root>/product/specs/<canonical_spec_key>.md` inside the workspace jail.
/// Rejects symlinks (`symlink_metadata().file_type().is_symlink()`) at workspace root, intermediate directories,
/// or target file, and enforces `canonicalize()` prefix containment.
pub fn resolve_jailed_spec_path(
    workspace_root: &Path,
    spec_key: CanonicalSpecKey,
) -> Result<PathBuf, HarnessError> {
    if !workspace_root.exists() {
        return Err(HarnessError::PathTraversalDenied {
            path: workspace_root.display().to_string(),
        });
    }

    let root_meta =
        fs::symlink_metadata(workspace_root).map_err(|_| HarnessError::PathTraversalDenied {
            path: workspace_root.display().to_string(),
        })?;
    if root_meta.file_type().is_symlink() {
        return Err(HarnessError::PathTraversalDenied {
            path: workspace_root.display().to_string(),
        });
    }

    let canonical_root =
        workspace_root
            .canonicalize()
            .map_err(|_| HarnessError::PathTraversalDenied {
                path: workspace_root.display().to_string(),
            })?;

    let product_dir = canonical_root.join("product");
    ensure_non_symlink_dir(&product_dir, &canonical_root)?;

    let specs_dir = product_dir.join("specs");
    ensure_non_symlink_dir(&specs_dir, &canonical_root)?;

    let canonical_specs_dir =
        specs_dir
            .canonicalize()
            .map_err(|_| HarnessError::PathTraversalDenied {
                path: specs_dir.display().to_string(),
            })?;
    if !canonical_specs_dir.starts_with(&canonical_root) {
        return Err(HarnessError::PathTraversalDenied {
            path: canonical_specs_dir.display().to_string(),
        });
    }

    let target_path = canonical_specs_dir.join(spec_key.as_str());
    if let Ok(meta) = fs::symlink_metadata(&target_path) {
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(HarnessError::PathTraversalDenied {
                path: target_path.display().to_string(),
            });
        }
        let canonical_target =
            target_path
                .canonicalize()
                .map_err(|_| HarnessError::PathTraversalDenied {
                    path: target_path.display().to_string(),
                })?;
        if !canonical_target.starts_with(&canonical_specs_dir) {
            return Err(HarnessError::PathTraversalDenied {
                path: canonical_target.display().to_string(),
            });
        }
    }

    Ok(target_path)
}

/// Validates an arbitrary candidate path (e.g. from a filesystem watcher event) against the workspace jail
/// and extracts its `CanonicalSpecKey`.
pub fn validate_candidate_spec_path(
    workspace_root: &Path,
    candidate_path: &Path,
) -> Result<(CanonicalSpecKey, PathBuf), HarnessError> {
    for comp in candidate_path.components() {
        if matches!(comp, Component::ParentDir) {
            return Err(HarnessError::PathTraversalDenied {
                path: candidate_path.display().to_string(),
            });
        }
    }

    let file_name = candidate_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| HarnessError::PathTraversalDenied {
            path: candidate_path.display().to_string(),
        })?;

    let spec_key = CanonicalSpecKey::parse(file_name)?;
    let expected_path = resolve_jailed_spec_path(workspace_root, spec_key)?;

    if let Ok(meta) = fs::symlink_metadata(candidate_path) {
        if meta.file_type().is_symlink() {
            return Err(HarnessError::PathTraversalDenied {
                path: candidate_path.display().to_string(),
            });
        }
        let canonical_candidate =
            candidate_path
                .canonicalize()
                .map_err(|_| HarnessError::PathTraversalDenied {
                    path: candidate_path.display().to_string(),
                })?;
        if canonical_candidate != expected_path {
            return Err(HarnessError::PathTraversalDenied {
                path: candidate_path.display().to_string(),
            });
        }
    } else if candidate_path != expected_path {
        let canonical_root =
            workspace_root
                .canonicalize()
                .map_err(|_| HarnessError::PathTraversalDenied {
                    path: workspace_root.display().to_string(),
                })?;
        let joined = if candidate_path.is_relative() {
            canonical_root.join(candidate_path)
        } else {
            candidate_path.to_path_buf()
        };
        if joined != expected_path {
            let parent_matched = candidate_path
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .map(|cp| cp.join(file_name) == expected_path)
                .unwrap_or(false);
            if !parent_matched {
                return Err(HarnessError::PathTraversalDenied {
                    path: candidate_path.display().to_string(),
                });
            }
        }
    }

    Ok((spec_key, expected_path))
}

fn ensure_non_symlink_dir(dir: &Path, canonical_root: &Path) -> Result<(), HarnessError> {
    if dir.exists() {
        let meta = fs::symlink_metadata(dir).map_err(|_| HarnessError::PathTraversalDenied {
            path: dir.display().to_string(),
        })?;
        if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
            return Err(HarnessError::PathTraversalDenied {
                path: dir.display().to_string(),
            });
        }
        let canon = dir
            .canonicalize()
            .map_err(|_| HarnessError::PathTraversalDenied {
                path: dir.display().to_string(),
            })?;
        if !canon.starts_with(canonical_root) {
            return Err(HarnessError::PathTraversalDenied {
                path: dir.display().to_string(),
            });
        }
    } else {
        fs::create_dir_all(dir).map_err(|_| HarnessError::PathTraversalDenied {
            path: dir.display().to_string(),
        })?;
    }
    Ok(())
}

/// Atomically writes `content` to a sibling `.tmp.<operation_id>` file, flushes, calls `sync_all()`,
/// atomically renames over `target_path`, and calls `sync_all()` on the parent `product/specs` directory.
/// On Unix, uses `openat` + `O_NOFOLLOW` + `renameat` step-by-step from `workspace_root` to eliminate
/// any TOCTOU symlink race between path validation and file open/rename.
pub fn atomic_write_spec_file(
    workspace_root: &Path,
    spec_key: CanonicalSpecKey,
    operation_id: Uuid,
    content: &[u8],
) -> Result<PathBuf, HarnessError> {
    let target_path = resolve_jailed_spec_path(workspace_root, spec_key)?;

    #[cfg(unix)]
    {
        atomic_write_spec_file_unix_nofollow(workspace_root, spec_key, operation_id, content)?;
        return Ok(target_path);
    }

    #[cfg(windows)]
    {
        atomic_write_spec_file_windows(workspace_root, spec_key, operation_id, content)?;
        return Ok(target_path);
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (operation_id, content);
        Err(HarnessError::ValidationFailed {
            reason: format!(
                "LOCK_UNAVAILABLE: atomic spec write is unsupported on target for {} (fail-closed)",
                target_path.display()
            ),
        })
    }
}

#[cfg(windows)]
fn atomic_write_spec_file_windows(
    workspace_root: &Path,
    spec_key: CanonicalSpecKey,
    operation_id: Uuid,
    content: &[u8],
) -> Result<PathBuf, HarnessError> {
    let target_path = resolve_jailed_spec_path(workspace_root, spec_key)?;
    let parent = target_path
        .parent()
        .ok_or_else(|| HarnessError::PathTraversalDenied {
            path: target_path.display().to_string(),
        })?;

    // Check for reparse point (symlink/junction) on parent and target if exists
    if let Ok(meta) = fs::symlink_metadata(parent) {
        use std::os::windows::fs::MetadataExt;
        if (meta.file_attributes() & 0x400) != 0 {
            return Err(HarnessError::PathTraversalDenied {
                path: parent.display().to_string(),
            });
        }
    }
    if let Ok(meta) = fs::symlink_metadata(&target_path) {
        use std::os::windows::fs::MetadataExt;
        if (meta.file_attributes() & 0x400) != 0 {
            return Err(HarnessError::PathTraversalDenied {
                path: target_path.display().to_string(),
            });
        }
    }

    let temp_name = format!(".{}.tmp.{}", spec_key.as_str(), operation_id);
    let temp_path = parent.join(&temp_name);
    {
        let mut file = File::create(&temp_path).map_err(|e| HarnessError::ValidationFailed {
            reason: format!(
                "Failed to create Windows temp file {}: {e}",
                temp_path.display()
            ),
        })?;
        file.write_all(content)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("Failed to write Windows temp file: {e}"),
            })?;
        file.sync_all()
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("Failed to sync Windows temp file: {e}"),
            })?;
    }

    use std::os::windows::ffi::OsStrExt;
    let wide_src: Vec<u16> = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let wide_dst: Vec<u16> = target_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x00000001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x00000008;

    extern "system" {
        fn MoveFileExW(lpExisting: *const u16, lpNew: *const u16, dwFlags: u32) -> i32;
    }

    let success = unsafe {
        MoveFileExW(
            wide_src.as_ptr(),
            wide_dst.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if success == 0 {
        let err = std::io::Error::last_os_error();
        let _ = fs::remove_file(&temp_path);
        return Err(HarnessError::ValidationFailed {
            reason: format!("MoveFileExW failed for {}: {err}", target_path.display()),
        });
    }

    if let Ok(dir_file) = File::open(parent) {
        let _ = dir_file.sync_all();
    }

    Ok(target_path)
}

#[cfg(unix)]
struct OwnedFdGuard(RawFd);

#[cfg(unix)]
impl Drop for OwnedFdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

#[cfg(unix)]
fn open_dir_nofollow(
    parent_fd: RawFd,
    name: &CString,
    display_path: &str,
) -> Result<OwnedFdGuard, HarnessError> {
    let fd = unsafe {
        libc::openat(
            parent_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(HarnessError::PathTraversalDenied {
            path: display_path.to_string(),
        });
    }
    Ok(OwnedFdGuard(fd))
}

#[cfg(unix)]
fn atomic_write_spec_file_unix_nofollow(
    workspace_root: &Path,
    spec_key: CanonicalSpecKey,
    operation_id: Uuid,
    content: &[u8],
) -> Result<(), HarnessError> {
    let root_cstr = CString::new(workspace_root.as_os_str().as_bytes()).map_err(|_| {
        HarnessError::PathTraversalDenied {
            path: workspace_root.display().to_string(),
        }
    })?;

    let root_fd = unsafe {
        libc::open(
            root_cstr.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if root_fd < 0 {
        return Err(HarnessError::PathTraversalDenied {
            path: workspace_root.display().to_string(),
        });
    }
    let root_guard = OwnedFdGuard(root_fd);

    let product_cstr = CString::new("product").unwrap();
    let product_guard = open_dir_nofollow(
        root_guard.0,
        &product_cstr,
        &workspace_root.join("product").display().to_string(),
    )?;

    let specs_cstr = CString::new("specs").unwrap();
    let specs_guard = open_dir_nofollow(
        product_guard.0,
        &specs_cstr,
        &workspace_root.join("product/specs").display().to_string(),
    )?;

    let target_cstr =
        CString::new(spec_key.as_str()).map_err(|_| HarnessError::InvalidSpecKey {
            key: spec_key.as_str().to_string(),
        })?;

    // Ensure existing target (if any) in specs_guard is not a symlink via fstatat(AT_SYMLINK_NOFOLLOW)
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let stat_rc = unsafe {
        libc::fstatat(
            specs_guard.0,
            target_cstr.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if stat_rc == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        return Err(HarnessError::PathTraversalDenied {
            path: workspace_root
                .join("product/specs")
                .join(spec_key.as_str())
                .display()
                .to_string(),
        });
    }

    let tmp_name = format!(".{}.tmp.{}", spec_key.as_str(), operation_id);
    let tmp_cstr = CString::new(tmp_name.as_str()).unwrap();

    let tmp_fd = unsafe {
        libc::openat(
            specs_guard.0,
            tmp_cstr.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o644,
        )
    };
    if tmp_fd < 0 {
        let err = std::io::Error::last_os_error();
        return Err(HarnessError::ValidationFailed {
            reason: format!("ATOMIC_WRITE_OPENAT_FAILED [{}]: {err}", spec_key.as_str()),
        });
    }

    let mut tmp_file = unsafe { File::from_raw_fd(tmp_fd) };
    let write_res = (|| -> Result<(), std::io::Error> {
        tmp_file.write_all(content)?;
        tmp_file.flush()?;
        tmp_file.sync_all()?;
        Ok(())
    })();
    drop(tmp_file);

    if let Err(err) = write_res {
        unsafe {
            libc::unlinkat(specs_guard.0, tmp_cstr.as_ptr(), 0);
        }
        return Err(HarnessError::ValidationFailed {
            reason: format!("ATOMIC_WRITE_FAILED [{}]: {err}", spec_key.as_str()),
        });
    }

    let rename_rc = unsafe {
        libc::renameat(
            specs_guard.0,
            tmp_cstr.as_ptr(),
            specs_guard.0,
            target_cstr.as_ptr(),
        )
    };
    if rename_rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe {
            libc::unlinkat(specs_guard.0, tmp_cstr.as_ptr(), 0);
        }
        return Err(HarnessError::ValidationFailed {
            reason: format!("ATOMIC_RENAMEAT_FAILED [{}]: {err}", spec_key.as_str()),
        });
    }

    unsafe {
        libc::fsync(specs_guard.0);
    }

    Ok(())
}

/// Reads `<workspace_root>/product/specs/<spec_key>.md` using `openat` + `O_NOFOLLOW` step-by-step
/// from `workspace_root` to prevent any TOCTOU symlink traversal during read/rehash/recovery.
/// Returns `Ok(None)` if the file does not exist (`ENOENT`).
pub fn read_jailed_spec_bytes(
    workspace_root: &Path,
    spec_key: CanonicalSpecKey,
) -> Result<Option<Vec<u8>>, HarnessError> {
    let _ = resolve_jailed_spec_path(workspace_root, spec_key)?;

    #[cfg(unix)]
    {
        let root_cstr = CString::new(workspace_root.as_os_str().as_bytes()).map_err(|_| {
            HarnessError::PathTraversalDenied {
                path: workspace_root.display().to_string(),
            }
        })?;
        let root_fd = unsafe {
            libc::open(
                root_cstr.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if root_fd < 0 {
            return Err(HarnessError::PathTraversalDenied {
                path: workspace_root.display().to_string(),
            });
        }
        let root_guard = OwnedFdGuard(root_fd);

        let product_cstr = CString::new("product").unwrap();
        let product_guard = open_dir_nofollow(
            root_guard.0,
            &product_cstr,
            &workspace_root.join("product").display().to_string(),
        )?;

        let specs_cstr = CString::new("specs").unwrap();
        let specs_guard = open_dir_nofollow(
            product_guard.0,
            &specs_cstr,
            &workspace_root.join("product/specs").display().to_string(),
        )?;

        let target_cstr =
            CString::new(spec_key.as_str()).map_err(|_| HarnessError::InvalidSpecKey {
                key: spec_key.as_str().to_string(),
            })?;

        let fd = unsafe {
            libc::openat(
                specs_guard.0,
                target_cstr.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOENT) {
                return Ok(None);
            }
            if err.raw_os_error() == Some(libc::ELOOP) {
                return Err(HarnessError::PathTraversalDenied {
                    path: workspace_root
                        .join("product/specs")
                        .join(spec_key.as_str())
                        .display()
                        .to_string(),
                });
            }
            return Err(HarnessError::ValidationFailed {
                reason: format!("JAILED_READ_FAILED [{}]: {err}", spec_key.as_str()),
            });
        }

        let mut file = unsafe { File::from_raw_fd(fd) };
        let meta = file
            .metadata()
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("JAILED_STAT_FAILED [{}]: {e}", spec_key.as_str()),
            })?;
        if !meta.file_type().is_file() {
            return Err(HarnessError::PathTraversalDenied {
                path: workspace_root
                    .join("product/specs")
                    .join(spec_key.as_str())
                    .display()
                    .to_string(),
            });
        }

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("JAILED_READ_BYTES_FAILED [{}]: {e}", spec_key.as_str()),
            })?;
        Ok(Some(buf))
    }

    #[cfg(not(unix))]
    {
        Err(HarnessError::ValidationFailed {
            reason: "LOCK_UNAVAILABLE: O_NOFOLLOW jailed read is unsupported on non-Unix targets (fail-closed)".to_string(),
        })
    }
}

/// Removes a temporary sibling file `.<spec_key>.tmp.<operation_id>` inside `<workspace_root>/product/specs/`
/// using `openat(O_DIRECTORY | O_NOFOLLOW)` + `unlinkat` to prevent any TOCTOU symlink race during cleanup.
pub fn remove_jailed_temp_file(
    workspace_root: &Path,
    spec_key: CanonicalSpecKey,
    operation_id: &str,
) -> Result<(), HarnessError> {
    let _ = resolve_jailed_spec_path(workspace_root, spec_key)?;

    #[cfg(unix)]
    {
        let root_cstr = CString::new(workspace_root.as_os_str().as_bytes()).map_err(|_| {
            HarnessError::PathTraversalDenied {
                path: workspace_root.display().to_string(),
            }
        })?;
        let root_fd = unsafe {
            libc::open(
                root_cstr.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if root_fd < 0 {
            return Err(HarnessError::PathTraversalDenied {
                path: workspace_root.display().to_string(),
            });
        }
        let root_guard = OwnedFdGuard(root_fd);

        let product_cstr = CString::new("product").unwrap();
        let product_guard = open_dir_nofollow(
            root_guard.0,
            &product_cstr,
            &workspace_root.join("product").display().to_string(),
        )?;

        let specs_cstr = CString::new("specs").unwrap();
        let specs_guard = open_dir_nofollow(
            product_guard.0,
            &specs_cstr,
            &workspace_root.join("product/specs").display().to_string(),
        )?;

        let tmp_name = format!(".{}.tmp.{}", spec_key.as_str(), operation_id);
        if let Ok(tmp_cstr) = CString::new(tmp_name.as_str()) {
            unsafe {
                libc::unlinkat(specs_guard.0, tmp_cstr.as_ptr(), 0);
            }
        }
        Ok(())
    }

    #[cfg(not(unix))]
    {
        let _ = operation_id;
        Err(HarnessError::ValidationFailed {
            reason: "LOCK_UNAVAILABLE: O_NOFOLLOW temp cleanup is unsupported on non-Unix targets (fail-closed)".to_string(),
        })
    }
}
