// @story 08.7
// @implements AC-04, AC-05 (Bridge 1: POST /api/v1/learn/studio-corrections -> V18a/V20 gate_failure_ledger at hit_count >= 3, Zero-PII, HUMAN_ONLY)
use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, require_human_workbench_actor, FlowPrincipal,
    LegacyAuthKind, VerifiedActorContext, VerifiedLegacyIngress,
};
use crate::routes::DaemonAppState;

pub const BRIDGE1_ESCALATION_THRESHOLD: i64 = 3;
const MAX_CORRECTION_BODY_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudioCorrectionKind {
    ChartTypeOverride,
    QaAutofixClick,
    PaletteContrastFix,
    DensityModeOverride,
    CalloutPlacementFix,
}

impl StudioCorrectionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChartTypeOverride => "chart_type_override",
            Self::QaAutofixClick => "qa_autofix_click",
            Self::PaletteContrastFix => "palette_contrast_fix",
            Self::DensityModeOverride => "density_mode_override",
            Self::CalloutPlacementFix => "callout_placement_fix",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordStudioCorrectionRequest {
    pub project_id: String,
    pub correction_kind: StudioCorrectionKind,
    pub signature_key: String,
    #[serde(default)]
    pub from_value: Option<String>,
    #[serde(default)]
    pub to_value: Option<String>,
    #[serde(default)]
    pub skill_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordStudioCorrectionResponse {
    pub signature_key: String,
    pub hit_count: i64,
    pub escalated_to_ledger: bool,
    pub ledger_entry_id: Option<String>,
    pub opt_out_skipped: bool,
}

#[derive(Debug)]
pub enum StudioCorrectionError {
    Validation {
        field: &'static str,
        message: String,
    },
    ProjectNotFound(String),
    Storage(String),
}

impl std::fmt::Display for StudioCorrectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation { field, message } => {
                write!(f, "validation error on {field}: {message}")
            }
            Self::ProjectNotFound(msg) => write!(f, "project not found or forbidden: {msg}"),
            Self::Storage(msg) => write!(f, "storage error: {msg}"),
        }
    }
}

impl std::error::Error for StudioCorrectionError {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

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

fn is_valid_signature_key(s: &str) -> bool {
    (3..=160).contains(&s.len())
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b':' | b'_' | b'-')
        })
}

fn is_safe_zero_pii_token(s: &str, max_len: usize) -> bool {
    !s.is_empty()
        && s.len() <= max_len
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b':' | b'_' | b'-')
        })
}

/// Validates strictly Zero-PII structural fields before any database access.
pub fn validate_studio_correction_request(
    req: &RecordStudioCorrectionRequest,
) -> Result<(), StudioCorrectionError> {
    let pid = req.project_id.trim();
    if pid.is_empty() || pid.len() > 128 {
        return Err(StudioCorrectionError::Validation {
            field: "projectId",
            message: "projectId must be 1..=128 characters".to_string(),
        });
    }
    if !is_valid_signature_key(&req.signature_key) {
        return Err(StudioCorrectionError::Validation {
            field: "signatureKey",
            message:
                "signatureKey must be 3..=160 lowercase chars matching ^[a-z0-9:_-]+$ (Zero-PII)"
                    .to_string(),
        });
    }
    if let Some(ref v) = req.from_value {
        if !is_safe_zero_pii_token(v, 64) {
            return Err(StudioCorrectionError::Validation {
                field: "fromValue",
                message: "fromValue must be a compact Zero-PII token (1..=64 safe chars)"
                    .to_string(),
            });
        }
    }
    if let Some(ref v) = req.to_value {
        if !is_safe_zero_pii_token(v, 64) {
            return Err(StudioCorrectionError::Validation {
                field: "toValue",
                message: "toValue must be a compact Zero-PII token (1..=64 safe chars)".to_string(),
            });
        }
    }
    if let Some(ref s) = req.skill_id {
        if !is_safe_zero_pii_token(s, 128) {
            return Err(StudioCorrectionError::Validation {
                field: "skillId",
                message: "skillId must be a valid skill identifier (1..=128 safe chars)"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn is_telemetry_opted_out(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT telemetry_enabled FROM telemetry_preferences ORDER BY updated_at DESC LIMIT 1",
        [],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .is_some_and(|enabled| enabled == 0)
}

fn compute_bridge1_fingerprint(signature_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"studio_correction_bridge1\0");
    hasher.update(signature_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Core Bridge 1 atomic persistence (`TransactionBehavior::Immediate`):
/// 1. Validates Zero-PII structural fields.
/// 2. Respects telemetry opt-out (`telemetry_preferences.telemetry_enabled = 0`).
/// 3. Increments `studio_correction_signals.hit_count` atomically.
/// 4. Escalates to `V18a`/`V20` `gate_failure_ledger` when `hit_count >= 3` with `HUMAN_ONLY` status (`PROPOSED`, never auto-activating skills).
pub fn record_studio_correction_in_db(
    conn: &mut Connection,
    tenant_id: &str,
    req: &RecordStudioCorrectionRequest,
) -> Result<RecordStudioCorrectionResponse, StudioCorrectionError> {
    validate_studio_correction_request(req)?;

    let now_ms = Utc::now().timestamp_millis();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

    // Always verify tenant project ownership BEFORE telemetry opt-out short-circuit
    let project_exists: Option<String> = tx
        .query_row(
            "SELECT id FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![req.project_id.trim(), tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;
    if project_exists.is_none() {
        return Err(StudioCorrectionError::ProjectNotFound(
            req.project_id.clone(),
        ));
    }

    if is_telemetry_opted_out(&tx) {
        return Ok(RecordStudioCorrectionResponse {
            signature_key: req.signature_key.clone(),
            hit_count: 0,
            escalated_to_ledger: false,
            ledger_entry_id: None,
            opt_out_skipped: true,
        });
    }

    let signal_id = Uuid::new_v4().to_string();
    let kind_str = req.correction_kind.as_str();
    tx.execute(
        "INSERT INTO studio_correction_signals (
            id, tenant_id, project_id, correction_kind, signature_key,
            from_value, to_value, skill_id, hit_count, escalated_ledger_id,
            first_seen_at, last_seen_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, NULL, ?9, ?9)
         ON CONFLICT(tenant_id, project_id, signature_key) DO UPDATE SET
            hit_count = studio_correction_signals.hit_count + 1,
            from_value = COALESCE(excluded.from_value, studio_correction_signals.from_value),
            to_value = COALESCE(excluded.to_value, studio_correction_signals.to_value),
            skill_id = COALESCE(excluded.skill_id, studio_correction_signals.skill_id),
            last_seen_at = excluded.last_seen_at",
        params![
            signal_id,
            tenant_id,
            req.project_id.trim(),
            kind_str,
            req.signature_key,
            req.from_value,
            req.to_value,
            req.skill_id,
            now_ms,
        ],
    )
    .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

    let (row_id, hit_count, mut ledger_entry_id): (String, i64, Option<String>) = tx
        .query_row(
            "SELECT id, hit_count, escalated_ledger_id \
             FROM studio_correction_signals \
             WHERE tenant_id = ?1 AND project_id = ?2 AND signature_key = ?3",
            params![tenant_id, req.project_id.trim(), req.signature_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

    let mut escalated_to_ledger = ledger_entry_id.is_some();

    if hit_count >= BRIDGE1_ESCALATION_THRESHOLD && ledger_entry_id.is_none() {
        let fingerprint = compute_bridge1_fingerprint(&req.signature_key);
        let new_ledger_id = Uuid::new_v4().to_string();
        let rule_id = format!("bridge1:{kind_str}");
        let sample_msg = format!(
            "source=studio_correction_bridge1 approval_policy=HUMAN_ONLY signature={} skill={}",
            req.signature_key,
            req.skill_id.as_deref().unwrap_or("data-viz-storyteller")
        );
        tx.execute(
            "INSERT INTO gate_failure_ledger (
                id, tenant_id, project_id, rule_id, fingerprint_sha256,
                sample_message, hit_count, slpr_score, promoted_card_id, status, last_seen_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0.0, NULL, 'PROPOSED', ?8)
             ON CONFLICT(tenant_id, project_id, fingerprint_sha256) DO UPDATE SET
                hit_count = MAX(gate_failure_ledger.hit_count, excluded.hit_count),
                status = CASE WHEN gate_failure_ledger.status = 'ACCUMULATING' THEN 'PROPOSED' ELSE gate_failure_ledger.status END,
                last_seen_at = excluded.last_seen_at",
            params![
                new_ledger_id,
                tenant_id,
                req.project_id.trim(),
                rule_id,
                fingerprint,
                sample_msg,
                hit_count,
                now_ms,
            ],
        )
        .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

        let actual_ledger_id: String = tx
            .query_row(
                "SELECT id FROM gate_failure_ledger \
                 WHERE tenant_id = ?1 AND project_id = ?2 AND fingerprint_sha256 = ?3",
                params![tenant_id, req.project_id.trim(), fingerprint],
                |r| r.get(0),
            )
            .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

        tx.execute(
            "UPDATE studio_correction_signals SET escalated_ledger_id = ?1 WHERE id = ?2",
            params![actual_ledger_id, row_id],
        )
        .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

        ledger_entry_id = Some(actual_ledger_id);
        escalated_to_ledger = true;
    }

    tx.commit()
        .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;

    Ok(RecordStudioCorrectionResponse {
        signature_key: req.signature_key.clone(),
        hit_count,
        escalated_to_ledger,
        ledger_entry_id,
        opt_out_skipped: false,
    })
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

fn json_ok<T: Serialize>(data: T, request_id: String) -> Response {
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

fn json_err(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    field: Option<&str>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.into(),
                field: field.map(str::to_string),
                request_id,
            },
        }),
    )
        .into_response()
}

fn extract_principal_strict(
    headers: &HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    request_id: &str,
) -> Result<FlowPrincipal, Response> {
    if let Some(Extension(principal)) = flow_principal {
        return Ok(principal);
    }
    if let Some(Extension(legacy)) = legacy_ingress {
        if legacy.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
            return Err(json_err(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Legacy bearer authentication is not permitted on studio-corrections endpoint",
                None,
                request_id.to_string(),
            ));
        }
        return Ok(legacy.principal);
    }
    Err(json_err(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        request_id.to_string(),
    ))
}

fn enforce_human_workbench_or_superadmin(
    state: &DaemonAppState,
    headers: &HeaderMap,
    principal: &FlowPrincipal,
    actor_ext: Option<&VerifiedActorContext>,
    request_id: &str,
) -> Result<(), Response> {
    if is_valid_superadmin_bearer(headers) {
        return Ok(());
    }
    if let Some(actor) = actor_ext {
        return require_human_workbench_actor(actor);
    }
    if let Some(human_tok) = headers
        .get("x-cowok-human-session")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Some(human_principal) = state.auth.get_valid_human_session(human_tok) {
            if human_principal.tenant_id == principal.tenant_id
                && human_principal.user_id == principal.user_id
            {
                return Ok(());
            }
        }
    }
    if let Some(sess_tok) = headers
        .get("x-cowok-session-token")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Some(human_principal) = state.auth.get_valid_human_session(sess_tok) {
            if human_principal.tenant_id == principal.tenant_id
                && human_principal.user_id == principal.user_id
            {
                return Ok(());
            }
        }
    }
    Err(json_err(
        StatusCode::FORBIDDEN,
        "HUMAN_APPROVAL_REQUIRED",
        "Bridge 1 studio corrections require an interactive HumanWorkbenchSession (AgentHmac/McpClient tokens are denied)",
        None,
        request_id.to_string(),
    ))
}

/// `POST /api/v1/learn/studio-corrections`
pub async fn handle_record_studio_correction(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    actor_ext: Option<Extension<VerifiedActorContext>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }
    if let Err(resp) = enforce_human_workbench_or_superadmin(
        &state,
        &headers,
        &principal,
        actor_ext.as_ref().map(|e| &e.0),
        &request_id,
    ) {
        return resp;
    }
    if body.len() > MAX_CORRECTION_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Studio correction payload exceeds 8 KiB ceiling",
            Some("body"),
            request_id,
        );
    }
    let req: RecordStudioCorrectionRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid /learn/studio-corrections JSON payload: {e}"),
                Some("body"),
                request_id,
            );
        }
    };

    let tenant_id = principal.tenant_id.clone();
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let mut conn = pool
            .connection()
            .map_err(|e| StudioCorrectionError::Storage(e.to_string()))?;
        record_studio_correction_in_db(&mut conn, &tenant_id, &req)
    })
    .await;

    match res {
        Ok(Ok(outcome)) => json_ok(outcome, request_id),
        Ok(Err(StudioCorrectionError::Validation { field, message })) => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            message,
            Some(field),
            request_id,
        ),
        Ok(Err(StudioCorrectionError::ProjectNotFound(_))) => json_err(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found for caller tenant",
            Some("projectId"),
            request_id,
        ),
        Ok(Err(StudioCorrectionError::Storage(msg))) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Studio correction worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

pub fn router() -> Router<DaemonAppState> {
    Router::new().route(
        "/api/v1/learn/studio-corrections",
        post(handle_record_studio_correction),
    )
}

pub fn build_learn_studio_corrections_router() -> Router<DaemonAppState> {
    router()
}
