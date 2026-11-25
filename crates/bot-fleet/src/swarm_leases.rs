// @story 10.2
// @implements AC-01, AC-03 (Disjoint worker_file_leases, Canonical Ancestor/Prefix Overlap Guard, .cowok-design/coordination ONE_JOB+NEVER Mailbox, Fencing-Token Staging Commit, 429 Semaphore Clamp 3->1 & Expired Lease -PGID Kill)
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// Canonical coordination mailbox root relative to a project workspace.
pub const COORDINATION_REL_DIR: &str = ".cowok-design/coordination";

/// Initial parallel swarm concurrency ceiling before any HTTP 429 pressure.
pub const INITIAL_CONCURRENCY: usize = 3;

/// Clamped concurrency ceiling applied immediately upon receiving HTTP 429.
pub const CLAMPED_CONCURRENCY: usize = 1;

/// Minimum allowed worker lease TTL in milliseconds (5 seconds).
pub const MIN_LEASE_TTL_MS: i64 = 5_000;

/// Default worker lease TTL in milliseconds (60 seconds).
pub const DEFAULT_LEASE_TTL_MS: i64 = 60_000;

/// Maximum allowed worker lease TTL in milliseconds (5 minutes).
pub const MAX_LEASE_TTL_MS: i64 = 300_000;

/// Default base exponential backoff in milliseconds after an HTTP 429 response.
pub const DEFAULT_BASE_BACKOFF_MS: u64 = 250;

/// Hard upper bound for jittered exponential backoff in milliseconds.
pub const MAX_BACKOFF_MS: u64 = 30_000;

/// Consecutive clean executions required while clamped before stepping concurrency upward.
pub const RECOVERY_SUCCESS_THRESHOLD: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SwarmLeaseError {
    #[error("INVALID_LEASE_PATH: {0}")]
    InvalidPath(String),
    #[error("PATH_TRAVERSAL: {0}")]
    PathTraversal(String),
    #[error("SYMLINK_VIOLATION: {0}")]
    SymlinkViolation(String),
    #[error("LEASE_PATH_CONFLICT: requested '{requested}' overlaps active lease '{held_path}' held by worker '{held_by}'")]
    LeasePathConflict {
        requested: String,
        held_by: String,
        held_path: String,
    },
    #[error("STALE_FENCING_TOKEN: expected {expected}, provided {provided}")]
    StaleFencingToken { expected: i64, provided: i64 },
    #[error("LEASE_EXPIRED: lease '{lease_id}' expired at {expires_at} (now={now_ms})")]
    LeaseExpired {
        lease_id: String,
        expires_at: i64,
        now_ms: i64,
    },
    #[error("LEASE_RELEASED: lease '{0}' has already been released")]
    LeaseReleased(String),
    #[error("PATH_NOT_OWNED_BY_LEASE: '{path}' is outside lease '{lease_id}' owned paths")]
    PathNotOwnedByLease { lease_id: String, path: String },
    #[error("INVALID_CONTRACT: {0}")]
    InvalidContract(String),
    #[error("UNSAFE_PGID: refusing to signal protected process group {0}")]
    UnsafePgid(i32),
    #[error("IO_ERROR: {0}")]
    Io(String),
}

/// Canonical worker execution role persisted in `V27 worker_file_leases.worker_role`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkerRole {
    #[serde(rename = "BUILDER")]
    Builder,
    #[serde(rename = "VERIFIER_QC")]
    VerifierQc,
    #[serde(rename = "DESIGN_JURY")]
    DesignJury,
}

impl WorkerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builder => "BUILDER",
            Self::VerifierQc => "VERIFIER_QC",
            Self::DesignJury => "DESIGN_JURY",
        }
    }

    pub fn parse_strict(raw: &str) -> Result<Self, SwarmLeaseError> {
        match raw.trim() {
            "BUILDER" => Ok(Self::Builder),
            "VERIFIER_QC" => Ok(Self::VerifierQc),
            "DESIGN_JURY" => Ok(Self::DesignJury),
            other => Err(SwarmLeaseError::InvalidContract(format!(
                "Unsupported workerRole '{other}'; expected BUILDER, VERIFIER_QC, or DESIGN_JURY"
            ))),
        }
    }
}

/// In-memory projection of a `V27 worker_file_leases` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerLeaseRecord {
    pub lease_id: String,
    pub project_id: String,
    pub worker_id: String,
    pub worker_role: WorkerRole,
    pub worker_pgid: Option<i32>,
    pub fencing_token: i64,
    pub owned_paths: Vec<String>,
    pub expires_at: i64,
    pub released: bool,
    pub created_at: i64,
}

/// `.cowok-design/coordination/` `ONE_JOB + NEVER` task assignment contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneJobContract {
    pub task_id: String,
    pub project_id: String,
    pub worker_id: String,
    pub worker_role: WorkerRole,
    pub lease_id: String,
    pub fencing_token: i64,
    pub one_job: String,
    pub never_rules: Vec<String>,
    pub owned_paths: Vec<String>,
    pub staging_rel_dir: String,
    pub expires_at: i64,
    pub issued_at: i64,
}

/// Receipt emitted after validating `fencing_token` and committing a staged file into workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedCommitReceipt {
    pub lease_id: String,
    pub worker_id: String,
    pub fencing_token: i64,
    pub canonical_path: String,
    pub content_sha256: String,
    pub bytes_written: usize,
    pub committed_at: i64,
}

/// Validates a non-empty safe identifier (worker_id, lease_id, task_id).
pub fn validate_safe_worker_id(raw: &str, field_name: &str) -> Result<String, SwarmLeaseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(SwarmLeaseError::InvalidContract(format!(
            "{field_name} must be 1..=128 characters"
        )));
    }
    if trimmed == "."
        || trimmed.contains("..")
        || !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
    {
        return Err(SwarmLeaseError::InvalidContract(format!(
            "{field_name} contains disallowed characters"
        )));
    }
    Ok(trimmed.to_string())
}

#[cfg(unix)]
fn verify_existing_path_not_symlink_nofollow(path: &Path) -> Result<(), SwarmLeaseError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(SwarmLeaseError::Io(format!("{}: {e}", path.display()))),
    };
    if meta.file_type().is_symlink() {
        return Err(SwarmLeaseError::SymlinkViolation(format!(
            "Symlink component rejected at '{}'",
            path.display()
        )));
    }
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| SwarmLeaseError::InvalidPath("Path contains interior NUL byte".to_string()))?;
    // Open with O_NOFOLLOW | O_CLOEXEC to close TOCTOU symlink races on existing paths.
    let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: `c_path` is a valid NUL-terminated C string; descriptor is closed immediately.
    let fd = unsafe { libc::open(c_path.as_ptr(), flags) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ELOOP) {
            return Err(SwarmLeaseError::SymlinkViolation(format!(
                "O_NOFOLLOW rejected symlink at '{}'",
                path.display()
            )));
        }
        if err.kind() == std::io::ErrorKind::NotFound {
            return Ok(());
        }
        return Err(SwarmLeaseError::Io(format!(
            "O_NOFOLLOW check failed for '{}': {err}",
            path.display()
        )));
    }
    // SAFETY: `fd` was just opened by `libc::open` and is valid.
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_existing_path_not_symlink_nofollow(path: &Path) -> Result<(), SwarmLeaseError> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Err(SwarmLeaseError::SymlinkViolation(format!(
            "Symlink component rejected at '{}'",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(SwarmLeaseError::Io(format!("{}: {e}", path.display()))),
    }
}

/// Canonicalizes a worker-owned path relative to `workspace_root` (when supplied).
///
/// Security invariants:
/// - Rejects empty paths, NUL bytes, Windows drive prefixes, and any `..` traversal component.
/// - Rejects absolute paths outside `workspace_root` (and all absolute paths when `workspace_root` is `None`).
/// - Walks every existing path prefix under `workspace_root` with `symlink_metadata` + `O_NOFOLLOW`
///   to block symlink escapes (`src/pages/link -> /etc/passwd` or symlinked parent dirs).
/// - Produces a clean slash-separated relative path without leading `./` or trailing `/`.
pub fn canonicalize_owned_path(
    workspace_root: Option<&Path>,
    raw_path: &str,
) -> Result<String, SwarmLeaseError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(SwarmLeaseError::InvalidPath(
            "Owned path must not be empty".to_string(),
        ));
    }
    if trimmed.bytes().any(|b| b == 0) {
        return Err(SwarmLeaseError::InvalidPath(
            "Owned path contains NUL byte".to_string(),
        ));
    }

    let normalized_slashes = trimmed.replace('\\', "/");
    let candidate = Path::new(&normalized_slashes);

    let relative_path: PathBuf = if candidate.is_absolute() {
        let Some(root) = workspace_root else {
            return Err(SwarmLeaseError::PathTraversal(format!(
                "Absolute path '{trimmed}' is forbidden outside workspace root"
            )));
        };
        let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        if let Ok(stripped) = candidate.strip_prefix(&canon_root) {
            stripped.to_path_buf()
        } else if let Ok(stripped) = candidate.strip_prefix(root) {
            stripped.to_path_buf()
        } else {
            return Err(SwarmLeaseError::PathTraversal(format!(
                "Absolute path '{trimmed}' escapes workspace root '{}'",
                root.display()
            )));
        }
    } else {
        candidate.to_path_buf()
    };

    let mut clean_segments: Vec<String> = Vec::new();
    for comp in relative_path.components() {
        match comp {
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(SwarmLeaseError::PathTraversal(format!(
                    "Parent directory traversal ('..') rejected in '{trimmed}'"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(SwarmLeaseError::PathTraversal(format!(
                    "Root or drive prefix rejected in '{trimmed}'"
                )));
            }
            Component::Normal(seg) => {
                let s = seg.to_str().ok_or_else(|| {
                    SwarmLeaseError::InvalidPath(format!(
                        "Non-UTF8 path segment rejected in '{trimmed}'"
                    ))
                })?;
                if s.is_empty() || s == "." || s == ".." {
                    return Err(SwarmLeaseError::PathTraversal(format!(
                        "Disallowed segment '{s}' in '{trimmed}'"
                    )));
                }
                clean_segments.push(s.to_string());
            }
        }
    }

    if clean_segments.is_empty() {
        return Err(SwarmLeaseError::InvalidPath(
            "Owned path must target a specific file or subdirectory, not workspace root"
                .to_string(),
        ));
    }

    if let Some(root) = workspace_root {
        let mut cursor = root.to_path_buf();
        for seg in &clean_segments {
            cursor.push(seg);
            verify_existing_path_not_symlink_nofollow(&cursor)?;
        }
    }

    Ok(clean_segments.join("/"))
}

fn normalize_segments_owned(path: &str) -> Vec<String> {
    path.trim()
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .map(str::to_string)
        .collect()
}

/// Returns `true` when two canonicalized paths collide by equality or ancestor/descendant prefix.
///
/// Examples:
/// - `paths_overlap_canonical("src/pages/", "src/pages/Home.tsx") == true`
/// - `paths_overlap_canonical("src/a/b.ts", "src/a") == true`
/// - `paths_overlap_canonical("src/pages", "src/pages-v2/Home.tsx") == false`
pub fn paths_overlap_canonical(a: &str, b: &str) -> bool {
    let segs_a = normalize_segments_owned(a);
    let segs_b = normalize_segments_owned(b);
    if segs_a.is_empty() || segs_b.is_empty() {
        return true;
    }
    let min_len = segs_a.len().min(segs_b.len());
    segs_a[..min_len] == segs_b[..min_len]
}

/// Returns `true` if `target_path` is strictly covered by at least one owned path (exact or descendant).
pub fn is_path_covered_by_owned_set(owned_paths: &[String], target_path: &str) -> bool {
    let target_segs = normalize_segments_owned(target_path);
    if target_segs.is_empty() {
        return false;
    }
    owned_paths.iter().any(|owned| {
        let owned_segs = normalize_segments_owned(owned);
        !owned_segs.is_empty()
            && owned_segs.len() <= target_segs.len()
            && target_segs[..owned_segs.len()] == owned_segs[..]
    })
}

/// Canonicalizes a single worker's requested `owned_paths` and rejects internal duplicates/overlaps.
pub fn canonicalize_and_check_self_disjoint(
    workspace_root: Option<&Path>,
    worker_id: &str,
    raw_paths: &[String],
) -> Result<Vec<String>, SwarmLeaseError> {
    if raw_paths.is_empty() {
        return Err(SwarmLeaseError::InvalidPath(
            "ownedPaths must contain at least one path".to_string(),
        ));
    }
    let mut canonical: Vec<String> = Vec::with_capacity(raw_paths.len());
    for raw in raw_paths {
        let norm = canonicalize_owned_path(workspace_root, raw)?;
        for existing in &canonical {
            if paths_overlap_canonical(existing, &norm) {
                return Err(SwarmLeaseError::LeasePathConflict {
                    requested: norm,
                    held_by: worker_id.to_string(),
                    held_path: existing.clone(),
                });
            }
        }
        canonical.push(norm);
    }
    Ok(canonical)
}

/// Checks requested canonical paths against all currently active (unreleased, non-expired) leases.
pub fn check_conflict_against_active_leases(
    requested_paths: &[String],
    active_leases: &[WorkerLeaseRecord],
    now_ms: i64,
) -> Result<(), SwarmLeaseError> {
    for req_path in requested_paths {
        for lease in active_leases {
            if lease.released || lease.expires_at <= now_ms {
                continue;
            }
            for held in &lease.owned_paths {
                if paths_overlap_canonical(req_path, held) {
                    return Err(SwarmLeaseError::LeasePathConflict {
                        requested: req_path.clone(),
                        held_by: lease.worker_id.clone(),
                        held_path: held.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Validates pairwise disjointness across a parallel swarm dispatch batch `(worker_id, raw_paths)`.
pub fn validate_disjoint_write_sets(
    workspace_root: Option<&Path>,
    worker_sets: &[(String, Vec<String>)],
) -> Result<Vec<(String, Vec<String>)>, SwarmLeaseError> {
    let mut seen_workers = BTreeSet::new();
    let mut canonical_sets: Vec<(String, Vec<String>)> = Vec::with_capacity(worker_sets.len());

    for (worker_id, raw_paths) in worker_sets {
        let safe_worker = validate_safe_worker_id(worker_id, "workerId")?;
        if !seen_workers.insert(safe_worker.clone()) {
            return Err(SwarmLeaseError::InvalidContract(format!(
                "Duplicate workerId '{safe_worker}' in single parallel dispatch batch"
            )));
        }
        let canon_paths =
            canonicalize_and_check_self_disjoint(workspace_root, &safe_worker, raw_paths)?;
        for (prev_worker, prev_paths) in &canonical_sets {
            for req in &canon_paths {
                for held in prev_paths {
                    if paths_overlap_canonical(req, held) {
                        return Err(SwarmLeaseError::LeasePathConflict {
                            requested: req.clone(),
                            held_by: prev_worker.clone(),
                            held_path: held.clone(),
                        });
                    }
                }
            }
        }
        canonical_sets.push((safe_worker, canon_paths));
    }

    Ok(canonical_sets)
}

/// Filesystem mailbox managing `.cowok-design/coordination/` `ONE_JOB + NEVER` task envelopes
/// and `fencing_token`-checked staging commits.
#[derive(Debug, Clone)]
pub struct CoordinationMailbox {
    workspace_root: PathBuf,
    coordination_root: PathBuf,
}

impl CoordinationMailbox {
    pub fn open(workspace_root: impl Into<PathBuf>) -> Result<Self, SwarmLeaseError> {
        let workspace_root = workspace_root.into();
        ensure_dir_chain_not_symlink(&workspace_root, &[".cowok-design", "coordination", "tasks"])?;
        ensure_dir_chain_not_symlink(
            &workspace_root,
            &[".cowok-design", "coordination", "staging"],
        )?;
        let coordination_root = workspace_root.join(COORDINATION_REL_DIR);

        Ok(Self {
            workspace_root,
            coordination_root,
        })
    }

    pub fn coordination_root(&self) -> &Path {
        &self.coordination_root
    }

    /// Writes a `ONE_JOB + NEVER` coordination task envelope atomically under
    /// `.cowok-design/coordination/tasks/<task_id>.json`.
    pub fn write_task_envelope(
        &self,
        contract: &OneJobContract,
    ) -> Result<PathBuf, SwarmLeaseError> {
        validate_safe_worker_id(&contract.task_id, "taskId")?;
        validate_safe_worker_id(&contract.worker_id, "workerId")?;
        validate_safe_worker_id(&contract.lease_id, "leaseId")?;
        if contract.one_job.trim().is_empty() {
            return Err(SwarmLeaseError::InvalidContract(
                "ONE_JOB contract directive must not be empty".to_string(),
            ));
        }
        if contract.never_rules.is_empty()
            || contract.never_rules.iter().all(|r| r.trim().is_empty())
        {
            return Err(SwarmLeaseError::InvalidContract(
                "NEVER boundary directives must contain at least one explicit prohibition"
                    .to_string(),
            ));
        }
        if contract.owned_paths.is_empty() {
            return Err(SwarmLeaseError::InvalidContract(
                "ONE_JOB contract must declare at least one leased owned path".to_string(),
            ));
        }

        let tasks_dir = verify_prefix_chain_not_symlink(
            &self.workspace_root,
            &[".cowok-design", "coordination", "tasks"],
        )?;
        let final_path = tasks_dir.join(format!("{}.json", contract.task_id));
        verify_existing_path_not_symlink_nofollow(&final_path)?;
        let tmp_path = tasks_dir.join(format!(
            ".{}.{}.tmp",
            contract.task_id,
            Uuid::new_v4().simple()
        ));

        let payload =
            serde_json::to_vec_pretty(contract).map_err(|e| SwarmLeaseError::Io(e.to_string()))?;
        write_atomic_private(&tmp_path, &final_path, &payload)?;
        Ok(final_path)
    }

    /// Stages a worker's file output inside `.cowok-design/coordination/staging/<lease_id>/<rel_path>`.
    pub fn stage_file(
        &self,
        lease: &WorkerLeaseRecord,
        relative_path: &str,
        content: &[u8],
    ) -> Result<PathBuf, SwarmLeaseError> {
        let safe_lease_id = validate_safe_worker_id(&lease.lease_id, "leaseId")?;
        let canonical = canonicalize_owned_path(Some(&self.workspace_root), relative_path)?;
        if !is_path_covered_by_owned_set(&lease.owned_paths, &canonical) {
            return Err(SwarmLeaseError::PathNotOwnedByLease {
                lease_id: lease.lease_id.clone(),
                path: canonical,
            });
        }

        let canonical_segs: Vec<&str> = canonical.split('/').collect();
        let mut stage_segments: Vec<&str> = Vec::with_capacity(4 + canonical_segs.len());
        stage_segments.extend_from_slice(&[
            ".cowok-design",
            "coordination",
            "staging",
            safe_lease_id.as_str(),
        ]);
        stage_segments.extend_from_slice(&canonical_segs);

        // Walk every prefix from workspace_root through staging/<lease_id>/<canonical> BEFORE mkdir/open.
        let stage_target = verify_prefix_chain_not_symlink(&self.workspace_root, &stage_segments)?;
        if stage_segments.len() > 1 {
            ensure_dir_chain_not_symlink(
                &self.workspace_root,
                &stage_segments[..stage_segments.len() - 1],
            )?;
        }
        verify_existing_path_not_symlink_nofollow(&stage_target)?;

        let tmp_path = stage_target.with_extension(format!("tmp.{}", Uuid::new_v4().simple()));
        write_atomic_private(&tmp_path, &stage_target, content)?;
        Ok(stage_target)
    }

    /// Validates that `active_lease` is unreleased, non-expired, and matches `provided_fencing_token`
    /// before atomically committing the staged file into `<workspace_root>/<canonical_path>`.
    pub fn commit_staged_file(
        &self,
        active_lease: &WorkerLeaseRecord,
        provided_fencing_token: i64,
        relative_path: &str,
        now_ms: i64,
    ) -> Result<StagedCommitReceipt, SwarmLeaseError> {
        let safe_lease_id = validate_safe_worker_id(&active_lease.lease_id, "leaseId")?;
        if active_lease.released {
            return Err(SwarmLeaseError::LeaseReleased(
                active_lease.lease_id.clone(),
            ));
        }
        if active_lease.expires_at <= now_ms {
            return Err(SwarmLeaseError::LeaseExpired {
                lease_id: active_lease.lease_id.clone(),
                expires_at: active_lease.expires_at,
                now_ms,
            });
        }
        if provided_fencing_token != active_lease.fencing_token {
            return Err(SwarmLeaseError::StaleFencingToken {
                expected: active_lease.fencing_token,
                provided: provided_fencing_token,
            });
        }

        let canonical = canonicalize_owned_path(Some(&self.workspace_root), relative_path)?;
        if !is_path_covered_by_owned_set(&active_lease.owned_paths, &canonical) {
            return Err(SwarmLeaseError::PathNotOwnedByLease {
                lease_id: active_lease.lease_id.clone(),
                path: canonical,
            });
        }

        let canonical_segs: Vec<&str> = canonical.split('/').collect();
        let mut stage_segments: Vec<&str> = Vec::with_capacity(4 + canonical_segs.len());
        stage_segments.extend_from_slice(&[
            ".cowok-design",
            "coordination",
            "staging",
            safe_lease_id.as_str(),
        ]);
        stage_segments.extend_from_slice(&canonical_segs);

        // Walk every prefix from workspace_root through staging/<lease_id>/<canonical> BEFORE opening.
        let stage_file = verify_prefix_chain_not_symlink(&self.workspace_root, &stage_segments)?;
        let bytes = read_file_nofollow(&stage_file)?;

        // Walk every destination prefix from workspace_root through <canonical> BEFORE creating dirs or renaming.
        let dest_path = verify_prefix_chain_not_symlink(&self.workspace_root, &canonical_segs)?;
        if canonical_segs.len() > 1 {
            ensure_dir_chain_not_symlink(
                &self.workspace_root,
                &canonical_segs[..canonical_segs.len() - 1],
            )?;
        }
        verify_existing_path_not_symlink_nofollow(&dest_path)?;

        let tmp_dest = dest_path
            .parent()
            .unwrap_or(&self.workspace_root)
            .join(format!(".commit.{}.tmp", Uuid::new_v4().simple()));
        write_atomic_private(&tmp_dest, &dest_path, &bytes)?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let content_sha256 = format!("{:x}", hasher.finalize());

        Ok(StagedCommitReceipt {
            lease_id: active_lease.lease_id.clone(),
            worker_id: active_lease.worker_id.clone(),
            fencing_token: active_lease.fencing_token,
            canonical_path: canonical,
            content_sha256,
            bytes_written: bytes.len(),
            committed_at: Utc::now().timestamp_millis(),
        })
    }
}

fn verify_prefix_chain_not_symlink(
    base: &Path,
    segments: &[&str],
) -> Result<PathBuf, SwarmLeaseError> {
    let mut cursor = base.to_path_buf();
    for seg in segments {
        if seg.is_empty() || *seg == "." || *seg == ".." {
            return Err(SwarmLeaseError::PathTraversal(format!(
                "Disallowed path segment '{seg}'"
            )));
        }
        cursor.push(seg);
        verify_existing_path_not_symlink_nofollow(&cursor)?;
    }
    Ok(cursor)
}

fn ensure_dir_chain_not_symlink(
    base: &Path,
    dir_segments: &[&str],
) -> Result<PathBuf, SwarmLeaseError> {
    let mut cursor = base.to_path_buf();
    for seg in dir_segments {
        if seg.is_empty() || *seg == "." || *seg == ".." {
            return Err(SwarmLeaseError::PathTraversal(format!(
                "Disallowed path segment '{seg}'"
            )));
        }
        cursor.push(seg);
        verify_existing_path_not_symlink_nofollow(&cursor)?;
        match fs::create_dir(&cursor) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(SwarmLeaseError::Io(format!(
                    "create_dir '{}': {e}",
                    cursor.display()
                )));
            }
        }
        verify_existing_path_not_symlink_nofollow(&cursor)?;
    }
    Ok(cursor)
}

fn read_file_nofollow(path: &Path) -> Result<Vec<u8>, SwarmLeaseError> {
    #[cfg(unix)]
    {
        let mut opts = OpenOptions::new();
        opts.read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let mut file = opts.open(path).map_err(|e| {
            if e.raw_os_error() == Some(libc::ELOOP) {
                SwarmLeaseError::SymlinkViolation(format!(
                    "O_NOFOLLOW rejected symlink at '{}'",
                    path.display()
                ))
            } else {
                SwarmLeaseError::Io(format!(
                    "Failed to open staged file '{}': {e}",
                    path.display()
                ))
            }
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| {
            SwarmLeaseError::Io(format!(
                "Failed to read staged file '{}': {e}",
                path.display()
            ))
        })?;
        Ok(bytes)
    }
    #[cfg(not(unix))]
    {
        verify_existing_path_not_symlink_nofollow(path)?;
        let mut file = fs::File::open(path).map_err(|e| {
            SwarmLeaseError::Io(format!(
                "Failed to open staged file '{}': {e}",
                path.display()
            ))
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| {
            SwarmLeaseError::Io(format!(
                "Failed to read staged file '{}': {e}",
                path.display()
            ))
        })?;
        Ok(bytes)
    }
}

fn write_atomic_private(
    tmp_path: &Path,
    final_path: &Path,
    bytes: &[u8],
) -> Result<(), SwarmLeaseError> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = opts
        .open(tmp_path)
        .map_err(|e| SwarmLeaseError::Io(format!("open tmp '{}': {e}", tmp_path.display())))?;
    file.write_all(bytes)
        .map_err(|e| SwarmLeaseError::Io(format!("write tmp '{}': {e}", tmp_path.display())))?;
    file.sync_all()
        .map_err(|e| SwarmLeaseError::Io(format!("sync tmp '{}': {e}", tmp_path.display())))?;
    drop(file);
    fs::rename(tmp_path, final_path).map_err(|e| {
        let _ = fs::remove_file(tmp_path);
        SwarmLeaseError::Io(format!(
            "rename '{}' -> '{}': {e}",
            tmp_path.display(),
            final_path.display()
        ))
    })
}

/// Snapshot of the `429` adaptive concurrency semaphore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshot {
    pub current_limit: usize,
    pub initial_limit: usize,
    pub clamped: bool,
    pub in_flight: usize,
    pub consecutive_429: u32,
    pub consecutive_success: u32,
    pub last_backoff_ms: u64,
}

/// Transition returned when `on_http_429` clamps the swarm semaphore `3 -> 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClampTransition {
    pub previous_limit: usize,
    pub current_limit: usize,
    pub backoff_ms: u64,
    pub consecutive_429: u32,
}

#[derive(Debug)]
struct RateLimitState {
    initial_limit: usize,
    current_limit: usize,
    in_flight: usize,
    consecutive_429: u32,
    consecutive_success: u32,
    last_backoff_ms: u64,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
}

/// RAII concurrency permit released automatically when a worker task completes or unwinds.
#[derive(Debug)]
pub struct RateLimitPermit {
    state: Arc<Mutex<RateLimitState>>,
}

impl Drop for RateLimitPermit {
    fn drop(&mut self) {
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.in_flight = guard.in_flight.saturating_sub(1);
    }
}

/// Adaptive parallel fleet semaphore starting at `INITIAL_CONCURRENCY = 3` and
/// deterministically clamping `3 -> 1` upon HTTP `429` with bounded jittered exponential backoff.
#[derive(Debug, Clone)]
pub struct RateLimitSemaphore {
    state: Arc<Mutex<RateLimitState>>,
}

impl Default for RateLimitSemaphore {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitSemaphore {
    pub fn new() -> Self {
        Self::with_config(INITIAL_CONCURRENCY, DEFAULT_BASE_BACKOFF_MS, MAX_BACKOFF_MS)
    }

    pub fn with_config(initial_limit: usize, base_backoff_ms: u64, max_backoff_ms: u64) -> Self {
        let safe_initial = initial_limit.max(CLAMPED_CONCURRENCY);
        let safe_base = base_backoff_ms.max(10);
        let safe_max = max_backoff_ms.max(safe_base);
        Self {
            state: Arc::new(Mutex::new(RateLimitState {
                initial_limit: safe_initial,
                current_limit: safe_initial,
                in_flight: 0,
                consecutive_429: 0,
                consecutive_success: 0,
                last_backoff_ms: 0,
                base_backoff_ms: safe_base,
                max_backoff_ms: safe_max,
            })),
        }
    }

    pub fn current_limit(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .current_limit
    }

    pub fn is_clamped(&self) -> bool {
        let guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        guard.current_limit < guard.initial_limit
    }

    pub fn snapshot(&self) -> RateLimitSnapshot {
        let guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        RateLimitSnapshot {
            current_limit: guard.current_limit,
            initial_limit: guard.initial_limit,
            clamped: guard.current_limit < guard.initial_limit,
            in_flight: guard.in_flight,
            consecutive_429: guard.consecutive_429,
            consecutive_success: guard.consecutive_success,
            last_backoff_ms: guard.last_backoff_ms,
        }
    }

    /// Attempts to acquire one concurrency slot respecting the live (possibly clamped) limit.
    pub fn try_acquire(&self) -> Option<RateLimitPermit> {
        let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if guard.in_flight >= guard.current_limit {
            return None;
        }
        guard.in_flight += 1;
        Some(RateLimitPermit {
            state: Arc::clone(&self.state),
        })
    }

    /// Immediately clamps concurrency `3 -> 1` upon HTTP `429` and computes bounded jittered exponential backoff.
    pub fn on_http_429(&self, retry_after_ms: Option<u64>, jitter_seed: u64) -> ClampTransition {
        let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let previous_limit = guard.current_limit;
        guard.current_limit = CLAMPED_CONCURRENCY;
        guard.consecutive_429 = guard.consecutive_429.saturating_add(1);
        guard.consecutive_success = 0;

        let exp_shift = (guard.consecutive_429 - 1).min(8);
        let base_exp = guard
            .base_backoff_ms
            .saturating_mul(1u64 << exp_shift)
            .min(guard.max_backoff_ms);
        let jitter_span = (base_exp / 4).max(1);
        // SplitMix64 single-step mixing for deterministic bounded jitter.
        let mixed = jitter_seed
            .wrapping_add(0x9E37_79B9_7F4A_7C15)
            .wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let jitter = (mixed ^ (mixed >> 27)) % jitter_span;
        let computed = base_exp.saturating_add(jitter).min(guard.max_backoff_ms);
        let backoff_ms = retry_after_ms
            .map(|ra| ra.max(computed).min(guard.max_backoff_ms))
            .unwrap_or(computed);
        guard.last_backoff_ms = backoff_ms;

        ClampTransition {
            previous_limit,
            current_limit: guard.current_limit,
            backoff_ms,
            consecutive_429: guard.consecutive_429,
        }
    }

    /// Records a clean execution and gradually recovers concurrency back toward `INITIAL_CONCURRENCY`.
    pub fn on_success(&self) -> usize {
        let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        guard.consecutive_429 = 0;
        guard.consecutive_success = guard.consecutive_success.saturating_add(1);
        if guard.current_limit < guard.initial_limit
            && guard.consecutive_success >= RECOVERY_SUCCESS_THRESHOLD
        {
            guard.current_limit = (guard.current_limit + 1).min(guard.initial_limit);
            guard.consecutive_success = 0;
            if guard.current_limit == guard.initial_limit {
                guard.last_backoff_ms = 0;
            }
        }
        guard.current_limit
    }
}

/// Injectable PGID killer abstraction allowing deterministic tests without signalling live host processes.
pub trait PgidKiller: Send + Sync {
    fn kill_pgid(&self, pgid: i32) -> Result<(), SwarmLeaseError>;
}

/// Returns the current process group ID so the reaper never signals its own daemon process group.
pub fn current_process_pgid() -> Option<i32> {
    #[cfg(unix)]
    {
        // SAFETY: `getpgrp` is a standard POSIX query with no memory safety preconditions.
        Some(unsafe { libc::getpgrp() })
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Validates that `pgid` is strictly safe to terminate (`pgid > 1` and `pgid != current_process_pgid()`).
pub fn is_safe_worker_pgid(pgid: i32) -> bool {
    if pgid <= 1 {
        return false;
    }
    if let Some(self_pgid) = current_process_pgid() {
        if pgid == self_pgid {
            return false;
        }
    }
    true
}

/// Production POSIX `-PGID` killer enforcing `pgid > 1` and `pgid != self_pgid` before `kill(-pgid, SIGKILL)`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PosixPgidKiller;

impl PgidKiller for PosixPgidKiller {
    fn kill_pgid(&self, pgid: i32) -> Result<(), SwarmLeaseError> {
        if !is_safe_worker_pgid(pgid) {
            return Err(SwarmLeaseError::UnsafePgid(pgid));
        }
        #[cfg(unix)]
        {
            // SAFETY: `pgid` is verified `> 1` and distinct from current process group; negative PID targets worker PGID only.
            let rc = unsafe { libc::kill(-pgid, libc::SIGKILL) };
            if rc == 0 {
                return Ok(());
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            Err(SwarmLeaseError::Io(format!(
                "kill(-{pgid}, SIGKILL) failed: {err}"
            )))
        }
        #[cfg(not(unix))]
        {
            let _ = pgid;
            Ok(())
        }
    }
}

/// Audit record emitted for each expired lease processed by `reap_expired_leases_and_kill_pgid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReapLeaseOutcome {
    pub lease_id: String,
    pub worker_id: String,
    pub killed_pgid: Option<i32>,
    pub skipped_unsafe_pgid: bool,
    pub released: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kill_error: Option<String>,
}

/// Reaps all expired active leases, issuing `kill(-pgid, SIGKILL)` to safe owned worker PGIDs
/// strictly BEFORE marking each lease as released (`AC-03`).
///
/// Failures from `killer.kill_pgid` are handled per-lease: the failing lease stays unreleased
/// with `kill_error` recorded on its outcome while reaping continues for all remaining expired leases.
pub fn reap_expired_leases_and_kill_pgid<K: PgidKiller>(
    leases: &mut [WorkerLeaseRecord],
    now_ms: i64,
    killer: &K,
) -> Vec<ReapLeaseOutcome> {
    let mut outcomes = Vec::new();
    for lease in leases.iter_mut() {
        if lease.released || lease.expires_at > now_ms {
            continue;
        }
        let mut killed_pgid = None;
        let mut skipped_unsafe_pgid = false;

        if let Some(pgid) = lease.worker_pgid {
            if is_safe_worker_pgid(pgid) {
                match killer.kill_pgid(pgid) {
                    Ok(()) => {
                        killed_pgid = Some(pgid);
                    }
                    Err(err) => {
                        outcomes.push(ReapLeaseOutcome {
                            lease_id: lease.lease_id.clone(),
                            worker_id: lease.worker_id.clone(),
                            killed_pgid: None,
                            skipped_unsafe_pgid: false,
                            released: false,
                            kill_error: Some(err.to_string()),
                        });
                        continue;
                    }
                }
            } else {
                skipped_unsafe_pgid = true;
            }
        }

        // Only release the lease AFTER the worker PGID has been terminated (or verified skipped).
        lease.released = true;
        outcomes.push(ReapLeaseOutcome {
            lease_id: lease.lease_id.clone(),
            worker_id: lease.worker_id.clone(),
            killed_pgid,
            skipped_unsafe_pgid,
            released: true,
            kill_error: None,
        });
    }
    outcomes
}
