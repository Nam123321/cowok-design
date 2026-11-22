// @story 02.4
// @implements AC-02, AC-04
use constraint_engine::{check_appshell_lock_constraint_with_slots, extract_component_signatures};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::errors::map_sqlite_error;
use crate::locks::{
    current_epoch_ms, get_appshell_lock_for_portal, is_valid_ssot_portal,
    retry_on_busy_with_jitter, AppShellLockDto, SsotLockError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPatternDto {
    pub id: String,
    pub project_id: String,
    pub portal_id: String,
    pub pattern_slug: String,
    pub component_name: String,
    pub source_path: Option<String>,
    pub ast_signature_sha256: String,
    pub dedup_marker_sha256: String,
    pub source_revision: i64,
    pub last_sync_source: String,
    pub tokens_snapshot_json: String,
    pub doc_markdown: String,
    pub drift_status: String,
    pub quarantined_diff_json: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SsotComponentsPage {
    pub patterns: Vec<ComponentPatternDto>,
    pub lock: Option<AppShellLockDto>,
    pub next_cursor: Option<String>,
    pub quarantined_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverseSyncDisposition {
    Synced,
    SuppressedNoOp,
    QuarantinedDrift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseSyncPatternResult {
    pub disposition: ReverseSyncDisposition,
    pub pattern: ComponentPatternDto,
}

#[derive(Debug, Clone)]
pub struct ReverseSyncRequest {
    pub tenant_id: String,
    pub project_id: String,
    pub portal_id: String,
    pub caller_actor: String,
    pub source_file_path: String,
    pub source_code: String,
    pub now_ms: Option<i64>,
}

fn to_kebab_slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('-') {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(if ch == '_' { '-' } else { ch.to_ascii_lowercase() });
        }
    }
    if out.is_empty() {
        "component".to_string()
    } else {
        out
    }
}

fn compute_dedup_marker(portal_id: &str, source_path: &str, ast_sha: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(portal_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(source_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(ast_sha.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn parse_cursor(cursor: Option<&str>) -> Result<Option<(i64, String)>, SsotLockError> {
    let Some(raw) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some((ts_str, id)) = raw.split_once(':') else {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "Invalid cursor format; expected '<updatedAt>:<id>'".to_string(),
        });
    };
    let ts = ts_str.parse::<i64>().map_err(|_| SsotLockError::Validation {
        code: "INVALID_REQUEST",
        message: "Invalid cursor timestamp".to_string(),
    })?;
    if id.is_empty() {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "Invalid cursor id".to_string(),
        });
    }
    Ok(Some((ts, id.to_string())))
}

pub fn list_component_patterns(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    portal_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<SsotComponentsPage, SsotLockError> {
    if tenant_id.trim().is_empty() || project_id.trim().is_empty() {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "tenantId and projectId must be non-empty".to_string(),
        });
    }
    if !is_valid_ssot_portal(portal_id) {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: format!("Unsupported portalId: {portal_id}"),
        });
    }
    let clamped_limit = limit.clamp(1, 100);
    let cursor_pair = parse_cursor(cursor)?;

    let project_exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| SsotLockError::Internal {
            code: "INTERNAL_ERROR",
            message: map_sqlite_error(e, "list_component_patterns.project").to_string(),
        })?;
    if project_exists.is_none() {
        return Err(SsotLockError::NotFound {
            code: "PROJECT_NOT_FOUND",
            message: format!("Project `{project_id}` not found in tenant"),
        });
    }

    let row_mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<ComponentPatternDto> {
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
    };

    let fetch_limit = (clamped_limit + 1) as i64;
    let mut patterns = Vec::new();
    if let Some((c_ts, c_id)) = cursor_pair {
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, portal_id, pattern_slug, component_name, source_path, \
                        ast_signature_sha256, dedup_marker_sha256, source_revision, last_sync_source, \
                        tokens_snapshot_json, doc_markdown, drift_status, quarantined_diff_json, \
                        revision, created_at, updated_at \
                 FROM component_patterns \
                 WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 \
                   AND (updated_at < ?4 OR (updated_at = ?4 AND id < ?5)) \
                 ORDER BY updated_at DESC, id DESC \
                 LIMIT ?6",
            )
            .map_err(|e| SsotLockError::Internal {
                code: "INTERNAL_ERROR",
                message: map_sqlite_error(e, "list_component_patterns.cursor_prepare").to_string(),
            })?;
        let rows = stmt
            .query_map(
                params![tenant_id, project_id, portal_id, c_ts, c_id, fetch_limit],
                row_mapper,
            )
            .map_err(|e| SsotLockError::Internal {
                code: "INTERNAL_ERROR",
                message: map_sqlite_error(e, "list_component_patterns.cursor_query").to_string(),
            })?;
        for r in rows {
            patterns.push(r.map_err(|e| SsotLockError::Internal {
                code: "INTERNAL_ERROR",
                message: map_sqlite_error(e, "list_component_patterns.row").to_string(),
            })?);
        }
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, portal_id, pattern_slug, component_name, source_path, \
                        ast_signature_sha256, dedup_marker_sha256, source_revision, last_sync_source, \
                        tokens_snapshot_json, doc_markdown, drift_status, quarantined_diff_json, \
                        revision, created_at, updated_at \
                 FROM component_patterns \
                 WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 \
                 ORDER BY updated_at DESC, id DESC \
                 LIMIT ?4",
            )
            .map_err(|e| SsotLockError::Internal {
                code: "INTERNAL_ERROR",
                message: map_sqlite_error(e, "list_component_patterns.prepare").to_string(),
            })?;
        let rows = stmt
            .query_map(params![tenant_id, project_id, portal_id, fetch_limit], row_mapper)
            .map_err(|e| SsotLockError::Internal {
                code: "INTERNAL_ERROR",
                message: map_sqlite_error(e, "list_component_patterns.query").to_string(),
            })?;
        for r in rows {
            patterns.push(r.map_err(|e| SsotLockError::Internal {
                code: "INTERNAL_ERROR",
                message: map_sqlite_error(e, "list_component_patterns.row").to_string(),
            })?);
        }
    }

    let next_cursor = if patterns.len() > clamped_limit {
        patterns.truncate(clamped_limit);
        patterns
            .last()
            .map(|last| format!("{}:{}", last.updated_at, last.id))
    } else {
        None
    };

    let quarantined_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM component_patterns \
             WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 AND drift_status = 'quarantined_drift'",
            params![tenant_id, project_id, portal_id],
            |r| r.get(0),
        )
        .map_err(|e| SsotLockError::Internal {
            code: "INTERNAL_ERROR",
            message: map_sqlite_error(e, "list_component_patterns.quarantined_count").to_string(),
        })?;

    let lock = get_appshell_lock_for_portal(conn, tenant_id, project_id, portal_id, None)?;

    Ok(SsotComponentsPage {
        patterns,
        lock,
        next_cursor,
        quarantined_count: quarantined_count.max(0) as usize,
    })
}

/// Runs OXC AST reverse-sync from a TSX source file into V18 `component_patterns`:
/// - Suppresses duplicate edits via `dedup_marker_sha256` (`SuppressedNoOp`)
/// - Quarantines incoming drift into `quarantined_diff_json` when an active `app_shell_lock` is
///   held by another actor (`QuarantinedDrift`) without overwriting canonical SSOT fields.
pub fn reverse_sync_tsx_components(
    conn: &mut Connection,
    req: &ReverseSyncRequest,
) -> Result<Vec<ReverseSyncPatternResult>, SsotLockError> {
    if !is_valid_ssot_portal(&req.portal_id) {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: format!("Unsupported portalId: {}", req.portal_id),
        });
    }

    let extracted = extract_component_signatures(&req.source_file_path, &req.source_code)
        .map_err(|e| SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: e.to_string(),
        })?;
    if extracted.is_empty() {
        return Ok(Vec::new());
    }

    retry_on_busy_with_jitter(
        &req.tenant_id,
        &req.project_id,
        &req.portal_id,
        "reverse_sync_tsx_components",
        || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let now_ms = req.now_ms.unwrap_or_else(current_epoch_ms);

            let active_lock: Option<(String, i64, Vec<String>)> = tx
                .query_row(
                    "SELECT lock_owner_id, expires_at, locked_slots_json FROM app_shell_locks \
                     WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3",
                    params![req.tenant_id, req.project_id, req.portal_id],
                    |r| {
                        let slots_raw: String = r.get(2)?;
                        let slots: Vec<String> = serde_json::from_str(&slots_raw).unwrap_or_default();
                        Ok((r.get(0)?, r.get(1)?, slots))
                    },
                )
                .optional()?;

            let mut results = Vec::with_capacity(extracted.len());
            for sig in &extracted {
                let pattern_slug = to_kebab_slug(&sig.component_key);
                let dedup_marker = compute_dedup_marker(
                    &req.portal_id,
                    &sig.source_path,
                    &sig.ast_signature_sha256,
                );
                let tokens_snapshot_json = serde_json::json!({
                    "cssVariables": sig.css_variables,
                    "ffIds": sig.ff_ids,
                    "props": sig.prop_names,
                    "jsxTags": sig.jsx_tags,
                })
                .to_string();
                let doc_markdown = format!(
                    "### `{}` (`{}`)\n- Portal: `{}`\n- Props: {}\n- CSS Tokens: {}\n",
                    sig.display_name,
                    sig.source_path,
                    req.portal_id,
                    if sig.prop_names.is_empty() {
                        "none".to_string()
                    } else {
                        sig.prop_names.join(", ")
                    },
                    if sig.css_variables.is_empty() {
                        "none".to_string()
                    } else {
                        sig.css_variables.join(", ")
                    },
                );

                let existing_row: Option<ComponentPatternDto> = tx
                    .query_row(
                        "SELECT id, project_id, portal_id, pattern_slug, component_name, source_path, \
                                ast_signature_sha256, dedup_marker_sha256, source_revision, last_sync_source, \
                                tokens_snapshot_json, doc_markdown, drift_status, quarantined_diff_json, \
                                revision, created_at, updated_at \
                         FROM component_patterns \
                         WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 AND pattern_slug = ?4",
                        params![req.tenant_id, req.project_id, req.portal_id, pattern_slug],
                        |row| {
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
                        },
                    )
                    .optional()?;

                let (lock_violation, locked_slots_ref): (_, &[String]) = match &active_lock {
                    Some((owner, expires_at, slots)) => (
                        check_appshell_lock_constraint_with_slots(
                            &pattern_slug,
                            Some(owner.as_str()),
                            &req.caller_actor,
                            Some(*expires_at),
                            now_ms,
                            slots,
                        ),
                        slots.as_slice(),
                    ),
                    None => (None, &[]),
                };

                if let Some(mut existing) = existing_row {
                    let existing_quarantined_marker =
                        serde_json::from_str::<serde_json::Value>(&existing.quarantined_diff_json)
                            .ok()
                            .and_then(|v| {
                                v.get("incomingDedupMarkerSha256")
                                    .and_then(|s| s.as_str())
                                    .map(str::to_string)
                            });
                    let is_canonical_noop = existing.dedup_marker_sha256 == dedup_marker
                        && (existing.drift_status == "synced" || lock_violation.is_some());
                    let is_quarantined_noop = lock_violation.is_some()
                        && existing.drift_status == "quarantined_drift"
                        && existing_quarantined_marker.as_deref() == Some(dedup_marker.as_str());
                    if is_canonical_noop || is_quarantined_noop {
                        results.push(ReverseSyncPatternResult {
                            disposition: ReverseSyncDisposition::SuppressedNoOp,
                            pattern: existing,
                        });
                        continue;
                    }

                    if let Some(violation) = lock_violation {
                        let quarantine_payload = serde_json::json!({
                            "ruleCode": violation.rule_code,
                            "message": violation.message,
                            "lockedSlots": locked_slots_ref,
                            "incomingAstSha256": sig.ast_signature_sha256,
                            "incomingDedupMarkerSha256": dedup_marker,
                            "incomingTokensSnapshot": tokens_snapshot_json,
                            "callerActor": req.caller_actor,
                        })
                        .to_string();

                        tx.execute(
                            "UPDATE component_patterns \
                             SET drift_status = 'quarantined_drift', \
                                 quarantined_diff_json = ?1, \
                                 revision = revision + 1, \
                                 updated_at = ?2 \
                             WHERE id = ?3 AND tenant_id = ?4",
                            params![
                                quarantine_payload,
                                now_ms,
                                existing.id,
                                req.tenant_id
                            ],
                        )?;

                        existing.drift_status = "quarantined_drift".to_string();
                        existing.quarantined_diff_json = quarantine_payload;
                        existing.revision += 1;
                        existing.updated_at = now_ms;
                        results.push(ReverseSyncPatternResult {
                            disposition: ReverseSyncDisposition::QuarantinedDrift,
                            pattern: existing,
                        });
                        continue;
                    }

                    let next_rev = existing.revision + 1;
                    let next_src_rev = existing.source_revision + 1;
                    tx.execute(
                        "UPDATE component_patterns \
                         SET component_name = ?1, source_path = ?2, ast_signature_sha256 = ?3, \
                             dedup_marker_sha256 = ?4, source_revision = ?5, \
                             last_sync_source = 'oxc_reverse_sync', tokens_snapshot_json = ?6, \
                             doc_markdown = ?7, drift_status = 'synced', quarantined_diff_json = '{}', \
                             revision = ?8, updated_at = ?9 \
                         WHERE id = ?10 AND tenant_id = ?11",
                        params![
                            sig.display_name,
                            sig.source_path,
                            sig.ast_signature_sha256,
                            dedup_marker,
                            next_src_rev,
                            tokens_snapshot_json,
                            doc_markdown,
                            next_rev,
                            now_ms,
                            existing.id,
                            req.tenant_id
                        ],
                    )?;

                    results.push(ReverseSyncPatternResult {
                        disposition: ReverseSyncDisposition::Synced,
                        pattern: ComponentPatternDto {
                            id: existing.id,
                            project_id: req.project_id.clone(),
                            portal_id: req.portal_id.clone(),
                            pattern_slug,
                            component_name: sig.display_name.clone(),
                            source_path: Some(sig.source_path.clone()),
                            ast_signature_sha256: sig.ast_signature_sha256.clone(),
                            dedup_marker_sha256: dedup_marker,
                            source_revision: next_src_rev,
                            last_sync_source: "oxc_reverse_sync".to_string(),
                            tokens_snapshot_json,
                            doc_markdown,
                            drift_status: "synced".to_string(),
                            quarantined_diff_json: "{}".to_string(),
                            revision: next_rev,
                            created_at: existing.created_at,
                            updated_at: now_ms,
                        },
                    });
                } else {
                    let new_id = Uuid::new_v4().to_string();
                    let (
                        drift_status,
                        quarantine_json,
                        disposition,
                        stored_ast_sha,
                        stored_dedup_marker,
                        stored_tokens,
                        stored_doc,
                    ) = if let Some(violation) = lock_violation {
                        let q = serde_json::json!({
                            "ruleCode": violation.rule_code,
                            "message": violation.message,
                            "lockedSlots": locked_slots_ref,
                            "incomingAstSha256": sig.ast_signature_sha256,
                            "incomingDedupMarkerSha256": dedup_marker,
                            "incomingTokensSnapshot": tokens_snapshot_json,
                            "callerActor": req.caller_actor,
                        })
                        .to_string();
                        (
                            "quarantined_drift".to_string(),
                            q,
                            ReverseSyncDisposition::QuarantinedDrift,
                            String::new(),
                            String::new(),
                            "{}".to_string(),
                            String::new(),
                        )
                    } else {
                        (
                            "synced".to_string(),
                            "{}".to_string(),
                            ReverseSyncDisposition::Synced,
                            sig.ast_signature_sha256.clone(),
                            dedup_marker,
                            tokens_snapshot_json,
                            doc_markdown,
                        )
                    };

                    tx.execute(
                        "INSERT INTO component_patterns \
                         (id, tenant_id, project_id, portal_id, pattern_slug, component_name, source_path, \
                          ast_signature_sha256, dedup_marker_sha256, source_revision, last_sync_source, \
                          tokens_snapshot_json, doc_markdown, drift_status, quarantined_diff_json, \
                          revision, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 'oxc_reverse_sync', ?10, ?11, ?12, ?13, 1, ?14, ?14)",
                        params![
                            new_id,
                            req.tenant_id,
                            req.project_id,
                            req.portal_id,
                            pattern_slug,
                            sig.display_name,
                            sig.source_path,
                            stored_ast_sha,
                            stored_dedup_marker,
                            stored_tokens,
                            stored_doc,
                            drift_status,
                            quarantine_json,
                            now_ms
                        ],
                    )?;

                    results.push(ReverseSyncPatternResult {
                        disposition,
                        pattern: ComponentPatternDto {
                            id: new_id,
                            project_id: req.project_id.clone(),
                            portal_id: req.portal_id.clone(),
                            pattern_slug,
                            component_name: sig.display_name.clone(),
                            source_path: Some(sig.source_path.clone()),
                            ast_signature_sha256: stored_ast_sha,
                            dedup_marker_sha256: stored_dedup_marker,
                            source_revision: 1,
                            last_sync_source: "oxc_reverse_sync".to_string(),
                            tokens_snapshot_json: stored_tokens,
                            doc_markdown: stored_doc,
                            drift_status,
                            quarantined_diff_json: quarantine_json,
                            revision: 1,
                            created_at: now_ms,
                            updated_at: now_ms,
                        },
                    });
                }
            }

            tx.commit()?;
            Ok(results)
        },
    )
}
