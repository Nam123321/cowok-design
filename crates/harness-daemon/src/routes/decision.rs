// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon /api/v1/decision/route & /api/v1/decision/audit with V21 decision_audit_log persistence)
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bot_fleet::{DecisionCascadeClient, DecisionCascadeError};
use chrono::Utc;
use domain::decision::{
    CascadeStage, ConfidenceKind, DecisionAuditRecord, DecisionRouteInput, DecisionTier,
    VerdictShieldOutput,
};
use domain::HarnessError;
use rusqlite::{params, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::bot_fleet::load_tenant_byok_credentials;
use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress,
};

const MAX_DECISION_BODY_BYTES: usize = 262_144;
const DEFAULT_AUDIT_LIMIT: i64 = 50;
const MAX_AUDIT_LIMIT: i64 = 200;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionRouteResponseData {
    pub audit_id: String,
    pub output: VerdictShieldOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionAuditListData {
    pub project_id: String,
    pub items: Vec<DecisionAuditRecord>,
    pub total_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionAuditQuery {
    pub project_id: String,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub stage: Option<String>,
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

/// Strict non-legacy-Bearer auth check matching `bot_fleet.rs` / `de_runtime.rs` (defense-in-depth C-01).
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
                "Decision Cascade routes require verified session token, loopback HMAC, or superadmin Bearer",
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
            "Decision routing requires editor or admin role",
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
) -> Result<(String, String), Response> {
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
    let tenant_id: String = conn
        .query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![normalized_project_id],
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

    Ok((tenant_id, normalized_project_id))
}

async fn verify_project_tenant_access_async(
    state: DaemonAppState,
    project_id: String,
    headers: HeaderMap,
    caller: Option<VerifiedLegacyIngress>,
    principal: FlowPrincipal,
    request_id: String,
) -> Result<(String, String), Response> {
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

fn persist_decision_audit_immediate(
    state: &DaemonAppState,
    record: &DecisionAuditRecord,
) -> Result<(), HarnessError> {
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(|e| HarnessError::SqliteBusy {
            details: format!("pool connection failed: {e}"),
        })?;
    spec_registry::retry_on_busy("decision_audit_log.insert", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rubric_int: Option<i64> = record.rubric_swap_consistent.map(i64::from);
        let breaker_int: i64 = i64::from(record.breaker_tripped);
        tx.execute(
            "INSERT INTO decision_audit_log (
                id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                stale_file_sha256, latency_ms, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.id,
                record.tenant_id,
                record.project_id,
                record.cascade_stage.as_db_str(),
                record.tier_used.as_db_str(),
                record.confidence_kind.as_db_str(),
                record.verdict_label,
                rubric_int,
                i64::from(record.attempt_number),
                breaker_int,
                record.stale_file_sha256,
                record.latency_ms as i64,
                record.created_at,
            ],
        )?;
        tx.commit()?;
        Ok(())
    })
}

/// `POST /api/v1/decision/route`
pub async fn handle_decision_route(
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
    if body.len() > MAX_DECISION_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Decision route payload exceeds 256 KiB ceiling",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: DecisionRouteInput = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid decision route payload: {e}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let (tenant_id, normalized_project_id) = match verify_project_tenant_access_async(
        state.clone(),
        req.project_id.clone(),
        headers.clone(),
        caller.map(|e| e.0),
        principal,
        request_id.clone(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };

    let vault_state = state.clone();
    let tenant_for_vault = tenant_id.clone();
    let preloaded_creds = if req.stage == CascadeStage::RubricShield {
        Vec::new()
    } else {
        tokio::task::spawn_blocking(move || {
            let (creds, _) = load_tenant_byok_credentials(&vault_state, &tenant_for_vault);
            creds
        })
        .await
        .unwrap_or_default()
    };
    let creds_arc = Arc::new(preloaded_creds);
    let client = DecisionCascadeClient::new()
        .with_vault_lookup(Arc::new(move |_tid: &str| (*creds_arc).clone()));

    let output = match client.route(&tenant_id, &req).await {
        Ok(out) => out,
        Err(DecisionCascadeError::StaleFileConflict {
            expected_sha256,
            current_sha256,
        }) => {
            return error_response(
                StatusCode::CONFLICT,
                "STALE_FILE_OCC",
                "Artifact SHA-256 changed since decision snapshot (StaleFile OCC conflict)",
                Some("staleFileSha256".to_string()),
                Some(serde_json::json!({
                    "expectedSha256": expected_sha256,
                    "currentSha256": current_sha256,
                })),
                request_id,
            );
        }
        Err(DecisionCascadeError::InvalidInput(msg)) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &msg,
                None,
                None,
                request_id,
            );
        }
    };

    let audit_id = Uuid::new_v4().to_string();
    let normalized_stale_sha = req
        .stale_file_sha256
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase());
    let record = DecisionAuditRecord {
        id: audit_id.clone(),
        tenant_id,
        project_id: normalized_project_id,
        cascade_stage: output.cascade_stage,
        tier_used: output.tier_used,
        confidence_kind: output.confidence_kind,
        verdict_label: output.verdict_label.clone(),
        rubric_swap_consistent: output.rubric_swap_consistent,
        attempt_number: output.attempt_number,
        breaker_tripped: output.breaker_tripped,
        stale_file_sha256: normalized_stale_sha,
        latency_ms: output.latency_ms,
        created_at: Utc::now().timestamp_millis(),
    };

    let persist_state = state.clone();
    let persist_res = tokio::task::spawn_blocking(move || {
        persist_decision_audit_immediate(&persist_state, &record)
    })
    .await;

    match persist_res {
        Ok(Ok(())) => ok_response(DecisionRouteResponseData { audit_id, output }, request_id),
        Ok(Err(HarnessError::SqliteBusy { details })) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            &format!("Storage busy persisting V21 decision_audit_log: {details}"),
            None,
            None,
            request_id,
        ),
        Ok(Err(err)) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            &format!("Failed persisting V21 decision_audit_log: {err}"),
            None,
            None,
            request_id,
        ),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Decision audit persistence worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/decision/audit`
pub async fn handle_get_decision_audit(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    Query(query): Query<DecisionAuditQuery>,
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

    let (tenant_id, normalized_project_id) = match verify_project_tenant_access_async(
        state.clone(),
        query.project_id.clone(),
        headers.clone(),
        caller.map(|e| e.0),
        principal,
        request_id.clone(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };

    let stage_filter = match query
        .stage
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => match CascadeStage::from_db_str(raw) {
            Some(st) => Some(st),
            None => {
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_REQUEST",
                    "stage must be DECISION_1, DECISION_2, or RUBRIC_SHIELD",
                    Some("stage".to_string()),
                    None,
                    request_id,
                );
            }
        },
        None => None,
    };

    let limit = query
        .limit
        .unwrap_or(DEFAULT_AUDIT_LIMIT)
        .clamp(1, MAX_AUDIT_LIMIT);
    let state_clone = state.clone();
    let project_for_resp = normalized_project_id.clone();

    let rows_res = tokio::task::spawn_blocking(
        move || -> Result<(Vec<DecisionAuditRecord>, usize), String> {
            let conn = state_clone
                .registry
                .pool()
                .connection()
                .map_err(|e| format!("pool connection failed: {e}"))?;
            let mut records = Vec::new();
            let total_count_i64: i64 = if let Some(st) = stage_filter {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM decision_audit_log
                         WHERE tenant_id = ?1 AND project_id = ?2 AND cascade_stage = ?3",
                        params![tenant_id, normalized_project_id, st.as_db_str()],
                        |r| r.get(0),
                    )
                    .map_err(|e| format!("count query failed: {e}"))?;
                let mut stmt = conn
                    .prepare(
                        "SELECT id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                                verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                                stale_file_sha256, latency_ms, created_at
                         FROM decision_audit_log
                         WHERE tenant_id = ?1 AND project_id = ?2 AND cascade_stage = ?3
                         ORDER BY created_at DESC
                         LIMIT ?4",
                    )
                    .map_err(|e| format!("prepare failed: {e}"))?;
                let iter = stmt
                    .query_map(
                        params![tenant_id, normalized_project_id, st.as_db_str(), limit],
                        map_audit_row,
                    )
                    .map_err(|e| format!("query failed: {e}"))?;
                for row in iter {
                    records.push(row.map_err(|e| format!("row decode failed: {e}"))?);
                }
                count
            } else {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM decision_audit_log
                         WHERE tenant_id = ?1 AND project_id = ?2",
                        params![tenant_id, normalized_project_id],
                        |r| r.get(0),
                    )
                    .map_err(|e| format!("count query failed: {e}"))?;
                let mut stmt = conn
                    .prepare(
                        "SELECT id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                                verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                                stale_file_sha256, latency_ms, created_at
                         FROM decision_audit_log
                         WHERE tenant_id = ?1 AND project_id = ?2
                         ORDER BY created_at DESC
                         LIMIT ?3",
                    )
                    .map_err(|e| format!("prepare failed: {e}"))?;
                let iter = stmt
                    .query_map(
                        params![tenant_id, normalized_project_id, limit],
                        map_audit_row,
                    )
                    .map_err(|e| format!("query failed: {e}"))?;
                for row in iter {
                    records.push(row.map_err(|e| format!("row decode failed: {e}"))?);
                }
                count
            };
            Ok((records, total_count_i64.max(0) as usize))
        },
    )
    .await;

    match rows_res {
        Ok(Ok((items, total_count))) => ok_response(
            DecisionAuditListData {
                project_id: project_for_resp,
                items,
                total_count,
            },
            request_id,
        ),
        Ok(Err(msg)) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            &format!("Failed querying V21 decision_audit_log: {msg}"),
            None,
            None,
            request_id,
        ),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Decision audit query worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

fn map_audit_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DecisionAuditRecord> {
    let stage_raw: String = r.get(3)?;
    let tier_raw: String = r.get(4)?;
    let conf_raw: String = r.get(5)?;
    let rubric_int: Option<i64> = r.get(7)?;
    let attempt_i64: i64 = r.get(8)?;
    let breaker_int: i64 = r.get(9)?;
    let latency_i64: i64 = r.get(11)?;

    Ok(DecisionAuditRecord {
        id: r.get(0)?,
        tenant_id: r.get(1)?,
        project_id: r.get(2)?,
        cascade_stage: CascadeStage::from_db_str(&stage_raw).unwrap_or(CascadeStage::Decision1),
        tier_used: DecisionTier::from_db_str(&tier_raw).unwrap_or(DecisionTier::HeuristicFallback),
        confidence_kind: ConfidenceKind::from_db_str(&conf_raw)
            .unwrap_or(ConfidenceKind::DeterministicFallback),
        verdict_label: r.get(6)?,
        rubric_swap_consistent: rubric_int.map(|v| v != 0),
        attempt_number: attempt_i64.clamp(1, 3) as u8,
        breaker_tripped: breaker_int != 0,
        stale_file_sha256: r.get(10)?,
        latency_ms: latency_i64.max(0) as u64,
        created_at: r.get(12)?,
    })
}

/// Standalone Axum sub-router for Story 09.2 (`POST /api/v1/decision/route`, `GET /api/v1/decision/audit`).
/// Parent Coordinator mounts this inside `crates/harness-daemon/src/routes/legacy_router.rs` (or `routes/mod.rs`).
pub fn router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/decision/route", post(handle_decision_route))
        .route("/api/v1/decision/audit", get(handle_get_decision_audit))
}

/// Alias matching `build_bot_fleet_router` / `build_de_runtime_router` naming convention.
pub fn build_decision_router() -> Router<DaemonAppState> {
    router()
}
