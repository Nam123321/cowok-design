// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (HTTP Routes POST /api/v1/eval/run & GET /api/v1/eval/history)
use axum::{
    body::Bytes,
    extract::{rejection::QueryRejection, Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress,
};
use crate::eval_runner::{
    execute_eval_run, list_eval_history, validate_candidate_dir_in_workspace, EvalRunOptions,
    EvalRunnerError,
};
use crate::eval_runner_suites::EvalTier;

const MAX_EVAL_BODY_BYTES: usize = 65_536;
const DEFAULT_HISTORY_LIMIT: usize = 20;
const MAX_HISTORY_LIMIT: usize = 100;

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

fn default_run_simulated_users() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvalRunRequest {
    pub tier: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub design_craft_candidate_dir: Option<String>,
    #[serde(default = "default_run_simulated_users")]
    pub run_simulated_users: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvalHistoryQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
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
    field: Option<&str>,
    details: Option<serde_json::Value>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
                field: field.map(str::to_string),
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
                "Eval certification routes require verified session token, loopback HMAC, or superadmin Bearer",
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
            "Eval certification runs require editor or admin role",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn validate_optional_project_id(
    state: &DaemonAppState,
    raw_project_id: Option<&str>,
    tenant_id: &str,
    request_id: &str,
) -> Result<Option<String>, Response> {
    let Some(raw) = raw_project_id else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "projectId must be between 1 and 128 characters",
            Some("projectId"),
            None,
            request_id.to_string(),
        ));
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
    let owner_tenant: Option<String> = conn
        .query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![trimmed],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| {
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage query unavailable",
                None,
                None,
                request_id.to_string(),
            )
        })?;

    match owner_tenant {
        Some(owner) if owner.trim() == tenant_id.trim() => Ok(Some(trimmed.to_string())),
        _ => Err(error_response(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found",
            Some("projectId"),
            None,
            request_id.to_string(),
        )),
    }
}

/// `POST /api/v1/eval/run`
pub async fn handle_post_eval_run(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
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

    if body.len() > MAX_EVAL_BODY_BYTES {
        return error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Request body exceeds 64 KiB limit",
            Some("body"),
            None,
            request_id,
        );
    }

    let req: EvalRunRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                &format!("Invalid eval run payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };

    let Some(tier) = EvalTier::parse(&req.tier) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "tier must be one of: pr, stage, release",
            Some("tier"),
            None,
            request_id,
        );
    };

    if !req.run_simulated_users && tier != EvalTier::Pr {
        return error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "runSimulatedUsers=false is only permitted on tier 'pr'; stage and release require full 25-case simulation",
            Some("runSimulatedUsers"),
            None,
            request_id,
        );
    }

    let tenant_id = principal.tenant_id.clone();
    let project_id = match validate_optional_project_id(
        &state,
        req.project_id.as_deref(),
        &tenant_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let workspace_root = state.registry.pool().workspace_root().to_path_buf();
    let candidate_dir = match req.design_craft_candidate_dir.as_deref() {
        Some(raw) => match validate_candidate_dir_in_workspace(&workspace_root, raw) {
            Ok(dir) => Some(dir),
            Err(EvalRunnerError::InvalidInput { field, message }) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_ERROR",
                    &message,
                    Some(field),
                    None,
                    request_id,
                );
            }
            Err(_) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_ERROR",
                    "Invalid designCraftCandidateDir",
                    Some("designCraftCandidateDir"),
                    None,
                    request_id,
                );
            }
        },
        None => None,
    };

    let pool = state.registry.pool().clone();
    let opts = EvalRunOptions {
        tier,
        tenant_id,
        project_id,
        design_craft_candidate_dir: candidate_dir,
        run_simulated_users: req.run_simulated_users,
        triggered_by: "workbench".to_string(),
        seal_if_clean: false,
    };

    let result =
        tokio::task::spawn_blocking(move || execute_eval_run(&pool, &workspace_root, opts)).await;

    match result {
        Ok(Ok(dto)) => ok_response(dto, request_id),
        Ok(Err(EvalRunnerError::AlreadyRunning)) => error_response(
            StatusCode::CONFLICT,
            "EVAL_ALREADY_RUNNING",
            "An evaluation benchmark run is already in progress; retry after completion",
            None,
            None,
            request_id,
        ),
        Ok(Err(EvalRunnerError::TamperDetected {
            run,
            reason,
            detail,
        })) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SEALED_EVAL_TAMPER_ERROR",
            &format!("Sealed evaluation integrity check failed [{reason}]: {detail}"),
            Some("sealedManifestSha256"),
            Some(serde_json::json!({
                "runId": run.run_id,
                "tamperReason": reason,
                "sealedManifestSha256": run.sealed_manifest_sha256,
                "manifestPinSource": run.manifest_pin_source,
                "verdict": run.verdict,
            })),
            request_id,
        ),
        Ok(Err(EvalRunnerError::InvalidInput { field, message })) => error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            &message,
            Some(field),
            None,
            request_id,
        ),
        Ok(Err(EvalRunnerError::Storage(msg))) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_ERROR",
            &msg,
            None,
            None,
            request_id,
        ),
        Err(join_err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "EVAL_EXECUTION_FAILED",
            &format!("Evaluation task failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/eval/history`
pub async fn handle_get_eval_history(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    query_res: Result<Query<EvalHistoryQuery>, QueryRejection>,
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
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                &format!("Invalid query parameters: {e}"),
                None,
                None,
                request_id,
            );
        }
    };

    let limit = match query.limit {
        None => DEFAULT_HISTORY_LIMIT,
        Some(v) if (1..=MAX_HISTORY_LIMIT as i64).contains(&v) => v as usize,
        Some(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "limit must be an integer between 1 and 100",
                Some("limit"),
                None,
                request_id,
            );
        }
    };

    let tier_filter = match query
        .tier
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some(raw) => match EvalTier::parse(raw) {
            Some(t) => Some(t),
            None => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_ERROR",
                    "tier must be one of: pr, stage, release",
                    Some("tier"),
                    None,
                    request_id,
                );
            }
        },
    };

    let project_id_filter = query
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(ref pid) = project_id_filter {
        if pid.len() > 128 {
            return error_response(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "projectId must be <= 128 characters",
                Some("projectId"),
                None,
                request_id,
            );
        }
    }

    let pool = state.registry.pool().clone();
    let workspace_root = pool.workspace_root().to_path_buf();
    let tenant_id = principal.tenant_id;

    let result = tokio::task::spawn_blocking(move || {
        list_eval_history(
            &pool,
            &workspace_root,
            &tenant_id,
            limit,
            tier_filter,
            project_id_filter.as_deref(),
        )
    })
    .await;

    match result {
        Ok(Ok(data)) => ok_response(data, request_id),
        Ok(Err(EvalRunnerError::Storage(msg))) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_ERROR",
            &msg,
            None,
            None,
            request_id,
        ),
        Ok(Err(_)) | Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "EVAL_HISTORY_FAILED",
            "Failed reading evaluation benchmark history",
            None,
            None,
            request_id,
        ),
    }
}

pub fn eval_certification_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/eval/run", post(handle_post_eval_run))
        .route("/api/v1/eval/history", get(handle_get_eval_history))
}
