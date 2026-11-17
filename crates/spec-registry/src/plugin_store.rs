// @story 08.4
// @implements AC-01, AC-03, AC-04, AC-05
//! SQLite WAL persistence helpers for `skill_plugins`, `knowledge_cards`,
//! `learn_consumed_nonces`, `plugin_cron_schedules`, and audit `gate_reports`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use domain::HarnessError;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::errors::{is_sqlite_busy_or_locked, map_sqlite_error};
use crate::pool::SqlitePool;

pub const DEFAULT_LEASE_TTL_MS: i64 = 30_000;
pub const MAX_CONSECUTIVE_CRON_FAILURES: i32 = 6;
const BUSY_BACKOFF_MS: [u64; 5] = [10, 20, 40, 80, 160];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPluginRow {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub plugin_slug: String,
    pub version: String,
    pub lifecycle_state: String,
    pub source_type: String,
    pub source_repo_url: String,
    pub upstream_repo_url: Option<String>,
    pub upstream_commit_sha: Option<String>,
    pub cron_schedule: Option<String>,
    pub skill_md_path: String,
    pub plugin_json_path: String,
    pub manifest_json: String,
    pub last_synced_at: Option<i64>,
    pub revision: u64,
    pub staging_version: Option<String>,
    pub staging_manifest_json: Option<String>,
    pub staging_diff_reasons_json: Option<String>,
    pub staging_bundle_dir: Option<String>,
    pub staging_package_sha256: Option<String>,
    pub breaking_change_isolated: bool,
    pub quarantined: bool,
    pub quarantine_reason: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCardRow {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub idempotency_key: Option<String>,
    pub category: String,
    pub title: String,
    pub content: String,
    pub content_sha256: String,
    pub source_evidence: String,
    pub status: String,
    pub revision: u64,
    pub reviewed_by: Option<String>,
    pub decision_reason: Option<String>,
    pub created_at: i64,
    pub accepted_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumedNonceRow {
    pub tenant_id: String,
    pub project_id: String,
    pub nonce: String,
    pub expires_at_s: i64,
    pub consumed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCronScheduleRow {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub plugin_id: String,
    pub cron_expression: String,
    pub upstream_repo_url: String,
    pub last_etag: Option<String>,
    pub last_commit_sha: Option<String>,
    pub lease_owner: Option<String>,
    pub fencing_token: i64,
    pub stale_after_timestamp: Option<i64>,
    pub consecutive_failures: i32,
    pub next_retry_at: Option<i64>,
    pub sync_status: String,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CasFailure {
    NotFound,
    RevisionConflict { current_revision: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditReceiptInput<'a> {
    pub tenant_id: &'a str,
    pub project_id: &'a str,
    pub correlation_id: &'a str,
    pub gate_name: &'a str,
    pub passed: bool,
    pub checker_id: &'a str,
    pub target_id: &'a str,
    pub action: &'a str,
    pub revision: u64,
    pub fencing_token: i64,
    pub reason_code: &'a str,
}

/// Runs an immediate SQLite transaction with bounded jittered backoff on `SQLITE_BUSY`.
fn run_immediate_tx<T, F>(pool: &SqlitePool, context: &str, mut op: F) -> Result<T, HarnessError>
where
    F: FnMut(&Transaction<'_>) -> Result<T, HarnessError>,
{
    for (attempt, &base_ms) in BUSY_BACKOFF_MS.iter().enumerate() {
        let mut conn = match pool.connection() {
            Ok(c) => c,
            Err(HarnessError::SqliteBusy { .. }) if attempt + 1 < BUSY_BACKOFF_MS.len() => {
                std::thread::sleep(Duration::from_millis(base_ms));
                continue;
            }
            Err(e) => return Err(e),
        };
        let tx = match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
            Ok(t) => t,
            Err(err) if is_sqlite_busy_or_locked(&err) && attempt + 1 < BUSY_BACKOFF_MS.len() => {
                let jitter = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| u64::from(d.subsec_nanos() % 16))
                    .unwrap_or(0);
                std::thread::sleep(Duration::from_millis(base_ms + jitter));
                continue;
            }
            Err(err) => return Err(map_sqlite_error(err, context)),
        };
        match op(&tx) {
            Ok(val) => match tx.commit() {
                Ok(()) => return Ok(val),
                Err(err)
                    if is_sqlite_busy_or_locked(&err) && attempt + 1 < BUSY_BACKOFF_MS.len() =>
                {
                    std::thread::sleep(Duration::from_millis(base_ms));
                    continue;
                }
                Err(err) => return Err(map_sqlite_error(err, context)),
            },
            Err(HarnessError::SqliteBusy { .. }) if attempt + 1 < BUSY_BACKOFF_MS.len() => {
                std::thread::sleep(Duration::from_millis(base_ms));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(HarnessError::SqliteBusy {
        details: format!("{context}: exceeded busy retry budget"),
    })
}

pub fn verify_project_scope(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
) -> Result<(), HarnessError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "verify_project_scope"))?;
    if exists.is_none() {
        return Err(HarnessError::ProjectNotFound {
            project_id: project_id.to_string(),
        });
    }
    Ok(())
}

pub fn record_gate_report_tx(
    conn: &Connection,
    input: &AuditReceiptInput<'_>,
) -> Result<String, HarnessError> {
    let id = Uuid::new_v4().to_string();
    let now_ms = Utc::now().timestamp_millis();
    let canonical = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        input.tenant_id,
        input.project_id,
        input.correlation_id,
        input.gate_name,
        input.target_id,
        input.action,
        input.revision,
        input.fencing_token,
        input.reason_code,
        input.checker_id,
        now_ms
    );
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let evidence_hash = format!("{:x}", hasher.finalize());
    conn.execute(
        "INSERT INTO gate_reports (id, project_id, gate_name, passed, checker_id, evidence_hash, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            input.project_id,
            input.gate_name,
            if input.passed { 1 } else { 0 },
            input.checker_id,
            evidence_hash,
            now_ms
        ],
    )
    .map_err(|e| map_sqlite_error(e, "insert_gate_report"))?;
    Ok(evidence_hash)
}

fn map_skill_plugin_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SkillPluginRow> {
    let rev_i64: i64 = r.get(15)?;
    Ok(SkillPluginRow {
        id: r.get(0)?,
        tenant_id: r.get(1)?,
        project_id: r.get(2)?,
        plugin_slug: r.get(3)?,
        version: r.get(4)?,
        lifecycle_state: r.get(5)?,
        source_type: r.get(6)?,
        source_repo_url: r.get(7)?,
        upstream_repo_url: r.get(8)?,
        upstream_commit_sha: r.get(9)?,
        cron_schedule: r.get(10)?,
        skill_md_path: r.get(11)?,
        plugin_json_path: r.get(12)?,
        manifest_json: r.get(13)?,
        last_synced_at: r.get(14)?,
        revision: u64::try_from(rev_i64.max(1)).unwrap_or(1),
        staging_version: r.get(16)?,
        staging_manifest_json: r.get(17)?,
        staging_diff_reasons_json: r.get(18)?,
        staging_bundle_dir: r.get(19)?,
        staging_package_sha256: r.get(20)?,
        breaking_change_isolated: r.get::<_, i64>(21)? != 0,
        quarantined: r.get::<_, i64>(22)? != 0,
        quarantine_reason: r.get(23)?,
        updated_at: r.get(24)?,
    })
}

const SELECT_SKILL_PLUGIN_COLS: &str = "sp.id, sp.tenant_id, sp.project_id, sp.plugin_slug, sp.version, \
    sp.lifecycle_state, sp.source_type, sp.source_repo_url, sp.upstream_repo_url, sp.upstream_commit_sha, \
    sp.cron_schedule, sp.skill_md_path, sp.plugin_json_path, sp.manifest_json, sp.last_synced_at, \
    sp.revision, sp.staging_version, sp.staging_manifest_json, sp.staging_diff_reasons_json, \
    sp.staging_bundle_dir, sp.staging_package_sha256, sp.breaking_change_isolated, sp.quarantined, \
    sp.quarantine_reason, sp.updated_at";

pub fn list_plugins_for_tenant(
    conn: &Connection,
    tenant_id: &str,
    project_id: Option<&str>,
) -> Result<Vec<SkillPluginRow>, HarnessError> {
    if let Some(pid) = project_id {
        verify_project_scope(conn, tenant_id, pid)?;
        let sql = format!(
            "SELECT {SELECT_SKILL_PLUGIN_COLS} FROM skill_plugins sp
             INNER JOIN projects pr ON pr.id = sp.project_id
             WHERE sp.tenant_id = ?1 AND pr.tenant_id = ?1 AND sp.project_id = ?2
               AND pr.trash_deleted_at IS NULL
             ORDER BY sp.updated_at DESC, sp.id ASC"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| map_sqlite_error(e, "prepare_list_plugins_scoped"))?;
        let rows = stmt
            .query_map(params![tenant_id, pid], map_skill_plugin_row)
            .map_err(|e| map_sqlite_error(e, "query_list_plugins_scoped"))?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item.map_err(|e| map_sqlite_error(e, "read_plugin_row"))?);
        }
        return Ok(out);
    }

    let sql = format!(
        "SELECT {SELECT_SKILL_PLUGIN_COLS} FROM skill_plugins sp
         INNER JOIN projects pr ON pr.id = sp.project_id
         WHERE sp.tenant_id = ?1 AND pr.tenant_id = ?1 AND pr.trash_deleted_at IS NULL
         ORDER BY sp.updated_at DESC, sp.id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| map_sqlite_error(e, "prepare_list_plugins_tenant"))?;
    let rows = stmt
        .query_map(params![tenant_id], map_skill_plugin_row)
        .map_err(|e| map_sqlite_error(e, "query_list_plugins_tenant"))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| map_sqlite_error(e, "read_plugin_row"))?);
    }
    Ok(out)
}

pub fn get_plugin_scoped(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    plugin_id: &str,
) -> Result<Option<SkillPluginRow>, HarnessError> {
    verify_project_scope(conn, tenant_id, project_id)?;
    let sql = format!(
        "SELECT {SELECT_SKILL_PLUGIN_COLS} FROM skill_plugins sp
         WHERE sp.id = ?1 AND sp.tenant_id = ?2 AND sp.project_id = ?3"
    );
    conn.query_row(
        &sql,
        params![plugin_id, tenant_id, project_id],
        map_skill_plugin_row,
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "get_plugin_scoped"))
}

pub fn find_plugin_for_tenant(
    conn: &Connection,
    tenant_id: &str,
    plugin_id: &str,
) -> Result<Option<SkillPluginRow>, HarnessError> {
    let sql = format!(
        "SELECT {SELECT_SKILL_PLUGIN_COLS} FROM skill_plugins sp
         INNER JOIN projects pr ON pr.id = sp.project_id
         WHERE sp.id = ?1 AND sp.tenant_id = ?2 AND pr.tenant_id = ?2 AND pr.trash_deleted_at IS NULL"
    );
    conn.query_row(&sql, params![plugin_id, tenant_id], map_skill_plugin_row)
        .optional()
        .map_err(|e| map_sqlite_error(e, "find_plugin_for_tenant"))
}

fn demote_conflicting_active_plugins_tx(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    plugin_slug: &str,
    active_plugin_id: &str,
    updated_at: i64,
) -> Result<(), HarnessError> {
    tx.execute(
        "UPDATE skill_plugins
         SET lifecycle_state = 'DEPRECATED',
             staging_version = NULL,
             staging_manifest_json = NULL,
             staging_diff_reasons_json = NULL,
             staging_bundle_dir = NULL,
             staging_package_sha256 = NULL,
             breaking_change_isolated = 0,
             revision = revision + 1,
             updated_at = ?1
         WHERE tenant_id = ?2 AND project_id = ?3 AND plugin_slug = ?4
           AND id != ?5 AND lifecycle_state = 'ACTIVE'",
        params![
            updated_at,
            tenant_id,
            project_id,
            plugin_slug,
            active_plugin_id,
        ],
    )
    .map(|_| ())
    .map_err(|e| map_sqlite_error(e, "demote_prior_active_plugin"))
}

pub fn upsert_plugin_record(
    pool: &SqlitePool,
    row: &SkillPluginRow,
    audit: Option<&AuditReceiptInput<'_>>,
) -> Result<SkillPluginRow, HarnessError> {
    run_immediate_tx(pool, "upsert_plugin_record", |tx| {
        verify_project_scope(tx, &row.tenant_id, &row.project_id)?;
        if row.lifecycle_state == "ACTIVE" {
            demote_conflicting_active_plugins_tx(
                tx,
                &row.tenant_id,
                &row.project_id,
                &row.plugin_slug,
                &row.id,
                row.updated_at,
            )?;
        }
        tx.execute(
            "INSERT INTO skill_plugins (
                id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_type,
                source_repo_url, upstream_repo_url, upstream_commit_sha, cron_schedule,
                skill_md_path, plugin_json_path, manifest_json, last_synced_at, revision,
                staging_version, staging_manifest_json, staging_diff_reasons_json,
                staging_bundle_dir, staging_package_sha256, breaking_change_isolated,
                quarantined, quarantine_reason, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19,
                ?20, ?21, ?22,
                ?23, ?24, ?25
             )
             ON CONFLICT(id) DO UPDATE SET
                lifecycle_state = excluded.lifecycle_state,
                source_type = excluded.source_type,
                source_repo_url = excluded.source_repo_url,
                upstream_repo_url = excluded.upstream_repo_url,
                upstream_commit_sha = excluded.upstream_commit_sha,
                cron_schedule = excluded.cron_schedule,
                skill_md_path = excluded.skill_md_path,
                plugin_json_path = excluded.plugin_json_path,
                manifest_json = excluded.manifest_json,
                last_synced_at = excluded.last_synced_at,
                revision = excluded.revision,
                staging_version = excluded.staging_version,
                staging_manifest_json = excluded.staging_manifest_json,
                staging_diff_reasons_json = excluded.staging_diff_reasons_json,
                staging_bundle_dir = excluded.staging_bundle_dir,
                staging_package_sha256 = excluded.staging_package_sha256,
                breaking_change_isolated = excluded.breaking_change_isolated,
                quarantined = excluded.quarantined,
                quarantine_reason = excluded.quarantine_reason,
                updated_at = excluded.updated_at",
            params![
                row.id,
                row.tenant_id,
                row.project_id,
                row.plugin_slug,
                row.version,
                row.lifecycle_state,
                row.source_type,
                row.source_repo_url,
                row.upstream_repo_url,
                row.upstream_commit_sha,
                row.cron_schedule,
                row.skill_md_path,
                row.plugin_json_path,
                row.manifest_json,
                row.last_synced_at,
                i64::try_from(row.revision.max(1)).unwrap_or(i64::MAX),
                row.staging_version,
                row.staging_manifest_json,
                row.staging_diff_reasons_json,
                row.staging_bundle_dir,
                row.staging_package_sha256,
                if row.breaking_change_isolated { 1 } else { 0 },
                if row.quarantined { 1 } else { 0 },
                row.quarantine_reason,
                row.updated_at,
            ],
        )
        .map_err(|e| map_sqlite_error(e, "upsert_skill_plugin"))?;
        if let Some(a) = audit {
            record_gate_report_tx(tx, a)?;
        }
        Ok(row.clone())
    })
}

pub fn update_plugin_cas(
    pool: &SqlitePool,
    expected_revision: u64,
    next: &SkillPluginRow,
    audit: Option<&AuditReceiptInput<'_>>,
) -> Result<Result<SkillPluginRow, CasFailure>, HarnessError> {
    run_immediate_tx(pool, "update_plugin_cas", |tx| {
        verify_project_scope(tx, &next.tenant_id, &next.project_id)?;
        let expected_rev_i64 = i64::try_from(expected_revision.max(1)).unwrap_or(i64::MAX);
        let next_rev_i64 = i64::try_from(next.revision.max(1)).unwrap_or(i64::MAX);

        if next.lifecycle_state == "ACTIVE" {
            demote_conflicting_active_plugins_tx(
                tx,
                &next.tenant_id,
                &next.project_id,
                &next.plugin_slug,
                &next.id,
                next.updated_at,
            )?;
        }

        let affected = tx
            .execute(
                "UPDATE skill_plugins SET
                    version = ?1,
                    lifecycle_state = ?2,
                    source_type = ?3,
                    source_repo_url = ?4,
                    upstream_repo_url = ?5,
                    upstream_commit_sha = ?6,
                    cron_schedule = ?7,
                    skill_md_path = ?8,
                    plugin_json_path = ?9,
                    manifest_json = ?10,
                    last_synced_at = ?11,
                    revision = ?12,
                    staging_version = ?13,
                    staging_manifest_json = ?14,
                    staging_diff_reasons_json = ?15,
                    staging_bundle_dir = ?16,
                    staging_package_sha256 = ?17,
                    breaking_change_isolated = ?18,
                    quarantined = ?19,
                    quarantine_reason = ?20,
                    updated_at = ?21
                 WHERE id = ?22 AND tenant_id = ?23 AND project_id = ?24 AND revision = ?25",
                params![
                    next.version,
                    next.lifecycle_state,
                    next.source_type,
                    next.source_repo_url,
                    next.upstream_repo_url,
                    next.upstream_commit_sha,
                    next.cron_schedule,
                    next.skill_md_path,
                    next.plugin_json_path,
                    next.manifest_json,
                    next.last_synced_at,
                    next_rev_i64,
                    next.staging_version,
                    next.staging_manifest_json,
                    next.staging_diff_reasons_json,
                    next.staging_bundle_dir,
                    next.staging_package_sha256,
                    if next.breaking_change_isolated { 1 } else { 0 },
                    if next.quarantined { 1 } else { 0 },
                    next.quarantine_reason,
                    next.updated_at,
                    next.id,
                    next.tenant_id,
                    next.project_id,
                    expected_rev_i64,
                ],
            )
            .map_err(|e| map_sqlite_error(e, "update_skill_plugin_cas"))?;

        if affected == 0 {
            let current_rev: Option<i64> = tx
                .query_row(
                    "SELECT revision FROM skill_plugins WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
                    params![next.id, next.tenant_id, next.project_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| map_sqlite_error(e, "check_plugin_cas_revision"))?;
            return Ok(Err(match current_rev {
                Some(r) => CasFailure::RevisionConflict {
                    current_revision: u64::try_from(r.max(1)).unwrap_or(1),
                },
                None => CasFailure::NotFound,
            }));
        }

        if let Some(a) = audit {
            record_gate_report_tx(tx, a)?;
        }
        Ok(Ok(next.clone()))
    })
}

fn map_learn_card_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeCardRow> {
    let rev_i64: i64 = r.get(10)?;
    Ok(KnowledgeCardRow {
        id: r.get(0)?,
        tenant_id: r.get(1)?,
        project_id: r.get(2)?,
        idempotency_key: r.get(3)?,
        category: r.get(4)?,
        title: r.get(5)?,
        content: r.get(6)?,
        content_sha256: r.get(7)?,
        source_evidence: r.get(8)?,
        status: r.get(9)?,
        revision: u64::try_from(rev_i64.max(1)).unwrap_or(1),
        reviewed_by: r.get(11)?,
        decision_reason: r.get(12)?,
        created_at: r.get(13)?,
        accepted_at: r.get(14)?,
    })
}

const SELECT_LEARN_CARD_COLS: &str =
    "id, tenant_id, project_id, idempotency_key, category, title, \
    content, content_sha256, source_evidence, status, revision, reviewed_by, decision_reason, \
    created_at, accepted_at";

pub fn list_learn_cards_scoped(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
) -> Result<Vec<KnowledgeCardRow>, HarnessError> {
    verify_project_scope(conn, tenant_id, project_id)?;
    let sql = format!(
        "SELECT {SELECT_LEARN_CARD_COLS} FROM knowledge_cards
         WHERE tenant_id = ?1 AND project_id = ?2
         ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| map_sqlite_error(e, "prepare_list_learn_cards"))?;
    let rows = stmt
        .query_map(params![tenant_id, project_id], map_learn_card_row)
        .map_err(|e| map_sqlite_error(e, "query_list_learn_cards"))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| map_sqlite_error(e, "read_learn_card_row"))?);
    }
    Ok(out)
}

pub fn list_consumed_nonces_scoped(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    now_s: i64,
) -> Result<Vec<ConsumedNonceRow>, HarnessError> {
    verify_project_scope(conn, tenant_id, project_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT tenant_id, project_id, nonce, expires_at_s, consumed_at
             FROM learn_consumed_nonces
             WHERE tenant_id = ?1 AND project_id = ?2 AND expires_at_s > ?3
             ORDER BY consumed_at ASC, nonce ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_list_nonces"))?;
    let rows = stmt
        .query_map(params![tenant_id, project_id, now_s], |r| {
            Ok(ConsumedNonceRow {
                tenant_id: r.get(0)?,
                project_id: r.get(1)?,
                nonce: r.get(2)?,
                expires_at_s: r.get(3)?,
                consumed_at: r.get(4)?,
            })
        })
        .map_err(|e| map_sqlite_error(e, "query_list_nonces"))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| map_sqlite_error(e, "read_nonce_row"))?);
    }
    Ok(out)
}

pub fn insert_learn_card_tx(
    pool: &SqlitePool,
    card: &KnowledgeCardRow,
    audit: Option<&AuditReceiptInput<'_>>,
) -> Result<KnowledgeCardRow, HarnessError> {
    run_immediate_tx(pool, "insert_learn_card_tx", |tx| {
        verify_project_scope(tx, &card.tenant_id, &card.project_id)?;

        if let Some(ref idem) = card.idempotency_key {
            let existing_sql = format!(
                "SELECT {SELECT_LEARN_CARD_COLS} FROM knowledge_cards
                 WHERE tenant_id = ?1 AND project_id = ?2 AND idempotency_key = ?3"
            );
            if let Some(found) = tx
                .query_row(
                    &existing_sql,
                    params![card.tenant_id, card.project_id, idem],
                    map_learn_card_row,
                )
                .optional()
                .map_err(|e| map_sqlite_error(e, "check_learn_idempotency"))?
            {
                if found.content_sha256 != card.content_sha256 || found.category != card.category {
                    return Err(HarnessError::ValidationFailed {
                        reason: "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH".to_string(),
                    });
                }
                return Ok(found);
            }
        }

        tx.execute(
            "INSERT INTO knowledge_cards (
                id, tenant_id, project_id, idempotency_key, category, title, content,
                content_sha256, source_evidence, status, revision, reviewed_by,
                decision_reason, created_at, accepted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                card.id,
                card.tenant_id,
                card.project_id,
                card.idempotency_key,
                card.category,
                card.title,
                card.content,
                card.content_sha256,
                card.source_evidence,
                card.status,
                i64::try_from(card.revision.max(1)).unwrap_or(1),
                card.reviewed_by,
                card.decision_reason,
                card.created_at,
                card.accepted_at,
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_knowledge_card"))?;

        if let Some(a) = audit {
            record_gate_report_tx(tx, a)?;
        }
        Ok(card.clone())
    })
}

pub fn commit_learn_card_decision(
    pool: &SqlitePool,
    expected_revision: u64,
    updated: &KnowledgeCardRow,
    nonce_record: &ConsumedNonceRow,
    audit: Option<&AuditReceiptInput<'_>>,
) -> Result<Result<KnowledgeCardRow, CasFailure>, HarnessError> {
    run_immediate_tx(pool, "commit_learn_card_decision", |tx| {
        verify_project_scope(tx, &updated.tenant_id, &updated.project_id)?;

        tx.execute(
            "DELETE FROM learn_consumed_nonces
             WHERE tenant_id = ?1 AND project_id = ?2 AND expires_at_s <= ?3",
            params![
                nonce_record.tenant_id,
                nonce_record.project_id,
                nonce_record.consumed_at
            ],
        )
        .map_err(|e| map_sqlite_error(e, "prune_learn_nonces"))?;

        let nonce_exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM learn_consumed_nonces
                 WHERE tenant_id = ?1 AND project_id = ?2 AND nonce = ?3",
                params![
                    nonce_record.tenant_id,
                    nonce_record.project_id,
                    nonce_record.nonce
                ],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "check_learn_nonce_replay"))?;
        if nonce_exists.is_some() {
            return Err(HarnessError::BoundaryViolation {
                reason: "LEARN_NONCE_REPLAYED".to_string(),
            });
        }

        let expected_rev_i64 = i64::try_from(expected_revision.max(1)).unwrap_or(i64::MAX);
        let next_rev_i64 = i64::try_from(updated.revision.max(1)).unwrap_or(i64::MAX);
        let changed = tx
            .execute(
                "UPDATE knowledge_cards
                 SET status = ?1,
                     revision = ?2,
                     reviewed_by = ?3,
                     decision_reason = ?4,
                     accepted_at = ?5
                 WHERE tenant_id = ?6 AND project_id = ?7 AND id = ?8
                   AND status = 'PROPOSED' AND revision = ?9",
                params![
                    updated.status,
                    next_rev_i64,
                    updated.reviewed_by,
                    updated.decision_reason,
                    updated.accepted_at,
                    updated.tenant_id,
                    updated.project_id,
                    updated.id,
                    expected_rev_i64,
                ],
            )
            .map_err(|e| map_sqlite_error(e, "update_knowledge_card_decision"))?;

        if changed == 0 {
            let current_rev: Option<i64> = tx
                .query_row(
                    "SELECT revision FROM knowledge_cards WHERE tenant_id = ?1 AND project_id = ?2 AND id = ?3",
                    params![updated.tenant_id, updated.project_id, updated.id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| map_sqlite_error(e, "lookup_card_revision"))?;
            return Ok(Err(match current_rev {
                Some(r) => CasFailure::RevisionConflict {
                    current_revision: u64::try_from(r.max(1)).unwrap_or(1),
                },
                None => CasFailure::NotFound,
            }));
        }

        tx.execute(
            "INSERT INTO learn_consumed_nonces (tenant_id, project_id, nonce, expires_at_s, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                nonce_record.tenant_id,
                nonce_record.project_id,
                nonce_record.nonce,
                nonce_record.expires_at_s,
                nonce_record.consumed_at,
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_consumed_nonce"))?;

        if let Some(a) = audit {
            record_gate_report_tx(tx, a)?;
        }
        Ok(Ok(updated.clone()))
    })
}

fn map_cron_schedule_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PluginCronScheduleRow> {
    Ok(PluginCronScheduleRow {
        id: r.get(0)?,
        tenant_id: r.get(1)?,
        project_id: r.get(2)?,
        plugin_id: r.get(3)?,
        cron_expression: r.get(4)?,
        upstream_repo_url: r.get(5)?,
        last_etag: r.get(6)?,
        last_commit_sha: r.get(7)?,
        lease_owner: r.get(8)?,
        fencing_token: r.get(9)?,
        stale_after_timestamp: r.get(10)?,
        consecutive_failures: r.get(11)?,
        next_retry_at: r.get(12)?,
        sync_status: r.get(13)?,
        last_error_code: r.get(14)?,
        last_error_message: r.get(15)?,
        updated_at: r.get(16)?,
    })
}

const SELECT_CRON_SCHEDULE_COLS: &str = "id, tenant_id, project_id, plugin_id, cron_expression, \
    upstream_repo_url, last_etag, last_commit_sha, lease_owner, fencing_token, \
    stale_after_timestamp, consecutive_failures, next_retry_at, sync_status, \
    last_error_code, last_error_message, updated_at";

pub fn get_cron_schedule_scoped(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    plugin_id: &str,
) -> Result<Option<PluginCronScheduleRow>, HarnessError> {
    verify_project_scope(conn, tenant_id, project_id)?;
    let sql = format!(
        "SELECT {SELECT_CRON_SCHEDULE_COLS} FROM plugin_cron_schedules
         WHERE tenant_id = ?1 AND project_id = ?2 AND plugin_id = ?3"
    );
    conn.query_row(
        &sql,
        params![tenant_id, project_id, plugin_id],
        map_cron_schedule_row,
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "get_cron_schedule_scoped"))
}

pub fn list_all_cron_schedules(
    conn: &Connection,
) -> Result<Vec<PluginCronScheduleRow>, HarnessError> {
    let sql = format!(
        "SELECT {SELECT_CRON_SCHEDULE_COLS} FROM plugin_cron_schedules ORDER BY updated_at ASC, id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| map_sqlite_error(e, "prepare_list_cron_schedules"))?;
    let rows = stmt
        .query_map([], map_cron_schedule_row)
        .map_err(|e| map_sqlite_error(e, "query_list_cron_schedules"))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| map_sqlite_error(e, "read_cron_schedule"))?);
    }
    Ok(out)
}

pub fn upsert_cron_schedule(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    plugin_id: &str,
    cron_expression: &str,
    upstream_repo_url: &str,
    audit: Option<&AuditReceiptInput<'_>>,
) -> Result<PluginCronScheduleRow, HarnessError> {
    run_immediate_tx(pool, "upsert_cron_schedule", |tx| {
        verify_project_scope(tx, tenant_id, project_id)?;

        let plugin_exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM skill_plugins WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
                params![plugin_id, tenant_id, project_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "verify_plugin_for_schedule"))?;
        if plugin_exists.is_none() {
            return Err(HarnessError::ValidationFailed {
                reason: format!("PLUGIN_NOT_FOUND [{plugin_id}]"),
            });
        }

        let now_ms = Utc::now().timestamp_millis();
        let sched_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO plugin_cron_schedules (
                id, tenant_id, project_id, plugin_id, cron_expression, upstream_repo_url,
                consecutive_failures, sync_status, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 'IDLE', ?7)
             ON CONFLICT(tenant_id, project_id, plugin_id) DO UPDATE SET
                cron_expression = excluded.cron_expression,
                upstream_repo_url = excluded.upstream_repo_url,
                sync_status = CASE WHEN sync_status = 'RUNNING' THEN 'RUNNING' ELSE 'IDLE' END,
                next_retry_at = NULL,
                updated_at = excluded.updated_at",
            params![
                sched_id,
                tenant_id,
                project_id,
                plugin_id,
                cron_expression,
                upstream_repo_url,
                now_ms,
            ],
        )
        .map_err(|e| map_sqlite_error(e, "upsert_plugin_cron_schedule"))?;

        tx.execute(
            "UPDATE skill_plugins
             SET cron_schedule = ?1, upstream_repo_url = ?2, updated_at = ?3
             WHERE id = ?4 AND tenant_id = ?5 AND project_id = ?6",
            params![
                cron_expression,
                upstream_repo_url,
                now_ms,
                plugin_id,
                tenant_id,
                project_id
            ],
        )
        .map_err(|e| map_sqlite_error(e, "sync_skill_plugin_cron_cols"))?;

        if let Some(a) = audit {
            record_gate_report_tx(tx, a)?;
        }
        let sql = format!(
            "SELECT {SELECT_CRON_SCHEDULE_COLS} FROM plugin_cron_schedules
             WHERE tenant_id = ?1 AND project_id = ?2 AND plugin_id = ?3"
        );
        tx.query_row(
            &sql,
            params![tenant_id, project_id, plugin_id],
            map_cron_schedule_row,
        )
        .map_err(|e| map_sqlite_error(e, "reload_cron_schedule"))
    })
}

#[allow(clippy::too_many_arguments)]
pub fn claim_cron_lease(
    pool: &SqlitePool,
    tenant_id: &str,
    project_id: &str,
    plugin_id: &str,
    lease_owner: &str,
    force_manual: bool,
    now_ms: i64,
    lease_ttl_ms: i64,
) -> Result<Option<PluginCronScheduleRow>, HarnessError> {
    run_immediate_tx(pool, "claim_cron_lease", |tx| {
        let stale_at = now_ms.saturating_add(lease_ttl_ms.max(1_000));
        let updated = if force_manual {
            tx.execute(
                "UPDATE plugin_cron_schedules
                 SET sync_status = 'RUNNING',
                     lease_owner = ?1,
                     fencing_token = fencing_token + 1,
                     stale_after_timestamp = ?2,
                     updated_at = ?3
                 WHERE tenant_id = ?4 AND project_id = ?5 AND plugin_id = ?6
                   AND sync_status != 'RUNNING'",
                params![
                    lease_owner,
                    stale_at,
                    now_ms,
                    tenant_id,
                    project_id,
                    plugin_id
                ],
            )
        } else {
            tx.execute(
                "UPDATE plugin_cron_schedules
                 SET sync_status = 'RUNNING',
                     lease_owner = ?1,
                     fencing_token = fencing_token + 1,
                     stale_after_timestamp = ?2,
                     updated_at = ?3
                 WHERE tenant_id = ?4 AND project_id = ?5 AND plugin_id = ?6
                   AND sync_status NOT IN ('RUNNING', 'AUTH_REQUIRED', 'FAILED')
                   AND (next_retry_at IS NULL OR next_retry_at <= ?3)",
                params![
                    lease_owner,
                    stale_at,
                    now_ms,
                    tenant_id,
                    project_id,
                    plugin_id
                ],
            )
        }
        .map_err(|e| map_sqlite_error(e, "update_claim_cron_lease"))?;

        if updated == 0 {
            return Ok(None);
        }
        let sql = format!(
            "SELECT {SELECT_CRON_SCHEDULE_COLS} FROM plugin_cron_schedules
             WHERE tenant_id = ?1 AND project_id = ?2 AND plugin_id = ?3"
        );
        let row = tx
            .query_row(
                &sql,
                params![tenant_id, project_id, plugin_id],
                map_cron_schedule_row,
            )
            .map_err(|e| map_sqlite_error(e, "read_claimed_cron_lease"))?;
        Ok(Some(row))
    })
}

pub fn refresh_cron_lease(
    pool: &SqlitePool,
    plugin_id: &str,
    fencing_token: i64,
    now_ms: i64,
    lease_ttl_ms: i64,
) -> Result<bool, HarnessError> {
    let conn = pool.connection()?;
    let rows = conn
        .execute(
            "UPDATE plugin_cron_schedules
             SET stale_after_timestamp = ?1, updated_at = ?2
             WHERE plugin_id = ?3 AND fencing_token = ?4 AND sync_status = 'RUNNING'",
            params![
                now_ms.saturating_add(lease_ttl_ms.max(1_000)),
                now_ms,
                plugin_id,
                fencing_token
            ],
        )
        .map_err(|e| map_sqlite_error(e, "refresh_cron_lease"))?;
    Ok(rows > 0)
}

pub fn clear_auth_required_schedules_for_tenant(
    pool: &SqlitePool,
    tenant_id: &str,
    now_ms: i64,
) -> Result<usize, HarnessError> {
    run_immediate_tx(pool, "clear_auth_required_schedules", |tx| {
        tx.execute(
            "UPDATE plugin_cron_schedules
             SET sync_status = 'IDLE',
                 consecutive_failures = 0,
                 next_retry_at = NULL,
                 last_error_code = NULL,
                 last_error_message = NULL,
                 updated_at = ?1
             WHERE tenant_id = ?2 AND sync_status = 'AUTH_REQUIRED'",
            params![now_ms, tenant_id],
        )
        .map_err(|e| map_sqlite_error(e, "clear_auth_required_schedules"))
    })
}

pub fn reap_stale_cron_leases(pool: &SqlitePool, now_ms: i64) -> Result<usize, HarnessError> {
    run_immediate_tx(pool, "reap_stale_cron_leases", |tx| {
        tx.execute(
            "UPDATE plugin_cron_schedules
             SET sync_status = 'FAILED',
                 fencing_token = fencing_token + 1,
                 lease_owner = NULL,
                 stale_after_timestamp = NULL,
                 last_error_code = 'LEASE_ORPHANED',
                 last_error_message = 'Cron sync lease expired or worker crashed; fence invalidated',
                 updated_at = ?1
             WHERE sync_status = 'RUNNING'
               AND (stale_after_timestamp IS NULL OR stale_after_timestamp < ?1)",
            params![now_ms],
        )
        .map_err(|e| map_sqlite_error(e, "reap_stale_cron_leases"))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencedCronCommit<'a> {
    pub tenant_id: &'a str,
    pub project_id: &'a str,
    pub plugin_id: &'a str,
    pub fencing_token: i64,
    pub sync_status: &'a str,
    pub last_etag: Option<&'a str>,
    pub last_commit_sha: Option<&'a str>,
    pub consecutive_failures: i32,
    pub next_retry_at: Option<i64>,
    pub last_error_code: Option<&'a str>,
    pub last_error_message: Option<&'a str>,
    pub updated_at: i64,
}

pub fn commit_cron_fenced(
    pool: &SqlitePool,
    commit: &FencedCronCommit<'_>,
) -> Result<bool, HarnessError> {
    run_immediate_tx(pool, "commit_cron_fenced", |tx| {
        let changed = tx
            .execute(
                "UPDATE plugin_cron_schedules
                 SET sync_status = ?1,
                     last_etag = COALESCE(?2, last_etag),
                     last_commit_sha = COALESCE(?3, last_commit_sha),
                     lease_owner = NULL,
                     stale_after_timestamp = NULL,
                     consecutive_failures = ?4,
                     next_retry_at = ?5,
                     last_error_code = ?6,
                     last_error_message = ?7,
                     updated_at = ?8
                 WHERE tenant_id = ?9 AND project_id = ?10 AND plugin_id = ?11
                   AND fencing_token = ?12 AND sync_status = 'RUNNING'",
                params![
                    commit.sync_status,
                    commit.last_etag,
                    commit.last_commit_sha,
                    commit.consecutive_failures.max(0),
                    commit.next_retry_at,
                    commit.last_error_code,
                    commit.last_error_message,
                    commit.updated_at,
                    commit.tenant_id,
                    commit.project_id,
                    commit.plugin_id,
                    commit.fencing_token,
                ],
            )
            .map_err(|e| map_sqlite_error(e, "update_commit_cron_fenced"))?;
        if changed == 0 {
            return Ok(false);
        }
        if commit.sync_status == "SUCCEEDED" {
            tx.execute(
                "UPDATE skill_plugins
                 SET upstream_commit_sha = COALESCE(?1, upstream_commit_sha),
                     last_synced_at = ?2,
                     updated_at = ?2
                 WHERE id = ?3 AND tenant_id = ?4 AND project_id = ?5",
                params![
                    commit.last_commit_sha,
                    commit.updated_at,
                    commit.plugin_id,
                    commit.tenant_id,
                    commit.project_id,
                ],
            )
            .map_err(|e| map_sqlite_error(e, "update_plugin_last_synced"))?;
        }
        Ok(true)
    })
}
