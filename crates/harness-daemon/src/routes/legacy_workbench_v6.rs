// @story 04.2
// @implements V6 SQLite Workbench Endpoints (Comments, Fixtures, Viewport, Settings, MCP, Job SSE, Key Verify)

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use chrono::{DateTime, Utc};
use domain::HarnessError;
use futures_util::stream::Stream;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::{map_sqlite_error, retry_on_busy, SqlitePool};
use uuid::Uuid;

use crate::daemon_auth::FlowPrincipal;
use crate::routes::{DaemonAppState, HttpErrorBody};

pub(crate) fn epoch_ms_to_iso(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339())
}

pub(crate) fn json_err(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    let body = HttpErrorBody {
        code: code.to_string(),
        kind: Some(code.to_string()),
        message: message.into(),
        details: serde_json::Value::Null,
        request_id: Uuid::new_v4().to_string(),
    };
    (status, Json(body)).into_response()
}

fn check_project_uuid(raw: &str) -> Result<String, Response> {
    Uuid::parse_str(raw.trim())
        .map(|u| u.to_string())
        .map_err(|_| {
            json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid project UUID",
            )
        })
}

pub(crate) fn check_non_empty_id(raw: &str, label: &str) -> Result<String, Response> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 160 {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            format!("Invalid {label}"),
        ));
    }
    Ok(trimmed.to_string())
}

fn verify_project_ownership(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
) -> Result<(), Response> {
    let row: Option<String> = conn
        .query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![project_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| {
            let err = map_sqlite_error(e, "verify_project_ownership");
            json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                err.to_string(),
            )
        })?;

    match row {
        Some(owner_tenant) if owner_tenant == tenant_id => Ok(()),
        Some(_) => Err(json_err(
            StatusCode::FORBIDDEN,
            "TENANT_ISOLATION_BREACH",
            "Project belongs to another tenant",
        )),
        None => Err(json_err(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            format!("Project '{project_id}' was not found"),
        )),
    }
}

pub(crate) async fn run_db<T, F>(op: F) -> Result<T, Response>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Response> + Send + 'static,
{
    tokio::task::spawn_blocking(op).await.map_err(|e| {
        json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Task join error: {e}"),
        )
    })?
}

pub(crate) fn map_harness_resp(err: HarnessError, op: &str) -> Response {
    json_err(
        StatusCode::INTERNAL_SERVER_ERROR,
        "DB_ERROR",
        format!("{op}: {err}"),
    )
}

// -----------------------------------------------------------------------------
// 1. Element Comments (V6 element_comments)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementCommentDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub node_id: String,
    pub element_ff_id: String,
    pub author: String,
    pub content: String,
    pub dom_hash: String,
    pub anchor_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateElementCommentInput {
    pub element_ff_id: String,
    pub author: String,
    pub content: String,
    pub dom_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateElementCommentInput {
    pub content: Option<String>,
    pub anchor_status: Option<String>,
    pub element_ff_id: Option<String>,
    pub dom_hash: Option<String>,
}

fn map_comment_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ElementCommentDto> {
    Ok(ElementCommentDto {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        project_id: row.get(2)?,
        node_id: row.get(3)?,
        element_ff_id: row.get(4)?,
        author: row.get(5)?,
        content: row.get(6)?,
        dom_hash: row.get(7)?,
        anchor_status: row.get(8)?,
        created_at: epoch_ms_to_iso(row.get(9)?),
        updated_at: epoch_ms_to_iso(row.get(10)?),
    })
}

pub async fn handle_list_comments(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
) -> Result<Json<Vec<ElementCommentDto>>, Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    let pool = state.registry.pool().clone();

    let list = run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, project_id, node_id, element_ff_id, author, content,
                        dom_hash, anchor_status, created_at, updated_at
                 FROM element_comments
                 WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3
                 ORDER BY created_at ASC",
            )
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "prep_comments"), "list_comments"))?;
        let rows = stmt
            .query_map(params![principal.tenant_id, pid, nid], map_comment_row)
            .map_err(|e| {
                map_harness_resp(map_sqlite_error(e, "query_comments"), "list_comments")
            })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| {
                map_harness_resp(map_sqlite_error(e, "row_comments"), "list_comments")
            })?);
        }
        Ok(out)
    })
    .await?;

    Ok(Json(list))
}

pub async fn handle_create_comment(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
    Json(req): Json<CreateElementCommentInput>,
) -> Result<(StatusCode, Json<ElementCommentDto>), Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    let elem = check_non_empty_id(&req.element_ff_id, "elementFfId")?;
    let author = check_non_empty_id(&req.author, "author")?;
    let content = req.content.trim().to_string();
    if content.is_empty() || content.len() > 2000 {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Comment content must be between 1 and 2000 characters",
        ));
    }
    let dom_hash = check_non_empty_id(&req.dom_hash, "domHash")?;
    let pool = state.registry.pool().clone();

    let created = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let now_ms = Utc::now().timestamp_millis();
        let comment_id = Uuid::new_v4().to_string();

        retry_on_busy("insert_element_comment", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO element_comments (
                    id, tenant_id, project_id, node_id, element_ff_id, author, content,
                    dom_hash, anchor_status, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', ?9, ?9)",
                params![
                    comment_id,
                    principal.tenant_id,
                    pid,
                    nid,
                    elem,
                    author,
                    content,
                    dom_hash,
                    now_ms
                ],
            )?;
            tx.commit()
        })
        .map_err(|e| map_harness_resp(e, "insert_element_comment"))?;

        let iso = epoch_ms_to_iso(now_ms);
        Ok(ElementCommentDto {
            id: comment_id,
            tenant_id: principal.tenant_id,
            project_id: pid,
            node_id: nid,
            element_ff_id: elem,
            author,
            content,
            dom_hash,
            anchor_status: "active".to_string(),
            created_at: iso.clone(),
            updated_at: iso,
        })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn handle_update_comment(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id, comment_id)): AxumPath<(String, String, String)>,
    Json(req): Json<UpdateElementCommentInput>,
) -> Result<Json<ElementCommentDto>, Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    let cid = check_non_empty_id(&comment_id, "comment_id")?;
    if let Some(ref st) = req.anchor_status {
        if !matches!(st.as_str(), "active" | "stale" | "resolved") {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid anchorStatus",
            ));
        }
    }
    if let Some(ref c) = req.content {
        let t = c.trim();
        if t.is_empty() || t.len() > 2000 {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Comment content must be between 1 and 2000 characters",
            ));
        }
    }
    let pool = state.registry.pool().clone();

    let updated = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;

        let existing = conn
            .query_row(
                "SELECT id, tenant_id, project_id, node_id, element_ff_id, author, content,
                        dom_hash, anchor_status, created_at, updated_at
                 FROM element_comments
                 WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3 AND node_id = ?4",
                params![cid, principal.tenant_id, pid, nid],
                map_comment_row,
            )
            .optional()
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "get_comment"), "update_comment"))?;

        let Some(mut dto) = existing else {
            return Err(json_err(
                StatusCode::NOT_FOUND,
                "COMMENT_NOT_FOUND",
                format!("Comment '{cid}' was not found"),
            ));
        };

        if let Some(c) = req.content {
            dto.content = c.trim().to_string();
        }
        if let Some(st) = req.anchor_status {
            dto.anchor_status = st;
        }
        if let Some(ff) = req.element_ff_id {
            dto.element_ff_id = ff.trim().to_string();
        }
        if let Some(h) = req.dom_hash {
            dto.dom_hash = h.trim().to_string();
        }
        let now_ms = Utc::now().timestamp_millis();
        dto.updated_at = epoch_ms_to_iso(now_ms);

        retry_on_busy("update_element_comment", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE element_comments
                 SET content = ?1, anchor_status = ?2, element_ff_id = ?3, dom_hash = ?4, updated_at = ?5
                 WHERE id = ?6 AND tenant_id = ?7 AND project_id = ?8 AND node_id = ?9",
                params![
                    dto.content,
                    dto.anchor_status,
                    dto.element_ff_id,
                    dto.dom_hash,
                    now_ms,
                    cid,
                    principal.tenant_id,
                    pid,
                    nid
                ],
            )?;
            tx.commit()
        })
        .map_err(|e| map_harness_resp(e, "update_element_comment"))?;

        Ok(dto)
    })
    .await?;

    Ok(Json(updated))
}

// -----------------------------------------------------------------------------
// 2. Screen Fixtures & Viewport Preset (V6 screen_fixtures + flow_nodes)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenFixtureDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub node_id: String,
    pub variant: String,
    pub payload_json: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScreenFixtureInput {
    pub variant: String,
    #[serde(default = "default_payload_json")]
    pub payload_json: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScreenFixtureInput {
    pub payload_json: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateViewportPresetInput {
    pub viewport_preset: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateViewportPresetResponse {
    pub success: bool,
    pub viewport_preset: String,
}

fn default_payload_json() -> String {
    "{}".to_string()
}

fn map_fixture_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScreenFixtureDto> {
    let is_def: i64 = row.get(6)?;
    Ok(ScreenFixtureDto {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        project_id: row.get(2)?,
        node_id: row.get(3)?,
        variant: row.get(4)?,
        payload_json: row.get(5)?,
        is_default: is_def != 0,
        created_at: epoch_ms_to_iso(row.get(7)?),
        updated_at: epoch_ms_to_iso(row.get(8)?),
    })
}

pub async fn handle_list_fixtures(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
) -> Result<Json<Vec<ScreenFixtureDto>>, Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    let pool = state.registry.pool().clone();

    let list = run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, project_id, node_id, variant, payload_json, is_default,
                        created_at, updated_at
                 FROM screen_fixtures
                 WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3
                 ORDER BY created_at ASC",
            )
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "prep_fixtures"), "list_fixtures"))?;
        let rows = stmt
            .query_map(params![principal.tenant_id, pid, nid], map_fixture_row)
            .map_err(|e| {
                map_harness_resp(map_sqlite_error(e, "query_fixtures"), "list_fixtures")
            })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| {
                map_harness_resp(map_sqlite_error(e, "row_fixtures"), "list_fixtures")
            })?);
        }
        Ok(out)
    })
    .await?;

    Ok(Json(list))
}

pub async fn handle_create_fixture(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
    Json(req): Json<CreateScreenFixtureInput>,
) -> Result<(StatusCode, Json<ScreenFixtureDto>), Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    if !matches!(
        req.variant.as_str(),
        "DEFAULT" | "EMPTY" | "LOADING" | "ERROR"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid fixture variant",
        ));
    }
    if serde_json::from_str::<serde_json::Value>(&req.payload_json).is_err() {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "payloadJson must be valid JSON",
        ));
    }
    let pool = state.registry.pool().clone();

    let created = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let now_ms = Utc::now().timestamp_millis();
        let new_id = Uuid::new_v4().to_string();

        let saved = retry_on_busy("upsert_screen_fixture", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if req.is_default {
                tx.execute(
                    "UPDATE screen_fixtures SET is_default = 0
                     WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3",
                    params![principal.tenant_id, pid, nid],
                )?;
            }
            let existing_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM screen_fixtures
                     WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3 AND variant = ?4",
                    params![principal.tenant_id, pid, nid, req.variant],
                    |r| r.get(0),
                )
                .optional()?;

            let final_id = if let Some(ex_id) = existing_id {
                tx.execute(
                    "UPDATE screen_fixtures
                     SET payload_json = ?1, is_default = ?2, updated_at = ?3
                     WHERE id = ?4",
                    params![
                        req.payload_json,
                        if req.is_default { 1 } else { 0 },
                        now_ms,
                        ex_id
                    ],
                )?;
                ex_id
            } else {
                tx.execute(
                    "INSERT INTO screen_fixtures (
                        id, tenant_id, project_id, node_id, variant, payload_json, is_default, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        new_id,
                        principal.tenant_id,
                        pid,
                        nid,
                        req.variant,
                        req.payload_json,
                        if req.is_default { 1 } else { 0 },
                        now_ms
                    ],
                )?;
                new_id.clone()
            };
            let dto = tx.query_row(
                "SELECT id, tenant_id, project_id, node_id, variant, payload_json, is_default, created_at, updated_at
                 FROM screen_fixtures WHERE id = ?1",
                params![final_id],
                map_fixture_row,
            )?;
            tx.commit()?;
            Ok::<ScreenFixtureDto, rusqlite::Error>(dto)
        })
        .map_err(|e| map_harness_resp(e, "upsert_screen_fixture"))?;

        Ok(saved)
    })
    .await?;

    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn handle_update_fixture(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id, fixture_id)): AxumPath<(String, String, String)>,
    Json(req): Json<UpdateScreenFixtureInput>,
) -> Result<Json<ScreenFixtureDto>, Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    let fid = check_non_empty_id(&fixture_id, "fixture_id")?;
    if let Some(ref pj) = req.payload_json {
        if serde_json::from_str::<serde_json::Value>(pj).is_err() {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "payloadJson must be valid JSON",
            ));
        }
    }
    let pool = state.registry.pool().clone();

    let updated = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;

        let existing = conn
            .query_row(
                "SELECT id, tenant_id, project_id, node_id, variant, payload_json, is_default, created_at, updated_at
                 FROM screen_fixtures
                 WHERE id = ?1 AND tenant_id = ?2 AND project_id = ?3 AND node_id = ?4",
                params![fid, principal.tenant_id, pid, nid],
                map_fixture_row,
            )
            .optional()
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "get_fixture"), "update_fixture"))?;

        let Some(mut dto) = existing else {
            return Err(json_err(
                StatusCode::NOT_FOUND,
                "FIXTURE_NOT_FOUND",
                format!("Fixture '{fid}' was not found"),
            ));
        };

        if let Some(pj) = req.payload_json {
            dto.payload_json = pj;
        }
        if let Some(is_def) = req.is_default {
            dto.is_default = is_def;
        }
        let now_ms = Utc::now().timestamp_millis();
        dto.updated_at = epoch_ms_to_iso(now_ms);

        retry_on_busy("update_screen_fixture", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if dto.is_default {
                tx.execute(
                    "UPDATE screen_fixtures SET is_default = 0
                     WHERE tenant_id = ?1 AND project_id = ?2 AND node_id = ?3 AND id != ?4",
                    params![principal.tenant_id, pid, nid, fid],
                )?;
            }
            tx.execute(
                "UPDATE screen_fixtures SET payload_json = ?1, is_default = ?2, updated_at = ?3
                 WHERE id = ?4 AND tenant_id = ?5 AND project_id = ?6 AND node_id = ?7",
                params![
                    dto.payload_json,
                    if dto.is_default { 1 } else { 0 },
                    now_ms,
                    fid,
                    principal.tenant_id,
                    pid,
                    nid
                ],
            )?;
            tx.commit()
        })
        .map_err(|e| map_harness_resp(e, "update_screen_fixture"))?;

        Ok(dto)
    })
    .await?;

    Ok(Json(updated))
}

pub async fn handle_update_viewport(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
    Json(req): Json<UpdateViewportPresetInput>,
) -> Result<Json<UpdateViewportPresetResponse>, Response> {
    let pid = check_project_uuid(&id)?;
    let nid = check_non_empty_id(&node_id, "node_id")?;
    if !matches!(
        req.viewport_preset.as_str(),
        "desktop-1440" | "tablet-768" | "mobile-390"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid viewportPreset",
        ));
    }
    let preset = req.viewport_preset;
    let pool = state.registry.pool().clone();

    run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let now_ms = Utc::now().timestamp_millis();

        let changed = retry_on_busy("update_node_viewport", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let cnt = tx.execute(
                "UPDATE flow_nodes SET viewport_preset = ?1, updated_at = ?2
                 WHERE tenant_id = ?3 AND project_id = ?4 AND (id = ?5 OR screen_key = ?5)",
                params![preset, now_ms, principal.tenant_id, pid, nid],
            )?;
            tx.commit()?;
            Ok::<usize, rusqlite::Error>(cnt)
        })
        .map_err(|e| map_harness_resp(e, "update_node_viewport"))?;

        if changed == 0 {
            return Err(json_err(
                StatusCode::NOT_FOUND,
                "NODE_NOT_FOUND",
                format!("Flow node '{nid}' was not found"),
            ));
        }

        Ok(UpdateViewportPresetResponse {
            success: true,
            viewport_preset: preset,
        })
    })
    .await
    .map(Json)
}

// -----------------------------------------------------------------------------
// 3. Agent Settings (V6 agent_settings) — stores metadata + credential_ref only
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettingDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub endpoint_url: String,
    pub credential_ref: String,
    pub is_default: bool,
    pub status: String,
    pub permission_mode: String,
    pub vault_mode: String,
    pub onboarding_done: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAgentSettingInput {
    pub project_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    #[serde(default)]
    pub endpoint_url: String,
    #[serde(default)]
    pub credential_ref: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_setting_status")]
    pub status: String,
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,
    #[serde(default = "default_vault_mode")]
    pub vault_mode: String,
    #[serde(default)]
    pub onboarding_done: bool,
}

fn default_setting_status() -> String {
    "configured".to_string()
}

fn default_permission_mode() -> String {
    "ask-every-time".to_string()
}

fn default_vault_mode() -> String {
    "keychain".to_string()
}

pub(crate) fn sanitize_credential_ref(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.starts_with("env:")
        || t.starts_with("keychain:")
        || t.starts_with("vault://")
        || t.contains("****")
    {
        return t.chars().take(64).collect();
    }
    let suffix: String = t
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("redacted:****{suffix}")
}

fn map_agent_setting_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSettingDto> {
    let is_def: i64 = row.get(7)?;
    let onboarded: i64 = row.get(11)?;
    Ok(AgentSettingDto {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        project_id: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        endpoint_url: row.get(5)?,
        credential_ref: row.get(6)?,
        is_default: is_def != 0,
        status: row.get(8)?,
        permission_mode: row.get(9)?,
        vault_mode: row.get(10)?,
        onboarding_done: onboarded != 0,
        created_at: epoch_ms_to_iso(row.get(12)?),
        updated_at: epoch_ms_to_iso(row.get(13)?),
    })
}

pub async fn handle_list_agent_settings(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
) -> Result<Json<Vec<AgentSettingDto>>, Response> {
    let pool = state.registry.pool().clone();
    let rows = run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, project_id, provider_id, model_id, endpoint_url,
                        credential_ref, is_default, status, permission_mode, vault_mode,
                        onboarding_done, created_at, updated_at
                 FROM agent_settings
                 WHERE tenant_id = ?1
                 ORDER BY is_default DESC, updated_at DESC",
            )
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "prep_agent_settings"), "list"))?;
        let items = stmt
            .query_map(params![principal.tenant_id], map_agent_setting_row)
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "query_agent_settings"), "list"))?;
        let mut out = Vec::new();
        for item in items {
            out.push(item.map_err(|e| map_harness_resp(map_sqlite_error(e, "row"), "list"))?);
        }
        Ok(out)
    })
    .await?;

    Ok(Json(rows))
}

pub async fn handle_upsert_agent_setting(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Json(req): Json<UpsertAgentSettingInput>,
) -> Result<Json<AgentSettingDto>, Response> {
    let provider_id = check_non_empty_id(&req.provider_id, "providerId")?;
    let model_id = check_non_empty_id(&req.model_id, "modelId")?;
    let endpoint_trimmed = req.endpoint_url.trim();
    if !endpoint_trimmed.is_empty() {
        validate_ssrf_safe_url(endpoint_trimmed, true).map_err(|e| {
            json_err(
                StatusCode::BAD_REQUEST,
                "SSRF_VIOLATION",
                format!("endpointUrl rejected: {e}"),
            )
        })?;
    }
    let perm_mode = req.permission_mode.trim();
    if !matches!(
        perm_mode,
        "ask-every-time" | "auto_sandbox" | "ask_risky" | "ask_all" | "accept_edits" | "bypass"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid permissionMode",
        ));
    }
    let vault_mode = req.vault_mode.trim();
    if !matches!(
        vault_mode,
        "keychain" | "vault_0600" | "ram_only" | "ephemeral"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid vaultMode",
        ));
    }
    if vault_mode == "ephemeral"
        && crate::vault::VaultStore::from_workspace_root(state.registry.pool().workspace_root())
            .is_vault_writable()
    {
        return Err(json_err(
            StatusCode::CONFLICT,
            "VAULT_DOWNGRADE_FORBIDDEN",
            "Cannot downgrade credential custody to ephemeral when encrypted 0o600 vault is writable",
        ));
    }
    let proj_opt = match req.project_id {
        Some(ref p) if !p.trim().is_empty() => Some(check_project_uuid(p)?),
        _ => None,
    };
    let safe_cred_ref = sanitize_credential_ref(&req.credential_ref);
    let perm_owned = perm_mode.to_string();
    let vault_owned = vault_mode.to_string();
    let onboarding_int = if req.onboarding_done { 1i64 } else { 0i64 };
    let pool = state.registry.pool().clone();

    let saved = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        if let Some(ref pid) = proj_opt {
            verify_project_ownership(&conn, &principal.tenant_id, pid)?;
        }
        let now_ms = Utc::now().timestamp_millis();
        let new_id = Uuid::new_v4().to_string();

        let dto = retry_on_busy("upsert_agent_setting", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if req.is_default {
                tx.execute(
                    "UPDATE agent_settings SET is_default = 0 WHERE tenant_id = ?1 AND project_id IS ?2",
                    params![principal.tenant_id, proj_opt],
                )?;
            }
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM agent_settings
                     WHERE tenant_id = ?1 AND project_id IS ?2 AND provider_id = ?3",
                    params![principal.tenant_id, proj_opt, provider_id],
                    |r| r.get(0),
                )
                .optional()?;

            let final_id = if let Some(ex_id) = existing {
                tx.execute(
                    "UPDATE agent_settings
                     SET model_id = ?1, endpoint_url = ?2,
                         credential_ref = CASE WHEN ?3 = '' THEN credential_ref ELSE ?3 END,
                         is_default = ?4, status = ?5,
                         permission_mode = ?6, vault_mode = ?7, onboarding_done = ?8,
                         updated_at = ?9
                     WHERE id = ?10",
                    params![
                        model_id,
                        req.endpoint_url.trim(),
                        safe_cred_ref,
                        if req.is_default { 1 } else { 0 },
                        req.status.trim(),
                        perm_owned,
                        vault_owned,
                        onboarding_int,
                        now_ms,
                        ex_id
                    ],
                )?;
                ex_id
            } else {
                tx.execute(
                    "INSERT INTO agent_settings (
                        id, tenant_id, project_id, provider_id, model_id, endpoint_url,
                        credential_ref, is_default, status, permission_mode, vault_mode,
                        onboarding_done, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
                    params![
                        new_id,
                        principal.tenant_id,
                        proj_opt,
                        provider_id,
                        model_id,
                        req.endpoint_url.trim(),
                        safe_cred_ref,
                        if req.is_default { 1 } else { 0 },
                        req.status.trim(),
                        perm_owned,
                        vault_owned,
                        onboarding_int,
                        now_ms
                    ],
                )?;
                new_id.clone()
            };

            let row = tx.query_row(
                "SELECT id, tenant_id, project_id, provider_id, model_id, endpoint_url,
                        credential_ref, is_default, status, permission_mode, vault_mode,
                        onboarding_done, created_at, updated_at
                 FROM agent_settings WHERE id = ?1",
                params![final_id],
                map_agent_setting_row,
            )?;
            tx.commit()?;
            Ok::<AgentSettingDto, rusqlite::Error>(row)
        })
        .map_err(|e| map_harness_resp(e, "upsert_agent_setting"))?;

        Ok(dto)
    })
    .await?;

    Ok(Json(saved))
}

// -----------------------------------------------------------------------------
// 4. MCP Servers (V6 mcp_servers)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: Option<String>,
    pub server_slug: String,
    pub display_name: String,
    pub transport_kind: String,
    pub command_or_url: String,
    pub permission_tier: String,
    pub enabled: bool,
    pub last_health_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertMcpServerInput {
    pub project_id: Option<String>,
    pub server_slug: String,
    pub display_name: String,
    #[serde(default = "default_transport_kind")]
    pub transport_kind: String,
    pub command_or_url: String,
    #[serde(default = "default_permission_tier")]
    pub permission_tier: String,
    #[serde(default = "default_enabled_true")]
    pub enabled: bool,
}

fn default_transport_kind() -> String {
    "stdio".to_string()
}

fn default_permission_tier() -> String {
    "read_only".to_string()
}

fn default_enabled_true() -> bool {
    true
}

fn map_mcp_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<McpServerDto> {
    let en: i64 = row.get(8)?;
    Ok(McpServerDto {
        id: row.get(0)?,
        tenant_id: row.get(1)?,
        project_id: row.get(2)?,
        server_slug: row.get(3)?,
        display_name: row.get(4)?,
        transport_kind: row.get(5)?,
        command_or_url: row.get(6)?,
        permission_tier: row.get(7)?,
        enabled: en != 0,
        last_health_status: row.get(9)?,
        created_at: epoch_ms_to_iso(row.get(10)?),
        updated_at: epoch_ms_to_iso(row.get(11)?),
    })
}

pub async fn handle_list_mcp_servers(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
) -> Result<Json<Vec<McpServerDto>>, Response> {
    let pool = state.registry.pool().clone();
    let list = run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, project_id, server_slug, display_name, transport_kind,
                        command_or_url, permission_tier, enabled, last_health_status,
                        created_at, updated_at
                 FROM mcp_servers
                 WHERE tenant_id = ?1
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "prep_mcp"), "list_mcp"))?;
        let rows = stmt
            .query_map(params![principal.tenant_id], map_mcp_row)
            .map_err(|e| map_harness_resp(map_sqlite_error(e, "query_mcp"), "list_mcp"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_harness_resp(map_sqlite_error(e, "row_mcp"), "list_mcp"))?);
        }
        Ok(out)
    })
    .await?;

    Ok(Json(list))
}

pub const ALLOWED_LOOPBACK_PORTS: &[u16] = &[11434, 1234, 8000, 8080, 5000];

fn validate_ipv4_ssrf(
    v4: std::net::Ipv4Addr,
    port: Option<u16>,
    allow_loopback_policy: bool,
    context: &str,
) -> Result<(), String> {
    let octets = v4.octets();
    if octets[0] == 0 {
        return Err(format!(
            "Reserved / this-network {context}target '{v4}' is blocked (SSRF prevention)"
        ));
    }
    if v4.is_link_local() || (octets[0] == 169 && octets[1] == 254) {
        return Err(format!(
            "Link-local / cloud-metadata {context}target '{v4}' is blocked (SSRF prevention)"
        ));
    }
    if v4.is_private() {
        return Err(format!(
            "Private RFC1918 {context}target '{v4}' is blocked (SSRF prevention)"
        ));
    }
    if v4.is_broadcast() || v4.is_unspecified() {
        return Err(format!(
            "Broadcast or unspecified {context}target '{v4}' is blocked"
        ));
    }
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return Err(format!(
            "Carrier-grade NAT {context}target '{v4}' is blocked (SSRF prevention)"
        ));
    }
    if v4.is_loopback() {
        if !allow_loopback_policy {
            return Err(format!(
                "Loopback {context}target '{v4}' is blocked by security policy"
            ));
        }
        let p = port.ok_or_else(|| {
            format!("Loopback {context}target requires an explicit port in allowlist")
        })?;
        if !ALLOWED_LOOPBACK_PORTS.contains(&p) {
            return Err(format!(
                "Loopback port {p} is not in allowed local Ollama/CLI port list: {ALLOWED_LOOPBACK_PORTS:?}"
            ));
        }
    }
    Ok(())
}

pub fn check_ssrf_socket_addr(addr: SocketAddr, allow_loopback_policy: bool) -> Result<(), String> {
    validate_ip_ssrf(addr.ip(), Some(addr.port()), allow_loopback_policy)
}

fn validate_ip_ssrf(
    ip: IpAddr,
    port: Option<u16>,
    allow_loopback_policy: bool,
) -> Result<(), String> {
    match ip {
        IpAddr::V4(v4) => validate_ipv4_ssrf(v4, port, allow_loopback_policy, "IPv4 "),
        IpAddr::V6(v6) => {
            if v6.is_loopback() {
                if !allow_loopback_policy {
                    return Err(format!(
                        "Loopback IPv6 target '{v6}' is blocked by security policy"
                    ));
                }
                let p = port.ok_or_else(|| {
                    "Loopback target requires an explicit port in allowlist".to_string()
                })?;
                if !ALLOWED_LOOPBACK_PORTS.contains(&p) {
                    return Err(format!(
                        "Loopback port {p} is not in allowed local Ollama/CLI port list: {ALLOWED_LOOPBACK_PORTS:?}"
                    ));
                }
            } else if v6.is_unspecified() {
                return Err(format!("Unspecified IPv6 target '{v6}' is blocked"));
            } else {
                let segments = v6.segments();
                let is_v4_embedded = segments[0] == 0
                    && segments[1] == 0
                    && segments[2] == 0
                    && segments[3] == 0
                    && segments[4] == 0
                    && (segments[5] == 0xffff || segments[5] == 0);

                if is_v4_embedded {
                    if let Some(v4) = v6.to_ipv4() {
                        validate_ipv4_ssrf(
                            v4,
                            port,
                            allow_loopback_policy,
                            "mapped/compatible IPv4 ",
                        )?;
                    }
                } else {
                    if (segments[0] & 0xffc0) == 0xfe80 {
                        return Err(format!(
                            "Link-local IPv6 target '{v6}' is blocked (SSRF prevention)"
                        ));
                    }
                    if (segments[0] & 0xfe00) == 0xfc00 {
                        return Err(format!(
                            "Unique-local private IPv6 target '{v6}' is blocked (SSRF prevention)"
                        ));
                    }
                }
            }
            Ok(())
        }
    }
}

pub fn validate_ssrf_safe_url(
    raw_url: &str,
    allow_loopback_policy: bool,
) -> Result<String, String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("URL contains whitespace or control characters".to_string());
    }

    let uri = trimmed
        .parse::<axum::http::Uri>()
        .map_err(|e| format!("Invalid URL syntax: {e}"))?;

    let scheme = uri
        .scheme_str()
        .ok_or_else(|| "URL missing scheme (http:// or https:// required)".to_string())?
        .to_ascii_lowercase();

    if scheme != "http" && scheme != "https" {
        return Err(format!(
            "Forbidden scheme '{scheme}': only http and https are permitted"
        ));
    }

    let raw_host = uri
        .host()
        .ok_or_else(|| "URL missing host authority".to_string())?;

    let host_clean = raw_host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();

    if host_clean.is_empty() {
        return Err("URL host cannot be empty".to_string());
    }

    // Block cloud metadata endpoints
    if host_clean == "169.254.169.254"
        || host_clean == "metadata.google.internal"
        || host_clean == "metadata.google"
        || host_clean == "instance-data"
        || host_clean == "100.100.100.200"
        || host_clean == "fd00:ec2::254"
    {
        return Err(format!(
            "Access to cloud metadata endpoint '{host_clean}' is strictly blocked (SSRF prevention)"
        ));
    }

    let host_no_dot = host_clean.trim_end_matches('.');
    for blocked_suffix in [
        ".nip.io",
        ".xip.io",
        ".sslip.io",
        ".localtest.me",
        ".internal",
        ".local",
        ".localhost.localdomain",
    ] {
        if host_no_dot == blocked_suffix.trim_start_matches('.')
            || host_no_dot.ends_with(blocked_suffix)
        {
            return Err(format!(
                "DNS-rebinding or internal domain '{host_clean}' is blocked (SSRF prevention)"
            ));
        }
    }

    let port = uri.port_u16();

    if let Ok(ip) = host_clean.parse::<IpAddr>() {
        validate_ip_ssrf(ip, port, allow_loopback_policy)?;
    } else if host_clean == "localhost" || host_clean.ends_with(".localhost") {
        if !allow_loopback_policy {
            return Err(format!(
                "Localhost target '{host_clean}' is blocked by security policy"
            ));
        }
        let p = port
            .ok_or_else(|| "Localhost target requires an explicit port in allowlist".to_string())?;
        if !ALLOWED_LOOPBACK_PORTS.contains(&p) {
            return Err(format!(
                "Localhost port {p} is not in allowed local Ollama/CLI port list: {ALLOWED_LOOPBACK_PORTS:?}"
            ));
        }
    } else {
        // Best-effort synchronous DNS check for public hostnames (catches records resolving to internal/loopback IPs)
        let eff_port = port.unwrap_or(if scheme == "https" { 443 } else { 80 });
        if let Ok(addrs) = (host_no_dot, eff_port).to_socket_addrs() {
            for sa in addrs {
                check_ssrf_socket_addr(sa, allow_loopback_policy)?;
            }
        }
    }

    Ok(trimmed.to_string())
}

const ALLOWED_STDIO_RUNNERS: &[&str] = &[
    "npx", "uvx", "uv", "node", "deno", "bun", "python", "python3", "docker", "podman", "cargo",
];

fn validate_mcp_target(transport_kind: &str, raw_target: &str) -> Result<String, Response> {
    let target = check_non_empty_id(raw_target, "commandOrUrl")?;
    if target.chars().any(|c| c.is_control()) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "commandOrUrl cannot contain control characters",
        ));
    }
    if transport_kind == "http_sse" {
        validate_ssrf_safe_url(&target, true).map_err(|e| {
            json_err(
                StatusCode::BAD_REQUEST,
                "SSRF_VIOLATION",
                format!("http_sse URL rejected: {e}"),
            )
        })?;
    } else {
        if [";", "&&", "||", "|", "`", "$(", "${", ">", "<", "&"]
            .iter()
            .any(|ch| target.contains(ch))
        {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "stdio commandOrUrl contains disallowed shell metacharacters",
            ));
        }
        let tokens: Vec<&str> = target.split_whitespace().collect();
        let Some(first_tok) = tokens.first().copied() else {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "stdio commandOrUrl cannot be empty",
            ));
        };
        let bin_name = std::path::Path::new(first_tok)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(first_tok);
        let is_allowed_runner = ALLOWED_STDIO_RUNNERS.contains(&bin_name)
            || bin_name.starts_with("mcp-server-")
            || bin_name.starts_with("mcp_");
        if !is_allowed_runner {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                format!("stdio runner '{bin_name}' is not in allowed MCP launcher allowlist"),
            ));
        }
        if tokens
            .iter()
            .skip(1)
            .any(|arg| matches!(*arg, "-c" | "--eval" | "-e" | "/c"))
        {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "stdio inline code execution flags (-c, -e, --eval, /c) are blocked",
            ));
        }
    }
    Ok(target)
}

pub async fn handle_upsert_mcp_server(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Json(req): Json<UpsertMcpServerInput>,
) -> Result<(StatusCode, Json<McpServerDto>), Response> {
    let slug = check_non_empty_id(&req.server_slug, "serverSlug")?;
    let name = check_non_empty_id(&req.display_name, "displayName")?;
    if !matches!(req.transport_kind.as_str(), "stdio" | "http_sse") {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "transportKind must be 'stdio' or 'http_sse'",
        ));
    }
    let target = validate_mcp_target(&req.transport_kind, &req.command_or_url)?;
    let perm_tier = req.permission_tier.trim().to_string();
    if !matches!(
        perm_tier.as_str(),
        "read_only" | "ask" | "auto_sandbox" | "admin"
    ) {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "permissionTier must be 'read_only', 'ask', 'auto_sandbox', or 'admin'",
        ));
    }
    let proj_opt = match req.project_id {
        Some(ref p) if !p.trim().is_empty() => Some(check_project_uuid(p)?),
        _ => None,
    };
    let pool = state.registry.pool().clone();

    let saved = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        if let Some(ref pid) = proj_opt {
            verify_project_ownership(&conn, &principal.tenant_id, pid)?;
        }
        let now_ms = Utc::now().timestamp_millis();
        let new_id = Uuid::new_v4().to_string();

        let dto = retry_on_busy("upsert_mcp_server", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let existing: Option<String> = tx
                .query_row(
                    "SELECT id FROM mcp_servers
                     WHERE tenant_id = ?1 AND project_id IS ?2 AND server_slug = ?3",
                    params![principal.tenant_id, proj_opt, slug],
                    |r| r.get(0),
                )
                .optional()?;

            let final_id = if let Some(ex_id) = existing {
                tx.execute(
                    "UPDATE mcp_servers
                     SET display_name = ?1, transport_kind = ?2, command_or_url = ?3,
                         permission_tier = ?4, enabled = ?5, last_health_status = 'configured',
                         updated_at = ?6
                     WHERE id = ?7",
                    params![
                        name,
                        req.transport_kind,
                        target,
                        perm_tier,
                        if req.enabled { 1 } else { 0 },
                        now_ms,
                        ex_id
                    ],
                )?;
                ex_id
            } else {
                tx.execute(
                    "INSERT INTO mcp_servers (
                        id, tenant_id, project_id, server_slug, display_name, transport_kind,
                        command_or_url, permission_tier, enabled, last_health_status,
                        created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'configured', ?10, ?10)",
                    params![
                        new_id,
                        principal.tenant_id,
                        proj_opt,
                        slug,
                        name,
                        req.transport_kind,
                        target,
                        perm_tier,
                        if req.enabled { 1 } else { 0 },
                        now_ms
                    ],
                )?;
                new_id.clone()
            };
            let row = tx.query_row(
                "SELECT id, tenant_id, project_id, server_slug, display_name, transport_kind,
                        command_or_url, permission_tier, enabled, last_health_status,
                        created_at, updated_at
                 FROM mcp_servers WHERE id = ?1",
                params![final_id],
                map_mcp_row,
            )?;
            tx.commit()?;
            Ok::<McpServerDto, rusqlite::Error>(row)
        })
        .map_err(|e| map_harness_resp(e, "upsert_mcp_server"))?;

        Ok(dto)
    })
    .await?;

    Ok((StatusCode::CREATED, Json(saved)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMcpResponse {
    pub deleted: bool,
    pub id: String,
}

pub async fn handle_delete_mcp_server(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(server_id): AxumPath<String>,
) -> Result<Json<DeleteMcpResponse>, Response> {
    let sid = check_non_empty_id(&server_id, "server_id")?;
    let pool = state.registry.pool().clone();
    let count = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        retry_on_busy("delete_mcp_server", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let c = tx.execute(
                "DELETE FROM mcp_servers WHERE id = ?1 AND tenant_id = ?2",
                params![sid, principal.tenant_id],
            )?;
            tx.commit()?;
            Ok::<usize, rusqlite::Error>(c)
        })
        .map_err(|e| map_harness_resp(e, "delete_mcp_server"))
    })
    .await?;

    if count == 0 {
        return Err(json_err(
            StatusCode::NOT_FOUND,
            "MCP_SERVER_NOT_FOUND",
            format!("MCP server '{server_id}' was not found"),
        ));
    }

    Ok(Json(DeleteMcpResponse {
        deleted: true,
        id: server_id.trim().to_string(),
    }))
}

// -----------------------------------------------------------------------------
// 5. Execution Job Events & SSE Stream (V4 node_execution_jobs + V6 execution_events)
// -----------------------------------------------------------------------------

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
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamJobEventsQuery {
    pub job_id: Option<String>,
    pub node_id: Option<String>,
    pub after_seq: Option<i64>,
}

pub async fn handle_post_job_event(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<PostJobEventInput>,
) -> Result<(StatusCode, Json<ExecutionEventDto>), Response> {
    let pid = check_project_uuid(&id)?;
    let job_id = check_non_empty_id(&req.job_id, "jobId")?;
    let node_id = check_non_empty_id(&req.node_id, "nodeId")?;
    let event_kind = check_non_empty_id(&req.event_kind, "eventKind")?;
    let next_status = match req.job_status.as_deref() {
        Some(
            st @ ("QUEUED" | "RUNNING" | "PAUSED" | "INTERRUPTED" | "COMPLETED" | "FAILED"
            | "CANCELLED"),
        ) => Some(st.to_string()),
        Some(_) => {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid jobStatus",
            ))
        }
        None => None,
    };
    let session_id_req = req
        .session_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("sess-{pid}"));
    let scope_key = format!("{pid}::{session_id_req}::{node_id}");
    let pool = state.registry.pool().clone();

    let created = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let now_ms = Utc::now().timestamp_millis();
        let event_id = Uuid::new_v4().to_string();

        let mut cross_scope_err: Option<(StatusCode, &'static str, String)> = None;
        let outcome = retry_on_busy("append_execution_event", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let existing_sess: Option<(String, String)> = tx
                .query_row(
                    "SELECT project_id, tenant_id FROM chat_sessions WHERE id = ?1",
                    params![session_id_req],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            match existing_sess {
                Some((_, sess_tid)) if sess_tid != principal.tenant_id => {
                    cross_scope_err = Some((
                        StatusCode::FORBIDDEN,
                        "TENANT_ISOLATION_BREACH",
                        "Session belongs to another tenant".to_string(),
                    ));
                    return Ok(None);
                }
                Some((sess_pid, _)) if sess_pid != pid => {
                    cross_scope_err = Some((
                        StatusCode::NOT_FOUND,
                        "PROJECT_NOT_FOUND",
                        format!("Session '{session_id_req}' does not belong to project '{pid}'"),
                    ));
                    return Ok(None);
                }
                Some(_) => {}
                None => {
                    tx.execute(
                        "INSERT INTO chat_sessions (id, tenant_id, project_id, title, model_provider, created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'Execution Stream Session', 'default', ?4, ?4)",
                        params![session_id_req, principal.tenant_id, pid, now_ms],
                    )?;
                }
            }

            let cur_job: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT project_id, tenant_id, status FROM node_execution_jobs WHERE id = ?1",
                    params![job_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;

            let eff_status = match cur_job {
                Some((_, job_tid, _)) if job_tid != principal.tenant_id => {
                    cross_scope_err = Some((
                        StatusCode::FORBIDDEN,
                        "TENANT_ISOLATION_BREACH",
                        "Job belongs to another tenant".to_string(),
                    ));
                    return Ok(None);
                }
                Some((job_pid, _, _)) if job_pid != pid => {
                    cross_scope_err = Some((
                        StatusCode::NOT_FOUND,
                        "PROJECT_NOT_FOUND",
                        format!("Job '{job_id}' does not belong to project '{pid}'"),
                    ));
                    return Ok(None);
                }
                Some((_, _, cur)) => {
                    let target = next_status.clone().unwrap_or(cur);
                    tx.execute(
                        "UPDATE node_execution_jobs
                         SET status = ?1,
                             fencing_token = fencing_token + 1,
                             composite_scope_key = CASE WHEN composite_scope_key = '' THEN ?2 ELSE composite_scope_key END,
                             started_at = COALESCE(started_at, ?3),
                             completed_at = CASE WHEN ?1 IN ('COMPLETED', 'FAILED', 'CANCELLED') THEN ?3 ELSE completed_at END,
                             updated_at = ?3
                         WHERE id = ?4 AND project_id = ?5 AND tenant_id = ?6",
                        params![target, scope_key, now_ms, job_id, pid, principal.tenant_id],
                    )?;
                    target
                }
                None => {
                    let target = next_status
                        .clone()
                        .unwrap_or_else(|| "RUNNING".to_string());
                    tx.execute(
                        "INSERT INTO node_execution_jobs (
                            id, tenant_id, project_id, session_id, node_id, composite_scope_key,
                            daemon_nonce, status, fencing_token, priority,
                            created_at, started_at, completed_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7, 1, 10, ?8, ?8, CASE WHEN ?7 IN ('COMPLETED', 'FAILED', 'CANCELLED') THEN ?8 ELSE NULL END, ?8)",
                        params![
                            job_id,
                            principal.tenant_id,
                            pid,
                            session_id_req,
                            node_id,
                            scope_key,
                            target,
                            now_ms
                        ],
                    )?;
                    target
                }
            };

            let max_seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(seq_no), 0) FROM execution_events WHERE job_id = ?1",
                params![job_id],
                |r| r.get(0),
            )?;
            let max_proj_seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(project_seq), 0) FROM execution_events WHERE project_id = ?1",
                params![pid],
                |r| r.get(0),
            )?;
            let next_seq = max_seq + 1;
            let next_proj_seq = max_proj_seq + 1;
            tx.execute(
                "INSERT INTO execution_events (
                    id, tenant_id, project_id, job_id, node_id, composite_scope_key,
                    event_kind, payload_text, seq_no, project_seq, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    event_id,
                    principal.tenant_id,
                    pid,
                    job_id,
                    node_id,
                    scope_key,
                    event_kind,
                    req.payload_text,
                    next_seq,
                    next_proj_seq,
                    now_ms
                ],
            )?;
            tx.commit()?;
            Ok::<Option<(i64, i64, String)>, rusqlite::Error>(Some((next_seq, next_proj_seq, eff_status)))
        })
        .map_err(|e| map_harness_resp(e, "append_execution_event"))?;
        let Some((seq_no, project_seq, final_status)) = outcome else {
            let (st, code, msg) = cross_scope_err.unwrap_or((
                StatusCode::NOT_FOUND,
                "PROJECT_NOT_FOUND",
                format!("Resource does not belong to project '{pid}'"),
            ));
            return Err(json_err(st, code, msg));
        };

        Ok(ExecutionEventDto {
            id: event_id,
            job_id,
            project_id: pid,
            node_id,
            event_kind,
            payload_text: req.payload_text,
            seq_no,
            project_seq,
            composite_scope_key: scope_key,
            job_status: final_status,
            created_at: epoch_ms_to_iso(now_ms),
        })
    })
    .await?;

    Ok((StatusCode::CREATED, Json(created)))
}

struct SsePollState {
    pool: SqlitePool,
    tenant_id: String,
    project_id: String,
    job_id: Option<String>,
    node_id: Option<String>,
    last_seq: i64,
    ticks: u32,
    closed: bool,
}

fn fetch_pending_events(
    state: &SsePollState,
) -> Result<(Vec<ExecutionEventDto>, bool), HarnessError> {
    let conn = state.pool.connection()?;
    let mut sql = String::from(
        "SELECT e.id, e.job_id, e.project_id, e.node_id, e.event_kind, e.payload_text,
                e.seq_no, COALESCE(e.project_seq, e.seq_no) AS eff_proj_seq,
                e.composite_scope_key, COALESCE(j.status, 'RUNNING'), e.created_at
         FROM execution_events e
         LEFT JOIN node_execution_jobs j ON j.id = e.job_id
         WHERE e.tenant_id = ?1 AND e.project_id = ?2
           AND COALESCE(e.project_seq, e.seq_no) > ?3",
    );
    if state.job_id.is_some() {
        sql.push_str(" AND e.job_id = ?4");
    } else if state.node_id.is_some() {
        sql.push_str(" AND e.node_id = ?4");
    }
    sql.push_str(" ORDER BY eff_proj_seq ASC, e.created_at ASC LIMIT 50");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| map_sqlite_error(e, "prep_sse_events"))?;

    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<ExecutionEventDto> {
        Ok(ExecutionEventDto {
            id: row.get(0)?,
            job_id: row.get(1)?,
            project_id: row.get(2)?,
            node_id: row.get(3)?,
            event_kind: row.get(4)?,
            payload_text: row.get(5)?,
            seq_no: row.get(6)?,
            project_seq: row.get(7)?,
            composite_scope_key: row.get(8)?,
            job_status: row.get(9)?,
            created_at: epoch_ms_to_iso(row.get(10)?),
        })
    };

    let rows = if let Some(ref jid) = state.job_id {
        stmt.query_map(
            params![state.tenant_id, state.project_id, state.last_seq, jid],
            map_row,
        )
    } else if let Some(ref nid) = state.node_id {
        stmt.query_map(
            params![state.tenant_id, state.project_id, state.last_seq, nid],
            map_row,
        )
    } else {
        stmt.query_map(
            params![state.tenant_id, state.project_id, state.last_seq],
            map_row,
        )
    }
    .map_err(|e| map_sqlite_error(e, "query_sse_events"))?;

    let mut events = Vec::new();
    for r in rows {
        events.push(r.map_err(|e| map_sqlite_error(e, "row_sse_events"))?);
    }

    let is_terminal = if let Some(ref jid) = state.job_id {
        conn.query_row(
            "SELECT status FROM node_execution_jobs WHERE id = ?1 AND project_id = ?2",
            params![jid, state.project_id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "sse_job_status"))?
        .is_some_and(|st| matches!(st.as_str(), "COMPLETED" | "FAILED" | "CANCELLED"))
    } else {
        false
    };

    Ok((events, is_terminal))
}

pub async fn handle_stream_job_events(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Query(q): Query<StreamJobEventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Response> {
    let pid = check_project_uuid(&id)?;
    let pool = state.registry.pool().clone();
    let tid = principal.tenant_id.clone();
    let pid_check = pid.clone();

    run_db(move || {
        let conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &tid, &pid_check)
    })
    .await?;

    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<i64>().ok());

    let init = SsePollState {
        pool: state.registry.pool().clone(),
        tenant_id: principal.tenant_id,
        project_id: pid,
        job_id: q.job_id.filter(|s| !s.trim().is_empty()),
        node_id: q.node_id.filter(|s| !s.trim().is_empty()),
        last_seq: q.after_seq.or(last_event_id).unwrap_or(0),
        ticks: 0,
        closed: false,
    };

    let max_ticks: u32 = std::env::var("COWOK_SSE_MAX_TICKS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(250);

    let stream = futures_util::stream::unfold(init, move |mut st| async move {
        if st.closed || st.ticks >= max_ticks {
            return None;
        }
        loop {
            if st.ticks > 0 {
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
            st.ticks += 1;

            let snapshot = SsePollState {
                pool: st.pool.clone(),
                tenant_id: st.tenant_id.clone(),
                project_id: st.project_id.clone(),
                job_id: st.job_id.clone(),
                node_id: st.node_id.clone(),
                last_seq: st.last_seq,
                ticks: st.ticks,
                closed: st.closed,
            };
            let res = tokio::task::spawn_blocking(move || fetch_pending_events(&snapshot)).await;
            match res {
                Ok(Ok((events, is_terminal))) => {
                    if let Some(last) = events.last() {
                        st.last_seq = last.project_seq.max(last.seq_no);
                    }
                    if !events.is_empty() {
                        if is_terminal {
                            st.closed = true;
                        }
                        let payload =
                            serde_json::to_string(&events).unwrap_or_else(|_| "[]".into());
                        let ev = Event::default()
                            .id(st.last_seq.to_string())
                            .event("execution_events")
                            .data(payload);
                        return Some((Ok(ev), st));
                    }
                    if is_terminal || st.ticks >= max_ticks {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// -----------------------------------------------------------------------------
// 6. Provider Key Verification (`POST /api/v1/doctor/verify-key`) -> Extracted to doctor_vault.rs (Story 04.3 Gate H6)
// -----------------------------------------------------------------------------

pub use crate::routes::doctor_vault::{
    handle_verify_provider_key, VerifyKeyRequest, VerifyKeyResponse,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssrf_blocks_cloud_metadata() {
        assert!(validate_ssrf_safe_url("http://169.254.169.254/latest/meta-data", true).is_err());
        assert!(validate_ssrf_safe_url("http://169.254.169.254", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::169.254.169.254]/", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::ffff:169.254.169.254]/", true).is_err());
        assert!(validate_ssrf_safe_url(
            "http://metadata.google.internal/computeMetadata/v1/",
            true
        )
        .is_err());
        assert!(validate_ssrf_safe_url("http://instance-data", true).is_err());
        assert!(validate_ssrf_safe_url("http://100.100.100.200/latest/meta-data", true).is_err());
        assert!(validate_ssrf_safe_url("http://[fd00:ec2::254]/latest/meta-data", true).is_err());
    }

    #[test]
    fn test_ssrf_blocks_rfc1918_private_ips() {
        assert!(validate_ssrf_safe_url("http://10.0.0.1:8080/v1", true).is_err());
        assert!(validate_ssrf_safe_url("http://192.168.1.1/api", true).is_err());
        assert!(validate_ssrf_safe_url("http://172.16.0.1/", true).is_err());
        assert!(validate_ssrf_safe_url("http://172.31.255.255:8000/", true).is_err());
        assert!(validate_ssrf_safe_url("http://100.64.0.1:8080/", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::ffff:100.64.0.1]/", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::ffff:10.0.0.1]:8080/", true).is_err());
    }

    #[test]
    fn test_ssrf_loopback_policy_with_port_allowlist() {
        // Allowed loopback ports: 11434, 1234, 8000, 8080, 5000
        assert!(validate_ssrf_safe_url("http://127.0.0.1:11434/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://localhost:11434/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://127.0.0.1:1234/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://localhost:8000/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://127.0.0.1:8080/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://127.0.0.1:5000/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://[::1]:11434/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("http://[::ffff:127.0.0.1]:11434/v1", true).is_ok());

        // Disallowed loopback ports
        assert!(validate_ssrf_safe_url("http://127.0.0.1:22/", true).is_err());
        assert!(validate_ssrf_safe_url("http://localhost:6379/", true).is_err());
        assert!(validate_ssrf_safe_url("http://127.0.0.1:5432/", true).is_err());
        assert!(validate_ssrf_safe_url("http://127.0.0.1/", true).is_err());
        assert!(validate_ssrf_safe_url("http://localhost/", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::1]:22/", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::1]/", true).is_err());
        assert!(validate_ssrf_safe_url("http://[::ffff:127.0.0.1]:22/", true).is_err());

        // When loopback is disallowed entirely
        assert!(validate_ssrf_safe_url("http://127.0.0.1:11434/v1", false).is_err());
        assert!(validate_ssrf_safe_url("http://localhost:11434/v1", false).is_err());
        assert!(validate_ssrf_safe_url("http://[::ffff:127.0.0.1]:11434/v1", false).is_err());
    }

    #[test]
    fn test_ssrf_allows_legitimate_public_urls() {
        assert!(validate_ssrf_safe_url("https://api.openai.com/v1", true).is_ok());
        assert!(validate_ssrf_safe_url("https://api.anthropic.com/v1/messages", true).is_ok());
        assert!(validate_ssrf_safe_url("https://generativelanguage.googleapis.com", true).is_ok());
        assert!(validate_ssrf_safe_url("http://api.deepseek.com/v1", true).is_ok());
    }

    #[test]
    fn test_stdio_command_hardening() {
        // Safe commands
        assert!(validate_mcp_target(
            "stdio",
            "npx -y @modelcontextprotocol/server-filesystem /tmp"
        )
        .is_ok());
        assert!(validate_mcp_target("stdio", "uvx mcp-server-sqlite --db /tmp/test.db").is_ok());
        assert!(validate_mcp_target("stdio", "python3 -m my_mcp_server").is_ok());

        // Block shell injection / chaining / redirection
        assert!(validate_mcp_target("stdio", "cat /etc/passwd; rm -rf /").is_err());
        assert!(validate_mcp_target("stdio", "echo hello && evil").is_err());
        assert!(validate_mcp_target("stdio", "echo hello & evil").is_err());
        assert!(validate_mcp_target("stdio", "cmd | evil").is_err());
        assert!(validate_mcp_target("stdio", "eval $(whoami)").is_err());
        assert!(validate_mcp_target("stdio", "echo ${SECRET}").is_err());
        assert!(validate_mcp_target("stdio", "cat < /etc/shadow").is_err());
        assert!(validate_mcp_target("stdio", "echo evil > /tmp/bad").is_err());
        assert!(validate_mcp_target("stdio", "`whoami`").is_err());
    }

    #[test]
    fn test_mcp_http_sse_ssrf_validation() {
        assert!(validate_mcp_target("http_sse", "http://127.0.0.1:11434/sse").is_ok());
        assert!(validate_mcp_target("http_sse", "https://mcp.example.com/sse").is_ok());
        assert!(validate_mcp_target("http_sse", "http://169.254.169.254/sse").is_err());
        assert!(validate_mcp_target("http_sse", "http://10.0.0.1:8080/sse").is_err());
        assert!(validate_mcp_target("http_sse", "http://127.0.0.1:22/sse").is_err());
    }
}
