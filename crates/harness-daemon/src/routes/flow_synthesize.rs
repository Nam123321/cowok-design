// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon POST /api/v1/flows/synthesize & GET /api/v1/flows/substates with V22 flow_node_substates & V21 decision_audit_log persistence)
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Bytes,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bot_fleet::DecisionCascadeClient;
use chrono::Utc;
use domain::decision::{
    CascadeStage, ConfidenceKind, DecisionAuditRecord, DecisionCandidate, DecisionRouteInput,
    DecisionTier,
};
use domain::HarnessError;
use flow_graph::synthesizer::{
    list_flow_substates, synthesize_and_persist_flow, validate_synthesize_request,
    FlowSynthesizeRequest, FlowTier, StoredFlowSubstateDto,
};
use rusqlite::{params, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::bot_fleet::load_tenant_byok_credentials;
use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress,
};

const MAX_SYNTHESIZE_BODY_BYTES: usize = 262_144;
const DECISION_CASCADE_TIMEOUT_MS: u64 = 2_500;
const DECISION_EXTERNAL_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
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
pub struct FlowSubstatesQuery {
    pub project_id: String,
    #[serde(default)]
    pub flow_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowSubstatesListData {
    pub project_id: String,
    pub items: Vec<StoredFlowSubstateDto>,
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
                "Flow synthesizer routes require verified session token, loopback HMAC, or superadmin Bearer",
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
            "Flow synthesis requires editor or admin role",
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
    let project_uuid = Uuid::parse_str(project_id.trim()).map_err(|_| {
        error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be a valid UUID",
            Some("projectId".to_string()),
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
                    request_id.to_string(),
                ));
            }
            Err(HarnessError::BoundaryViolation { .. }) => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "PROJECT_TENANT_MISMATCH",
                    "Cross-tenant project access forbidden",
                    Some("projectId".to_string()),
                    request_id.to_string(),
                ));
            }
            Err(_) => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "FORBIDDEN",
                    "Project access denied",
                    Some("projectId".to_string()),
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
                request_id.to_string(),
            )
        })?;

    if principal.tenant_id.trim() != tenant_id.trim() {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "Cross-tenant project access forbidden",
            Some("projectId".to_string()),
            request_id.to_string(),
        ));
    }

    Ok((tenant_id, normalized_project_id))
}

fn map_flow_service_error(err: flow_graph::FlowServiceError, request_id: String) -> Response {
    let status = StatusCode::from_u16(err.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    error_response(status, err.code, &err.message, None, request_id)
}

fn sha256_hex_for_synthesis(req: &FlowSynthesizeRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(req.project_id.as_bytes());
    hasher.update(b":");
    hasher.update(req.brief_summary.as_bytes());
    for s in &req.screens {
        hasher.update(b"|");
        hasher.update(s.screen_key.as_bytes());
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn build_synthesis_decision_input(
    project_id: &str,
    req: &FlowSynthesizeRequest,
) -> DecisionRouteInput {
    let screen_context = req
        .screens
        .iter()
        .map(|s| {
            format!(
                "{} {} {}",
                s.screen_key,
                s.title,
                s.fmea_failure_mode.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let candidates = vec![
        DecisionCandidate {
            id: "synth-3tier-fmea".to_string(),
            label: "SYNTHESIZE_3TIER_DAG_WITH_FMEA".to_string(),
            text: "flow dag screen checkout payment onboarding recovery offline timeout fmea cluster workbench".to_string(),
            de_score_bps: 8_600,
        },
        DecisionCandidate {
            id: "synth-happy-path".to_string(),
            label: "SYNTHESIZE_HAPPY_PATH_CLUSTER".to_string(),
            text: "happy path linear minimal simple sequence wizard steps".to_string(),
            de_score_bps: 8_100,
        },
        DecisionCandidate {
            id: "synth-recovery".to_string(),
            label: "SYNTHESIZE_RECOVERY_BRANCHES".to_string(),
            text: "fallback error empty retry loopback guard abort".to_string(),
            de_score_bps: 7_900,
        },
    ];
    DecisionRouteInput {
        project_id: project_id.to_string(),
        stage: CascadeStage::Decision1,
        prompt: req.brief_summary.clone(),
        context: screen_context,
        untrusted_artifact_sha256: sha256_hex_for_synthesis(req),
        attempt_number: 1,
        prior_verdicts: Vec::new(),
        stale_file_sha256: None,
        current_file_sha256: None,
        candidates,
        rubric_pair: None,
        prefer_external_tier: false,
    }
}

fn persist_flow_decision_audit_immediate(
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
    spec_registry::retry_on_busy("flow_synthesize.decision_audit_log.insert", || {
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

async fn run_verdict_shield_for_synthesis(
    state: &DaemonAppState,
    tenant_id: &str,
    project_id: &str,
    req: &FlowSynthesizeRequest,
) -> Result<ConfidenceKind, HarnessError> {
    let vault_state = state.clone();
    let tenant_for_vault = tenant_id.to_string();
    let preloaded_creds = tokio::task::spawn_blocking(move || {
        let (creds, _) = load_tenant_byok_credentials(&vault_state, &tenant_for_vault);
        creds
    })
    .await
    .unwrap_or_default();

    let creds_arc = Arc::new(preloaded_creds);
    let client = DecisionCascadeClient::new()
        .with_external_timeout_ms(DECISION_EXTERNAL_TIMEOUT_MS)
        .with_vault_lookup(Arc::new(move |_tid: &str| (*creds_arc).clone()));

    let decision_input = build_synthesis_decision_input(project_id, req);
    let started = Instant::now();
    let routed = tokio::time::timeout(
        Duration::from_millis(DECISION_CASCADE_TIMEOUT_MS),
        client.route(tenant_id, &decision_input),
    )
    .await;

    let (
        confidence_kind,
        tier_used,
        verdict_label,
        rubric_swap_consistent,
        attempt_number,
        breaker_tripped,
        latency_ms,
    ) = match routed {
        Ok(Ok(out)) => (
            out.confidence_kind,
            out.tier_used,
            out.verdict_label,
            out.rubric_swap_consistent,
            out.attempt_number,
            out.breaker_tripped,
            out.latency_ms,
        ),
        Ok(Err(_)) | Err(_) => (
            ConfidenceKind::DeterministicFallback,
            DecisionTier::HeuristicFallback,
            "SYNTHESIZE_3TIER_DAG_FALLBACK".to_string(),
            None,
            1,
            false,
            started.elapsed().as_millis() as u64,
        ),
    };

    let audit_record = DecisionAuditRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.to_string(),
        project_id: project_id.to_string(),
        cascade_stage: CascadeStage::Decision1,
        tier_used,
        confidence_kind,
        verdict_label,
        rubric_swap_consistent,
        attempt_number,
        breaker_tripped,
        stale_file_sha256: None,
        latency_ms,
        created_at: Utc::now().timestamp_millis(),
    };

    let persist_state = state.clone();
    tokio::task::spawn_blocking(move || {
        persist_flow_decision_audit_immediate(&persist_state, &audit_record)
    })
    .await
    .map_err(|_| HarnessError::SqliteBusy {
        details: "decision audit worker join failed".to_string(),
    })??;

    Ok(confidence_kind)
}

/// `POST /api/v1/flows/synthesize` — Synthesizes 3-Tier Flow DAG (`<= 8` happy-path default clusters),
/// invokes real `bot_fleet::DecisionCascadeClient` (`VerdictShield`, 0 raw floats) + V21 audit log,
/// enforces 3-Color DFS guard, and persists atomically in SQLite WAL.
pub async fn handle_synthesize_flow(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller_opt: Option<Extension<VerifiedLegacyIngress>>,
    principal_opt: Option<Extension<FlowPrincipal>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    if body.len() > MAX_SYNTHESIZE_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Request body exceeds 256 KB limit",
            None,
            request_id,
        );
    }

    let caller_ref = caller_opt.as_ref().map(|e| &e.0);
    let principal_ref = principal_opt.as_ref().map(|e| &e.0);
    let principal = match resolve_principal(caller_ref, principal_ref, &headers, &request_id) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(&principal, &request_id) {
        return resp;
    }

    let mut req: FlowSynthesizeRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(err) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid flow synthesis payload: {err}"),
                None,
                request_id,
            );
        }
    };

    if let Err(err) = validate_synthesize_request(&req) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &err.message,
            None,
            request_id,
        );
    }

    let caller_owned = caller_opt.map(|e| e.0);
    let state_clone = state.clone();
    let project_id_raw = req.project_id.clone();
    let headers_clone = headers.clone();
    let req_id_clone = request_id.clone();

    let access_res = tokio::task::spawn_blocking(move || {
        verify_project_tenant_access(
            &state_clone,
            &project_id_raw,
            &headers_clone,
            caller_owned.as_ref(),
            &principal,
            &req_id_clone,
        )
    })
    .await;

    let (tenant_id, normalized_project_id) = match access_res {
        Ok(Ok(pair)) => pair,
        Ok(Err(resp)) => return resp,
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Tenant access check failed unexpectedly",
                None,
                request_id,
            );
        }
    };

    req.project_id = normalized_project_id.clone();
    let decision_kind =
        match run_verdict_shield_for_synthesis(&state, &tenant_id, &normalized_project_id, &req)
            .await
        {
            Ok(kind) => kind,
            Err(HarnessError::SqliteBusy { details }) => {
                return error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    &format!("Storage busy persisting V21 decision_audit_log: {details}"),
                    None,
                    request_id,
                );
            }
            Err(err) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    &format!("Failed persisting V21 decision_audit_log: {err}"),
                    None,
                    request_id,
                );
            }
        };

    let pool = state.registry.pool().clone();
    let synth_res = tokio::task::spawn_blocking(move || {
        synthesize_and_persist_flow(&pool, &tenant_id, &req, decision_kind)
    })
    .await;

    match synth_res {
        Ok(Ok(plan)) => ok_response(plan, request_id),
        Ok(Err(err)) => map_flow_service_error(err, request_id),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Flow synthesis worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/flows/substates?projectId=...&flowTier=...` — Lists persisted 4-SubState fixture bindings.
pub async fn handle_list_flow_substates(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller_opt: Option<Extension<VerifiedLegacyIngress>>,
    principal_opt: Option<Extension<FlowPrincipal>>,
    Query(query): Query<FlowSubstatesQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let caller_ref = caller_opt.as_ref().map(|e| &e.0);
    let principal_ref = principal_opt.as_ref().map(|e| &e.0);
    let principal = match resolve_principal(caller_ref, principal_ref, &headers, &request_id) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };

    let parsed_tier = match query.flow_tier.as_deref() {
        Some(raw) if !raw.trim().is_empty() => match FlowTier::from_db_str(raw) {
            Some(t) => Some(t),
            None => {
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_REQUEST",
                    "flowTier must be PRIMARY or EDGE_CASE",
                    Some("flowTier".to_string()),
                    request_id,
                );
            }
        },
        _ => None,
    };

    let caller_owned = caller_opt.map(|e| e.0);
    let state_clone = state.clone();
    let project_id_raw = query.project_id.clone();
    let headers_clone = headers.clone();
    let req_id_clone = request_id.clone();

    let access_res = tokio::task::spawn_blocking(move || {
        verify_project_tenant_access(
            &state_clone,
            &project_id_raw,
            &headers_clone,
            caller_owned.as_ref(),
            &principal,
            &req_id_clone,
        )
    })
    .await;

    let (tenant_id, normalized_project_id) = match access_res {
        Ok(Ok(pair)) => pair,
        Ok(Err(resp)) => return resp,
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Tenant access check failed unexpectedly",
                None,
                request_id,
            );
        }
    };

    let pool = state.registry.pool().clone();
    let pid_for_query = normalized_project_id.clone();
    let list_res = tokio::task::spawn_blocking(move || {
        list_flow_substates(&pool, &tenant_id, &pid_for_query, parsed_tier)
    })
    .await;

    match list_res {
        Ok(Ok(items)) => {
            let total_count = items.len();
            ok_response(
                FlowSubstatesListData {
                    project_id: normalized_project_id,
                    items,
                    total_count,
                },
                request_id,
            )
        }
        Ok(Err(err)) => map_flow_service_error(err, request_id),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Flow substate query worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

pub fn build_flow_synthesize_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/flows/synthesize", post(handle_synthesize_flow))
        .route("/api/v1/flows/substates", get(handle_list_flow_substates))
}
