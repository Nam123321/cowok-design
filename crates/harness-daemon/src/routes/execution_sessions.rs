// @story 04.4
// @implements AC-04, AC-06 (Node Session Unread Reset & Skill Plugin Listing Endpoint)
use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::daemon_auth::FlowPrincipal;
use crate::routes::execution_coordinator_sql::{authorize_project, err_resp, run_blocking_resp};
use crate::routes::DaemonAppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkNodeSessionReadInput {
    pub composite_scope_key: Option<String>,
    pub session_id: Option<String>,
    pub node_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkNodeSessionReadResponse {
    pub ok: bool,
    pub composite_scope_key: String,
    pub cleared_rows: usize,
}

pub async fn handle_mark_node_session_read(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<MarkNodeSessionReadInput>,
) -> Result<Json<MarkNodeSessionReadResponse>, Response> {
    let (pid, _) = authorize_project(&state, &id, &headers, &principal)?;
    let scope = match req
        .composite_scope_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) if raw.starts_with(&format!("{pid}::")) => raw.to_string(),
        Some(_) => {
            return Err(err_resp(
                StatusCode::CONFLICT,
                "SCOPE_MISMATCH",
                "compositeScopeKey must belong to target project",
            ))
        }
        None => {
            let sid = req
                .session_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    err_resp(
                        StatusCode::BAD_REQUEST,
                        "VALIDATION_ERROR",
                        "sessionId or compositeScopeKey required",
                    )
                })?;
            let nid = req
                .node_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    err_resp(
                        StatusCode::BAD_REQUEST,
                        "VALIDATION_ERROR",
                        "nodeId or compositeScopeKey required",
                    )
                })?;
            format!("{pid}::{sid}::{nid}")
        }
    };
    let (pool, sse_tx, pid_c, scope_c) = (
        state.registry.pool().clone(),
        state.sse_tx.clone(),
        pid.clone(),
        scope.clone(),
    );
    let cleared_rows = run_blocking_resp(move || {
        let conn = pool.connection().map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE node_chat_sessions SET unread_count = 0, updated_at = ?1 WHERE tenant_id = ?2 AND project_id = ?3 AND composite_scope_key = ?4",
            params![now_ms, principal.tenant_id, pid_c, scope_c],
        ).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))
    }).await?;
    let _ = sse_tx.send(pid);
    Ok(Json(MarkNodeSessionReadResponse {
        ok: true,
        composite_scope_key: scope,
        cleared_rows,
    }))
}
