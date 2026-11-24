// @story 14.1
// @implements AC-02, AC-03, AC-04 (Zero-PII Diagnostic Scrubber, SQLite FIFO-500 Telemetry Outbox, Async Reqwest Flusher & Non-Tech Local Support Tickets)
use std::collections::BTreeSet;
use std::fmt::{self, Write as _};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use chrono::Utc;
use domain::HarnessError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{retry_on_busy, SqlitePool};
use uuid::Uuid;

pub const TELEMETRY_OUTBOX_FIFO_CAP: usize = 500;
pub const BACKOFF_SCHEDULE_SECS: [i64; 5] = [30, 60, 120, 300, 900];
pub const TELEMETRY_FLUSH_INTERVAL_SECS: u64 = 45;
pub const TELEMETRY_STALE_FLUSHING_MS: i64 = 5 * 60 * 1000;
pub const TELEMETRY_FLUSH_BATCH_SIZE: usize = 50;
pub const TELEMETRY_HTTP_TIMEOUT_SECS: u64 = 10;
pub const MAX_TICKET_CODE_RETRIES: usize = 5;
pub const REDACTED_SECRET: &str = "[REDACTED_SECRET]";
pub const REDACTED_HOME: &str = "<HOME>";
pub const DEFAULT_PREFERENCE_ROW_ID: &str = "default";
pub const MAX_TICKET_TITLE_LEN: usize = 160;
pub const MIN_TICKET_TITLE_LEN: usize = 3;
pub const MAX_TICKET_DESC_LEN: usize = 4000;
pub const MIN_TICKET_DESC_LEN: usize = 5;
pub const MIN_LOCALE_LEN: usize = 2;
pub const MAX_LOCALE_LEN: usize = 16;
pub const MAX_SCREENSHOT_DATA_URL_LEN: usize = 7_000_000;
pub const MAX_ATTACHMENT_BYTES: usize = 5_000_000;
pub const MAX_RECENT_ERROR_CODES: usize = 20;
pub const MAX_ACTIVE_SKILLS: usize = 30;
pub const MAX_DIAG_TOKEN_LEN: usize = 64;

const SUPPORT_ATTACHMENTS_DIR: &str = "support-attachments";
const ATTACHMENT_MEDIA_SPECS: [(&str, &str, &str); 5] = [
    ("data:image/png;base64,", "png", "image/png"),
    ("data:image/jpeg;base64,", "jpg", "image/jpeg"),
    ("data:image/webp;base64,", "webp", "image/webp"),
    ("data:video/mp4;base64,", "mp4", "video/mp4"),
    ("data:video/webm;base64,", "webm", "video/webm"),
];

pub const VALID_CHANNELS: [&str; 8] = [
    "cli",
    "desktop",
    "claude_code",
    "codex",
    "antigravity",
    "cursor",
    "cowok_ai",
    "docker",
];

pub const VALID_EVENT_GROUPS: [&str; 5] = [
    "lifecycle",
    "onboarding_funnel",
    "feature_usage",
    "reliability_errors",
    "skill_ecosystem",
];

pub const VALID_CATEGORIES: [&str; 4] = [
    "bug_report",
    "feature_request",
    "skill_template_request",
    "question",
];

/// Normalized lowercase alphanumeric-only keys (underscores/hyphens stripped) that are stripped from telemetry payloads.
const FORBIDDEN_NORMALIZED_KEYS: [&str; 19] = [
    "sourcecode",
    "prompt",
    "userprompt",
    "systemprompt",
    "chatprompt",
    "chathistory",
    "rawhtml",
    "dotenv",
    "env",
    "apikey",
    "xapikey",
    "accesstoken",
    "refreshtoken",
    "secret",
    "password",
    "authorization",
    "cookie",
    "setcookie",
    "token",
];

const ENSURE_V23_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS telemetry_outbox (
    id TEXT PRIMARY KEY NOT NULL,
    install_id TEXT NOT NULL,
    channel TEXT NOT NULL CHECK (
        channel IN ('cli','desktop','claude_code','codex','antigravity','cursor','cowok_ai','docker')
    ),
    event_group TEXT NOT NULL CHECK (
        event_group IN ('lifecycle','onboarding_funnel','feature_usage','reliability_errors','skill_ecosystem')
    ),
    event_name TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','flushing','sent','failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_retry_at BIGINT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_telemetry_outbox_status_retry
    ON telemetry_outbox(status, next_retry_at);
CREATE INDEX IF NOT EXISTS idx_telemetry_outbox_created_at
    ON telemetry_outbox(created_at);
CREATE TABLE IF NOT EXISTS local_support_tickets (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_code TEXT NOT NULL UNIQUE,
    install_id TEXT NOT NULL,
    channel TEXT NOT NULL CHECK (
        channel IN ('cli','desktop','claude_code','codex','antigravity','cursor','cowok_ai','docker')
    ),
    category TEXT NOT NULL CHECK (
        category IN ('bug_report','feature_request','skill_template_request','question')
    ),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    contact_email TEXT,
    screenshot_sha256 TEXT,
    diagnostics_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'submitted' CHECK (
        status IN ('submitted','triaged','in_progress','resolved','closed')
    ),
    admin_replies_json TEXT NOT NULL DEFAULT '[]',
    sync_status TEXT NOT NULL DEFAULT 'queued' CHECK (
        sync_status IN ('queued','synced','local_only','failed')
    ),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_local_support_tickets_status
    ON local_support_tickets(status, updated_at);
CREATE TABLE IF NOT EXISTS telemetry_preferences (
    id TEXT PRIMARY KEY NOT NULL,
    install_id TEXT NOT NULL,
    telemetry_enabled INTEGER NOT NULL DEFAULT 1 CHECK (telemetry_enabled IN (0, 1)),
    updated_at BIGINT NOT NULL
);
"#;

/// Typed error separating client input validation (422), oversized payloads (413),
/// and internal SQLite/storage failures (500 STORAGE_ERROR with sanitized output).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportEdgeError {
    Validation {
        code: &'static str,
        field: Option<&'static str>,
        message: String,
    },
    PayloadTooLarge {
        field: Option<&'static str>,
        message: String,
    },
    Storage {
        context: &'static str,
        detail: String,
    },
}

impl fmt::Display for SupportEdgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { message, .. } => write!(f, "{message}"),
            Self::PayloadTooLarge { message, .. } => write!(f, "{message}"),
            Self::Storage { context, .. } => {
                write!(f, "Local support/telemetry storage error ({context})")
            }
        }
    }
}

impl std::error::Error for SupportEdgeError {}

fn validation_err(field: Option<&'static str>, message: impl Into<String>) -> SupportEdgeError {
    SupportEdgeError::Validation {
        code: "INVALID_REQUEST",
        field,
        message: message.into(),
    }
}

fn storage_err(context: &'static str, err: impl fmt::Display) -> SupportEdgeError {
    let scrubbed = scrub_zero_pii(&err.to_string()).scrubbed;
    SupportEdgeError::Storage {
        context,
        detail: scrubbed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrubResult {
    pub scrubbed: String,
    pub redaction_count: usize,
    pub redacted_categories: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEnvOverride {
    pub telemetry_disabled_by_env: bool,
    pub airgapped: bool,
}

impl TelemetryEnvOverride {
    pub fn from_process_env() -> Self {
        let telemetry_val = std::env::var("COWOK_TELEMETRY").ok();
        let airgapped_val = std::env::var("COWOK_AIRGAPPED").ok();
        Self::from_values(telemetry_val.as_deref(), airgapped_val.as_deref())
    }

    pub fn from_values(telemetry_env: Option<&str>, airgapped_env: Option<&str>) -> Self {
        let telemetry_disabled_by_env = telemetry_env
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "0" | "false" | "off"
                )
            })
            .unwrap_or(false);
        let airgapped = airgapped_env
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        Self {
            telemetry_disabled_by_env,
            airgapped,
        }
    }

    pub fn is_outbound_blocked(&self) -> bool {
        self.telemetry_disabled_by_env || self.airgapped
    }

    pub fn locked_reason(&self) -> Option<String> {
        if self.airgapped {
            Some("COWOK_AIRGAPPED=1".to_string())
        } else if self.telemetry_disabled_by_env {
            Some("COWOK_TELEMETRY=0".to_string())
        } else {
            None
        }
    }
}

/// Strictly allowlisted diagnostic bundle (AC-02: never attaches source code or chat prompts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundle {
    pub app_version: String,
    pub channel: String,
    pub os: String,
    pub locale: String,
    pub recent_error_codes: Vec<String>,
    pub active_skills: Vec<String>,
    pub redaction_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminReplyDto {
    pub id: String,
    pub author: String,
    pub message: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSupportTicketDto {
    pub id: String,
    pub ticket_code: String,
    pub install_id: String,
    pub channel: String,
    pub category: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_mime: Option<String>,
    pub diagnostics: DiagnosticBundle,
    pub status: String,
    pub admin_replies: Vec<AdminReplyDto>,
    pub sync_status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupportTicketInput {
    pub category: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub contact_email: Option<String>,
    #[serde(default)]
    pub screenshot_data_url: Option<String>,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_true")]
    pub include_diagnostics: bool,
    #[serde(default)]
    pub recent_error_codes: Vec<String>,
    #[serde(default)]
    pub active_skills: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueTelemetryInput {
    pub channel: String,
    pub event_group: String,
    pub event_name: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueTelemetryResult {
    pub queued: bool,
    pub dropped_by_opt_out: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    pub outbox_depth: usize,
    pub redaction_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryPreferencesDto {
    pub telemetry_enabled: bool,
    pub effective_enabled: bool,
    pub env_locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_reason: Option<String>,
    pub airgapped: bool,
    pub install_id: String,
    pub pending_outbox_count: usize,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryOutboxItem {
    pub id: String,
    pub install_id: String,
    pub channel: String,
    pub event_group: String,
    pub event_name: String,
    pub payload_json: String,
    pub status: String,
    pub attempt_count: u32,
    pub next_retry_at: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlushOutcome {
    pub attempted: usize,
    pub sent: usize,
    pub failed: usize,
    pub skipped_by_opt_out: bool,
    pub http_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportSendOutcome {
    Delivered,
    RetryableFailure(String),
}

pub trait TelemetryBatchTransport {
    fn send_batch(&self, endpoint: &str, items: &[TelemetryOutboxItem]) -> TransportSendOutcome;
}

/// Default no-op local transport used when no remote telemetry collector endpoint is configured.
pub struct LocalNoopTelemetryTransport;

impl TelemetryBatchTransport for LocalNoopTelemetryTransport {
    fn send_batch(&self, _endpoint: &str, _items: &[TelemetryOutboxItem]) -> TransportSendOutcome {
        TransportSendOutcome::RetryableFailure("TELEMETRY_COLLECTOR_UNREACHABLE".to_string())
    }
}

/// Async `reqwest` HTTP transport for flushing scrubbed telemetry batches when opted-in,
/// non-air-gapped, and `COWOK_TELEMETRY_ENDPOINT` is explicitly configured.
#[derive(Clone)]
pub struct HttpTelemetryTransport {
    client: reqwest::Client,
}

impl HttpTelemetryTransport {
    pub fn new() -> Result<Self, SupportEdgeError> {
        Self::with_timeout(Duration::from_secs(TELEMETRY_HTTP_TIMEOUT_SECS))
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, SupportEdgeError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|e| storage_err("telemetry_http_client_init", e))?;
        Ok(Self { client })
    }

    pub async fn send_batch_async(
        &self,
        endpoint: &str,
        items: &[TelemetryOutboxItem],
    ) -> TransportSendOutcome {
        let trimmed = endpoint.trim();
        let allowed_scheme = trimmed.starts_with("https://")
            || trimmed.starts_with("http://127.0.0.1:")
            || trimmed.starts_with("http://localhost:");
        if !allowed_scheme {
            return TransportSendOutcome::RetryableFailure(
                "INVALID_TELEMETRY_ENDPOINT_SCHEME".to_string(),
            );
        }
        let body = serde_json::json!({
            "schemaVersion": "1.0",
            "events": items,
        });
        match self.client.post(trimmed).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => TransportSendOutcome::Delivered,
            Ok(resp) => TransportSendOutcome::RetryableFailure(format!(
                "HTTP {} upstream collector error",
                resp.status().as_u16()
            )),
            Err(_) => {
                TransportSendOutcome::RetryableFailure("TELEMETRY_COLLECTOR_UNREACHABLE".into())
            }
        }
    }
}

/// Convenience wrapper for flushing due telemetry batches using process env overrides and current UTC time.
pub fn flush_telemetry_batch<T: TelemetryBatchTransport>(
    pool: &SqlitePool,
    endpoint_url: Option<&str>,
    max_batch: usize,
    transport: &T,
) -> Result<FlushOutcome, SupportEdgeError> {
    flush_telemetry_batch_with_transport(
        pool,
        &TelemetryEnvOverride::from_process_env(),
        endpoint_url,
        max_batch,
        Utc::now().timestamp_millis(),
        transport,
    )
}

fn default_channel() -> String {
    "desktop".to_string()
}

fn default_locale() -> String {
    "vi-VN".to_string()
}

fn default_true() -> bool {
    true
}

fn is_secret_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'+' | b'/' | b'=')
}

fn is_jwt_segment_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
}

fn is_path_segment_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
}

fn is_boundary_before(bytes: &[u8], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    !bytes[idx - 1].is_ascii_alphanumeric()
}

fn match_ascii_ci(bytes: &[u8], idx: usize, pat: &[u8]) -> bool {
    if idx + pat.len() > bytes.len() {
        return false;
    }
    bytes[idx..idx + pat.len()].eq_ignore_ascii_case(pat)
}

fn try_match_cookie_header(
    bytes: &[u8],
    idx: usize,
) -> Option<(usize, &'static str, &'static str)> {
    if !is_boundary_before(bytes, idx) {
        return None;
    }
    let prefix = if match_ascii_ci(bytes, idx, b"set-cookie:") {
        "Set-Cookie: "
    } else if match_ascii_ci(bytes, idx, b"cookie:") {
        "Cookie: "
    } else {
        return None;
    };
    let mut end = idx + prefix.trim_end().len();
    while end < bytes.len() && bytes[end] != b'\n' && bytes[end] != b'\r' {
        end += 1;
    }
    if end <= idx + prefix.trim_end().len() {
        return None;
    }
    Some((end, prefix, "cookie_header"))
}

fn try_match_bearer(bytes: &[u8], idx: usize) -> Option<usize> {
    if !is_boundary_before(bytes, idx) || !match_ascii_ci(bytes, idx, b"bearer") {
        return None;
    }
    let mut pos = idx + 6;
    if pos >= bytes.len() || !matches!(bytes[pos], b' ' | b'\t') {
        return None;
    }
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
        pos += 1;
    }
    let start_tok = pos;
    while pos < bytes.len() && is_secret_token_char(bytes[pos]) {
        pos += 1;
    }
    if pos - start_tok >= 8 {
        Some(pos)
    } else {
        None
    }
}

fn try_match_jwt(bytes: &[u8], idx: usize) -> Option<usize> {
    if !is_boundary_before(bytes, idx) || idx + 3 >= bytes.len() || &bytes[idx..idx + 3] != b"eyJ" {
        return None;
    }
    let mut pos = idx;
    let mut seg1 = 0usize;
    while pos < bytes.len() && is_jwt_segment_char(bytes[pos]) {
        pos += 1;
        seg1 += 1;
    }
    if seg1 < 6 || pos >= bytes.len() || bytes[pos] != b'.' {
        return None;
    }
    pos += 1;
    if pos + 3 > bytes.len() || &bytes[pos..pos + 3] != b"eyJ" {
        return None;
    }
    let mut seg2 = 0usize;
    while pos < bytes.len() && is_jwt_segment_char(bytes[pos]) {
        pos += 1;
        seg2 += 1;
    }
    if seg2 < 6 || pos >= bytes.len() || bytes[pos] != b'.' {
        return None;
    }
    pos += 1;
    let mut seg3 = 0usize;
    while pos < bytes.len() && is_jwt_segment_char(bytes[pos]) {
        pos += 1;
        seg3 += 1;
    }
    if seg3 >= 6 {
        Some(pos)
    } else {
        None
    }
}

fn try_match_api_key(bytes: &[u8], idx: usize) -> Option<(usize, &'static str)> {
    if !is_boundary_before(bytes, idx) {
        return None;
    }
    let tail = &bytes[idx..];
    let (prefix_len, min_body, cat) = if tail.starts_with(b"sk-") {
        (3, 8, "api_key")
    } else if tail.starts_with(b"AIza") {
        (4, 16, "api_key")
    } else if tail.starts_with(b"ghp_")
        || tail.starts_with(b"gho_")
        || tail.starts_with(b"ghu_")
        || tail.starts_with(b"ghs_")
        || tail.starts_with(b"ghr_")
    {
        (4, 12, "api_key")
    } else if tail.len() >= 5
        && &tail[..3] == b"xox"
        && matches!(tail[3], b'b' | b'a' | b'p' | b'r' | b's')
        && tail[4] == b'-'
    {
        (5, 10, "api_key")
    } else if tail.starts_with(b"AKIA") {
        (4, 16, "api_key")
    } else {
        return None;
    };

    let mut pos = idx + prefix_len;
    while pos < bytes.len()
        && (bytes[pos].is_ascii_alphanumeric() || matches!(bytes[pos], b'_' | b'-'))
    {
        pos += 1;
    }
    if pos - (idx + prefix_len) >= min_body {
        Some((pos, cat))
    } else {
        None
    }
}

fn try_match_home_path(bytes: &[u8], idx: usize) -> Option<usize> {
    let tail = &bytes[idx..];
    let prefix_len = if match_ascii_ci(bytes, idx, b"/users/") {
        7
    } else if match_ascii_ci(bytes, idx, b"/home/") {
        6
    } else if is_boundary_before(bytes, idx)
        && tail.len() >= 9
        && tail[0].is_ascii_alphabetic()
        && tail[1] == b':'
        && matches!(tail[2], b'\\' | b'/')
        && tail[3..8].eq_ignore_ascii_case(b"users")
        && matches!(tail[8], b'\\' | b'/')
    {
        9
    } else {
        return None;
    };

    let mut pos = idx + prefix_len;
    let user_start = pos;
    while pos < bytes.len() && is_path_segment_char(bytes[pos]) {
        pos += 1;
    }
    if pos > user_start {
        Some(pos)
    } else {
        None
    }
}

/// Deterministic Zero-PII scrubber (AC-02): removes API keys (`sk-*`, `AIza*`, `ghp_*`, `gho_*`,
/// `xox[baprs]-*`, `AKIA*`), `Bearer` tokens (spaces/tabs), `Cookie`/`Set-Cookie` headers,
/// 3-segment JWTs, and OS home paths (POSIX + Windows backslash & forward-slash, case-insensitive).
pub fn scrub_zero_pii(input: &str) -> ScrubResult {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut redaction_count = 0usize;
    let mut categories = BTreeSet::new();
    let mut idx = 0usize;

    while idx < bytes.len() {
        if let Some((end, prefix, cat)) = try_match_cookie_header(bytes, idx) {
            out.push_str(prefix);
            out.push_str(REDACTED_SECRET);
            redaction_count += 1;
            categories.insert(cat.to_string());
            idx = end;
            continue;
        }
        if let Some(end) = try_match_bearer(bytes, idx) {
            out.push_str("Bearer ");
            out.push_str(REDACTED_SECRET);
            redaction_count += 1;
            categories.insert("bearer_token".to_string());
            idx = end;
            continue;
        }
        if let Some(end) = try_match_jwt(bytes, idx) {
            out.push_str(REDACTED_SECRET);
            redaction_count += 1;
            categories.insert("jwt".to_string());
            idx = end;
            continue;
        }
        if let Some((end, cat)) = try_match_api_key(bytes, idx) {
            out.push_str(REDACTED_SECRET);
            redaction_count += 1;
            categories.insert(cat.to_string());
            idx = end;
            continue;
        }
        if let Some(end) = try_match_home_path(bytes, idx) {
            out.push_str(REDACTED_HOME);
            redaction_count += 1;
            categories.insert("home_path".to_string());
            idx = end;
            continue;
        }
        let ch = input[idx..].chars().next().unwrap_or('\0');
        out.push(ch);
        idx += ch.len_utf8();
    }

    ScrubResult {
        scrubbed: out,
        redaction_count,
        redacted_categories: categories.into_iter().collect(),
    }
}

fn normalize_payload_key(key: &str) -> String {
    key.chars()
        .filter(|c| *c != '_' && *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Recursively scrubs JSON payloads (both object keys and values) and strips forbidden
/// source-code, prompt, raw HTML, and secret fields.
pub fn scrub_json_value(value: &serde_json::Value) -> (serde_json::Value, usize) {
    match value {
        serde_json::Value::String(s) => {
            let res = scrub_zero_pii(s);
            (serde_json::Value::String(res.scrubbed), res.redaction_count)
        }
        serde_json::Value::Array(items) => {
            let mut total = 0usize;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let (v, c) = scrub_json_value(item);
                total += c;
                out.push(v);
            }
            (serde_json::Value::Array(out), total)
        }
        serde_json::Value::Object(map) => {
            let mut total = 0usize;
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let normalized = normalize_payload_key(k);
                if FORBIDDEN_NORMALIZED_KEYS.contains(&normalized.as_str()) {
                    total += 1;
                    continue;
                }
                let clean_key = scrub_zero_pii(k);
                total += clean_key.redaction_count;
                let (clean_v, c) = scrub_json_value(v);
                total += c;
                out.insert(clean_key.scrubbed, clean_v);
            }
            (serde_json::Value::Object(out), total)
        }
        other => (other.clone(), 0),
    }
}

pub fn build_scrubbed_diagnostics(
    channel: &str,
    locale: &str,
    include_diagnostics: bool,
    recent_error_codes: &[String],
    active_skills: &[String],
) -> DiagnosticBundle {
    let mut redaction_count = 0usize;
    let clean_channel = scrub_zero_pii(channel);
    redaction_count += clean_channel.redaction_count;
    let clean_locale = scrub_zero_pii(locale);
    redaction_count += clean_locale.redaction_count;
    let capped_locale: String = clean_locale.scrubbed.chars().take(MAX_LOCALE_LEN).collect();

    if !include_diagnostics {
        return DiagnosticBundle {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            channel: clean_channel.scrubbed,
            os: std::env::consts::OS.to_string(),
            locale: capped_locale,
            recent_error_codes: Vec::new(),
            active_skills: Vec::new(),
            redaction_count,
        };
    }

    let mut clean_errors = Vec::new();
    for raw in recent_error_codes.iter().take(MAX_RECENT_ERROR_CODES) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let s = scrub_zero_pii(trimmed);
        redaction_count += s.redaction_count;
        clean_errors.push(s.scrubbed.chars().take(MAX_DIAG_TOKEN_LEN).collect());
    }

    let mut clean_skills = Vec::new();
    for raw in active_skills.iter().take(MAX_ACTIVE_SKILLS) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let s = scrub_zero_pii(trimmed);
        redaction_count += s.redaction_count;
        clean_skills.push(s.scrubbed.chars().take(MAX_DIAG_TOKEN_LEN).collect());
    }

    DiagnosticBundle {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        channel: clean_channel.scrubbed,
        os: std::env::consts::OS.to_string(),
        locale: capped_locale,
        recent_error_codes: clean_errors,
        active_skills: clean_skills,
        redaction_count,
    }
}

pub fn compute_backoff_secs(attempt_count: u32) -> i64 {
    if attempt_count == 0 {
        return BACKOFF_SCHEDULE_SECS[0];
    }
    let idx = (attempt_count as usize)
        .saturating_sub(1)
        .min(BACKOFF_SCHEDULE_SECS.len() - 1);
    BACKOFF_SCHEDULE_SECS[idx]
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn generate_ticket_code() -> String {
    let raw = Uuid::new_v4().simple().to_string().to_ascii_uppercase();
    format!("CWK-{}", &raw[..8])
}

fn generate_install_id() -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("inst_{}", &raw[..16])
}

fn validate_channel(channel: &str) -> Result<&str, SupportEdgeError> {
    let trimmed = channel.trim();
    if VALID_CHANNELS.contains(&trimmed) {
        Ok(trimmed)
    } else {
        Err(validation_err(
            Some("channel"),
            format!(
                "INVALID_CHANNEL: '{}' must be one of {}",
                trimmed,
                VALID_CHANNELS.join(", ")
            ),
        ))
    }
}

fn validate_event_group(event_group: &str) -> Result<&str, SupportEdgeError> {
    let trimmed = event_group.trim();
    if VALID_EVENT_GROUPS.contains(&trimmed) {
        Ok(trimmed)
    } else {
        Err(validation_err(
            Some("eventGroup"),
            format!(
                "INVALID_EVENT_GROUP: '{}' must be one of {}",
                trimmed,
                VALID_EVENT_GROUPS.join(", ")
            ),
        ))
    }
}

fn validate_category(category: &str) -> Result<&str, SupportEdgeError> {
    let trimmed = category.trim();
    if VALID_CATEGORIES.contains(&trimmed) {
        Ok(trimmed)
    } else {
        Err(validation_err(
            Some("category"),
            format!(
                "INVALID_CATEGORY: '{}' must be one of {}",
                trimmed,
                VALID_CATEGORIES.join(", ")
            ),
        ))
    }
}

fn validate_locale(locale: &str) -> Result<String, SupportEdgeError> {
    let trimmed = locale.trim();
    let char_len = trimmed.chars().count();
    if !(MIN_LOCALE_LEN..=MAX_LOCALE_LEN).contains(&char_len)
        || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(validation_err(
            Some("locale"),
            format!(
                "INVALID_LOCALE: locale must be {MIN_LOCALE_LEN}..={MAX_LOCALE_LEN} non-whitespace characters"
            ),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_optional_email(email: Option<&str>) -> Result<Option<String>, SupportEdgeError> {
    let Some(raw) = email else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 254
        || trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
        || !trimmed.contains('@')
    {
        return Err(validation_err(
            Some("contactEmail"),
            "INVALID_EMAIL: contactEmail must be a valid email address",
        ));
    }
    let parts: Vec<&str> = trimmed.split('@').collect();
    if parts.len() != 2
        || parts[0].is_empty()
        || parts[0].contains("..")
        || !parts[1].contains('.')
        || parts[1].starts_with('.')
        || parts[1].ends_with('.')
        || parts[1].contains("..")
        || parts[1].split('.').any(|seg| seg.is_empty())
    {
        return Err(validation_err(
            Some("contactEmail"),
            "INVALID_EMAIL: contactEmail domain is invalid",
        ));
    }
    Ok(Some(trimmed.to_string()))
}

struct ValidatedAttachment {
    sha256: String,
    ext: &'static str,
    mime: &'static str,
    bytes: Vec<u8>,
}

fn is_strict_sha256_hex(val: &str) -> bool {
    val.len() == 64 && val.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_screenshot_data_url(
    screenshot: Option<&str>,
) -> Result<Option<ValidatedAttachment>, SupportEdgeError> {
    let Some(raw) = screenshot.map(str::trim) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.len() > MAX_SCREENSHOT_DATA_URL_LEN {
        return Err(SupportEdgeError::PayloadTooLarge {
            field: Some("screenshotDataUrl"),
            message: "SCREENSHOT_TOO_LARGE: attachment exceeds 5 MB limit".to_string(),
        });
    }
    let matched = ATTACHMENT_MEDIA_SPECS
        .iter()
        .find_map(|(prefix, ext, mime)| raw.strip_prefix(prefix).map(|p| (*ext, *mime, p)));
    let Some((ext, mime, payload)) = matched else {
        return Err(validation_err(
            Some("screenshotDataUrl"),
            "INVALID_SCREENSHOT: screenshotDataUrl must be a base64 PNG, JPEG, WebP, MP4, or WebM data URL",
        ));
    };
    if payload.is_empty()
        || !payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
    {
        return Err(validation_err(
            Some("screenshotDataUrl"),
            "INVALID_SCREENSHOT: screenshotDataUrl base64 payload is invalid",
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| {
            validation_err(
                Some("screenshotDataUrl"),
                "INVALID_SCREENSHOT: screenshotDataUrl base64 payload is invalid",
            )
        })?;
    if bytes.is_empty() {
        return Err(validation_err(
            Some("screenshotDataUrl"),
            "INVALID_SCREENSHOT: screenshotDataUrl decoded payload is empty",
        ));
    }
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(SupportEdgeError::PayloadTooLarge {
            field: Some("screenshotDataUrl"),
            message: "SCREENSHOT_TOO_LARGE: attachment exceeds 5 MB limit".to_string(),
        });
    }
    let sha256 = sha256_hex(&bytes);
    Ok(Some(ValidatedAttachment {
        sha256,
        ext,
        mime,
        bytes,
    }))
}

fn ensure_safe_dir(path: &Path, enforce_0700: bool) -> Result<(), SupportEdgeError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                return Err(storage_err(
                    "support_attachment_dir_unsafe",
                    "Refusing to use symlink or non-directory attachment path",
                ));
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            if let Err(create_err) = fs::create_dir(path) {
                if create_err.kind() != ErrorKind::AlreadyExists {
                    return Err(storage_err("support_attachment_dir_create", create_err));
                }
                let meta = fs::symlink_metadata(path)
                    .map_err(|e| storage_err("support_attachment_dir_stat", e))?;
                if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                    return Err(storage_err(
                        "support_attachment_dir_unsafe",
                        "Refusing to use symlink or non-directory attachment path",
                    ));
                }
            }
        }
        Err(err) => return Err(storage_err("support_attachment_dir_stat", err)),
    }
    #[cfg(unix)]
    if enforce_0700 {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| storage_err("support_attachment_dir_chmod", e))?;
    }
    #[cfg(not(unix))]
    let _ = enforce_0700;
    Ok(())
}

fn resolve_attachments_dir(
    workspace_root: &Path,
    create_if_missing: bool,
) -> Result<Option<PathBuf>, SupportEdgeError> {
    let dot_dir = workspace_root.join(".cowok-design");
    let att_dir = dot_dir.join(SUPPORT_ATTACHMENTS_DIR);
    if create_if_missing {
        ensure_safe_dir(&dot_dir, false)?;
        ensure_safe_dir(&att_dir, true)?;
        return Ok(Some(att_dir));
    }
    for dir in [&dot_dir, &att_dir] {
        match fs::symlink_metadata(dir) {
            Ok(meta) => {
                if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                    return Err(storage_err(
                        "support_attachment_dir_unsafe",
                        "Refusing to traverse symlink or non-directory attachment path",
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(storage_err("support_attachment_dir_stat", err)),
        }
    }
    Ok(Some(att_dir))
}

fn verify_target_not_symlink(target_path: &Path) -> Result<(), SupportEdgeError> {
    match fs::symlink_metadata(target_path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.file_type().is_file() => {
            Err(storage_err(
                "support_attachment_target_unsafe",
                "Refusing to write attachment over symlink or non-regular file",
            ))
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(storage_err("support_attachment_target_stat", err)),
    }
}

fn persist_support_attachment(
    workspace_root: &Path,
    attachment: &ValidatedAttachment,
) -> Result<(), SupportEdgeError> {
    if !is_strict_sha256_hex(&attachment.sha256) {
        return Err(storage_err(
            "support_attachment_sha_invalid",
            "Invalid attachment digest",
        ));
    }
    let att_dir = resolve_attachments_dir(workspace_root, true)?
        .ok_or_else(|| storage_err("support_attachment_dir_missing", "Missing attachments dir"))?;
    let target_path = att_dir.join(format!("{}.{}", attachment.sha256, attachment.ext));
    verify_target_not_symlink(&target_path)?;

    let tmp_path = att_dir.join(format!(
        ".tmp-{}-{}.part",
        attachment.sha256,
        Uuid::new_v4().simple()
    ));
    let write_res = (|| -> std::io::Result<()> {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp_path)?;
        file.write_all(&attachment.bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(storage_err("support_attachment_write", err));
    }
    if let Err(err) = verify_target_not_symlink(&target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    if let Err(err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(storage_err("support_attachment_rename", err));
    }
    Ok(())
}

fn detect_attachment_mime(workspace_root: &Path, sha256: Option<&str>) -> Option<String> {
    let sha = sha256?;
    if !is_strict_sha256_hex(sha) {
        return None;
    }
    let att_dir = resolve_attachments_dir(workspace_root, false)
        .ok()
        .flatten()?;
    for (_, ext, mime) in ATTACHMENT_MEDIA_SPECS {
        let candidate = att_dir.join(format!("{sha}.{ext}"));
        if let Ok(meta) = fs::symlink_metadata(&candidate) {
            if !meta.file_type().is_symlink() && meta.file_type().is_file() {
                return Some(mime.to_string());
            }
        }
    }
    None
}

/// Reads the persisted local image/video attachment for a support ticket by ticket ID.
/// Returns `Ok(None)` when the ticket does not exist or has no persisted attachment file.
pub fn read_support_ticket_attachment(
    pool: &SqlitePool,
    ticket_id: &str,
) -> Result<Option<(Vec<u8>, &'static str)>, SupportEdgeError> {
    let trimmed_id = ticket_id.trim();
    if trimmed_id.is_empty() || trimmed_id.len() > 128 {
        return Ok(None);
    }
    let conn = pool
        .connection()
        .map_err(|e| storage_err("support_attachment_read_connect", e))?;
    let sha_opt: Option<Option<String>> = conn
        .query_row(
            "SELECT screenshot_sha256 FROM local_support_tickets WHERE id = ?1",
            params![trimmed_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| storage_err("support_attachment_read_query", e))?;
    drop(conn);

    let Some(Some(sha)) = sha_opt else {
        return Ok(None);
    };
    if !is_strict_sha256_hex(&sha) {
        return Ok(None);
    }
    let Some(att_dir) = resolve_attachments_dir(pool.workspace_root(), false)? else {
        return Ok(None);
    };
    for (_, ext, mime) in ATTACHMENT_MEDIA_SPECS {
        let candidate = att_dir.join(format!("{sha}.{ext}"));
        let meta = match fs::symlink_metadata(&candidate) {
            Ok(m) => m,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(storage_err("support_attachment_stat", err)),
        };
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(storage_err(
                "support_attachment_symlink",
                "Refusing to serve symlink or non-regular attachment file",
            ));
        }
        if meta.len() > MAX_ATTACHMENT_BYTES as u64 {
            return Err(storage_err(
                "support_attachment_size",
                "Persisted attachment exceeds 5 MB limit",
            ));
        }
        let mut opts = OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.custom_flags(libc::O_NOFOLLOW);
        }
        let file = opts
            .open(&candidate)
            .map_err(|e| storage_err("support_attachment_open", e))?;
        let mut buf = Vec::with_capacity(meta.len().min(MAX_ATTACHMENT_BYTES as u64) as usize);
        file.take((MAX_ATTACHMENT_BYTES as u64) + 1)
            .read_to_end(&mut buf)
            .map_err(|e| storage_err("support_attachment_read", e))?;
        if buf.len() > MAX_ATTACHMENT_BYTES {
            return Err(storage_err(
                "support_attachment_size",
                "Persisted attachment exceeds 5 MB limit",
            ));
        }
        return Ok(Some((buf, mime)));
    }
    Ok(None)
}

/// Optional explicit DDL initializer for legacy pools opened without V23 migrations.
/// Normal daemon traffic relies on V23 migration applied at `SqlitePool::open_wal` (zero per-request DDL).
pub fn ensure_support_edge_tables(pool: &SqlitePool) -> Result<(), SupportEdgeError> {
    let conn = pool
        .connection()
        .map_err(|e| storage_err("support_edge_schema_connect", e))?;
    retry_on_busy("ensure_support_edge_tables", || {
        conn.execute_batch(ENSURE_V23_SQL)
    })
    .map_err(|e| storage_err("support_edge_schema_init", e))
}

fn ensure_default_preference_in_tx(
    tx: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<(String, bool, i64)> {
    if let Some((install_id, enabled_int, updated_at)) = tx
        .query_row(
            "SELECT install_id, telemetry_enabled, updated_at FROM telemetry_preferences WHERE id = ?1",
            params![DEFAULT_PREFERENCE_ROW_ID],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
        )
        .optional()?
    {
        return Ok((install_id, enabled_int != 0, updated_at));
    }

    let now_ms = Utc::now().timestamp_millis();
    let install_id = generate_install_id();
    tx.execute(
        "INSERT OR IGNORE INTO telemetry_preferences (id, install_id, telemetry_enabled, updated_at)
         VALUES (?1, ?2, 1, ?3)",
        params![DEFAULT_PREFERENCE_ROW_ID, install_id, now_ms],
    )?;

    let row: (String, i64, i64) = tx.query_row(
        "SELECT install_id, telemetry_enabled, updated_at FROM telemetry_preferences WHERE id = ?1",
        params![DEFAULT_PREFERENCE_ROW_ID],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok((row.0, row.1 != 0, row.2))
}

fn ensure_default_preference_row(
    conn: &mut rusqlite::Connection,
) -> Result<(String, bool, i64), SupportEdgeError> {
    let existing: Option<(String, i64, i64)> = conn
        .query_row(
            "SELECT install_id, telemetry_enabled, updated_at FROM telemetry_preferences WHERE id = ?1",
            params![DEFAULT_PREFERENCE_ROW_ID],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| storage_err("telemetry_pref_read", e))?;

    if let Some((install_id, enabled_int, updated_at)) = existing {
        return Ok((install_id, enabled_int != 0, updated_at));
    }

    retry_on_busy("init_telemetry_preferences", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let res = ensure_default_preference_in_tx(&tx)?;
        tx.commit()?;
        Ok(res)
    })
    .map_err(|e| storage_err("telemetry_pref_init", e))
}

fn count_pending_outbox(conn: &rusqlite::Connection) -> Result<usize, SupportEdgeError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM telemetry_outbox WHERE status IN ('pending', 'failed')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| storage_err("telemetry_outbox_count", e))?;
    Ok(count.max(0) as usize)
}

pub fn get_telemetry_preferences(
    pool: &SqlitePool,
) -> Result<TelemetryPreferencesDto, SupportEdgeError> {
    get_telemetry_preferences_with_env(pool, &TelemetryEnvOverride::from_process_env())
}

pub fn get_telemetry_preferences_with_env(
    pool: &SqlitePool,
    env_override: &TelemetryEnvOverride,
) -> Result<TelemetryPreferencesDto, SupportEdgeError> {
    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("telemetry_prefs_connect", e))?;
    let (install_id, telemetry_enabled, updated_at) = ensure_default_preference_row(&mut conn)?;
    let pending_outbox_count = count_pending_outbox(&conn)?;
    let env_locked = env_override.is_outbound_blocked();
    let effective_enabled = telemetry_enabled && !env_locked;

    Ok(TelemetryPreferencesDto {
        telemetry_enabled,
        effective_enabled,
        env_locked,
        locked_reason: env_override.locked_reason(),
        airgapped: env_override.airgapped,
        install_id,
        pending_outbox_count,
        updated_at,
    })
}

pub fn set_telemetry_preferences(
    pool: &SqlitePool,
    enabled: bool,
) -> Result<TelemetryPreferencesDto, SupportEdgeError> {
    set_telemetry_preferences_with_env(pool, enabled, &TelemetryEnvOverride::from_process_env())
}

pub fn set_telemetry_preferences_with_env(
    pool: &SqlitePool,
    enabled: bool,
    env_override: &TelemetryEnvOverride,
) -> Result<TelemetryPreferencesDto, SupportEdgeError> {
    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("telemetry_prefs_update_connect", e))?;
    let (install_id, _, _) = ensure_default_preference_row(&mut conn)?;
    let now_ms = Utc::now().timestamp_millis();
    let enabled_int: i64 = if enabled { 1 } else { 0 };

    retry_on_busy("update_telemetry_preferences", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE telemetry_preferences SET telemetry_enabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![enabled_int, now_ms, DEFAULT_PREFERENCE_ROW_ID],
        )?;
        tx.commit()
    })
    .map_err(|e| storage_err("telemetry_prefs_update", e))?;

    let pending_outbox_count = count_pending_outbox(&conn)?;
    let env_locked = env_override.is_outbound_blocked();
    let effective_enabled = enabled && !env_locked;

    Ok(TelemetryPreferencesDto {
        telemetry_enabled: enabled,
        effective_enabled,
        env_locked,
        locked_reason: env_override.locked_reason(),
        airgapped: env_override.airgapped,
        install_id,
        pending_outbox_count,
        updated_at: now_ms,
    })
}

pub fn enqueue_telemetry_event(
    pool: &SqlitePool,
    input: &EnqueueTelemetryInput,
) -> Result<EnqueueTelemetryResult, SupportEdgeError> {
    enqueue_telemetry_event_with_env(pool, input, &TelemetryEnvOverride::from_process_env())
}

pub fn enqueue_telemetry_event_with_env(
    pool: &SqlitePool,
    input: &EnqueueTelemetryInput,
    env_override: &TelemetryEnvOverride,
) -> Result<EnqueueTelemetryResult, SupportEdgeError> {
    let channel = validate_channel(&input.channel)?;
    let event_group = validate_event_group(&input.event_group)?;
    let event_name_trimmed = input.event_name.trim();
    if event_name_trimmed.is_empty() || event_name_trimmed.len() > 128 {
        return Err(validation_err(
            Some("eventName"),
            "INVALID_EVENT_NAME: eventName must be 1..=128 characters",
        ));
    }

    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("enqueue_telemetry_connect", e))?;
    let (install_id, db_enabled, _) = ensure_default_preference_row(&mut conn)?;
    let effective_enabled = db_enabled && !env_override.is_outbound_blocked();

    if !effective_enabled {
        let outbox_depth = count_pending_outbox(&conn)?;
        return Ok(EnqueueTelemetryResult {
            queued: false,
            dropped_by_opt_out: true,
            event_id: None,
            outbox_depth,
            redaction_count: 0,
        });
    }

    let clean_name = scrub_zero_pii(event_name_trimmed);
    let payload_src = if input.payload.is_null() {
        serde_json::json!({})
    } else {
        input.payload.clone()
    };
    let (clean_payload, payload_redactions) = scrub_json_value(&payload_src);
    let redaction_count = clean_name.redaction_count + payload_redactions;
    let payload_json = serde_json::to_string(&clean_payload).unwrap_or_else(|_| "{}".to_string());
    let event_id = Uuid::new_v4().to_string();
    let now_ms = Utc::now().timestamp_millis();

    let outbox_depth = retry_on_busy("enqueue_telemetry_outbox_fifo_cap", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO telemetry_outbox
                (id, install_id, channel, event_group, event_name, payload_json, status, attempt_count, next_retry_at, last_error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 0, 0, NULL, ?7)",
            params![
                event_id,
                install_id,
                channel,
                event_group,
                clean_name.scrubbed,
                payload_json,
                now_ms
            ],
        )?;
        // Enforce strict FIFO-500 ring cap (keep newest 500 by created_at DESC, rowid DESC).
        tx.execute(
            "DELETE FROM telemetry_outbox
             WHERE rowid IN (
                 SELECT rowid FROM telemetry_outbox
                 ORDER BY created_at DESC, rowid DESC
                 LIMIT -1 OFFSET ?1
             )",
            params![TELEMETRY_OUTBOX_FIFO_CAP as i64],
        )?;
        let depth: i64 = tx.query_row("SELECT COUNT(*) FROM telemetry_outbox", [], |r| r.get(0))?;
        tx.commit()?;
        Ok(depth.max(0) as usize)
    })
    .map_err(|e| storage_err("enqueue_telemetry_outbox", e))?;

    Ok(EnqueueTelemetryResult {
        queued: true,
        dropped_by_opt_out: false,
        event_id: Some(event_id),
        outbox_depth,
        redaction_count,
    })
}

pub fn list_outbox_items(
    pool: &SqlitePool,
    limit: usize,
) -> Result<Vec<TelemetryOutboxItem>, SupportEdgeError> {
    let conn = pool
        .connection()
        .map_err(|e| storage_err("outbox_list_connect", e))?;
    let mut stmt = conn
        .prepare(
            "SELECT id, install_id, channel, event_group, event_name, payload_json, status, attempt_count, next_retry_at, last_error, created_at
             FROM telemetry_outbox
             ORDER BY created_at ASC, rowid ASC
             LIMIT ?1",
        )
        .map_err(|e| storage_err("outbox_list_prepare", e))?;
    let rows = stmt
        .query_map(
            params![limit.clamp(1, TELEMETRY_OUTBOX_FIFO_CAP) as i64],
            |r| {
                Ok(TelemetryOutboxItem {
                    id: r.get(0)?,
                    install_id: r.get(1)?,
                    channel: r.get(2)?,
                    event_group: r.get(3)?,
                    event_name: r.get(4)?,
                    payload_json: r.get(5)?,
                    status: r.get(6)?,
                    attempt_count: r.get::<_, i64>(7)?.max(0) as u32,
                    next_retry_at: r.get(8)?,
                    last_error: r.get(9)?,
                    created_at: r.get(10)?,
                })
            },
        )
        .map_err(|e| storage_err("outbox_list_query", e))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| storage_err("outbox_row_decode", e))?);
    }
    Ok(out)
}

enum ClaimBatchOutcome {
    OptedOut,
    Empty,
    Claimed(Vec<TelemetryOutboxItem>),
}

/// Atomically claims due outbox rows into `'flushing'` inside `TransactionBehavior::Immediate`,
/// recording `now_ms` in `next_retry_at` for stale-claim recovery, and drops `conn` before returning.
fn claim_due_outbox_batch(
    pool: &SqlitePool,
    env_override: &TelemetryEnvOverride,
    has_endpoint: bool,
    max_batch: usize,
    now_ms: i64,
) -> Result<ClaimBatchOutcome, SupportEdgeError> {
    if env_override.is_outbound_blocked() {
        return Ok(ClaimBatchOutcome::OptedOut);
    }
    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("claim_outbox_connect", e))?;
    let outcome = retry_on_busy("claim_telemetry_outbox_batch", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (_, db_enabled, _) = ensure_default_preference_in_tx(&tx)?;
        if !db_enabled {
            tx.commit()?;
            return Ok(ClaimBatchOutcome::OptedOut);
        }
        if !has_endpoint {
            tx.commit()?;
            return Ok(ClaimBatchOutcome::Empty);
        }

        let candidates: Vec<TelemetryOutboxItem> = {
            let mut stmt = tx.prepare(
                "SELECT id, install_id, channel, event_group, event_name, payload_json, status, attempt_count, next_retry_at, last_error, created_at
                 FROM telemetry_outbox
                 WHERE status IN ('pending', 'failed') AND next_retry_at <= ?1
                 ORDER BY created_at ASC, rowid ASC
                 LIMIT ?2",
            )?;
            let mapped = stmt.query_map(params![now_ms, max_batch.clamp(1, 100) as i64], |r| {
                Ok(TelemetryOutboxItem {
                    id: r.get(0)?,
                    install_id: r.get(1)?,
                    channel: r.get(2)?,
                    event_group: r.get(3)?,
                    event_name: r.get(4)?,
                    payload_json: r.get(5)?,
                    status: "flushing".to_string(),
                    attempt_count: r.get::<_, i64>(7)?.max(0) as u32,
                    next_retry_at: now_ms,
                    last_error: r.get(9)?,
                    created_at: r.get(10)?,
                })
            })?;
            let mut buf = Vec::new();
            for row in mapped {
                buf.push(row?);
            }
            buf
        };

        if candidates.is_empty() {
            tx.commit()?;
            return Ok(ClaimBatchOutcome::Empty);
        }

        let mut claimed = Vec::with_capacity(candidates.len());
        for item in candidates {
            let updated = tx.execute(
                "UPDATE telemetry_outbox
                 SET status = 'flushing', next_retry_at = ?1
                 WHERE id = ?2 AND status IN ('pending', 'failed')",
                params![now_ms, item.id],
            )?;
            if updated == 1 {
                claimed.push(item);
            }
        }
        tx.commit()?;
        if claimed.is_empty() {
            Ok(ClaimBatchOutcome::Empty)
        } else {
            Ok(ClaimBatchOutcome::Claimed(claimed))
        }
    })
    .map_err(|e| storage_err("claim_outbox_batch", e))?;
    drop(conn);
    Ok(outcome)
}

fn finalize_claimed_outbox_batch(
    pool: &SqlitePool,
    due_items: &[TelemetryOutboxItem],
    now_ms: i64,
    send_outcome: TransportSendOutcome,
) -> Result<FlushOutcome, SupportEdgeError> {
    let attempted = due_items.len();
    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("finalize_outbox_connect", e))?;
    match send_outcome {
        TransportSendOutcome::Delivered => {
            retry_on_busy("mark_outbox_batch_sent", || {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                for item in due_items {
                    tx.execute(
                        "UPDATE telemetry_outbox
                         SET status = 'sent', last_error = NULL
                         WHERE id = ?1 AND status = 'flushing'",
                        params![item.id],
                    )?;
                }
                tx.commit()
            })
            .map_err(|e| storage_err("mark_outbox_batch_sent", e))?;
            Ok(FlushOutcome {
                attempted,
                sent: attempted,
                failed: 0,
                skipped_by_opt_out: false,
                http_calls: 1,
            })
        }
        TransportSendOutcome::RetryableFailure(err_msg) => {
            let clean_err = scrub_zero_pii(&err_msg).scrubbed;
            retry_on_busy("mark_outbox_batch_retry", || {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                for item in due_items {
                    let next_attempt = item.attempt_count.saturating_add(1);
                    let delay_ms = compute_backoff_secs(next_attempt).saturating_mul(1000);
                    let next_retry = now_ms.saturating_add(delay_ms);
                    tx.execute(
                        "UPDATE telemetry_outbox
                         SET status = 'failed', attempt_count = ?1, next_retry_at = ?2, last_error = ?3
                         WHERE id = ?4 AND status = 'flushing'",
                        params![next_attempt as i64, next_retry, clean_err, item.id],
                    )?;
                }
                tx.commit()
            })
            .map_err(|e| storage_err("mark_outbox_batch_retry", e))?;
            Ok(FlushOutcome {
                attempted,
                sent: 0,
                failed: attempted,
                skipped_by_opt_out: false,
                http_calls: 1,
            })
        }
    }
}

/// Recovers rows stuck in `'flushing'` longer than `stale_after_ms` (e.g. after daemon crash)
/// back to `'failed'` so the next flush tick retries them cleanly.
pub fn recover_stale_flushing_rows(
    pool: &SqlitePool,
    now_ms: i64,
    stale_after_ms: i64,
) -> Result<usize, SupportEdgeError> {
    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("recover_stale_flushing_connect", e))?;
    let cutoff_ms = now_ms.saturating_sub(stale_after_ms.max(0));
    retry_on_busy("recover_stale_flushing_rows", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let recovered = tx.execute(
            "UPDATE telemetry_outbox
             SET status = 'failed', next_retry_at = ?1
             WHERE status = 'flushing' AND (next_retry_at = 0 OR next_retry_at <= ?2)",
            params![now_ms, cutoff_ms],
        )?;
        tx.commit()?;
        Ok(recovered)
    })
    .map_err(|e| storage_err("recover_stale_flushing_rows", e))
}

/// Flushes a due batch of pending/failed telemetry events with immediate opt-out/airgap guard
/// (`0` HTTP calls when disabled), atomic `'flushing'` claim (dropping SQLite `conn` before
/// network I/O), and exponential backoff (`30s -> 60s -> 120s -> 300s -> 900s`).
pub fn flush_telemetry_batch_with_transport<T: TelemetryBatchTransport>(
    pool: &SqlitePool,
    env_override: &TelemetryEnvOverride,
    endpoint_url: Option<&str>,
    max_batch: usize,
    now_ms: i64,
    transport: &T,
) -> Result<FlushOutcome, SupportEdgeError> {
    let trimmed_endpoint = endpoint_url.map(str::trim).filter(|s| !s.is_empty());
    let claimed = claim_due_outbox_batch(
        pool,
        env_override,
        trimmed_endpoint.is_some(),
        max_batch,
        now_ms,
    )?;
    let due_items = match claimed {
        ClaimBatchOutcome::OptedOut => {
            return Ok(FlushOutcome {
                attempted: 0,
                sent: 0,
                failed: 0,
                skipped_by_opt_out: true,
                http_calls: 0,
            });
        }
        ClaimBatchOutcome::Empty => {
            return Ok(FlushOutcome {
                attempted: 0,
                sent: 0,
                failed: 0,
                skipped_by_opt_out: false,
                http_calls: 0,
            });
        }
        ClaimBatchOutcome::Claimed(items) => items,
    };

    let Some(endpoint) = trimmed_endpoint else {
        return Ok(FlushOutcome {
            attempted: 0,
            sent: 0,
            failed: 0,
            skipped_by_opt_out: false,
            http_calls: 0,
        });
    };

    let send_outcome = transport.send_batch(endpoint, &due_items);
    finalize_claimed_outbox_batch(pool, &due_items, now_ms, send_outcome)
}

/// Async telemetry batch flush using `HttpTelemetryTransport` (`reqwest::Client`), never holding
/// any SQLite connection across async network I/O.
pub async fn flush_telemetry_batch_async(
    pool: &SqlitePool,
    env_override: &TelemetryEnvOverride,
    endpoint_url: Option<&str>,
    max_batch: usize,
    now_ms: i64,
    transport: &HttpTelemetryTransport,
) -> Result<FlushOutcome, SupportEdgeError> {
    let trimmed_endpoint = endpoint_url.map(str::trim).filter(|s| !s.is_empty());
    let has_endpoint = trimmed_endpoint.is_some();
    let claim_pool = pool.clone();
    let env_clone = env_override.clone();
    let claimed = tokio::task::spawn_blocking(move || {
        claim_due_outbox_batch(&claim_pool, &env_clone, has_endpoint, max_batch, now_ms)
    })
    .await
    .map_err(|e| storage_err("flush_claim_join", e))??;

    let due_items = match claimed {
        ClaimBatchOutcome::OptedOut => {
            return Ok(FlushOutcome {
                attempted: 0,
                sent: 0,
                failed: 0,
                skipped_by_opt_out: true,
                http_calls: 0,
            });
        }
        ClaimBatchOutcome::Empty => {
            return Ok(FlushOutcome {
                attempted: 0,
                sent: 0,
                failed: 0,
                skipped_by_opt_out: false,
                http_calls: 0,
            });
        }
        ClaimBatchOutcome::Claimed(items) => items,
    };

    let Some(endpoint) = trimmed_endpoint else {
        return Ok(FlushOutcome {
            attempted: 0,
            sent: 0,
            failed: 0,
            skipped_by_opt_out: false,
            http_calls: 0,
        });
    };

    let send_outcome = transport.send_batch_async(endpoint, &due_items).await;
    let finalize_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        finalize_claimed_outbox_batch(&finalize_pool, &due_items, now_ms, send_outcome)
    })
    .await
    .map_err(|e| storage_err("flush_finalize_join", e))?
}

/// Spawns the cancellable 45-second background telemetry flusher loop.
/// Recovers stale `'flushing'` rows (> 5 min) on startup, checks opt-out / air-gap / endpoint
/// before every tick (guaranteeing 0 HTTP calls when disabled or unset), and never blocks the UI.
pub fn spawn_telemetry_background_flusher(pool: SqlitePool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(transport) = HttpTelemetryTransport::new() else {
            return;
        };
        let startup_pool = pool.clone();
        let _ = tokio::task::spawn_blocking(move || {
            recover_stale_flushing_rows(
                &startup_pool,
                Utc::now().timestamp_millis(),
                TELEMETRY_STALE_FLUSHING_MS,
            )
        })
        .await;

        let mut ticker = tokio::time::interval(Duration::from_secs(TELEMETRY_FLUSH_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume immediate tick so daemon startup remains instantaneous with zero outbound I/O.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            let env_override = TelemetryEnvOverride::from_process_env();
            if env_override.is_outbound_blocked() {
                continue;
            }
            let endpoint = std::env::var("COWOK_TELEMETRY_ENDPOINT")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let Some(endpoint_url) = endpoint else {
                continue;
            };
            let now_ms = Utc::now().timestamp_millis();
            let recover_pool = pool.clone();
            let _ = tokio::task::spawn_blocking(move || {
                recover_stale_flushing_rows(&recover_pool, now_ms, TELEMETRY_STALE_FLUSHING_MS)
            })
            .await;
            let _ = flush_telemetry_batch_async(
                &pool,
                &env_override,
                Some(endpoint_url.as_str()),
                TELEMETRY_FLUSH_BATCH_SIZE,
                now_ms,
                &transport,
            )
            .await;
        }
    })
}

pub fn create_support_ticket(
    pool: &SqlitePool,
    input: &CreateSupportTicketInput,
) -> Result<LocalSupportTicketDto, SupportEdgeError> {
    create_support_ticket_with_env(pool, input, &TelemetryEnvOverride::from_process_env())
}

pub fn create_support_ticket_with_env(
    pool: &SqlitePool,
    input: &CreateSupportTicketInput,
    env_override: &TelemetryEnvOverride,
) -> Result<LocalSupportTicketDto, SupportEdgeError> {
    let category = validate_category(&input.category)?;
    let channel = validate_channel(&input.channel)?;
    let validated_locale = validate_locale(&input.locale)?;

    let raw_title = input.title.trim();
    if raw_title.chars().count() < MIN_TICKET_TITLE_LEN
        || raw_title.chars().count() > MAX_TICKET_TITLE_LEN
    {
        return Err(validation_err(
            Some("title"),
            format!(
                "INVALID_TITLE: title must be {MIN_TICKET_TITLE_LEN}..={MAX_TICKET_TITLE_LEN} characters"
            ),
        ));
    }

    let raw_desc = input.description.trim();
    if raw_desc.chars().count() < MIN_TICKET_DESC_LEN
        || raw_desc.chars().count() > MAX_TICKET_DESC_LEN
    {
        return Err(validation_err(
            Some("description"),
            format!(
                "INVALID_DESCRIPTION: description must be {MIN_TICKET_DESC_LEN}..={MAX_TICKET_DESC_LEN} characters"
            ),
        ));
    }

    let contact_email = validate_optional_email(input.contact_email.as_deref())?;
    let attachment = validate_screenshot_data_url(input.screenshot_data_url.as_deref())?;
    if let Some(ref att) = attachment {
        persist_support_attachment(pool.workspace_root(), att)?;
    }
    let screenshot_sha256 = attachment.as_ref().map(|a| a.sha256.clone());
    let attachment_mime = attachment.as_ref().map(|a| a.mime.to_string());

    let clean_title = scrub_zero_pii(raw_title);
    let clean_desc = scrub_zero_pii(raw_desc);
    let mut diagnostics = build_scrubbed_diagnostics(
        channel,
        &validated_locale,
        input.include_diagnostics,
        &input.recent_error_codes,
        &input.active_skills,
    );
    diagnostics.redaction_count += clean_title.redaction_count + clean_desc.redaction_count;

    let mut conn = pool
        .connection()
        .map_err(|e| storage_err("create_support_ticket_connect", e))?;
    let (install_id, _, _) = ensure_default_preference_row(&mut conn)?;

    let id = Uuid::new_v4().to_string();
    let now_ms = Utc::now().timestamp_millis();
    let diagnostics_json = serde_json::to_string(&diagnostics).unwrap_or_else(|_| "{}".to_string());
    let sync_status = if env_override.airgapped {
        "local_only"
    } else {
        "queued"
    };

    let mut ticket_code = generate_ticket_code();
    for attempt in 0..MAX_TICKET_CODE_RETRIES {
        let insert_res = retry_on_busy("insert_local_support_ticket", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO local_support_tickets
                    (id, ticket_code, install_id, channel, category, title, description, contact_email, screenshot_sha256, diagnostics_json, status, admin_replies_json, sync_status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'submitted', '[]', ?11, ?12, ?12)",
                params![
                    id,
                    ticket_code,
                    install_id,
                    channel,
                    category,
                    clean_title.scrubbed,
                    clean_desc.scrubbed,
                    contact_email,
                    screenshot_sha256,
                    diagnostics_json,
                    sync_status,
                    now_ms
                ],
            )?;
            tx.commit()
        });
        match insert_res {
            Ok(()) => break,
            Err(HarnessError::ValidationFailed { ref reason })
                if reason.starts_with("UNIQUE_CONSTRAINT")
                    && attempt + 1 < MAX_TICKET_CODE_RETRIES =>
            {
                ticket_code = generate_ticket_code();
            }
            Err(err) => return Err(storage_err("insert_local_support_ticket", err)),
        }
    }
    drop(conn);

    // Emit anonymized feature_usage telemetry event (automatically no-ops if opted out or air-gapped).
    let _ = enqueue_telemetry_event_with_env(
        pool,
        &EnqueueTelemetryInput {
            channel: channel.to_string(),
            event_group: "feature_usage".to_string(),
            event_name: "support_ticket_submitted".to_string(),
            payload: serde_json::json!({
                "category": category,
                "hasScreenshot": screenshot_sha256.is_some(),
                "includeDiagnostics": input.include_diagnostics,
            }),
        },
        env_override,
    );

    Ok(LocalSupportTicketDto {
        id,
        ticket_code,
        install_id,
        channel: channel.to_string(),
        category: category.to_string(),
        title: clean_title.scrubbed,
        description: clean_desc.scrubbed,
        contact_email,
        screenshot_sha256,
        attachment_mime,
        diagnostics,
        status: "submitted".to_string(),
        admin_replies: Vec::new(),
        sync_status: sync_status.to_string(),
        created_at: now_ms,
        updated_at: now_ms,
    })
}

pub fn list_support_tickets(
    pool: &SqlitePool,
    limit: usize,
) -> Result<Vec<LocalSupportTicketDto>, SupportEdgeError> {
    let conn = pool
        .connection()
        .map_err(|e| storage_err("support_ticket_list_connect", e))?;
    let mut stmt = conn
        .prepare(
            "SELECT id, ticket_code, install_id, channel, category, title, description, contact_email, screenshot_sha256, diagnostics_json, status, admin_replies_json, sync_status, created_at, updated_at
             FROM local_support_tickets
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?1",
        )
        .map_err(|e| storage_err("support_ticket_list_prepare", e))?;

    let rows = stmt
        .query_map(params![limit.clamp(1, 200) as i64], |r| {
            let diagnostics_raw: String = r.get(9)?;
            let replies_raw: String = r.get(11)?;
            let channel: String = r.get(3)?;
            let mut diagnostics = serde_json::from_str::<DiagnosticBundle>(&diagnostics_raw)
                .unwrap_or_else(|_| DiagnosticBundle {
                    app_version: env!("CARGO_PKG_VERSION").to_string(),
                    channel: channel.clone(),
                    os: std::env::consts::OS.to_string(),
                    locale: "vi-VN".to_string(),
                    recent_error_codes: Vec::new(),
                    active_skills: Vec::new(),
                    redaction_count: 0,
                });
            if diagnostics.locale.chars().count() > MAX_LOCALE_LEN {
                diagnostics.locale = diagnostics.locale.chars().take(MAX_LOCALE_LEN).collect();
            }
            let admin_replies =
                serde_json::from_str::<Vec<AdminReplyDto>>(&replies_raw).unwrap_or_default();
            let screenshot_sha256: Option<String> = r.get(8)?;
            let attachment_mime =
                detect_attachment_mime(pool.workspace_root(), screenshot_sha256.as_deref());
            Ok(LocalSupportTicketDto {
                id: r.get(0)?,
                ticket_code: r.get(1)?,
                install_id: r.get(2)?,
                channel,
                category: r.get(4)?,
                title: r.get(5)?,
                description: r.get(6)?,
                contact_email: r.get(7)?,
                screenshot_sha256,
                attachment_mime,
                diagnostics,
                status: r.get(10)?,
                admin_replies,
                sync_status: r.get(12)?,
                created_at: r.get(13)?,
                updated_at: r.get(14)?,
            })
        })
        .map_err(|e| storage_err("support_ticket_list_query", e))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| storage_err("support_ticket_row_decode", e))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_scrub_zero_pii_strips_dynamic_secrets_and_home_paths() {
        let fake_sk = ["sk", "-", "live_", "testsecret1234567890abcdef"].concat();
        let fake_aiza = ["AIza", "SyTestFakeKey1234567890ABCDEF"].concat();
        let fake_ghp = ["ghp", "_", "1234567890abcdefABCDEF123456"].concat();
        let fake_xox = ["xox", "b-", "1234567890-abcdef123456"].concat();
        let fake_jwt = [
            "eyJ",
            "hbGciOiJIUzI1NiJ9",
            ".",
            "eyJ",
            "zdWIiOiIxMjM0NTY3ODkwIn0",
            ".",
            "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        ]
        .concat();
        let fake_bearer = ["Bearer\t", "tok_", "secretvalue123456789"].concat();
        let fake_cookie = ["x_cookie: ", "session_id=abc123; csrf=xyz"].concat();

        let sample = format!(
            "err key={fake_sk} google={fake_aiza} gh=E_AUTH_{fake_ghp} slack={fake_xox} jwt_{fake_jwt}\nhdr-{fake_bearer}\n{fake_cookie}\nmac=/users/alice/projects/app linux=/home/bob/work win=C:\\Users\\carol\\repo win2=d:/users/dave/app"
        );

        let res = scrub_zero_pii(&sample);
        assert!(res.redaction_count >= 11);
        assert!(!res.scrubbed.contains(&fake_sk));
        assert!(!res.scrubbed.contains(&fake_aiza));
        assert!(!res.scrubbed.contains(&fake_ghp));
        assert!(!res.scrubbed.contains(&fake_xox));
        assert!(!res.scrubbed.contains(&fake_jwt));
        assert!(!res.scrubbed.contains("secretvalue123456789"));
        assert!(!res.scrubbed.contains("session_id=abc123"));
        assert!(!res.scrubbed.contains("/users/alice"));
        assert!(!res.scrubbed.contains("/home/bob"));
        assert!(!res.scrubbed.contains("C:\\Users\\carol"));
        assert!(!res.scrubbed.contains("d:/users/dave"));
        assert!(res.scrubbed.contains("<HOME>/projects/app"));
        assert!(res.scrubbed.contains("<HOME>/work"));
        assert!(res.scrubbed.contains("<HOME>\\repo"));
        assert!(res.scrubbed.contains("<HOME>/app"));
    }

    #[test]
    fn unit_backoff_schedule_matches_spec() {
        assert_eq!(compute_backoff_secs(1), 30);
        assert_eq!(compute_backoff_secs(2), 60);
        assert_eq!(compute_backoff_secs(3), 120);
        assert_eq!(compute_backoff_secs(4), 300);
        assert_eq!(compute_backoff_secs(5), 900);
        assert_eq!(compute_backoff_secs(9), 900);
    }
}
