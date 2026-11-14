// @story 04.4
// @implements AC-04, AC-06 (Node Session Unread Reset & Skill Plugin Listing Endpoint)
use axum::{extract::{Extension, Path as AxumPath, Query, State}, http::{HeaderMap, StatusCode}, response::Response, Json};
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
    let scope = match req.composite_scope_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) if raw.starts_with(&format!("{pid}::")) => raw.to_string(),
        Some(_) => return Err(err_resp(StatusCode::CONFLICT, "SCOPE_MISMATCH", "compositeScopeKey must belong to target project")),
        None => {
            let sid = req.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| err_resp(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", "sessionId or compositeScopeKey required"))?;
            let nid = req.node_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| err_resp(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", "nodeId or compositeScopeKey required"))?;
            format!("{pid}::{sid}::{nid}")
        }
    };
    let (pool, sse_tx, pid_c, scope_c) = (state.registry.pool().clone(), state.sse_tx.clone(), pid.clone(), scope.clone());
    let cleared_rows = run_blocking_resp(move || {
        let conn = pool.connection().map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE node_chat_sessions SET unread_count = 0, updated_at = ?1 WHERE tenant_id = ?2 AND project_id = ?3 AND composite_scope_key = ?4",
            params![now_ms, principal.tenant_id, pid_c, scope_c],
        ).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))
    }).await?;
    let _ = sse_tx.send(pid);
    Ok(Json(MarkNodeSessionReadResponse { ok: true, composite_scope_key: scope, cleared_rows }))
}

#[derive(Debug, Default, Deserialize)]
pub struct ListPluginsQuery {
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPluginDto {
    pub id: String,
    pub project_id: String,
    pub plugin_slug: String,
    pub name: String,
    pub version: String,
    pub lifecycle_state: String,
    pub source_repo_url: String,
    pub manifest: serde_json::Value,
    pub enabled: bool,
    pub updated_at: String,
}

pub async fn handle_list_plugins(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Query(q): Query<ListPluginsQuery>,
) -> Result<Json<Vec<SkillPluginDto>>, Response> {
    let pool = state.registry.pool().clone();
    let only_enabled = q.enabled.unwrap_or(false);
    let plugins = run_blocking_resp(move || {
        let conn = pool.connection().map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT sp.id, sp.project_id, sp.plugin_slug, sp.version, sp.lifecycle_state, sp.source_repo_url, sp.manifest_json, sp.updated_at
             FROM skill_plugins sp INNER JOIN projects p ON p.id = sp.project_id
             WHERE p.tenant_id = ?1 AND p.trash_deleted_at IS NULL AND (?2 = 0 OR sp.lifecycle_state IN ('ENABLED', 'ACTIVE'))
             ORDER BY sp.plugin_slug ASC",
        ).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let rows = stmt.query_map(params![principal.tenant_id, i64::from(only_enabled)], |r| {
            let slug: String = r.get(2)?;
            let lifecycle_state: String = r.get(4)?;
            let manifest_raw: String = r.get(6)?;
            let manifest: serde_json::Value = serde_json::from_str(&manifest_raw).unwrap_or_else(|_| serde_json::json!({}));
            let name = manifest.get("name").and_then(|v| v.as_str()).unwrap_or(&slug).to_string();
            let updated_ms: i64 = r.get(7)?;
            Ok(SkillPluginDto {
                id: r.get(0)?, project_id: r.get(1)?, plugin_slug: slug, name, version: r.get(3)?,
                enabled: matches!(lifecycle_state.as_str(), "ENABLED" | "ACTIVE"), lifecycle_state,
                source_repo_url: r.get(5)?, manifest,
                updated_at: super::legacy_workbench_v6::epoch_ms_to_iso(updated_ms),
            })
        }).map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))
    }).await?;
    Ok(Json(plugins))
}
