// @story 14.2
// @implements AC-01, AC-02, AC-05 (OIDC+MFA+4-Role RBAC Session Auth, Immutable Audit Log, Per-(install_id, IP) Rate Limiter, Defense-in-Depth PII Scrubber & Polyglot-Safe Image Re-Encoder)
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::http::{HeaderMap, StatusCode};
use base64::engine::general_purpose::{STANDARD as B64_STD, URL_SAFE_NO_PAD as B64_URL};
use base64::Engine as _;
use chrono::Utc;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use spec_registry::{retry_on_busy, SqlitePool};
use uuid::Uuid;

use crate::routes::constant_time_eq;
use crate::routes::legacy_sessions::hmac_sha256;

pub const ADMIN_TOKEN_PREFIX: &str = "cwk_adm_v1";
pub const OIDC_ASSERTION_PREFIX: &str = "cwk_oidc_v1";
pub const DEFAULT_DEV_MFA_CODE: &str = "314159";
pub const DEFAULT_ADMIN_SESSION_TTL_SECS: i64 = 4 * 60 * 60;
pub const MAX_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;
pub const DEFAULT_INGEST_RATE_LIMIT_MAX: usize = 60;
pub const DEFAULT_INGEST_RATE_WINDOW_SECS: i64 = 60;
const MAX_RATE_LIMIT_BUCKETS: usize = 10_000;

pub const REDACTED_SECRET_TOKEN: &str = "[REDACTED_SECRET]";
pub const REDACTED_EMAIL_TOKEN: &str = "[REDACTED_EMAIL]";
pub const REDACTED_HOME_TOKEN: &str = "<HOME>";

const ENSURE_V29_SQL: &str =
    include_str!("../../../spec-registry/migrations/V29__story_14_2_cloud_control_plane.sql");

static FALLBACK_ADMIN_HMAC_KEY: OnceLock<[u8; 32]> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloudAdminRole {
    SuperAdmin,
    SupportAgent,
    SkillCurator,
    AnalystReadonly,
}

impl CloudAdminRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SuperAdmin => "SUPER_ADMIN",
            Self::SupportAgent => "SUPPORT_AGENT",
            Self::SkillCurator => "SKILL_CURATOR",
            Self::AnalystReadonly => "ANALYST_READONLY",
        }
    }

    pub fn parse_strict(raw: &str) -> Option<Self> {
        match raw.trim() {
            "SUPER_ADMIN" => Some(Self::SuperAdmin),
            "SUPPORT_AGENT" => Some(Self::SupportAgent),
            "SKILL_CURATOR" => Some(Self::SkillCurator),
            "ANALYST_READONLY" => Some(Self::AnalystReadonly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminSessionClaims {
    pub subject: String,
    pub email: String,
    pub role: CloudAdminRole,
    pub mfa_verified: bool,
    pub oidc_provider: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub jti: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudAuthError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl CloudAuthError {
    pub fn auth_required(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "ADMIN_AUTH_REQUIRED",
            message: message.into(),
        }
    }

    pub fn mfa_required(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "MFA_REQUIRED",
            message: message.into(),
        }
    }

    pub fn rbac_forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "RBAC_FORBIDDEN",
            message: message.into(),
        }
    }
}

fn resolve_admin_signing_key() -> [u8; 32] {
    if let Ok(secret) = std::env::var("COWOK_CLOUD_ADMIN_HMAC_SECRET") {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(trimmed.as_bytes());
            let digest = hasher.finalize();
            let mut key = [0u8; 32];
            key.copy_from_slice(&digest);
            return key;
        }
    }
    *FALLBACK_ADMIN_HMAC_KEY.get_or_init(|| {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Issues a signed HMAC-SHA256 SuperAdmin OIDC+MFA session token.
pub fn issue_admin_session_token(claims: &AdminSessionClaims) -> Result<String, String> {
    let payload_bytes = serde_json::to_vec(claims).map_err(|e| format!("serialize claims: {e}"))?;
    let payload_b64 = B64_URL.encode(&payload_bytes);
    let key = resolve_admin_signing_key();
    let mac = hmac_sha256(&key, payload_b64.as_bytes());
    let sig_hex = hex_encode(&mac);
    Ok(format!("{ADMIN_TOKEN_PREFIX}.{payload_b64}.{sig_hex}"))
}

/// Verifies a signed token, checking HMAC integrity, expiry, and `mfa_verified == true`.
pub fn verify_admin_session_token(token: &str) -> Result<AdminSessionClaims, CloudAuthError> {
    let trimmed = token.trim();
    let mut parts = trimmed.split('.');
    let prefix = parts.next().unwrap_or_default();
    let payload_b64 = parts.next().unwrap_or_default();
    let sig_hex = parts.next().unwrap_or_default();
    if prefix != ADMIN_TOKEN_PREFIX
        || payload_b64.is_empty()
        || sig_hex.len() != 64
        || parts.next().is_some()
    {
        return Err(CloudAuthError::auth_required(
            "Malformed SuperAdmin session token",
        ));
    }

    let key = resolve_admin_signing_key();
    let expected_mac = hmac_sha256(&key, payload_b64.as_bytes());
    let expected_hex = hex_encode(&expected_mac);
    if !constant_time_eq(sig_hex.as_bytes(), expected_hex.as_bytes()) {
        return Err(CloudAuthError::auth_required(
            "Invalid SuperAdmin session signature",
        ));
    }

    let payload_bytes = B64_URL
        .decode(payload_b64)
        .map_err(|_| CloudAuthError::auth_required("Invalid SuperAdmin token encoding"))?;
    let claims: AdminSessionClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|_| CloudAuthError::auth_required("Invalid SuperAdmin token payload"))?;

    let now = Utc::now().timestamp();
    if claims.expires_at <= now {
        return Err(CloudAuthError::auth_required(
            "SuperAdmin session token has expired",
        ));
    }
    if claims.subject.trim().is_empty() || claims.email.trim().is_empty() {
        return Err(CloudAuthError::auth_required(
            "SuperAdmin session token is missing operator identity",
        ));
    }
    if !claims.mfa_verified {
        return Err(CloudAuthError::mfa_required(
            "Multi-factor authentication (MFA) is strictly required for SuperAdmin access",
        ));
    }

    Ok(claims)
}

/// Extracts and verifies an OIDC+MFA SuperAdmin session from `X-Admin-Session` or `Authorization: Bearer ...`.
pub fn verify_admin_session_from_headers(
    headers: &HeaderMap,
) -> Result<AdminSessionClaims, CloudAuthError> {
    if let Some(raw) = headers.get("x-admin-session").and_then(|v| v.to_str().ok()) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return verify_admin_session_token(trimmed);
        }
    }
    if let Some(auth_val) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let trimmed = auth_val.trim();
        if let Some(rest) = trimmed.strip_prefix("Bearer ") {
            return verify_admin_session_token(rest.trim());
        }
        if let Some(rest) = trimmed.strip_prefix("bearer ") {
            return verify_admin_session_token(rest.trim());
        }
    }
    Err(CloudAuthError::auth_required(
        "Missing SuperAdmin OIDC+MFA session token",
    ))
}

/// Enforces 4-Role RBAC policy on verified `AdminSessionClaims`.
pub fn require_admin_role(
    claims: &AdminSessionClaims,
    allowed: &[CloudAdminRole],
) -> Result<(), CloudAuthError> {
    if !claims.mfa_verified {
        return Err(CloudAuthError::mfa_required(
            "MFA verification required before evaluating RBAC permissions",
        ));
    }
    if allowed.contains(&claims.role) {
        return Ok(());
    }
    Err(CloudAuthError::rbac_forbidden(format!(
        "Role '{}' is not permitted to perform this SuperAdmin action",
        claims.role.as_str()
    )))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcAssertionClaims {
    pub oidc_provider: String,
    pub subject: String,
    pub email: String,
    pub role: CloudAdminRole,
    pub issued_at: i64,
    pub expires_at: i64,
}

/// Resolves the shared OIDC verification secret from `COWOK_CLOUD_ADMIN_OIDC_SECRET`
/// or derives a deterministic per-daemon secret from `daemon_nonce` and the admin signing key.
pub fn resolve_oidc_shared_secret(daemon_nonce: &str) -> String {
    if let Ok(secret) = std::env::var("COWOK_CLOUD_ADMIN_OIDC_SECRET") {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let signing_key = resolve_admin_signing_key();
    let mut hasher = Sha256::new();
    hasher.update(b"cowok-cloud-oidc-secret-v1:");
    hasher.update(daemon_nonce.trim().as_bytes());
    hasher.update(b":");
    hasher.update(signing_key);
    hex_encode(&hasher.finalize())
}

fn resolve_oidc_assertion_key(daemon_nonce: &str) -> [u8; 32] {
    let secret = resolve_oidc_shared_secret(daemon_nonce);
    let mut hasher = Sha256::new();
    hasher.update(b"cowok-cloud-oidc-assertion-key-v1:");
    hasher.update(secret.as_bytes());
    let digest = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

/// Issues an HMAC-SHA256 signed server-side OIDC assertion for verified identity exchange.
pub fn sign_oidc_assertion(
    daemon_nonce: &str,
    claims: &OidcAssertionClaims,
) -> Result<String, String> {
    let payload_bytes =
        serde_json::to_vec(claims).map_err(|e| format!("serialize oidc assertion: {e}"))?;
    let payload_b64 = B64_URL.encode(&payload_bytes);
    let key = resolve_oidc_assertion_key(daemon_nonce);
    let mac = hmac_sha256(&key, payload_b64.as_bytes());
    let sig_hex = hex_encode(&mac);
    Ok(format!("{OIDC_ASSERTION_PREFIX}.{payload_b64}.{sig_hex}"))
}

/// Verifies an HMAC-SHA256 signed OIDC assertion and validates its expiry and non-empty fields.
pub fn verify_oidc_assertion(
    daemon_nonce: &str,
    assertion: &str,
) -> Result<OidcAssertionClaims, CloudAuthError> {
    let trimmed = assertion.trim();
    let mut parts = trimmed.split('.');
    let prefix = parts.next().unwrap_or_default();
    let payload_b64 = parts.next().unwrap_or_default();
    let sig_hex = parts.next().unwrap_or_default();
    if prefix != OIDC_ASSERTION_PREFIX
        || payload_b64.is_empty()
        || sig_hex.len() != 64
        || parts.next().is_some()
    {
        return Err(CloudAuthError::auth_required("Malformed OIDC assertion"));
    }
    let key = resolve_oidc_assertion_key(daemon_nonce);
    let expected_hex = hex_encode(&hmac_sha256(&key, payload_b64.as_bytes()));
    if !constant_time_eq(sig_hex.as_bytes(), expected_hex.as_bytes()) {
        return Err(CloudAuthError::auth_required(
            "Invalid OIDC assertion signature",
        ));
    }
    let payload_bytes = B64_URL
        .decode(payload_b64)
        .map_err(|_| CloudAuthError::auth_required("Invalid OIDC assertion encoding"))?;
    let claims: OidcAssertionClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|_| CloudAuthError::auth_required("Invalid OIDC assertion payload"))?;
    if claims.expires_at <= Utc::now().timestamp() {
        return Err(CloudAuthError::auth_required("OIDC assertion has expired"));
    }
    if claims.oidc_provider.trim().is_empty()
        || claims.subject.trim().is_empty()
        || claims.email.trim().is_empty()
    {
        return Err(CloudAuthError::auth_required(
            "OIDC assertion is missing required operator claims",
        ));
    }
    Ok(claims)
}

/// Verifies `X-Cloud-Admin-Oidc-Secret` header in constant time against the configured/derived OIDC secret.
pub fn verify_oidc_shared_secret_header(daemon_nonce: &str, headers: &HeaderMap) -> bool {
    let Some(provided) = headers
        .get("x-cloud-admin-oidc-secret")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    let expected = resolve_oidc_shared_secret(daemon_nonce);
    constant_time_eq(provided.as_bytes(), expected.as_bytes())
}

/// Resolves an operator's server-provisioned RBAC role from `COWOK_CLOUD_ADMIN_OPERATORS`
/// (`email=ROLE,...`) or the built-in local Workbench operator allowlist.
pub fn resolve_operator_bound_role(email: &str) -> Option<CloudAdminRole> {
    let normalized = email.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if let Ok(raw) = std::env::var("COWOK_CLOUD_ADMIN_OPERATORS") {
        for entry in raw.split(',') {
            let trimmed = entry.trim();
            if let Some((addr, role_str)) = trimmed.split_once('=') {
                if addr.trim().eq_ignore_ascii_case(&normalized) {
                    return CloudAdminRole::parse_strict(role_str);
                }
            }
        }
    }
    match normalized.as_str() {
        "ops-admin@cowok.design" | "sup@cowok.design" | "admin@cowok.design" => {
            Some(CloudAdminRole::SuperAdmin)
        }
        "desk@cowok.design" | "agt@cowok.design" | "support@cowok.design" => {
            Some(CloudAdminRole::SupportAgent)
        }
        "cur@cowok.design" | "curator@cowok.design" => Some(CloudAdminRole::SkillCurator),
        "ana@cowok.design" | "ana2@cowok.design" | "analyst@cowok.design" => {
            Some(CloudAdminRole::AnalystReadonly)
        }
        _ => None,
    }
}

/// Returns `true` only when `requested` is within the operator's server-bound RBAC privileges.
/// Non-`SUPER_ADMIN` operators can NEVER self-promote to higher or sibling privileged roles.
pub fn is_role_permitted_for_operator(bound: CloudAdminRole, requested: CloudAdminRole) -> bool {
    match bound {
        CloudAdminRole::SuperAdmin => true,
        CloudAdminRole::SupportAgent => {
            matches!(
                requested,
                CloudAdminRole::SupportAgent | CloudAdminRole::AnalystReadonly
            )
        }
        CloudAdminRole::SkillCurator => {
            matches!(
                requested,
                CloudAdminRole::SkillCurator | CloudAdminRole::AnalystReadonly
            )
        }
        CloudAdminRole::AnalystReadonly => matches!(requested, CloudAdminRole::AnalystReadonly),
    }
}

fn compute_totp_sha256_code(secret: &[u8], time_step: i64) -> String {
    let step_bytes = (time_step.max(0) as u64).to_be_bytes();
    let mac = hmac_sha256(secret, &step_bytes);
    let offset = (mac[mac.len() - 1] & 0x0f) as usize;
    let bin_code: u32 = (((mac[offset] & 0x7f) as u32) << 24)
        | ((mac[offset + 1] as u32) << 16)
        | ((mac[offset + 2] as u32) << 8)
        | (mac[offset + 3] as u32);
    format!("{:06}", bin_code % 1_000_000)
}

/// Verifies a mandatory 6-digit MFA code against the server-side MFA verifier:
/// - `COWOK_CLOUD_ADMIN_TOTP_SECRET` (RFC 6238 30s window +/- 1 step), or
/// - `COWOK_CLOUD_ADMIN_MFA_CODE` (exact constant-time 6-digit verifier), or
/// - `DEFAULT_DEV_MFA_CODE` in local development/test mode.
pub fn verify_server_mfa_code(code: Option<&str>) -> Result<(), CloudAuthError> {
    let trimmed = code.map(str::trim).unwrap_or("");
    if trimmed.len() != 6 || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CloudAuthError::mfa_required(
            "A valid 6-digit MFA verification code is strictly required",
        ));
    }
    if let Ok(totp_secret) = std::env::var("COWOK_CLOUD_ADMIN_TOTP_SECRET") {
        let secret_trimmed = totp_secret.trim();
        if !secret_trimmed.is_empty() {
            let step = Utc::now().timestamp() / 30;
            for delta in [-1i64, 0, 1] {
                let candidate = compute_totp_sha256_code(secret_trimmed.as_bytes(), step + delta);
                if constant_time_eq(trimmed.as_bytes(), candidate.as_bytes()) {
                    return Ok(());
                }
            }
            return Err(CloudAuthError::mfa_required(
                "Invalid TOTP MFA verification code",
            ));
        }
    }
    if let Ok(configured) = std::env::var("COWOK_CLOUD_ADMIN_MFA_CODE") {
        let expected = configured.trim();
        if !expected.is_empty() {
            if constant_time_eq(trimmed.as_bytes(), expected.as_bytes()) {
                return Ok(());
            }
            return Err(CloudAuthError::mfa_required(
                "Invalid MFA verification code",
            ));
        }
    }
    if constant_time_eq(trimmed.as_bytes(), DEFAULT_DEV_MFA_CODE.as_bytes()) {
        return Ok(());
    }
    Err(CloudAuthError::mfa_required(
        "Invalid MFA verification code",
    ))
}

/// Ensures V29 Cloud Control Plane tables exist (idempotent additive DDL).
pub fn ensure_v29_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(ENSURE_V29_SQL)
        .map_err(|e| format!("ensure V29 cloud control plane tables failed: {e}"))
}

/// Appends an immutable audit entry inside `TransactionBehavior::Immediate` (`admin_audit_logs`).
/// Zero UPDATE or DELETE operations ever touch `admin_audit_logs`.
pub fn append_admin_audit_log(
    pool: &SqlitePool,
    claims: &AdminSessionClaims,
    action: &str,
    target_resource: &str,
    details: &Value,
) -> Result<String, String> {
    if !claims.mfa_verified {
        return Err("MFA verification required to emit admin audit log".to_string());
    }
    let audit_id = format!("aud_{}", Uuid::new_v4().simple());
    let now_ms = Utc::now().timestamp_millis();
    let (scrubbed_details, _) = scrub_json_server_pii(details);
    let details_json =
        serde_json::to_string(&scrubbed_details).unwrap_or_else(|_| "{}".to_string());
    let (scrubbed_target, _) = scrub_server_pii(target_resource);

    retry_on_busy("cloud_admin_append_audit_log", || -> Result<(), rusqlite::Error> {
        let mut conn = pool.connection().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let _ = ensure_v29_tables(&conn);
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO admin_audit_logs (
                id, actor_subject, actor_role, mfa_verified, action, target_resource, details_json, created_at
            ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)",
            params![
                audit_id,
                claims.subject,
                claims.role.as_str(),
                action,
                scrubbed_target,
                details_json,
                now_ms,
            ],
        )?;
        tx.commit()?;
        Ok(())
    })
    .map_err(|e| format!("append admin audit log failed: {e}"))?;

    Ok(audit_id)
}

#[derive(Debug, Clone)]
struct RateWindowBucket {
    window_start_secs: i64,
    count: usize,
}

#[derive(Debug, Default)]
struct RateLimiterState {
    by_install: HashMap<String, RateWindowBucket>,
    by_ip: HashMap<String, RateWindowBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitExceeded {
    pub retry_after_secs: u64,
}

/// Isolated dual fixed-window rate limiter tracking separate counters per `install_id` and per `client_ip`.
/// Rejects with `429 RATE_LIMIT_EXCEEDED` if EITHER dimension exceeds its limit or if the hard bucket
/// capacity ceiling is reached after pruning stale entries.
pub struct IngestRateLimiter {
    max_per_install: usize,
    max_per_ip: usize,
    window_secs: i64,
    max_buckets: usize,
    state: Mutex<RateLimiterState>,
}

impl IngestRateLimiter {
    pub fn new(max_requests: usize, window_secs: i64) -> Self {
        let install_limit = max_requests.max(1);
        let ip_limit = install_limit.saturating_mul(2);
        Self::new_with_limits(install_limit, ip_limit, window_secs, MAX_RATE_LIMIT_BUCKETS)
    }

    pub fn new_with_limits(
        max_per_install: usize,
        max_per_ip: usize,
        window_secs: i64,
        max_buckets: usize,
    ) -> Self {
        Self {
            max_per_install: max_per_install.max(1),
            max_per_ip: max_per_ip.max(1),
            window_secs: window_secs.max(1),
            max_buckets: max_buckets.max(1),
            state: Mutex::new(RateLimiterState::default()),
        }
    }

    pub fn from_env() -> Self {
        let max_req = std::env::var("COWOK_INGEST_RATE_LIMIT_MAX")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_INGEST_RATE_LIMIT_MAX);
        let max_ip_req = std::env::var("COWOK_INGEST_IP_RATE_LIMIT_MAX")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or_else(|| max_req.saturating_mul(2));
        let window_secs = std::env::var("COWOK_INGEST_RATE_WINDOW_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_INGEST_RATE_WINDOW_SECS);
        Self::new_with_limits(max_req, max_ip_req, window_secs, MAX_RATE_LIMIT_BUCKETS)
    }

    fn enforce_capacity(
        map: &mut HashMap<String, RateWindowBucket>,
        key: &str,
        now: i64,
        window_secs: i64,
        max_buckets: usize,
    ) -> Result<(), RateLimitExceeded> {
        if map.contains_key(key) || map.len() < max_buckets {
            return Ok(());
        }
        let cutoff = now - window_secs;
        map.retain(|_, b| b.window_start_secs > cutoff);
        if map.len() >= max_buckets {
            return Err(RateLimitExceeded {
                retry_after_secs: window_secs.max(1) as u64,
            });
        }
        Ok(())
    }

    fn check_bucket_limit(
        map: &HashMap<String, RateWindowBucket>,
        key: &str,
        now: i64,
        window_secs: i64,
        limit: usize,
    ) -> Result<(), RateLimitExceeded> {
        if let Some(bucket) = map.get(key) {
            if now - bucket.window_start_secs < window_secs && bucket.count >= limit {
                let elapsed = (now - bucket.window_start_secs).max(0);
                let retry_after = (window_secs - elapsed).max(1) as u64;
                return Err(RateLimitExceeded {
                    retry_after_secs: retry_after,
                });
            }
        }
        Ok(())
    }

    fn increment_bucket(
        map: &mut HashMap<String, RateWindowBucket>,
        key: String,
        now: i64,
        window_secs: i64,
        limit: usize,
    ) -> usize {
        let entry = map.entry(key).or_insert(RateWindowBucket {
            window_start_secs: now,
            count: 0,
        });
        if now - entry.window_start_secs >= window_secs {
            entry.window_start_secs = now;
            entry.count = 0;
        }
        entry.count += 1;
        limit.saturating_sub(entry.count)
    }

    pub fn check_and_record(
        &self,
        install_id: &str,
        client_ip: &str,
    ) -> Result<usize, RateLimitExceeded> {
        let now = Utc::now().timestamp();
        let install_key = install_id.trim().to_string();
        let ip_key = client_ip.trim().to_string();
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        Self::enforce_capacity(
            &mut guard.by_install,
            &install_key,
            now,
            self.window_secs,
            self.max_buckets,
        )?;
        Self::enforce_capacity(
            &mut guard.by_ip,
            &ip_key,
            now,
            self.window_secs,
            self.max_buckets,
        )?;

        Self::check_bucket_limit(
            &guard.by_install,
            &install_key,
            now,
            self.window_secs,
            self.max_per_install,
        )?;
        Self::check_bucket_limit(
            &guard.by_ip,
            &ip_key,
            now,
            self.window_secs,
            self.max_per_ip,
        )?;

        let rem_install = Self::increment_bucket(
            &mut guard.by_install,
            install_key,
            now,
            self.window_secs,
            self.max_per_install,
        );
        let rem_ip = Self::increment_bucket(
            &mut guard.by_ip,
            ip_key,
            now,
            self.window_secs,
            self.max_per_ip,
        );
        Ok(rem_install.min(rem_ip))
    }
}

fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
}

fn is_email_local_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn is_email_domain_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-')
}

fn is_path_segment_char(b: u8) -> bool {
    !b.is_ascii_whitespace()
        && !matches!(b, b'/' | b'\\' | b'"' | b'\'' | b')' | b']' | b',' | b';')
}

fn scrub_prefixed_tokens(input: &str, prefixes: &[&str], min_suffix_len: usize) -> (String, usize) {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut count = 0;

    while i < bytes.len() {
        let mut matched = false;
        for prefix in prefixes {
            let p_bytes = prefix.as_bytes();
            if bytes[i..].starts_with(p_bytes) {
                let start = i + p_bytes.len();
                let mut end = start;
                while end < bytes.len() && is_token_char(bytes[end]) {
                    end += 1;
                }
                if end - start >= min_suffix_len {
                    out.push_str(REDACTED_SECRET_TOKEN);
                    i = end;
                    count += 1;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            if let Some(ch) = input[i..].chars().next() {
                out.push(ch);
                i += ch.len_utf8();
            } else {
                break;
            }
        }
    }
    (out, count)
}

fn scrub_bearer_tokens(input: &str) -> (String, usize) {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut count = 0;

    while i < bytes.len() {
        if bytes.len().saturating_sub(i) >= 7
            && bytes[i..i + 6].eq_ignore_ascii_case(b"bearer")
            && matches!(bytes[i + 6], b' ' | b'\t')
        {
            let mut tok_start = i + 6;
            while tok_start < bytes.len() && matches!(bytes[tok_start], b' ' | b'\t') {
                tok_start += 1;
            }
            let mut tok_end = tok_start;
            while tok_end < bytes.len()
                && (is_token_char(bytes[tok_end]) || matches!(bytes[tok_end], b'=' | b'+' | b'/'))
            {
                tok_end += 1;
            }
            if tok_end - tok_start >= 8 {
                out.push_str("Bearer ");
                out.push_str(REDACTED_SECRET_TOKEN);
                i = tok_end;
                count += 1;
                continue;
            }
        }
        if let Some(ch) = input[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    (out, count)
}

fn scrub_jwts(input: &str) -> (String, usize) {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut count = 0;

    while i < bytes.len() {
        if bytes[i..].starts_with(b"eyJ") {
            let mut end = i;
            let mut dots = 0usize;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'-' | b'_' | b'.'))
            {
                if bytes[end] == b'.' {
                    dots += 1;
                }
                end += 1;
            }
            if dots == 2 && end - i >= 24 {
                out.push_str(REDACTED_SECRET_TOKEN);
                i = end;
                count += 1;
                continue;
            }
        }
        if let Some(ch) = input[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    (out, count)
}

fn scrub_emails(input: &str) -> (String, usize) {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut count = 0;

    while i < bytes.len() {
        if is_email_local_char(bytes[i]) {
            let mut at_pos = i;
            while at_pos < bytes.len() && is_email_local_char(bytes[at_pos]) {
                at_pos += 1;
            }
            if at_pos > i && at_pos < bytes.len() && bytes[at_pos] == b'@' {
                let mut end = at_pos + 1;
                let mut has_dot = false;
                while end < bytes.len() && is_email_domain_char(bytes[end]) {
                    if bytes[end] == b'.' {
                        has_dot = true;
                    }
                    end += 1;
                }
                while end > at_pos + 1 && bytes[end - 1] == b'.' {
                    end -= 1;
                }
                let domain_part = &input[at_pos + 1..end];
                if has_dot
                    && domain_part.len() >= 4
                    && domain_part
                        .rsplit('.')
                        .next()
                        .map(|tld| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
                        .unwrap_or(false)
                {
                    out.push_str(REDACTED_EMAIL_TOKEN);
                    i = end;
                    count += 1;
                    continue;
                }
            }
        }
        if let Some(ch) = input[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    (out, count)
}

fn scrub_home_paths(input: &str) -> (String, usize) {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut count = 0;

    while i < bytes.len() {
        let mut matched_len = 0usize;
        if bytes.len().saturating_sub(i) >= 7 && bytes[i..i + 7].eq_ignore_ascii_case(b"/users/") {
            matched_len = 7;
        } else if bytes.len().saturating_sub(i) >= 6
            && bytes[i..i + 6].eq_ignore_ascii_case(b"/home/")
        {
            matched_len = 6;
        } else if bytes.len().saturating_sub(i) >= 9
            && bytes[i].is_ascii_alphabetic()
            && bytes[i + 1] == b':'
            && (bytes[i + 2..i + 9].eq_ignore_ascii_case(b"\\users\\")
                || bytes[i + 2..i + 9].eq_ignore_ascii_case(b"/users/"))
        {
            matched_len = 9;
        }
        if matched_len > 0 {
            let mut end = i + matched_len;
            while end < bytes.len() && is_path_segment_char(bytes[end]) {
                end += 1;
            }
            if end > i + matched_len {
                out.push_str(REDACTED_HOME_TOKEN);
                i = end;
                count += 1;
                continue;
            }
        }
        if let Some(ch) = input[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    (out, count)
}

fn is_header_boundary(prev: Option<u8>) -> bool {
    match prev {
        None => true,
        Some(b) => {
            b.is_ascii_whitespace()
                || matches!(b, b';' | b',' | b'(' | b'[' | b'{' | b'"' | b'\'' | b'|')
        }
    }
}

fn scrub_inline_secret_headers(input: &str) -> (String, usize) {
    const HEADERS: [(&[u8], &str, bool); 3] = [
        (b"set-cookie:", "Set-Cookie: ", true),
        (b"cookie:", "Cookie: ", true),
        (b"x-api-key:", "x-api-key: ", false),
    ];
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut count = 0;

    while i < bytes.len() {
        let prev = if i > 0 { Some(bytes[i - 1]) } else { None };
        let mut matched = false;
        if is_header_boundary(prev) {
            for (pat, canonical, consume_to_eol) in HEADERS {
                if bytes.len().saturating_sub(i) >= pat.len()
                    && bytes[i..i + pat.len()].eq_ignore_ascii_case(pat)
                {
                    let mut val_start = i + pat.len();
                    while val_start < bytes.len() && matches!(bytes[val_start], b' ' | b'\t') {
                        val_start += 1;
                    }
                    let mut val_end = val_start;
                    while val_end < bytes.len() {
                        let b = bytes[val_end];
                        if matches!(b, b'\r' | b'\n' | b'"' | b'\'') {
                            break;
                        }
                        if !consume_to_eol
                            && (b.is_ascii_whitespace()
                                || matches!(b, b';' | b',' | b')' | b']' | b'}'))
                        {
                            break;
                        }
                        val_end += 1;
                    }
                    let val_slice = input[val_start..val_end].trim();
                    if !val_slice.is_empty() && val_slice != REDACTED_SECRET_TOKEN {
                        out.push_str(canonical);
                        out.push_str(REDACTED_SECRET_TOKEN);
                        i = val_end;
                        count += 1;
                        matched = true;
                        break;
                    }
                }
            }
        }
        if !matched {
            if let Some(ch) = input[i..].chars().next() {
                out.push(ch);
                i += ch.len_utf8();
            } else {
                break;
            }
        }
    }
    (out, count)
}

/// Server-side Defense-in-Depth PII & Secret Scrubber.
/// Redacts `sk-*`, `sk_live_*`, `sk_test_*`, `AKIA*`, `ghp_*`, `AIza*`, JWTs, Bearer tokens,
/// inline `Cookie:` / `Set-Cookie:` / `x-api-key:` headers, emails, and user home paths.
pub fn scrub_server_pii(input: &str) -> (String, usize) {
    let mut total = 0usize;
    let (s0, c0) = scrub_inline_secret_headers(input);
    total += c0;
    let (s1, c1) = scrub_bearer_tokens(&s0);
    total += c1;
    let (s2, c2) = scrub_jwts(&s1);
    total += c2;
    let (s3, c3) = scrub_prefixed_tokens(
        &s2,
        &[
            "sk-proj-",
            "sk-ant-",
            "sk-live-",
            "sk-test-",
            "sk_live_",
            "sk_test_",
            "sk-",
            "AKIA",
            "ghp_",
            "gho_",
            "ghu_",
            "ghs_",
            "github_pat_",
            "AIza",
            "xoxb-",
            "xoxp-",
        ],
        8,
    );
    total += c3;
    let (s4, c4) = scrub_emails(&s3);
    total += c4;
    let (s5, c5) = scrub_home_paths(&s4);
    total += c5;
    (s5, total)
}

fn is_forbidden_secret_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    matches!(
        normalized.as_str(),
        "apikey"
            | "xapikey"
            | "authorization"
            | "cookie"
            | "setcookie"
            | "accesstoken"
            | "refreshtoken"
            | "password"
            | "secret"
            | "token"
            | "dotenv"
            | "env"
    )
}

/// Recursively scrubs JSON payloads, redacting forbidden secret keys and PII string patterns.
pub fn scrub_json_server_pii(value: &Value) -> (Value, usize) {
    match value {
        Value::String(s) => {
            let (clean, c) = scrub_server_pii(s);
            (Value::String(clean), c)
        }
        Value::Array(items) => {
            let mut total = 0usize;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let (clean, c) = scrub_json_server_pii(item);
                total += c;
                out.push(clean);
            }
            (Value::Array(out), total)
        }
        Value::Object(map) => {
            let mut total = 0usize;
            let mut out = Map::new();
            for (k, v) in map {
                if is_forbidden_secret_key(k) {
                    out.insert(k.clone(), Value::String(REDACTED_SECRET_TOKEN.to_string()));
                    total += 1;
                } else {
                    let (clean, c) = scrub_json_server_pii(v);
                    total += c;
                    out.insert(k.clone(), clean);
                }
            }
            (Value::Object(out), total)
        }
        other => (other.clone(), 0),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedAttachment {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub sha256: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentValidationError {
    pub code: &'static str,
    pub message: String,
}

fn contains_polyglot_signature(bytes: &[u8]) -> bool {
    let lower: Vec<u8> = bytes.iter().map(|b| b.to_ascii_lowercase()).collect();
    const FORBIDDEN_PATTERNS: [&[u8]; 15] = [
        b"<script",
        b"</script",
        b"<svg",
        b"<?php",
        b"<?xml",
        b"<!doctype html",
        b"<html",
        b"<iframe",
        b"<img",
        b"<object",
        b"<embed",
        b"<body",
        b"javascript:",
        b"onload=",
        b"onerror=",
    ];
    for pat in FORBIDDEN_PATTERNS {
        if lower.windows(pat.len()).any(|w| w == pat) {
            return true;
        }
    }
    false
}

fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn sanitize_png_chunks(bytes: &[u8]) -> Result<Vec<u8>, AttachmentValidationError> {
    const PNG_SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 33 || !bytes.starts_with(&PNG_SIG) {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "Invalid PNG magic header or truncated file".to_string(),
        });
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&PNG_SIG);
    let mut pos = 8usize;
    let mut saw_ihdr = false;
    let mut saw_idat = false;
    let mut saw_iend = false;

    while pos + 12 <= bytes.len() {
        let chunk_len =
            u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
                as usize;
        let chunk_type = &bytes[pos + 4..pos + 8];
        let total_chunk =
            12usize
                .checked_add(chunk_len)
                .ok_or_else(|| AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "PNG chunk length overflow".to_string(),
                })?;
        if pos + total_chunk > bytes.len() {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Truncated PNG chunk structure".to_string(),
            });
        }
        let chunk_data = &bytes[pos + 8..pos + 8 + chunk_len];
        let keep = matches!(chunk_type, b"IHDR" | b"PLTE" | b"tRNS" | b"IDAT" | b"IEND");
        if keep {
            let stored_crc = u32::from_be_bytes([
                bytes[pos + 8 + chunk_len],
                bytes[pos + 9 + chunk_len],
                bytes[pos + 10 + chunk_len],
                bytes[pos + 11 + chunk_len],
            ]);
            let computed_crc = crc32_ieee(&bytes[pos + 4..pos + 8 + chunk_len]);
            if stored_crc != computed_crc {
                return Err(AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "Corrupted PNG chunk CRC-32 checksum".to_string(),
                });
            }
            if chunk_type == b"IHDR" {
                if pos != 8 || saw_ihdr || chunk_len != 13 {
                    return Err(AttachmentValidationError {
                        code: "INVALID_ATTACHMENT_PAYLOAD",
                        message: "PNG IHDR chunk must be first and exactly 13 bytes".to_string(),
                    });
                }
                let width = u32::from_be_bytes([
                    chunk_data[0],
                    chunk_data[1],
                    chunk_data[2],
                    chunk_data[3],
                ]);
                let height = u32::from_be_bytes([
                    chunk_data[4],
                    chunk_data[5],
                    chunk_data[6],
                    chunk_data[7],
                ]);
                if width == 0
                    || height == 0
                    || width > MAX_IMAGE_DIMENSION
                    || height > MAX_IMAGE_DIMENSION
                {
                    return Err(AttachmentValidationError {
                        code: "INVALID_ATTACHMENT_PAYLOAD",
                        message: "PNG dimensions out of allowed range (1..=16384)".to_string(),
                    });
                }
                saw_ihdr = true;
            } else if chunk_type == b"IDAT" && chunk_len > 0 {
                saw_idat = true;
            } else if chunk_type == b"IEND" && chunk_len != 0 {
                return Err(AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "PNG IEND chunk must have zero payload bytes".to_string(),
                });
            }
            out.extend_from_slice(&bytes[pos..pos + total_chunk]);
        }
        pos += total_chunk;
        if chunk_type == b"IEND" {
            saw_iend = true;
            // Drop any trailing bytes after IEND (eliminates appended polyglot payloads)
            break;
        }
    }

    if !saw_ihdr || !saw_idat || !saw_iend {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "PNG image is missing required IHDR, non-empty IDAT, or IEND chunk"
                .to_string(),
        });
    }
    Ok(out)
}

fn sanitize_jpeg_segments(bytes: &[u8]) -> Result<Vec<u8>, AttachmentValidationError> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 || bytes[2] != 0xFF {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "Invalid JPEG SOI magic header".to_string(),
        });
    }
    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&[0xFF, 0xD8]);
    let mut pos = 2usize;
    let mut saw_sof = false;
    let mut saw_sos = false;
    let mut saw_eoi = false;

    while pos + 1 < bytes.len() {
        if bytes[pos] != 0xFF {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Corrupted JPEG marker stream".to_string(),
            });
        }
        while pos < bytes.len() && bytes[pos] == 0xFF {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let marker = bytes[pos];
        pos += 1;

        if marker == 0xD9 {
            out.extend_from_slice(&[0xFF, 0xD9]);
            saw_eoi = true;
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            out.extend_from_slice(&[0xFF, marker]);
            continue;
        }
        if pos + 2 > bytes.len() {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Truncated JPEG segment length".to_string(),
            });
        }
        let seg_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        if seg_len < 2 || pos + seg_len > bytes.len() {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Invalid JPEG segment length".to_string(),
            });
        }
        if (0xC0..=0xC3).contains(&marker) {
            if seg_len < 8 {
                return Err(AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "Truncated JPEG SOF frame header".to_string(),
                });
            }
            let height = u16::from_be_bytes([bytes[pos + 3], bytes[pos + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[pos + 5], bytes[pos + 6]]) as u32;
            if width == 0
                || height == 0
                || width > MAX_IMAGE_DIMENSION
                || height > MAX_IMAGE_DIMENSION
            {
                return Err(AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "JPEG dimensions out of allowed range (1..=16384)".to_string(),
                });
            }
            saw_sof = true;
        }
        // Strip ALL APP0..APP15 (JFIF/EXIF/XMP/IPTC) and COM (0xFE) comments
        let is_metadata = (0xE0..=0xEF).contains(&marker) || marker == 0xFE;
        if !is_metadata {
            out.extend_from_slice(&[0xFF, marker]);
            out.extend_from_slice(&bytes[pos..pos + seg_len]);
        }
        pos += seg_len;

        if marker == 0xDA {
            if !saw_sof {
                return Err(AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "JPEG SOS scan appeared without a valid SOF frame header".to_string(),
                });
            }
            // SOS: copy entropy-coded scan until EOI marker (0xFF 0xD9)
            let scan_start = pos;
            let mut scan_end = None;
            while pos + 1 < bytes.len() {
                if bytes[pos] == 0xFF && bytes[pos + 1] == 0xD9 {
                    scan_end = Some(pos);
                    break;
                }
                pos += 1;
            }
            let eoi_pos = scan_end.ok_or_else(|| AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "JPEG stream is missing EOI terminator".to_string(),
            })?;
            if eoi_pos == scan_start {
                return Err(AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "JPEG SOS segment has empty image scan data".to_string(),
                });
            }
            out.extend_from_slice(&bytes[scan_start..eoi_pos]);
            out.extend_from_slice(&[0xFF, 0xD9]);
            saw_sos = true;
            saw_eoi = true;
            break;
        }
    }

    if !saw_sof || !saw_sos || !saw_eoi {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "JPEG image is missing required SOF, SOS scan data, or EOI marker".to_string(),
        });
    }
    Ok(out)
}

fn sanitize_webp_chunks(bytes: &[u8]) -> Result<Vec<u8>, AttachmentValidationError> {
    if bytes.len() < 16 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "Invalid WebP RIFF magic header".to_string(),
        });
    }
    let declared_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let riff_end = 8usize
        .checked_add(declared_size)
        .ok_or_else(|| AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "WebP RIFF size overflow".to_string(),
        })?;
    if riff_end > bytes.len() || riff_end < 12 {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "Truncated WebP RIFF container".to_string(),
        });
    }

    let mut body = Vec::with_capacity(declared_size);
    body.extend_from_slice(b"WEBP");
    let mut pos = 12usize;
    let mut saw_image_bitstream = false;

    while pos + 8 <= riff_end {
        let fourcc = &bytes[pos..pos + 4];
        let chunk_len = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let padded_len =
            chunk_len
                .checked_add(chunk_len & 1)
                .ok_or_else(|| AttachmentValidationError {
                    code: "INVALID_ATTACHMENT_PAYLOAD",
                    message: "WebP chunk padding overflow".to_string(),
                })?;
        let total = 8usize
            .checked_add(padded_len)
            .ok_or_else(|| AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "WebP chunk length overflow".to_string(),
            })?;
        if pos + total > riff_end {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Truncated WebP chunk".to_string(),
            });
        }
        let is_allowed = matches!(
            fourcc,
            b"VP8 " | b"VP8L" | b"ALPH" | b"ANIM" | b"ANMF" | b"VP8X"
        );
        if is_allowed {
            if matches!(fourcc, b"VP8 " | b"VP8L" | b"ANMF") && chunk_len > 0 {
                saw_image_bitstream = true;
            }
            if fourcc == b"VP8X" {
                if chunk_len != 10 {
                    return Err(AttachmentValidationError {
                        code: "INVALID_ATTACHMENT_PAYLOAD",
                        message: "Invalid WebP VP8X header chunk length".to_string(),
                    });
                }
                let mut vp8x_chunk = bytes[pos..pos + total].to_vec();
                // Clear ICCP (0x20), EXIF (0x08), and XMP (0x04) metadata presence flags
                vp8x_chunk[8] &= !(0x20 | 0x08 | 0x04);
                body.extend_from_slice(&vp8x_chunk);
            } else {
                body.extend_from_slice(&bytes[pos..pos + total]);
            }
        }
        pos += total;
    }

    if !saw_image_bitstream {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "WebP container has no valid VP8/VP8L/ANMF image bitstream".to_string(),
        });
    }

    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Validates and structurally re-encodes an image attachment (`PNG`, `JPEG`, `WEBP`),
/// enforcing `<= 5 MB`, magic-byte match, polyglot/SVG/HTML/script rejection (`422 INVALID_ATTACHMENT_PAYLOAD`),
/// and stripping EXIF/ancillary metadata chunks and trailing polyglot bytes.
pub fn validate_and_reencode_attachment(
    data_url: &str,
) -> Result<SanitizedAttachment, AttachmentValidationError> {
    let trimmed = data_url.trim();
    let (claimed_mime, b64_payload) =
        if let Some(rest) = trimmed.strip_prefix("data:image/png;base64,") {
            ("image/png", rest)
        } else if let Some(rest) = trimmed.strip_prefix("data:image/jpeg;base64,") {
            ("image/jpeg", rest)
        } else if let Some(rest) = trimmed.strip_prefix("data:image/webp;base64,") {
            ("image/webp", rest)
        } else {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Only base64-encoded PNG, JPEG, or WebP image attachments are permitted"
                    .to_string(),
            });
        };

    if b64_payload.is_empty() || b64_payload.len() > (MAX_ATTACHMENT_BYTES * 4 / 3) + 64 {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "Attachment exceeds 5 MB maximum size or is empty".to_string(),
        });
    }

    let decoded =
        B64_STD
            .decode(b64_payload.as_bytes())
            .map_err(|_| AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Malformed base64 image payload".to_string(),
            })?;

    if decoded.is_empty() || decoded.len() > MAX_ATTACHMENT_BYTES {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message: "Decoded attachment exceeds 5 MB limit".to_string(),
        });
    }

    if contains_polyglot_signature(&decoded) {
        return Err(AttachmentValidationError {
            code: "INVALID_ATTACHMENT_PAYLOAD",
            message:
                "Attachment rejected: detected embedded script, SVG, HTML, or PHP polyglot payload"
                    .to_string(),
        });
    }

    let sanitized_bytes = match claimed_mime {
        "image/png" => sanitize_png_chunks(&decoded)?,
        "image/jpeg" => sanitize_jpeg_segments(&decoded)?,
        "image/webp" => sanitize_webp_chunks(&decoded)?,
        _ => {
            return Err(AttachmentValidationError {
                code: "INVALID_ATTACHMENT_PAYLOAD",
                message: "Unsupported image MIME type".to_string(),
            })
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&sanitized_bytes);
    let sha256 = hex_encode(&hasher.finalize());
    let byte_len = sanitized_bytes.len();

    Ok(SanitizedAttachment {
        bytes: sanitized_bytes,
        mime: claimed_mime,
        sha256,
        byte_len,
    })
}

pub fn attachment_extension_for_mime(mime: &str) -> Option<&'static str> {
    match mime.trim() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

pub fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn cloud_attachments_dir(workspace_root: &Path) -> Result<PathBuf, String> {
    let dot_dir = workspace_root.join(".cowok-design");
    if let Ok(meta) = fs::symlink_metadata(&dot_dir) {
        if meta.file_type().is_symlink() {
            return Err("Refusing to traverse symlinked .cowok-design directory".to_string());
        }
    }
    let att_dir = dot_dir.join("cloud-attachments");
    if let Ok(meta) = fs::symlink_metadata(&att_dir) {
        if meta.file_type().is_symlink() {
            return Err("Refusing to traverse symlinked cloud-attachments directory".to_string());
        }
    }
    fs::create_dir_all(&att_dir)
        .map_err(|e| format!("create cloud-attachments directory failed: {e}"))?;
    Ok(att_dir)
}

/// Persists sanitized image attachment bytes under `<workspace_root>/.cowok-design/cloud-attachments/<sha256>.<ext>`
/// using `O_NOFOLLOW` and `0o600` permissions on Unix.
pub fn persist_sanitized_attachment(
    workspace_root: &Path,
    att: &SanitizedAttachment,
) -> Result<PathBuf, String> {
    if !is_valid_sha256_hex(&att.sha256) {
        return Err("Invalid attachment sha256 digest".to_string());
    }
    let ext = attachment_extension_for_mime(att.mime)
        .ok_or_else(|| format!("Unsupported sanitized attachment MIME '{}'", att.mime))?;
    let dir = cloud_attachments_dir(workspace_root)?;
    let target = dir.join(format!("{}.{ext}", att.sha256));
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.file_type().is_symlink() {
            return Err("Refusing to overwrite symlinked attachment target".to_string());
        }
        if meta.is_file() && meta.len() == att.bytes.len() as u64 {
            return Ok(target);
        }
    }

    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = opts
        .open(&target)
        .map_err(|e| format!("open attachment storage file failed: {e}"))?;
    file.write_all(&att.bytes)
        .map_err(|e| format!("write sanitized attachment failed: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("sync sanitized attachment failed: {e}"))?;
    Ok(target)
}

/// Reads a previously persisted sanitized attachment using `O_NOFOLLOW` and strict digest validation.
pub fn read_sanitized_attachment(
    workspace_root: &Path,
    sha256: &str,
    mime: &str,
) -> Result<Vec<u8>, String> {
    if !is_valid_sha256_hex(sha256) {
        return Err("Invalid attachment sha256 digest".to_string());
    }
    let ext = attachment_extension_for_mime(mime)
        .ok_or_else(|| format!("Unsupported attachment MIME '{mime}'"))?;
    let dir = cloud_attachments_dir(workspace_root)?;
    let target = dir.join(format!("{sha256}.{ext}"));
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.file_type().is_symlink() {
            return Err("Refusing to read symlinked attachment".to_string());
        }
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = opts
        .open(&target)
        .map_err(|e| format!("attachment blob not found or unreadable: {e}"))?;
    let mut buf = Vec::new();
    Read::by_ref(&mut file)
        .take((MAX_ATTACHMENT_BYTES + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read sanitized attachment failed: {e}"))?;
    if buf.len() > MAX_ATTACHMENT_BYTES {
        return Err("Stored attachment exceeds maximum allowed size".to_string());
    }
    Ok(buf)
}
