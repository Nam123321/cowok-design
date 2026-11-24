// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::collections::BTreeMap;

use axum::{
    body::Bytes,
    extract::{Extension, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use domain::HarnessError;
use html_forge::{
    compute_sha256_hex, evaluate_oklch_contrast_guard, is_valid_ff_id, patch_html_element,
    reconcile_ff_ids, sanitize_html_with_profile, HtmlForgeError, HtmlPatchOperation,
    OklchContrastReport, SanitizeProfile, DEFAULT_RECONCILE_MIN_CONFIDENCE, WCAG_AA_MIN_CONTRAST,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::locks::is_valid_ssot_portal;
use spec_registry::skill_router::{CachePreservingSkillRouter, SkillRouteCommand};
use uuid::Uuid;

use super::execution_reaper::{discard_shadow_dir, terminate_pgid_checked};
use super::legacy_specs::verify_anti_idor_with_caller;
use super::orchestrator::{map_actor_error_to_response, shared_write_actor_for_pool};
use super::DaemonAppState;
use crate::daemon_auth::{LegacyAuthKind, VerifiedLegacyIngress};

const MAX_BODY_BYTES: usize = 512 * 1024;
const MAX_GOAL_PROMPT_LEN: usize = 4_000;
const MAX_SCREEN_ID_LEN: usize = 128;

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
#[serde(rename_all = "camelCase")]
pub struct LiveDirectionOption {
    pub id: String,
    pub label: String,
    pub summary: String,
    pub accent_hex: String,
    pub surface_hex: String,
    pub skill_slug: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HttpClarifyRequest {
    project_id: String,
    goal_prompt: String,
    locale: String,
    #[serde(default)]
    active_direction_id: Option<String>,
    #[serde(default)]
    preempt_pgid: Option<i64>,
    #[serde(default)]
    expected_generation_epoch: Option<i64>,
    #[serde(default = "default_portal_id")]
    portal_id: String,
}

fn default_portal_id() -> String {
    "SalesApp".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarifySessionData {
    pub session_id: String,
    pub project_id: String,
    pub status: String,
    pub goal_prompt: String,
    pub locale: String,
    pub active_direction_id: String,
    pub directions: Vec<LiveDirectionOption>,
    pub generation_epoch: i64,
    pub preempted_workers: usize,
    pub routed_skills: Vec<String>,
    pub prefix_sha256: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HttpSurgicalPatchRequest {
    project_id: String,
    screen_id: String,
    base_artifact_sha256: String,
    ff_id: String,
    op_kind: String,
    payload_json: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    expected_ast_revision: Option<i64>,
    #[serde(default)]
    base_html: Option<String>,
    #[serde(default)]
    next_html: Option<String>,
    #[serde(default = "default_portal_id")]
    portal_id: String,
    #[serde(default)]
    confirm_locked_rule_override: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedRuleChangeReceipt {
    pub rule_code: String,
    pub lock_owner_id: String,
    pub fencing_token: i64,
    pub affected_ff_id: String,
    pub summary: String,
    pub requires_review: bool,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurgicalPatchResultData {
    pub patch_id: String,
    pub project_id: String,
    pub screen_id: String,
    pub ast_revision: i64,
    pub base_artifact_sha256: String,
    pub result_artifact_sha256: String,
    pub ff_id: String,
    pub op_kind: String,
    pub apply_tier: String,
    pub reconciled_ff_ids: usize,
    pub html_snapshot: String,
    pub oklch_guard: Option<OklchContrastReport>,
    pub locked_rule_receipt: Option<LockedRuleChangeReceipt>,
    pub undo_available: bool,
    pub execution_time_micros: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenPatchRecord {
    pub patch_id: String,
    pub ast_revision: i64,
    pub base_artifact_sha256: String,
    pub result_artifact_sha256: String,
    pub ff_id: String,
    pub op_kind: String,
    pub apply_tier: String,
    pub reconciled_ff_ids: usize,
    pub payload_json: serde_json::Value,
    pub locked_rule_receipt: Option<LockedRuleChangeReceipt>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenPatchHistoryData {
    pub project_id: String,
    pub screen_id: String,
    pub ast_revision: i64,
    pub current_artifact_sha256: String,
    pub html_snapshot: String,
    pub patches: Vec<ScreenPatchRecord>,
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

fn is_write_role(role: &str) -> bool {
    matches!(
        role.trim().to_ascii_lowercase().as_str(),
        "editor" | "admin" | "owner"
    )
}

fn require_caller<'a>(
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

fn require_write_role(verified: &VerifiedLegacyIngress, request_id: &str) -> Result<(), Response> {
    if !is_write_role(&verified.principal.role) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Write operation requires editor or admin role",
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
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be 1..=128 characters",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        ));
    }

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

    match verify_anti_idor_with_caller(&state.registry, project_uuid, headers, Some(verified)) {
        Ok(_) => {
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
                    params![trimmed],
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
            Ok(tenant_id)
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
        Err(_) => Err(error_response(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Project access denied",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        )),
    }
}

fn parse_query_params(raw_query: Option<&str>) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Some(q) = raw_query else {
        return map;
    };
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let dk = percent_decode_simple(k);
            let dv = percent_decode_simple(v);
            map.insert(dk, dv);
        }
    }
    map
}

fn percent_decode_simple(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(val);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

fn build_live_directions(locale: &str) -> Vec<LiveDirectionOption> {
    if locale == "vi" {
        vec![
            LiveDirectionOption {
                id: "dir-editorial-sketch".to_string(),
                label: "Phác thảo gọn nhẹ".to_string(),
                summary:
                    "Bắt đầu bằng bản vẽ nét bút đơn giản để tập trung vào bố cục và nội dung trước khi lên màu."
                        .to_string(),
                accent_hex: "#2563EB".to_string(),
                surface_hex: "#FAF6EC".to_string(),
                skill_slug: "sketch-monochrome".to_string(),
            },
            LiveDirectionOption {
                id: "dir-polish-workbench".to_string(),
                label: "Hoàn thiện sắc nét".to_string(),
                summary:
                    "Hiển thị giao diện đầy đủ với thẻ số liệu rõ ràng, sẵn sàng trình bày và bàn giao."
                        .to_string(),
                accent_hex: "#0F766E".to_string(),
                surface_hex: "#F8FAFC".to_string(),
                skill_slug: "impeccable-polish".to_string(),
            },
        ]
    } else {
        vec![
            LiveDirectionOption {
                id: "dir-editorial-sketch".to_string(),
                label: "Clean Sketch First".to_string(),
                summary:
                    "Start with a simple pencil-style draft to focus on layout and content before adding brand colors."
                        .to_string(),
                accent_hex: "#2563EB".to_string(),
                surface_hex: "#FAF6EC".to_string(),
                skill_slug: "sketch-monochrome".to_string(),
            },
            LiveDirectionOption {
                id: "dir-polish-workbench".to_string(),
                label: "Polished Layout".to_string(),
                summary:
                    "Preview a crisp, ready-to-present layout with clear metrics and balanced colors."
                        .to_string(),
                accent_hex: "#0F766E".to_string(),
                surface_hex: "#F8FAFC".to_string(),
                skill_slug: "impeccable-polish".to_string(),
            },
        ]
    }
}

fn default_starter_screen_html(screen_id: &str, ff_id: &str) -> String {
    let safe_screen = html_forge::escape_html_attr(screen_id);
    let builtin_ids = [
        "stage-root",
        "sec-header",
        "hero-card",
        "hero-title",
        "hero-subtitle",
        "ff-slide-eyebrow",
        "ff-slide-headline",
        "ff-slide-subtitle",
        "sec-kpi",
        "ff-kpi-1-label",
        "ff-kpi-1-val",
        "ff-kpi-2-label",
        "ff-kpi-2-val",
        "ff-kpi-3-label",
        "ff-kpi-3-val",
        "sec-biometric",
        "ff-action-title",
        "ff-action-hint",
        "ff-action-cta",
    ];
    let extra_slot = if builtin_ids.contains(&ff_id) {
        String::new()
    } else {
        let safe_ff = html_forge::escape_html_attr(ff_id);
        format!("<div data-ff-id=\"{safe_ff}\">Workspace Element</div>")
    };
    format!(
        "<!DOCTYPE html><html><head><style>body{{margin:0;padding:16px;font-family:sans-serif;display:flex;flex-direction:column;gap:12px}}.card{{padding:12px;border-radius:8px;border:1px solid #d4d1c7}}.kpi-grid{{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}}</style></head><body><main data-ff-id=\"stage-root\" data-screen-id=\"{safe_screen}\"><section class=\"card\" data-ff-id=\"sec-header\"><span data-ff-id=\"ff-slide-eyebrow\">WORKSPACE</span><h2 data-ff-id=\"ff-slide-headline\">Workspace Screen</h2><p data-ff-id=\"ff-slide-subtitle\">Click any element to refine wording or spacing.</p></section><section class=\"card\" data-ff-id=\"hero-card\"><h1 data-ff-id=\"hero-title\">Workspace Screen</h1><p data-ff-id=\"hero-subtitle\">Click any element to refine wording or spacing.</p></section><section class=\"card kpi-grid\" data-ff-id=\"sec-kpi\"><div><div data-ff-id=\"ff-kpi-1-label\">Metric 1</div><div data-ff-id=\"ff-kpi-1-val\">$0</div></div><div><div data-ff-id=\"ff-kpi-2-label\">Metric 2</div><div data-ff-id=\"ff-kpi-2-val\">Active</div></div><div><div data-ff-id=\"ff-kpi-3-label\">Metric 3</div><div data-ff-id=\"ff-kpi-3-val\">12ms</div></div></section><section class=\"card\" data-ff-id=\"sec-biometric\"><div data-ff-id=\"ff-action-title\">Primary Action</div><div data-ff-id=\"ff-action-hint\">Auto-synced live HTML preview.</div><button type=\"button\" data-ff-id=\"ff-action-cta\">Continue</button></section>{extra_slot}</main></body></html>"
    )
}

fn is_valid_sha256_hex(raw: &str) -> bool {
    raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit())
}

fn extract_string_map(
    map: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(serde_json::Value::Object(obj)) = map.get(key) {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                out.insert(k.clone(), s.to_string());
            }
        }
    }
    out
}

fn resolve_steering_chip_styles(chip_id: &str) -> BTreeMap<String, String> {
    let mut styles = BTreeMap::new();
    match chip_id {
        "more-whitespace" => {
            styles.insert("padding".to_string(), "28px".to_string());
            styles.insert("gap".to_string(), "20px".to_string());
        }
        "denser-layout" => {
            styles.insert("padding".to_string(), "12px".to_string());
            styles.insert("gap".to_string(), "8px".to_string());
        }
        "sharper-corners" => {
            styles.insert("border-radius".to_string(), "4px".to_string());
        }
        "softer-corners" => {
            styles.insert("border-radius".to_string(), "16px".to_string());
        }
        "stronger-type" => {
            styles.insert("font-weight".to_string(), "700".to_string());
            styles.insert("letter-spacing".to_string(), "-0.02em".to_string());
        }
        _ => {
            styles.insert("line-height".to_string(), "1.5".to_string());
        }
    }
    styles
}

struct LockedSlotCheckInput<'a> {
    tenant_id: &'a str,
    project_id: &'a str,
    portal_id: &'a str,
    ff_id: &'a str,
    op_kind: &'a str,
    now_ms: i64,
    confirm_override: bool,
}

fn check_active_locked_slot(
    tx: &rusqlite::Transaction<'_>,
    input: &LockedSlotCheckInput<'_>,
) -> Result<Option<LockedRuleChangeReceipt>, String> {
    let lock_row: Option<(String, i64, String)> = tx
        .query_row(
            "SELECT lock_owner_id, fencing_token, locked_slots_json
             FROM app_shell_locks
             WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 AND expires_at > ?4",
            params![
                input.tenant_id,
                input.project_id,
                input.portal_id,
                input.now_ms
            ],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("Failed to verify active AppShell lock: {e}"))?;

    if let Some((owner_id, fencing_token, locked_slots_json)) = lock_row {
        let slots: Vec<String> = serde_json::from_str(&locked_slots_json)
            .map_err(|e| format!("Corrupted AppShell locked_slots_json (fail-closed): {e}"))?;
        let touches_locked = slots.iter().any(|s| {
            s == input.ff_id
                || s == "*"
                || ((s == "app-shell" || s == "shell")
                    && (input.ff_id.starts_with("shell-") || input.ff_id.starts_with("nav-")))
        });
        if touches_locked || input.op_kind == "LOCKED_RULE_CHANGE" {
            return Ok(Some(LockedRuleChangeReceipt {
                rule_code: "APPSHELL_LOCK_BOUNDARY".to_string(),
                lock_owner_id: owner_id,
                fencing_token,
                affected_ff_id: input.ff_id.to_string(),
                summary: format!(
                    "Element '{}' is protected by an active AppShell lock on portal '{}'.",
                    input.ff_id, input.portal_id
                ),
                requires_review: !input.confirm_override,
                applied: input.confirm_override,
            }));
        }
    }

    if input.op_kind == "LOCKED_RULE_CHANGE" {
        return Ok(Some(LockedRuleChangeReceipt {
            rule_code: "DESIGN_SYSTEM_RULE_CHANGE".to_string(),
            lock_owner_id: "constraint-profile".to_string(),
            fencing_token: 0,
            affected_ff_id: input.ff_id.to_string(),
            summary: format!("Structural rule change requested on '{}'.", input.ff_id),
            requires_review: !input.confirm_override,
            applied: input.confirm_override,
        }));
    }
    Ok(None)
}

enum ClarifyWriteOutcome {
    Committed {
        session_id: String,
        status: String,
        generation_epoch: i64,
    },
    StaleEpoch {
        expected: i64,
        current: i64,
        uninitialized: bool,
    },
}

enum PatchOpError {
    InvalidRequest {
        message: String,
        field: &'static str,
    },
    HtmlForge(HtmlForgeError),
}

enum PatchWriteOutcome {
    Committed(Box<SurgicalPatchResultData>),
    HtmlError(HtmlForgeError),
    ApiError {
        status: StatusCode,
        code: &'static str,
        message: String,
        field: Option<&'static str>,
        details: Option<serde_json::Value>,
    },
}

struct ClarifySessionRow {
    session_id: String,
    status: String,
    goal_prompt: String,
    locale: String,
    active_direction_id: String,
    directions_json: String,
    generation_epoch: i64,
    routed_slugs_json: String,
    updated_at: i64,
}

/// `GET /api/v1/sdi/clarify?projectId=...`
pub async fn handle_get_sdi_clarify(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let qp = parse_query_params(raw_query.as_deref());
    let project_id = qp.get("projectId").cloned().unwrap_or_default();
    let tenant_id =
        match resolve_tenant_for_project(&state, &project_id, &headers, verified, &request_id) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };

    let row: Option<ClarifySessionRow> = conn
        .query_row(
            "SELECT id, status, goal_prompt, locale, active_direction_id, directions_json,
                    generation_epoch, routed_skill_slugs_json, updated_at
             FROM sdi_clarification_sessions
             WHERE tenant_id = ?1 AND project_id = ?2",
            params![tenant_id, project_id],
            |r| {
                Ok(ClarifySessionRow {
                    session_id: r.get(0)?,
                    status: r.get(1)?,
                    goal_prompt: r.get(2)?,
                    locale: r.get(3)?,
                    active_direction_id: r.get(4)?,
                    directions_json: r.get(5)?,
                    generation_epoch: r.get(6)?,
                    routed_slugs_json: r.get(7)?,
                    updated_at: r.get(8)?,
                })
            },
        )
        .optional()
        .unwrap_or(None);

    let Some(row) = row else {
        return ok_response(serde_json::json!({ "session": null }), request_id);
    };

    let directions: Vec<LiveDirectionOption> = serde_json::from_str(&row.directions_json)
        .unwrap_or_else(|_| build_live_directions(&row.locale));
    let routed_skills: Vec<String> =
        serde_json::from_str(&row.routed_slugs_json).unwrap_or_default();

    ok_response(
        serde_json::json!({
            "session": ClarifySessionData {
                session_id: row.session_id,
                project_id,
                status: row.status,
                goal_prompt: row.goal_prompt,
                locale: row.locale,
                active_direction_id: row.active_direction_id,
                directions,
                generation_epoch: row.generation_epoch,
                preempted_workers: 0,
                routed_skills,
                prefix_sha256: String::new(),
                updated_at: row.updated_at,
            }
        }),
        request_id,
    )
}

/// `POST /api/v1/sdi/clarify`
pub async fn handle_sdi_clarify(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Clarify request body exceeds maximum size",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: HttpClarifyRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid clarify payload: {e}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let goal_prompt = req.goal_prompt.trim();
    if goal_prompt.is_empty() || goal_prompt.len() > MAX_GOAL_PROMPT_LEN {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "goalPrompt must be 1..=4000 characters",
            Some("goalPrompt".to_string()),
            None,
            request_id,
        );
    }
    let locale = req.locale.trim().to_ascii_lowercase();
    if locale != "vi" && locale != "en" {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "locale must be 'vi' or 'en'",
            Some("locale".to_string()),
            None,
            request_id,
        );
    }
    if !is_valid_ssot_portal(&req.portal_id) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Unsupported portalId",
            Some("portalId".to_string()),
            None,
            request_id,
        );
    }

    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.project_id,
        &headers,
        verified,
        &request_id,
    ) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let directions = build_live_directions(&locale);
    let active_direction_id = match req.active_direction_id.as_deref().map(str::trim) {
        Some(dir_id) if directions.iter().any(|d| d.id == dir_id) => dir_id.to_string(),
        Some(invalid) if !invalid.is_empty() => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Unknown activeDirectionId '{invalid}'"),
                Some("activeDirectionId".to_string()),
                None,
                request_id,
            );
        }
        _ => directions[0].id.clone(),
    };

    // Route craft skills via CachePreservingSkillRouter (Story 02.4 integration)
    let routed = CachePreservingSkillRouter::route(&SkillRouteCommand {
        project_id: req.project_id.clone(),
        intent_summary: goal_prompt.to_string(),
        portal_id: req.portal_id.clone(),
        requested_slugs: Some(vec![
            "sketch-monochrome".to_string(),
            "impeccable-polish".to_string(),
        ]),
        max_spine_tokens: 250,
    });
    let (routed_skills, prefix_sha256) = match routed {
        Ok(outcome) => (outcome.selected_slugs, outcome.prefix_sha256),
        Err(_) => (
            vec![
                "sketch-monochrome".to_string(),
                "impeccable-polish".to_string(),
            ],
            String::new(),
        ),
    };

    let validated_preempt_pgid: Option<i32> = match req.preempt_pgid {
        Some(raw_pgid) if raw_pgid <= 0 || raw_pgid > i64::from(i32::MAX) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "preemptPgid must be a positive 32-bit process group id",
                Some("preemptPgid".to_string()),
                None,
                request_id,
            );
        }
        Some(raw_pgid) => Some(raw_pgid as i32),
        None => None,
    };

    let now_ms = Utc::now().timestamp_millis();
    let directions_json = serde_json::to_string(&directions).unwrap_or_else(|_| "[]".to_string());
    let skills_json = serde_json::to_string(&routed_skills).unwrap_or_else(|_| "[]".to_string());

    let pool_state = state.clone();
    let actor_req_id = request_id.clone();
    let actor = match tokio::task::spawn_blocking(move || {
        shared_write_actor_for_pool(pool_state.registry.pool())
    })
    .await
    {
        Ok(Ok(a)) => a,
        Ok(Err(err)) => return map_actor_error_to_response(err, actor_req_id),
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Write actor lookup worker failed unexpectedly",
                None,
                None,
                actor_req_id,
            );
        }
    };

    let tid_w = tenant_id.clone();
    let pid_w = req.project_id.clone();
    let goal_w = goal_prompt.to_string();
    let locale_w = locale.clone();
    let active_dir_w = active_direction_id.clone();
    let expected_epoch = req.expected_generation_epoch;
    let preempt_pgid_raw = req.preempt_pgid;
    let has_preempt = validated_preempt_pgid.is_some();

    let receipt = match actor.try_enqueue_immediate(move |tx| {
        let existing: Option<(String, String, i64)> = tx
            .query_row(
                "SELECT id, active_direction_id, generation_epoch
                 FROM sdi_clarification_sessions
                 WHERE tenant_id = ?1 AND project_id = ?2",
                params![tid_w, pid_w],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .unwrap_or(None);

        match existing {
            Some((existing_id, prev_dir, prev_epoch)) => {
                if let Some(exp_epoch) = expected_epoch {
                    if exp_epoch != prev_epoch {
                        return Ok(ClarifyWriteOutcome::StaleEpoch {
                            expected: exp_epoch,
                            current: prev_epoch,
                            uninitialized: false,
                        });
                    }
                }
                let switched = prev_dir != active_dir_w || has_preempt;
                let next_epoch = prev_epoch + 1;
                let next_status = if switched { "preempted" } else { "clarifying" };
                tx.execute(
                    "UPDATE sdi_clarification_sessions
                     SET status = ?1,
                         goal_prompt = ?2,
                         locale = ?3,
                         active_direction_id = ?4,
                         directions_json = ?5,
                         generation_epoch = ?6,
                         preempt_pgid = ?7,
                         routed_skill_slugs_json = ?8,
                         updated_at = ?9
                     WHERE id = ?10 AND tenant_id = ?11",
                    params![
                        next_status,
                        goal_w,
                        locale_w,
                        active_dir_w,
                        directions_json,
                        next_epoch,
                        preempt_pgid_raw,
                        skills_json,
                        now_ms,
                        existing_id,
                        tid_w,
                    ],
                )?;
                Ok(ClarifyWriteOutcome::Committed {
                    session_id: existing_id,
                    status: next_status.to_string(),
                    generation_epoch: next_epoch,
                })
            }
            None => {
                if let Some(exp_epoch) = expected_epoch {
                    if exp_epoch != 0 {
                        return Ok(ClarifyWriteOutcome::StaleEpoch {
                            expected: exp_epoch,
                            current: 0,
                            uninitialized: true,
                        });
                    }
                }
                let new_id = Uuid::new_v4().to_string();
                let next_status = if has_preempt {
                    "preempted"
                } else {
                    "clarifying"
                };
                tx.execute(
                    "INSERT INTO sdi_clarification_sessions (
                        id, tenant_id, project_id, status, goal_prompt, locale,
                        active_direction_id, directions_json, generation_epoch,
                        preempt_pgid, routed_skill_slugs_json, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?11)",
                    params![
                        new_id,
                        tid_w,
                        pid_w,
                        next_status,
                        goal_w,
                        locale_w,
                        active_dir_w,
                        directions_json,
                        preempt_pgid_raw,
                        skills_json,
                        now_ms,
                    ],
                )?;
                Ok(ClarifyWriteOutcome::Committed {
                    session_id: new_id,
                    status: next_status.to_string(),
                    generation_epoch: 1,
                })
            }
        }
    }) {
        Ok(r) => r,
        Err(err) => return map_actor_error_to_response(err, request_id),
    };

    let (session_id, status, generation_epoch) =
        match tokio::task::spawn_blocking(move || receipt.wait()).await {
            Ok(Ok(ClarifyWriteOutcome::Committed {
                session_id,
                status,
                generation_epoch,
            })) => (session_id, status, generation_epoch),
            Ok(Ok(ClarifyWriteOutcome::StaleEpoch {
                expected,
                current,
                uninitialized,
            })) => {
                let message = if uninitialized {
                    "Expected generation epoch does not match uninitialized session (0)"
                } else {
                    "Generation epoch is stale and was preempted by a newer direction"
                };
                return error_response(
                    StatusCode::CONFLICT,
                    "STALE_GENERATION_EPOCH",
                    message,
                    Some("expectedGenerationEpoch".to_string()),
                    Some(serde_json::json!({
                        "expectedGenerationEpoch": expected,
                        "currentGenerationEpoch": current,
                    })),
                    request_id,
                );
            }
            Ok(Err(err)) => return map_actor_error_to_response(err, request_id),
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Clarification session write actor worker failed unexpectedly",
                    None,
                    None,
                    request_id,
                );
            }
        };

    // Zero-Trust -PGID preemption runs ONLY AFTER epoch OCC check & commit succeed,
    // inside spawn_blocking so terminate_pgid_checked never blocks Tokio worker threads.
    let preempted_workers = if let Some(target_pgid) = validated_preempt_pgid {
        let entries = state
            .active_workers
            .take_by_project_and_pgid(&req.project_id, target_pgid);
        let count = entries.len();
        if !entries.is_empty() {
            let _ = tokio::task::spawn_blocking(move || {
                for entry in entries {
                    let _ = terminate_pgid_checked(entry.pid, entry.pgid, &entry.start_time);
                    let _ = discard_shadow_dir(&entry.shadow_dir);
                }
            })
            .await;
        }
        count
    } else {
        0
    };

    ok_response(
        ClarifySessionData {
            session_id,
            project_id: req.project_id,
            status,
            goal_prompt: goal_prompt.to_string(),
            locale,
            active_direction_id,
            directions,
            generation_epoch,
            preempted_workers,
            routed_skills,
            prefix_sha256,
            updated_at: now_ms,
        },
        request_id,
    )
}

/// `GET /api/v1/patch?projectId=...&screenId=...`
pub async fn handle_get_sketch_patch_history(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let qp = parse_query_params(raw_query.as_deref());
    let project_id = qp.get("projectId").cloned().unwrap_or_default();
    let screen_id = qp.get("screenId").cloned().unwrap_or_default();
    if screen_id.trim().is_empty() || screen_id.len() > MAX_SCREEN_ID_LEN {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "screenId query parameter is required (1..=128 chars)",
            Some("screenId".to_string()),
            None,
            request_id,
        );
    }

    let tenant_id =
        match resolve_tenant_for_project(&state, &project_id, &headers, verified, &request_id) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT id, ast_revision, base_artifact_sha, result_artifact_sha, ff_id, op_kind,
                apply_tier, reconciled_ff_ids, payload_json, html_snapshot, locked_rule_json, created_at
         FROM screen_artifact_patches
         WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
         ORDER BY ast_revision DESC
         LIMIT 200",
    ) {
        Ok(s) => s,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                &format!("Failed to query screen patches: {e}"),
                None,
                None,
                request_id,
            );
        }
    };

    let rows = match stmt.query_map(params![tenant_id, project_id, screen_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, String>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, i64>(11)?,
        ))
    }) {
        Ok(iter) => iter.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                &format!("Failed to read screen patches: {e}"),
                None,
                None,
                request_id,
            );
        }
    };

    let (ast_revision, current_artifact_sha256, html_snapshot) = rows
        .first()
        .map(|top| (top.1, top.3.clone(), top.9.clone()))
        .unwrap_or_else(|| (0, compute_sha256_hex(b""), String::new()));

    let patches = rows
        .into_iter()
        .map(|row| ScreenPatchRecord {
            patch_id: row.0,
            ast_revision: row.1,
            base_artifact_sha256: row.2,
            result_artifact_sha256: row.3,
            ff_id: row.4,
            op_kind: row.5,
            apply_tier: row.6,
            reconciled_ff_ids: row.7.max(0) as usize,
            payload_json: serde_json::from_str(&row.8).unwrap_or(serde_json::json!({})),
            locked_rule_receipt: row
                .10
                .as_deref()
                .and_then(|s| serde_json::from_str::<LockedRuleChangeReceipt>(s).ok()),
            created_at: row.11,
        })
        .collect();

    ok_response(
        ScreenPatchHistoryData {
            project_id,
            screen_id,
            ast_revision,
            current_artifact_sha256,
            html_snapshot,
            patches,
        },
        request_id,
    )
}

/// `POST /api/v1/patch`
pub async fn handle_apply_sketch_patch(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Patch request body exceeds 512KB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: HttpSurgicalPatchRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid patch payload: {e}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let screen_id = req.screen_id.trim();
    if screen_id.is_empty() || screen_id.len() > MAX_SCREEN_ID_LEN {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "screenId must be 1..=128 characters",
            Some("screenId".to_string()),
            None,
            request_id,
        );
    }
    if !is_valid_sha256_hex(&req.base_artifact_sha256) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "baseArtifactSha256 must be a 64-character hex SHA-256",
            Some("baseArtifactSha256".to_string()),
            None,
            request_id,
        );
    }
    if !is_valid_ff_id(&req.ff_id) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Invalid data-ff-id format",
            Some("ffId".to_string()),
            None,
            request_id,
        );
    }
    if !matches!(
        req.op_kind.as_str(),
        "TEXT_EDIT" | "TOKEN_SWAP" | "STEERING_CHIP" | "LOCKED_RULE_CHANGE" | "UNDO"
    ) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Unsupported opKind",
            Some("opKind".to_string()),
            None,
            request_id,
        );
    }
    if !is_valid_ssot_portal(&req.portal_id) {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Unsupported portalId",
            Some("portalId".to_string()),
            None,
            request_id,
        );
    }

    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.project_id,
        &headers,
        verified,
        &request_id,
    ) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let pool_state = state.clone();
    let actor_req_id = request_id.clone();
    let actor = match tokio::task::spawn_blocking(move || {
        shared_write_actor_for_pool(pool_state.registry.pool())
    })
    .await
    {
        Ok(Ok(a)) => a,
        Ok(Err(err)) => return map_actor_error_to_response(err, actor_req_id),
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Write actor lookup worker failed unexpectedly",
                None,
                None,
                actor_req_id,
            );
        }
    };

    let screen_id_owned = screen_id.to_string();
    let receipt = match actor.try_enqueue_immediate(move |tx| {
        let screen_id = screen_id_owned.as_str();
        let latest: Option<(i64, String, String)> = tx
            .query_row(
                "SELECT ast_revision, result_artifact_sha, html_snapshot
                 FROM screen_artifact_patches
                 WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
                 ORDER BY ast_revision DESC
                 LIMIT 1",
                params![tenant_id, req.project_id, screen_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .unwrap_or(None);

        let current_rev = latest.as_ref().map(|r| r.0).unwrap_or(0);
        if let Some((rev, ref latest_sha, _)) = latest {
            let sha_mismatch = latest_sha != &req.base_artifact_sha256;
            let rev_mismatch = req.expected_ast_revision.is_some_and(|exp| exp != rev);
            if sha_mismatch || rev_mismatch {
                return Ok(PatchWriteOutcome::ApiError {
                    status: StatusCode::CONFLICT,
                    code: "STALE_MAPPING",
                    message:
                        "Concurrent screen edit detected; baseArtifactSha256 or astRevision is stale"
                            .to_string(),
                    field: Some("baseArtifactSha256"),
                    details: Some(serde_json::json!({
                        "currentAstRevision": rev,
                        "currentArtifactSha256": latest_sha,
                        "expectedAstRevision": req.expected_ast_revision,
                    })),
                });
            }
        } else if req.expected_ast_revision.is_some_and(|exp| exp != 0) {
            return Ok(PatchWriteOutcome::ApiError {
                status: StatusCode::CONFLICT,
                code: "STALE_MAPPING",
                message: "Expected astRevision does not match uninitialized screen revision (0)"
                    .to_string(),
                field: Some("expectedAstRevision"),
                details: Some(serde_json::json!({
                    "currentAstRevision": 0,
                    "expectedAstRevision": req.expected_ast_revision,
                })),
            });
        }

        let raw_working_html = match latest.as_ref().filter(|r| !r.2.trim().is_empty()) {
            Some((_, _, snap)) => snap.clone(),
            None => req
                .base_html
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| default_starter_screen_html(screen_id, &req.ff_id)),
        };

        let (mut working_html, _) =
            match sanitize_html_with_profile(&raw_working_html, SanitizeProfile::AppPreview) {
                Ok(pair) => pair,
                Err(err) => return Ok(PatchWriteOutcome::HtmlError(err)),
            };

        let now_ms = Utc::now().timestamp_millis();
        let locked_rule_receipt = match check_active_locked_slot(
            tx,
            &LockedSlotCheckInput {
                tenant_id: &tenant_id,
                project_id: &req.project_id,
                portal_id: &req.portal_id,
                ff_id: &req.ff_id,
                op_kind: &req.op_kind,
                now_ms,
                confirm_override: req.confirm_locked_rule_override,
            },
        ) {
            Ok(r) => r,
            Err(msg) => {
                return Ok(PatchWriteOutcome::ApiError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    code: "STORAGE_ERROR",
                    message: msg,
                    field: None,
                    details: None,
                });
            }
        };
        let apply_tier = if locked_rule_receipt.is_some() {
            "LOCKED_RULE_RECEIPT"
        } else {
            "INSTANT_UNDO"
        };

        let mut oklch_guard: Option<OklchContrastReport> = None;
        let should_apply_mutation = locked_rule_receipt
            .as_ref()
            .map(|r| r.applied)
            .unwrap_or(true);

        let mut reconciled_ff_ids = 0usize;
        if should_apply_mutation {
            if let Some(ref candidate_next_html) = req.next_html {
                if !candidate_next_html.trim().is_empty() {
                    let (sanitized_next, _) = match sanitize_html_with_profile(
                        candidate_next_html,
                        SanitizeProfile::AppPreview,
                    ) {
                        Ok(pair) => pair,
                        Err(err) => return Ok(PatchWriteOutcome::HtmlError(err)),
                    };
                    match reconcile_ff_ids(
                        &working_html,
                        &sanitized_next,
                        DEFAULT_RECONCILE_MIN_CONFIDENCE,
                    ) {
                        Ok(report) => {
                            let (verified_html, _) = match sanitize_html_with_profile(
                                &report.reconciled_html,
                                SanitizeProfile::AppPreview,
                            ) {
                                Ok(pair) => pair,
                                Err(err) => return Ok(PatchWriteOutcome::HtmlError(err)),
                            };
                            working_html = verified_html;
                            reconciled_ff_ids = report.total_ff_ids;
                        }
                        Err(err) => return Ok(PatchWriteOutcome::HtmlError(err)),
                    }
                }
            }
        }

        let mut persisted_payload = req.payload_json.clone();
        let (result_html, result_sha, exec_micros) = if !should_apply_mutation {
            let sha = compute_sha256_hex(working_html.as_bytes());
            (working_html, sha, 0u64)
        } else if req.op_kind == "UNDO" {
            let restored = resolve_monotonic_undo_snapshot(
                tx,
                &tenant_id,
                &req.project_id,
                screen_id,
                current_rev,
                &working_html,
                &mut persisted_payload,
            );
            let sha = compute_sha256_hex(restored.as_bytes());
            (restored, sha, 0u64)
        } else {
            let patch_op = match build_patch_operation(&req, &mut oklch_guard) {
                Ok(op) => op,
                Err(PatchOpError::InvalidRequest { message, field }) => {
                    return Ok(PatchWriteOutcome::ApiError {
                        status: StatusCode::UNPROCESSABLE_ENTITY,
                        code: "INVALID_REQUEST",
                        message,
                        field: Some(field),
                        details: None,
                    });
                }
                Err(PatchOpError::HtmlForge(err)) => return Ok(PatchWriteOutcome::HtmlError(err)),
            };
            match patch_html_element(
                &working_html,
                &req.ff_id,
                &patch_op,
                SanitizeProfile::AppPreview,
            ) {
                Ok(out) if out.patched_count == 0 => {
                    return Ok(PatchWriteOutcome::ApiError {
                        status: StatusCode::CONFLICT,
                        code: "STALE_MAPPING",
                        message: format!(
                            "Target data-ff-id '{}' was not found in screen HTML",
                            req.ff_id
                        ),
                        field: Some("ffId"),
                        details: Some(serde_json::json!({
                            "missingFfId": req.ff_id,
                            "currentAstRevision": current_rev,
                        })),
                    });
                }
                Ok(out) => (
                    out.patched_html,
                    out.content_sha256,
                    out.execution_time_micros,
                ),
                Err(err) => return Ok(PatchWriteOutcome::HtmlError(err)),
            }
        };

        let next_rev = current_rev + 1;
        let patch_id = Uuid::new_v4().to_string();
        let payload_serialized =
            serde_json::to_string(&persisted_payload).unwrap_or_else(|_| "{}".to_string());
        let locked_serialized = locked_rule_receipt
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok());

        if let Err(e) = tx.execute(
            "INSERT INTO screen_artifact_patches (
                id, tenant_id, project_id, screen_id, ast_revision, base_artifact_sha,
                result_artifact_sha, ff_id, op_kind, apply_tier, reconciled_ff_ids,
                payload_json, html_snapshot, locked_rule_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                patch_id,
                tenant_id,
                req.project_id,
                screen_id,
                next_rev,
                req.base_artifact_sha256,
                result_sha,
                req.ff_id,
                req.op_kind,
                apply_tier,
                reconciled_ff_ids as i64,
                payload_serialized,
                result_html,
                locked_serialized,
                now_ms,
            ],
        ) {
            let is_unique_conflict = matches!(
                e,
                rusqlite::Error::SqliteFailure(ref sf, _)
                    if sf.code == rusqlite::ErrorCode::ConstraintViolation
                        && (sf.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                            || sf.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY)
            );
            if is_unique_conflict {
                return Ok(PatchWriteOutcome::ApiError {
                    status: StatusCode::CONFLICT,
                    code: "STALE_MAPPING",
                    message: format!("Concurrent screen revision conflict: {e}"),
                    field: None,
                    details: None,
                });
            }
            return Err(e);
        }

        Ok(PatchWriteOutcome::Committed(Box::new(SurgicalPatchResultData {
            patch_id,
            project_id: req.project_id,
            screen_id: screen_id_owned,
            ast_revision: next_rev,
            base_artifact_sha256: req.base_artifact_sha256,
            result_artifact_sha256: result_sha,
            ff_id: req.ff_id,
            op_kind: req.op_kind,
            apply_tier: apply_tier.to_string(),
            reconciled_ff_ids,
            html_snapshot: result_html,
            oklch_guard,
            locked_rule_receipt,
            undo_available: apply_tier == "INSTANT_UNDO",
            execution_time_micros: exec_micros,
            created_at: now_ms,
        })))
    }) {
        Ok(r) => r,
        Err(err) => return map_actor_error_to_response(err, request_id),
    };

    match tokio::task::spawn_blocking(move || receipt.wait()).await {
        Ok(Ok(PatchWriteOutcome::Committed(data))) => ok_response(*data, request_id),
        Ok(Ok(PatchWriteOutcome::HtmlError(err))) => map_html_forge_error(err, request_id),
        Ok(Ok(PatchWriteOutcome::ApiError {
            status,
            code,
            message,
            field,
            details,
        })) => error_response(
            status,
            code,
            &message,
            field.map(str::to_string),
            details,
            request_id,
        ),
        Ok(Err(err)) => map_actor_error_to_response(err, request_id),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Surgical patch write actor worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

fn is_unapplied_locked_receipt(apply_tier: &str, locked_rule_json: Option<&str>) -> bool {
    if apply_tier != "LOCKED_RULE_RECEIPT" {
        return false;
    }
    locked_rule_json
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|v| v.get("applied").and_then(|a| a.as_bool()))
        .is_some_and(|applied| !applied)
}

fn resolve_monotonic_undo_snapshot(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    screen_id: &str,
    current_rev: i64,
    fallback_html: &str,
    persisted_payload: &mut serde_json::Map<String, serde_json::Value>,
) -> String {
    let mut stack: Vec<(i64, String)> = Vec::new();
    if let Ok(mut stmt) = tx.prepare(
        "SELECT ast_revision, op_kind, apply_tier, locked_rule_json, html_snapshot
         FROM screen_artifact_patches
         WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
           AND ast_revision <= ?4
         ORDER BY ast_revision ASC",
    ) {
        if let Ok(rows) = stmt.query_map(
            params![tenant_id, project_id, screen_id, current_rev],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        ) {
            for (rev, op_kind, apply_tier, locked_json, html_snapshot) in rows.flatten() {
                if is_unapplied_locked_receipt(&apply_tier, locked_json.as_deref()) {
                    continue;
                }
                if op_kind == "UNDO" {
                    stack.pop();
                } else {
                    stack.push((rev, html_snapshot));
                }
            }
        }
    }

    stack.pop();
    if let Some((restored_rev, html)) = stack.last() {
        persisted_payload.insert(
            "restoredFromRevision".to_string(),
            serde_json::json!(*restored_rev),
        );
        html.clone()
    } else {
        persisted_payload.insert("restoredFromRevision".to_string(), serde_json::json!(0));
        fallback_html.to_string()
    }
}

fn build_patch_operation(
    req: &HttpSurgicalPatchRequest,
    oklch_guard: &mut Option<OklchContrastReport>,
) -> Result<HtmlPatchOperation, PatchOpError> {
    match req.op_kind.as_str() {
        "TEXT_EDIT" => {
            let text = req
                .payload_json
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PatchOpError::InvalidRequest {
                    message: "TEXT_EDIT requires payloadJson.text string".to_string(),
                    field: "payloadJson.text",
                })?;
            Ok(HtmlPatchOperation::TextEdit {
                text: text.to_string(),
            })
        }
        "TOKEN_SWAP" => {
            let mut css_vars = extract_string_map(&req.payload_json, "cssVars");
            if let (Some(fg), Some(bg)) = (
                req.payload_json.get("fgHex").and_then(|v| v.as_str()),
                req.payload_json.get("bgHex").and_then(|v| v.as_str()),
            ) {
                let report = evaluate_oklch_contrast_guard(fg, bg, WCAG_AA_MIN_CONTRAST)
                    .map_err(PatchOpError::HtmlForge)?;
                css_vars.insert(
                    "--stage-fg-safe".to_string(),
                    report.adjusted_fg_hex.clone(),
                );
                css_vars.insert("--stage-bg-safe".to_string(), report.bg_hex.clone());
                *oklch_guard = Some(report);
            }
            if css_vars.is_empty() {
                css_vars.insert("--stage-sketch-bg".to_string(), "#FAF9F6".to_string());
                css_vars.insert("--stage-sketch-ink".to_string(), "#2E2D2B".to_string());
            }
            Ok(HtmlPatchOperation::TokenSwap { css_vars })
        }
        "STEERING_CHIP" => {
            let chip_id = req
                .payload_json
                .get("chipId")
                .and_then(|v| v.as_str())
                .unwrap_or("more-whitespace")
                .to_string();
            let mut safe_styles = resolve_steering_chip_styles(&chip_id);
            for (k, v) in extract_string_map(&req.payload_json, "safeStyles") {
                safe_styles.insert(k, v);
            }
            Ok(HtmlPatchOperation::SteeringChip {
                chip_id,
                safe_styles,
            })
        }
        "LOCKED_RULE_CHANGE" => {
            if let Some(text) = req.payload_json.get("text").and_then(|v| v.as_str()) {
                Ok(HtmlPatchOperation::TextEdit {
                    text: text.to_string(),
                })
            } else {
                let safe_styles = extract_string_map(&req.payload_json, "safeStyles");
                Ok(HtmlPatchOperation::StylePatch { safe_styles })
            }
        }
        _ => Err(PatchOpError::InvalidRequest {
            message: "Unsupported opKind".to_string(),
            field: "opKind",
        }),
    }
}

fn map_html_forge_error(err: HtmlForgeError, request_id: String) -> Response {
    match err {
        HtmlForgeError::StaleMapping {
            missing_ff_ids,
            lowest_confidence,
            threshold,
        } => error_response(
            StatusCode::CONFLICT,
            "STALE_MAPPING",
            "Unable to reconcile data-ff-id anchors with sufficient confidence",
            Some("nextHtml".to_string()),
            Some(serde_json::json!({
                "missingFfIds": missing_ff_ids,
                "lowestConfidence": lowest_confidence,
                "threshold": threshold,
            })),
            request_id,
        ),
        HtmlForgeError::PayloadTooLarge { actual, max } => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            &format!("HTML size {actual} exceeds {max} bytes"),
            Some("baseHtml".to_string()),
            None,
            request_id,
        ),
        HtmlForgeError::MxssBlocked(detail) | HtmlForgeError::CssTokenBlocked(detail) => {
            error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "E_MXSS_BLOCKED",
                &format!("Blocked unsafe HTML/CSS construct: {detail}"),
                Some("payloadJson".to_string()),
                None,
                request_id,
            )
        }
        other => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &other.to_string(),
            None,
            None,
            request_id,
        ),
    }
}
