// @story 02.4
// @implements AC-02, AC-04
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use crate::locks::{
    current_epoch_ms, is_valid_ssot_portal, retry_on_busy_with_jitter, AppShellLockDto,
};
use crate::reverse_sync::ComponentPatternDto;

pub const EXPORT_RETENTION_KEEP_COUNT: usize = 5;
pub const EXPORT_RETENTION_GRACE_MS: i64 = 60_000;

static EXPORT_MUTEX_MAP: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();

fn get_export_process_mutex(tenant_id: &str, project_id: &str) -> Arc<Mutex<()>> {
    let map = EXPORT_MUTEX_MAP.get_or_init(|| Mutex::new(HashMap::new()));
    let key = format!("{tenant_id}\0{project_id}");
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn with_export_lock<R, F>(
    workspace_root: &Path,
    tenant_id: &str,
    project_id: &str,
    timeout: Duration,
    f: F,
) -> Result<R, SsotExportError>
where
    F: FnOnce() -> Result<R, SsotExportError>,
{
    let mutex = get_export_process_mutex(tenant_id, project_id);
    let _in_proc = mutex.lock().unwrap_or_else(|e| e.into_inner());

    verify_no_symlink_in_path(workspace_root, Path::new(".cowok-locks"))?;
    let locks_dir = workspace_root.join(".cowok-locks");
    fs::create_dir_all(&locks_dir).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to create lock dir: {e}"),
    })?;

    let mut hasher = Sha256::new();
    hasher.update(b"ssot_export\0");
    hasher.update(tenant_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(project_id.as_bytes());
    let lock_path = locks_dir.join(format!("export-{:x}.lock", hasher.finalize()));

    if let Ok(meta) = fs::symlink_metadata(&lock_path) {
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(SsotExportError::PathTraversal {
                path: lock_path.display().to_string(),
            });
        }
    }

    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);

    let file = opts.open(&lock_path).map_err(|e| {
        #[cfg(unix)]
        if e.raw_os_error() == Some(libc::ELOOP) {
            return SsotExportError::PathTraversal {
                path: lock_path.display().to_string(),
            };
        }
        SsotExportError::Io {
            code: "INTERNAL_ERROR",
            message: format!("Failed to open export lock file: {e}"),
        }
    })?;

    #[cfg(unix)]
    {
        let start = Instant::now();
        let fd = file.as_raw_fd();
        loop {
            let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if rc == 0 {
                break;
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EWOULDBLOCK) && start.elapsed() < timeout {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            return Err(SsotExportError::StorageBusy {
                details: format!("Export lock contention: {err}"),
            });
        }
    }

    let result = f();

    #[cfg(unix)]
    unsafe {
        libc::flock(file.as_raw_fd(), libc::LOCK_UN);
    }

    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifestFileEntry {
    pub relative_path: String,
    pub sha256: String,
    pub byte_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifest {
    pub export_id: String,
    pub export_revision: i64,
    pub tenant_id: String,
    pub project_id: String,
    pub portal_id: String,
    pub generation_id: String,
    pub generation_rel_dir: String,
    pub markdown_rel_path: String,
    pub markdown_sha256: String,
    pub json_rel_path: String,
    pub json_sha256: String,
    pub checksum_sha256: String,
    pub component_count: usize,
    pub created_at_ms: i64,
    pub files: Vec<ExportManifestFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SsotExportResult {
    pub export_id: String,
    pub revision: i64,
    pub portal_id: String,
    pub generated_at: i64,
    pub markdown_path: String,
    pub markdown_sha256: String,
    pub json_path: String,
    pub json_sha256: String,
}

#[derive(Debug, Clone)]
pub struct SsotExportRequest {
    pub tenant_id: String,
    pub project_id: String,
    pub portal_id: String,
    pub formats: Vec<String>,
    pub include_quarantined: bool,
    pub relative_export_subdir: Option<String>,
    pub now_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsotExportError {
    Validation { code: &'static str, message: String },
    PathTraversal { path: String },
    NotFound { code: &'static str, message: String },
    ExportSuperseded {
        attempted_revision: i64,
        latest_revision: i64,
    },
    ManifestInvalid { reason: String },
    StorageBusy { details: String },
    Io { code: &'static str, message: String },
}

impl std::fmt::Display for SsotExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsotExportError::Validation { code, message }
            | SsotExportError::NotFound { code, message }
            | SsotExportError::Io { code, message } => write!(f, "{code}: {message}"),
            SsotExportError::PathTraversal { path } => {
                write!(f, "PATH_JAIL_VIOLATION: {path}")
            }
            SsotExportError::ExportSuperseded {
                attempted_revision,
                latest_revision,
            } => write!(
                f,
                "EXPORT_SUPERSEDED: attempted revision {attempted_revision}, latest revision is {latest_revision}"
            ),
            SsotExportError::ManifestInvalid { reason } => {
                write!(f, "EXPORT_MANIFEST_INVALID: {reason}")
            }
            SsotExportError::StorageBusy { details } => write!(f, "STORAGE_BUSY: {details}"),
        }
    }
}

impl std::error::Error for SsotExportError {}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn verify_no_symlink_in_path(root: &Path, rel: &Path) -> Result<(), SsotExportError> {
    let mut cursor = root.to_path_buf();
    for comp in rel.components() {
        match comp {
            Component::Normal(seg) => {
                cursor.push(seg);
                if let Ok(meta) = fs::symlink_metadata(&cursor) {
                    if meta.file_type().is_symlink() {
                        return Err(SsotExportError::PathTraversal {
                            path: cursor.display().to_string(),
                        });
                    }
                }
            }
            Component::CurDir => {}
            _ => {
                return Err(SsotExportError::PathTraversal {
                    path: rel.display().to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Resolves and jails the export root directory strictly under `<workspace_root>/.cowok-design/exports`.
pub fn resolve_jailed_export_root(
    workspace_root: &Path,
    relative_subdir: Option<&str>,
) -> Result<PathBuf, SsotExportError> {
    let base_rel = PathBuf::from(".cowok-design").join("exports");
    verify_no_symlink_in_path(workspace_root, &base_rel)?;

    let mut target_rel = base_rel;
    if let Some(sub) = relative_subdir {
        let trimmed = sub.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with('/') || trimmed.contains('\\') || trimmed.contains('\0') {
                return Err(SsotExportError::PathTraversal {
                    path: trimmed.to_string(),
                });
            }
            let sub_path = Path::new(trimmed);
            for comp in sub_path.components() {
                match comp {
                    Component::Normal(seg) => target_rel.push(seg),
                    _ => {
                        return Err(SsotExportError::PathTraversal {
                            path: trimmed.to_string(),
                        });
                    }
                }
            }
            verify_no_symlink_in_path(workspace_root, &target_rel)?;
        }
    }

    let full = workspace_root.join(&target_rel);
    fs::create_dir_all(&full).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to create export dir: {e}"),
    })?;

    let canonical_base = fs::canonicalize(workspace_root.join(".cowok-design/exports")).map_err(
        |e| SsotExportError::Io {
            code: "INTERNAL_ERROR",
            message: format!("Failed to canonicalize base export dir: {e}"),
        },
    )?;
    let canonical_target = fs::canonicalize(&full).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to canonicalize target export dir: {e}"),
    })?;

    if !canonical_target.starts_with(&canonical_base) {
        return Err(SsotExportError::PathTraversal {
            path: canonical_target.display().to_string(),
        });
    }

    Ok(canonical_target)
}

fn open_file_nofollow_read(path: &Path) -> Result<File, std::io::Error> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::from_raw_os_error(libc::ELOOP));
        }
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NOFOLLOW);
    opts.open(path)
}

fn write_atomic_file(target: &Path, bytes: &[u8]) -> Result<(), SsotExportError> {
    let parent = target.parent().ok_or_else(|| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: "Missing parent directory".to_string(),
    })?;
    fs::create_dir_all(parent).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to create parent dir: {e}"),
    })?;

    let tmp_name = format!(
        ".tmp-{}-{}",
        Uuid::new_v4(),
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
    );
    let tmp_path = parent.join(tmp_name);
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);

    let mut file = opts.open(&tmp_path).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to open staged file: {e}"),
    })?;
    file.write_all(bytes).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to write staged file: {e}"),
    })?;
    file.sync_all().map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to fsync staged file: {e}"),
    })?;
    fs::rename(&tmp_path, target).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to rename staged file: {e}"),
    })?;
    Ok(())
}

/// DB snapshot + atomic monotonic export revision allocator (`BEGIN IMMEDIATE` on `idempotency_records`).
pub struct ExportSnapshot {
    pub export_revision: i64,
    pub patterns: Vec<ComponentPatternDto>,
    pub locks: Vec<AppShellLockDto>,
}

pub fn allocate_export_snapshot(
    conn: &mut Connection,
    tenant_id: &str,
    project_id: &str,
    portal_id: &str,
    include_quarantined: bool,
    now_ms: i64,
) -> Result<ExportSnapshot, SsotExportError> {
    if tenant_id.trim().is_empty() || project_id.trim().is_empty() {
        return Err(SsotExportError::Validation {
            code: "INVALID_REQUEST",
            message: "tenantId and projectId must be non-empty".to_string(),
        });
    }
    if !is_valid_ssot_portal(portal_id) {
        return Err(SsotExportError::Validation {
            code: "INVALID_REQUEST",
            message: format!("Unsupported portalId: {portal_id}"),
        });
    }

    let idemp_key = format!("export_rev:{project_id}");
    let _ = conn.busy_timeout(Duration::from_millis(250));

    enum Outcome {
        Ok(ExportSnapshot),
        ProjectNotFound,
    }

    let res = retry_on_busy_with_jitter(
        tenant_id,
        project_id,
        portal_id,
        "allocate_export_snapshot",
        || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let project_exists: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2",
                    params![project_id, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            if project_exists.is_none() {
                return Ok(Outcome::ProjectNotFound);
            }

            let prev_rev: i64 = tx
                .query_row(
                    "SELECT lease_expires_at FROM idempotency_records \
                     WHERE tenant_id = ?1 AND actor_id = '__ssot_export__' AND idempotency_key = ?2",
                    params![tenant_id, idemp_key],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or(0);
            let next_rev = prev_rev.saturating_add(1);

            tx.execute(
                "INSERT INTO idempotency_records \
                 (tenant_id, actor_id, idempotency_key, request_sha256, status, lease_expires_at, response_status, response_body_json, expires_at) \
                 VALUES (?1, '__ssot_export__', ?2, ?3, 'COMPLETED', ?4, 200, NULL, 4000000000000) \
                 ON CONFLICT(tenant_id, actor_id, idempotency_key) DO UPDATE SET \
                   lease_expires_at = excluded.lease_expires_at, \
                   request_sha256 = excluded.request_sha256",
                params![tenant_id, idemp_key, format!("rev:{next_rev}"), next_rev],
            )?;

            let mut patterns = Vec::new();
            {
                let mut stmt = tx.prepare(
                    "SELECT id, project_id, portal_id, pattern_slug, component_name, source_path, \
                            ast_signature_sha256, dedup_marker_sha256, source_revision, last_sync_source, \
                            tokens_snapshot_json, doc_markdown, drift_status, quarantined_diff_json, \
                            revision, created_at, updated_at \
                     FROM component_patterns \
                     WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 \
                     ORDER BY pattern_slug ASC",
                )?;
                let rows = stmt.query_map(params![tenant_id, project_id, portal_id], |row| {
                    Ok(ComponentPatternDto {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        portal_id: row.get(2)?,
                        pattern_slug: row.get(3)?,
                        component_name: row.get(4)?,
                        source_path: row.get(5)?,
                        ast_signature_sha256: row.get(6)?,
                        dedup_marker_sha256: row.get(7)?,
                        source_revision: row.get(8)?,
                        last_sync_source: row.get(9)?,
                        tokens_snapshot_json: row.get(10)?,
                        doc_markdown: row.get(11)?,
                        drift_status: row.get(12)?,
                        quarantined_diff_json: row.get(13)?,
                        revision: row.get(14)?,
                        created_at: row.get(15)?,
                        updated_at: row.get(16)?,
                    })
                })?;
                for row in rows {
                    let rec = row?;
                    if include_quarantined || rec.drift_status != "quarantined_drift" {
                        patterns.push(rec);
                    }
                }
            }

            let mut locks = Vec::new();
            {
                let mut stmt = tx.prepare(
                    "SELECT id, project_id, portal_id, lock_owner_id, fencing_token, locked_slots_json, \
                            expires_at, revision, created_at, updated_at \
                     FROM app_shell_locks \
                     WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3",
                )?;
                let rows = stmt.query_map(params![tenant_id, project_id, portal_id], |row| {
                    let lock_owner_id: String = row.get(3)?;
                    let slots_raw: String = row.get(5)?;
                    let expires_at: i64 = row.get(6)?;
                    let is_expired = lock_owner_id.trim().is_empty() || expires_at <= now_ms;
                    Ok(AppShellLockDto {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        portal_id: row.get(2)?,
                        lock_owner_id,
                        fencing_token: row.get(4)?,
                        locked_slots: serde_json::from_str(&slots_raw).unwrap_or_default(),
                        expires_at,
                        is_expired,
                        revision: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                })?;
                for row in rows {
                    locks.push(row?);
                }
            }

            tx.commit()?;
            Ok(Outcome::Ok(ExportSnapshot {
                export_revision: next_rev,
                patterns,
                locks,
            }))
        },
    )
    .map_err(|e| match e {
        crate::locks::SsotLockError::StorageBusy { details, .. } => {
            SsotExportError::StorageBusy { details }
        }
        other => SsotExportError::Validation {
            code: "INVALID_REQUEST",
            message: other.to_string(),
        },
    })?;

    match res {
        Outcome::Ok(s) => Ok(s),
        Outcome::ProjectNotFound => Err(SsotExportError::NotFound {
            code: "PROJECT_NOT_FOUND",
            message: format!("Project `{project_id}` not found in tenant"),
        }),
    }
}

/// Acquires an export reader lease protecting `generation_id` from concurrent retention pruning.
#[derive(Debug)]
pub struct ExportReaderLease {
    lease_path: PathBuf,
}

impl Drop for ExportReaderLease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lease_path);
    }
}

pub fn acquire_export_reader_lease(
    export_root: &Path,
    generation_id: &str,
    ttl_ms: i64,
    now_ms: i64,
) -> Result<ExportReaderLease, SsotExportError> {
    verify_no_symlink_in_path(export_root, Path::new("leases"))?;
    let leases_dir = export_root.join("leases");
    fs::create_dir_all(&leases_dir).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to create leases dir: {e}"),
    })?;
    let lease_id = Uuid::new_v4().to_string();
    let lease_path = leases_dir.join(format!(".reader-lease-{lease_id}"));
    let payload = serde_json::json!({
        "leaseId": lease_id,
        "generationId": generation_id,
        "expiresAtMs": now_ms.saturating_add(ttl_ms),
    });
    write_atomic_file(&lease_path, payload.to_string().as_bytes())?;
    Ok(ExportReaderLease { lease_path })
}

fn active_leased_generations(export_root: &Path, now_ms: i64) -> std::collections::BTreeSet<String> {
    let mut set = std::collections::BTreeSet::new();
    let leases_dir = export_root.join("leases");
    let Ok(entries) = fs::read_dir(&leases_dir) else {
        return set;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with(".reader-lease-") {
            continue;
        }
        if let Ok(raw) = fs::read_to_string(entry.path()) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                let expires_at = val.get("expiresAtMs").and_then(|v| v.as_i64()).unwrap_or(0);
                if expires_at > now_ms {
                    if let Some(gen_id) = val.get("generationId").and_then(|v| v.as_str()) {
                        set.insert(gen_id.to_string());
                    }
                }
            }
        }
    }
    set
}

fn parse_revision_from_gen_id(gen_id: &str) -> i64 {
    gen_id
        .strip_prefix("rev-")
        .and_then(|rest| rest.split('-').next())
        .and_then(|num| num.parse::<i64>().ok())
        .unwrap_or(0)
}

/// Prunes old export generation folders under `<export_root>/generations` while strictly preserving:
/// 1) active `latest-manifest.json` target,
/// 2) newest 5 committed generations,
/// 3) any generation `< 60_000` ms old,
/// 4) any generation with an active `.reader-lease-*`.
pub fn prune_export_generations(
    export_root: &Path,
    now_ms: i64,
) -> Result<Vec<String>, SsotExportError> {
    let gens_dir = export_root.join("generations");
    if !gens_dir.exists() {
        return Ok(Vec::new());
    }

    let active_gen_id = read_manifest_raw_no_verify(export_root)
        .ok()
        .map(|m| m.generation_id);
    let leased = active_leased_generations(export_root, now_ms);

    let mut committed: Vec<(i64, i64, String, PathBuf)> = Vec::new();
    let entries = fs::read_dir(&gens_dir).map_err(|e| SsotExportError::Io {
        code: "INTERNAL_ERROR",
        message: format!("Failed to read generations dir: {e}"),
    })?;

    for entry in entries.flatten() {
        if !entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
            continue;
        }
        let gen_id = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let manifest_path = path.join("manifest.json");
        let (rev, created_at_ms) = if let Ok(raw) = fs::read_to_string(&manifest_path) {
            if let Ok(m) = serde_json::from_str::<ExportManifest>(&raw) {
                (m.export_revision, m.created_at_ms)
            } else {
                (parse_revision_from_gen_id(&gen_id), now_ms)
            }
        } else {
            (parse_revision_from_gen_id(&gen_id), now_ms)
        };
        committed.push((rev, created_at_ms, gen_id, path));
    }

    committed.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    let mut pruned = Vec::new();
    for (idx, (_rev, created_at_ms, gen_id, path)) in committed.into_iter().enumerate() {
        if idx < EXPORT_RETENTION_KEEP_COUNT {
            continue;
        }
        if active_gen_id.as_deref() == Some(gen_id.as_str()) {
            continue;
        }
        if leased.contains(&gen_id) {
            continue;
        }
        if now_ms.saturating_sub(created_at_ms) < EXPORT_RETENTION_GRACE_MS {
            continue;
        }
        if fs::remove_dir_all(&path).is_ok() {
            pruned.push(gen_id);
        }
    }

    Ok(pruned)
}

fn read_manifest_raw_no_verify(export_root: &Path) -> Result<ExportManifest, SsotExportError> {
    let manifest_path = export_root.join("latest-manifest.json");
    let mut attempt = 0;
    let file = loop {
        match open_file_nofollow_read(&manifest_path) {
            Ok(f) => break f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && attempt == 0 => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
                return Err(SsotExportError::PathTraversal {
                    path: manifest_path.display().to_string(),
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SsotExportError::NotFound {
                    code: "EXPORT_MANIFEST_INVALID",
                    message: "latest-manifest.json does not exist".to_string(),
                });
            }
            Err(e) => {
                return Err(SsotExportError::Io {
                    code: "INTERNAL_ERROR",
                    message: format!("Failed to read latest-manifest.json: {e}"),
                });
            }
        }
    };
    let manifest: ExportManifest =
        serde_json::from_reader(file).map_err(|e| SsotExportError::ManifestInvalid {
            reason: format!("Corrupted latest-manifest.json: {e}"),
        })?;
    Ok(manifest)
}

/// Reads and cryptographically verifies `latest-manifest.json` + all listed bundle files via `O_NOFOLLOW`.
pub fn read_and_verify_latest_export(
    workspace_root: &Path,
    relative_subdir: Option<&str>,
    now_ms: Option<i64>,
) -> Result<ExportManifest, SsotExportError> {
    let export_root = resolve_jailed_export_root(workspace_root, relative_subdir)?;
    let now = now_ms.unwrap_or_else(current_epoch_ms);
    let manifest = read_manifest_raw_no_verify(&export_root)?;

    let mut gen_comps = Path::new(&manifest.generation_id).components();
    match (gen_comps.next(), gen_comps.next()) {
        (Some(Component::Normal(_)), None) if !manifest.generation_id.contains('\\') => {}
        _ => {
            return Err(SsotExportError::PathTraversal {
                path: manifest.generation_id,
            });
        }
    }
    let gens_root = export_root.join("generations");
    verify_no_symlink_in_path(&export_root, Path::new("generations"))?;
    verify_no_symlink_in_path(&gens_root, Path::new(&manifest.generation_id))?;

    let _lease = acquire_export_reader_lease(&export_root, &manifest.generation_id, 10_000, now)?;
    let gen_dir = gens_root.join(&manifest.generation_id);
    if !gen_dir.exists() {
        return Err(SsotExportError::ManifestInvalid {
            reason: format!("Missing generation dir {}", manifest.generation_id),
        });
    }

    let mut check_entries = Vec::with_capacity(manifest.files.len());
    for entry in &manifest.files {
        let rel = Path::new(&entry.relative_path);
        verify_no_symlink_in_path(&gen_dir, rel)?;
        let full = gen_dir.join(rel);
        let mut f = open_file_nofollow_read(&full).map_err(|e| {
            if e.raw_os_error() == Some(libc::ELOOP) {
                return SsotExportError::PathTraversal {
                    path: full.display().to_string(),
                };
            }
            SsotExportError::ManifestInvalid {
                reason: format!(
                    "Missing or unreadable export artifact {}: {e}",
                    entry.relative_path
                ),
            }
        })?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buf).map_err(|e| {
            SsotExportError::ManifestInvalid {
                reason: format!("Failed reading {}: {e}", entry.relative_path),
            }
        })?;
        let actual_sha = sha256_hex(&buf);
        if actual_sha != entry.sha256 {
            return Err(SsotExportError::ManifestInvalid {
                reason: format!(
                    "SHA-256 mismatch for {}: expected {}, got {}",
                    entry.relative_path, entry.sha256, actual_sha
                ),
            });
        }
        check_entries.push(format!("{}:{}", entry.relative_path, actual_sha));
    }

    let aggregate_sha = sha256_hex(check_entries.join("\n").as_bytes());
    if aggregate_sha != manifest.checksum_sha256 {
        return Err(SsotExportError::ManifestInvalid {
            reason: format!(
                "Manifest checksum mismatch: expected {}, got {}",
                manifest.checksum_sha256, aggregate_sha
            ),
        });
    }

    Ok(manifest)
}

fn render_human_markdown_export(
    req: &SsotExportRequest,
    snapshot: &ExportSnapshot,
) -> String {
    let mut md = format!(
        "# SSOT Registry Snapshot — Portal `{}` (Revision {})\n\n- Project ID: `{}`\n- Pattern Count: `{}`\n\n## Component Patterns\n\n",
        req.portal_id,
        snapshot.export_revision,
        req.project_id,
        snapshot.patterns.len()
    );
    for pat in &snapshot.patterns {
        md.push_str(&format!(
            "### {} (`{}`)\n- Drift: `{}`\n- AST SHA-256: `{}`\n- Doc:\n{}\n",
            pat.component_name,
            pat.pattern_slug,
            pat.drift_status,
            pat.ast_signature_sha256,
            pat.doc_markdown
        ));
    }
    md.push_str("\n## AppShell Locks\n\n");
    for lk in &snapshot.locks {
        md.push_str(&format!(
            "- Owner: `{}` | Fencing Token: `{}` | Revision: `{}` | Expired: `{}`\n",
            if lk.lock_owner_id.is_empty() {
                "(unlocked)"
            } else {
                lk.lock_owner_id.as_str()
            },
            lk.fencing_token,
            lk.revision,
            lk.is_expired
        ));
    }
    md
}

/// Commits an already-computed `ExportSnapshot` onto disk under advisory `flock` + in-process mutex.
/// Rejects stale revisions with `EXPORT_SUPERSEDED` (`409`).
pub fn commit_export_snapshot(
    workspace_root: &Path,
    req: &SsotExportRequest,
    snapshot: ExportSnapshot,
) -> Result<SsotExportResult, SsotExportError> {
    let export_root =
        resolve_jailed_export_root(workspace_root, req.relative_export_subdir.as_deref())?;
    let now_ms = req.now_ms.unwrap_or_else(current_epoch_ms);

    with_export_lock(
        workspace_root,
        &req.tenant_id,
        &req.project_id,
        Duration::from_secs(5),
        || {
            if let Ok(current) = read_manifest_raw_no_verify(&export_root) {
                if current.tenant_id == req.tenant_id
                    && current.project_id == req.project_id
                    && snapshot.export_revision <= current.export_revision
                {
                    return Err(SsotExportError::ExportSuperseded {
                        attempted_revision: snapshot.export_revision,
                        latest_revision: current.export_revision,
                    });
                }
            }

            let gens_dir = export_root.join("generations");
            let staging_root = export_root.join(".staging");
            fs::create_dir_all(&gens_dir).map_err(|e| SsotExportError::Io {
                code: "INTERNAL_ERROR",
                message: format!("Failed to create generations dir: {e}"),
            })?;
            fs::create_dir_all(&staging_root).map_err(|e| SsotExportError::Io {
                code: "INTERNAL_ERROR",
                message: format!("Failed to create staging dir: {e}"),
            })?;

            let export_id = Uuid::new_v4().to_string();
            let gen_id = format!(
                "rev-{:06}-{}",
                snapshot.export_revision,
                Uuid::new_v4().simple()
            );
            let stage_dir = staging_root.join(&gen_id);
            fs::create_dir_all(&stage_dir).map_err(|e| SsotExportError::Io {
                code: "INTERNAL_ERROR",
                message: format!("Failed to create stage dir: {e}"),
            })?;

            let mut file_map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
            let bundle_payload = serde_json::json!({
                "exportId": export_id,
                "exportRevision": snapshot.export_revision,
                "tenantId": req.tenant_id,
                "projectId": req.project_id,
                "portalId": req.portal_id,
                "formats": req.formats,
                "patterns": snapshot.patterns,
                "appShellLocks": snapshot.locks,
            });
            let json_bytes = serde_json::to_vec_pretty(&bundle_payload).map_err(|e| {
                SsotExportError::Io {
                    code: "INTERNAL_ERROR",
                    message: format!("Failed to serialize ssot bundle: {e}"),
                }
            })?;
            let json_sha256 = sha256_hex(&json_bytes);
            file_map.insert("ssot-bundle.json".to_string(), json_bytes);

            let markdown_bytes = render_human_markdown_export(req, &snapshot).into_bytes();
            let markdown_sha256 = sha256_hex(&markdown_bytes);
            file_map.insert("ssot-report.md".to_string(), markdown_bytes);

            let mut file_entries = Vec::with_capacity(file_map.len());
            let mut digest_lines = Vec::with_capacity(file_map.len());
            for (rel_path, bytes) in &file_map {
                let sha = sha256_hex(bytes);
                write_atomic_file(&stage_dir.join(rel_path), bytes)?;
                digest_lines.push(format!("{rel_path}:{sha}"));
                file_entries.push(ExportManifestFileEntry {
                    relative_path: rel_path.clone(),
                    sha256: sha,
                    byte_size: bytes.len() as u64,
                });
            }

            let checksum_sha256 = sha256_hex(digest_lines.join("\n").as_bytes());
            let generation_rel_dir = format!("generations/{gen_id}");
            let markdown_rel_path = format!(".cowok-design/exports/{generation_rel_dir}/ssot-report.md");
            let json_rel_path = format!(".cowok-design/exports/{generation_rel_dir}/ssot-bundle.json");

            let manifest = ExportManifest {
                export_id: export_id.clone(),
                export_revision: snapshot.export_revision,
                tenant_id: req.tenant_id.clone(),
                project_id: req.project_id.clone(),
                portal_id: req.portal_id.clone(),
                generation_id: gen_id.clone(),
                generation_rel_dir: generation_rel_dir.clone(),
                markdown_rel_path: markdown_rel_path.clone(),
                markdown_sha256: markdown_sha256.clone(),
                json_rel_path: json_rel_path.clone(),
                json_sha256: json_sha256.clone(),
                checksum_sha256,
                component_count: snapshot.patterns.len(),
                created_at_ms: now_ms,
                files: file_entries,
            };

            let manifest_bytes =
                serde_json::to_vec_pretty(&manifest).map_err(|e| SsotExportError::Io {
                    code: "INTERNAL_ERROR",
                    message: format!("Failed to serialize manifest: {e}"),
                })?;
            write_atomic_file(&stage_dir.join("manifest.json"), &manifest_bytes)?;

            let committed_gen_dir = gens_dir.join(&gen_id);
            fs::rename(&stage_dir, &committed_gen_dir).map_err(|e| SsotExportError::Io {
                code: "INTERNAL_ERROR",
                message: format!("Failed to promote staged generation: {e}"),
            })?;

            write_atomic_file(&export_root.join("latest-manifest.json"), &manifest_bytes)?;
            let _ = prune_export_generations(&export_root, now_ms)?;

            Ok(SsotExportResult {
                export_id,
                revision: snapshot.export_revision,
                portal_id: req.portal_id.clone(),
                generated_at: now_ms,
                markdown_path: markdown_rel_path,
                markdown_sha256,
                json_path: json_rel_path,
                json_sha256,
            })
        },
    )
}

/// End-to-end deterministic SSOT export pipeline:
/// DB `BEGIN IMMEDIATE` snapshot -> monotonic `revision` -> jailed `.cowok-design/exports/`
/// atomic staging + advisory `flock` + `latest-manifest.json` swap + reader-safe retention prune.
pub fn export_hidden_ssot_bundle(
    conn: &mut Connection,
    workspace_root: &Path,
    req: &SsotExportRequest,
) -> Result<SsotExportResult, SsotExportError> {
    if req.formats.is_empty()
        || !req
            .formats
            .iter()
            .all(|f| matches!(f.as_str(), "markdown" | "json"))
    {
        return Err(SsotExportError::Validation {
            code: "INVALID_REQUEST",
            message: "formats must contain 1..=2 items from ['markdown', 'json']".to_string(),
        });
    }
    let now_ms = req.now_ms.unwrap_or_else(current_epoch_ms);
    let snapshot = allocate_export_snapshot(
        conn,
        &req.tenant_id,
        &req.project_id,
        &req.portal_id,
        req.include_quarantined,
        now_ms,
    )?;
    commit_export_snapshot(workspace_root, req, snapshot)
}
