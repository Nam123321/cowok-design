// @story 04.4
// @implements AC-01, AC-02, AC-04 (KeyedParallelCoordinator & Node Chat Sessions API)
use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use chrono::Utc;
use rusqlite::params;
use serde_json::json;
use spec_registry::retry_on_busy;
use uuid::Uuid;

use crate::daemon_auth::FlowPrincipal;
use crate::routes::execution_coordinator_sql::*;
use crate::routes::execution_reaper::reap_job_worker_and_shadow;
use crate::routes::DaemonAppState;

pub async fn handle_enqueue_node_job(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<EnqueueNodeJobInput>,
) -> Result<(StatusCode, Json<EnqueueNodeJobsResponse>), Response> {
    if req.node_ids.is_empty() || req.session_id.trim().is_empty() || req.prompt.trim().is_empty() {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "sessionId, nodeIds, and prompt are required",
        ));
    }
    let (pid, _) = authorize_project(&state, &id, &headers, &principal)?;
    let (pool, nonce, sse_tx, pid_c) = (
        state.registry.pool().clone(),
        state.auth.daemon_nonce.clone(),
        state.sse_tx.clone(),
        pid.clone(),
    );
    let res =
        run_blocking_resp(move || enqueue_tx(&pool, &principal.tenant_id, &pid_c, &nonce, req))
            .await?;
    let _ = sse_tx.send(pid);
    Ok((StatusCode::CREATED, Json(res)))
}

fn enqueue_tx(
    pool: &spec_registry::SqlitePool,
    tid: &str,
    pid: &str,
    nonce: &str,
    req: EnqueueNodeJobInput,
) -> Result<EnqueueNodeJobsResponse, Response> {
    let mut conn = pool
        .connection()
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    let now_ms = Utc::now().timestamp_millis();
    let mut scope_err: Option<(StatusCode, &'static str, String)> = None;
    let out = retry_on_busy("enqueue_node_job", || {
        let tx = begin_immediate(&mut conn)?;
        if let Err(e) = ensure_chat_session_tx(&tx, tid, pid, req.session_id.trim(), now_ms) {
            scope_err = Some(e);
            return Ok(None);
        }
        let mut active: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED')", [], |r| r.get(0))?;
        let mut jobs = Vec::new();
        for (idx, raw_nid) in req.node_ids.iter().enumerate() {
            let nid = raw_nid.trim();
            if nid.is_empty() { continue; }
            let scope = format!("{pid}::{}::{nid}", req.session_id.trim());
            let busy: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE composite_scope_key = ?1 AND status IN ('QUEUED', 'RUNNING', 'PAUSED')", params![scope], |r| r.get(0))?;
            let can_run = active < MAX_GLOBAL_ACTIVE_SLOTS && busy == 0 && !(req.mode == "queue" && idx > 0);
            let status = if can_run { active += 1; "RUNNING" } else { "QUEUED" };
            let fence: i64 = tx.query_row("SELECT COALESCE(MAX(fencing_token), 0) + 1 FROM node_execution_jobs WHERE composite_scope_key = ?1", params![scope], |r| r.get(0))?;
            let jid = format!("job-{}", Uuid::new_v4());
            tx.execute(
                "INSERT INTO node_execution_jobs (id, tenant_id, project_id, session_id, node_id, composite_scope_key, daemon_nonce, status, fencing_token, priority, stale_after_timestamp, created_at, started_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 10, ?10, ?11, ?12, ?11)",
                params![jid, tid, pid, req.session_id.trim(), nid, scope, nonce, status, fence, can_run.then_some(now_ms + LEASE_TTL_MS), now_ms, can_run.then_some(now_ms)],
            )?;
            tx.execute(
                "INSERT INTO node_chat_sessions (id, tenant_id, project_id, node_id, chat_session_id, composite_scope_key, title, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ACTIVE', ?8, ?8)
                 ON CONFLICT(project_id, node_id, chat_session_id) DO UPDATE SET composite_scope_key = excluded.composite_scope_key, updated_at = excluded.updated_at",
                params![Uuid::new_v4().to_string(), tid, pid, nid, req.session_id.trim(), scope, format!("Node {nid}"), now_ms],
            )?;
            let payload = json!({"prompt": req.prompt, "status": status, "fencingToken": fence}).to_string();
            append_event_tx(&tx, &NewEventParams { tid, pid, job_id: &jid, node_id: nid, scope_key: &scope, kind: "enqueued", payload: &payload, now_ms })?;
            jobs.push(read_job_dto_conn(&tx, &jid)?);
        }
        tx.commit()?;
        Ok(Some(EnqueueNodeJobsResponse { jobs, active_slot_count: active }))
    }).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    out.ok_or_else(|| {
        let (s, c, m) = scope_err.unwrap_or((
            StatusCode::CONFLICT,
            "ENQUEUE_FAILED",
            "Enqueue aborted".into(),
        ));
        err_resp(s, c, m)
    })
}

pub async fn handle_steer_node_job(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SteerNodeJobInput>,
) -> Result<Json<NodeExecutionJobDto>, Response> {
    if req.instruction.trim().is_empty() || req.job_id.trim().is_empty() {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "jobId and instruction are required",
        ));
    }
    let (pid, root) = authorize_project(&state, &id, &headers, &principal)?;
    let (pool, workers, nonce, sse_tx, pid_c) = (
        state.registry.pool().clone(),
        state.active_workers.clone(),
        state.auth.daemon_nonce.clone(),
        state.sse_tx.clone(),
        pid.clone(),
    );
    let dto = run_blocking_resp(move || {
        let ctx = TransitionCtx {
            pool: &pool,
            workers: &workers,
            root: &root,
            tid: &principal.tenant_id,
            pid: &pid_c,
            nonce: &nonce,
        };
        steer_tx(&ctx, &req)
    })
    .await?;
    let _ = sse_tx.send(pid);
    Ok(Json(dto))
}

fn steer_tx(
    ctx: &TransitionCtx<'_>,
    req: &SteerNodeJobInput,
) -> Result<NodeExecutionJobDto, Response> {
    let mut conn = ctx
        .pool
        .connection()
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    let jid = req.job_id.trim();
    let now_ms = Utc::now().timestamp_millis();
    let mut outcome: Result<(), Response> = Err(err_resp(
        StatusCode::CONFLICT,
        "STALE_FENCING_TOKEN",
        "Concurrent fencing token mutation",
    ));
    retry_on_busy("steer_cas", || {
        let tx = begin_immediate(&mut conn)?;
        let (tok, status, nid, scope, _old_nonce) = match fetch_job_meta(&tx, jid, ctx.tid, ctx.pid) {
            Ok(v) => v,
            Err(r) => { outcome = Err(r); return Ok(()); }
        };
        if tok != req.expected_fencing_token || !matches!(status.as_str(), "QUEUED" | "RUNNING" | "PAUSED") {
            outcome = Err(err_resp(StatusCode::CONFLICT, "STALE_FENCING_TOKEN", format!("Expected token {tok} (status {status})")));
            return Ok(());
        }
        // Kill active -PGID and discard shadow buffer BEFORE incrementing fencing_token
        // so the superseded turn cannot race with or outlive the new fencing token commit.
        if req.pgid_reap_requested {
            if let Err(err) = reap_job_worker_and_shadow(ctx.workers, ctx.root, ctx.pid, jid) {
                outcome = Err(err_resp(StatusCode::INTERNAL_SERVER_ERROR, "REAP_FAILED", err.to_string()));
                return Ok(());
            }
        }
        let next_status = if status == "QUEUED" {
            let active: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED')", [], |r| r.get(0))?;
            let scope_busy: i64 = tx.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE composite_scope_key = ?1 AND status IN ('RUNNING', 'PAUSED')", params![scope], |r| r.get(0))?;
            if active < MAX_GLOBAL_ACTIVE_SLOTS && scope_busy == 0 { "RUNNING" } else { "QUEUED" }
        } else { "RUNNING" };
        let stale = (next_status == "RUNNING").then_some(now_ms + LEASE_TTL_MS);
        let n = tx.execute(
            "UPDATE node_execution_jobs SET fencing_token = fencing_token + 1, daemon_nonce = ?1, status = ?2, stale_after_timestamp = ?3, updated_at = ?4
             WHERE id = ?5 AND tenant_id = ?6 AND project_id = ?7 AND composite_scope_key = ?8 AND fencing_token = ?9 AND status = ?10",
            params![ctx.nonce, next_status, stale, now_ms, jid, ctx.tid, ctx.pid, scope, req.expected_fencing_token, status],
        )?;
        if n == 0 { outcome = Err(err_resp(StatusCode::CONFLICT, "STALE_FENCING_TOKEN", "Concurrent fencing token mutation")); return Ok(()); }
        let payload = json!({"instruction": req.instruction, "fencingToken": req.expected_fencing_token + 1}).to_string();
        append_event_tx(&tx, &NewEventParams { tid: ctx.tid, pid: ctx.pid, job_id: jid, node_id: &nid, scope_key: &scope, kind: "steered", payload: &payload, now_ms })?;
        tx.commit()?;
        outcome = Ok(());
        Ok(())
    }).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    outcome?;
    read_job_dto_conn(&conn, jid)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))
}

struct TransitionCtx<'a> {
    pool: &'a spec_registry::SqlitePool,
    workers: &'a crate::routes::execution_reaper::ActiveWorkerRegistry,
    root: &'a std::path::Path,
    tid: &'a str,
    pid: &'a str,
    nonce: &'a str,
}

async fn apply_transition(
    state: DaemonAppState,
    principal: FlowPrincipal,
    id: String,
    headers: HeaderMap,
    req: TransitionNodeJobInput,
    target: &'static str,
) -> Result<Json<NodeExecutionJobDto>, Response> {
    if req.job_id.trim().is_empty() {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "jobId is required",
        ));
    }
    let (pid, root) = authorize_project(&state, &id, &headers, &principal)?;
    let (pool, workers, nonce, sse_tx, pid_c) = (
        state.registry.pool().clone(),
        state.active_workers.clone(),
        state.auth.daemon_nonce.clone(),
        state.sse_tx.clone(),
        pid.clone(),
    );
    let (dto, promoted) = run_blocking_resp(move || {
        let ctx = TransitionCtx {
            pool: &pool,
            workers: &workers,
            root: &root,
            tid: &principal.tenant_id,
            pid: &pid_c,
            nonce: &nonce,
        };
        transition_tx(&ctx, &req, target)
    })
    .await?;
    let _ = sse_tx.send(pid);
    for (_, p_pid) in promoted {
        let _ = sse_tx.send(p_pid);
    }
    Ok(Json(dto))
}

fn resolve_transition_next(status: &str, target: &str) -> Option<(&'static str, &'static str)> {
    match (target, status) {
        ("PAUSED", "RUNNING") => Some(("PAUSED", "paused")),
        ("RUNNING", "PAUSED") => Some(("RUNNING", "resumed")),
        ("RUNNING", "INTERRUPTED") => Some(("QUEUED", "queued")),
        ("CANCELLED", "QUEUED" | "RUNNING" | "PAUSED" | "INTERRUPTED") => {
            Some(("CANCELLED", "cancelled"))
        }
        _ => None,
    }
}

fn transition_tx(
    ctx: &TransitionCtx<'_>,
    req: &TransitionNodeJobInput,
    target: &'static str,
) -> Result<(NodeExecutionJobDto, Vec<(String, String)>), Response> {
    let mut conn = ctx
        .pool
        .connection()
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    let jid = req.job_id.trim();
    let now_ms = Utc::now().timestamp_millis();
    let mut promoted = Vec::new();
    let mut outcome: Result<(), Response> = Err(err_resp(
        StatusCode::CONFLICT,
        "STALE_FENCING_TOKEN",
        "Concurrent fencing token mutation",
    ));
    retry_on_busy("transition_cas", || {
        let tx = begin_immediate(&mut conn)?;
        let (tok, status, nid, scope, _old_nonce) = match fetch_job_meta(&tx, jid, ctx.tid, ctx.pid) {
            Ok(v) => v,
            Err(r) => { outcome = Err(r); return Ok(()); }
        };
        let Some((next_status, kind)) = resolve_transition_next(&status, target) else {
            outcome = Err(err_resp(StatusCode::CONFLICT, "STALE_FENCING_TOKEN", format!("Cannot transition {status} -> {target}")));
            return Ok(());
        };
        if tok != req.expected_fencing_token {
            outcome = Err(err_resp(StatusCode::CONFLICT, "STALE_FENCING_TOKEN", format!("Expected token {tok} (status {status})")));
            return Ok(());
        }
        if target == "CANCELLED" {
            if let Err(err) = reap_job_worker_and_shadow(ctx.workers, ctx.root, ctx.pid, jid) {
                outcome = Err(err_resp(StatusCode::INTERNAL_SERVER_ERROR, "REAP_FAILED", err.to_string()));
                return Ok(());
            }
        }
        let stale = (next_status == "RUNNING").then_some(now_ms + LEASE_TTL_MS);
        let n = tx.execute(
            "UPDATE node_execution_jobs SET status = ?1, fencing_token = fencing_token + 1, daemon_nonce = ?2, stale_after_timestamp = ?3, completed_at = CASE WHEN ?1 = 'CANCELLED' THEN ?4 ELSE completed_at END, updated_at = ?4
             WHERE id = ?5 AND tenant_id = ?6 AND project_id = ?7 AND composite_scope_key = ?8 AND fencing_token = ?9 AND status = ?10",
            params![next_status, ctx.nonce, stale, now_ms, jid, ctx.tid, ctx.pid, scope, req.expected_fencing_token, status],
        )?;
        if n == 0 { outcome = Err(err_resp(StatusCode::CONFLICT, "STALE_FENCING_TOKEN", "Concurrent fencing token mutation")); return Ok(()); }
        let payload = json!({"fencingToken": req.expected_fencing_token + 1, "status": next_status}).to_string();
        append_event_tx(&tx, &NewEventParams { tid: ctx.tid, pid: ctx.pid, job_id: jid, node_id: &nid, scope_key: &scope, kind, payload: &payload, now_ms })?;
        if matches!(next_status, "CANCELLED" | "QUEUED") { promoted = promote_next_queued_tx(&tx, ctx.nonce, now_ms)?; }
        tx.commit()?;
        outcome = Ok(());
        Ok(())
    }).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    outcome?;
    let dto = read_job_dto_conn(&conn, jid)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    Ok((dto, promoted))
}

pub async fn handle_pause_node_job(
    State(s): State<DaemonAppState>,
    Extension(p): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    h: HeaderMap,
    Json(r): Json<TransitionNodeJobInput>,
) -> Result<Json<NodeExecutionJobDto>, Response> {
    apply_transition(s, p, id, h, r, "PAUSED").await
}
pub async fn handle_resume_node_job(
    State(s): State<DaemonAppState>,
    Extension(p): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    h: HeaderMap,
    Json(r): Json<TransitionNodeJobInput>,
) -> Result<Json<NodeExecutionJobDto>, Response> {
    apply_transition(s, p, id, h, r, "RUNNING").await
}
pub async fn handle_cancel_node_job(
    State(s): State<DaemonAppState>,
    Extension(p): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    h: HeaderMap,
    Json(r): Json<TransitionNodeJobInput>,
) -> Result<Json<NodeExecutionJobDto>, Response> {
    apply_transition(s, p, id, h, r, "CANCELLED").await
}

pub async fn handle_list_node_chat_sessions(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Query(q): Query<NodeSessionsQuery>,
) -> Result<Json<NodeSessionsResponse>, Response> {
    let (pid, _) = authorize_project(&state, &id, &headers, &principal)?;
    let pool = state.registry.pool().clone();
    let res = run_blocking_resp(move || {
        let conn = pool.connection().map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let active_slot_count: i64 = conn.query_row("SELECT COUNT(*) FROM node_execution_jobs WHERE status IN ('RUNNING', 'PAUSED')", [], |r| r.get(0)).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let sid_filter = q.session_id.unwrap_or_default().trim().to_string();
        let mut s_stmt = conn.prepare(
            "SELECT id, project_id, chat_session_id, node_id, composite_scope_key, title, status, unread_count, last_message_id, updated_at
             FROM node_chat_sessions WHERE tenant_id = ?1 AND project_id = ?2 AND (?3 = '' OR chat_session_id = ?3) ORDER BY updated_at DESC",
        ).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let sessions = s_stmt.query_map(params![principal.tenant_id, pid, sid_filter], |r| Ok(NodeChatSessionDto {
            id: r.get(0)?, project_id: r.get(1)?, chat_session_id: r.get(2)?, node_id: r.get(3)?, composite_scope_key: r.get(4)?,
            title: r.get(5)?, status: r.get(6)?, unread_count: r.get(7)?, last_message_id: r.get(8)?, updated_at: r.get(9)?,
        })).and_then(|it| it.collect::<Result<Vec<_>, _>>()).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let mut j_stmt = conn.prepare(
            "SELECT id, project_id, session_id, node_id, composite_scope_key, daemon_nonce, status, fencing_token, priority, stale_after_timestamp, created_at, updated_at
             FROM node_execution_jobs WHERE tenant_id = ?1 AND project_id = ?2 AND (?3 = '' OR session_id = ?3) ORDER BY created_at ASC",
        ).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let jobs = j_stmt.query_map(params![principal.tenant_id, pid, sid_filter], map_job_row).and_then(|it| it.collect::<Result<Vec<_>, _>>()).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        Ok(NodeSessionsResponse { sessions, jobs, active_slot_count, max_concurrent: MAX_GLOBAL_ACTIVE_SLOTS })
    }).await?;
    Ok(Json(res))
}
