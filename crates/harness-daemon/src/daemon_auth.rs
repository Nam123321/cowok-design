// @story 04.1
// @implements AC-06
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use domain::HarnessError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::lockfile::{generate_csprng_hex_32, read_daemon_lock, DaemonLockInfo};
use crate::routes::{
    constant_time_eq, decode_hex, hmac_sha256, state_auth, DaemonAppState, HttpErrorBody,
};

const MAX_SKEW_SECS: i64 = 30;
const NONCE_LRU_CAPACITY: usize = 10_000;
const SESSION_TTL_SECS: i64 = 28_800;
const MAX_BODY_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowPrincipal {
    #[serde(default = "default_tenant_id")]
    pub tenant_id: String,
    #[serde(default = "default_user_id")]
    pub user_id: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_tenant_id() -> String {
    "local-default".to_string()
}
fn default_user_id() -> String {
    "local-operator".to_string()
}
fn default_role() -> String {
    "editor".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSessionRequest {
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSessionResponse {
    pub session_token: String,
    pub tenant_id: String,
    pub user_id: String,
    pub role: String,
    pub expires_at: i64,
}

#[derive(Clone)]
struct SessionEntry {
    principal: FlowPrincipal,
    expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyAuthKind {
    HmacV2,
    SessionToken,
    Bearer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLegacyIngress {
    pub principal: FlowPrincipal,
    pub kind: LegacyAuthKind,
    pub bearer_token: Option<String>,
}

pub struct DaemonAuthState {
    pub daemon_nonce: String,
    hmac_key: Vec<u8>,
    nonce_order: Mutex<VecDeque<String>>,
    nonce_set: Mutex<HashSet<String>>,
    sessions: Mutex<HashMap<String, SessionEntry>>,
}

impl DaemonAuthState {
    pub fn from_lock_info(lock: &DaemonLockInfo) -> Result<Arc<Self>, HarnessError> {
        let key_bytes =
            decode_hex(lock.hmac_key_hex.trim()).ok_or_else(|| HarnessError::ValidationFailed {
                reason: "DAEMON_LOCK_INVALID_KEY: invalid hmac_key_hex".to_string(),
            })?;
        if key_bytes.len() < 16 || lock.daemon_nonce.trim().is_empty() {
            return Err(HarnessError::ValidationFailed {
                reason: "DAEMON_LOCK_INVALID_SECRET: missing daemon_nonce or short hmac_key"
                    .to_string(),
            });
        }
        Ok(Self::with_secrets(lock.daemon_nonce.clone(), key_bytes))
    }

    pub fn from_registry_root(workspace_root: &std::path::Path) -> Arc<Self> {
        if let Ok(lock) = read_daemon_lock(workspace_root) {
            if let Ok(state) = Self::from_lock_info(&lock) {
                return state;
            }
        }
        let mut key_bytes = Vec::with_capacity(32);
        key_bytes.extend_from_slice(Uuid::new_v4().as_bytes());
        key_bytes.extend_from_slice(Uuid::new_v4().as_bytes());
        Self::with_secrets(generate_csprng_hex_32(), key_bytes)
    }

    pub fn with_secrets(daemon_nonce: String, hmac_key: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            daemon_nonce,
            hmac_key,
            nonce_order: Mutex::new(VecDeque::new()),
            nonce_set: Mutex::new(HashSet::new()),
            sessions: Mutex::new(HashMap::new()),
        })
    }

    /// Signs a canonical 8-field HMAC-SHA256 v2 tuple `(x-cowok-timestamp, x-cowok-nonce, x-cowok-principal, x-cowok-hmac-sig)`.
    pub fn sign_hmac_v2_headers(
        &self,
        method: &Method,
        path: &str,
        query: &str,
        principal: &FlowPrincipal,
        raw_body: &[u8],
    ) -> [(String, String); 4] {
        let ts = Utc::now().timestamp().to_string();
        let req_nonce = Uuid::new_v4().to_string();
        let principal_b64 =
            B64_STD.encode(serde_json::to_vec(principal).unwrap_or_else(|_| b"{}".to_vec()));
        let canonical = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{:x}",
            self.daemon_nonce,
            ts,
            req_nonce,
            method.as_str().to_ascii_uppercase(),
            path,
            query,
            principal_b64,
            Sha256::digest(raw_body)
        );
        let sig_hex: String = hmac_sha256(&self.hmac_key, canonical.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        [
            ("x-cowok-timestamp".to_string(), ts),
            ("x-cowok-nonce".to_string(), req_nonce),
            ("x-cowok-principal".to_string(), principal_b64),
            ("x-cowok-hmac-sig".to_string(), sig_hex),
        ]
    }

    pub fn record_nonce_once(&self, nonce: &str) -> bool {
        let mut set = self.nonce_set.lock().expect("nonce_set");
        if set.contains(nonce) {
            return false;
        }
        let mut order = self.nonce_order.lock().expect("nonce_order");
        if order.len() >= NONCE_LRU_CAPACITY {
            if let Some(evicted) = order.pop_front() {
                set.remove(&evicted);
            }
        }
        set.insert(nonce.to_string());
        order.push_back(nonce.to_string());
        true
    }
}

pub fn is_allowed_loopback_host(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    h == "127.0.0.1"
        || h == "localhost"
        || h.strip_prefix("127.0.0.1:")
            .is_some_and(|p| p.parse::<u16>().is_ok())
        || h.strip_prefix("localhost:")
            .is_some_and(|p| p.parse::<u16>().is_ok())
}

fn verify_loopback_headers(
    headers: &HeaderMap,
) -> Result<(), (StatusCode, &'static str, &'static str)> {
    if let Some(h) = headers.get("host").and_then(|v| v.to_str().ok()) {
        if !is_allowed_loopback_host(h) {
            return Err((
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Non-loopback Host rejected",
            ));
        }
    }
    if let Some(sfs) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        if sfs != "same-origin" && sfs != "none" {
            return Err((
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Cross-site Sec-Fetch-Site rejected",
            ));
        }
    }
    if let Some(orig) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let trimmed = orig
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        if !is_allowed_loopback_host(trimmed) {
            return Err((
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Non-loopback Origin rejected",
            ));
        }
    }
    Ok(())
}

pub async fn handle_issue_local_session(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    body_opt: Option<Json<LocalSessionRequest>>,
) -> Result<Json<LocalSessionResponse>, Response> {
    let has_loopback_host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .is_some_and(is_allowed_loopback_host);
    if !has_loopback_host {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "ORIGIN_REJECTED",
            "Session bootstrap requires an explicit loopback Host header",
        ));
    }
    if let Err((st, code, msg)) = verify_loopback_headers(&headers) {
        return Err(auth_err_resp(st, code, msg));
    }
    let req = body_opt.map(|Json(v)| v).unwrap_or_default();
    let role = req.role.unwrap_or_else(default_role);
    if !matches!(role.as_str(), "viewer" | "editor" | "admin") {
        return Err(auth_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid role",
        ));
    }
    let requested_tenant = req
        .tenant_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(default_tenant_id);
    let auth = state_auth(&state);
    let has_daemon_nonce = headers
        .get("x-cowok-daemon-nonce")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .is_some_and(|n| {
            !n.is_empty() && constant_time_eq(n.as_bytes(), auth.daemon_nonce.as_bytes())
        });
    let is_superadmin = is_valid_superadmin_bearer(&headers);

    if role == "admin" && !is_superadmin {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_ROLE",
            "Issuing an admin local session requires valid superadmin authorization",
        ));
    }
    if (role == "editor" || requested_tenant != default_tenant_id())
        && !is_superadmin
        && !has_daemon_nonce
    {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_ROLE",
            "Elevated role or cross-tenant local session requires daemon nonce proof or superadmin authorization",
        ));
    }
    let principal = FlowPrincipal {
        tenant_id: requested_tenant,
        user_id: req
            .user_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_user_id),
        role: role.clone(),
    };
    let token = format!("cwk_sess_{}", generate_csprng_hex_32());
    let expires_at = Utc::now().timestamp() + SESSION_TTL_SECS;
    state_auth(&state)
        .sessions
        .lock()
        .expect("sessions")
        .insert(
            token.clone(),
            SessionEntry {
                principal: principal.clone(),
                expires_at,
            },
        );
    Ok(Json(LocalSessionResponse {
        session_token: token,
        tenant_id: principal.tenant_id,
        user_id: principal.user_id,
        role,
        expires_at,
    }))
}

pub async fn flow_dual_ingress_auth_middleware(
    State(state): State<DaemonAppState>,
    req: Request,
    next: Next,
) -> Response {
    if let Err((st, code, msg)) = verify_loopback_headers(req.headers()) {
        return auth_err_resp(st, code, msg);
    }
    let (parts, body) = req.into_parts();
    let raw_bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(_) => {
            return auth_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "Body exceeds 1 MiB",
            )
        }
    };
    let auth = state_auth(&state);
    let principal = if parts.headers.contains_key("x-cowok-hmac-sig") {
        match verify_hmac_v2(&auth, &parts.method, &parts.uri, &parts.headers, &raw_bytes) {
            Ok(p) => p,
            Err((st, code, msg)) => return auth_err_resp(st, code, msg),
        }
    } else if let Some(tok) = parts
        .headers
        .get("x-cowok-session-token")
        .and_then(|v| v.to_str().ok())
    {
        let found = auth
            .sessions
            .lock()
            .expect("sessions")
            .get(tok.trim())
            .cloned();
        match found {
            Some(entry) if entry.expires_at >= Utc::now().timestamp() => entry.principal,
            _ => {
                return auth_err_resp(
                    StatusCode::UNAUTHORIZED,
                    "UNAUTHENTICATED",
                    "Invalid session token",
                )
            }
        }
    } else {
        return auth_err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Missing authentication",
        );
    };
    if principal.role == "viewer" && parts.method != Method::GET {
        return auth_err_resp(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_ROLE",
            "Viewer role cannot mutate flow graph",
        );
    }
    let mut rebuilt = Request::from_parts(parts, Body::from(raw_bytes));
    rebuilt.extensions_mut().insert(principal);
    next.run(rebuilt).await
}

fn verify_hmac_v2(
    auth: &DaemonAuthState,
    method: &Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> Result<FlowPrincipal, (StatusCode, &'static str, &'static str)> {
    let sig_hex = hdr_str(headers, "x-cowok-hmac-sig")?;
    let ts_str = hdr_str(headers, "x-cowok-timestamp")?;
    let req_nonce = hdr_str(headers, "x-cowok-nonce")?;
    let principal_b64 = hdr_str(headers, "x-cowok-principal")?;
    let ts: i64 = ts_str.parse().map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Invalid timestamp",
        )
    })?;
    if (Utc::now().timestamp() - ts).abs() > MAX_SKEW_SECS {
        return Err((
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Timestamp outside +-30s skew",
        ));
    }
    Uuid::parse_str(req_nonce).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Nonce must be UUID",
        )
    })?;
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{:x}",
        auth.daemon_nonce,
        ts_str,
        req_nonce,
        method.as_str().to_ascii_uppercase(),
        uri.path(),
        uri.query().unwrap_or(""),
        principal_b64,
        Sha256::digest(raw_body)
    );
    let expected_hex: String = hmac_sha256(&auth.hmac_key, canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if sig_hex.len() != expected_hex.len()
        || sig_hex
            .bytes()
            .zip(expected_hex.bytes())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            != 0
    {
        return Err((
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Invalid HMAC-SHA256 v2 signature",
        ));
    }
    if !auth.record_nonce_once(req_nonce) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Replayed request nonce rejected",
        ));
    }
    let decoded = B64_STD.decode(principal_b64).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Invalid principal base64",
        )
    })?;
    let principal: FlowPrincipal = serde_json::from_slice(&decoded).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Invalid principal JSON",
        )
    })?;
    if !matches!(principal.role.as_str(), "viewer" | "editor" | "admin") {
        return Err((
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Invalid principal role",
        ));
    }
    Ok(principal)
}

fn hdr_str<'a>(
    headers: &'a HeaderMap,
    key: &str,
) -> Result<&'a str, (StatusCode, &'static str, &'static str)> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Missing HMAC header",
        ))
}

pub fn auth_err_resp(status: StatusCode, code: &str, message: &str) -> Response {
    let kind = if status == StatusCode::UNAUTHORIZED {
        "UNAUTHORIZED"
    } else {
        code
    };
    (
        status,
        Json(HttpErrorBody {
            code: code.to_string(),
            kind: Some(kind.to_string()),
            message: message.to_string(),
            details: serde_json::Value::Null,
            request_id: Uuid::new_v4().to_string(),
        }),
    )
        .into_response()
}

pub(crate) fn is_valid_superadmin_bearer(headers: &HeaderMap) -> bool {
    let Some(auth_hdr) = headers.get("authorization").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(token) = auth_hdr.strip_prefix("Bearer ").map(str::trim) else {
        return false;
    };
    if token.is_empty() {
        return false;
    }
    match std::env::var("COWOK_SUPERADMIN_SECRET") {
        Ok(ref secret) if !secret.trim().is_empty() => {
            constant_time_eq(token.as_bytes(), secret.trim().as_bytes())
        }
        _ => false,
    }
}

pub async fn cors_and_loopback_middleware(req: Request, next: Next) -> Response {
    if let Err((st, code, msg)) = verify_loopback_headers(req.headers()) {
        return auth_err_resp(st, code, msg);
    }
    let origin_hdr = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    if req.method() == Method::OPTIONS {
        let mut resp = StatusCode::NO_CONTENT.into_response();
        attach_loopback_cors_headers(resp.headers_mut(), origin_hdr.as_deref());
        return resp;
    }

    let mut resp = next.run(req).await;
    attach_loopback_cors_headers(resp.headers_mut(), origin_hdr.as_deref());
    resp
}

fn attach_loopback_cors_headers(headers: &mut HeaderMap, origin: Option<&str>) {
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    if let Some(orig) = origin {
        let host_part = orig
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        if is_allowed_loopback_host(host_part) {
            if let Ok(val) = HeaderValue::from_str(orig) {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, val);
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
                );
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_static(
                        "Authorization, Content-Type, X-Cowok-Session-Token, X-Cowok-Workspace-Root, X-Cowok-Actor-Id, X-Cowok-Portal, X-Correlation-Id",
                    ),
                );
            }
        }
    }
}

pub async fn legacy_project_auth_middleware(
    State(state): State<DaemonAppState>,
    req: Request,
    next: Next,
) -> Response {
    if let Err((st, code, msg)) = verify_loopback_headers(req.headers()) {
        return auth_err_resp(st, code, msg);
    }
    let (parts, body) = req.into_parts();
    let raw_bytes = match to_bytes(body, MAX_BODY_BYTES * 2).await {
        Ok(b) => b,
        Err(_) => {
            return auth_err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "Body exceeds 2 MiB",
            )
        }
    };
    let auth = state_auth(&state);
    let verified: Option<VerifiedLegacyIngress> = if parts.headers.contains_key("x-cowok-hmac-sig")
    {
        match verify_hmac_v2(&auth, &parts.method, &parts.uri, &parts.headers, &raw_bytes) {
            Ok(principal) => Some(VerifiedLegacyIngress {
                principal,
                kind: LegacyAuthKind::HmacV2,
                bearer_token: None,
            }),
            Err((st, code, msg)) => return auth_err_resp(st, code, msg),
        }
    } else if let Some(tok) = parts
        .headers
        .get("x-cowok-session-token")
        .and_then(|v| v.to_str().ok())
    {
        let found = auth
            .sessions
            .lock()
            .expect("sessions")
            .get(tok.trim())
            .cloned();
        match found {
            Some(entry) if entry.expires_at >= Utc::now().timestamp() => {
                Some(VerifiedLegacyIngress {
                    principal: entry.principal,
                    kind: LegacyAuthKind::SessionToken,
                    bearer_token: None,
                })
            }
            _ => {
                return auth_err_resp(
                    StatusCode::UNAUTHORIZED,
                    "UNAUTHENTICATED",
                    "Invalid session token",
                )
            }
        }
    } else if let Some(auth_hdr) = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(tok) = auth_hdr.strip_prefix("Bearer ").map(str::trim) {
            let is_superadmin = is_valid_superadmin_bearer(&parts.headers);
            Some(VerifiedLegacyIngress {
                principal: FlowPrincipal {
                    tenant_id: default_tenant_id(),
                    user_id: default_user_id(),
                    role: if is_superadmin { "admin" } else { "editor" }.to_string(),
                },
                kind: LegacyAuthKind::Bearer,
                bearer_token: Some(tok.to_string()),
            })
        } else {
            return auth_err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authorization header must use Bearer scheme",
            );
        }
    } else {
        None
    };

    if let Some(ref caller) = verified {
        if caller.principal.role == "viewer" && parts.method != Method::GET {
            return auth_err_resp(
                StatusCode::FORBIDDEN,
                "INSUFFICIENT_ROLE",
                "Viewer role cannot mutate project resources",
            );
        }
    } else if parts.uri.path() == "/api/v1/projects" {
        return auth_err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Missing authentication for projects endpoint",
        );
    }

    if parts.uri.path() == "/api/v1/projects" {
        if let Some(ref caller) = verified {
            if caller.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(&parts.headers)
            {
                return auth_err_resp(
                    StatusCode::FORBIDDEN,
                    "TENANT_ISOLATION_BREACH",
                    "Listing or creating root projects requires superadmin Bearer or verified loopback HMAC",
                );
            }
        }
    }

    let mut rebuilt = Request::from_parts(parts, Body::from(raw_bytes));
    if let Some(caller) = verified {
        rebuilt.extensions_mut().insert(caller);
    }
    next.run(rebuilt).await
}
