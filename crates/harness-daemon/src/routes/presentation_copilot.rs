// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04
use std::collections::HashMap;

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use presentation_cms::{
    is_valid_ff_id, is_valid_slide_uid, patch_slide_slots_lol_html, validate_chart_json_str,
    ChartKind2D, ChartSpecV12, CmsPatcherError, CmsSlotKind, SlotPatchInput,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::presentation_repo::{
    load_slide_slots_snapshot, SlotUpsertInput, WordingSlotV12,
};
use spec_registry::retry_on_busy;
use uuid::Uuid;

use crate::daemon_auth::VerifiedLegacyIngress;
use crate::routes::execution_coordinator_sql::{
    append_event_tx, ensure_chat_session_tx, promote_next_queued_tx, NewEventParams,
    MAX_GLOBAL_ACTIVE_SLOTS, SLIDE_COPILOT_LEASE_TTL_MS,
};
use crate::routes::presentation::{map_repo_err, resolve_deck_scope};
use crate::routes::presentation_storage::{
    check_and_reserve_scoped_idempotency, cleanup_staged_files, err_resp, finalize_idempotency,
    read_and_verify_slide_html, release_idempotency, stage_tx_owned_slide_html_file, ErrorDetails,
    IdemState,
};
use crate::routes::DaemonAppState;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentRegionRectBody {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructuredCommentBody {
    pub comment_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub slide_uid: Option<String>,
    #[serde(default)]
    pub target_type: Option<String>,
    pub element_ff_id: String,
    #[serde(default)]
    pub covered_ff_ids: Option<Vec<String>>,
    #[serde(default)]
    pub region_rect: Option<CommentRegionRectBody>,
    #[serde(default)]
    pub dom_hash: String,
    #[serde(default)]
    pub artifact_sha256: String,
    pub comment_body: String,
    #[serde(default)]
    pub target_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideCopilotDispatchBody {
    pub session_id: String,
    pub target_ff_id: String,
    pub prompt: String,
    pub expected_revision: i64,
    pub pre_flushed_sha256: String,
    #[serde(default)]
    pub snapshot_read_only: bool,
    #[serde(default)]
    pub requested_chart_kind: Option<ChartKind2D>,
    #[serde(default)]
    pub structured_comment: Option<StructuredCommentBody>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideCopilotCancelBody {
    pub job_id: String,
    pub expected_fencing_token: i64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidateChartBody {
    pub ff_id: String,
    pub spec: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideCopilotLeaseDto {
    pub job_id: String,
    pub slide_uid: String,
    pub target_ff_id: String,
    pub composite_scope_key: String,
    pub status: String,
    pub fencing_token: i64,
    pub queue_position: i64,
    pub lease_ttl_ms: i64,
    pub lease_expires_at_ms: Option<i64>,
    pub active_global_slots: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateChartResp {
    valid: bool,
    ff_id: String,
    kind: ChartKind2D,
    estimated_svg_primitives: usize,
    canonical_charts_json: String,
    aria_label: String,
    normalized_spec: ChartSpecV12,
}

#[derive(Debug, Clone)]
pub struct CopilotSlotMutation {
    pub ff_id: String,
    pub slot_kind: String,
    pub text_value: String,
    pub charts_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CopilotCommitOutcome {
    pub new_revision: i64,
    pub content_sha256: String,
    pub resolved_comment: bool,
    pub slots: Vec<WordingSlotV12>,
}

pub(crate) fn has_active_copilot_lease(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    deck_id: &str,
    slide_uid: &str,
    now_ms: i64,
) -> bool {
    let owner = format!("slide-copilot:{deck_id}:{slide_uid}");
    conn.query_row(
        "SELECT 1 FROM node_execution_jobs
         WHERE tenant_id = ?1 AND project_id = ?2 AND lease_owner = ?3
           AND status = 'RUNNING' AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?4)
         LIMIT 1",
        params![tenant_id, project_id, owner, now_ms],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .is_some()
}

fn require_editor_role(caller: &VerifiedLegacyIngress) -> Result<(), Response> {
    if matches!(caller.principal.role.as_str(), "editor" | "admin") {
        return Ok(());
    }
    Err(err_resp(
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
        "Insufficient role for Slide Copilot execution",
        false,
        None,
    ))
}

fn require_expected_project_from_headers(headers: &HeaderMap) -> Result<String, Response> {
    headers
        .get("x-cowok-project-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            err_resp(
                StatusCode::NOT_FOUND,
                "E_DECK_NOT_FOUND",
                "Requested presentation deck was not found",
                false,
                None,
            )
        })
}

fn replay_response(status_u16: u16, body_json: String) -> Response {
    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::OK);
    let parsed: serde_json::Value =
        serde_json::from_str(&body_json).unwrap_or_else(|_| serde_json::json!({}));
    (status, Json(parsed)).into_response()
}

fn is_valid_sha256_hex(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

struct VerifiedCommentBinding {
    comment_id: String,
    node_id: String,
    element_ff_id: String,
    dom_hash: String,
    artifact_sha256: String,
}

fn verify_structured_comment_tx(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    slide_uid: &str,
    target_ff_id: &str,
    sc: &StructuredCommentBody,
) -> Result<VerifiedCommentBinding, Response> {
    let cid = sc.comment_id.trim();
    if cid.is_empty() {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "structuredComment.commentId is required",
            false,
            None,
        ));
    }
    if sc.element_ff_id != target_ff_id {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "structuredComment.elementFfId must match targetFfId",
            false,
            None,
        ));
    }
    if let Some(ref sc_slide) = sc.slide_uid {
        if sc_slide.trim() != slide_uid {
            return Err(err_resp(
                StatusCode::NOT_FOUND,
                "E_COMMENT_NOT_FOUND",
                "Structured comment does not belong to target slide",
                false,
                None,
            ));
        }
    }

    let row: Option<(String, String, String, String, String, String)> = tx
        .query_row(
            "SELECT id, node_id, element_ff_id, dom_hash, artifact_sha256, anchor_status
             FROM element_comments
             WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
            params![cid, tenant_id, project_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Comment lookup failed: {e}"),
                true,
                None,
            )
        })?;

    let Some((db_id, db_node_id, db_ff_id, db_dom_hash, db_sha, db_status)) = row else {
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "E_COMMENT_NOT_FOUND",
            "Referenced element comment was not found",
            false,
            None,
        ));
    };

    if db_ff_id != sc.element_ff_id {
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "E_COMMENT_NOT_FOUND",
            "Comment element binding mismatch",
            false,
            None,
        ));
    }

    if db_node_id != slide_uid {
        let valid_flow_node: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM flow_nodes fn
                 JOIN projects p ON p.id = fn.project_id
                 WHERE fn.id = ?1 AND fn.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL",
                params![db_node_id, project_id, tenant_id],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None);
        if valid_flow_node.is_none() {
            return Err(err_resp(
                StatusCode::NOT_FOUND,
                "E_COMMENT_NOT_FOUND",
                "Comment scope mismatch",
                false,
                None,
            ));
        }
    }

    if db_status != "active"
        || (!sc.dom_hash.is_empty() && db_dom_hash != sc.dom_hash)
        || (!sc.artifact_sha256.is_empty() && db_sha != sc.artifact_sha256)
    {
        return Err(err_resp(
            StatusCode::CONFLICT,
            "E_COMMENT_ANCHOR_STALE",
            "Comment anchor hash or status is stale; re-anchor before dispatching Copilot",
            true,
            None,
        ));
    }

    Ok(VerifiedCommentBinding {
        comment_id: db_id,
        node_id: db_node_id,
        element_ff_id: db_ff_id,
        dom_hash: db_dom_hash,
        artifact_sha256: db_sha,
    })
}

pub async fn handle_dispatch_slide_copilot_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<SlideCopilotDispatchBody>,
) -> Response {
    handle_dispatch_slide_copilot_inner(
        state,
        caller,
        Some(project_id),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

pub async fn handle_dispatch_slide_copilot_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<SlideCopilotDispatchBody>,
) -> Response {
    let header_pid = match require_expected_project_from_headers(&headers) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };
    handle_dispatch_slide_copilot_inner(
        state,
        caller,
        Some(header_pid),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

async fn handle_dispatch_slide_copilot_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
    headers: HeaderMap,
    body: SlideCopilotDispatchBody,
) -> Response {
    if let Err(resp) = require_editor_role(&caller) {
        return resp;
    }
    if !is_valid_slide_uid(&slide_uid) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid slideUid format",
            false,
            None,
        );
    }
    if body.snapshot_read_only {
        return err_resp(
            StatusCode::FORBIDDEN,
            "E_SNAPSHOT_READ_ONLY",
            "Historical SNAP-* snapshot is immutable and cannot dispatch Slide Copilot",
            false,
            None,
        );
    }
    if body.session_id.trim().is_empty()
        || body.session_id.len() > 128
        || !is_valid_ff_id(&body.target_ff_id)
        || body.prompt.trim().is_empty()
        || body.prompt.chars().count() > 4_000
        || body.expected_revision < 0
        || !is_valid_sha256_hex(&body.pre_flushed_sha256)
    {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid Slide Copilot dispatch payload",
            false,
            None,
        );
    }

    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let body_canon = serde_json::to_string(&body).unwrap_or_default();
    let fingerprint = format!(
        "POST\n/slides/copilot/dispatch\n{}\n{}\n{}\n{}\n{}\n{}",
        scoped.tenant_id,
        caller.principal.user_id,
        scoped.project_id,
        deck_id,
        slide_uid,
        body_canon
    );
    let idem_key = match check_and_reserve_scoped_idempotency(
        &state,
        &scoped.tenant_id,
        &caller.principal.user_id,
        &headers,
        "copilot-dispatch",
        &scoped.project_id,
        &deck_id,
        &slide_uid,
        fingerprint.as_bytes(),
    ) {
        Ok(IdemState::None) => None,
        Ok(IdemState::Proceed(k)) => Some(k),
        Ok(IdemState::Replay(status, resp_json)) => return replay_response(status, resp_json),
        Err(resp) => return resp,
    };

    let release_idem = |st: &DaemonAppState| {
        if let Some(ref k) = idem_key {
            release_idempotency(st, &scoped.tenant_id, &caller.principal.user_id, k);
        }
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            release_idem(&state);
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let snap = match load_slide_slots_snapshot(
        &conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
    ) {
        Ok(s) => s,
        Err(err) => {
            release_idem(&state);
            return map_repo_err(err);
        }
    };

    if let Err(resp) = read_and_verify_slide_html(
        &scoped.workspace_root,
        &snap.slide.html_path,
        &snap.slide.content_sha256,
        &snap.slide.slide_uid,
    ) {
        release_idem(&state);
        return resp;
    }

    let now_ms = Utc::now().timestamp_millis();
    let session_id = body.session_id.trim().to_string();
    let lease_owner = format!("slide-copilot:{deck_id}:{slide_uid}");
    let scope_key = format!(
        "{}::{session_id}::{deck_id}::{slide_uid}",
        scoped.project_id
    );
    let nonce = state.auth.daemon_nonce.clone();
    let mut tx_err: Option<Response> = None;
    let mut out_dto: Option<SlideCopilotLeaseDto> = None;

    let retry_res = retry_on_busy("copilot_dispatch_tx", || {
        tx_err = None;
        out_dto = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let slide_meta: Option<(String, i64, i64, String)> = tx
            .query_row(
                "SELECT d.deck_mode, d.revision, s.revision, s.content_sha256
                 FROM presentation_decks d
                 JOIN deck_slides s ON s.deck_id = d.id
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1 AND d.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL AND s.slide_uid = ?4",
                params![deck_id, scoped.project_id, scoped.tenant_id, slide_uid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((deck_mode, deck_rev, slide_rev, committed_sha)) = slide_meta else {
            tx_err = Some(err_resp(
                StatusCode::NOT_FOUND,
                "E_DECK_NOT_FOUND",
                "Requested slide was not found",
                false,
                None,
            ));
            return Ok(());
        };
        let active_rev = if deck_mode == "one-page" {
            deck_rev
        } else {
            slide_rev
        };
        if active_rev != body.expected_revision || committed_sha != body.pre_flushed_sha256 {
            tx_err = Some(err_resp(
                StatusCode::CONFLICT,
                "E_PREFLUSH_CONFLICT",
                format!(
                    "Pre-flush CAS mismatch (expected rev {}, actual rev {})",
                    body.expected_revision, active_rev
                ),
                true,
                Some(ErrorDetails {
                    expected_revision: Some(body.expected_revision),
                    actual_revision: Some(active_rev),
                    slide_uid: Some(slide_uid.clone()),
                    duplicate_ff_id: None,
                }),
            ));
            return Ok(());
        }

        let verified_comment = if let Some(ref sc) = body.structured_comment {
            match verify_structured_comment_tx(
                &tx,
                &scoped.tenant_id,
                &scoped.project_id,
                &slide_uid,
                &body.target_ff_id,
                sc,
            ) {
                Ok(v) => Some(v),
                Err(r) => {
                    tx_err = Some(r);
                    return Ok(());
                }
            }
        } else {
            None
        };

        if let Err((status, code, msg)) = ensure_chat_session_tx(
            &tx,
            &scoped.tenant_id,
            &scoped.project_id,
            &session_id,
            now_ms,
        ) {
            tx_err = Some(err_resp(status, code, msg, false, None));
            return Ok(());
        }

        let scope_busy: i64 = tx.query_row(
            "SELECT COUNT(*) FROM node_execution_jobs
             WHERE tenant_id = ?1 AND project_id = ?2
               AND (composite_scope_key = ?3 OR lease_owner = ?4)
               AND status IN ('QUEUED', 'RUNNING', 'PAUSED')",
            params![scoped.tenant_id, scoped.project_id, scope_key, lease_owner],
            |r| r.get(0),
        )?;
        let active_global: i64 = tx.query_row(
            "SELECT COUNT(*) FROM node_execution_jobs
             WHERE tenant_id = ?1 AND status IN ('RUNNING', 'PAUSED')
               AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?2)",
            params![scoped.tenant_id, now_ms],
            |r| r.get(0),
        )?;
        let active_distinct_slides: i64 = tx.query_row(
            "SELECT COUNT(DISTINCT node_id) FROM node_execution_jobs
             WHERE tenant_id = ?1 AND status IN ('RUNNING', 'PAUSED')
               AND lease_owner LIKE 'slide-copilot:%'
               AND node_id LIKE 'SLD-%'
               AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?2)",
            params![scoped.tenant_id, now_ms],
            |r| r.get(0),
        )?;

        let can_run = active_global < MAX_GLOBAL_ACTIVE_SLOTS
            && active_distinct_slides < MAX_GLOBAL_ACTIVE_SLOTS
            && scope_busy == 0;
        let status = if can_run { "RUNNING" } else { "QUEUED" };
        let lease_expires_at = can_run.then_some(now_ms + SLIDE_COPILOT_LEASE_TTL_MS);
        let fence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(fencing_token), 0) + 1 FROM node_execution_jobs
             WHERE tenant_id = ?1 AND project_id = ?2
               AND (composite_scope_key = ?3 OR lease_owner = ?4)",
            params![scoped.tenant_id, scoped.project_id, scope_key, lease_owner],
            |r| r.get(0),
        )?;
        let job_id = format!("job-{}", Uuid::new_v4());
        tx.execute(
            "INSERT INTO node_execution_jobs
             (id, tenant_id, project_id, session_id, node_id, composite_scope_key, daemon_nonce, status, fencing_token, priority, lease_owner, stale_after_timestamp, created_at, started_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 10, ?10, ?11, ?12, ?13, ?12)",
            params![
                job_id,
                scoped.tenant_id,
                scoped.project_id,
                session_id,
                slide_uid,
                scope_key,
                nonce,
                status,
                fence,
                lease_owner,
                lease_expires_at,
                now_ms,
                can_run.then_some(now_ms),
            ],
        )?;

        let queue_position: i64 = if can_run {
            0
        } else {
            tx.query_row(
                "SELECT COUNT(*) FROM node_execution_jobs
                 WHERE tenant_id = ?1 AND project_id = ?2
                   AND (composite_scope_key = ?3 OR lease_owner = ?4) AND status = 'QUEUED'",
                params![scoped.tenant_id, scoped.project_id, scope_key, lease_owner],
                |r| r.get(0),
            )?
        };
        let total_active = active_global + if can_run { 1 } else { 0 };

        let ev_payload = serde_json::json!({
            "prompt": body.prompt,
            "targetFfId": body.target_ff_id,
            "slideUid": slide_uid,
            "deckId": deck_id,
            "expectedRevision": body.expected_revision,
            "requestedChartKind": body.requested_chart_kind,
            "commentId": body.structured_comment.as_ref().map(|c| c.comment_id.as_str()),
            "status": status,
            "fencingToken": fence,
            "leaseTtlMs": SLIDE_COPILOT_LEASE_TTL_MS,
            "leaseExpiresAtMs": lease_expires_at,
            "queuePosition": queue_position,
            "activeGlobalSlots": total_active,
            "verifiedComment": verified_comment.as_ref().map(|vc| serde_json::json!({
                "commentId": vc.comment_id,
                "nodeId": vc.node_id,
                "elementFfId": vc.element_ff_id,
                "domHash": vc.dom_hash,
                "artifactSha256": vc.artifact_sha256,
            })),
        })
        .to_string();

        append_event_tx(
            &tx,
            &NewEventParams {
                tid: &scoped.tenant_id,
                pid: &scoped.project_id,
                job_id: &job_id,
                node_id: &slide_uid,
                scope_key: &scope_key,
                kind: if can_run {
                    "lease_acquired"
                } else {
                    "enqueued"
                },
                payload: &ev_payload,
                now_ms,
            },
        )?;

        tx.commit()?;
        out_dto = Some(SlideCopilotLeaseDto {
            job_id,
            slide_uid: slide_uid.clone(),
            target_ff_id: body.target_ff_id.clone(),
            composite_scope_key: scope_key.clone(),
            status: status.to_string(),
            fencing_token: fence,
            queue_position,
            lease_ttl_ms: SLIDE_COPILOT_LEASE_TTL_MS,
            lease_expires_at_ms: lease_expires_at,
            active_global_slots: total_active,
        });
        Ok(())
    });

    if let Err(err) = retry_res {
        release_idem(&state);
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Copilot dispatch TX failed: {err}"),
            true,
            None,
        );
    }
    if let Some(resp) = tx_err {
        release_idem(&state);
        return resp;
    }
    let Some(dto) = out_dto else {
        release_idem(&state);
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Missing Copilot dispatch outcome",
            true,
            None,
        );
    };

    let _ = state.sse_tx.send(scoped.project_id.clone());
    if let Some(ref k) = idem_key {
        if let Ok(json_str) = serde_json::to_string(&dto) {
            finalize_idempotency(
                &state,
                &scoped.tenant_id,
                &caller.principal.user_id,
                k,
                201,
                &json_str,
            );
        }
    }
    (StatusCode::CREATED, Json(dto)).into_response()
}

pub async fn handle_cancel_slide_copilot_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<SlideCopilotCancelBody>,
) -> Response {
    handle_cancel_slide_copilot_inner(
        state,
        caller,
        Some(project_id),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

pub async fn handle_cancel_slide_copilot_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<SlideCopilotCancelBody>,
) -> Response {
    let header_pid = match require_expected_project_from_headers(&headers) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };
    handle_cancel_slide_copilot_inner(
        state,
        caller,
        Some(header_pid),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

async fn handle_cancel_slide_copilot_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
    headers: HeaderMap,
    body: SlideCopilotCancelBody,
) -> Response {
    if let Err(resp) = require_editor_role(&caller) {
        return resp;
    }
    if !is_valid_slide_uid(&slide_uid)
        || body.job_id.trim().is_empty()
        || body.expected_fencing_token <= 0
    {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid Copilot cancel payload",
            false,
            None,
        );
    }
    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let body_canon = serde_json::to_string(&body).unwrap_or_default();
    let fingerprint = format!(
        "POST\n/slides/copilot/cancel\n{}\n{}\n{}\n{}\n{}\n{}",
        scoped.tenant_id,
        caller.principal.user_id,
        scoped.project_id,
        deck_id,
        slide_uid,
        body_canon
    );
    let idem_key = match check_and_reserve_scoped_idempotency(
        &state,
        &scoped.tenant_id,
        &caller.principal.user_id,
        &headers,
        "copilot-cancel",
        &scoped.project_id,
        &deck_id,
        &slide_uid,
        fingerprint.as_bytes(),
    ) {
        Ok(IdemState::None) => None,
        Ok(IdemState::Proceed(k)) => Some(k),
        Ok(IdemState::Replay(status, resp_json)) => return replay_response(status, resp_json),
        Err(resp) => return resp,
    };
    let release_idem = |st: &DaemonAppState| {
        if let Some(ref k) = idem_key {
            release_idempotency(st, &scoped.tenant_id, &caller.principal.user_id, k);
        }
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            release_idem(&state);
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let now_ms = Utc::now().timestamp_millis();
    let jid = body.job_id.trim().to_string();
    let expected_owner = format!("slide-copilot:{deck_id}:{slide_uid}");
    let nonce = state.auth.daemon_nonce.clone();
    let mut tx_err: Option<Response> = None;
    let mut out_dto: Option<SlideCopilotLeaseDto> = None;

    let retry_res = retry_on_busy("copilot_cancel_tx", || {
        tx_err = None;
        out_dto = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row: Option<(String, String, i64, String, Option<String>)> = tx
            .query_row(
                "SELECT status, composite_scope_key, fencing_token, node_id, lease_owner
                 FROM node_execution_jobs
                 WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
                params![jid, scoped.tenant_id, scoped.project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((cur_status, scope_key, cur_fence, node_id, lease_owner)) = row else {
            tx_err = Some(err_resp(
                StatusCode::NOT_FOUND,
                "JOB_NOT_FOUND",
                "Slide Copilot job not found",
                false,
                None,
            ));
            return Ok(());
        };
        if node_id != slide_uid || lease_owner.as_deref() != Some(expected_owner.as_str()) {
            tx_err = Some(err_resp(
                StatusCode::NOT_FOUND,
                "JOB_NOT_FOUND",
                "Slide Copilot job does not belong to target slide",
                false,
                None,
            ));
            return Ok(());
        }
        if cur_fence != body.expected_fencing_token
            || !matches!(cur_status.as_str(), "QUEUED" | "RUNNING" | "PAUSED")
        {
            tx_err = Some(err_resp(
                StatusCode::CONFLICT,
                "STALE_FENCING_TOKEN",
                format!("Expected fencingToken {cur_fence} (status {cur_status})"),
                true,
                None,
            ));
            return Ok(());
        }
        let next_fence = cur_fence + 1;
        let updated = tx.execute(
            "UPDATE node_execution_jobs
             SET status = 'CANCELLED', fencing_token = ?1, stale_after_timestamp = NULL, completed_at = ?2, updated_at = ?2
             WHERE id = ?3 AND composite_scope_key = ?4 AND fencing_token = ?5 AND status = ?6",
            params![next_fence, now_ms, jid, scope_key, cur_fence, cur_status],
        )?;
        if updated == 0 {
            tx_err = Some(err_resp(
                StatusCode::CONFLICT,
                "STALE_FENCING_TOKEN",
                "Concurrent fencing token update",
                true,
                None,
            ));
            return Ok(());
        }
        let payload = serde_json::json!({
            "status": "CANCELLED",
            "fencingToken": next_fence,
            "reason": body.reason.as_deref().unwrap_or("user_cancelled"),
        })
        .to_string();
        append_event_tx(
            &tx,
            &NewEventParams {
                tid: &scoped.tenant_id,
                pid: &scoped.project_id,
                job_id: &jid,
                node_id: &slide_uid,
                scope_key: &scope_key,
                kind: "cancelled",
                payload: &payload,
                now_ms,
            },
        )?;
        let _ = promote_next_queued_tx(&tx, &nonce, now_ms)?;
        let active_global: i64 = tx.query_row(
            "SELECT COUNT(*) FROM node_execution_jobs
             WHERE tenant_id = ?1 AND status IN ('RUNNING', 'PAUSED')
               AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?2)",
            params![scoped.tenant_id, now_ms],
            |r| r.get(0),
        )?;
        let target_ff_id_opt: Option<String> = tx
            .query_row(
                "SELECT json_extract(payload_text, '$.targetFfId')
                 FROM execution_events
                 WHERE job_id = ?1 AND tenant_id = ?2 AND project_id = ?3
                   AND event_kind IN ('lease_acquired', 'enqueued')
                 ORDER BY seq_no ASC LIMIT 1",
                params![jid, scoped.tenant_id, scoped.project_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        tx.commit()?;
        out_dto = Some(SlideCopilotLeaseDto {
            job_id: jid.clone(),
            slide_uid: slide_uid.clone(),
            target_ff_id: target_ff_id_opt.unwrap_or_default(),
            composite_scope_key: scope_key,
            status: "CANCELLED".to_string(),
            fencing_token: next_fence,
            queue_position: 0,
            lease_ttl_ms: SLIDE_COPILOT_LEASE_TTL_MS,
            lease_expires_at_ms: None,
            active_global_slots: active_global,
        });
        Ok(())
    });

    if let Err(err) = retry_res {
        release_idem(&state);
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Copilot cancel TX failed: {err}"),
            true,
            None,
        );
    }
    if let Some(resp) = tx_err {
        release_idem(&state);
        return resp;
    }
    let Some(dto) = out_dto else {
        release_idem(&state);
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Missing cancel outcome",
            true,
            None,
        );
    };

    let _ = state.sse_tx.send(scoped.project_id.clone());
    if let Some(ref k) = idem_key {
        if let Ok(json_str) = serde_json::to_string(&dto) {
            finalize_idempotency(
                &state,
                &scoped.tenant_id,
                &caller.principal.user_id,
                k,
                200,
                &json_str,
            );
        }
    }
    (StatusCode::OK, Json(dto)).into_response()
}

pub async fn handle_validate_chart_spec_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
    Json(body): Json<ValidateChartBody>,
) -> Response {
    handle_validate_chart_spec_inner(state, caller, Some(project_id), deck_id, slide_uid, body)
        .await
}

pub async fn handle_validate_chart_spec_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    headers: HeaderMap,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
    Json(body): Json<ValidateChartBody>,
) -> Response {
    let header_pid = match require_expected_project_from_headers(&headers) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };
    handle_validate_chart_spec_inner(state, caller, Some(header_pid), deck_id, slide_uid, body)
        .await
}

async fn handle_validate_chart_spec_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
    body: ValidateChartBody,
) -> Response {
    if !is_valid_slide_uid(&slide_uid) || !is_valid_ff_id(&body.ff_id) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid slideUid or ffId format",
            false,
            None,
        );
    }
    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            )
        }
    };
    if let Err(err) = load_slide_slots_snapshot(
        &conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
    ) {
        return map_repo_err(err);
    }

    let raw_spec = serde_json::to_string(&body.spec).unwrap_or_default();
    match validate_chart_json_str(&raw_spec) {
        Ok(valid) => (
            StatusCode::OK,
            Json(ValidateChartResp {
                valid: true,
                ff_id: body.ff_id,
                kind: valid.normalized_spec.kind,
                estimated_svg_primitives: valid.estimated_svg_primitives,
                canonical_charts_json: valid.canonical_charts_json,
                aria_label: valid.aria_label,
                normalized_spec: valid.normalized_spec,
            }),
        )
            .into_response(),
        Err(err) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            err.code(),
            err.to_string(),
            true,
            None,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn mark_copilot_job_failed(
    conn: &mut Connection,
    tenant_id: &str,
    project_id: &str,
    slide_uid: &str,
    job_id: &str,
    target_ff_id: &str,
    expected_fencing_token: i64,
    nonce: &str,
    error_code: &str,
    error_message: &str,
) {
    let now_ms = Utc::now().timestamp_millis();
    let _ = retry_on_busy("mark_copilot_failed", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_opt: Option<String> = tx
            .query_row(
                "SELECT composite_scope_key FROM node_execution_jobs WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3 AND fencing_token = ?4",
                params![job_id, tenant_id, project_id, expected_fencing_token],
                |r| r.get(0),
            )
            .optional()?;
        let Some(scope_key) = scope_opt else {
            return Ok(());
        };
        let err_json = serde_json::json!({
            "code": error_code,
            "message": error_message,
            "targetFfId": target_ff_id,
        })
        .to_string();
        tx.execute(
            "UPDATE node_execution_jobs
             SET status = 'FAILED', fencing_token = fencing_token + 1, stale_after_timestamp = NULL, error_json = ?1, completed_at = ?2, updated_at = ?2
             WHERE id = ?3 AND fencing_token = ?4",
            params![err_json, now_ms, job_id, expected_fencing_token],
        )?;
        append_event_tx(
            &tx,
            &NewEventParams {
                tid: tenant_id,
                pid: project_id,
                job_id,
                node_id: slide_uid,
                scope_key: &scope_key,
                kind: "chart.validation.rejected",
                payload: &err_json,
                now_ms,
            },
        )?;
        let _ = promote_next_queued_tx(&tx, nonce, now_ms)?;
        tx.commit()
    });
}

#[allow(clippy::too_many_arguments)]
pub fn commit_slide_copilot_turn(
    state: &DaemonAppState,
    tenant_id: &str,
    project_id: &str,
    deck_id: &str,
    slide_uid: &str,
    job_id: &str,
    expected_fencing_token: i64,
    expected_revision: i64,
    mutations: &[CopilotSlotMutation],
    comment_id: Option<&str>,
) -> Result<CopilotCommitOutcome, String> {
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(|e| format!("DB unavailable: {e}"))?;
    let ws_raw: String = conn
        .query_row(
            "SELECT workspace_root FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("Project lookup failed: {e}"))?;
    let trimmed_ws = ws_raw.trim();
    let ws_root = if trimmed_ws.is_empty() || trimmed_ws == "." {
        state.registry.pool().workspace_root().to_path_buf()
    } else {
        std::path::PathBuf::from(trimmed_ws)
    };

    let snap = load_slide_slots_snapshot(&conn, project_id, tenant_id, deck_id, slide_uid)
        .map_err(|e| format!("Load snapshot failed: {e:?}"))?;
    let current_html = read_and_verify_slide_html(
        &ws_root,
        &snap.slide.html_path,
        &snap.slide.content_sha256,
        &snap.slide.slide_uid,
    )
    .map_err(|_| "E_DECK_ARTIFACT_MISSING".to_string())?;

    let mut patch_inputs = Vec::with_capacity(mutations.len());
    for m in mutations {
        let kind = CmsSlotKind::from_str_checked(&m.slot_kind).map_err(|e| {
            mark_copilot_job_failed(
                &mut conn,
                tenant_id,
                project_id,
                slide_uid,
                job_id,
                &m.ff_id,
                expected_fencing_token,
                &state.auth.daemon_nonce,
                "VALIDATION_ERROR",
                &e.to_string(),
            );
            let _ = state.sse_tx.send(project_id.to_string());
            e.to_string()
        })?;
        patch_inputs.push(SlotPatchInput {
            ff_id: m.ff_id.clone(),
            slot_kind: kind,
            text_value: m.text_value.clone(),
            bullet_items: None,
            charts_json: m.charts_json.clone(),
            safe_styles_json: None,
        });
    }

    let is_one_page = snap.deck_mode == "one-page";
    let scope_uid = if is_one_page { Some(slide_uid) } else { None };
    let patch_out =
        patch_slide_slots_lol_html(&current_html, scope_uid, &patch_inputs).map_err(|e| {
            let code = match &e {
                CmsPatcherError::ChartValidationFailed { code, .. } => *code,
                _ => "VALIDATION_ERROR",
            };
            let msg = e.to_string();
            let failed_ff = patch_inputs.first().map(|p| p.ff_id.as_str()).unwrap_or("");
            mark_copilot_job_failed(
                &mut conn,
                tenant_id,
                project_id,
                slide_uid,
                job_id,
                failed_ff,
                expected_fencing_token,
                &state.auth.daemon_nonce,
                code,
                &msg,
            );
            let _ = state.sse_tx.send(project_id.to_string());
            msg
        })?;

    let tx_uuid = Uuid::new_v4().to_string();
    let next_rev = expected_revision + 1;
    let staged_rel_path = stage_tx_owned_slide_html_file(
        &ws_root,
        deck_id,
        slide_uid,
        is_one_page,
        next_rev,
        &tx_uuid,
        &patch_out.patched_html,
    )
    .map_err(|_| "E_FS_STAGE_FAILED".to_string())?;

    let existing_by_ff: HashMap<&str, &WordingSlotV12> =
        snap.slots.iter().map(|s| (s.ff_id.as_str(), s)).collect();
    let slot_upserts: Vec<SlotUpsertInput> = patch_inputs
        .iter()
        .map(|p| {
            let existing = existing_by_ff.get(p.ff_id.as_str()).copied();
            SlotUpsertInput {
                ff_id: p.ff_id.clone(),
                slot_label: existing.map(|s| s.slot_label.clone()).unwrap_or_default(),
                slot_kind: p.slot_kind.as_str().to_string(),
                text_value: p.text_value.clone(),
                charts_json: p.charts_json.clone().unwrap_or_else(|| {
                    existing
                        .map(|s| s.charts_json.clone())
                        .unwrap_or_else(|| "[]".to_string())
                }),
                safe_styles_json: existing
                    .map(|s| s.safe_styles_json.clone())
                    .unwrap_or_else(|| "{}".to_string()),
                updated_by: "copilot".to_string(),
            }
        })
        .collect();

    let now_ms = Utc::now().timestamp_millis();
    let expected_owner = format!("slide-copilot:{deck_id}:{slide_uid}");
    let nonce = state.auth.daemon_nonce.clone();
    let mut tx_err: Option<String> = None;
    let mut outcome: Option<CopilotCommitOutcome> = None;

    let commit_res = retry_on_busy("copilot_commit_tx", || {
        tx_err = None;
        outcome = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let job_row: Option<(String, String, i64, Option<String>)> = tx
            .query_row(
                "SELECT status, composite_scope_key, fencing_token, lease_owner
                 FROM node_execution_jobs
                 WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
                params![job_id, tenant_id, project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((job_status, scope_key, job_fence, lease_owner)) = job_row else {
            tx_err = Some("JOB_NOT_FOUND".to_string());
            return Ok(());
        };
        if job_status != "RUNNING"
            || job_fence != expected_fencing_token
            || lease_owner.as_deref() != Some(expected_owner.as_str())
        {
            tx_err = Some("STALE_FENCING_TOKEN".to_string());
            return Ok(());
        }

        if is_one_page {
            let updated_deck = tx.execute(
                "UPDATE presentation_decks SET revision = revision + 1, updated_at = ?1 WHERE id = ?2 AND revision = ?3",
                params![now_ms, deck_id, expected_revision],
            )?;
            if updated_deck != 1 {
                tx_err = Some("E_STALE_REVISION".to_string());
                return Ok(());
            }
            tx.execute(
                "UPDATE deck_slides SET html_path = ?1, content_sha256 = ?2, revision = ?3, updated_at = ?4 WHERE deck_id = ?5",
                params![staged_rel_path, patch_out.content_sha256, next_rev, now_ms, deck_id],
            )?;
        } else {
            let updated_slide = tx.execute(
                "UPDATE deck_slides SET html_path = ?1, content_sha256 = ?2, revision = revision + 1, updated_at = ?3 WHERE id = ?4 AND revision = ?5",
                params![staged_rel_path, patch_out.content_sha256, now_ms, snap.slide.id, expected_revision],
            )?;
            if updated_slide != 1 {
                tx_err = Some("E_STALE_REVISION".to_string());
                return Ok(());
            }
        }

        for slot in &slot_upserts {
            tx.execute(
                "INSERT INTO wording_slots
                 (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'copilot', 1, ?9)
                 ON CONFLICT(slide_id, ff_id) DO UPDATE SET
                   slot_label = CASE WHEN excluded.slot_label != '' THEN excluded.slot_label ELSE wording_slots.slot_label END,
                   slot_kind = excluded.slot_kind,
                   text_value = excluded.text_value,
                   charts_json = excluded.charts_json,
                   safe_styles_json = excluded.safe_styles_json,
                   updated_by = 'copilot',
                   ast_revision = wording_slots.ast_revision + 1,
                   updated_at = excluded.updated_at",
                params![
                    Uuid::new_v4().to_string(),
                    snap.slide.id,
                    slot.ff_id,
                    slot.slot_label,
                    slot.slot_kind,
                    slot.text_value,
                    slot.charts_json,
                    slot.safe_styles_json,
                    now_ms,
                ],
            )?;
        }

        let mut resolved_comment = false;
        if let Some(cid) = comment_id {
            let comment_meta: Option<(String, String, String, String)> = tx
                .query_row(
                    "SELECT node_id, element_ff_id, dom_hash, artifact_sha256 FROM element_comments WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3",
                    params![cid, tenant_id, project_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            if let Some((c_nid, c_ff, c_dom, c_sha)) = comment_meta {
                let changes = tx.execute(
                    "UPDATE element_comments SET anchor_status = 'resolved', updated_at = ?1
                     WHERE id = ?2 AND tenant_id = ?3 AND project_id = ?4 AND node_id = ?5 AND element_ff_id = ?6 AND dom_hash = ?7 AND artifact_sha256 = ?8 AND anchor_status = 'active'",
                    params![now_ms, cid, tenant_id, project_id, c_nid, c_ff, c_dom, c_sha],
                )?;
                resolved_comment = changes == 1;
            }
        }

        let next_fence = expected_fencing_token + 1;
        let job_upd = tx.execute(
            "UPDATE node_execution_jobs
             SET status = 'COMPLETED', fencing_token = ?1, stale_after_timestamp = NULL, completed_at = ?2, updated_at = ?2
             WHERE id = ?3 AND fencing_token = ?4 AND status = 'RUNNING'",
            params![next_fence, now_ms, job_id, expected_fencing_token],
        )?;
        if job_upd != 1 {
            tx_err = Some("STALE_FENCING_TOKEN".to_string());
            return Ok(());
        }

        let fresh_snap = load_slide_slots_snapshot(&tx, project_id, tenant_id, deck_id, slide_uid)
            .map_err(|_| rusqlite::Error::QueryReturnedNoRows)?;
        let ev_payload = serde_json::json!({
            "jobId": job_id,
            "slideUid": slide_uid,
            "newRevision": next_rev,
            "contentSha256": patch_out.content_sha256,
            "sanitizedHtml": patch_out.patched_html,
            "slots": fresh_snap.slots,
            "resolvedCommentId": if resolved_comment { comment_id } else { None },
            "fencingToken": next_fence,
        })
        .to_string();
        append_event_tx(
            &tx,
            &NewEventParams {
                tid: tenant_id,
                pid: project_id,
                job_id,
                node_id: slide_uid,
                scope_key: &scope_key,
                kind: "completed",
                payload: &ev_payload,
                now_ms,
            },
        )?;
        let _ = promote_next_queued_tx(&tx, &nonce, now_ms)?;
        tx.commit()?;
        outcome = Some(CopilotCommitOutcome {
            new_revision: next_rev,
            content_sha256: patch_out.content_sha256.clone(),
            resolved_comment,
            slots: fresh_snap.slots,
        });
        Ok(())
    });

    let (Some(result), None) = (outcome, tx_err.as_ref()) else {
        cleanup_staged_files(&ws_root, &[staged_rel_path]);
        return Err(tx_err.unwrap_or_else(|| "COPILOT_COMMIT_FAILED".to_string()));
    };
    if commit_res.is_err() {
        cleanup_staged_files(&ws_root, &[staged_rel_path]);
        return Err("COPILOT_COMMIT_FAILED".to_string());
    }
    let _ = state.sse_tx.send(project_id.to_string());
    Ok(result)
}

fn extract_prompt_numeric_values(prompt: &str) -> Vec<f64> {
    let mut vals = Vec::new();
    for raw_tok in prompt.split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-') {
        if raw_tok.is_empty() || raw_tok == "-" || raw_tok == "." {
            continue;
        }
        if let Ok(num) = raw_tok.parse::<f64>() {
            if num.is_finite() {
                vals.push(num);
                if vals.len() >= 12 {
                    break;
                }
            }
        }
    }
    vals
}

fn build_copilot_worker_chart_json(
    prompt: &str,
    requested_kind: Option<ChartKind2D>,
    existing_charts_json: Option<&str>,
) -> String {
    let extracted = extract_prompt_numeric_values(prompt);
    let kind_str: String = match requested_kind {
        Some(ChartKind2D::Pie) => "pie".to_string(),
        Some(ChartKind2D::Donut) => "donut".to_string(),
        Some(ChartKind2D::OneHundredStacked) => "100_stacked".to_string(),
        Some(ChartKind2D::Bar) => "bar".to_string(),
        Some(ChartKind2D::Line) => "line".to_string(),
        Some(ChartKind2D::Area) => "area".to_string(),
        Some(ChartKind2D::Funnel) => "funnel".to_string(),
        None => {
            if let Some(raw) = existing_charts_json {
                serde_json::from_str::<serde_json::Value>(raw)
                    .ok()
                    .and_then(|v| {
                        v.get("kind")
                            .and_then(|k| k.as_str())
                            .map(ToString::to_string)
                    })
                    .unwrap_or_else(|| "bar".to_string())
            } else {
                "bar".to_string()
            }
        }
    };
    let is_proportional = matches!(
        kind_str.as_str(),
        "pie" | "donut" | "100_stacked" | "funnel"
    );
    let color_cycle = [
        "human-accent",
        "agent-accent",
        "state-success",
        "state-info",
    ];
    if is_proportional {
        let points: Vec<serde_json::Value> = if extracted.len() >= 2 {
            extracted
                .iter()
                .enumerate()
                .map(|(i, val)| {
                    serde_json::json!({
                        "label": format!("Segment {}", i + 1),
                        "value": val,
                        "colorToken": color_cycle[i % color_cycle.len()],
                    })
                })
                .collect()
        } else {
            vec![
                serde_json::json!({ "label": "Primary", "value": 60.0, "colorToken": "human-accent" }),
                serde_json::json!({ "label": "Secondary", "value": 40.0, "colorToken": "agent-accent" }),
            ]
        };
        serde_json::json!({
            "kind": kind_str,
            "title": prompt.chars().take(64).collect::<String>(),
            "unit": "%",
            "series": [{
                "name": "Distribution",
                "points": points
            }]
        })
        .to_string()
    } else {
        let points: Vec<serde_json::Value> = if extracted.len() >= 2 {
            extracted
                .iter()
                .enumerate()
                .map(|(i, val)| {
                    serde_json::json!({
                        "label": format!("P{}", i + 1),
                        "value": val,
                        "colorToken": color_cycle[i % color_cycle.len()],
                    })
                })
                .collect()
        } else {
            vec![
                serde_json::json!({ "label": "Q1", "value": 34.0, "colorToken": "human-accent" }),
                serde_json::json!({ "label": "Q2", "value": 52.0, "colorToken": "agent-accent" }),
                serde_json::json!({ "label": "Q3", "value": 68.0, "colorToken": "state-success" }),
            ]
        };
        serde_json::json!({
            "kind": kind_str,
            "title": prompt.chars().take(64).collect::<String>(),
            "series": [{
                "name": "Series 1",
                "points": points
            }]
        })
        .to_string()
    }
}

fn execute_single_copilot_job(state: &DaemonAppState, job_id: &str) -> Result<(), String> {
    let conn = state
        .registry
        .pool()
        .connection()
        .map_err(|e| format!("{e}"))?;
    let row: Option<(String, String, String, i64, Option<String>)> = conn
        .query_row(
            "SELECT tenant_id, project_id, node_id, fencing_token, lease_owner
             FROM node_execution_jobs
             WHERE id = ?1 AND status = 'RUNNING' AND lease_owner LIKE 'slide-copilot:%'",
            params![job_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| format!("{e}"))?;
    let Some((tenant_id, project_id, slide_uid, fence, lease_owner_opt)) = row else {
        return Ok(());
    };
    let Some(lease_owner) = lease_owner_opt else {
        return Ok(());
    };
    let rest = lease_owner.strip_prefix("slide-copilot:").unwrap_or("");
    let Some((deck_id, owner_uid)) = rest.split_once(':') else {
        return Ok(());
    };
    if owner_uid != slide_uid {
        return Ok(());
    }
    drop(conn);
    let res = execute_single_copilot_job_inner(
        state,
        &tenant_id,
        &project_id,
        deck_id,
        &slide_uid,
        job_id,
        fence,
    );
    if let Err(ref err_msg) = res {
        if let Ok(mut fail_conn) = state.registry.pool().connection() {
            mark_copilot_job_failed(
                &mut fail_conn,
                &tenant_id,
                &project_id,
                &slide_uid,
                job_id,
                "",
                fence,
                &state.auth.daemon_nonce,
                "COPILOT_WORKER_FAILED",
                err_msg,
            );
            let _ = state.sse_tx.send(project_id);
        }
    }
    res
}

#[allow(clippy::too_many_arguments)]
fn execute_single_copilot_job_inner(
    state: &DaemonAppState,
    tenant_id: &str,
    project_id: &str,
    deck_id: &str,
    slide_uid: &str,
    job_id: &str,
    fence: i64,
) -> Result<(), String> {
    let conn = state
        .registry
        .pool()
        .connection()
        .map_err(|e| format!("{e}"))?;
    let init_payload: Option<String> = conn
        .query_row(
            "SELECT payload_text FROM execution_events
             WHERE job_id = ?1 AND event_kind IN ('lease_acquired', 'enqueued')
             ORDER BY seq_no ASC LIMIT 1",
            params![job_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("{e}"))?;
    let parsed_init: serde_json::Value = init_payload
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let prompt = parsed_init
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("Update slide slot")
        .trim();
    let comment_id = parsed_init
        .get("commentId")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let requested_kind: Option<ChartKind2D> = parsed_init
        .get("requestedChartKind")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let snap = load_slide_slots_snapshot(&conn, project_id, tenant_id, deck_id, slide_uid)
        .map_err(|e| format!("Load snap failed: {e:?}"))?;
    drop(conn);
    let effective_rev = if snap.deck_mode == "one-page" {
        snap.deck_revision
    } else {
        snap.slide.revision
    };
    let raw_target_ff = parsed_init
        .get("targetFfId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let existing_slot = snap
        .slots
        .iter()
        .find(|s| s.ff_id == raw_target_ff)
        .or_else(|| {
            if is_valid_ff_id(raw_target_ff) {
                None
            } else {
                snap.slots.first()
            }
        });
    let target_ff_id = if is_valid_ff_id(raw_target_ff) {
        raw_target_ff.to_string()
    } else {
        existing_slot.map(|s| s.ff_id.clone()).unwrap_or_default()
    };
    if !is_valid_ff_id(&target_ff_id) {
        return Err("INVALID_TARGET_FF_ID".to_string());
    }
    let slot_kind =
        if requested_kind.is_some() || existing_slot.is_some_and(|s| s.slot_kind == "chart_json") {
            "chart_json".to_string()
        } else {
            existing_slot
                .map(|s| s.slot_kind.clone())
                .unwrap_or_else(|| "text".to_string())
        };
    let charts_json = if slot_kind == "chart_json" {
        Some(build_copilot_worker_chart_json(
            prompt,
            requested_kind,
            existing_slot.map(|s| s.charts_json.as_str()),
        ))
    } else {
        None
    };
    let text_value = if slot_kind == "chart_json" {
        existing_slot
            .map(|s| s.text_value.clone())
            .unwrap_or_default()
    } else {
        prompt.chars().take(160).collect::<String>()
    };

    commit_slide_copilot_turn(
        state,
        tenant_id,
        project_id,
        deck_id,
        slide_uid,
        job_id,
        fence,
        effective_rev,
        &[CopilotSlotMutation {
            ff_id: target_ff_id,
            slot_kind,
            text_value,
            charts_json,
        }],
        comment_id.as_deref(),
    )?;
    Ok(())
}

/// Processes active `RUNNING` Slide Copilot lease jobs asynchronously in the background worker loop.
pub async fn run_pending_slide_copilot_jobs(state: &DaemonAppState) -> usize {
    let pool = state.registry.pool().clone();
    let pending_ids: Vec<String> = tokio::task::spawn_blocking(move || {
        let Ok(conn) = pool.connection() else {
            return Vec::new();
        };
        let now_ms = Utc::now().timestamp_millis();
        let mut stmt = match conn.prepare(
            "SELECT id FROM node_execution_jobs
             WHERE status = 'RUNNING'
               AND lease_owner LIKE 'slide-copilot:%'
               AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?1)
             ORDER BY created_at ASC LIMIT 4",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![now_ms], |r| r.get::<_, String>(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let mut processed = 0usize;
    for jid in pending_ids {
        let st = state.clone();
        let res = tokio::task::spawn_blocking(move || execute_single_copilot_job(&st, &jid)).await;
        if let Ok(Ok(())) = res {
            processed += 1;
        }
    }
    processed
}
