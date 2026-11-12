// @story 01.2
// @implements AC-07
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use domain::{CanonicalSpecKey, HarnessError, Project, ProvenanceStatus, SafeJsInteger};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{map_sqlite_error, retry_on_busy};
use crate::lock::SpecLockGuard;
use crate::pool::SqlitePool;
use crate::recovery::recover_pending_spec;
use crate::sync::{
    atomic_write_spec_file, compute_sha256_hex, compute_summary_digest, read_jailed_spec_bytes,
    resolve_jailed_spec_path, validate_candidate_spec_path,
};

const MAX_PROVENANCE_JSON_BYTES: usize = 16 * 1024;
const LOCK_TIMEOUT: Duration = Duration::from_millis(5_000);

/// Input payload for upserting one of the 9 Foundational Specs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertSpecInput {
    pub project_id: Uuid,
    pub spec_key: CanonicalSpecKey,
    pub markdown_content: String,
    pub expected_sha256: Option<String>,
    pub status: ProvenanceStatus,
    pub provenance_summary_json: String,
    pub source_project_id: Option<Uuid>,
    pub source_revision_sha256: Option<String>,
    pub actor_kind: String,
    pub actor_id: String,
    pub change_summary: String,
}

/// Persisted spec record returned by `SpecRegistry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub spec_key: CanonicalSpecKey,
    pub file_path: String,
    pub status: ProvenanceStatus,
    pub version: u32,
    pub content_sha256: String,
    pub summary_digest: String,
    pub provenance_summary_json: String,
    pub source_project_id: Option<Uuid>,
    pub source_revision_sha256: Option<String>,
    pub updated_at: SafeJsInteger,
}

/// Persisted immutable revision record returned by `SpecRegistry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecRevisionRecord {
    pub id: Uuid,
    pub spec_id: Uuid,
    pub project_id: Uuid,
    pub spec_key: CanonicalSpecKey,
    pub revision_number: u32,
    pub operation_id: Uuid,
    pub previous_sha256: Option<String>,
    pub content_hash: String,
    pub actor_kind: String,
    pub actor_id: String,
    pub change_summary: String,
    pub source_project_id: Option<Uuid>,
    pub source_revision_sha256: Option<String>,
    pub created_at: SafeJsInteger,
}

/// Outcome of a synchronization or filesystem watcher event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum SyncOutcome {
    #[serde(rename = "SYNCED_TO_DISK_AND_DB")]
    SyncedToDiskAndDb {
        spec_key: CanonicalSpecKey,
        version: u32,
        content_sha256: String,
    },
    #[serde(rename = "SUPPRESSED_NO_OP")]
    SuppressedNoOp {
        spec_key: CanonicalSpecKey,
        content_sha256: String,
    },
    #[serde(rename = "IGNORED_ZERO_BYTE_TRANSIENT")]
    IgnoredZeroByteTransient { path: String },
    #[serde(rename = "MISSING_TRACKED_FILE")]
    MissingTrackedFile {
        spec_key: CanonicalSpecKey,
        path: String,
    },
    #[serde(rename = "TOMBSTONE_RECONCILED")]
    TombstoneReconciled {
        spec_key: CanonicalSpecKey,
        previous_sha256: String,
    },
}

/// High-assurance 9-Spec Registry managing SQLite WAL persistence, atomic disk sync, and SHA-256 OCC.
#[derive(Debug, Clone)]
pub struct SpecRegistry {
    pool: SqlitePool,
}

impl SpecRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Registers a new project aggregate in the `projects` table.
    pub fn create_project(
        &self,
        id: Uuid,
        slug: &str,
        name: &str,
        product_brief: &str,
        inherit_brand_from_project_id: Option<Uuid>,
    ) -> Result<Project, HarnessError> {
        if slug.trim().is_empty() || name.trim().is_empty() {
            return Err(HarnessError::ValidationFailed {
                reason: "Project slug and name must not be empty".to_string(),
            });
        }

        let canonical_root = self
            .pool
            .workspace_root()
            .canonicalize()
            .unwrap_or_else(|_| self.pool.workspace_root().to_path_buf());
        let workspace_root_str = canonical_root.display().to_string();

        let conn = self.pool.connection()?;
        let existing_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE workspace_root = ?1",
                params![workspace_root_str],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let final_ws_str = if existing_count > 0 {
            let isolated = canonical_root
                .join(".cowok-design")
                .join("projects")
                .join(id.to_string());
            let _ = std::fs::create_dir_all(&isolated);
            isolated.display().to_string()
        } else {
            workspace_root_str
        };

        let now_ms = Utc::now().timestamp_millis();
        let safe_now = SafeJsInteger::new(now_ms)?;

        retry_on_busy("create_project", || {
            conn.execute(
                "INSERT INTO projects (
                    id, slug, name, workspace_root, product_brief,
                    inherit_brand_from_project_id, status, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ACTIVE', ?7, ?8)",
                params![
                    id.to_string(),
                    slug,
                    name,
                    final_ws_str,
                    product_brief,
                    inherit_brand_from_project_id.map(|u| u.to_string()),
                    now_ms,
                    now_ms,
                ],
            )
        })?;

        Ok(Project {
            id,
            name: name.to_string(),
            slug: slug.to_string(),
            created_at: safe_now,
            updated_at: safe_now,
        })
    }

    /// Verifies that `project_id` exists and is bound to this registry's `workspace_root` (Anti-IDOR local scope check).
    pub fn verify_project_scope(&self, project_id: Uuid) -> Result<PathBuf, HarnessError> {
        let conn = self.pool.connection()?;
        let row: Option<String> = conn
            .query_row(
                "SELECT workspace_root FROM projects WHERE id = ?1",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "verify_project_scope"))?;

        let db_root_str = row.ok_or_else(|| HarnessError::ProjectNotFound {
            project_id: project_id.to_string(),
        })?;

        let canonical_pool_root = self.pool.workspace_root().canonicalize().map_err(|_| {
            HarnessError::PathTraversalDenied {
                path: self.pool.workspace_root().display().to_string(),
            }
        })?;

        let db_root_path = PathBuf::from(&db_root_str);
        if db_root_str != "." {
            if let Ok(canon_db_root) = db_root_path.canonicalize() {
                if canon_db_root != canonical_pool_root {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "PROJECT_SCOPE_VIOLATION: project {project_id} belongs to '{}', not '{}'",
                            canon_db_root.display(),
                            canonical_pool_root.display()
                        ),
                    });
                }
            }
        }

        Ok(canonical_pool_root)
    }

    /// Linearized 10-step atomic disk + SQLite WAL CAS upsert for one of the 9 Foundational Specs.
    pub fn upsert_spec(
        &self,
        input: UpsertSpecInput,
    ) -> Result<(SpecRecord, SyncOutcome), HarnessError> {
        let workspace_root = self.verify_project_scope(input.project_id)?;

        // 1. Acquire per-spec exclusive advisory lock
        let _lock = SpecLockGuard::acquire(
            &workspace_root,
            input.project_id,
            input.spec_key,
            LOCK_TIMEOUT,
        )?;

        let mut conn = self.pool.connection()?;

        // 2. Recover any pending intents for this spec before serving read/write
        recover_pending_spec(&mut conn, &workspace_root, input.project_id, input.spec_key)?;

        // Validate target path inside workspace jail
        let target_path = resolve_jailed_spec_path(&workspace_root, input.spec_key)?;
        let rel_file_path = format!("product/specs/{}", input.spec_key.as_str());

        let operation_id = Uuid::new_v4();
        let canonical_prov_json = normalize_and_validate_provenance_json(
            &input.provenance_summary_json,
            &input.actor_id,
            operation_id,
            input.source_project_id,
            input.source_revision_sha256.as_deref(),
        )?;

        // 3. Read current DB state and disk bytes
        let existing_db: Option<(String, i64, String)> = conn
            .query_row(
                "SELECT id, version, content_sha256 FROM project_specs WHERE project_id = ?1 AND spec_key = ?2",
                params![input.project_id.to_string(), input.spec_key.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_existing_spec"))?;

        let new_bytes = input.markdown_content.as_bytes();
        let new_sha256 = compute_sha256_hex(new_bytes);
        let now_ms = Utc::now().timestamp_millis();

        let (spec_id_str, expected_ver, old_sha_opt, is_new_row) = match existing_db {
            Some((id_str, db_ver, db_sha)) if !db_sha.is_empty() => {
                // Existing tracked spec: disk file must exist and match DB hash (read via O_NOFOLLOW)
                let Some(disk_bytes) = read_jailed_spec_bytes(&workspace_root, input.spec_key)?
                else {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "MISSING_TRACKED_FILE: {} is tracked in SQLite but missing on disk",
                            target_path.display()
                        ),
                    });
                };
                let disk_sha = compute_sha256_hex(&disk_bytes);

                // 4. Mandatory OCC check: expected_sha256 == db.content_sha256 == disk_sha256
                let expected = input.expected_sha256.as_deref().unwrap_or("");
                if expected.is_empty() || expected != db_sha || db_sha != disk_sha {
                    let actual = if db_sha != disk_sha { disk_sha } else { db_sha };
                    return Err(HarnessError::StaleFile {
                        path: rel_file_path,
                        expected_hash: expected.to_string(),
                        actual_hash: actual,
                    });
                }

                (id_str, db_ver + 1, Some(db_sha), false)
            }
            Some((id_str, db_ver, _empty_sha)) => (id_str, db_ver, None, false),
            None => {
                // Initial creation: if caller supplied a non-matching expected_sha256 while file already exists, check it via O_NOFOLLOW
                if let Some(disk_bytes) = read_jailed_spec_bytes(&workspace_root, input.spec_key)? {
                    let disk_sha = compute_sha256_hex(&disk_bytes);
                    if let Some(exp) = input.expected_sha256.as_deref() {
                        if exp != disk_sha {
                            return Err(HarnessError::StaleFile {
                                path: rel_file_path,
                                expected_hash: exp.to_string(),
                                actual_hash: disk_sha,
                            });
                        }
                    }
                }
                (Uuid::new_v4().to_string(), 1, None, true)
            }
        };

        let status_str = provenance_status_to_str(input.status);
        let summary_digest = compute_summary_digest(
            input.spec_key,
            &new_sha256,
            &input.actor_id,
            &input.change_summary,
            input.source_project_id,
            input.source_revision_sha256.as_deref(),
        );

        // 5. In BEGIN IMMEDIATE, insert placeholder row (if brand new) and record PENDING write intent
        retry_on_busy("record_write_intent", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if is_new_row {
                tx.execute(
                    "INSERT INTO project_specs (
                        id, project_id, spec_key, file_path, status, version,
                        content_sha256, summary_digest, provenance_summary_json,
                        source_project_id, source_revision_sha256, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, 1, '', '', '{}', ?6, ?7, ?8, ?8)",
                    params![
                        spec_id_str,
                        input.project_id.to_string(),
                        input.spec_key.as_str(),
                        rel_file_path,
                        status_str,
                        input.source_project_id.map(|u| u.to_string()),
                        input.source_revision_sha256,
                        now_ms,
                    ],
                )?;
            }

            tx.execute(
                "INSERT INTO spec_write_intents (
                    operation_id, spec_id, project_id, spec_key, expected_version,
                    old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                    provenance_summary_json, source_project_id, source_revision_sha256,
                    state, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'PENDING', ?14, ?14)",
                params![
                    operation_id.to_string(),
                    spec_id_str,
                    input.project_id.to_string(),
                    input.spec_key.as_str(),
                    expected_ver,
                    old_sha_opt,
                    new_sha256,
                    input.actor_kind,
                    input.actor_id,
                    input.change_summary,
                    canonical_prov_json,
                    input.source_project_id.map(|u| u.to_string()),
                    input.source_revision_sha256,
                    now_ms,
                ],
            )?;
            tx.commit()
        })?;

        // 6. Write sibling temp, flush, sync_all, atomic rename, and fsync directory
        if let Err(write_err) =
            atomic_write_spec_file(&workspace_root, input.spec_key, operation_id, new_bytes)
        {
            let _ = retry_on_busy("abort_intent_on_write_err", || {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "UPDATE spec_write_intents SET state = 'ABORTED', updated_at = ?1 WHERE operation_id = ?2",
                    params![now_ms, operation_id.to_string()],
                )?;
                if is_new_row {
                    tx.execute(
                        "DELETE FROM project_specs WHERE id = ?1 AND content_sha256 = ''",
                        params![spec_id_str],
                    )?;
                }
                tx.commit()
            });
            return Err(write_err);
        }

        // 7. Re-hash target file after rename via O_NOFOLLOW to detect mid-flight interference
        let post_rename_bytes = read_jailed_spec_bytes(&workspace_root, input.spec_key)?
            .ok_or_else(|| HarnessError::ValidationFailed {
                reason: format!("POST_RENAME_MISSING [{}]", target_path.display()),
            })?;
        let post_rename_sha = compute_sha256_hex(&post_rename_bytes);
        if post_rename_sha != new_sha256 {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "RECOVERY_REQUIRED [{}]: post-rename hash ({post_rename_sha}) != expected new_sha256 ({new_sha256})",
                    input.spec_key.as_str()
                ),
            });
        }

        // 8 & 9. In BEGIN IMMEDIATE, execute SQL CAS + append immutable spec_revision + mark intent COMMITTED
        let expected_db_ver = if old_sha_opt.is_none() {
            1
        } else {
            expected_ver - 1
        };
        let expected_db_sha = old_sha_opt.clone().unwrap_or_default();
        let rev_id = Uuid::new_v4().to_string();

        let cas_succeeded = retry_on_busy("cas_finalize_spec", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let updated = tx.execute(
                "UPDATE project_specs
                 SET file_path = ?1,
                     status = ?2,
                     version = ?3,
                     content_sha256 = ?4,
                     summary_digest = ?5,
                     provenance_summary_json = ?6,
                     source_project_id = ?7,
                     source_revision_sha256 = ?8,
                     updated_at = ?9
                 WHERE id = ?10 AND version = ?11 AND content_sha256 = ?12",
                params![
                    rel_file_path,
                    status_str,
                    expected_ver,
                    new_sha256,
                    summary_digest,
                    canonical_prov_json,
                    input.source_project_id.map(|u| u.to_string()),
                    input.source_revision_sha256,
                    now_ms,
                    spec_id_str,
                    expected_db_ver,
                    expected_db_sha,
                ],
            )?;

            if updated != 1 {
                tx.execute(
                    "UPDATE spec_write_intents SET state = 'ABORTED', updated_at = ?1 WHERE operation_id = ?2",
                    params![now_ms, operation_id.to_string()],
                )?;
                tx.commit()?;
                return Ok(false);
            }

            tx.execute(
                "INSERT OR IGNORE INTO spec_revisions (
                    id, spec_id, project_id, spec_key, revision_number,
                    operation_id, previous_sha256, content_hash,
                    actor_kind, actor_id, change_summary,
                    source_project_id, source_revision_sha256, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    rev_id,
                    spec_id_str,
                    input.project_id.to_string(),
                    input.spec_key.as_str(),
                    expected_ver,
                    operation_id.to_string(),
                    old_sha_opt,
                    new_sha256,
                    input.actor_kind,
                    input.actor_id,
                    input.change_summary,
                    input.source_project_id.map(|u| u.to_string()),
                    input.source_revision_sha256,
                    now_ms,
                ],
            )?;

            tx.execute(
                "UPDATE spec_write_intents SET state = 'COMMITTED', updated_at = ?1 WHERE operation_id = ?2",
                params![now_ms, operation_id.to_string()],
            )?;
            tx.commit()?;
            Ok(true)
        })?;

        if !cas_succeeded {
            return Err(HarnessError::StaleFile {
                path: rel_file_path,
                expected_hash: expected_db_sha,
                actual_hash: "CONCURRENT_CAS_MODIFICATION".to_string(),
            });
        }

        let record = SpecRecord {
            id: Uuid::parse_str(&spec_id_str).unwrap_or_else(|_| Uuid::new_v4()),
            project_id: input.project_id,
            spec_key: input.spec_key,
            file_path: rel_file_path,
            status: input.status,
            version: expected_ver as u32,
            content_sha256: new_sha256.clone(),
            summary_digest,
            provenance_summary_json: canonical_prov_json,
            source_project_id: input.source_project_id,
            source_revision_sha256: input.source_revision_sha256,
            updated_at: SafeJsInteger::new(now_ms)?,
        };

        let outcome = SyncOutcome::SyncedToDiskAndDb {
            spec_key: input.spec_key,
            version: expected_ver as u32,
            content_sha256: new_sha256,
        };

        Ok((record, outcome))
    }

    /// Reads the latest committed `SpecRecord` for `(project_id, spec_key)` after running pending recovery.
    pub fn get_spec(
        &self,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
    ) -> Result<Option<SpecRecord>, HarnessError> {
        let workspace_root = self.verify_project_scope(project_id)?;
        let _lock = SpecLockGuard::acquire(&workspace_root, project_id, spec_key, LOCK_TIMEOUT)?;
        let mut conn = self.pool.connection()?;
        recover_pending_spec(&mut conn, &workspace_root, project_id, spec_key)?;

        let row = conn
            .query_row(
                "SELECT id, file_path, status, version, content_sha256, summary_digest,
                        provenance_summary_json, source_project_id, source_revision_sha256, updated_at
                 FROM project_specs
                 WHERE project_id = ?1 AND spec_key = ?2 AND content_sha256 != ''",
                params![project_id.to_string(), spec_key.as_str()],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, i64>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "get_spec"))?;

        match row {
            Some((
                id_s,
                file_path,
                status_s,
                ver,
                sha,
                digest,
                prov_json,
                src_proj_opt,
                src_rev_opt,
                updated_at,
            )) => Ok(Some(SpecRecord {
                id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
                project_id,
                spec_key,
                file_path,
                status: parse_provenance_status(&status_s),
                version: ver as u32,
                content_sha256: sha,
                summary_digest: digest,
                provenance_summary_json: prov_json,
                source_project_id: src_proj_opt.and_then(|s| Uuid::parse_str(&s).ok()),
                source_revision_sha256: src_rev_opt,
                updated_at: SafeJsInteger::new(updated_at)?,
            })),
            None => Ok(None),
        }
    }

    /// Lists all immutable revisions for `(project_id, spec_key)` ordered by `revision_number ASC`.
    pub fn list_revisions(
        &self,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
    ) -> Result<Vec<SpecRevisionRecord>, HarnessError> {
        let workspace_root = self.verify_project_scope(project_id)?;
        let _lock = SpecLockGuard::acquire(&workspace_root, project_id, spec_key, LOCK_TIMEOUT)?;
        let mut conn = self.pool.connection()?;
        recover_pending_spec(&mut conn, &workspace_root, project_id, spec_key)?;

        let mut stmt = conn
            .prepare(
                "SELECT id, spec_id, revision_number, operation_id, previous_sha256,
                        content_hash, actor_kind, actor_id, change_summary,
                        source_project_id, source_revision_sha256, created_at
                 FROM spec_revisions
                 WHERE project_id = ?1 AND spec_key = ?2
                 ORDER BY revision_number ASC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_list_revisions"))?;

        let rows = stmt
            .query_map(params![project_id.to_string(), spec_key.as_str()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, i64>(11)?,
                ))
            })
            .map_err(|e| map_sqlite_error(e, "query_list_revisions"))?;

        let mut out = Vec::new();
        for row in rows {
            let (
                id_s,
                spec_id_s,
                rev_num,
                op_id_s,
                prev_sha,
                content_hash,
                actor_kind,
                actor_id,
                change_summary,
                src_proj_opt,
                src_rev_opt,
                created_at,
            ) = row.map_err(|e| map_sqlite_error(e, "collect_revision_row"))?;

            out.push(SpecRevisionRecord {
                id: Uuid::parse_str(&id_s).unwrap_or_else(|_| Uuid::nil()),
                spec_id: Uuid::parse_str(&spec_id_s).unwrap_or_else(|_| Uuid::nil()),
                project_id,
                spec_key,
                revision_number: rev_num as u32,
                operation_id: Uuid::parse_str(&op_id_s).unwrap_or_else(|_| Uuid::nil()),
                previous_sha256: prev_sha,
                content_hash,
                actor_kind,
                actor_id,
                change_summary,
                source_project_id: src_proj_opt.and_then(|s| Uuid::parse_str(&s).ok()),
                source_revision_sha256: src_rev_opt,
                created_at: SafeJsInteger::new(created_at)?,
            });
        }
        Ok(out)
    }

    /// Reconciles an external filesystem event for `candidate_path` with Hash-Gated Echo Suppression
    /// and a 50ms debounce window for transient 0-byte IDE save truncations.
    pub fn handle_watcher_path_event(
        &self,
        project_id: Uuid,
        candidate_path: &std::path::Path,
    ) -> Result<SyncOutcome, HarnessError> {
        let workspace_root = self.verify_project_scope(project_id)?;
        let (spec_key, target_path) =
            validate_candidate_spec_path(&workspace_root, candidate_path)?;

        let _lock = SpecLockGuard::acquire(&workspace_root, project_id, spec_key, LOCK_TIMEOUT)?;
        let mut conn = self.pool.connection()?;
        recover_pending_spec(&mut conn, &workspace_root, project_id, spec_key)?;

        let Some(mut bytes1) = read_jailed_spec_bytes(&workspace_root, spec_key)? else {
            return Self::reconcile_missing_file_as_tombstone(
                &mut conn,
                project_id,
                spec_key,
                &target_path,
            );
        };

        // 50ms debounce for transient 0-byte IDE save truncations (EC-01.2-02)
        if bytes1.is_empty() {
            std::thread::sleep(Duration::from_millis(50));
            let Some(retry_bytes) = read_jailed_spec_bytes(&workspace_root, spec_key)? else {
                return Self::reconcile_missing_file_as_tombstone(
                    &mut conn,
                    project_id,
                    spec_key,
                    &target_path,
                );
            };
            bytes1 = retry_bytes;
            if bytes1.is_empty() {
                return Ok(SyncOutcome::IgnoredZeroByteTransient {
                    path: target_path.display().to_string(),
                });
            }
        }

        let disk_sha1 = compute_sha256_hex(&bytes1);

        // Read current DB record
        let db_row: Option<(String, i64, String, Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT id, version, content_sha256, source_project_id, source_revision_sha256
                 FROM project_specs
                 WHERE project_id = ?1 AND spec_key = ?2 AND content_sha256 != ''",
                params![project_id.to_string(), spec_key.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "watcher_read_db_spec"))?;

        let Some((spec_id_str, db_ver, db_sha, src_proj_opt, src_rev_opt)) = db_row else {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "UNTRACKED_SPEC_FILE: {} is not yet registered in project_specs",
                    spec_key.as_str()
                ),
            });
        };

        // Hash-Gated Echo Suppression: if disk_sha256 == db_sha256 -> SuppressedNoOp (0 writes!)
        if disk_sha1 == db_sha {
            return Ok(SyncOutcome::SuppressedNoOp {
                spec_key,
                content_sha256: disk_sha1,
            });
        }

        // Verify UTF-8 validity and stability (hash twice around read via O_NOFOLLOW)
        let _utf8_str =
            std::str::from_utf8(&bytes1).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("INVALID_UTF8_SPEC [{}]: {e}", spec_key.as_str()),
            })?;
        let bytes2 = read_jailed_spec_bytes(&workspace_root, spec_key)?.ok_or_else(|| {
            HarnessError::ValidationFailed {
                reason: format!("WATCHER_RECHECK_MISSING [{}]", target_path.display()),
            }
        })?;
        let disk_sha2 = compute_sha256_hex(&bytes2);
        if disk_sha1 != disk_sha2 {
            return Err(HarnessError::StaleFile {
                path: format!("product/specs/{}", spec_key.as_str()),
                expected_hash: disk_sha1,
                actual_hash: disk_sha2,
            });
        }

        let operation_id = Uuid::new_v4();
        let src_proj_uuid = src_proj_opt
            .as_deref()
            .and_then(|s| Uuid::parse_str(s).ok());
        let prov_json = normalize_and_validate_provenance_json(
            r#"{"origin":"filesystem"}"#,
            "external_watcher",
            operation_id,
            src_proj_uuid,
            src_rev_opt.as_deref(),
        )?;
        let change_summary = "External filesystem edit reconciled by notify watcher";
        let summary_digest = compute_summary_digest(
            spec_key,
            &disk_sha1,
            "external_watcher",
            change_summary,
            src_proj_uuid,
            src_rev_opt.as_deref(),
        );
        let new_ver = db_ver + 1;
        let now_ms = Utc::now().timestamp_millis();
        let rev_id = Uuid::new_v4().to_string();

        retry_on_busy("watcher_reconcile_cas", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO spec_write_intents (
                    operation_id, spec_id, project_id, spec_key, expected_version,
                    old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                    provenance_summary_json, source_project_id, source_revision_sha256,
                    state, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'EXTERNAL_WATCHER', 'external_watcher', ?8, ?9, ?10, ?11, 'COMMITTED', ?12, ?12)",
                params![
                    operation_id.to_string(),
                    spec_id_str,
                    project_id.to_string(),
                    spec_key.as_str(),
                    new_ver,
                    db_sha,
                    disk_sha1,
                    change_summary,
                    prov_json,
                    src_proj_opt,
                    src_rev_opt,
                    now_ms,
                ],
            )?;

            let updated = tx.execute(
                "UPDATE project_specs
                 SET version = ?1,
                     content_sha256 = ?2,
                     summary_digest = ?3,
                     provenance_summary_json = ?4,
                     updated_at = ?5
                 WHERE id = ?6 AND version = ?7 AND content_sha256 = ?8",
                params![
                    new_ver,
                    disk_sha1,
                    summary_digest,
                    prov_json,
                    now_ms,
                    spec_id_str,
                    db_ver,
                    db_sha,
                ],
            )?;

            if updated == 1 {
                tx.execute(
                    "INSERT OR IGNORE INTO spec_revisions (
                        id, spec_id, project_id, spec_key, revision_number,
                        operation_id, previous_sha256, content_hash,
                        actor_kind, actor_id, change_summary,
                        source_project_id, source_revision_sha256, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'EXTERNAL_WATCHER', 'external_watcher', ?9, ?10, ?11, ?12)",
                    params![
                        rev_id,
                        spec_id_str,
                        project_id.to_string(),
                        spec_key.as_str(),
                        new_ver,
                        operation_id.to_string(),
                        db_sha,
                        disk_sha1,
                        change_summary,
                        src_proj_opt,
                        src_rev_opt,
                        now_ms,
                    ],
                )?;
            }
            tx.commit()
        })?;

        Ok(SyncOutcome::SyncedToDiskAndDb {
            spec_key,
            version: new_ver as u32,
            content_sha256: disk_sha1,
        })
    }

    fn reconcile_missing_file_as_tombstone(
        conn: &mut rusqlite::Connection,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
        target_path: &std::path::Path,
    ) -> Result<SyncOutcome, HarnessError> {
        let db_row: Option<(String, i64, String)> = conn
            .query_row(
                "SELECT id, version, content_sha256 FROM project_specs WHERE project_id = ?1 AND spec_key = ?2",
                params![project_id.to_string(), spec_key.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "check_spec_for_deletion"))?;

        if let Some((spec_id_str, db_ver, db_sha)) = db_row {
            let tombstone_id = Uuid::new_v4().to_string();
            let rev_id = Uuid::new_v4().to_string();
            let now_ms = Utc::now().timestamp_millis();
            let next_rev = db_ver + 1;

            retry_on_busy("reconcile_deletion_tombstone", || {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "INSERT OR REPLACE INTO spec_tombstones (
                        id, project_id, spec_key, deleted_at, deleted_by, content_sha256_at_deletion
                    ) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, ?4, ?5)",
                    params![
                        tombstone_id,
                        project_id.to_string(),
                        spec_key.as_str(),
                        "watcher_reconciliation",
                        db_sha,
                    ],
                )?;
                tx.execute(
                    "UPDATE project_specs 
                     SET status = 'FILE_DELETED', content_sha256 = '', updated_at = ?1 
                     WHERE id = ?2",
                    params![now_ms, spec_id_str],
                )?;
                tx.execute(
                    "INSERT INTO spec_revisions (
                        id, spec_id, project_id, spec_key, revision_number,
                        operation_id, previous_sha256, content_hash,
                        actor_kind, actor_id, change_summary,
                        source_project_id, source_revision_sha256, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'FILE_DELETED', 'SYSTEM', 'external_watcher', 'Physical file deletion reconciled to tombstone', NULL, NULL, ?8)",
                    params![
                        rev_id,
                        spec_id_str,
                        project_id.to_string(),
                        spec_key.as_str(),
                        next_rev,
                        tombstone_id,
                        db_sha,
                        now_ms,
                    ],
                )?;
                tx.commit()
            })?;

            Ok(SyncOutcome::TombstoneReconciled {
                spec_key,
                previous_sha256: db_sha,
            })
        } else {
            Ok(SyncOutcome::MissingTrackedFile {
                spec_key,
                path: target_path.display().to_string(),
            })
        }
    }

    /// Explicitly deletes a spec from disk and SQLite, recording a tombstone and revision audit log.
    pub fn delete_spec(
        &self,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
        deleted_by: &str,
    ) -> Result<SyncOutcome, HarnessError> {
        let workspace_root = self.verify_project_scope(project_id)?;
        let _lock = SpecLockGuard::acquire(&workspace_root, project_id, spec_key, LOCK_TIMEOUT)?;
        let mut conn = self.pool.connection()?;

        let db_row: Option<(String, i64, String)> = conn
            .query_row(
                "SELECT id, version, content_sha256 FROM project_specs WHERE project_id = ?1 AND spec_key = ?2",
                params![project_id.to_string(), spec_key.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_spec_for_delete"))?;

        let Some((spec_id_str, db_ver, db_sha)) = db_row else {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "Spec {} not found for project {}",
                    spec_key.as_str(),
                    project_id
                ),
            });
        };

        let target_path = resolve_jailed_spec_path(&workspace_root, spec_key)?;
        if target_path.exists() {
            let _ = std::fs::remove_file(&target_path);
        }

        let tombstone_id = Uuid::new_v4().to_string();
        let rev_id = Uuid::new_v4().to_string();
        let now_ms = Utc::now().timestamp_millis();
        let next_rev = db_ver + 1;

        retry_on_busy("delete_spec_tombstone", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT OR REPLACE INTO spec_tombstones (
                    id, project_id, spec_key, deleted_at, deleted_by, content_sha256_at_deletion
                ) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, ?4, ?5)",
                params![
                    tombstone_id,
                    project_id.to_string(),
                    spec_key.as_str(),
                    deleted_by,
                    db_sha,
                ],
            )?;
            tx.execute(
                "UPDATE project_specs 
                 SET status = 'FILE_DELETED', content_sha256 = '', updated_at = ?1 
                 WHERE id = ?2",
                params![now_ms, spec_id_str],
            )?;
            tx.execute(
                "INSERT INTO spec_revisions (
                    id, spec_id, project_id, spec_key, revision_number,
                    operation_id, previous_sha256, content_hash,
                    actor_kind, actor_id, change_summary,
                    source_project_id, source_revision_sha256, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'FILE_DELETED', 'USER', ?8, 'Explicit spec deletion tombstoned', NULL, NULL, ?9)",
                params![
                    rev_id,
                    spec_id_str,
                    project_id.to_string(),
                    spec_key.as_str(),
                    next_rev,
                    tombstone_id,
                    db_sha,
                    deleted_by,
                    now_ms,
                ],
            )?;
            tx.commit()
        })?;

        Ok(SyncOutcome::TombstoneReconciled {
            spec_key,
            previous_sha256: db_sha,
        })
    }

    /// Checks whether a spec has been tombstoned for a project.
    pub fn is_spec_tombstoned(
        &self,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
    ) -> Result<bool, HarnessError> {
        let conn = self.pool.connection()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM spec_tombstones WHERE project_id = ?1 AND spec_key = ?2",
                params![project_id.to_string(), spec_key.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| map_sqlite_error(e, "check_tombstone"))?;
        Ok(count > 0)
    }
}

fn provenance_status_to_str(status: ProvenanceStatus) -> &'static str {
    match status {
        ProvenanceStatus::Confirmed => "CONFIRMED",
        ProvenanceStatus::Inferred => "INFERRED",
        ProvenanceStatus::Missing => "MISSING",
    }
}

fn parse_provenance_status(raw: &str) -> ProvenanceStatus {
    match raw {
        "INFERRED" => ProvenanceStatus::Inferred,
        "MISSING" => ProvenanceStatus::Missing,
        _ => ProvenanceStatus::Confirmed,
    }
}

/// Validates and canonicalizes `provenance_summary_json` per U-12:
/// - UTF-8 JSON object, maximum 16 KiB
/// - Required keys filled/validated: `origin`, `actor`, `operation_id`
/// - Allowed optional keys: `source_project_id`, `source_revision_sha256`, `CONFIRMED`, `INFERRED`, `MISSING`
/// - Rejects unknown arbitrary keys
fn normalize_and_validate_provenance_json(
    raw_json: &str,
    default_actor: &str,
    operation_id: Uuid,
    source_project_id: Option<Uuid>,
    source_revision_sha256: Option<&str>,
) -> Result<String, HarnessError> {
    if raw_json.len() > MAX_PROVENANCE_JSON_BYTES {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "PROVENANCE_JSON_TOO_LARGE: {} bytes exceeds 16 KiB limit",
                raw_json.len()
            ),
        });
    }

    let trimmed = raw_json.trim();
    let parsed: serde_json::Value = if trimmed.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(trimmed).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("INVALID_PROVENANCE_JSON: {e}"),
        })?
    };

    let obj = parsed
        .as_object()
        .ok_or_else(|| HarnessError::ValidationFailed {
            reason: "INVALID_PROVENANCE_JSON: root must be a JSON object".to_string(),
        })?;

    const ALLOWED_KEYS: &[&str] = &[
        "origin",
        "actor",
        "operation_id",
        "source_project_id",
        "source_revision_sha256",
        "CONFIRMED",
        "INFERRED",
        "MISSING",
    ];

    let mut canonical_map = BTreeMap::new();
    for (k, v) in obj {
        if !ALLOWED_KEYS.contains(&k.as_str()) {
            return Err(HarnessError::ValidationFailed {
                reason: format!("INVALID_PROVENANCE_JSON: unknown key '{k}' is forbidden"),
            });
        }
        canonical_map.insert(k.clone(), v.clone());
    }

    canonical_map
        .entry("origin".to_string())
        .or_insert_with(|| serde_json::Value::String("registry_upsert".to_string()));
    canonical_map
        .entry("actor".to_string())
        .or_insert_with(|| serde_json::Value::String(default_actor.to_string()));
    canonical_map.insert(
        "operation_id".to_string(),
        serde_json::Value::String(operation_id.to_string()),
    );

    if let Some(sp) = source_project_id {
        canonical_map.insert(
            "source_project_id".to_string(),
            serde_json::Value::String(sp.to_string()),
        );
    }
    if let Some(sr) = source_revision_sha256 {
        canonical_map.insert(
            "source_revision_sha256".to_string(),
            serde_json::Value::String(sr.to_string()),
        );
    }

    serde_json::to_string(&canonical_map).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("PROVENANCE_SERIALIZE_ERROR: {e}"),
    })
}
