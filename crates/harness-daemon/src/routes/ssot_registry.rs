// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::path::PathBuf;

use axum::{
    body::Bytes,
    extract::{Extension, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use domain::HarnessError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::exports::{
    export_hidden_ssot_bundle, SsotExportError, SsotExportRequest, SsotExportResult,
};
use spec_registry::locks::{
    is_valid_ssot_portal, mutate_appshell_lock, AppShellLockAction, AppShellLockCommand,
    AppShellLockConflictDetails, AppShellLockDto, SsotLockError,
};
use spec_registry::reverse_sync::{
    list_component_patterns, ComponentPatternDto, SsotComponentsPage,
};
use spec_registry::skill_router::{
    CachePreservingSkillRouter, SkillRouteCommand, SkillRouteOutcome,
};
use uuid::Uuid;

use super::legacy_specs::{constant_time_eq, verify_anti_idor_with_caller};
use super::DaemonAppState;
use crate::daemon_auth::{LegacyAuthKind, VerifiedLegacyIngress};

const MAX_SSOT_BODY_BYTES: usize = 65_536;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SsotApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct SsotApiErrorBody {
    error: SsotApiErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SsotSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HttpAppShellLockRequest {
    project_id: String,
    portal_id: String,
    action: AppShellLockAction,
    lock_owner_id: String,
    expected_revision: i64,
    #[serde(default)]
    expected_fencing_token: Option<i64>,
    #[serde(default)]
    locked_slots: Vec<String>,
    #[serde(default = "default_lock_ttl_ms")]
    ttl_ms: i64,
    #[serde(default)]
    force_transfer_reason: Option<String>,
}

fn default_lock_ttl_ms() -> i64 {
    30_000
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HttpSsotExportRequest {
    project_id: String,
    portal_id: String,
    #[serde(default = "default_export_formats")]
    formats: Vec<String>,
    #[serde(default)]
    include_quarantined: bool,
}

fn default_export_formats() -> Vec<String> {
    vec!["markdown".to_string(), "json".to_string()]
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HttpSkillRouteRequest {
    project_id: String,
    intent_summary: String,
    portal_id: String,
    #[serde(default)]
    requested_slugs: Option<Vec<String>>,
    #[serde(default = "default_max_spine")]
    max_spine_tokens: usize,
}

fn default_max_spine() -> usize {
    250
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpLockMutationData {
    lock: AppShellLockDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpComponentsData {
    patterns: Vec<ComponentPatternDto>,
    lock: Option<AppShellLockDto>,
    next_cursor: Option<String>,
    quarantined_count: usize,
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
    let body = SsotApiErrorBody {
        error: SsotApiErrorDetail {
            code: code.to_string(),
            message: message.to_string(),
            field,
            request_id,
            details,
        },
    };
    let mut resp = (status, Json(body)).into_response();
    if code == "STORAGE_BUSY" {
        resp.headers_mut()
            .insert("Retry-After", HeaderValue::from_static("1"));
    }
    resp
}

fn ok_response<T: Serialize>(data: T, request_id: String) -> Response {
    (
        StatusCode::OK,
        Json(SsotSuccessEnvelope {
            ok: true,
            data,
            request_id,
        }),
    )
        .into_response()
}

fn require_authenticated_caller<'a>(
    caller: Option<&'a VerifiedLegacyIngress>,
    request_id: &str,
) -> Result<&'a VerifiedLegacyIngress, Response> {
    caller.ok_or_else(|| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Missing verified authentication credentials",
            None,
            None,
            request_id.to_string(),
        )
    })
}

fn enforce_portal_role_guard(
    headers: &HeaderMap,
    portal_id: &str,
    is_superadmin: bool,
    request_id: &str,
) -> Result<(), Response> {
    if !is_valid_ssot_portal(portal_id) {
        return Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &format!("Unsupported portalId: {portal_id}"),
            Some("portalId".to_string()),
            None,
            request_id.to_string(),
        ));
    }

    if let Some(raw_val) = headers.get("x-cowok-portal") {
        let Ok(raw_hdr) = raw_val.to_str() else {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "CROSS_PORTAL_LEAK_BLOCKED",
                "Invalid x-cowok-portal header encoding",
                Some("portalId".to_string()),
                None,
                request_id.to_string(),
            ));
        };
        let trimmed = raw_hdr.trim();
        if !is_valid_ssot_portal(trimmed) || trimmed != portal_id {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "CROSS_PORTAL_LEAK_BLOCKED",
                "Cross-portal boundary violation",
                Some("portalId".to_string()),
                None,
                request_id.to_string(),
            ));
        }
    }

    if portal_id == "SuperAdmin" && !is_superadmin {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "CROSS_PORTAL_LEAK_BLOCKED",
            "SuperAdmin portal requires elevated admin role",
            Some("portalId".to_string()),
            None,
            request_id.to_string(),
        ));
    }

    Ok(())
}

struct ProjectScope {
    tenant_id: String,
    workspace_root: PathBuf,
    is_superadmin: bool,
}

fn resolve_project_scope(
    state: &DaemonAppState,
    project_id: &str,
    headers: &HeaderMap,
    verified: &VerifiedLegacyIngress,
    request_id: &str,
) -> Result<ProjectScope, Response> {
    if project_id.trim().is_empty() || project_id.len() > 128 {
        return Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be 1..=128 characters",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        ));
    }

    if let Ok(project_uuid) = Uuid::parse_str(project_id) {
        return match verify_anti_idor_with_caller(&state.registry, project_uuid, headers, Some(verified))
        {
            Ok((_reg, ws_root, anti_idor_superadmin)) => {
                let conn = state.registry.pool().connection().map_err(|e| {
                    error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "STORAGE_BUSY",
                        &e.to_string(),
                        None,
                        None,
                        request_id.to_string(),
                    )
                })?;
                let tenant_id: String = conn
                    .query_row(
                        "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
                        params![project_id],
                        |r| r.get(0),
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
                let is_privileged = (matches!(verified.kind, LegacyAuthKind::Bearer)
                    && anti_idor_superadmin)
                    || (matches!(
                        verified.kind,
                        LegacyAuthKind::HmacV2 | LegacyAuthKind::SessionToken
                    ) && (verified.principal.role.eq_ignore_ascii_case("admin")
                        || verified.principal.role.eq_ignore_ascii_case("owner")));
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
                Ok(ProjectScope {
                    tenant_id,
                    workspace_root: ws_root,
                    is_superadmin: is_privileged,
                })
            }
            Err(HarnessError::ProjectNotFound { .. }) => Err(error_response(
                StatusCode::NOT_FOUND,
                "PROJECT_NOT_FOUND",
                "Project not found",
                Some("projectId".to_string()),
                None,
                request_id.to_string(),
            )),
            Err(HarnessError::BoundaryViolation { .. }) => Err(error_response(
                StatusCode::FORBIDDEN,
                "PROJECT_TENANT_MISMATCH",
                "Cross-tenant project access forbidden",
                Some("projectId".to_string()),
                None,
                request_id.to_string(),
            )),
            Err(e) => Err(error_response(
                StatusCode::FORBIDDEN,
                "AUTH_FORBIDDEN",
                &e.to_string(),
                None,
                None,
                request_id.to_string(),
            )),
        };
    }

    // String (non-UUID) project ID fallback with identical strict tenant/token anti-IDOR verification.
    let conn = state.registry.pool().connection().map_err(|e| {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            &e.to_string(),
            None,
            None,
            request_id.to_string(),
        )
    })?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT tenant_id, workspace_root FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![project_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| {
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                &e.to_string(),
                None,
                None,
                request_id.to_string(),
            )
        })?;
    let Some((project_tenant, ws_raw)) = row else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        ));
    };

    let raw_root = if ws_raw.trim().is_empty() || ws_raw == "." {
        state.registry.pool().workspace_root().to_path_buf()
    } else {
        let p = PathBuf::from(&ws_raw);
        if p.is_absolute() {
            p
        } else {
            state.registry.pool().workspace_root().join(p)
        }
    };
    let canon_root = crate::lockfile::validate_and_canonicalize_workspace_root(&raw_root)
        .map_err(|e| {
            error_response(
                StatusCode::FORBIDDEN,
                "PATH_JAIL_VIOLATION",
                &e.to_string(),
                None,
                None,
                request_id.to_string(),
            )
        })?;

    let is_superadmin = match verified.kind {
        LegacyAuthKind::HmacV2 | LegacyAuthKind::SessionToken => {
            if verified.principal.tenant_id.trim() != project_tenant.trim() {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "PROJECT_TENANT_MISMATCH",
                    "Cross-tenant project access forbidden",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
            verified.principal.role.eq_ignore_ascii_case("admin")
                || verified.principal.role.eq_ignore_ascii_case("owner")
        }
        LegacyAuthKind::Bearer => {
            let tok = verified.bearer_token.as_deref().unwrap_or("");
            let superadmin = match std::env::var("COWOK_SUPERADMIN_SECRET") {
                Ok(ref secret) if !secret.trim().is_empty() && !tok.is_empty() => {
                    constant_time_eq(tok.as_bytes(), secret.trim().as_bytes())
                }
                _ => false,
            };
            if !superadmin
                && (tok.is_empty()
                    || !super::legacy_specs::read_verified_project_token(&canon_root, tok))
            {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "AUTH_FORBIDDEN",
                    "Project token mismatch",
                    None,
                    None,
                    request_id.to_string(),
                ));
            }
            superadmin
        }
    };

    Ok(ProjectScope {
        tenant_id: project_tenant,
        workspace_root: canon_root,
        is_superadmin,
    })
}

fn map_lock_error(err: SsotLockError, request_id: String) -> Response {
    match err {
        SsotLockError::Validation { code, message } => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            &message,
            None,
            None,
            request_id,
        ),
        SsotLockError::Forbidden { code, message } => {
            error_response(StatusCode::FORBIDDEN, code, &message, None, None, request_id)
        }
        SsotLockError::NotFound { code, message } => {
            error_response(StatusCode::NOT_FOUND, code, &message, None, None, request_id)
        }
        SsotLockError::FencingConflict(details @ AppShellLockConflictDetails { .. }) => {
            let details_val = serde_json::to_value(details).ok();
            error_response(
                StatusCode::CONFLICT,
                "LOCK_FENCING_CONFLICT",
                "AppShell lock fencing token or revision mismatch",
                None,
                details_val,
                request_id,
            )
        }
        SsotLockError::StorageBusy { details, .. } => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            &details,
            None,
            None,
            request_id,
        ),
    }
}

fn map_export_error(err: SsotExportError, request_id: String) -> Response {
    match err {
        SsotExportError::Validation { code, message } => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            &message,
            None,
            None,
            request_id,
        ),
        SsotExportError::PathTraversal { path } => error_response(
            StatusCode::FORBIDDEN,
            "PATH_JAIL_VIOLATION",
            &format!("Export path escaped jail: {path}"),
            None,
            None,
            request_id,
        ),
        SsotExportError::NotFound { code, message } => {
            error_response(StatusCode::NOT_FOUND, code, &message, None, None, request_id)
        }
        SsotExportError::ExportSuperseded {
            attempted_revision,
            latest_revision,
        } => error_response(
            StatusCode::CONFLICT,
            "EXPORT_SUPERSEDED",
            &format!(
                "Export revision {attempted_revision} superseded by revision {latest_revision}"
            ),
            None,
            None,
            request_id,
        ),
        SsotExportError::ManifestInvalid { reason } => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "EXPORT_MANIFEST_INVALID",
            &reason,
            None,
            None,
            request_id,
        ),
        SsotExportError::StorageBusy { details } => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            &details,
            None,
            None,
            request_id,
        ),
        SsotExportError::Io { code, message } => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            code,
            &message,
            None,
            None,
            request_id,
        ),
    }
}

fn parse_query_params(raw_query: Option<&str>) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Some(q) = raw_query else {
        return map;
    };
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        map.insert(k.to_string(), url_decode_simple(v));
    }
    map
}

fn url_decode_simple(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let raw = input.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let (Some(h), Some(l)) = (
                char::from(raw[i + 1]).to_digit(16),
                char::from(raw[i + 2]).to_digit(16),
            ) {
                bytes.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        bytes.push(if raw[i] == b'+' { b' ' } else { raw[i] });
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub(crate) async fn handle_get_ssot_components(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_authenticated_caller(caller.as_deref(), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let params_map = parse_query_params(raw_query.as_deref());
    let project_id = params_map.get("projectId").cloned().unwrap_or_default();
    let portal_id = params_map.get("portalId").cloned().unwrap_or_default();
    let cursor = params_map.get("cursor").map(String::as_str);
    let limit = match params_map.get("limit") {
        Some(raw) => match raw.parse::<usize>() {
            Ok(n) if (1..=100).contains(&n) => n,
            _ => {
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_REQUEST",
                    "limit must be an integer between 1 and 100",
                    Some("limit".to_string()),
                    None,
                    request_id,
                );
            }
        },
        None => 50,
    };

    let scope = match resolve_project_scope(&state, &project_id, &headers, verified, &request_id) {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    if let Err(resp) =
        enforce_portal_role_guard(&headers, &portal_id, scope.is_superadmin, &request_id)
    {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let tenant_id = scope.tenant_id;
    let cursor_owned = cursor.map(str::to_string);
    let project_id_owned = project_id.clone();
    let portal_id_owned = portal_id.clone();
    let req_id_for_blocking = request_id.clone();

    let blocking_res = tokio::task::spawn_blocking(move || {
        let conn = pool.connection().map_err(|e| {
            SsotLockError::StorageBusy {
                details: e.to_string(),
                retry_after_secs: 1,
            }
        })?;
        list_component_patterns(
            &conn,
            &tenant_id,
            &project_id_owned,
            &portal_id_owned,
            cursor_owned.as_deref(),
            limit,
        )
    })
    .await;

    match blocking_res {
        Ok(Ok(SsotComponentsPage {
            patterns,
            lock,
            next_cursor,
            quarantined_count,
        })) => ok_response(
            HttpComponentsData {
                patterns,
                lock,
                next_cursor,
                quarantined_count,
            },
            req_id_for_blocking,
        ),
        Ok(Err(err)) => map_lock_error(err, req_id_for_blocking),
        Err(join_err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &join_err.to_string(),
            None,
            None,
            req_id_for_blocking,
        ),
    }
}

pub(crate) async fn handle_mutate_appshell_lock(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_authenticated_caller(caller.as_deref(), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if verified.principal.role.eq_ignore_ascii_case("viewer") {
        return error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot mutate AppShell locks",
            None,
            None,
            request_id,
        );
    }

    if body.len() > MAX_SSOT_BODY_BYTES {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Request body exceeds 64KB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }
    let req: HttpAppShellLockRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "Invalid AppShellLockRequest JSON payload",
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let scope =
        match resolve_project_scope(&state, &req.project_id, &headers, verified, &request_id) {
            Ok(s) => s,
            Err(resp) => return resp,
        };
    if let Err(resp) =
        enforce_portal_role_guard(&headers, &req.portal_id, scope.is_superadmin, &request_id)
    {
        return resp;
    }

    let cmd = AppShellLockCommand {
        tenant_id: scope.tenant_id,
        project_id: req.project_id,
        portal_id: req.portal_id,
        caller_actor_id: verified.principal.user_id.clone(),
        is_privileged_role: scope.is_superadmin,
        action: req.action,
        lock_owner_id: req.lock_owner_id,
        expected_revision: req.expected_revision,
        expected_fencing_token: req.expected_fencing_token,
        locked_slots: req.locked_slots,
        ttl_ms: req.ttl_ms,
        force_transfer_reason: req.force_transfer_reason,
        now_ms: None,
    };

    let pool = state.registry.pool().clone();
    let req_id_for_blocking = request_id.clone();
    let blocking_res = tokio::task::spawn_blocking(move || {
        let mut conn = pool.connection().map_err(|e| SsotLockError::StorageBusy {
            details: e.to_string(),
            retry_after_secs: 1,
        })?;
        mutate_appshell_lock(&mut conn, &cmd)
    })
    .await;

    match blocking_res {
        Ok(Ok(res)) => ok_response(HttpLockMutationData { lock: res.lock }, req_id_for_blocking),
        Ok(Err(err)) => map_lock_error(err, req_id_for_blocking),
        Err(join_err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &join_err.to_string(),
            None,
            None,
            req_id_for_blocking,
        ),
    }
}

pub(crate) async fn handle_export_ssot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_authenticated_caller(caller.as_deref(), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if verified.principal.role.eq_ignore_ascii_case("viewer") {
        return error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot trigger SSOT export",
            None,
            None,
            request_id,
        );
    }

    if body.len() > MAX_SSOT_BODY_BYTES {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Request body exceeds 64KB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }
    let req: HttpSsotExportRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "Invalid SsotExportRequest JSON payload",
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let scope =
        match resolve_project_scope(&state, &req.project_id, &headers, verified, &request_id) {
            Ok(s) => s,
            Err(resp) => return resp,
        };
    if let Err(resp) =
        enforce_portal_role_guard(&headers, &req.portal_id, scope.is_superadmin, &request_id)
    {
        return resp;
    }

    let export_req = SsotExportRequest {
        tenant_id: scope.tenant_id,
        project_id: req.project_id,
        portal_id: req.portal_id,
        formats: req.formats,
        include_quarantined: req.include_quarantined,
        relative_export_subdir: None,
        now_ms: None,
    };

    let pool = state.registry.pool().clone();
    let ws_root = scope.workspace_root;
    let req_id_for_blocking = request_id.clone();
    let blocking_res = tokio::task::spawn_blocking(move || {
        let mut conn = pool.connection().map_err(|e| SsotExportError::StorageBusy {
            details: e.to_string(),
        })?;
        export_hidden_ssot_bundle(&mut conn, &ws_root, &export_req)
    })
    .await;

    match blocking_res {
        Ok(Ok(out @ SsotExportResult { .. })) => ok_response(out, req_id_for_blocking),
        Ok(Err(err)) => map_export_error(err, req_id_for_blocking),
        Err(join_err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &join_err.to_string(),
            None,
            None,
            req_id_for_blocking,
        ),
    }
}

pub(crate) async fn handle_route_skills(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_authenticated_caller(caller.as_deref(), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if body.len() > MAX_SSOT_BODY_BYTES {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Request body exceeds 64KB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }
    let req: HttpSkillRouteRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "Invalid SkillRouteRequest JSON payload",
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let scope =
        match resolve_project_scope(&state, &req.project_id, &headers, verified, &request_id) {
            Ok(s) => s,
            Err(resp) => return resp,
        };
    if let Err(resp) =
        enforce_portal_role_guard(&headers, &req.portal_id, scope.is_superadmin, &request_id)
    {
        return resp;
    }

    let cmd = SkillRouteCommand {
        project_id: req.project_id,
        intent_summary: req.intent_summary,
        portal_id: req.portal_id,
        requested_slugs: req.requested_slugs,
        max_spine_tokens: req.max_spine_tokens,
    };

    match CachePreservingSkillRouter::route(&cmd) {
        Ok(out @ SkillRouteOutcome { .. }) => ok_response(out, request_id),
        Err(err) => map_lock_error(err, request_id),
    }
}
