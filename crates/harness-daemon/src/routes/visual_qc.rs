// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
use std::sync::{Arc, OnceLock};

use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use domain::HarnessError;
use html_forge::visual_qc::{
    apply_agent_structural_patch, apply_deterministic_lint_fix, audit_e2e_flow_completeness,
    compute_layout_sha1, discover_local_chromium_binary, CdpBreakerStatus,
    DeterministicLintFixResult, DomGeometryNode, FlowAuditEdge, FlowAuditNode,
    FlowCompletenessReport, PooledCdpEvaluator, QcIssue, QcTargetKind, QcVerdict,
    VisualQcEvalInput, CANONICAL_APP_VIEWPORTS, DEFAULT_SSIM_PASS_THRESHOLD, MAX_AUTOFIX_ATTEMPTS,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::retry_on_busy;
use uuid::Uuid;

use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::{is_valid_superadmin_bearer, LegacyAuthKind, VerifiedLegacyIngress};

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_HTML_LEN: usize = 512 * 1024;

pub const V26_FALLBACK_DDL: &str =
    include_str!("../../../spec-registry/migrations/V26__story_07_1_qc_gate_runs.sql");

static SHARED_CDP_POOL: OnceLock<Arc<PooledCdpEvaluator>> = OnceLock::new();

pub fn shared_cdp_evaluator() -> Arc<PooledCdpEvaluator> {
    Arc::clone(
        SHARED_CDP_POOL
            .get_or_init(|| Arc::new(PooledCdpEvaluator::new(discover_local_chromium_binary()))),
    )
}

pub fn ensure_v26_tables(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(V26_FALLBACK_DDL)
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
pub struct QcRunRequest {
    pub project_id: String,
    #[serde(default = "default_target_kind")]
    pub target_kind: QcTargetKind,
    pub target_id: String,
    #[serde(default)]
    pub html: String,
    #[serde(default = "default_constraints_json")]
    pub constraints_json: String,
    #[serde(default = "default_viewports")]
    pub viewports: Vec<u32>,
    #[serde(default = "default_fixture_ref")]
    pub fixture_ref: String,
    #[serde(default)]
    pub geometry_nodes: Vec<DomGeometryNode>,
    #[serde(default)]
    pub flow_nodes: Vec<FlowAuditNode>,
    #[serde(default)]
    pub flow_edges: Vec<FlowAuditEdge>,
    #[serde(default)]
    pub reference_rgba: Option<Vec<u8>>,
    #[serde(default)]
    pub candidate_rgba: Option<Vec<u8>>,
    #[serde(default)]
    pub frame_width: u32,
    #[serde(default)]
    pub frame_height: u32,
    #[serde(default = "default_ssim_threshold")]
    pub ssim_threshold: f64,
    #[serde(default)]
    pub force_refresh: bool,
    #[serde(default)]
    pub force_no_browser: bool,
}

fn default_target_kind() -> QcTargetKind {
    QcTargetKind::AppScreen
}

fn default_constraints_json() -> String {
    "{}".to_string()
}

fn default_viewports() -> Vec<u32> {
    CANONICAL_APP_VIEWPORTS.to_vec()
}

fn default_fixture_ref() -> String {
    "default".to_string()
}

fn default_ssim_threshold() -> f64 {
    DEFAULT_SSIM_PASS_THRESHOLD
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcRunResponseData {
    pub qc_run_id: String,
    pub project_id: String,
    pub target_kind: QcTargetKind,
    pub target_id: String,
    pub layout_sha1: String,
    pub viewports: Vec<u32>,
    pub verdict: QcVerdict,
    pub p0_count: usize,
    pub p1_count: usize,
    pub ssim_score: Option<f64>,
    pub issues: Vec<QcIssue>,
    pub autofix_attempt: u8,
    pub max_autofix_attempts: u8,
    pub cache_hit: bool,
    pub used_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_completeness: Option<FlowCompletenessReport>,
    pub rules_evaluated: usize,
    pub breaker_status: CdpBreakerStatus,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum QcAutoFixMode {
    #[serde(rename = "DETERMINISTIC_LINT_FIX")]
    DeterministicLintFix,
    #[serde(rename = "AGENT_PATCH")]
    AgentPatch,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QcAutoFixRequest {
    pub project_id: String,
    pub qc_run_id: String,
    pub issue_ids: Vec<String>,
    pub mode: QcAutoFixMode,
    pub attempt_number: u32,
    #[serde(default)]
    pub current_html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcAutoFixResponseData {
    pub qc_run_id: String,
    pub project_id: String,
    pub mode: QcAutoFixMode,
    pub attempt_number: u8,
    pub max_autofix_attempts: u8,
    pub remaining_attempts: u8,
    pub patched_html: String,
    pub previous_html: String,
    pub fixed_issue_ids: Vec<String>,
    pub unfixable_issue_ids: Vec<String>,
    pub elapsed_ms: f64,
    pub llm_tokens_used: u32,
    pub diff_summary: Vec<String>,
    pub rollback_supported: bool,
    pub requires_agent_patch: bool,
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

fn err_resp(
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

fn ok_resp<T: Serialize>(data: T, request_id: String) -> Response {
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

fn require_caller<'a>(
    caller: Option<&'a VerifiedLegacyIngress>,
    headers: &HeaderMap,
    request_id: &str,
) -> Result<&'a VerifiedLegacyIngress, Response> {
    let verified = caller.ok_or_else(|| {
        err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Missing verified authentication credentials",
            None,
            None,
            request_id.to_string(),
        )
    })?;
    if verified.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
        return Err(err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Untrusted Bearer token rejected",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(verified)
}

fn require_editor_role(verified: &VerifiedLegacyIngress, request_id: &str) -> Result<(), Response> {
    let allowed = matches!(
        verified.principal.role.trim().to_ascii_lowercase().as_str(),
        "editor" | "admin" | "owner"
    );
    if !allowed {
        return Err(err_resp(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Visual QC mutation requires editor or admin role",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn resolve_tenant_for_project(
    state: &DaemonAppState,
    project_id: &str,
    headers: &HeaderMap,
    verified: &VerifiedLegacyIngress,
    request_id: &str,
) -> Result<String, Response> {
    let trimmed = project_id.trim();
    let project_uuid = Uuid::parse_str(trimmed).map_err(|_| {
        err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be a valid UUID",
            Some("projectId"),
            None,
            request_id.to_string(),
        )
    })?;
    match verify_anti_idor_with_caller(&state.registry, project_uuid, headers, Some(verified)) {
        Ok(_) => {
            let conn = state.registry.pool().connection().map_err(|_| {
                err_resp(
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
                    params![trimmed],
                    |r| r.get(0),
                )
                .map_err(|_| {
                    err_resp(
                        StatusCode::NOT_FOUND,
                        "PROJECT_NOT_FOUND",
                        "Project not found",
                        Some("projectId"),
                        None,
                        request_id.to_string(),
                    )
                })?;
            if matches!(
                verified.kind,
                LegacyAuthKind::HmacV2 | LegacyAuthKind::SessionToken
            ) && verified.principal.tenant_id.trim() != tenant_id.trim()
            {
                return Err(err_resp(
                    StatusCode::FORBIDDEN,
                    "PROJECT_TENANT_MISMATCH",
                    "Cross-tenant project access forbidden",
                    Some("projectId"),
                    None,
                    request_id.to_string(),
                ));
            }
            Ok(tenant_id)
        }
        Err(HarnessError::ProjectNotFound { .. }) => Err(err_resp(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found",
            Some("projectId"),
            None,
            request_id.to_string(),
        )),
        Err(_) => Err(err_resp(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "Project access denied",
            Some("projectId"),
            None,
            request_id.to_string(),
        )),
    }
}

fn normalized_viewports_for_cache(kind: QcTargetKind, requested: &[u32]) -> Vec<u32> {
    if kind == QcTargetKind::PresentationSlide {
        return vec![1920];
    }
    let mut vp: Vec<u32> = requested
        .iter()
        .copied()
        .filter(|&w| (320..=1920).contains(&w))
        .collect();
    if vp.is_empty() {
        vp.extend_from_slice(CANONICAL_APP_VIEWPORTS);
    }
    vp.sort_unstable_by(|a, b| b.cmp(a));
    vp.dedup();
    vp
}

fn hash_bytes_sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

fn compute_qc_cache_key(req: &QcRunRequest, norm_viewports: &[u32]) -> String {
    let base_sha1 = compute_layout_sha1(
        &req.html,
        &req.constraints_json,
        norm_viewports,
        &req.fixture_ref,
    );
    let effective_threshold = if req.ssim_threshold > 0.0 {
        req.ssim_threshold
    } else {
        DEFAULT_SSIM_PASS_THRESHOLD
    };
    let geom_bytes = serde_json::to_vec(&req.geometry_nodes).unwrap_or_default();
    let geom_digest = hash_bytes_sha256(&geom_bytes);

    let flow_nodes_bytes = serde_json::to_vec(&req.flow_nodes).unwrap_or_default();
    let flow_edges_bytes = serde_json::to_vec(&req.flow_edges).unwrap_or_default();
    let mut flow_hasher = Sha256::new();
    flow_hasher.update((flow_nodes_bytes.len() as u64).to_be_bytes());
    flow_hasher.update(&flow_nodes_bytes);
    flow_hasher.update((flow_edges_bytes.len() as u64).to_be_bytes());
    flow_hasher.update(&flow_edges_bytes);
    let flow_digest: [u8; 32] = flow_hasher.finalize().into();

    let mut rgba_hasher = Sha256::new();
    rgba_hasher.update(req.frame_width.to_be_bytes());
    rgba_hasher.update(req.frame_height.to_be_bytes());
    match req.reference_rgba.as_deref() {
        Some(buf) => {
            rgba_hasher.update([1u8]);
            rgba_hasher.update((buf.len() as u64).to_be_bytes());
            rgba_hasher.update(buf);
        }
        None => rgba_hasher.update([0u8]),
    }
    match req.candidate_rgba.as_deref() {
        Some(buf) => {
            rgba_hasher.update([1u8]);
            rgba_hasher.update((buf.len() as u64).to_be_bytes());
            rgba_hasher.update(buf);
        }
        None => rgba_hasher.update([0u8]),
    }
    let rgba_digest: [u8; 32] = rgba_hasher.finalize().into();

    let mut hasher = Sha256::new();
    for part in [
        base_sha1.as_bytes(),
        req.target_kind.as_db_str().as_bytes(),
        if req.force_no_browser { b"1" } else { b"0" },
        &effective_threshold.to_bits().to_be_bytes(),
        &geom_digest,
        &flow_digest,
        &rgba_digest,
    ] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(40);
    for byte in &digest[..20] {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Reusable async Visual QC evaluation entry point for both `POST /api/v1/qc/run`
/// and downstream gates (e.g. Story 10.2 `Builder != Scorer` verifier).
pub async fn evaluate_qc_for_project(
    state: &DaemonAppState,
    req: QcRunRequest,
) -> Result<QcRunResponseData, String> {
    if req.target_id.trim().is_empty() || req.target_id.len() > 160 {
        return Err("targetId must be 1..=160 chars".into());
    }
    if req.html.len() > MAX_HTML_LEN {
        return Err("HTML payload exceeds 512KB limit".into());
    }

    let pool = shared_cdp_evaluator();
    let norm_viewports = normalized_viewports_for_cache(req.target_kind, &req.viewports);
    let cache_sha1 = compute_qc_cache_key(&req, &norm_viewports);

    let has_browser_inputs = !req.geometry_nodes.is_empty()
        || (req.reference_rgba.is_some()
            && req.candidate_rgba.is_some()
            && req.frame_width > 0
            && req.frame_height > 0);
    let live_verification_ready =
        !req.force_no_browser && has_browser_inputs && pool.can_attempt_cdp();

    if !req.force_refresh {
        let conn = state
            .registry
            .pool()
            .connection()
            .map_err(|e| format!("storage connection unavailable: {e}"))?;
        ensure_v26_tables(&conn).map_err(|e| format!("ensure v26 failed: {e}"))?;

        type CachedQcRow = (
            String,
            String,
            String,
            String,
            String,
            i64,
            i64,
            Option<f64>,
            String,
            i64,
            i64,
        );
        let cached: Option<CachedQcRow> = conn
            .query_row(
                "SELECT id, target_kind, target_id, viewports_json, verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
                 FROM qc_gate_runs
                 WHERE project_id = ?1 AND layout_sha1 = ?2 AND target_id = ?3
                 ORDER BY created_at DESC
                 LIMIT 1",
                params![req.project_id.trim(), cache_sha1, req.target_id.trim()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("cache lookup failed: {e}"))?;

        if let Some((
            id,
            kind_raw,
            target_id,
            vp_raw,
            verdict_raw,
            p0,
            p1,
            ssim_score,
            issues_raw,
            attempt,
            created_at,
        )) = cached
        {
            let verdict =
                QcVerdict::from_db_str(&verdict_raw).unwrap_or(QcVerdict::UnverifiedNoBrowser);
            let stale_unverified =
                verdict == QcVerdict::UnverifiedNoBrowser && live_verification_ready;
            if !stale_unverified {
                let target_kind = QcTargetKind::from_db_str(&kind_raw).unwrap_or(req.target_kind);
                let viewports: Vec<u32> =
                    serde_json::from_str(&vp_raw).unwrap_or_else(|_| norm_viewports.clone());
                let issues: Vec<QcIssue> = serde_json::from_str(&issues_raw).unwrap_or_default();
                let used_fallback = verdict == QcVerdict::UnverifiedNoBrowser;
                let flow_completeness =
                    if target_kind == QcTargetKind::FullFlow || !req.flow_nodes.is_empty() {
                        Some(audit_e2e_flow_completeness(
                            &req.flow_nodes,
                            &req.flow_edges,
                        ))
                    } else {
                        None
                    };
                return Ok(QcRunResponseData {
                    qc_run_id: id,
                    project_id: req.project_id,
                    target_kind,
                    target_id,
                    layout_sha1: cache_sha1,
                    viewports,
                    verdict,
                    p0_count: p0.max(0) as usize,
                    p1_count: p1.max(0) as usize,
                    ssim_score,
                    issues,
                    autofix_attempt: attempt.clamp(0, i64::from(MAX_AUTOFIX_ATTEMPTS)) as u8,
                    max_autofix_attempts: MAX_AUTOFIX_ATTEMPTS,
                    cache_hit: true,
                    used_fallback,
                    fallback_mode: if used_fallback {
                        Some("iframe_bridge_fallback".to_string())
                    } else {
                        None
                    },
                    flow_completeness,
                    rules_evaluated: html_forge::visual_qc::registered_qc_rules().len(),
                    breaker_status: pool.breaker_status(),
                    created_at,
                });
            }
        }
    }

    let eval_input = VisualQcEvalInput {
        target_kind: req.target_kind,
        target_id: req.target_id.clone(),
        html: req.html.clone(),
        constraints_json: req.constraints_json.clone(),
        viewports: norm_viewports,
        fixture_ref: req.fixture_ref.clone(),
        geometry_nodes: req.geometry_nodes.clone(),
        flow_nodes: req.flow_nodes.clone(),
        flow_edges: req.flow_edges.clone(),
        reference_rgba: req.reference_rgba.clone(),
        candidate_rgba: req.candidate_rgba.clone(),
        frame_width: req.frame_width,
        frame_height: req.frame_height,
        ssim_threshold: req.ssim_threshold,
        force_no_browser: req.force_no_browser,
    };

    let pool_clone = Arc::clone(&pool);
    let eval_res = tokio::task::spawn_blocking(move || pool_clone.evaluate(&eval_input))
        .await
        .map_err(|e| format!("visual qc worker panicked: {e}"))?;

    let qc_run_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();
    let viewports_json =
        serde_json::to_string(&eval_res.viewports).unwrap_or_else(|_| "[]".to_string());
    let issues_json = serde_json::to_string(&eval_res.issues).unwrap_or_else(|_| "[]".to_string());

    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(|e| format!("storage connection unavailable: {e}"))?;
    ensure_v26_tables(&conn).map_err(|e| format!("ensure v26 failed: {e}"))?;

    retry_on_busy("insert_qc_gate_run_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO qc_gate_runs (
                id, project_id, target_kind, target_id, layout_sha1, viewports_json,
                verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12)",
            params![
                qc_run_id,
                req.project_id.trim(),
                eval_res.target_kind.as_db_str(),
                eval_res.target_id,
                cache_sha1,
                viewports_json,
                eval_res.verdict.as_db_str(),
                eval_res.p0_count as i64,
                eval_res.p1_count as i64,
                eval_res.ssim_score,
                issues_json,
                created_at,
            ],
        )?;
        tx.commit()?;
        Ok(())
    })
    .map_err(|e| format!("insert qc_gate_runs failed: {e}"))?;

    Ok(QcRunResponseData {
        qc_run_id,
        project_id: req.project_id,
        target_kind: eval_res.target_kind,
        target_id: eval_res.target_id,
        layout_sha1: cache_sha1,
        viewports: eval_res.viewports,
        verdict: eval_res.verdict,
        p0_count: eval_res.p0_count,
        p1_count: eval_res.p1_count,
        ssim_score: eval_res.ssim_score,
        issues: eval_res.issues,
        autofix_attempt: 0,
        max_autofix_attempts: MAX_AUTOFIX_ATTEMPTS,
        cache_hit: false,
        used_fallback: eval_res.used_fallback,
        fallback_mode: eval_res.fallback_mode,
        flow_completeness: eval_res.flow_completeness,
        rules_evaluated: eval_res.rules_evaluated,
        breaker_status: eval_res.breaker_status,
        created_at,
    })
}

async fn handle_post_qc_run(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_editor_role(verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_BODY_BYTES {
        return err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Request body exceeds 1MB limit",
            Some("body"),
            None,
            request_id,
        );
    }
    let req: QcRunRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(err) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid QC run payload: {err}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };
    if let Err(resp) =
        resolve_tenant_for_project(&state, &req.project_id, &headers, verified, &request_id)
    {
        return resp;
    }

    match evaluate_qc_for_project(&state, req).await {
        Ok(data) => ok_resp(data, request_id),
        Err(msg) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "QC_EVALUATION_FAILED",
            &msg,
            None,
            None,
            request_id,
        ),
    }
}

async fn handle_post_qc_auto_fix(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_editor_role(verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_BODY_BYTES {
        return err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Request body exceeds 1MB limit",
            Some("body"),
            None,
            request_id,
        );
    }
    let req: QcAutoFixRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(err) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid QC auto-fix payload: {err}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };
    if let Err(resp) =
        resolve_tenant_for_project(&state, &req.project_id, &headers, verified, &request_id)
    {
        return resp;
    }
    if req.issue_ids.is_empty() {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "issueIds must contain at least one issue identifier",
            Some("issueIds"),
            None,
            request_id,
        );
    }
    if req.attempt_number == 0 || req.attempt_number > u32::from(MAX_AUTOFIX_ATTEMPTS) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AUTOFIX_MAX_ATTEMPTS_EXCEEDED",
            "Auto-Fix is strictly capped at MAX_ATTEMPTS = 3 to prevent runaway CSS rewrites",
            Some("attemptNumber"),
            Some(serde_json::json!({
                "attemptNumber": req.attempt_number,
                "maxAttempts": MAX_AUTOFIX_ATTEMPTS
            })),
            request_id,
        );
    }

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    if let Err(err) = ensure_v26_tables(&conn) {
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &format!("Failed to initialize qc_gate_runs schema: {err}"),
            None,
            None,
            request_id,
        );
    }

    enum AutoFixTxOutcome {
        NotFound,
        MaxAttemptsExceeded {
            persisted_attempts: i64,
        },
        Completed {
            attempt_number: u8,
            fix: DeterministicLintFixResult,
            requires_agent_patch: bool,
        },
    }

    let requested_set: std::collections::HashSet<&str> =
        req.issue_ids.iter().map(String::as_str).collect();
    let tx_outcome = match retry_on_busy("qc_auto_fix_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row_opt: Option<(String, i64)> = tx
            .query_row(
                "SELECT issues_json, autofix_attempt FROM qc_gate_runs WHERE id = ?1 AND project_id = ?2",
                params![req.qc_run_id.trim(), req.project_id.trim()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        let Some((issues_json, persisted_attempts)) = row_opt else {
            return Ok(AutoFixTxOutcome::NotFound);
        };

        if req.mode == QcAutoFixMode::AgentPatch
            && persisted_attempts >= i64::from(MAX_AUTOFIX_ATTEMPTS)
        {
            return Ok(AutoFixTxOutcome::MaxAttemptsExceeded { persisted_attempts });
        }

        let stored_issues: Vec<QcIssue> = serde_json::from_str(&issues_json).unwrap_or_default();
        let selected_issues: Vec<QcIssue> = stored_issues
            .into_iter()
            .filter(|i| requested_set.contains(i.id.as_str()))
            .collect();

        match req.mode {
            QcAutoFixMode::DeterministicLintFix => {
                // Deterministic lint_fix uses 0 LLM tokens and MUST NOT increment autofix_attempt.
                let fix = apply_deterministic_lint_fix(&req.current_html, &selected_issues);
                let requires_agent_patch = !fix.unfixable_issue_ids.is_empty();
                let current_attempts =
                    persisted_attempts.clamp(0, i64::from(MAX_AUTOFIX_ATTEMPTS)) as u8;
                Ok(AutoFixTxOutcome::Completed {
                    attempt_number: current_attempts,
                    fix,
                    requires_agent_patch,
                })
            }
            QcAutoFixMode::AgentPatch => {
                let fix = apply_agent_structural_patch(&req.current_html, &selected_issues);
                let produced_patch =
                    fix.patched_html != fix.previous_html && !fix.diff_summary.is_empty();
                if produced_patch {
                    // Server-side monotonic increment (ignores client attemptNumber=1 replay).
                    let next_attempt =
                        (persisted_attempts + 1).clamp(1, i64::from(MAX_AUTOFIX_ATTEMPTS)) as u8;
                    tx.execute(
                        "UPDATE qc_gate_runs SET autofix_attempt = ?1 WHERE id = ?2 AND project_id = ?3",
                        params![
                            i64::from(next_attempt),
                            req.qc_run_id.trim(),
                            req.project_id.trim()
                        ],
                    )?;
                    tx.commit()?;
                    Ok(AutoFixTxOutcome::Completed {
                        attempt_number: next_attempt,
                        fix,
                        requires_agent_patch: false,
                    })
                } else {
                    // Zero patch operations could be produced -> do NOT increment autofix_attempt.
                    let current_attempts =
                        persisted_attempts.clamp(0, i64::from(MAX_AUTOFIX_ATTEMPTS)) as u8;
                    Ok(AutoFixTxOutcome::Completed {
                        attempt_number: current_attempts,
                        fix,
                        requires_agent_patch: false,
                    })
                }
            }
        }
    }) {
        Ok(outcome) => outcome,
        Err(HarnessError::SqliteBusy { .. }) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Failed to acquire SQLite write transaction after retry",
                None,
                None,
                request_id,
            );
        }
        Err(_) => {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to persist autofix attempt counter",
                None,
                None,
                request_id,
            );
        }
    };

    match tx_outcome {
        AutoFixTxOutcome::NotFound => err_resp(
            StatusCode::NOT_FOUND,
            "QC_RUN_NOT_FOUND",
            "Referenced QC gate run does not exist for this project",
            Some("qcRunId"),
            None,
            request_id,
        ),
        AutoFixTxOutcome::MaxAttemptsExceeded { persisted_attempts } => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AUTOFIX_MAX_ATTEMPTS_EXCEEDED",
            "Maximum 3 Agent Auto-Fix attempts already consumed for this QC run; restore snapshot or edit manually",
            Some("attemptNumber"),
            Some(serde_json::json!({
                "persistedAttempts": persisted_attempts,
                "maxAttempts": MAX_AUTOFIX_ATTEMPTS
            })),
            request_id,
        ),
        AutoFixTxOutcome::Completed {
            attempt_number,
            fix,
            requires_agent_patch,
        } => ok_resp(
            QcAutoFixResponseData {
                qc_run_id: req.qc_run_id,
                project_id: req.project_id,
                mode: req.mode,
                attempt_number,
                max_autofix_attempts: MAX_AUTOFIX_ATTEMPTS,
                remaining_attempts: MAX_AUTOFIX_ATTEMPTS.saturating_sub(attempt_number),
                patched_html: fix.patched_html,
                previous_html: fix.previous_html,
                fixed_issue_ids: fix.fixed_issue_ids,
                unfixable_issue_ids: fix.unfixable_issue_ids,
                elapsed_ms: fix.elapsed_ms,
                llm_tokens_used: fix.llm_tokens_used,
                diff_summary: fix.diff_summary,
                rollback_supported: true,
                requires_agent_patch,
            },
            request_id,
        ),
    }
}

pub fn visual_qc_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/qc/run", post(handle_post_qc_run))
        .route("/api/v1/qc/auto-fix", post(handle_post_qc_auto_fix))
}
