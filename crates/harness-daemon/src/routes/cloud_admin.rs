// @story 14.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Cloud Control Plane Ingestion API & SuperAdmin Portal MVP Routes)
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Extension, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, OptionalExtension, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use spec_registry::{retry_on_busy, SqlitePool};
use uuid::Uuid;

use crate::daemon_auth::authenticate_request;
use crate::routes::cloud_admin_auth::{
    append_admin_audit_log, ensure_v29_tables, is_role_permitted_for_operator,
    issue_admin_session_token, persist_sanitized_attachment, read_sanitized_attachment,
    require_admin_role, resolve_operator_bound_role, scrub_json_server_pii, scrub_server_pii,
    validate_and_reencode_attachment, verify_admin_session_from_headers, verify_oidc_assertion,
    verify_oidc_shared_secret_header, verify_server_mfa_code, AdminSessionClaims, CloudAdminRole,
    CloudAuthError, IngestRateLimiter, DEFAULT_ADMIN_SESSION_TTL_SECS,
};
use crate::routes::constant_time_eq;
use crate::routes::DaemonAppState;

const MAX_INGEST_TICKET_BODY_BYTES: usize = 8 * 1024 * 1024;
const INGEST_TICKET_ROUTE_LIMIT: usize = MAX_INGEST_TICKET_BODY_BYTES + 64 * 1024;
const MAX_INGEST_TELEMETRY_BODY_BYTES: usize = 128 * 1024;
const MAX_ADMIN_JSON_BODY_BYTES: usize = 128 * 1024;
const MS_PER_DAY: i64 = 86_400_000;
const MAX_FUTURE_SKEW_MS: i64 = 5 * 60 * 1_000;

const VALID_CLOUD_CATEGORIES: [&str; 5] =
    ["bug", "feature_request", "ux_friction", "billing", "other"];
const VALID_PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];
const VALID_TICKET_STATUSES: [&str; 5] = ["open", "triaged", "replied", "resolved", "closed"];
const VALID_CLUSTER_STATUSES: [&str; 4] = ["open", "planned", "shipped", "declined"];
const FUNNEL_STAGES: [(&str, &str); 5] = [
    ("day_0", "Day-0"),
    ("ai_connected", "AI Connected"),
    ("project", "Project"),
    ("preview", "Preview"),
    ("export", "Export"),
];
const PRIMARY_CHANNELS: [&str; 4] = ["cli", "desktop", "docker", "plugin"];
const PLUGIN_HOSTS: [&str; 5] = ["claude_code", "codex", "antigravity", "cursor", "cowok_ai"];

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

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn is_trusted_proxy_request(
    state: &DaemonAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> bool {
    if let Ok(secret) = std::env::var("COWOK_CLOUD_TRUSTED_PROXY_SECRET") {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            if let Some(provided) = headers
                .get("x-cloud-trusted-proxy-secret")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
            {
                if constant_time_eq(provided.as_bytes(), trimmed.as_bytes()) {
                    return true;
                }
            }
        }
    }
    authenticate_request(&state.auth, method, uri, headers, raw_body).is_ok()
}

fn extract_client_ip(
    state: &DaemonAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> String {
    if is_trusted_proxy_request(state, method, uri, headers, raw_body) {
        for key in ["x-client-ip", "x-real-ip", "x-forwarded-for"] {
            if let Some(raw) = headers.get(key).and_then(|v| v.to_str().ok()) {
                if let Some(first) = raw
                    .split(',')
                    .next()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return first.chars().take(64).collect();
                }
            }
        }
    }
    "loopback".to_string()
}

fn json_ok<T: Serialize>(status: StatusCode, data: T, request_id: String) -> Response {
    (
        status,
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

fn map_auth_err(err: CloudAuthError, request_id: String) -> Response {
    json_err(err.status, err.code, err.message, None, request_id)
}

fn rate_limited_response(retry_after_secs: u64, request_id: String) -> Response {
    let mut resp = json_err(
        StatusCode::TOO_MANY_REQUESTS,
        "RATE_LIMIT_EXCEEDED",
        format!("Cloud ingestion rate limit exceeded; retry after {retry_after_secs}s"),
        None,
        request_id,
    );
    if let Ok(val) = HeaderValue::from_str(&retry_after_secs.to_string()) {
        resp.headers_mut().insert(header::RETRY_AFTER, val);
    }
    resp
}

fn normalize_category(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "bug" | "bug_report" => Some("bug"),
        "feature_request" | "skill_template_request" => Some("feature_request"),
        "ux_friction" => Some("ux_friction"),
        "billing" => Some("billing"),
        "other" | "question" => Some("other"),
        _ => None,
    }
}

fn normalize_funnel_stage(raw: Option<&str>, event_name: &str) -> Option<String> {
    if let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) {
        let low = s.to_ascii_lowercase();
        if FUNNEL_STAGES.iter().any(|(k, _)| *k == low) {
            return Some(low);
        }
    }
    let low_event = event_name.trim().to_ascii_lowercase();
    match low_event.as_str() {
        "app_installed" | "day_0" | "onboarding_started" => Some("day_0".to_string()),
        "ai_connected" | "provider_connected" => Some("ai_connected".to_string()),
        "project_created" | "project" => Some("project".to_string()),
        "preview_opened" | "preview_rendered" | "preview" => Some("preview".to_string()),
        "export_completed" | "code_exported" | "export" => Some("export".to_string()),
        _ => None,
    }
}

fn percent_encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else if b == b' ' {
            out.push_str("%20");
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn build_github_issue_url(title: &str, summary: &str, ticket_count: usize) -> String {
    let encoded_title = percent_encode_query(&format!("[Feature Cluster] {title}"));
    let body = format!(
        "Aggregated from {ticket_count} non-tech user requests in Cowok SuperAdmin Portal.\n\nSummary:\n{summary}"
    );
    let encoded_body = percent_encode_query(&body);
    format!(
        "https://github.com/cowok-design/cowok-design/issues/new?title={encoded_title}&body={encoded_body}&labels=feature-request"
    )
}

fn compute_sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestTicketRequest {
    pub install_id: String,
    #[serde(default)]
    pub client_ticket_id: Option<String>,
    pub category: String,
    #[serde(default)]
    pub priority: Option<String>,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub diagnostic_bundle: Option<Value>,
    #[serde(default, alias = "screenshotDataUrl")]
    pub attachment_data_url: Option<String>,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_host_client")]
    pub host_client: String,
    #[serde(default = "default_app_version")]
    pub app_version: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

fn default_channel() -> String {
    "cli".to_string()
}

fn default_host_client() -> String {
    "standalone".to_string()
}

fn default_app_version() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudTicketReplyDto {
    pub id: String,
    pub ticket_id: String,
    pub author_actor: String,
    pub author_role: String,
    pub body_sanitized: String,
    pub synced_to_client: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSupportTicketDto {
    pub id: String,
    pub install_id: String,
    pub client_ticket_id: Option<String>,
    pub category: String,
    pub priority: String,
    pub status: String,
    pub title: String,
    pub description_sanitized: String,
    pub diagnostic_bundle: Value,
    pub attachment_sha256: Option<String>,
    pub attachment_mime: Option<String>,
    pub attachment_bytes: Option<i64>,
    pub cluster_id: Option<String>,
    pub channel: String,
    pub host_client: String,
    pub app_version: String,
    pub idempotency_key: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub replies: Vec<CloudTicketReplyDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestTicketPayload {
    ticket: CloudSupportTicketDto,
    deduplicated: bool,
    server_redaction_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestTelemetryRequest {
    pub install_id: String,
    pub event_name: String,
    #[serde(default)]
    pub funnel_stage: Option<String>,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_host_client")]
    pub host_client: String,
    #[serde(default = "default_app_version")]
    pub app_version: String,
    #[serde(default)]
    pub properties: Option<Value>,
    #[serde(default)]
    pub occurred_at: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestTelemetryPayload {
    event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    crash_fingerprint: Option<String>,
    server_redaction_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAuthSessionRequest {
    pub oidc_provider: String,
    pub subject: String,
    pub email: String,
    pub role: String,
    #[serde(default)]
    pub mfa_verified: bool,
    #[serde(default)]
    pub mfa_code: Option<String>,
    #[serde(default)]
    pub oidc_assertion: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminOperatorDto {
    subject: String,
    email: String,
    role: CloudAdminRole,
    mfa_verified: bool,
    oidc_provider: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminAuthSessionPayload {
    session_token: String,
    expires_at: i64,
    operator: AdminOperatorDto,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAdminTicketsQuery {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub channel: Option<String>,
    pub host_client: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListAdminTicketsPayload {
    tickets: Vec<CloudSupportTicketDto>,
    total_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyAdminTicketRequest {
    pub body: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplyAdminTicketPayload {
    ticket: CloudSupportTicketDto,
    reply: CloudTicketReplyDto,
    audit_log_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertClusterRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub ticket_ids: Vec<String>,
    #[serde(default)]
    pub github_issue_url: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFeatureClusterDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub ticket_count: i64,
    pub ticket_ids: Vec<String>,
    pub github_issue_url: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListClustersPayload {
    clusters: Vec<CloudFeatureClusterDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpsertClusterPayload {
    cluster: CloudFeatureClusterDto,
    audit_log_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BreakdownMetricDto {
    key: String,
    users: usize,
    events: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FunnelStepMetricDto {
    stage: String,
    label: String,
    users: usize,
    conversion_rate_pct: f64,
    drop_off_count: usize,
    drop_off_rate_pct: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureAdoptionMetricDto {
    event_name: String,
    total_events: usize,
    unique_installs: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalyticsFunnelPayload {
    dau: usize,
    wau: usize,
    mau: usize,
    open_tickets: usize,
    p0_tickets: usize,
    active_crash_groups: usize,
    channel_breakdown: Vec<BreakdownMetricDto>,
    host_breakdown: Vec<BreakdownMetricDto>,
    funnel_steps: Vec<FunnelStepMetricDto>,
    feature_adoption: Vec<FeatureAdoptionMetricDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCrashesQuery {
    pub app_version: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashGroupDto {
    pub fingerprint: String,
    pub app_version: String,
    pub error_signature: String,
    pub sample_stack_sanitized: String,
    pub occurrence_count: i64,
    pub affected_installs: i64,
    pub status: String,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListCrashesPayload {
    crashes: Vec<CrashGroupDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAuditLogDto {
    pub id: String,
    pub actor_subject: String,
    pub actor_role: String,
    pub mfa_verified: bool,
    pub action: String,
    pub target_resource: String,
    pub details: Value,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListAuditLogsPayload {
    logs: Vec<AdminAuditLogDto>,
}

fn load_replies_for_ticket(
    conn: &rusqlite::Connection,
    ticket_id: &str,
) -> Result<Vec<CloudTicketReplyDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, ticket_id, author_actor, author_role, body_sanitized, synced_to_client, created_at
         FROM cloud_ticket_replies
         WHERE ticket_id = ?1
         ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![ticket_id], |r| {
        Ok(CloudTicketReplyDto {
            id: r.get(0)?,
            ticket_id: r.get(1)?,
            author_actor: r.get(2)?,
            author_role: r.get(3)?,
            body_sanitized: r.get(4)?,
            synced_to_client: r.get::<_, i64>(5)? != 0,
            created_at: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item?);
    }
    Ok(out)
}

fn load_ticket_by_id(
    conn: &rusqlite::Connection,
    ticket_id: &str,
) -> Result<Option<CloudSupportTicketDto>, rusqlite::Error> {
    let row = conn
        .query_row(
            "SELECT id, install_id, client_ticket_id, category, priority, status, title,
                    description_sanitized, diagnostic_bundle_json, attachment_sha256,
                    attachment_mime, attachment_bytes, cluster_id, channel, host_client,
                    app_version, idempotency_key, created_at, updated_at
             FROM cloud_support_tickets
             WHERE id = ?1",
            params![ticket_id],
            |r| {
                let diag_str: String = r.get(8)?;
                let diag_val = serde_json::from_str(&diag_str).unwrap_or_else(|_| json!({}));
                Ok(CloudSupportTicketDto {
                    id: r.get(0)?,
                    install_id: r.get(1)?,
                    client_ticket_id: r.get(2)?,
                    category: r.get(3)?,
                    priority: r.get(4)?,
                    status: r.get(5)?,
                    title: r.get(6)?,
                    description_sanitized: r.get(7)?,
                    diagnostic_bundle: diag_val,
                    attachment_sha256: r.get(9)?,
                    attachment_mime: r.get(10)?,
                    attachment_bytes: r.get(11)?,
                    cluster_id: r.get(12)?,
                    channel: r.get(13)?,
                    host_client: r.get(14)?,
                    app_version: r.get(15)?,
                    idempotency_key: r.get(16)?,
                    created_at: r.get(17)?,
                    updated_at: r.get(18)?,
                    replies: Vec::new(),
                })
            },
        )
        .optional()?;
    match row {
        Some(mut ticket) => {
            ticket.replies = load_replies_for_ticket(conn, &ticket.id)?;
            Ok(Some(ticket))
        }
        None => Ok(None),
    }
}

struct SanitizedTicketFields<'a> {
    install_id: &'a str,
    client_ticket_id: Option<&'a str>,
    category: &'static str,
    priority: &'static str,
    title: &'a str,
    description: &'a str,
    diagnostic_bundle: &'a Value,
    channel: &'a str,
    host_client: &'a str,
    app_version: &'a str,
    idempotency_key: Option<&'a str>,
    attachment_meta: Option<(&'a str, &'a str, usize)>,
}

fn ingest_ticket_db(
    pool: &SqlitePool,
    fields: &SanitizedTicketFields<'_>,
) -> Result<(CloudSupportTicketDto, bool), String> {
    let diag_json =
        serde_json::to_string(fields.diagnostic_bundle).unwrap_or_else(|_| "{}".to_string());
    let now_ms = Utc::now().timestamp_millis();
    let new_id = format!("ctkt_{}", Uuid::new_v4().simple());
    let idem_key = fields
        .idempotency_key
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    retry_on_busy("cloud_admin_ingest_ticket", || -> Result<(CloudSupportTicketDto, bool), rusqlite::Error> {
        let mut conn = pool.connection().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let _ = ensure_v29_tables(&conn);
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(ref key) = idem_key {
            let existing_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM cloud_support_tickets WHERE idempotency_key = ?1",
                    params![key],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(eid) = existing_id {
                let ticket = load_ticket_by_id(&tx, &eid)?
                    .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                tx.commit()?;
                return Ok((ticket, true));
            }
        }
        let (att_sha, att_mime, att_bytes) = match fields.attachment_meta {
            Some((sha, mime, len)) => (Some(sha.to_string()), Some(mime.to_string()), Some(len as i64)),
            None => (None, None, None),
        };
        tx.execute(
            "INSERT INTO cloud_support_tickets (
                id, install_id, client_ticket_id, category, priority, status, title,
                description_sanitized, diagnostic_bundle_json, attachment_sha256,
                attachment_mime, attachment_bytes, cluster_id, channel, host_client,
                app_version, idempotency_key, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, ?14, ?15, ?16, ?16)",
            params![
                new_id,
                fields.install_id,
                fields.client_ticket_id,
                fields.category,
                fields.priority,
                fields.title,
                fields.description,
                diag_json,
                att_sha,
                att_mime,
                att_bytes,
                fields.channel,
                fields.host_client,
                fields.app_version,
                idem_key,
                now_ms,
            ],
        )?;
        let ticket =
            load_ticket_by_id(&tx, &new_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        tx.commit()?;
        Ok((ticket, false))
    })
    .map_err(|e| format!("ingest ticket storage failed: {e}"))
}

/// `POST /v1/ingest/tickets`
pub async fn handle_ingest_ticket(
    State(state): State<DaemonAppState>,
    Extension(limiter): Extension<Arc<IngestRateLimiter>>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    if body.len() > MAX_INGEST_TICKET_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Ingest ticket payload exceeds 8 MiB limit",
            Some("body"),
            request_id,
        );
    }
    let req: IngestTicketRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid ingest ticket JSON: {e}"),
                Some("body"),
                request_id,
            );
        }
    };
    if req.install_id.trim().is_empty() || req.install_id.trim().len() > 128 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "installId is required (1..=128 chars)",
            Some("installId"),
            request_id,
        );
    }
    let client_ip = extract_client_ip(&state, &Method::POST, &uri, &headers, &body);
    if let Err(exceeded) = limiter.check_and_record(&req.install_id, &client_ip) {
        return rate_limited_response(exceeded.retry_after_secs, request_id);
    }

    let Some(category) = normalize_category(&req.category) else {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            format!(
                "Unsupported category '{}'; expected one of {:?}",
                req.category, VALID_CLOUD_CATEGORIES
            ),
            Some("category"),
            request_id,
        );
    };
    let priority = match req.priority.as_deref().map(str::trim) {
        None | Some("") => "P2",
        Some(p) if VALID_PRIORITIES.contains(&p) => match p {
            "P0" => "P0",
            "P1" => "P1",
            "P3" => "P3",
            _ => "P2",
        },
        Some(_) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "priority must be one of P0, P1, P2, P3",
                Some("priority"),
                request_id,
            );
        }
    };

    let trimmed_title = req.title.trim();
    let trimmed_desc = req.description.trim();
    if trimmed_title.len() < 3 || trimmed_title.len() > 160 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "title must be 3..=160 characters",
            Some("title"),
            request_id,
        );
    }
    if trimmed_desc.len() < 5 || trimmed_desc.len() > 4000 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "description must be 5..=4000 characters",
            Some("description"),
            request_id,
        );
    }

    let sanitized_attachment = match req.attachment_data_url.as_deref() {
        Some(raw) if !raw.trim().is_empty() => match validate_and_reencode_attachment(raw) {
            Ok(att) => Some(att),
            Err(err) => {
                return json_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    err.code,
                    err.message,
                    Some("attachmentDataUrl"),
                    request_id,
                );
            }
        },
        _ => None,
    };

    let (clean_title, c1) = scrub_server_pii(trimmed_title);
    let (clean_desc, c2) = scrub_server_pii(trimmed_desc);
    let raw_diag = req.diagnostic_bundle.clone().unwrap_or_else(|| json!({}));
    let (clean_diag, c3) = scrub_json_server_pii(&raw_diag);
    let (clean_channel, c4) = scrub_server_pii(req.channel.trim());
    let (clean_host_client, c5) = scrub_server_pii(req.host_client.trim());
    let (clean_app_version, c6) = scrub_server_pii(req.app_version.trim());
    let (clean_client_ticket_id, c7) = match req.client_ticket_id.as_deref().map(str::trim) {
        Some(raw) if !raw.is_empty() => {
            let (scrubbed, cnt) = scrub_server_pii(raw);
            (Some(scrubbed), cnt)
        }
        _ => (None, 0),
    };
    let total_redactions = c1 + c2 + c3 + c4 + c5 + c6 + c7;

    let pool = state.registry.pool().clone();
    let workspace_root = pool.workspace_root().to_path_buf();
    let res = tokio::task::spawn_blocking(move || {
        if let Some(ref att) = sanitized_attachment {
            persist_sanitized_attachment(&workspace_root, att)?;
        }
        let meta = sanitized_attachment
            .as_ref()
            .map(|a| (a.sha256.as_str(), a.mime, a.byte_len));
        let fields = SanitizedTicketFields {
            install_id: req.install_id.trim(),
            client_ticket_id: clean_client_ticket_id.as_deref(),
            category,
            priority,
            title: &clean_title,
            description: &clean_desc,
            diagnostic_bundle: &clean_diag,
            channel: &clean_channel,
            host_client: &clean_host_client,
            app_version: &clean_app_version,
            idempotency_key: req.idempotency_key.as_deref(),
            attachment_meta: meta,
        };
        ingest_ticket_db(&pool, &fields)
    })
    .await;

    match res {
        Ok(Ok((ticket, deduplicated))) => json_ok(
            if deduplicated {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            },
            IngestTicketPayload {
                ticket,
                deduplicated,
                server_redaction_count: total_redactions,
            },
            request_id,
        ),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Ingest ticket worker panicked",
            None,
            request_id,
        ),
    }
}

struct SanitizedTelemetryFields<'a> {
    install_id: &'a str,
    event_name: &'a str,
    funnel_stage: Option<String>,
    channel: &'a str,
    host_client: &'a str,
    app_version: &'a str,
    properties: &'a Value,
    occurred_at: Option<i64>,
}

fn ingest_telemetry_db(
    pool: &SqlitePool,
    fields: &SanitizedTelemetryFields<'_>,
) -> Result<(String, Option<String>), String> {
    let event_id = format!("ctel_{}", Uuid::new_v4().simple());
    let server_now_ms = Utc::now().timestamp_millis();
    let min_allowed_ms = server_now_ms - 30 * MS_PER_DAY;
    let max_allowed_ms = server_now_ms + MAX_FUTURE_SKEW_MS;
    let now_ms = fields
        .occurred_at
        .filter(|ts| *ts > 0)
        .map(|ts| ts.clamp(min_allowed_ms, max_allowed_ms))
        .unwrap_or(server_now_ms);
    let is_crash = fields.event_name.trim().eq_ignore_ascii_case("app_crash");

    let crash_info = if is_crash {
        let err_sig = fields
            .properties
            .get("errorSignature")
            .or_else(|| fields.properties.get("error_signature"))
            .or_else(|| fields.properties.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("UnknownCrashSignature");
        let stack = fields
            .properties
            .get("sampleStack")
            .or_else(|| fields.properties.get("stack"))
            .and_then(Value::as_str)
            .unwrap_or("No sanitized stack provided");
        let explicit_fp = fields
            .properties
            .get("fingerprint")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let fp = explicit_fp.map(str::to_string).unwrap_or_else(|| {
            let hash = compute_sha256_hex(&format!("{}::{}", fields.app_version, err_sig));
            format!("fp_{}", &hash[..16])
        });
        let (clean_sig, _) = scrub_server_pii(err_sig);
        let (clean_stack, _) = scrub_server_pii(stack);
        Some((fp, clean_sig, clean_stack))
    } else {
        None
    };

    let mut props_to_store = fields.properties.clone();
    if let Some((ref fp, _, _)) = crash_info {
        if let Some(obj) = props_to_store.as_object_mut() {
            obj.insert("fingerprint".to_string(), Value::String(fp.clone()));
        }
    }
    let props_json = serde_json::to_string(&props_to_store).unwrap_or_else(|_| "{}".to_string());

    retry_on_busy("cloud_admin_ingest_telemetry", || -> Result<(), rusqlite::Error> {
        let mut conn = pool.connection().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let _ = ensure_v29_tables(&conn);
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO cloud_telemetry_events (
                id, install_id, event_name, funnel_stage, channel, host_client, app_version, properties_json, occurred_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event_id,
                fields.install_id,
                fields.event_name,
                fields.funnel_stage,
                fields.channel,
                fields.host_client,
                fields.app_version,
                props_json,
                now_ms,
            ],
        )?;

        if let Some((ref fp, ref sig, ref stack)) = crash_info {
            let distinct_installs: i64 = tx
                .query_row(
                    "SELECT COUNT(DISTINCT install_id)
                     FROM cloud_telemetry_events
                     WHERE LOWER(event_name) = 'app_crash'
                       AND json_extract(properties_json, '$.fingerprint') = ?1",
                    params![fp],
                    |r| r.get(0),
                )
                .unwrap_or(1)
                .max(1);
            tx.execute(
                "INSERT INTO cloud_crash_groups (
                    fingerprint, app_version, error_signature, sample_stack_sanitized,
                    occurrence_count, affected_installs, status, first_seen_at, last_seen_at
                ) VALUES (?1, ?2, ?3, ?4, 1, ?5, 'open', ?6, ?6)
                ON CONFLICT(fingerprint) DO UPDATE SET
                    app_version = excluded.app_version,
                    error_signature = excluded.error_signature,
                    sample_stack_sanitized = excluded.sample_stack_sanitized,
                    occurrence_count = cloud_crash_groups.occurrence_count + 1,
                    affected_installs = MAX(cloud_crash_groups.affected_installs, excluded.affected_installs),
                    status = CASE WHEN cloud_crash_groups.status = 'resolved' THEN 'open' ELSE cloud_crash_groups.status END,
                    last_seen_at = MAX(cloud_crash_groups.last_seen_at, excluded.last_seen_at)",
                params![fp, fields.app_version, sig, stack, distinct_installs, now_ms],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .map_err(|e| format!("ingest telemetry storage failed: {e}"))?;

    Ok((event_id, crash_info.map(|(fp, _, _)| fp)))
}

/// `POST /v1/ingest/telemetry`
pub async fn handle_ingest_telemetry(
    State(state): State<DaemonAppState>,
    Extension(limiter): Extension<Arc<IngestRateLimiter>>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    if body.len() > MAX_INGEST_TELEMETRY_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Telemetry payload exceeds 128 KiB limit",
            Some("body"),
            request_id,
        );
    }
    let req: IngestTelemetryRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid ingest telemetry JSON: {e}"),
                Some("body"),
                request_id,
            );
        }
    };
    if req.install_id.trim().is_empty() || req.event_name.trim().is_empty() {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "installId and eventName are required",
            Some("installId"),
            request_id,
        );
    }
    let client_ip = extract_client_ip(&state, &Method::POST, &uri, &headers, &body);
    if let Err(exceeded) = limiter.check_and_record(&req.install_id, &client_ip) {
        return rate_limited_response(exceeded.retry_after_secs, request_id);
    }

    let raw_props = req.properties.clone().unwrap_or_else(|| json!({}));
    let (clean_props, c0) = scrub_json_server_pii(&raw_props);
    let (clean_event_name, c1) = scrub_server_pii(req.event_name.trim());
    let (clean_channel, c2) = scrub_server_pii(req.channel.trim());
    let (clean_host_client, c3) = scrub_server_pii(req.host_client.trim());
    let (clean_app_version, c4) = scrub_server_pii(req.app_version.trim());
    let redaction_count = c0 + c1 + c2 + c3 + c4;
    let funnel_stage = normalize_funnel_stage(req.funnel_stage.as_deref(), &clean_event_name);
    let pool = state.registry.pool().clone();

    let res = tokio::task::spawn_blocking(move || {
        let fields = SanitizedTelemetryFields {
            install_id: req.install_id.trim(),
            event_name: &clean_event_name,
            funnel_stage,
            channel: &clean_channel,
            host_client: &clean_host_client,
            app_version: &clean_app_version,
            properties: &clean_props,
            occurred_at: req.occurred_at,
        };
        ingest_telemetry_db(&pool, &fields)
    })
    .await;

    match res {
        Ok(Ok((event_id, crash_fingerprint))) => json_ok(
            StatusCode::CREATED,
            IngestTelemetryPayload {
                event_id,
                crash_fingerprint,
                server_redaction_count: redaction_count,
            },
            request_id,
        ),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Ingest telemetry worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/auth/session`
pub async fn handle_admin_auth_session(
    State(state): State<DaemonAppState>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    if body.len() > MAX_ADMIN_JSON_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Auth session payload too large",
            Some("body"),
            request_id,
        );
    }
    let req: AdminAuthSessionRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid OIDC session payload: {e}"),
                Some("body"),
                request_id,
            );
        }
    };
    let trimmed_provider = req.oidc_provider.trim();
    let trimmed_subject = req.subject.trim();
    let trimmed_email = req.email.trim();
    if trimmed_provider.is_empty() || trimmed_subject.is_empty() || trimmed_email.is_empty() {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "oidcProvider, subject, and email are required",
            Some("subject"),
            request_id,
        );
    }

    let Some(requested_role) = CloudAdminRole::parse_strict(&req.role) else {
        return json_err(
            StatusCode::FORBIDDEN,
            "RBAC_FORBIDDEN",
            "Role must be one of SUPER_ADMIN, SUPPORT_AGENT, SKILL_CURATOR, ANALYST_READONLY",
            Some("role"),
            request_id,
        );
    };

    // 1. Server-side OIDC credential verification (HMAC-signed assertion, shared OIDC secret, or authenticated BFF loopback)
    let assertion_role = if let Some(assertion) = req
        .oidc_assertion
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let verified = match verify_oidc_assertion(&state.auth.daemon_nonce, assertion) {
            Ok(c) => c,
            Err(err) => return map_auth_err(err, request_id),
        };
        if !verified.email.eq_ignore_ascii_case(trimmed_email)
            || verified.subject != trimmed_subject
            || verified.oidc_provider != trimmed_provider
        {
            return json_err(
                StatusCode::UNAUTHORIZED,
                "ADMIN_AUTH_REQUIRED",
                "OIDC assertion identity does not match requested operator",
                Some("oidcAssertion"),
                request_id,
            );
        }
        Some(verified.role)
    } else if verify_oidc_shared_secret_header(&state.auth.daemon_nonce, &headers)
        || authenticate_request(&state.auth, &Method::POST, &uri, &headers, &body).is_ok()
    {
        None
    } else {
        return json_err(
            StatusCode::UNAUTHORIZED,
            "ADMIN_AUTH_REQUIRED",
            "Server-side OIDC assertion or trusted BFF credential is required to mint a SuperAdmin session",
            None,
            request_id,
        );
    };

    // 2. Mandatory server-side operator allowlist & role binding (never allow untrusted self-promotion)
    let Some(allowlist_role) = resolve_operator_bound_role(trimmed_email) else {
        return json_err(
            StatusCode::FORBIDDEN,
            "RBAC_FORBIDDEN",
            "Operator identity is not authorized in the Cloud Admin OIDC directory",
            Some("email"),
            request_id,
        );
    };
    if !is_role_permitted_for_operator(allowlist_role, requested_role) {
        return json_err(
            StatusCode::FORBIDDEN,
            "RBAC_FORBIDDEN",
            format!(
                "Operator '{}' bound to '{}' cannot self-promote to '{}'",
                trimmed_email,
                allowlist_role.as_str(),
                requested_role.as_str()
            ),
            Some("role"),
            request_id,
        );
    }
    if let Some(bound_by_assertion) = assertion_role {
        if !is_role_permitted_for_operator(bound_by_assertion, requested_role) {
            return json_err(
                StatusCode::FORBIDDEN,
                "RBAC_FORBIDDEN",
                "Requested role exceeds verified OIDC assertion role binding",
                Some("role"),
                request_id,
            );
        }
    }

    // 3. If an existing admin session header is supplied (e.g. role preview), forbid upward escalation
    if headers.contains_key("x-admin-session") {
        let existing_claims = match verify_admin_session_from_headers(&headers) {
            Ok(c) => c,
            Err(err) => return map_auth_err(err, request_id),
        };
        if !is_role_permitted_for_operator(existing_claims.role, requested_role) {
            return json_err(
                StatusCode::FORBIDDEN,
                "RBAC_FORBIDDEN",
                "Active admin session cannot escalate privileges; re-authenticate with an authorized operator",
                Some("role"),
                request_id,
            );
        }
    }

    // 4. Mandatory server-side 6-digit MFA verification
    if !req.mfa_verified {
        return json_err(
            StatusCode::UNAUTHORIZED,
            "MFA_REQUIRED",
            "OIDC login rejected: hardware/TOTP MFA verification is mandatory for SuperAdmin Portal",
            Some("mfaVerified"),
            request_id,
        );
    }
    if let Err(err) = verify_server_mfa_code(req.mfa_code.as_deref()) {
        return map_auth_err(err, request_id);
    }

    let now = Utc::now().timestamp();
    let expires_at = now + DEFAULT_ADMIN_SESSION_TTL_SECS;
    let claims = AdminSessionClaims {
        subject: trimmed_subject.to_string(),
        email: trimmed_email.to_string(),
        role: requested_role,
        mfa_verified: true,
        oidc_provider: trimmed_provider.to_string(),
        issued_at: now,
        expires_at,
        jti: format!("jti_{}", Uuid::new_v4().simple()),
    };
    let token = match issue_admin_session_token(&claims) {
        Ok(tok) => tok,
        Err(e) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TOKEN_SIGN_ERROR",
                e,
                None,
                request_id,
            );
        }
    };

    let pool = state.registry.pool().clone();
    let audit_claims = claims.clone();
    let audit_res = tokio::task::spawn_blocking(move || {
        append_admin_audit_log(
            &pool,
            &audit_claims,
            "admin.session.issued",
            &format!("oidc:{}", audit_claims.oidc_provider),
            &json!({ "role": audit_claims.role.as_str(), "jti": audit_claims.jti }),
        )
    })
    .await;
    match audit_res {
        Ok(Ok(_)) => {}
        Ok(Err(msg)) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUDIT_LOG_ERROR",
                format!("Failed to persist immutable session audit log: {msg}"),
                None,
                request_id,
            );
        }
        Err(_) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Admin session audit log worker panicked",
                None,
                request_id,
            );
        }
    }

    json_ok(
        StatusCode::OK,
        AdminAuthSessionPayload {
            session_token: token,
            expires_at,
            operator: AdminOperatorDto {
                subject: claims.subject,
                email: claims.email,
                role: claims.role,
                mfa_verified: claims.mfa_verified,
                oidc_provider: claims.oidc_provider,
            },
        },
        request_id,
    )
}

fn escape_sql_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for ch in input.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            other => out.push(other),
        }
    }
    out
}

fn list_admin_tickets_db(
    pool: &SqlitePool,
    query: &ListAdminTicketsQuery,
) -> Result<(Vec<CloudSupportTicketDto>, usize), String> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v29_tables(&conn)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200) as i64;

    let mut clauses: Vec<String> = Vec::new();
    let mut bind_values: Vec<SqlValue> = Vec::new();

    if let Some(ref st) = query.status {
        let trimmed = st.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            clauses.push("status = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    if let Some(ref pr) = query.priority {
        let trimmed = pr.trim();
        if !trimmed.is_empty() && trimmed != "ALL" {
            clauses.push("priority = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    if let Some(ref ch) = query.channel {
        let trimmed = ch.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            clauses.push("channel = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    if let Some(ref hc) = query.host_client {
        let trimmed = hc.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            clauses.push("host_client = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    if let Some(ref cat) = query.category {
        let trimmed = cat.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            clauses.push("category = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    if let Some(needle) = query
        .search
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
    {
        let pat = format!("%{}%", escape_sql_like(&needle));
        clauses.push(
            "(LOWER(title) LIKE ? ESCAPE '\\' OR LOWER(description_sanitized) LIKE ? ESCAPE '\\' OR LOWER(install_id) LIKE ? ESCAPE '\\' OR LOWER(id) LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        for _ in 0..4 {
            bind_values.push(SqlValue::Text(pat.clone()));
        }
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(1) FROM cloud_support_tickets{where_sql}");
    let total_count: usize = conn
        .query_row(&count_sql, params_from_iter(bind_values.iter()), |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| format!("count tickets: {e}"))?
        .max(0) as usize;

    let select_sql = format!(
        "SELECT id, install_id, client_ticket_id, category, priority, status, title,
                description_sanitized, diagnostic_bundle_json, attachment_sha256,
                attachment_mime, attachment_bytes, cluster_id, channel, host_client,
                app_version, idempotency_key, created_at, updated_at
         FROM cloud_support_tickets{where_sql}
         ORDER BY
           CASE priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
           updated_at DESC
         LIMIT ?"
    );
    let mut query_values = bind_values;
    query_values.push(SqlValue::Integer(limit));

    let mut stmt = conn
        .prepare(&select_sql)
        .map_err(|e| format!("prepare list tickets: {e}"))?;

    let rows = stmt
        .query_map(params_from_iter(query_values.iter()), |r| {
            let diag_str: String = r.get(8)?;
            let diag_val = serde_json::from_str(&diag_str).unwrap_or_else(|_| json!({}));
            Ok(CloudSupportTicketDto {
                id: r.get(0)?,
                install_id: r.get(1)?,
                client_ticket_id: r.get(2)?,
                category: r.get(3)?,
                priority: r.get(4)?,
                status: r.get(5)?,
                title: r.get(6)?,
                description_sanitized: r.get(7)?,
                diagnostic_bundle: diag_val,
                attachment_sha256: r.get(9)?,
                attachment_mime: r.get(10)?,
                attachment_bytes: r.get(11)?,
                cluster_id: r.get(12)?,
                channel: r.get(13)?,
                host_client: r.get(14)?,
                app_version: r.get(15)?,
                idempotency_key: r.get(16)?,
                created_at: r.get(17)?,
                updated_at: r.get(18)?,
                replies: Vec::new(),
            })
        })
        .map_err(|e| format!("query list tickets: {e}"))?;

    let mut out = Vec::new();
    for item in rows {
        let mut ticket = item.map_err(|e| format!("row ticket: {e}"))?;
        ticket.replies =
            load_replies_for_ticket(&conn, &ticket.id).map_err(|e| format!("load replies: {e}"))?;
        out.push(ticket);
    }
    Ok((out, total_count))
}

/// `GET /v1/admin/tickets`
pub async fn handle_list_admin_tickets(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<ListAdminTicketsQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[
            CloudAdminRole::SuperAdmin,
            CloudAdminRole::SupportAgent,
            CloudAdminRole::SkillCurator,
            CloudAdminRole::AnalystReadonly,
        ],
    ) {
        return map_auth_err(err, request_id);
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || list_admin_tickets_db(&pool, &query)).await;
    match res {
        Ok(Ok((tickets, total_count))) => json_ok(
            StatusCode::OK,
            ListAdminTicketsPayload {
                tickets,
                total_count,
            },
            request_id,
        ),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List admin tickets worker panicked",
            None,
            request_id,
        ),
    }
}

enum AttachmentLookupError {
    TicketNotFound,
    AttachmentNotFound,
    Storage(String),
}

fn load_ticket_attachment_bytes(
    pool: &SqlitePool,
    ticket_id: &str,
) -> Result<(Vec<u8>, String), AttachmentLookupError> {
    let conn = pool
        .connection()
        .map_err(|e| AttachmentLookupError::Storage(format!("db conn: {e}")))?;
    ensure_v29_tables(&conn).map_err(AttachmentLookupError::Storage)?;
    let meta: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT attachment_sha256, attachment_mime FROM cloud_support_tickets WHERE id = ?1",
            params![ticket_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| AttachmentLookupError::Storage(format!("lookup attachment: {e}")))?;

    let Some((sha_opt, mime_opt)) = meta else {
        return Err(AttachmentLookupError::TicketNotFound);
    };
    let (Some(sha), Some(mime)) = (sha_opt, mime_opt) else {
        return Err(AttachmentLookupError::AttachmentNotFound);
    };
    let bytes = read_sanitized_attachment(pool.workspace_root(), &sha, &mime)
        .map_err(|_| AttachmentLookupError::AttachmentNotFound)?;
    Ok((bytes, mime))
}

/// `GET /v1/admin/tickets/:id/attachment` (RBAC-guarded sanitized image blob retrieval)
pub async fn handle_get_ticket_attachment(
    State(state): State<DaemonAppState>,
    Path(ticket_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[
            CloudAdminRole::SuperAdmin,
            CloudAdminRole::SupportAgent,
            CloudAdminRole::SkillCurator,
            CloudAdminRole::AnalystReadonly,
        ],
    ) {
        return map_auth_err(err, request_id);
    }

    let pool = state.registry.pool().clone();
    let res =
        tokio::task::spawn_blocking(move || load_ticket_attachment_bytes(&pool, ticket_id.trim()))
            .await;

    match res {
        Ok(Ok((bytes, mime))) => {
            let mut resp = (StatusCode::OK, bytes).into_response();
            if let Ok(ct) = HeaderValue::from_str(&mime) {
                resp.headers_mut().insert(header::CONTENT_TYPE, ct);
            }
            resp.headers_mut().insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            resp
        }
        Ok(Err(AttachmentLookupError::TicketNotFound)) => json_err(
            StatusCode::NOT_FOUND,
            "TICKET_NOT_FOUND",
            "Cloud support ticket not found",
            None,
            request_id,
        ),
        Ok(Err(AttachmentLookupError::AttachmentNotFound)) => json_err(
            StatusCode::NOT_FOUND,
            "ATTACHMENT_NOT_FOUND",
            "Ticket has no stored sanitized attachment",
            None,
            request_id,
        ),
        Ok(Err(AttachmentLookupError::Storage(msg))) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Attachment reader worker panicked",
            None,
            request_id,
        ),
    }
}

fn sync_reply_to_local_support_ticket(
    tx: &rusqlite::Transaction<'_>,
    client_ticket_id: &str,
    reply_id: &str,
    author_actor: &str,
    body_sanitized: &str,
    created_at: i64,
    cloud_status: &str,
) -> Result<bool, rusqlite::Error> {
    let has_table: bool = tx.query_row(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='local_support_tickets'",
        [],
        |r| r.get::<_, i64>(0),
    )? > 0;
    if !has_table {
        return Ok(false);
    }
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT id, admin_replies_json FROM local_support_tickets WHERE id = ?1 OR ticket_code = ?1",
            params![client_ticket_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((local_id, replies_raw)) = existing else {
        return Ok(false);
    };
    let mut arr: Vec<Value> = serde_json::from_str(&replies_raw).unwrap_or_default();
    arr.push(json!({
        "id": reply_id,
        "author": author_actor,
        "message": body_sanitized,
        "createdAt": created_at,
    }));
    let updated_json = serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
    let mapped_status = match cloud_status {
        "triaged" => "triaged",
        "resolved" => "resolved",
        "closed" => "closed",
        "replied" => "in_progress",
        _ => "submitted",
    };
    let rows_updated = tx.execute(
        "UPDATE local_support_tickets
         SET admin_replies_json = ?1, status = ?2, sync_status = 'synced', updated_at = ?3
         WHERE id = ?4",
        params![updated_json, mapped_status, created_at, local_id],
    )?;
    Ok(rows_updated > 0)
}

enum ReplyError {
    NotFound,
    Storage(String),
}

fn reply_ticket_db(
    pool: &SqlitePool,
    ticket_id: &str,
    claims: &AdminSessionClaims,
    clean_body: &str,
    next_priority: Option<&str>,
    next_status: &str,
) -> Result<(CloudSupportTicketDto, CloudTicketReplyDto, String), ReplyError> {
    let reply_id = format!("crep_{}", Uuid::new_v4().simple());
    let audit_id = format!("aud_{}", Uuid::new_v4().simple());
    let now_ms = Utc::now().timestamp_millis();

    let result = retry_on_busy(
        "cloud_admin_reply_ticket",
        || -> Result<Option<(CloudSupportTicketDto, CloudTicketReplyDto)>, rusqlite::Error> {
            let mut conn = pool.connection().map_err(|_| rusqlite::Error::InvalidQuery)?;
            let _ = ensure_v29_tables(&conn);
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let Some(existing) = load_ticket_by_id(&tx, ticket_id)? else {
                tx.commit()?;
                return Ok(None);
            };
            let effective_priority = next_priority.unwrap_or(&existing.priority);
            let synced_to_client = match existing.client_ticket_id.as_deref() {
                Some(cid) if !cid.trim().is_empty() => sync_reply_to_local_support_ticket(
                    &tx,
                    cid.trim(),
                    &reply_id,
                    &claims.email,
                    clean_body,
                    now_ms,
                    next_status,
                )?,
                _ => false,
            };
            let synced_flag: i64 = if synced_to_client { 1 } else { 0 };
            tx.execute(
                "INSERT INTO cloud_ticket_replies (
                    id, ticket_id, author_actor, author_role, body_sanitized, synced_to_client, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    reply_id,
                    ticket_id,
                    claims.email,
                    claims.role.as_str(),
                    clean_body,
                    synced_flag,
                    now_ms,
                ],
            )?;
            tx.execute(
                "UPDATE cloud_support_tickets
                 SET status = ?1, priority = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![next_status, effective_priority, now_ms, ticket_id],
            )?;
            let reply_dto = CloudTicketReplyDto {
                id: reply_id.clone(),
                ticket_id: ticket_id.to_string(),
                author_actor: claims.email.clone(),
                author_role: claims.role.as_str().to_string(),
                body_sanitized: clean_body.to_string(),
                synced_to_client,
                created_at: now_ms,
            };
            let details_json = json!({
                "replyId": reply_id,
                "priority": effective_priority,
                "status": next_status,
                "syncedToClient": synced_to_client,
            })
            .to_string();
            tx.execute(
                "INSERT INTO admin_audit_logs (
                    id, actor_subject, actor_role, mfa_verified, action, target_resource, details_json, created_at
                ) VALUES (?1, ?2, ?3, 1, 'ticket.replied', ?4, ?5, ?6)",
                params![
                    audit_id,
                    claims.subject,
                    claims.role.as_str(),
                    format!("ticket:{ticket_id}"),
                    details_json,
                    now_ms,
                ],
            )?;
            let updated_ticket =
                load_ticket_by_id(&tx, ticket_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            tx.commit()?;
            Ok(Some((updated_ticket, reply_dto)))
        },
    )
    .map_err(|e| ReplyError::Storage(format!("reply ticket storage failed: {e}")))?;

    match result {
        Some((ticket, reply)) => Ok((ticket, reply, audit_id)),
        None => Err(ReplyError::NotFound),
    }
}

/// `POST /v1/admin/tickets/:id/reply`
pub async fn handle_reply_admin_ticket(
    State(state): State<DaemonAppState>,
    Path(ticket_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[CloudAdminRole::SuperAdmin, CloudAdminRole::SupportAgent],
    ) {
        return map_auth_err(err, request_id);
    }
    if body.len() > MAX_ADMIN_JSON_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Reply payload exceeds limit",
            Some("body"),
            request_id,
        );
    }
    let req: ReplyAdminTicketRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid ticket reply JSON: {e}"),
                Some("body"),
                request_id,
            );
        }
    };
    let trimmed_body = req.body.trim();
    if trimmed_body.is_empty() || trimmed_body.len() > 4000 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Reply body must be 1..=4000 characters",
            Some("body"),
            request_id,
        );
    }
    if let Some(ref pr) = req.priority {
        if !VALID_PRIORITIES.contains(&pr.trim()) {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "priority must be P0, P1, P2, or P3",
                Some("priority"),
                request_id,
            );
        }
    }
    let next_status = req
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("replied")
        .to_string();
    if !VALID_TICKET_STATUSES.contains(&next_status.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "status must be open, triaged, replied, resolved, or closed",
            Some("status"),
            request_id,
        );
    }

    let (clean_body, _) = scrub_server_pii(trimmed_body);
    let next_priority = req.priority.map(|p| p.trim().to_string());
    let pool = state.registry.pool().clone();

    let res = tokio::task::spawn_blocking(move || {
        reply_ticket_db(
            &pool,
            ticket_id.trim(),
            &claims,
            &clean_body,
            next_priority.as_deref(),
            &next_status,
        )
    })
    .await;

    match res {
        Ok(Ok((ticket, reply, audit_log_id))) => json_ok(
            StatusCode::CREATED,
            ReplyAdminTicketPayload {
                ticket,
                reply,
                audit_log_id,
            },
            request_id,
        ),
        Ok(Err(ReplyError::NotFound)) => json_err(
            StatusCode::NOT_FOUND,
            "TICKET_NOT_FOUND",
            "Cloud support ticket not found",
            None,
            request_id,
        ),
        Ok(Err(ReplyError::Storage(msg))) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Reply ticket worker panicked",
            None,
            request_id,
        ),
    }
}

fn load_cluster_ticket_ids(
    conn: &rusqlite::Connection,
    cluster_id: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id FROM cloud_support_tickets WHERE cluster_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![cluster_id], |r| r.get::<_, String>(0))?;
    let mut ids = Vec::new();
    for item in rows {
        ids.push(item?);
    }
    Ok(ids)
}

fn list_clusters_db(pool: &SqlitePool) -> Result<Vec<CloudFeatureClusterDto>, String> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v29_tables(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, summary, ticket_count, github_issue_url, status, created_at, updated_at
             FROM cloud_feature_clusters
             ORDER BY ticket_count DESC, updated_at DESC",
        )
        .map_err(|e| format!("prepare list clusters: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CloudFeatureClusterDto {
                id: r.get(0)?,
                title: r.get(1)?,
                summary: r.get(2)?,
                ticket_count: r.get(3)?,
                ticket_ids: Vec::new(),
                github_issue_url: r.get(4)?,
                status: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })
        .map_err(|e| format!("query list clusters: {e}"))?;

    let mut out = Vec::new();
    for item in rows {
        let mut cluster = item.map_err(|e| format!("cluster row: {e}"))?;
        cluster.ticket_ids = load_cluster_ticket_ids(&conn, &cluster.id)
            .map_err(|e| format!("cluster ticket ids: {e}"))?;
        out.push(cluster);
    }
    Ok(out)
}

/// `GET /v1/admin/clusters`
pub async fn handle_list_clusters(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[
            CloudAdminRole::SuperAdmin,
            CloudAdminRole::SupportAgent,
            CloudAdminRole::SkillCurator,
            CloudAdminRole::AnalystReadonly,
        ],
    ) {
        return map_auth_err(err, request_id);
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || list_clusters_db(&pool)).await;
    match res {
        Ok(Ok(clusters)) => json_ok(StatusCode::OK, ListClustersPayload { clusters }, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List clusters worker panicked",
            None,
            request_id,
        ),
    }
}

fn upsert_cluster_db(
    pool: &SqlitePool,
    claims: &AdminSessionClaims,
    req: &UpsertClusterRequest,
    clean_title: &str,
    clean_summary: &str,
    status: &str,
) -> Result<(CloudFeatureClusterDto, String), String> {
    let cluster_id = req
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("cclu_{}", Uuid::new_v4().simple()));
    let audit_id = format!("aud_{}", Uuid::new_v4().simple());
    let now_ms = Utc::now().timestamp_millis();

    let cluster = retry_on_busy("cloud_admin_create_cluster", || -> Result<CloudFeatureClusterDto, rusqlite::Error> {
        let mut conn = pool.connection().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let _ = ensure_v29_tables(&conn);
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let existing_created: Option<i64> = tx
            .query_row(
                "SELECT created_at FROM cloud_feature_clusters WHERE id = ?1",
                params![cluster_id],
                |r| r.get(0),
            )
            .optional()?;
        let created_at = existing_created.unwrap_or(now_ms);

        let mut target_ticket_set: HashSet<String> = HashSet::new();
        let mut target_ticket_ids: Vec<String> = Vec::new();
        for tid in &req.ticket_ids {
            let trimmed_tid = tid.trim();
            if !trimmed_tid.is_empty() && target_ticket_set.insert(trimmed_tid.to_string()) {
                target_ticket_ids.push(trimmed_tid.to_string());
            }
        }

        let mut affected_clusters: HashSet<String> = HashSet::new();
        affected_clusters.insert(cluster_id.clone());

        if existing_created.is_some() {
            let currently_linked = load_cluster_ticket_ids(&tx, &cluster_id)?;
            for old_tid in currently_linked {
                if !target_ticket_set.contains(&old_tid) {
                    tx.execute(
                        "UPDATE cloud_support_tickets
                         SET cluster_id = NULL, updated_at = ?1
                         WHERE id = ?2 AND cluster_id = ?3",
                        params![now_ms, old_tid, cluster_id],
                    )?;
                }
            }
        }

        for trimmed_tid in &target_ticket_ids {
            let prev_cluster: Option<String> = tx
                .query_row(
                    "SELECT cluster_id FROM cloud_support_tickets WHERE id = ?1",
                    params![trimmed_tid],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            if let Some(prev_cid) = prev_cluster {
                if prev_cid != cluster_id {
                    affected_clusters.insert(prev_cid);
                }
            }
            tx.execute(
                "UPDATE cloud_support_tickets SET cluster_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![cluster_id, now_ms, trimmed_tid],
            )?;
        }

        let linked_ids = load_cluster_ticket_ids(&tx, &cluster_id)?;
        let count = linked_ids.len() as i64;
        let gh_url = req
            .github_issue_url
            .as_deref()
            .map(str::trim)
            .filter(|s| s.starts_with("https://github.com/"))
            .map(str::to_string)
            .unwrap_or_else(|| build_github_issue_url(clean_title, clean_summary, linked_ids.len()));

        tx.execute(
            "INSERT INTO cloud_feature_clusters (
                id, title, summary, ticket_count, github_issue_url, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                summary = excluded.summary,
                ticket_count = excluded.ticket_count,
                github_issue_url = excluded.github_issue_url,
                status = excluded.status,
                updated_at = excluded.updated_at",
            params![
                cluster_id,
                clean_title,
                clean_summary,
                count,
                gh_url,
                status,
                created_at,
                now_ms,
            ],
        )?;

        for affected_cid in &affected_clusters {
            tx.execute(
                "UPDATE cloud_feature_clusters
                 SET ticket_count = (SELECT COUNT(1) FROM cloud_support_tickets WHERE cluster_id = ?1),
                     updated_at = ?2
                 WHERE id = ?1",
                params![affected_cid, now_ms],
            )?;
        }

        let details_json = json!({
            "clusterId": cluster_id,
            "ticketCount": count,
            "status": status,
        })
        .to_string();
        tx.execute(
            "INSERT INTO admin_audit_logs (
                id, actor_subject, actor_role, mfa_verified, action, target_resource, details_json, created_at
            ) VALUES (?1, ?2, ?3, 1, 'cluster.upserted', ?4, ?5, ?6)",
            params![
                audit_id,
                claims.subject,
                claims.role.as_str(),
                format!("cluster:{cluster_id}"),
                details_json,
                now_ms,
            ],
        )?;
        tx.commit()?;

        Ok(CloudFeatureClusterDto {
            id: cluster_id.clone(),
            title: clean_title.to_string(),
            summary: clean_summary.to_string(),
            ticket_count: count,
            ticket_ids: linked_ids,
            github_issue_url: Some(gh_url),
            status: status.to_string(),
            created_at,
            updated_at: now_ms,
        })
    })
    .map_err(|e| format!("upsert cluster failed: {e}"))?;

    Ok((cluster, audit_id))
}

/// `POST /v1/admin/clusters`
pub async fn handle_upsert_cluster(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[
            CloudAdminRole::SuperAdmin,
            CloudAdminRole::SupportAgent,
            CloudAdminRole::SkillCurator,
        ],
    ) {
        return map_auth_err(err, request_id);
    }
    if body.len() > MAX_ADMIN_JSON_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Cluster payload exceeds limit",
            Some("body"),
            request_id,
        );
    }
    let req: UpsertClusterRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid cluster payload: {e}"),
                Some("body"),
                request_id,
            );
        }
    };
    let trimmed_title = req.title.trim();
    if trimmed_title.len() < 3 || trimmed_title.len() > 160 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Cluster title must be 3..=160 characters",
            Some("title"),
            request_id,
        );
    }
    let status = req
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("open")
        .to_string();
    if !VALID_CLUSTER_STATUSES.contains(&status.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "status must be open, planned, shipped, or declined",
            Some("status"),
            request_id,
        );
    }

    let (clean_title, _) = scrub_server_pii(trimmed_title);
    let (clean_summary, _) = scrub_server_pii(req.summary.as_deref().unwrap_or(""));
    let pool = state.registry.pool().clone();

    let res = tokio::task::spawn_blocking(move || {
        upsert_cluster_db(&pool, &claims, &req, &clean_title, &clean_summary, &status)
    })
    .await;

    match res {
        Ok(Ok((cluster, audit_log_id))) => json_ok(
            StatusCode::CREATED,
            UpsertClusterPayload {
                cluster,
                audit_log_id,
            },
            request_id,
        ),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Upsert cluster worker panicked",
            None,
            request_id,
        ),
    }
}

fn map_to_primary_channel(channel: &str, host_client: &str) -> &'static str {
    match channel.trim() {
        "cli" => "cli",
        "desktop" => "desktop",
        "docker" => "docker",
        "plugin" | "claude_code" | "codex" | "antigravity" | "cursor" | "cowok_ai" => "plugin",
        _ => {
            if PLUGIN_HOSTS.contains(&host_client.trim()) {
                "plugin"
            } else {
                "cli"
            }
        }
    }
}

fn round_pct(num: usize, den: usize) -> f64 {
    if den == 0 {
        return 0.0;
    }
    ((num as f64 / den as f64) * 1000.0).round() / 10.0
}

fn compute_analytics_funnel_db(pool: &SqlitePool) -> Result<AnalyticsFunnelPayload, String> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v29_tables(&conn)?;

    let now_ms = Utc::now().timestamp_millis();
    let day_cutoff = now_ms - MS_PER_DAY;
    let week_cutoff = now_ms - 7 * MS_PER_DAY;
    let month_cutoff = now_ms - 30 * MS_PER_DAY;

    let mut dau_set = HashSet::<String>::new();
    let mut wau_set = HashSet::<String>::new();
    let mut mau_set = HashSet::<String>::new();

    let mut channel_users: HashMap<&str, HashSet<String>> = HashMap::new();
    let mut channel_events: HashMap<&str, usize> = HashMap::new();
    for ch in PRIMARY_CHANNELS {
        channel_users.insert(ch, HashSet::new());
        channel_events.insert(ch, 0);
    }

    let mut host_users: HashMap<&str, HashSet<String>> = HashMap::new();
    let mut host_events: HashMap<&str, usize> = HashMap::new();
    for h in PLUGIN_HOSTS {
        host_users.insert(h, HashSet::new());
        host_events.insert(h, 0);
    }

    let mut stage_users: HashMap<&str, HashSet<String>> = HashMap::new();
    for (stage_key, _) in FUNNEL_STAGES {
        stage_users.insert(stage_key, HashSet::new());
    }

    let mut adoption_counts: BTreeMap<String, (usize, HashSet<String>)> = BTreeMap::new();

    let mut stmt = conn
        .prepare(
            "SELECT install_id, event_name, funnel_stage, channel, host_client, occurred_at
             FROM cloud_telemetry_events",
        )
        .map_err(|e| format!("prepare telemetry analytics: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| format!("query telemetry analytics: {e}"))?;

    for row_res in rows {
        let (install_id, event_name, funnel_stage, channel, host_client, occurred_at) =
            row_res.map_err(|e| format!("telemetry row: {e}"))?;

        if occurred_at >= day_cutoff && occurred_at <= now_ms {
            dau_set.insert(install_id.clone());
        }
        if occurred_at >= week_cutoff && occurred_at <= now_ms {
            wau_set.insert(install_id.clone());
        }
        if occurred_at >= month_cutoff && occurred_at <= now_ms {
            mau_set.insert(install_id.clone());
        }

        let primary_ch = map_to_primary_channel(&channel, &host_client);
        if let Some(set) = channel_users.get_mut(primary_ch) {
            set.insert(install_id.clone());
        }
        *channel_events.entry(primary_ch).or_insert(0) += 1;

        let effective_host = if PLUGIN_HOSTS.contains(&host_client.as_str()) {
            Some(host_client.as_str())
        } else if PLUGIN_HOSTS.contains(&channel.as_str()) {
            Some(channel.as_str())
        } else {
            None
        };
        if let Some(host_key) = effective_host {
            if let Some(set) = host_users.get_mut(host_key) {
                set.insert(install_id.clone());
            }
            if let Some(cnt) = host_events.get_mut(host_key) {
                *cnt += 1;
            }
        }

        let resolved_stage = normalize_funnel_stage(funnel_stage.as_deref(), &event_name);
        if let Some(stage_str) = resolved_stage {
            if let Some(set) = stage_users.get_mut(stage_str.as_str()) {
                set.insert(install_id.clone());
            }
        }

        let entry = adoption_counts
            .entry(event_name)
            .or_insert_with(|| (0, HashSet::new()));
        entry.0 += 1;
        entry.1.insert(install_id);
    }

    let channel_breakdown = PRIMARY_CHANNELS
        .iter()
        .map(|ch| BreakdownMetricDto {
            key: (*ch).to_string(),
            users: channel_users.get(ch).map(HashSet::len).unwrap_or(0),
            events: *channel_events.get(ch).unwrap_or(&0),
        })
        .collect();

    let host_breakdown = PLUGIN_HOSTS
        .iter()
        .map(|h| BreakdownMetricDto {
            key: (*h).to_string(),
            users: host_users.get(h).map(HashSet::len).unwrap_or(0),
            events: *host_events.get(h).unwrap_or(&0),
        })
        .collect();

    let day_0_cohort = stage_users.get("day_0").cloned().unwrap_or_default();
    let day_0_count = day_0_cohort.len();
    let mut active_cohort = day_0_cohort;
    let mut prev_users = day_0_count;
    let mut funnel_steps = Vec::with_capacity(FUNNEL_STAGES.len());
    for (idx, (stage_key, label)) in FUNNEL_STAGES.iter().enumerate() {
        if idx > 0 {
            let step_set = stage_users.get(stage_key);
            active_cohort = match step_set {
                Some(set) => active_cohort.intersection(set).cloned().collect(),
                None => HashSet::new(),
            };
        }
        let users = active_cohort.len();
        let (drop_off_count, drop_off_rate_pct) = if idx == 0 {
            (0, 0.0)
        } else {
            let dropped = prev_users.saturating_sub(users);
            (dropped, round_pct(dropped, prev_users).clamp(0.0, 100.0))
        };
        let conversion_rate_pct = if day_0_count == 0 {
            0.0
        } else if idx == 0 {
            100.0
        } else {
            round_pct(users, day_0_count).clamp(0.0, 100.0)
        };
        funnel_steps.push(FunnelStepMetricDto {
            stage: (*stage_key).to_string(),
            label: (*label).to_string(),
            users,
            conversion_rate_pct,
            drop_off_count,
            drop_off_rate_pct,
        });
        prev_users = users;
    }

    let mut feature_adoption: Vec<FeatureAdoptionMetricDto> = adoption_counts
        .into_iter()
        .map(
            |(event_name, (total_events, set))| FeatureAdoptionMetricDto {
                event_name,
                total_events,
                unique_installs: set.len(),
            },
        )
        .collect();
    feature_adoption.sort_by_key(|a| std::cmp::Reverse(a.total_events));
    feature_adoption.truncate(20);

    let open_tickets: usize = conn
        .query_row(
            "SELECT COUNT(1) FROM cloud_support_tickets WHERE status NOT IN ('resolved','closed')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;
    let p0_tickets: usize = conn
        .query_row(
            "SELECT COUNT(1) FROM cloud_support_tickets WHERE priority = 'P0' AND status NOT IN ('resolved','closed')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;
    let active_crash_groups: usize = conn
        .query_row(
            "SELECT COUNT(1) FROM cloud_crash_groups WHERE status != 'resolved'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    Ok(AnalyticsFunnelPayload {
        dau: dau_set.len(),
        wau: wau_set.len(),
        mau: mau_set.len(),
        open_tickets,
        p0_tickets,
        active_crash_groups,
        channel_breakdown,
        host_breakdown,
        funnel_steps,
        feature_adoption,
    })
}

/// `GET /v1/admin/analytics/funnel`
pub async fn handle_get_analytics_funnel(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[
            CloudAdminRole::SuperAdmin,
            CloudAdminRole::SupportAgent,
            CloudAdminRole::SkillCurator,
            CloudAdminRole::AnalystReadonly,
        ],
    ) {
        return map_auth_err(err, request_id);
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || compute_analytics_funnel_db(&pool)).await;
    match res {
        Ok(Ok(payload)) => json_ok(StatusCode::OK, payload, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Analytics funnel worker panicked",
            None,
            request_id,
        ),
    }
}

fn list_crashes_db(
    pool: &SqlitePool,
    query: &ListCrashesQuery,
) -> Result<Vec<CrashGroupDto>, String> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v29_tables(&conn)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200) as i64;

    let mut clauses: Vec<String> = Vec::new();
    let mut bind_values: Vec<SqlValue> = Vec::new();
    if let Some(ref ver) = query.app_version {
        let trimmed = ver.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            clauses.push("app_version = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    if let Some(ref st) = query.status {
        let trimmed = st.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            clauses.push("status = ?".to_string());
            bind_values.push(SqlValue::Text(trimmed.to_string()));
        }
    }
    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    let sql = format!(
        "SELECT fingerprint, app_version, error_signature, sample_stack_sanitized,
                occurrence_count, affected_installs, status, first_seen_at, last_seen_at
         FROM cloud_crash_groups{where_sql}
         ORDER BY occurrence_count DESC, last_seen_at DESC
         LIMIT ?"
    );
    bind_values.push(SqlValue::Integer(limit));

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare list crashes: {e}"))?;
    let rows = stmt
        .query_map(params_from_iter(bind_values.iter()), |r| {
            Ok(CrashGroupDto {
                fingerprint: r.get(0)?,
                app_version: r.get(1)?,
                error_signature: r.get(2)?,
                sample_stack_sanitized: r.get(3)?,
                occurrence_count: r.get(4)?,
                affected_installs: r.get(5)?,
                status: r.get(6)?,
                first_seen_at: r.get(7)?,
                last_seen_at: r.get(8)?,
            })
        })
        .map_err(|e| format!("query list crashes: {e}"))?;

    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| format!("crash row: {e}"))?);
    }
    Ok(out)
}

/// `GET /v1/admin/crashes`
pub async fn handle_list_crashes(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<ListCrashesQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[
            CloudAdminRole::SuperAdmin,
            CloudAdminRole::SupportAgent,
            CloudAdminRole::SkillCurator,
            CloudAdminRole::AnalystReadonly,
        ],
    ) {
        return map_auth_err(err, request_id);
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || list_crashes_db(&pool, &query)).await;
    match res {
        Ok(Ok(crashes)) => json_ok(StatusCode::OK, ListCrashesPayload { crashes }, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List crashes worker panicked",
            None,
            request_id,
        ),
    }
}

fn list_audit_logs_db(pool: &SqlitePool) -> Result<Vec<AdminAuditLogDto>, String> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v29_tables(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, actor_subject, actor_role, mfa_verified, action, target_resource, details_json, created_at
             FROM admin_audit_logs
             ORDER BY created_at DESC
             LIMIT 200",
        )
        .map_err(|e| format!("prepare audit logs: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let details_str: String = r.get(6)?;
            let details_val = serde_json::from_str(&details_str).unwrap_or_else(|_| json!({}));
            Ok(AdminAuditLogDto {
                id: r.get(0)?,
                actor_subject: r.get(1)?,
                actor_role: r.get(2)?,
                mfa_verified: r.get::<_, i64>(3)? == 1,
                action: r.get(4)?,
                target_resource: r.get(5)?,
                details: details_val,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| format!("query audit logs: {e}"))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| format!("audit row: {e}"))?);
    }
    Ok(out)
}

/// `GET /v1/admin/audit-logs` (`SUPER_ADMIN` only)
pub async fn handle_list_audit_logs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &[CloudAdminRole::SuperAdmin]) {
        return map_auth_err(err, request_id);
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || list_audit_logs_db(&pool)).await;
    match res {
        Ok(Ok(logs)) => json_ok(StatusCode::OK, ListAuditLogsPayload { logs }, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List audit logs worker panicked",
            None,
            request_id,
        ),
    }
}

/// Builds the isolated Cloud Control Plane router with a custom `IngestRateLimiter` (ideal for tests).
pub fn cloud_admin_router_with_limiter(limiter: Arc<IngestRateLimiter>) -> Router<DaemonAppState> {
    let ingest_tickets_route = Router::new()
        .route("/v1/ingest/tickets", post(handle_ingest_ticket))
        .layer(DefaultBodyLimit::max(INGEST_TICKET_ROUTE_LIMIT));

    Router::new()
        .merge(ingest_tickets_route)
        .route("/v1/ingest/telemetry", post(handle_ingest_telemetry))
        .route("/v1/admin/auth/session", post(handle_admin_auth_session))
        .route("/v1/admin/tickets", get(handle_list_admin_tickets))
        .route(
            "/v1/admin/tickets/:id/attachment",
            get(handle_get_ticket_attachment),
        )
        .route(
            "/v1/admin/tickets/:id/reply",
            post(handle_reply_admin_ticket),
        )
        .route(
            "/v1/admin/clusters",
            get(handle_list_clusters).post(handle_upsert_cluster),
        )
        .route(
            "/v1/admin/analytics/funnel",
            get(handle_get_analytics_funnel),
        )
        .route("/v1/admin/crashes", get(handle_list_crashes))
        .route("/v1/admin/audit-logs", get(handle_list_audit_logs))
        .layer(Extension(limiter))
}

/// Canonical Cloud Control Plane router (`/v1/ingest/*` and `/v1/admin/*`).
pub fn cloud_admin_router() -> Router<DaemonAppState> {
    cloud_admin_router_with_limiter(Arc::new(IngestRateLimiter::from_env()))
}

/// Alias matching daemon `build_*_router()` naming convention.
pub fn build_cloud_admin_router() -> Router<DaemonAppState> {
    cloud_admin_router()
}
