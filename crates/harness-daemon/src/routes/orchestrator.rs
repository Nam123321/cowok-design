// @story 10.1
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon /api/v1/orchestrator/chat, /live-patch, /assets backed by Single MPSC SQLite Write Actor & 3-Lane Router)
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use axum::{
    body::Bytes,
    extract::{rejection::QueryRejection, Extension, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bot_fleet::{
    deduplicate_provenance_backlinks, parse_live_spec_jsonl, read_live_spec_file_nofollow,
    validate_session_id, CowokOrchestrator, DecisionCascadeClient, LiveSpecBridgeState,
    LiveSpecPatchLine, OrchestratorCommandKind, OrchestratorError, OrchestratorLane,
    OrchestratorRouteInput, ProvenanceBacklink, LIVE_SPEC_MAX_BYTES,
};
use chrono::Utc;
use domain::decision::{CascadeStage, ConfidenceKind, DecisionTier};
use domain::HarnessError;
use html_forge::{
    compute_sha256_hex, escape_html_attr, patch_html_element, sanitize_html_with_profile,
    HtmlForgeError, HtmlPatchOperation, SanitizeProfile,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::{
    SqliteActorError, SqlitePool, SqliteWriteActor, SqliteWriteActorHandle,
    DEFAULT_WRITE_ACTOR_CAPACITY,
};
use uuid::Uuid;

use super::bot_fleet::{load_tenant_byok_credentials, shared_cooldown_tracker};
use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress,
};

const MAX_ORCHESTRATOR_BODY_BYTES: usize = 655_360;
const MAX_MESSAGE_LEN: usize = 16_384;
const MAX_SCREEN_ID_LEN: usize = 128;
const MAX_RESIZE_DIMENSION_PX: u64 = 4_096;

type ScreenBridgeKey = (String, String, String);
type SharedScreenBridge = Arc<tokio::sync::Mutex<LiveSpecBridgeState>>;

fn actor_registry() -> &'static Mutex<HashMap<PathBuf, SqliteWriteActorHandle>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, SqliteWriteActorHandle>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn live_bridge_registry() -> &'static Mutex<HashMap<ScreenBridgeKey, SharedScreenBridge>> {
    static REGISTRY: OnceLock<Mutex<HashMap<ScreenBridgeKey, SharedScreenBridge>>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_or_create_screen_bridge(
    tenant_id: &str,
    project_id: &str,
    screen_id: &str,
) -> SharedScreenBridge {
    let key = (
        tenant_id.to_string(),
        project_id.to_string(),
        screen_id.to_string(),
    );
    let mut guard = live_bridge_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Arc::clone(guard.entry(key).or_insert_with(|| {
        Arc::new(tokio::sync::Mutex::new(LiveSpecBridgeState::for_screen(
            screen_id,
        )))
    }))
}

async fn collect_project_bridge_provenance(
    tenant_id: &str,
    project_id: &str,
    session_filter: Option<&str>,
) -> Vec<ProvenanceBacklink> {
    let bridges: Vec<SharedScreenBridge> = {
        let guard = live_bridge_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .iter()
            .filter(|((tid, pid, _), _)| tid == tenant_id && pid == project_id)
            .map(|(_, b)| Arc::clone(b))
            .collect()
    };

    let mut out = Vec::new();
    for bridge in bridges {
        let state_guard = bridge.lock().await;
        for bl in state_guard.provenance_backlinks() {
            let matches_session = match session_filter {
                None => true,
                Some(sf) => sf == bl.session_id,
            };
            if matches_session {
                out.push(bl);
            }
        }
    }
    out
}

/// Returns (or lazily spawns / respawns) the single shared MPSC SQLite Write Actor for `pool.db_path()`.
pub fn shared_write_actor_for_pool(
    pool: &SqlitePool,
) -> Result<SqliteWriteActorHandle, SqliteActorError> {
    let key = pool
        .db_path()
        .canonicalize()
        .unwrap_or_else(|_| pool.db_path().to_path_buf());
    let mut guard = actor_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = guard.get(&key) {
        if existing.is_alive() {
            return Ok(existing.clone());
        }
    }
    let handle = SqliteWriteActor::spawn_with_capacity(
        pool.db_path().to_path_buf(),
        DEFAULT_WRITE_ACTOR_CAPACITY,
    )?;
    guard.insert(key, handle.clone());
    Ok(handle)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestratorChatRequest {
    pub project_id: String,
    pub session_id: String,
    #[serde(default)]
    pub node_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub command_kind: OrchestratorCommandKind,
    #[serde(default)]
    pub live_spec_payload: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestratorChatData {
    pub lane: OrchestratorLane,
    pub latency_ms: u64,
    pub token_cost: u64,
    pub reply: String,
    pub applied_patch_id: Option<String>,
    pub can_undo: bool,
    pub live_spec_path: Option<String>,
    pub provenance_backlinks: Vec<ProvenanceBacklink>,
    pub actor_queue_depth: usize,
}

fn default_patch_action() -> String {
    "apply".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestratorLivePatchRequest {
    pub project_id: String,
    pub session_id: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub screen_id: Option<String>,
    #[serde(default = "default_patch_action")]
    pub action: String,
    #[serde(default)]
    pub live_spec_payload: Option<String>,
    #[serde(default)]
    pub live_spec_file_path: Option<String>,
    #[serde(default)]
    pub base_html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestratorLivePatchData {
    pub patch_id: String,
    pub project_id: String,
    pub screen_id: String,
    pub ast_revision: i64,
    pub action: String,
    pub canvas_sha256: String,
    pub html_snapshot: String,
    pub can_undo: bool,
    pub latency_ms: u64,
    pub applied_lines: usize,
    pub provenance_backlinks: Vec<ProvenanceBacklink>,
    pub actor_queue_depth: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestratorAssetsQuery {
    pub project_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrchestratorAssetsData {
    pub project_id: String,
    pub items: Vec<ProvenanceBacklink>,
    pub total_count: usize,
}

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    field: Option<String>,
    details: Option<serde_json::Value>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
                field,
                request_id,
                details,
            },
        }),
    )
        .into_response()
}

fn ok_response<T: Serialize>(data: T, request_id: String) -> Response {
    (
        StatusCode::OK,
        Json(ApiSuccessEnvelope {
            ok: true,
            data,
            request_id,
        }),
    )
        .into_response()
}

pub(crate) fn map_actor_error_to_response(err: SqliteActorError, request_id: String) -> Response {
    match err {
        SqliteActorError::Backpressure {
            retry_after_secs,
            queue_capacity,
        } => {
            let mut resp = error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "ACTOR_BACKPRESSURE",
                "SQLite write actor queue saturated; retry shortly",
                None,
                Some(serde_json::json!({
                    "retryAfterSecs": retry_after_secs,
                    "queueCapacity": queue_capacity,
                })),
                request_id,
            );
            if let Ok(val) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                resp.headers_mut().insert("retry-after", val);
            }
            resp
        }
        SqliteActorError::SymlinkDenied(_) => error_response(
            StatusCode::FORBIDDEN,
            "SYMLINK_VIOLATION",
            "Refusing symlink SQLite path",
            None,
            None,
            request_id,
        ),
        other => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            &other.to_string(),
            None,
            None,
            request_id,
        ),
    }
}

fn map_orchestrator_error_to_response(err: OrchestratorError, request_id: String) -> Response {
    match err {
        OrchestratorError::SymlinkViolation(msg) => error_response(
            StatusCode::FORBIDDEN,
            "SYMLINK_VIOLATION",
            &msg,
            Some("liveSpecFilePath".to_string()),
            None,
            request_id,
        ),
        OrchestratorError::PathTraversal(msg) => error_response(
            StatusCode::FORBIDDEN,
            "PATH_TRAVERSAL",
            &msg,
            Some("liveSpecFilePath".to_string()),
            None,
            request_id,
        ),
        OrchestratorError::LiveSpecNotFound(msg) => error_response(
            StatusCode::NOT_FOUND,
            "LIVE_SPEC_NOT_FOUND",
            &msg,
            Some("liveSpecFilePath".to_string()),
            None,
            request_id,
        ),
        OrchestratorError::PayloadTooLarge { actual, max } => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "LIVE_SPEC_PAYLOAD_TOO_LARGE",
            &format!("Live spec payload {actual} bytes exceeds {max} byte ceiling"),
            Some("liveSpecPayload".to_string()),
            Some(serde_json::json!({ "actualBytes": actual, "maxBytes": max })),
            request_id,
        ),
        OrchestratorError::InvalidLiveSpecPatch(msg) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_LIVE_SPEC_PATCH",
            &msg,
            Some("liveSpecPayload".to_string()),
            None,
            request_id,
        ),
        OrchestratorError::NothingToUndo => error_response(
            StatusCode::CONFLICT,
            "NOTHING_TO_UNDO",
            "No prior live patch snapshot available for 1-step Cmd+Z rollback",
            Some("action".to_string()),
            None,
            request_id,
        ),
        OrchestratorError::InvalidProvenanceHash(msg) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &msg,
            Some("assetContentHash".to_string()),
            None,
            request_id,
        ),
        other => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &other.to_string(),
            None,
            None,
            request_id,
        ),
    }
}

fn map_html_forge_error_to_response(err: HtmlForgeError, request_id: String) -> Response {
    match err {
        HtmlForgeError::MxssBlocked(detail) | HtmlForgeError::CssTokenBlocked(detail) => {
            error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "E_MXSS_BLOCKED",
                &format!("Blocked unsafe HTML/CSS construct: {detail}"),
                Some("baseHtml".to_string()),
                None,
                request_id,
            )
        }
        HtmlForgeError::PayloadTooLarge { actual, max } => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "LIVE_SPEC_PAYLOAD_TOO_LARGE",
            &format!("HTML payload {actual} bytes exceeds {max} byte ceiling"),
            Some("baseHtml".to_string()),
            Some(serde_json::json!({ "actualBytes": actual, "maxBytes": max })),
            request_id,
        ),
        other => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_LIVE_SPEC_PATCH",
            &other.to_string(),
            Some("liveSpecPayload".to_string()),
            None,
            request_id,
        ),
    }
}

fn resolve_principal<'a>(
    caller: Option<&'a VerifiedLegacyIngress>,
    principal_ext: Option<&'a FlowPrincipal>,
    headers: &HeaderMap,
    request_id: &str,
) -> Result<&'a FlowPrincipal, Response> {
    if let Some(v) = caller {
        if v.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
            return Err(error_response(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Orchestrator routes require verified session token, loopback HMAC, or superadmin Bearer",
                None,
                None,
                request_id.to_string(),
            ));
        }
        return Ok(&v.principal);
    }
    if let Some(p) = principal_ext {
        return Ok(p);
    }
    Err(error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        None,
        request_id.to_string(),
    ))
}

fn require_write_role(principal: &FlowPrincipal, request_id: &str) -> Result<(), Response> {
    if !matches!(
        principal.role.trim().to_ascii_lowercase().as_str(),
        "editor" | "admin" | "owner"
    ) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Orchestrator write operation requires editor or admin role",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn verify_project_tenant_access(
    state: &DaemonAppState,
    project_id: &str,
    headers: &HeaderMap,
    caller: Option<&VerifiedLegacyIngress>,
    principal: &FlowPrincipal,
    request_id: &str,
) -> Result<(String, String, PathBuf), Response> {
    let trimmed = project_id.trim();
    let project_uuid = Uuid::parse_str(trimmed).map_err(|_| {
        error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be a valid UUID",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        )
    })?;
    let normalized_project_id = project_uuid.to_string();

    if let Some(verified) = caller {
        match verify_anti_idor_with_caller(&state.registry, project_uuid, headers, Some(verified)) {
            Ok(_) => {}
            Err(HarnessError::ProjectNotFound { .. }) => {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "PROJECT_NOT_FOUND",
                    "Project not found",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
            Err(HarnessError::BoundaryViolation { .. }) => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "PROJECT_TENANT_MISMATCH",
                    "Cross-tenant project access forbidden",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
            Err(_) => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "FORBIDDEN",
                    "Project access denied",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
        }
    }

    let conn = state.registry.pool().connection().map_err(|_| {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            "Storage connection unavailable",
            None,
            None,
            request_id.to_string(),
        )
    })?;
    let (tenant_id, ws_root_str): (String, String) = conn
        .query_row(
            "SELECT tenant_id, workspace_root FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![normalized_project_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| {
            error_response(
                StatusCode::NOT_FOUND,
                "PROJECT_NOT_FOUND",
                "Project not found",
                Some("projectId".to_string()),
                None,
                request_id.to_string(),
            )
        })?;

    if let Some(verified) = caller {
        if matches!(
            verified.kind,
            LegacyAuthKind::HmacV2 | LegacyAuthKind::SessionToken
        ) && verified.principal.tenant_id.trim() != tenant_id.trim()
        {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "PROJECT_TENANT_MISMATCH",
                "Cross-tenant project access forbidden",
                Some("projectId".to_string()),
                None,
                request_id.to_string(),
            ));
        }
    } else if principal.tenant_id.trim() != tenant_id.trim() {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "Cross-tenant project access forbidden",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        ));
    }

    let ws_root = if ws_root_str.trim().is_empty() {
        state.registry.pool().workspace_root().to_path_buf()
    } else {
        PathBuf::from(ws_root_str)
    };
    Ok((tenant_id, normalized_project_id, ws_root))
}

async fn verify_project_tenant_access_async(
    state: DaemonAppState,
    project_id: String,
    headers: HeaderMap,
    caller: Option<VerifiedLegacyIngress>,
    principal: FlowPrincipal,
    request_id: String,
) -> Result<(String, String, PathBuf), Response> {
    let err_request_id = request_id.clone();
    tokio::task::spawn_blocking(move || {
        verify_project_tenant_access(
            &state,
            &project_id,
            &headers,
            caller.as_ref(),
            &principal,
            &request_id,
        )
    })
    .await
    .map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Project tenant verification worker failed unexpectedly",
            None,
            None,
            err_request_id,
        )
    })?
}

async fn resolve_actor_handle_async(
    state: DaemonAppState,
    actor_ext: Option<Extension<SqliteWriteActorHandle>>,
    request_id: &str,
) -> Result<SqliteWriteActorHandle, Response> {
    if let Some(Extension(handle)) = actor_ext {
        if handle.is_alive() {
            return Ok(handle);
        }
    }
    let req_id = request_id.to_string();
    let err_req_id = req_id.clone();
    tokio::task::spawn_blocking(move || shared_write_actor_for_pool(state.registry.pool()))
        .await
        .map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Write actor lookup worker failed unexpectedly",
                None,
                None,
                err_req_id,
            )
        })?
        .map_err(|e| map_actor_error_to_response(e, req_id))
}

fn default_live_canvas_html(screen_id: &str, ff_id: &str) -> String {
    let safe_screen = escape_html_attr(screen_id);
    let safe_ff = escape_html_attr(ff_id);
    let extra = if ff_id == "stage-root" || ff_id == "hero-title" {
        String::new()
    } else {
        format!("<div data-ff-id=\"{safe_ff}\">Live Element</div>")
    };
    format!(
        "<!DOCTYPE html><html><head><style>body{{margin:0;padding:16px;font-family:sans-serif}}</style></head><body><main data-ff-id=\"stage-root\" data-screen-id=\"{safe_screen}\"><h1 data-ff-id=\"hero-title\">Workspace Screen</h1>{extra}</main></body></html>"
    )
}

fn apply_lines_to_html_snapshot(
    base_html: &str,
    lines: &[LiveSpecPatchLine],
) -> Result<(String, String), HtmlForgeError> {
    let (sanitized_base, _) = sanitize_html_with_profile(base_html, SanitizeProfile::AppPreview)?;
    let mut working = sanitized_base.clone();
    for line in lines {
        let op = match line.op_kind.as_str() {
            "TEXT_EDIT" => line
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .map(|text| HtmlPatchOperation::TextEdit {
                    text: text.to_string(),
                }),
            "TOKEN_SWAP" => {
                let mut css_vars = BTreeMap::new();
                if let Some(serde_json::Value::Object(obj)) = line.payload.get("cssVars") {
                    for (k, v) in obj {
                        if let Some(s) = v.as_str() {
                            css_vars.insert(k.clone(), s.to_string());
                        }
                    }
                }
                if css_vars.is_empty() {
                    None
                } else {
                    Some(HtmlPatchOperation::TokenSwap { css_vars })
                }
            }
            "RESIZE" => {
                let mut safe_styles = BTreeMap::new();
                if let Some(w) = line.payload.get("width").and_then(|v| v.as_u64()) {
                    safe_styles.insert(
                        "width".to_string(),
                        format!("{}px", w.clamp(1, MAX_RESIZE_DIMENSION_PX)),
                    );
                }
                if let Some(h) = line.payload.get("height").and_then(|v| v.as_u64()) {
                    safe_styles.insert(
                        "height".to_string(),
                        format!("{}px", h.clamp(1, MAX_RESIZE_DIMENSION_PX)),
                    );
                }
                if safe_styles.is_empty() {
                    None
                } else {
                    Some(HtmlPatchOperation::StylePatch { safe_styles })
                }
            }
            "STEERING_CHIP" | "LOCKED_RULE_CHANGE" | "LINT_FIX" => {
                if let Some(text) = line.payload.get("text").and_then(|v| v.as_str()) {
                    Some(HtmlPatchOperation::TextEdit {
                        text: text.to_string(),
                    })
                } else if let Some(serde_json::Value::Object(obj)) = line.payload.get("cssVars") {
                    let mut css_vars = BTreeMap::new();
                    for (k, v) in obj {
                        if let Some(s) = v.as_str() {
                            css_vars.insert(k.clone(), s.to_string());
                        }
                    }
                    if css_vars.is_empty() {
                        None
                    } else {
                        Some(HtmlPatchOperation::TokenSwap { css_vars })
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(patch_op) = op {
            let out = patch_html_element(
                &working,
                &line.ff_id,
                &patch_op,
                SanitizeProfile::AppPreview,
            )?;
            if out.patched_count > 0 {
                working = out.patched_html;
            }
        }
    }
    Ok((sanitized_base, working))
}

fn map_live_op_kind_to_v19(op_kind: &str) -> &'static str {
    match op_kind {
        "TEXT_EDIT" => "TEXT_EDIT",
        "TOKEN_SWAP" => "TOKEN_SWAP",
        "LOCKED_RULE_CHANGE" => "LOCKED_RULE_CHANGE",
        "UNDO" => "UNDO",
        _ => "STEERING_CHIP",
    }
}

/// `POST /api/v1/orchestrator/chat`
pub async fn handle_orchestrator_chat(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    actor_ext: Option<Extension<SqliteWriteActorHandle>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(&principal, &request_id) {
        return resp;
    }
    if body.len() > MAX_ORCHESTRATOR_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Orchestrator chat payload exceeds ceiling",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: OrchestratorChatRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid orchestrator chat payload: {e}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let message = req.message.trim();
    if message.is_empty() || message.len() > MAX_MESSAGE_LEN {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "message must be 1..=16384 characters",
            Some("message".to_string()),
            None,
            request_id,
        );
    }
    let safe_session_id = match validate_session_id(&req.session_id) {
        Ok(s) => s,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "sessionId must match ^[A-Za-z0-9._-]+$ without '..'",
                Some("sessionId".to_string()),
                None,
                request_id,
            );
        }
    };
    if let Some(ref payload) = req.live_spec_payload {
        if payload.len() > LIVE_SPEC_MAX_BYTES {
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "LIVE_SPEC_PAYLOAD_TOO_LARGE",
                &format!(
                    "liveSpecPayload {} bytes exceeds {} byte ceiling",
                    payload.len(),
                    LIVE_SPEC_MAX_BYTES
                ),
                Some("liveSpecPayload".to_string()),
                None,
                request_id,
            );
        }
    }

    let (tenant_id, normalized_project_id, workspace_root) =
        match verify_project_tenant_access_async(
            state.clone(),
            req.project_id.clone(),
            headers.clone(),
            caller.map(|e| e.0),
            principal,
            request_id.clone(),
        )
        .await
        {
            Ok(triple) => triple,
            Err(resp) => return resp,
        };

    let actor = match resolve_actor_handle_async(state.clone(), actor_ext, &request_id).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };

    let lane_preview = CowokOrchestrator::classify_lane(
        req.command_kind,
        message,
        req.live_spec_payload.as_deref(),
    );

    let orchestrator = match lane_preview {
        OrchestratorLane::Lane1DecisionCascade => {
            let vault_state = state.clone();
            let tid_vault = tenant_id.clone();
            let creds = tokio::task::spawn_blocking(move || {
                let (c, _) = load_tenant_byok_credentials(&vault_state, &tid_vault);
                c
            })
            .await
            .unwrap_or_default();
            let creds_arc = Arc::new(creds);
            CowokOrchestrator::with_decision_client(
                DecisionCascadeClient::new()
                    .with_vault_lookup(Arc::new(move |_tid: &str| (*creds_arc).clone())),
            )
        }
        OrchestratorLane::Lane2OpcFleet => {
            let vault_state = state.clone();
            let tid_vault = tenant_id.clone();
            let creds = tokio::task::spawn_blocking(move || {
                let (c, _) = load_tenant_byok_credentials(&vault_state, &tid_vault);
                c
            })
            .await
            .unwrap_or_default();
            CowokOrchestrator::new().with_byok_fleet(creds, shared_cooldown_tracker())
        }
        OrchestratorLane::Lane0Deterministic => CowokOrchestrator::new(),
    };

    let target_screen_id = if let Some(ref payload) = req.live_spec_payload {
        let preview_lines = match parse_live_spec_jsonl(payload) {
            Ok(l) => l,
            Err(err) => return map_orchestrator_error_to_response(err, request_id),
        };
        preview_lines
            .last()
            .map(|l| l.screen_id.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main-screen".to_string())
    } else {
        "main-screen".to_string()
    };

    let route_input = OrchestratorRouteInput {
        tenant_id: tenant_id.clone(),
        project_id: normalized_project_id.clone(),
        session_id: safe_session_id.clone(),
        node_id: req.node_id.clone(),
        message: message.to_string(),
        command_kind: req.command_kind,
        live_spec_payload: req.live_spec_payload.clone(),
        workspace_root: Some(workspace_root),
        untrusted_artifact_sha256: None,
    };

    let screen_bridge =
        get_or_create_screen_bridge(&tenant_id, &normalized_project_id, &target_screen_id);

    let audit_id = Uuid::new_v4().to_string();
    let now_ms = Utc::now().timestamp_millis();

    let route_out = if lane_preview == OrchestratorLane::Lane0Deterministic {
        // Hold per-screen async Mutex across Lane 0 routing + SQLite commit so concurrent
        // /chat and /live-patch mutations on the same screen never lose updates.
        let mut bridge_guard = screen_bridge.lock().await;
        let mut working_bridge = bridge_guard.clone();

        let route_out = match orchestrator
            .route_three_lane(&route_input, &mut working_bridge)
            .await
        {
            Ok(out) => out,
            Err(err) => return map_orchestrator_error_to_response(err, request_id),
        };

        let resolved_screen_id = if route_out.screen_id.trim().is_empty() {
            target_screen_id.clone()
        } else {
            route_out.screen_id.clone()
        };
        let first_ff = route_out
            .applied_patch_lines
            .first()
            .map(|l| l.ff_id.clone())
            .or_else(|| req.node_id.clone())
            .unwrap_or_else(|| "stage-root".to_string());
        let op_kind_v19 = if route_out.is_undo {
            "UNDO".to_string()
        } else {
            route_out
                .applied_patch_lines
                .first()
                .map(|l| map_live_op_kind_to_v19(&l.op_kind))
                .unwrap_or("STEERING_CHIP")
                .to_string()
        };

        let html_snapshot = if route_out.is_undo {
            let candidate = route_out
                .restored_html_snapshot
                .clone()
                .unwrap_or_else(|| default_live_canvas_html(&resolved_screen_id, "stage-root"));
            match sanitize_html_with_profile(&candidate, SanitizeProfile::AppPreview) {
                Ok((clean, _)) => clean,
                Err(err) => return map_html_forge_error_to_response(err, request_id),
            }
        } else {
            let base_html = match bridge_guard.current_html_snapshot() {
                Some(snap) if !snap.trim().is_empty() => snap.to_string(),
                _ => fetch_latest_html_snapshot_async(
                    state.clone(),
                    tenant_id.clone(),
                    normalized_project_id.clone(),
                    resolved_screen_id.clone(),
                )
                .await
                .unwrap_or_else(|| default_live_canvas_html(&resolved_screen_id, &first_ff)),
            };
            let (sanitized_base, reconciled) =
                match apply_lines_to_html_snapshot(&base_html, &route_out.applied_patch_lines) {
                    Ok(pair) => pair,
                    Err(err) => return map_html_forge_error_to_response(err, request_id),
                };
            working_bridge.record_top_undo_html_snapshot(sanitized_base, reconciled.clone());
            reconciled
        };

        let patch_id_write = if route_out.is_undo {
            Uuid::new_v4().to_string()
        } else {
            route_out
                .applied_patch_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string())
        };
        let result_html_sha = compute_sha256_hex(html_snapshot.as_bytes());
        let patch_payload_json = serde_json::json!({
            "source": if route_out.is_undo { "orchestrator_chat_undo" } else { "orchestrator_chat_lane0" },
            "sessionId": safe_session_id,
            "nodeId": req.node_id,
            "appliedLines": route_out.applied_patch_lines.len(),
            "canvasSha256": route_out.canvas_sha256,
            "provenanceBacklinks": route_out.provenance_backlinks,
        })
        .to_string();

        let latency_i64 = route_out.latency_ms as i64;
        let lane_label = route_out.lane.as_str().to_string();
        let tid_write = tenant_id.clone();
        let pid_write = normalized_project_id.clone();
        let screen_write = resolved_screen_id;
        let html_write = html_snapshot;

        let receipt = match actor.try_enqueue_immediate(move |tx| {
            tx.execute(
                "INSERT INTO decision_audit_log (
                    id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                    verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                    stale_file_sha256, latency_ms, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 1, 0, NULL, ?8, ?9)",
                params![
                    audit_id,
                    tid_write,
                    pid_write,
                    CascadeStage::Decision1.as_db_str(),
                    DecisionTier::LocalFts5Bm25.as_db_str(),
                    ConfidenceKind::ExactMatch.as_db_str(),
                    lane_label,
                    latency_i64,
                    now_ms,
                ],
            )?;

            let latest: Option<(i64, String)> = tx
                .query_row(
                    "SELECT ast_revision, result_artifact_sha
                     FROM screen_artifact_patches
                     WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
                     ORDER BY ast_revision DESC
                     LIMIT 1",
                    params![tid_write, pid_write, screen_write],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (prev_rev, base_sha) = latest.unwrap_or_else(|| (0, result_html_sha.clone()));
            let next_rev = prev_rev + 1;
            tx.execute(
                "INSERT INTO screen_artifact_patches (
                    id, tenant_id, project_id, screen_id, ast_revision, base_artifact_sha,
                    result_artifact_sha, ff_id, op_kind, apply_tier, reconciled_ff_ids,
                    payload_json, html_snapshot, locked_rule_json, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'INSTANT_UNDO', 1, ?10, ?11, NULL, ?12)",
                params![
                    patch_id_write,
                    tid_write,
                    pid_write,
                    screen_write,
                    next_rev,
                    base_sha,
                    result_html_sha,
                    first_ff,
                    op_kind_v19,
                    patch_payload_json,
                    html_write,
                    now_ms,
                ],
            )?;
            Ok(())
        }) {
            Ok(r) => r,
            Err(err) => return map_actor_error_to_response(err, request_id),
        };

        match tokio::task::spawn_blocking(move || receipt.wait()).await {
            Ok(Ok(())) => {
                *bridge_guard = working_bridge;
            }
            Ok(Err(err)) => return map_actor_error_to_response(err, request_id),
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "SQLite write actor wait worker panicked",
                    None,
                    None,
                    request_id,
                );
            }
        }
        route_out
    } else {
        // Read-only Lane 1 / Lane 2 turns: snapshot bridge state briefly without holding the lock
        // across remote network I/O, and NEVER overwrite the live screen bridge afterwards.
        let mut read_only_snapshot = {
            let guard = screen_bridge.lock().await;
            guard.clone()
        };

        let route_out = match orchestrator
            .route_three_lane(&route_input, &mut read_only_snapshot)
            .await
        {
            Ok(out) => out,
            Err(err) => return map_orchestrator_error_to_response(err, request_id),
        };

        let (cascade_stage, tier_used, confidence_kind, verdict_label, attempt_no, breaker_tripped) =
            if let Some(ref v) = route_out.decision_verdict {
                (
                    v.cascade_stage.as_db_str().to_string(),
                    v.tier_used.as_db_str().to_string(),
                    v.confidence_kind.as_db_str().to_string(),
                    v.verdict_label.clone(),
                    i64::from(v.attempt_number.clamp(1, 3)),
                    i64::from(v.breaker_tripped),
                )
            } else {
                (
                    CascadeStage::Decision1.as_db_str().to_string(),
                    DecisionTier::LocalFts5Bm25.as_db_str().to_string(),
                    ConfidenceKind::ExactMatch.as_db_str().to_string(),
                    route_out.lane.as_str().to_string(),
                    1i64,
                    0i64,
                )
            };
        let latency_i64 = route_out.latency_ms as i64;
        let tid_write = tenant_id.clone();
        let pid_write = normalized_project_id.clone();

        let receipt = match actor.try_enqueue_immediate(move |tx| {
            tx.execute(
                "INSERT INTO decision_audit_log (
                    id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                    verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                    stale_file_sha256, latency_ms, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, NULL, ?10, ?11)",
                params![
                    audit_id,
                    tid_write,
                    pid_write,
                    cascade_stage,
                    tier_used,
                    confidence_kind,
                    verdict_label,
                    attempt_no,
                    breaker_tripped,
                    latency_i64,
                    now_ms,
                ],
            )?;
            Ok(())
        }) {
            Ok(r) => r,
            Err(err) => return map_actor_error_to_response(err, request_id),
        };

        match tokio::task::spawn_blocking(move || receipt.wait()).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return map_actor_error_to_response(err, request_id),
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "SQLite write actor wait worker panicked",
                    None,
                    None,
                    request_id,
                );
            }
        }
        route_out
    };

    ok_response(
        OrchestratorChatData {
            lane: route_out.lane,
            latency_ms: route_out.latency_ms,
            token_cost: route_out.token_cost,
            reply: route_out.reply,
            applied_patch_id: route_out.applied_patch_id,
            can_undo: route_out.can_undo,
            live_spec_path: route_out.live_spec_path,
            provenance_backlinks: route_out.provenance_backlinks,
            actor_queue_depth: actor.queue_depth(),
        },
        request_id,
    )
}

/// `POST /api/v1/orchestrator/live-patch`
pub async fn handle_orchestrator_live_patch(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    actor_ext: Option<Extension<SqliteWriteActorHandle>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(&principal, &request_id) {
        return resp;
    }
    if body.len() > MAX_ORCHESTRATOR_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "LIVE_SPEC_PAYLOAD_TOO_LARGE",
            "Live patch request body exceeds ceiling",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: OrchestratorLivePatchRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid live-patch payload: {e}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let action = req.action.trim().to_ascii_lowercase();
    if action != "apply" && action != "undo" {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "action must be 'apply' or 'undo'",
            Some("action".to_string()),
            None,
            request_id,
        );
    }
    let safe_session_id = match validate_session_id(&req.session_id) {
        Ok(s) => s,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "sessionId must match ^[A-Za-z0-9._-]+$ without '..'",
                Some("sessionId".to_string()),
                None,
                request_id,
            );
        }
    };

    let (tenant_id, normalized_project_id, workspace_root) =
        match verify_project_tenant_access_async(
            state.clone(),
            req.project_id.clone(),
            headers.clone(),
            caller.map(|e| e.0),
            principal,
            request_id.clone(),
        )
        .await
        {
            Ok(triple) => triple,
            Err(resp) => return resp,
        };

    let actor = match resolve_actor_handle_async(state.clone(), actor_ext, &request_id).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let started = Instant::now();

    if let Some(ref sid) = req.screen_id {
        let trimmed = sid.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_SCREEN_ID_LEN {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "screenId must be 1..=128 characters",
                Some("screenId".to_string()),
                None,
                request_id,
            );
        }
    }

    // Read & parse live spec strictly OUTSIDE both the screen Mutex and MPSC SQLite Write Actor.
    let (target_screen_id, pre_parsed_lines) = if action == "apply" {
        let raw_jsonl = if let Some(ref rel_or_abs) = req.live_spec_file_path {
            let ws_c = workspace_root.clone();
            let path_c = PathBuf::from(rel_or_abs);
            let read_res = tokio::task::spawn_blocking(move || {
                read_live_spec_file_nofollow(&ws_c, Path::new(&path_c))
            })
            .await;
            match read_res {
                Ok(Ok(content)) => content,
                Ok(Err(err)) => return map_orchestrator_error_to_response(err, request_id),
                Err(_) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "INTERNAL_ERROR",
                        "Live spec reader worker failed",
                        None,
                        None,
                        request_id,
                    );
                }
            }
        } else if let Some(ref inline_payload) = req.live_spec_payload {
            inline_payload.clone()
        } else {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "apply action requires liveSpecPayload or liveSpecFilePath",
                Some("liveSpecPayload".to_string()),
                None,
                request_id,
            );
        };

        let parsed_lines = match parse_live_spec_jsonl(&raw_jsonl) {
            Ok(l) => l,
            Err(err) => return map_orchestrator_error_to_response(err, request_id),
        };
        let resolved_screen = parsed_lines
            .last()
            .map(|l| l.screen_id.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "main-screen".to_string());
        (resolved_screen, Some(parsed_lines))
    } else {
        let resolved_screen = req
            .screen_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("main-screen")
            .to_string();
        (resolved_screen, None)
    };

    // Acquire per-(tenant, project, screen) async Mutex so concurrent /live-patch & /chat calls
    // serialize cleanly across .await boundaries with zero lost updates.
    let screen_bridge =
        get_or_create_screen_bridge(&tenant_id, &normalized_project_id, &target_screen_id);
    let mut bridge_guard = screen_bridge.lock().await;
    let mut working_bridge = bridge_guard.clone();

    let (
        patch_id,
        screen_id,
        op_kind_v19,
        ff_id,
        applied_lines,
        canvas_sha256,
        can_undo,
        html_snapshot,
        payload_json,
    ) = if action == "undo" {
        let undo = match working_bridge.undo_last_patch() {
            Ok(u) => u,
            Err(err) => return map_orchestrator_error_to_response(err, request_id),
        };
        let resolved_screen_id = req
            .screen_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| undo.screen_id.clone());
        let prev_html_raw = undo
            .restored_html_snapshot
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| default_live_canvas_html(&resolved_screen_id, "stage-root"));
        let prev_html =
            match sanitize_html_with_profile(&prev_html_raw, SanitizeProfile::AppPreview) {
                Ok((clean, _)) => clean,
                Err(err) => return map_html_forge_error_to_response(err, request_id),
            };
        (
            Uuid::new_v4().to_string(),
            resolved_screen_id,
            "UNDO".to_string(),
            "stage-root".to_string(),
            0usize,
            undo.canvas_sha256,
            undo.can_undo,
            prev_html,
            serde_json::json!({
                "source": "orchestrator_live_patch_undo",
                "restoredPatchId": undo.restored_patch_id,
                "sessionId": safe_session_id,
                "nodeId": req.node_id,
            })
            .to_string(),
        )
    } else {
        let parsed_lines = pre_parsed_lines.unwrap_or_default();
        let first_ff = parsed_lines
            .first()
            .map(|l| l.ff_id.clone())
            .unwrap_or_else(|| "stage-root".to_string());
        let first_op = parsed_lines
            .first()
            .map(|l| map_live_op_kind_to_v19(&l.op_kind))
            .unwrap_or("STEERING_CHIP")
            .to_string();

        let base_html =
            if let Some(explicit) = req.base_html.clone().filter(|s| !s.trim().is_empty()) {
                explicit
            } else if let Some(mem_html) = working_bridge
                .current_html_snapshot()
                .filter(|s| !s.trim().is_empty())
            {
                mem_html.to_string()
            } else {
                fetch_latest_html_snapshot_async(
                    state.clone(),
                    tenant_id.clone(),
                    normalized_project_id.clone(),
                    target_screen_id.clone(),
                )
                .await
                .unwrap_or_else(|| default_live_canvas_html(&target_screen_id, &first_ff))
            };

        // Fail-closed HTML sanitization + patching BEFORE mutating bridge state or SQLite.
        let (sanitized_base, reconciled_html) =
            match apply_lines_to_html_snapshot(&base_html, &parsed_lines) {
                Ok(pair) => pair,
                Err(err) => return map_html_forge_error_to_response(err, request_id),
            };

        let outcome = match working_bridge.apply_live_patch_lines_with_html(
            &parsed_lines,
            &safe_session_id,
            req.node_id.as_deref(),
            Some(sanitized_base),
            Some(reconciled_html.clone()),
        ) {
            Ok(o) => o,
            Err(err) => return map_orchestrator_error_to_response(err, request_id),
        };

        let resolved_screen_id = outcome.screen_id.clone();
        let payload_str = serde_json::json!({
            "source": "orchestrator_live_patch",
            "sessionId": safe_session_id,
            "nodeId": req.node_id,
            "appliedLines": outcome.applied_lines,
            "canvasSha256": outcome.canvas_sha256,
            "provenanceBacklinks": outcome.provenance_backlinks,
        })
        .to_string();

        (
            outcome.applied_patch_id,
            resolved_screen_id,
            first_op,
            first_ff,
            outcome.applied_lines,
            outcome.canvas_sha256,
            outcome.can_undo,
            reconciled_html,
            payload_str,
        )
    };

    let now_ms = Utc::now().timestamp_millis();
    let result_html_sha = compute_sha256_hex(html_snapshot.as_bytes());
    let tid_write = tenant_id.clone();
    let pid_write = normalized_project_id.clone();
    let screen_write = screen_id.clone();
    let patch_id_write = patch_id.clone();
    let html_write = html_snapshot.clone();
    let sha_write = result_html_sha.clone();

    let receipt = match actor.try_enqueue_immediate(move |tx| {
        let latest: Option<(i64, String)> = tx
            .query_row(
                "SELECT ast_revision, result_artifact_sha
                 FROM screen_artifact_patches
                 WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
                 ORDER BY ast_revision DESC
                 LIMIT 1",
                params![tid_write, pid_write, screen_write],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (prev_rev, base_sha) = latest.unwrap_or_else(|| (0, sha_write.clone()));
        let next_rev = prev_rev + 1;
        tx.execute(
            "INSERT INTO screen_artifact_patches (
                id, tenant_id, project_id, screen_id, ast_revision, base_artifact_sha,
                result_artifact_sha, ff_id, op_kind, apply_tier, reconciled_ff_ids,
                payload_json, html_snapshot, locked_rule_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'INSTANT_UNDO', 1, ?10, ?11, NULL, ?12)",
            params![
                patch_id_write,
                tid_write,
                pid_write,
                screen_write,
                next_rev,
                base_sha,
                sha_write,
                ff_id,
                op_kind_v19,
                payload_json,
                html_write,
                now_ms,
            ],
        )?;
        Ok(next_rev)
    }) {
        Ok(r) => r,
        Err(err) => return map_actor_error_to_response(err, request_id),
    };

    let ast_revision = match tokio::task::spawn_blocking(move || receipt.wait()).await {
        Ok(Ok(rev)) => rev,
        Ok(Err(err)) => return map_actor_error_to_response(err, request_id),
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "SQLite write actor wait worker panicked",
                None,
                None,
                request_id,
            );
        }
    };

    let provenance_backlinks = working_bridge.provenance_backlinks();
    *bridge_guard = working_bridge;
    drop(bridge_guard);

    ok_response(
        OrchestratorLivePatchData {
            patch_id,
            project_id: normalized_project_id,
            screen_id,
            ast_revision,
            action,
            canvas_sha256,
            html_snapshot,
            can_undo,
            latency_ms: started.elapsed().as_millis() as u64,
            applied_lines,
            provenance_backlinks,
            actor_queue_depth: actor.queue_depth(),
        },
        request_id,
    )
}

/// `GET /api/v1/orchestrator/assets`
pub async fn handle_orchestrator_assets(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    query_res: Result<Query<OrchestratorAssetsQuery>, QueryRejection>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };

    let Query(query) = match query_res {
        Ok(q) => q,
        Err(rej) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                &format!("Invalid assets query parameters: {rej}"),
                Some("query".to_string()),
                None,
                request_id,
            );
        }
    };

    let (tenant_id, normalized_project_id, _) = match verify_project_tenant_access_async(
        state.clone(),
        query.project_id.clone(),
        headers.clone(),
        caller.map(|e| e.0),
        principal,
        request_id.clone(),
    )
    .await
    {
        Ok(triple) => triple,
        Err(resp) => return resp,
    };

    let session_filter = query
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let state_clone = state.clone();
    let tid_q = tenant_id.clone();
    let pid_q = normalized_project_id.clone();
    let sess_q = session_filter.clone();

    let db_items =
        tokio::task::spawn_blocking(move || -> Result<Vec<ProvenanceBacklink>, String> {
            let conn = state_clone
                .registry
                .pool()
                .connection()
                .map_err(|e| format!("pool connection failed: {e}"))?;
            let mut items = Vec::new();
            if let Some(ref sid) = sess_q {
                let mut stmt = conn
                    .prepare(
                        "SELECT b.sha256, a.session_id, b.mime_type, a.created_at
                         FROM cas_attachments a
                         INNER JOIN cas_blobs b ON b.sha256 = a.blob_sha256
                         WHERE a.tenant_id = ?1 AND a.project_id = ?2 AND a.session_id = ?3
                         ORDER BY a.created_at DESC",
                    )
                    .map_err(|e| format!("prepare failed: {e}"))?;
                let rows = stmt
                    .query_map(params![tid_q, pid_q, sid], |r| {
                        Ok(ProvenanceBacklink {
                            content_hash: r.get(0)?,
                            session_id: r.get(1)?,
                            node_id: None,
                            patch_id: None,
                            mime_type: r.get(2)?,
                            created_at: r.get(3)?,
                        })
                    })
                    .map_err(|e| format!("query failed: {e}"))?;
                for row in rows {
                    items.push(row.map_err(|e| format!("decode failed: {e}"))?);
                }
            } else {
                let mut stmt = conn
                    .prepare(
                        "SELECT b.sha256, a.session_id, b.mime_type, a.created_at
                         FROM cas_attachments a
                         INNER JOIN cas_blobs b ON b.sha256 = a.blob_sha256
                         WHERE a.tenant_id = ?1 AND a.project_id = ?2
                         ORDER BY a.created_at DESC",
                    )
                    .map_err(|e| format!("prepare failed: {e}"))?;
                let rows = stmt
                    .query_map(params![tid_q, pid_q], |r| {
                        Ok(ProvenanceBacklink {
                            content_hash: r.get(0)?,
                            session_id: r.get(1)?,
                            node_id: None,
                            patch_id: None,
                            mime_type: r.get(2)?,
                            created_at: r.get(3)?,
                        })
                    })
                    .map_err(|e| format!("query failed: {e}"))?;
                for row in rows {
                    items.push(row.map_err(|e| format!("decode failed: {e}"))?);
                }
            }

            // Hydrate persisted provenanceBacklinks from screen_artifact_patches.payload_json
            // so backlinks (nodeId + patchId) survive daemon restarts.
            let mut patch_stmt = conn
                .prepare(
                    "SELECT payload_json
                     FROM screen_artifact_patches
                     WHERE tenant_id = ?1 AND project_id = ?2
                     ORDER BY created_at DESC
                     LIMIT 500",
                )
                .map_err(|e| format!("prepare patch provenance failed: {e}"))?;
            let patch_rows = patch_stmt
                .query_map(params![tid_q, pid_q], |r| r.get::<_, String>(0))
                .map_err(|e| format!("query patch provenance failed: {e}"))?;
            for payload_str in patch_rows.flatten() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload_str) {
                    if let Some(arr) = val.get("provenanceBacklinks") {
                        if let Ok(backlinks) =
                            serde_json::from_value::<Vec<ProvenanceBacklink>>(arr.clone())
                        {
                            for bl in backlinks {
                                let matches_session = match sess_q.as_deref() {
                                    None => true,
                                    Some(sf) => sf == bl.session_id,
                                };
                                if matches_session {
                                    items.push(bl);
                                }
                            }
                        }
                    }
                }
            }

            Ok(items)
        })
        .await;

    let mut combined = match db_items {
        Ok(Ok(rows)) => rows,
        Ok(Err(msg)) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                &format!("Failed querying cas_blobs provenance: {msg}"),
                None,
                None,
                request_id,
            );
        }
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Provenance query worker failed unexpectedly",
                None,
                None,
                request_id,
            );
        }
    };

    // Merge live-bridge provenance backlinks across all screens for this project and deduplicate by contentHash.
    combined.extend(
        collect_project_bridge_provenance(
            &tenant_id,
            &normalized_project_id,
            session_filter.as_deref(),
        )
        .await,
    );

    let items = deduplicate_provenance_backlinks(combined);
    let total_count = items.len();
    ok_response(
        OrchestratorAssetsData {
            project_id: normalized_project_id,
            items,
            total_count,
        },
        request_id,
    )
}

async fn fetch_latest_html_snapshot_async(
    state: DaemonAppState,
    tenant_id: String,
    project_id: String,
    screen_id: String,
) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let conn = state.registry.pool().connection().ok()?;
        conn.query_row(
            "SELECT html_snapshot
             FROM screen_artifact_patches
             WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
             ORDER BY ast_revision DESC
             LIMIT 1",
            params![tenant_id, project_id, screen_id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
    })
    .await
    .ok()
    .flatten()
}

/// Standalone Axum sub-router for Story 10.1 (`POST /api/v1/orchestrator/chat`,
/// `POST /api/v1/orchestrator/live-patch`, `GET /api/v1/orchestrator/assets`).
pub fn router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/orchestrator/chat", post(handle_orchestrator_chat))
        .route(
            "/api/v1/orchestrator/live-patch",
            post(handle_orchestrator_live_patch),
        )
        .route(
            "/api/v1/orchestrator/assets",
            get(handle_orchestrator_assets),
        )
}

/// Canonical router constructor matching `build_decision_router` / `build_bot_fleet_router`.
pub fn build_orchestrator_router() -> Router<DaemonAppState> {
    router()
}

/// Sub-router wired with an explicit [`SqliteWriteActorHandle`] (ideal for shared `DaemonAppState`
/// injection or deterministic backpressure testing with a custom-capacity actor).
pub fn build_orchestrator_router_with_actor(
    actor: SqliteWriteActorHandle,
) -> Router<DaemonAppState> {
    router().layer(Extension(actor))
}
