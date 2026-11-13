// @story 04.2
// @implements V6 SQLite Workbench Endpoints (Comments, Fixtures, Viewport, Settings, MCP, Job SSE, Key Verify)

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::StatusCode,
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

fn epoch_ms_to_iso(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339())
}

fn json_err(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
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

fn check_non_empty_id(raw: &str, label: &str) -> Result<String, Response> {
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

async fn run_db<T, F>(op: F) -> Result<T, Response>
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

fn map_harness_resp(err: HarnessError, op: &str) -> Response {
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
}

fn default_setting_status() -> String {
    "configured".to_string()
}

fn sanitize_credential_ref(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.starts_with("env:") || t.starts_with("keychain:") || t.contains("****") {
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
        created_at: epoch_ms_to_iso(row.get(9)?),
        updated_at: epoch_ms_to_iso(row.get(10)?),
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
                        credential_ref, is_default, status, created_at, updated_at
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
    let proj_opt = match req.project_id {
        Some(ref p) if !p.trim().is_empty() => Some(check_project_uuid(p)?),
        _ => None,
    };
    let safe_cred_ref = sanitize_credential_ref(&req.credential_ref);
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
                         is_default = ?4, status = ?5, updated_at = ?6
                     WHERE id = ?7",
                    params![
                        model_id,
                        req.endpoint_url.trim(),
                        safe_cred_ref,
                        if req.is_default { 1 } else { 0 },
                        req.status.trim(),
                        now_ms,
                        ex_id
                    ],
                )?;
                ex_id
            } else {
                tx.execute(
                    "INSERT INTO agent_settings (
                        id, tenant_id, project_id, provider_id, model_id, endpoint_url,
                        credential_ref, is_default, status, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
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
                        now_ms
                    ],
                )?;
                new_id.clone()
            };

            let row = tx.query_row(
                "SELECT id, tenant_id, project_id, provider_id, model_id, endpoint_url,
                        credential_ref, is_default, status, created_at, updated_at
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

pub async fn handle_upsert_mcp_server(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Json(req): Json<UpsertMcpServerInput>,
) -> Result<(StatusCode, Json<McpServerDto>), Response> {
    let slug = check_non_empty_id(&req.server_slug, "serverSlug")?;
    let name = check_non_empty_id(&req.display_name, "displayName")?;
    let target = check_non_empty_id(&req.command_or_url, "commandOrUrl")?;
    if !matches!(req.transport_kind.as_str(), "stdio" | "http_sse") {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "transportKind must be 'stdio' or 'http_sse'",
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
                        req.permission_tier.trim(),
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
                        req.permission_tier.trim(),
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
    let deleted = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        let count = retry_on_busy("delete_mcp_server", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let c = tx.execute(
                "DELETE FROM mcp_servers WHERE id = ?1 AND tenant_id = ?2",
                params![sid, principal.tenant_id],
            )?;
            tx.commit()?;
            Ok::<usize, rusqlite::Error>(c)
        })
        .map_err(|e| map_harness_resp(e, "delete_mcp_server"))?;
        Ok(DeleteMcpResponse {
            deleted: count > 0,
            id: sid,
        })
    })
    .await?;

    Ok(Json(deleted))
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
        Some(st @ ("QUEUED" | "RUNNING" | "COMPLETED" | "FAILED" | "CANCELLED")) => {
            Some(st.to_string())
        }
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
    let pool = state.registry.pool().clone();

    let created = run_db(move || {
        let mut conn = pool.connection().map_err(|e| map_harness_resp(e, "conn"))?;
        verify_project_ownership(&conn, &principal.tenant_id, &pid)?;
        let now_ms = Utc::now().timestamp_millis();
        let event_id = Uuid::new_v4().to_string();

        let (seq_no, final_status) = retry_on_busy("append_execution_event", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let existing_sess: Option<String> = tx
                .query_row(
                    "SELECT id FROM chat_sessions WHERE id = ?1",
                    params![session_id_req],
                    |r| r.get(0),
                )
                .optional()?;
            if existing_sess.is_none() {
                tx.execute(
                    "INSERT INTO chat_sessions (id, project_id, title, model_provider, created_at, updated_at)
                     VALUES (?1, ?2, 'Execution Stream Session', 'default', ?3, ?3)",
                    params![session_id_req, pid, now_ms],
                )?;
            }

            let cur_status: Option<String> = tx
                .query_row(
                    "SELECT status FROM node_execution_jobs WHERE id = ?1 AND project_id = ?2",
                    params![job_id, pid],
                    |r| r.get(0),
                )
                .optional()?;

            let eff_status = match cur_status {
                Some(cur) => {
                    let target = next_status.clone().unwrap_or(cur);
                    tx.execute(
                        "UPDATE node_execution_jobs
                         SET status = ?1,
                             started_at = COALESCE(started_at, ?2),
                             completed_at = CASE WHEN ?1 IN ('COMPLETED', 'FAILED', 'CANCELLED') THEN ?2 ELSE completed_at END
                         WHERE id = ?3 AND project_id = ?4",
                        params![target, now_ms, job_id, pid],
                    )?;
                    target
                }
                None => {
                    let target = next_status
                        .clone()
                        .unwrap_or_else(|| "RUNNING".to_string());
                    tx.execute(
                        "INSERT INTO node_execution_jobs (
                            id, project_id, session_id, node_id, status, fencing_token,
                            priority, created_at, started_at, completed_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 10, ?6, ?6, CASE WHEN ?5 IN ('COMPLETED', 'FAILED', 'CANCELLED') THEN ?6 ELSE NULL END)",
                        params![job_id, pid, session_id_req, node_id, target, now_ms],
                    )?;
                    target
                }
            };

            let max_seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(seq_no), 0) FROM execution_events WHERE job_id = ?1",
                params![job_id],
                |r| r.get(0),
            )?;
            let next_seq = max_seq + 1;
            tx.execute(
                "INSERT INTO execution_events (
                    id, tenant_id, project_id, job_id, node_id, event_kind, payload_text, seq_no, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    event_id,
                    principal.tenant_id,
                    pid,
                    job_id,
                    node_id,
                    event_kind,
                    req.payload_text,
                    next_seq,
                    now_ms
                ],
            )?;
            tx.commit()?;
            Ok::<(i64, String), rusqlite::Error>((next_seq, eff_status))
        })
        .map_err(|e| map_harness_resp(e, "append_execution_event"))?;

        Ok(ExecutionEventDto {
            id: event_id,
            job_id,
            project_id: pid,
            node_id,
            event_kind,
            payload_text: req.payload_text,
            seq_no,
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
        "SELECT e.id, e.job_id, e.project_id, e.node_id, e.event_kind, e.payload_text, e.seq_no,
                COALESCE(j.status, 'RUNNING'), e.created_at
         FROM execution_events e
         LEFT JOIN node_execution_jobs j ON j.id = e.job_id
         WHERE e.tenant_id = ?1 AND e.project_id = ?2 AND e.seq_no > ?3",
    );
    if state.job_id.is_some() {
        sql.push_str(" AND e.job_id = ?4");
    } else if state.node_id.is_some() {
        sql.push_str(" AND e.node_id = ?4");
    }
    sql.push_str(" ORDER BY e.seq_no ASC LIMIT 50");

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
            job_status: row.get(7)?,
            created_at: epoch_ms_to_iso(row.get(8)?),
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

    let init = SsePollState {
        pool: state.registry.pool().clone(),
        tenant_id: principal.tenant_id,
        project_id: pid,
        job_id: q.job_id.filter(|s| !s.trim().is_empty()),
        node_id: q.node_id.filter(|s| !s.trim().is_empty()),
        last_seq: q.after_seq.unwrap_or(0),
        ticks: 0,
        closed: false,
    };

    let stream = futures_util::stream::unfold(init, |mut st| async move {
        if st.closed || st.ticks >= 40 {
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
                        st.last_seq = last.seq_no;
                    }
                    if !events.is_empty() {
                        if is_terminal {
                            st.closed = true;
                        }
                        let payload =
                            serde_json::to_string(&events).unwrap_or_else(|_| "[]".into());
                        let ev = Event::default().event("execution_events").data(payload);
                        return Some((Ok(ev), st));
                    }
                    if is_terminal || st.ticks >= 40 {
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
// 6. Provider Key Verification (`POST /api/v1/doctor/verify-key`)
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyKeyRequest {
    pub provider_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub endpoint_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyKeyResponse {
    pub valid: bool,
    pub status: String,
    pub provider_id: String,
    pub message: String,
    pub credential_ref: String,
}

pub async fn handle_verify_provider_key(
    Extension(_principal): Extension<FlowPrincipal>,
    Json(req): Json<VerifyKeyRequest>,
) -> Result<Json<VerifyKeyResponse>, Response> {
    let provider = req.provider_id.trim().to_ascii_lowercase();
    let key = req.api_key.trim();
    let endpoint = req.endpoint_url.trim();

    if provider.is_empty() {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "providerId is required",
        ));
    }

    let (valid, status, message) = match provider.as_str() {
        "anthropic" | "claude" => {
            if key.starts_with("sk-ant-") && key.len() >= 24 {
                (
                    true,
                    "verified",
                    "Anthropic credential format verified locally",
                )
            } else {
                (
                    false,
                    "invalid_key",
                    "Expected Anthropic key starting with 'sk-ant-' (min 24 chars)",
                )
            }
        }
        "openai" => {
            if key.starts_with("sk-") && key.len() >= 20 {
                (
                    true,
                    "verified",
                    "OpenAI credential format verified locally",
                )
            } else {
                (
                    false,
                    "invalid_key",
                    "Expected OpenAI key starting with 'sk-' (min 20 chars)",
                )
            }
        }
        "gemini" | "google" => {
            if key.starts_with("AIza") && key.len() >= 24 {
                (
                    true,
                    "verified",
                    "Gemini credential format verified locally",
                )
            } else {
                (
                    false,
                    "invalid_key",
                    "Expected Gemini key starting with 'AIza' (min 24 chars)",
                )
            }
        }
        "deepseek" => {
            if key.starts_with("sk-") && key.len() >= 18 {
                (
                    true,
                    "verified",
                    "DeepSeek credential format verified locally",
                )
            } else {
                (
                    false,
                    "invalid_key",
                    "Expected DeepSeek key starting with 'sk-'",
                )
            }
        }
        "openrouter" => {
            if key.starts_with("sk-or-") && key.len() >= 20 {
                (
                    true,
                    "verified",
                    "OpenRouter credential format verified locally",
                )
            } else {
                (
                    false,
                    "invalid_key",
                    "Expected OpenRouter key starting with 'sk-or-'",
                )
            }
        }
        "ollama" | "byos" => {
            let valid_ep = endpoint.starts_with("http://127.0.0.1:")
                || endpoint.starts_with("http://localhost:")
                || endpoint.starts_with("https://");
            if valid_ep {
                (true, "verified", "Local/BYOS endpoint URL validated")
            } else {
                (
                    false,
                    "invalid_endpoint",
                    "Expected valid http://127.0.0.1:<port> or https:// endpointUrl",
                )
            }
        }
        _ => {
            if key.len() >= 16 {
                (true, "verified", "Custom provider key shape validated")
            } else {
                (
                    false,
                    "invalid_key",
                    "API key must be at least 16 characters",
                )
            }
        }
    };

    Ok(Json(VerifyKeyResponse {
        valid,
        status: status.to_string(),
        provider_id: provider,
        message: message.to_string(),
        credential_ref: sanitize_credential_ref(key),
    }))
}
