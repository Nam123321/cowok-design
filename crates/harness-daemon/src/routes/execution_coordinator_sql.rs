// @story 04.4
// @implements AC-01, AC-02, AC-04 (SQLite V7 Coordinator DTOs & Transaction Helpers)
use std::path::PathBuf;

use axum::{http::HeaderMap, http::StatusCode, response::IntoResponse, response::Response, Json};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::{FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress};
use crate::routes::legacy_specs::verify_anti_idor_with_caller;
use crate::routes::{map_error_to_response, DaemonAppState, HttpErrorBody};

pub const MAX_GLOBAL_ACTIVE_SLOTS: i64 = 4;
pub const LEASE_TTL_MS: i64 = 15_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeExecutionJobDto {
    pub id: String,
    pub project_id: String,
    pub session_id: String,
    pub node_id: String,
    pub composite_scope_key: String,
    pub daemon_nonce: String,
    pub status: String,
    pub fencing_token: i64,
    pub priority: i64,
    pub stale_after_timestamp: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeChatSessionDto {
    pub id: String,
    pub project_id: String,
    pub chat_session_id: String,
    pub node_id: String,
    pub composite_scope_key: String,
    pub title: String,
    pub status: String,
    pub unread_count: i64,
    pub last_message_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueNodeJobInput {
    pub session_id: String,
    pub node_ids: Vec<String>,
    pub prompt: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}
fn default_mode() -> String { "parallel".to_string() }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteerNodeJobInput {
    pub job_id: String,
    pub expected_fencing_token: i64,
    pub instruction: String,
    #[serde(default = "default_true")]
    pub pgid_reap_requested: bool,
}
fn default_true() -> bool { true }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionNodeJobInput {
    pub job_id: String,
    pub expected_fencing_token: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSessionsQuery {
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueNodeJobsResponse {
    pub jobs: Vec<NodeExecutionJobDto>,
    pub active_slot_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSessionsResponse {
    pub sessions: Vec<NodeChatSessionDto>,
    pub jobs: Vec<NodeExecutionJobDto>,
    pub active_slot_count: i64,
    pub max_concurrent: i64,
}

pub fn err_resp(status: StatusCode, code: &str, msg: impl Into<String>) -> Response {
    let body = HttpErrorBody {
        code: code.to_string(),
        kind: Some(code.to_string()),
        message: msg.into(),
        details: serde_json::Value::Null,
        request_id: Uuid::new_v4().to_string(),
    };
    (status, Json(body)).into_response()
}

pub fn authorize_project(
    state: &DaemonAppState,
    raw_id: &str,
    headers: &HeaderMap,
    principal: &FlowPrincipal,
) -> Result<(String, PathBuf), Response> {
    let uuid = Uuid::parse_str(raw_id.trim())
        .map_err(|_| err_resp(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", "Invalid project UUID"))?;
    let caller = VerifiedLegacyIngress {
        principal: principal.clone(),
        kind: LegacyAuthKind::SessionToken,
        bearer_token: None,
    };
    let (_, root, _) = verify_anti_idor_with_caller(&state.registry, uuid, headers, Some(&caller))
        .map_err(map_error_to_response)?;
    Ok((uuid.to_string(), root))
}

pub async fn run_blocking_resp<T, F>(op: F) -> Result<T, Response>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Response> + Send + 'static,
{
    tokio::task::spawn_blocking(op).await.map_err(|e| {
        err_resp(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", format!("Join error: {e}"))
    })?
}

pub fn ensure_chat_session_tx(
    tx: &rusqlite::Transaction<'_>,
    tid: &str,
    pid: &str,
    sid: &str,
    now_ms: i64,
) -> Result<(), (StatusCode, &'static str, String)> {
    let row: Option<(String, String)> = tx
        .query_row("SELECT project_id, tenant_id FROM chat_sessions WHERE id = ?1", params![sid], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    match row {
        Some((_, s_tid)) if s_tid != tid => Err((StatusCode::FORBIDDEN, "TENANT_ISOLATION_BREACH", "Session belongs to another tenant".into())),
        Some((s_pid, _)) if s_pid != pid => Err((StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND", "Session belongs to another project".into())),
        Some(_) => Ok(()),
        None => {
            tx.execute(
                "INSERT INTO chat_sessions (id, tenant_id, project_id, title, model_provider, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'Flow Execution Session', 'default', ?4, ?4)",
                params![sid, tid, pid, now_ms],
            ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
            Ok(())
        }
    }
}

pub struct NewEventParams<'a> {
    pub tid: &'a str,
    pub pid: &'a str,
    pub job_id: &'a str,
    pub node_id: &'a str,
    pub scope_key: &'a str,
    pub kind: &'a str,
    pub payload: &'a str,
    pub now_ms: i64,
}

pub fn append_event_tx(tx: &rusqlite::Transaction<'_>, ev: &NewEventParams<'_>) -> Result<(), rusqlite::Error> {
    let next_seq: i64 = tx.query_row("SELECT COALESCE(MAX(seq_no), 0) + 1 FROM execution_events WHERE job_id = ?1", params![ev.job_id], |r| r.get(0))?;
    let next_pseq: i64 = tx.query_row("SELECT COALESCE(MAX(project_seq), 0) + 1 FROM execution_events WHERE project_id = ?1", params![ev.pid], |r| r.get(0))?;
    tx.execute(
        "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, composite_scope_key, event_kind, payload_text, seq_no, project_seq, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![Uuid::new_v4().to_string(), ev.tid, ev.pid, ev.job_id, ev.node_id, ev.scope_key, ev.kind, ev.payload, next_seq, next_pseq, ev.now_ms],
    )?;
    Ok(())
}

pub fn map_job_row(r: &rusqlite::Row<'_>) -> Result<NodeExecutionJobDto, rusqlite::Error> {
    Ok(NodeExecutionJobDto {
        id: r.get(0)?, project_id: r.get(1)?, session_id: r.get(2)?, node_id: r.get(3)?,
        composite_scope_key: r.get(4)?, daemon_nonce: r.get(5)?, status: r.get(6)?,
        fencing_token: r.get(7)?, priority: r.get(8)?, stale_after_timestamp: r.get(9)?,
        created_at: r.get(10)?, updated_at: r.get(11)?,
    })
}

pub fn read_job_dto_conn(conn: &rusqlite::Connection, job_id: &str) -> Result<NodeExecutionJobDto, rusqlite::Error> {
    conn.query_row(
        "SELECT id, project_id, session_id, node_id, composite_scope_key, daemon_nonce, status, fencing_token, priority, stale_after_timestamp, created_at, updated_at FROM node_execution_jobs WHERE id = ?1",
        params![job_id], map_job_row,
    )
}

pub fn fetch_job_meta(conn: &rusqlite::Connection, job_id: &str, tid: &str, pid: &str) -> Result<(i64, String, String, String, String), Response> {
    let row: Option<(String, String, i64, String, String, String, String)> = conn
        .query_row(
            "SELECT tenant_id, project_id, fencing_token, status, node_id, composite_scope_key, daemon_nonce FROM node_execution_jobs WHERE id = ?1",
            params![job_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional().map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    let Some((j_tid, j_pid, tok, status, nid, scope, nonce)) = row else {
        return Err(err_resp(StatusCode::NOT_FOUND, "JOB_NOT_FOUND", "Job not found"));
    };
    if j_tid != tid { return Err(err_resp(StatusCode::FORBIDDEN, "TENANT_ISOLATION_BREACH", "Cross-tenant job denied")); }
    if j_pid != pid { return Err(err_resp(StatusCode::NOT_FOUND, "JOB_NOT_FOUND", "Job belongs to another project")); }
    Ok((tok, status, nid, scope, nonce))
}

pub fn promote_next_queued_tx(tx: &rusqlite::Transaction<'_>, daemon_nonce: &str, now_ms: i64) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut promoted = Vec::new();
    loop {
        let active: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED')", [], |r| r.get(0))?;
        if active >= MAX_GLOBAL_ACTIVE_SLOTS { break; }
        let next: Option<(String, String, String, String, String, i64)> = tx.query_row(
            "SELECT q.id, q.tenant_id, q.project_id, q.node_id, q.composite_scope_key, q.fencing_token FROM node_execution_jobs q WHERE q.status = 'QUEUED' AND NOT EXISTS (SELECT 1 FROM node_execution_jobs a WHERE a.composite_scope_key = q.composite_scope_key AND a.status IN ('RUNNING', 'PAUSED')) ORDER BY q.created_at ASC, q.id ASC LIMIT 1",
            [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        ).optional()?;
        let Some((jid, tid, pid, nid, scope, fence)) = next else { break; };
        let next_fence = fence + 1;
        let rows = tx.execute(
            "UPDATE node_execution_jobs SET status = 'RUNNING', daemon_nonce = ?1, fencing_token = ?2, started_at = COALESCE(started_at, ?3), stale_after_timestamp = ?4, updated_at = ?3 WHERE id = ?5 AND fencing_token = ?6 AND composite_scope_key = ?7 AND status = 'QUEUED'",
            params![daemon_nonce, next_fence, now_ms, now_ms + LEASE_TTL_MS, jid, fence, scope],
        )?;
        if rows == 0 { break; }
        let payload = serde_json::json!({ "promoted": true, "fencingToken": next_fence }).to_string();
        append_event_tx(tx, &NewEventParams { tid: &tid, pid: &pid, job_id: &jid, node_id: &nid, scope_key: &scope, kind: "started", payload: &payload, now_ms })?;
        promoted.push((jid, pid));
    }
    Ok(promoted)
}

pub fn begin_immediate(conn: &mut rusqlite::Connection) -> Result<rusqlite::Transaction<'_>, rusqlite::Error> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
}
