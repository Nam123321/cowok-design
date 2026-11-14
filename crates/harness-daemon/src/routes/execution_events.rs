// @story 04.4
// @implements AC-01, AC-02, AC-04 (Hardened Execution Job Events CAS & Persistent SSE Replay)
use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::retry_on_busy;
use uuid::Uuid;

use crate::daemon_auth::FlowPrincipal;
use crate::routes::execution_coordinator_sql::{
    authorize_project, promote_next_queued_tx, LEASE_TTL_MS, MAX_GLOBAL_ACTIVE_SLOTS,
};
use crate::routes::execution_reaper::reap_job_worker_and_shadow;
use crate::routes::legacy_workbench_v6::{
    check_non_empty_id, epoch_ms_to_iso, json_err, map_harness_resp, run_db,
};
use crate::routes::DaemonAppState;

pub use super::execution_stream::handle_stream_job_events;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEventDto {
    pub id: String,
    pub job_id: String,
    pub project_id: String,
    pub node_id: String,
    pub event_kind: String,
    pub payload_text: String,
    pub seq_no: i64,
    pub project_seq: i64,
    pub composite_scope_key: String,
    pub job_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostJobEventInput {
    pub job_id: String,
    pub node_id: String,
    pub session_id: Option<String>,
    pub event_kind: String,
    #[serde(default)]
    pub payload_text: String,
    pub job_status: Option<String>,
    #[serde(default)]
    pub expected_fencing_token: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamJobEventsQuery {
    pub job_id: Option<String>,
    pub node_id: Option<String>,
    pub after_seq: Option<i64>,
}

fn validate_job_status(raw: Option<&str>) -> Result<Option<String>, Response> {
    match raw {
        Some(st @ ("QUEUED" | "RUNNING" | "PAUSED" | "COMPLETED" | "FAILED" | "CANCELLED")) => {
            Ok(Some(st.to_string()))
        }
        Some(_) => Err(json_err(StatusCode::CONFLICT, "STALE_FENCING_TOKEN", "Invalid or protected jobStatus transition")),
        None => Ok(None),
    }
}

struct EventTxInput<'a> {
    tid: &'a str,
    pid: &'a str,
    job_id: &'a str,
    node_id: &'a str,
    req_sid: Option<&'a str>,
    event_kind: &'a str,
    payload_text: &'a str,
    next_status: Option<&'a str>,
    expected_token: Option<i64>,
    nonce: &'a str,
    event_id: &'a str,
    now_ms: i64,
}

type EventErr = (StatusCode, &'static str, String);
type EventPreconditions = (String, i64, String, String);
type EventApplied = (i64, i64, String, String, Vec<(String, String)>);

fn validate_event_preconditions(
    tx: &rusqlite::Transaction<'_>,
    inp: &EventTxInput<'_>,
) -> Result<EventPreconditions, EventErr> {
    let row: Option<(String, String, String, i64, String, String)> = tx
        .query_row(
            "SELECT project_id, tenant_id, status, fencing_token, composite_scope_key, node_id FROM node_execution_jobs WHERE id = ?1",
            params![inp.job_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    let Some((j_pid, j_tid, cur, fence, scope, j_nid)) = row else {
        return Err((StatusCode::NOT_FOUND, "JOB_NOT_FOUND", format!("Job '{}' must be enqueued via /flow/jobs/enqueue", inp.job_id)));
    };
    if j_tid != inp.tid { return Err((StatusCode::FORBIDDEN, "TENANT_ISOLATION_BREACH", "Job belongs to another tenant".into())); }
    if j_pid != inp.pid { return Err((StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "Job belongs to another project".into())); }
    if j_nid != inp.node_id { return Err((StatusCode::CONFLICT, "SCOPE_MISMATCH", "Job nodeId mismatch".into())); }
    if let Some(sid) = inp.req_sid {
        let exp_scope = format!("{}::{sid}::{}", inp.pid, inp.node_id);
        if !scope.is_empty() && scope != exp_scope {
            return Err((StatusCode::CONFLICT, "SCOPE_MISMATCH", "Job composite_scope_key mismatch".into()));
        }
    }
    if matches!(cur.as_str(), "COMPLETED" | "FAILED" | "CANCELLED" | "INTERRUPTED") {
        return Err((StatusCode::CONFLICT, "STALE_FENCING_TOKEN", format!("Cannot mutate job in status {cur}")));
    }
    if inp.expected_token != Some(fence) {
        return Err((StatusCode::CONFLICT, "STALE_FENCING_TOKEN", format!("Expected fencingToken {fence}, got {:?}", inp.expected_token)));
    }
    let eff_scope = if scope.is_empty() { format!("{}::sess-{}::{}", inp.pid, inp.pid, inp.node_id) } else { scope };
    let target = inp.next_status.unwrap_or(&cur).to_string();
    Ok((cur, fence, eff_scope, target))
}

fn apply_job_event_tx(
    tx: &rusqlite::Transaction<'_>,
    inp: &EventTxInput<'_>,
) -> Result<EventApplied, EventErr> {
    let map_db = |e: rusqlite::Error| (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string());
    let (cur, cur_fence, eff_scope, target) = validate_event_preconditions(tx, inp)?;
    if cur == "QUEUED" && matches!(target.as_str(), "RUNNING" | "PAUSED") {
        let active: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED')", [], |r| r.get(0)).map_err(map_db)?;
        let busy: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE composite_scope_key = ?1 AND status IN ('RUNNING', 'PAUSED')", params![eff_scope], |r| r.get(0)).map_err(map_db)?;
        if active >= MAX_GLOBAL_ACTIVE_SLOTS || busy > 0 {
            return Err((StatusCode::CONFLICT, "SLOT_LIMIT_REACHED", "Global active slot cap or scope concurrency reached".into()));
        }
    }
    let updated = tx.execute(
        "UPDATE node_execution_jobs SET status = ?1, fencing_token = fencing_token + 1, daemon_nonce = ?2, started_at = CASE WHEN ?1 = 'RUNNING' THEN COALESCE(started_at, ?3) ELSE started_at END, stale_after_timestamp = CASE WHEN ?1 = 'RUNNING' THEN ?4 ELSE NULL END, completed_at = CASE WHEN ?1 IN ('COMPLETED', 'FAILED', 'CANCELLED') THEN ?3 ELSE completed_at END, updated_at = ?3
         WHERE id = ?5 AND project_id = ?6 AND tenant_id = ?7 AND composite_scope_key = ?8 AND fencing_token = ?9 AND status = ?10",
        params![target, inp.nonce, inp.now_ms, inp.now_ms + LEASE_TTL_MS, inp.job_id, inp.pid, inp.tid, eff_scope, cur_fence, cur],
    ).map_err(map_db)?;
    if updated == 0 {
        return Err((StatusCode::CONFLICT, "STALE_FENCING_TOKEN", "Concurrent fencing token update detected".into()));
    }
    let seq_no: i64 = tx.query_row("SELECT COALESCE(MAX(seq_no), 0) + 1 FROM execution_events WHERE job_id = ?1", params![inp.job_id], |r| r.get(0)).map_err(map_db)?;
    let p_seq: i64 = tx.query_row("SELECT COALESCE(MAX(project_seq), 0) + 1 FROM execution_events WHERE project_id = ?1", params![inp.pid], |r| r.get(0)).map_err(map_db)?;
    tx.execute(
        "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, composite_scope_key, event_kind, payload_text, seq_no, project_seq, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![inp.event_id, inp.tid, inp.pid, inp.job_id, inp.node_id, eff_scope, inp.event_kind, inp.payload_text, seq_no, p_seq, inp.now_ms],
    ).map_err(map_db)?;
    let bump_unread: i64 = if matches!(target.as_str(), "COMPLETED" | "FAILED") || matches!(inp.event_kind, "chunk" | "completed" | "STEP_COMPLETED" | "steered" | "failed") { 1 } else { 0 };
    let _ = tx.execute("UPDATE node_chat_sessions SET last_message_id = ?1, unread_count = unread_count + ?2, updated_at = ?3 WHERE composite_scope_key = ?4", params![inp.event_id, bump_unread, inp.now_ms, eff_scope]);
    let promoted = if matches!(target.as_str(), "COMPLETED" | "FAILED" | "CANCELLED") { promote_next_queued_tx(tx, inp.nonce, inp.now_ms).map_err(map_db)? } else { Vec::new() };
    Ok((seq_no, p_seq, eff_scope, target, promoted))
}

pub async fn handle_post_job_event(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<PostJobEventInput>,
) -> Result<(StatusCode, Json<ExecutionEventDto>), Response> {
    let (pid, root) = authorize_project(&state, &id, &headers, &principal)?;
    let job_id = check_non_empty_id(&req.job_id, "jobId")?;
    let node_id = check_non_empty_id(&req.node_id, "nodeId")?;
    let event_kind = check_non_empty_id(&req.event_kind, "eventKind")?;
    let next_status = validate_job_status(req.job_status.as_deref())?;
    let req_sid = req.session_id.filter(|s| !s.trim().is_empty());
    let (pool, workers, nonce, sse_tx) = (state.registry.pool().clone(), state.active_workers.clone(), state.auth.daemon_nonce.clone(), state.sse_tx.clone());
    let (created, promoted) = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let now_ms = Utc::now().timestamp_millis();
        let event_id = Uuid::new_v4().to_string();
        let mut app_err: Option<(StatusCode, &'static str, String)> = None;
        let out = retry_on_busy("append_execution_event", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let inp = EventTxInput { tid: &principal.tenant_id, pid: &pid, job_id: &job_id, node_id: &node_id, req_sid: req_sid.as_deref(), event_kind: &event_kind, payload_text: &req.payload_text, next_status: next_status.as_deref(), expected_token: req.expected_fencing_token, nonce: &nonce, event_id: &event_id, now_ms };
            match apply_job_event_tx(&tx, &inp) {
                Ok(v) => { tx.commit()?; Ok(Some(v)) }
                Err(e) => { app_err = Some(e); Ok(None) }
            }
        }).map_err(|e| map_harness_resp(e, "append_execution_event"))?;
        let Some((seq_no, project_seq, scope, final_status, promoted)) = out else {
            let (st, code, msg) = app_err.unwrap_or((StatusCode::CONFLICT, "STALE_FENCING_TOKEN", "Event rejected".into()));
            return Err(json_err(st, code, msg));
        };
        if matches!(final_status.as_str(), "COMPLETED" | "FAILED" | "CANCELLED") {
            let _ = reap_job_worker_and_shadow(&workers, &root, &pid, &job_id);
        }
        Ok((ExecutionEventDto { id: event_id, job_id, project_id: pid, node_id, event_kind, payload_text: req.payload_text, seq_no, project_seq, composite_scope_key: scope, job_status: final_status, created_at: epoch_ms_to_iso(now_ms) }, promoted))
    }).await?;
    let _ = sse_tx.send(created.project_id.clone());
    for (_, p_pid) in promoted { let _ = sse_tx.send(p_pid); }
    Ok((StatusCode::CREATED, Json(created)))
}
