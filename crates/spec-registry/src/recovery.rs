use std::path::Path;

use chrono::Utc;
use domain::{CanonicalSpecKey, HarnessError};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

use crate::errors::{map_sqlite_error, retry_on_busy};
use crate::sync::{
    compute_sha256_hex, compute_summary_digest, read_jailed_spec_bytes, remove_jailed_temp_file,
    resolve_jailed_spec_path,
};

#[derive(Debug, Clone)]
pub struct PendingIntentRow {
    pub operation_id: String,
    pub spec_id: String,
    pub project_id: String,
    pub spec_key: String,
    pub expected_version: i64,
    pub old_sha256: Option<String>,
    pub new_sha256: String,
    pub actor_kind: String,
    pub actor_id: String,
    pub change_summary: String,
    pub provenance_summary_json: String,
    pub source_project_id: Option<String>,
    pub source_revision_sha256: Option<String>,
}

/// Scans and deterministically resolves any `PENDING` `spec_write_intents` for `(project_id, spec_key)`
/// before serving any read/write or watcher operation.
pub fn recover_pending_spec(
    conn: &mut Connection,
    workspace_root: &Path,
    project_id: Uuid,
    spec_key: CanonicalSpecKey,
) -> Result<(), HarnessError> {
    let _ = resolve_jailed_spec_path(workspace_root, spec_key)?;

    let pending_list: Vec<PendingIntentRow> = {
        let mut stmt = conn
            .prepare(
                "SELECT operation_id, spec_id, project_id, spec_key, expected_version,
                        old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                        provenance_summary_json, source_project_id, source_revision_sha256
                 FROM spec_write_intents
                 WHERE project_id = ?1 AND spec_key = ?2 AND state = 'PENDING'
                 ORDER BY created_at ASC",
            )
            .map_err(|e| map_sqlite_error(e, "query_pending_intents"))?;

        let rows = stmt
            .query_map(params![project_id.to_string(), spec_key.as_str()], |row| {
                Ok(PendingIntentRow {
                    operation_id: row.get(0)?,
                    spec_id: row.get(1)?,
                    project_id: row.get(2)?,
                    spec_key: row.get(3)?,
                    expected_version: row.get(4)?,
                    old_sha256: row.get(5)?,
                    new_sha256: row.get(6)?,
                    actor_kind: row.get(7)?,
                    actor_id: row.get(8)?,
                    change_summary: row.get(9)?,
                    provenance_summary_json: row.get(10)?,
                    source_project_id: row.get(11)?,
                    source_revision_sha256: row.get(12)?,
                })
            })
            .map_err(|e| map_sqlite_error(e, "map_pending_intents"))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| map_sqlite_error(e, "collect_pending_intents"))?);
        }
        list
    };

    if pending_list.is_empty() {
        return Ok(());
    }

    let disk_sha_opt =
        read_jailed_spec_bytes(workspace_root, spec_key)?.map(|bytes| compute_sha256_hex(&bytes));

    for intent in pending_list {
        let _ = remove_jailed_temp_file(workspace_root, spec_key, &intent.operation_id);

        let now_ms = Utc::now().timestamp_millis();

        // Check current DB spec state
        let db_spec: Option<(i64, String)> = conn
            .query_row(
                "SELECT version, content_sha256 FROM project_specs WHERE id = ?1",
                params![intent.spec_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "recovery_read_spec"))?;

        match (&disk_sha_opt, &db_spec) {
            // Case 1: Disk hash matches new_sha256 (rename succeeded before crash)
            (Some(disk_sha), Some((db_ver, db_sha))) if disk_sha == &intent.new_sha256 => {
                if db_sha == &intent.new_sha256 {
                    // Already finalized in DB; just mark intent COMMITTED
                    retry_on_busy("recovery_mark_committed", || {
                        conn.execute(
                            "UPDATE spec_write_intents SET state = 'COMMITTED', updated_at = ?1 WHERE operation_id = ?2",
                            params![now_ms, intent.operation_id],
                        )
                    })?;
                } else if Some(db_sha.as_str()) == intent.old_sha256.as_deref() || db_sha.is_empty()
                {
                    // Finalize CAS + append revision + mark COMMITTED atomically
                    let max_rev: i64 = conn
                        .query_row(
                            "SELECT COALESCE(MAX(revision_number), 0) FROM spec_revisions WHERE spec_id = ?1",
                            params![intent.spec_id],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    let new_ver = if max_rev == 0 && db_sha.is_empty() {
                        1
                    } else {
                        (*db_ver).max(max_rev) + 1
                    };
                    let src_proj_uuid = intent
                        .source_project_id
                        .as_deref()
                        .and_then(|s| Uuid::parse_str(s).ok());
                    let summary_digest = compute_summary_digest(
                        spec_key,
                        &intent.new_sha256,
                        &intent.actor_id,
                        &intent.change_summary,
                        src_proj_uuid,
                        intent.source_revision_sha256.as_deref(),
                    );
                    let rev_id = Uuid::new_v4().to_string();

                    let recovered = retry_on_busy("recovery_finalize_cas", || {
                        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                        let updated = tx.execute(
                            "UPDATE project_specs
                             SET version = ?1,
                                 content_sha256 = ?2,
                                 summary_digest = ?3,
                                 provenance_summary_json = ?4,
                                 source_project_id = ?5,
                                 source_revision_sha256 = ?6,
                                 updated_at = ?7
                             WHERE id = ?8 AND version = ?9 AND content_sha256 = ?10",
                            params![
                                new_ver,
                                intent.new_sha256,
                                summary_digest,
                                intent.provenance_summary_json,
                                intent.source_project_id,
                                intent.source_revision_sha256,
                                now_ms,
                                intent.spec_id,
                                db_ver,
                                db_sha,
                            ],
                        )?;
                        if updated != 1 {
                            tx.execute(
                                "UPDATE spec_write_intents SET state = 'ABORTED', updated_at = ?1 WHERE operation_id = ?2",
                                params![now_ms, intent.operation_id],
                            )?;
                            tx.commit()?;
                            return Ok(false);
                        }
                        tx.execute(
                            "INSERT INTO spec_revisions (
                                id, spec_id, project_id, spec_key, revision_number,
                                operation_id, previous_sha256, content_hash,
                                actor_kind, actor_id, change_summary,
                                source_project_id, source_revision_sha256, created_at
                            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                            params![
                                rev_id,
                                intent.spec_id,
                                intent.project_id,
                                intent.spec_key,
                                new_ver,
                                intent.operation_id,
                                intent.old_sha256,
                                intent.new_sha256,
                                intent.actor_kind,
                                intent.actor_id,
                                intent.change_summary,
                                intent.source_project_id,
                                intent.source_revision_sha256,
                                now_ms,
                            ],
                        )?;
                        tx.execute(
                            "UPDATE spec_write_intents SET state = 'COMMITTED', updated_at = ?1 WHERE operation_id = ?2",
                            params![now_ms, intent.operation_id],
                        )?;
                        tx.commit()?;
                        Ok(true)
                    })?;
                    if !recovered {
                        return Err(HarnessError::StaleFile {
                            path: format!("product/specs/{}", spec_key.as_str()),
                            expected_hash: db_sha.clone(),
                            actual_hash: "RECOVERY_CAS_CONFLICT".to_string(),
                        });
                    }
                } else {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "RECOVERY_CONFLICT [{}]: disk matches new_sha256 but DB sha ({db_sha}) != old_sha256 ({:?})",
                            spec_key.as_str(),
                            intent.old_sha256
                        ),
                    });
                }
            }
            // Case 2: Disk hash matches old_sha256 or file does not exist for initial creation (crash before rename)
            (disk_opt, _)
                if disk_opt.as_deref() == intent.old_sha256.as_deref()
                    || (disk_opt.is_none() && intent.old_sha256.is_none()) =>
            {
                retry_on_busy("recovery_mark_aborted", || {
                    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                    tx.execute(
                        "UPDATE spec_write_intents SET state = 'ABORTED', updated_at = ?1 WHERE operation_id = ?2",
                        params![now_ms, intent.operation_id],
                    )?;
                    if intent.old_sha256.is_none() {
                        tx.execute(
                            "DELETE FROM project_specs WHERE id = ?1 AND content_sha256 = ''",
                            params![intent.spec_id],
                        )?;
                    }
                    tx.commit()
                })?;
            }
            // Case 3: Any third hash -> RecoveryConflict (fail closed, preserve disk bytes)
            (disk_opt, _) => {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "RECOVERY_CONFLICT [{}]: unexpected disk sha ({:?}) neither matches old_sha256 ({:?}) nor new_sha256 ({})",
                        spec_key.as_str(),
                        disk_opt,
                        intent.old_sha256,
                        intent.new_sha256
                    ),
                });
            }
        }
    }

    Ok(())
}
