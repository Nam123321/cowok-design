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
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::SqlitePool;
use uuid::Uuid;

use crate::lockfile::{generate_csprng_hex_32, read_daemon_lock, DaemonLockInfo};
use crate::routes::{
    constant_time_eq, decode_hex, hmac_sha256, state_auth, DaemonAppState, HttpErrorBody,
};

const MAX_SKEW_SECS: i64 = 30;
const NONCE_LRU_CAPACITY: usize = 10_000;
const SESSION_TTL_SECS: i64 = 28_800;
const MAX_SESSIONS_CAPACITY: usize = 4_096;
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

pub type AuthPrincipal = FlowPrincipal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum HttpActorKind {
    HumanWorkbenchSession,
    AgentHmac,
    McpClient,
    SuperadminCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedActorContext {
    pub kind: HttpActorKind,
}

/// Enforces that `HUMAN_ONLY` routes are executed exclusively by an interactive human session
/// (never an Agent HMAC, MCP client, or `daemon.lock`-derived bearer/nonce session).
pub fn require_human_workbench_actor(actor: &VerifiedActorContext) -> Result<(), Response> {
    if actor.kind != HttpActorKind::HumanWorkbenchSession {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            "This operation requires an interactive HumanWorkbenchSession (HUMAN_ONLY); Agent/MCP/CLI tokens are denied",
        ));
    }
    Ok(())
}

/// Resolves the effective role for a principal given HTTP headers and an optional token string.
/// If `x-cowok-role` header is supplied: only `"editor"` is accepted; any other value (`"viewer"`,
/// `"admin"`, or unknown) fails closed to `"viewer"`.
/// If omitted: tokens ending in `:viewer` resolve to `"viewer"`, otherwise defaults to `"editor"`.
/// NOTE: Untrusted headers can NEVER grant `"admin"` or escalate beyond `"editor"`.
pub fn resolve_role(headers: &HeaderMap, token_opt: Option<&str>) -> String {
    if let Some(tok) = token_opt {
        if tok.ends_with(":viewer") {
            return "viewer".to_string();
        }
    }
    if let Some(role_hdr) = headers.get("x-cowok-role").and_then(|v| v.to_str().ok()) {
        let trimmed = role_hdr.trim();
        if trimmed.eq_ignore_ascii_case("editor") {
            return "editor".to_string();
        }
        // Fail closed on "viewer", "admin", or any unknown role header
        return "viewer".to_string();
    }
    "editor".to_string()
}

/// Enforces that mutating operations require at least editor (or admin) role.
/// Rejects `viewer` role with 403 FORBIDDEN.
pub fn require_editor_role(principal: &FlowPrincipal) -> Result<(), Response> {
    if !matches!(principal.role.as_str(), "editor" | "admin") {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_ROLE",
            "Viewer or unknown role cannot mutate resources; editor role required",
        ));
    }
    Ok(())
}

/// Authenticates an incoming request using either HMAC-SHA256 v2, session token, or verified superadmin Bearer.
/// Arbitrary non-superadmin Bearer tokens are strictly rejected for flow dual-ingress.
/// Supports explicit role scoping on local-default tokens: if request header `x-cowok-role` is `"viewer"`
/// (or token carries a `:viewer` suffix), sets `AuthPrincipal.role = "viewer"`.
pub fn authenticate_request(
    auth: &DaemonAuthState,
    method: &Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> Result<AuthPrincipal, Response> {
    authenticate_request_with_actor(auth, method, uri, headers, raw_body).map(|(p, _)| p)
}

/// Server-derived `(AuthPrincipal, VerifiedActorContext)` authenticator (EC-LEARN-001).
/// `HttpActorKind` is NEVER read from `principal_b64` or granted from `daemon.lock` (`daemon_nonce`).
pub fn authenticate_request_with_actor(
    auth: &DaemonAuthState,
    method: &Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> Result<(AuthPrincipal, VerifiedActorContext), Response> {
    let client_hint = headers
        .get("x-cowok-client-kind")
        .and_then(|v| v.to_str().ok())
        .map(str::trim);
    let mcp_or_agent = |principal: &FlowPrincipal| -> HttpActorKind {
        if client_hint == Some("mcp") || principal.user_id.to_ascii_lowercase().starts_with("mcp-")
        {
            HttpActorKind::McpClient
        } else {
            HttpActorKind::AgentHmac
        }
    };

    if headers.contains_key("x-cowok-hmac-sig") {
        let mut principal = verify_hmac_v2(auth, method, uri, headers, raw_body)
            .map_err(|(st, code, msg)| auth_err_resp(st, code, msg))?;
        let role = resolve_role(headers, None);
        if role == "viewer" {
            principal.role = "viewer".to_string();
        }
        if let Some(human_tok) = headers
            .get("x-cowok-human-session")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let Some(human_principal) = auth.get_valid_human_session(human_tok) else {
                return Err(auth_err_resp(
                    StatusCode::UNAUTHORIZED,
                    "UNAUTHENTICATED",
                    "Invalid human workbench session token",
                ));
            };
            if human_principal.tenant_id != principal.tenant_id
                || human_principal.user_id != principal.user_id
            {
                return Err(auth_err_resp(
                    StatusCode::FORBIDDEN,
                    "HUMAN_APPROVAL_REQUIRED",
                    "Human session principal mismatch",
                ));
            }
            return Ok((
                principal,
                VerifiedActorContext {
                    kind: HttpActorKind::HumanWorkbenchSession,
                },
            ));
        }
        let kind = mcp_or_agent(&principal);
        Ok((principal, VerifiedActorContext { kind }))
    } else if let Some(human_tok) = headers
        .get("x-cowok-human-session")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        match auth.get_valid_human_session(human_tok) {
            Some(mut principal) => {
                let role = resolve_role(headers, Some(human_tok));
                if role == "viewer" {
                    principal.role = "viewer".to_string();
                }
                Ok((
                    principal,
                    VerifiedActorContext {
                        kind: HttpActorKind::HumanWorkbenchSession,
                    },
                ))
            }
            None => Err(auth_err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Invalid human workbench session token",
            )),
        }
    } else if let Some(tok) = headers
        .get("x-cowok-session-token")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(mut human_principal) = auth.get_valid_human_session(tok) {
            let role = resolve_role(headers, Some(tok));
            if role == "viewer" {
                human_principal.role = "viewer".to_string();
            }
            return Ok((
                human_principal,
                VerifiedActorContext {
                    kind: HttpActorKind::HumanWorkbenchSession,
                },
            ));
        }
        match auth.get_valid_session(tok) {
            Some(mut principal) => {
                let role = resolve_role(headers, Some(tok));
                if role == "viewer" {
                    principal.role = "viewer".to_string();
                }
                let kind = mcp_or_agent(&principal);
                Ok((principal, VerifiedActorContext { kind }))
            }
            None => Err(auth_err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Invalid session token",
            )),
        }
    } else if let Some(auth_hdr) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(_tok) = auth_hdr
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            if is_valid_superadmin_bearer(headers) {
                Ok((
                    FlowPrincipal {
                        tenant_id: default_tenant_id(),
                        user_id: default_user_id(),
                        role: "admin".to_string(),
                    },
                    VerifiedActorContext {
                        kind: HttpActorKind::SuperadminCli,
                    },
                ))
            } else {
                Err(auth_err_resp(
                    StatusCode::UNAUTHORIZED,
                    "UNAUTHENTICATED",
                    "Invalid bearer token for flow ingress",
                ))
            }
        } else {
            Err(auth_err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authorization header must provide a non-empty Bearer token",
            ))
        }
    } else {
        Err(auth_err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Missing authentication",
        ))
    }
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
    human_sessions: Mutex<HashMap<String, SessionEntry>>,
    pool: Option<SqlitePool>,
}

fn sha256_hex_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

impl DaemonAuthState {
    pub fn from_lock_info(lock: &DaemonLockInfo) -> Result<Arc<Self>, HarnessError> {
        Self::from_lock_info_with_pool(lock, None)
    }

    pub fn from_lock_info_with_pool(
        lock: &DaemonLockInfo,
        pool: Option<SqlitePool>,
    ) -> Result<Arc<Self>, HarnessError> {
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
        Ok(Self::with_secrets_and_pool(
            lock.daemon_nonce.clone(),
            key_bytes,
            pool,
        ))
    }

    pub fn from_registry_root(workspace_root: &std::path::Path) -> Arc<Self> {
        Self::from_registry_root_with_pool(workspace_root, None)
    }

    pub fn from_registry_root_with_pool(
        workspace_root: &std::path::Path,
        pool: Option<SqlitePool>,
    ) -> Arc<Self> {
        if let Ok(lock) = read_daemon_lock(workspace_root) {
            if let Ok(state) = Self::from_lock_info_with_pool(&lock, pool.clone()) {
                return state;
            }
        }
        let mut key_bytes = vec![0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        Self::with_secrets_and_pool(generate_csprng_hex_32(), key_bytes, pool)
    }

    pub fn with_secrets(daemon_nonce: String, hmac_key: Vec<u8>) -> Arc<Self> {
        Self::with_secrets_and_pool(daemon_nonce, hmac_key, None)
    }

    pub fn with_secrets_and_pool(
        daemon_nonce: String,
        hmac_key: Vec<u8>,
        pool: Option<SqlitePool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            daemon_nonce,
            hmac_key,
            nonce_order: Mutex::new(VecDeque::new()),
            nonce_set: Mutex::new(HashSet::new()),
            sessions: Mutex::new(HashMap::new()),
            human_sessions: Mutex::new(HashMap::new()),
            pool,
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
        {
            let mut set = self.nonce_set.lock().unwrap_or_else(|e| e.into_inner());
            if set.contains(nonce) {
                return false;
            }
            if let Some(ref pool) = self.pool {
                let Ok(conn) = pool.connection() else {
                    return false;
                };
                let now = Utc::now().timestamp();
                let expires_at = now.saturating_add(MAX_SKEW_SECS * 2);
                let _ = conn.execute(
                    "DELETE FROM daemon_replay_nonces WHERE expires_at < ?1",
                    params![now],
                );
                let Ok(inserted) = conn.execute(
                    "INSERT OR IGNORE INTO daemon_replay_nonces (nonce, daemon_nonce, expires_at, seen_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![nonce, self.daemon_nonce, expires_at, now],
                ) else {
                    return false;
                };
                if inserted == 0 {
                    return false;
                }
            }
            let mut order = self.nonce_order.lock().unwrap_or_else(|e| e.into_inner());
            if order.len() >= NONCE_LRU_CAPACITY {
                if let Some(evicted) = order.pop_front() {
                    set.remove(&evicted);
                }
            }
            set.insert(nonce.to_string());
            order.push_back(nonce.to_string());
        }
        true
    }

    pub fn insert_session(&self, token: String, principal: FlowPrincipal, expires_at: i64) {
        self.insert_session_with_capacity(token, principal, expires_at, MAX_SESSIONS_CAPACITY);
    }

    pub fn insert_session_with_capacity(
        &self,
        token: String,
        principal: FlowPrincipal,
        expires_at: i64,
        max_capacity: usize,
    ) {
        let now = Utc::now().timestamp();
        if let Some(ref pool) = self.pool {
            if let Ok(conn) = pool.connection() {
                let _ = conn.execute(
                    "DELETE FROM daemon_sessions WHERE expires_at < ?1",
                    params![now],
                );
                let token_hash = sha256_hex_token(token.trim());
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO daemon_sessions (token_sha256, tenant_id, user_id, role, expires_at, created_at, revoked_at, actor_kind)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 'AGENT_HMAC')",
                    params![
                        token_hash,
                        principal.tenant_id,
                        principal.user_id,
                        principal.role,
                        expires_at,
                        now
                    ],
                );
            }
        }
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.retain(|_, entry| entry.expires_at >= now);
        while sessions.len() >= max_capacity.max(1) {
            let Some(oldest_key) = sessions
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            sessions.remove(&oldest_key);
        }
        sessions.insert(
            token,
            SessionEntry {
                principal,
                expires_at,
            },
        );
    }

    pub fn get_valid_session(&self, token: &str) -> Option<FlowPrincipal> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return None;
        }
        let now = Utc::now().timestamp();
        let cached = {
            let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            match sessions.get(trimmed).cloned() {
                Some(entry) if entry.expires_at >= now => Some(entry.principal),
                Some(_) => {
                    sessions.remove(trimmed);
                    None
                }
                None => None,
            }
        };
        if let Some(ref pool) = self.pool {
            if let Ok(conn) = pool.connection() {
                let token_hash = sha256_hex_token(trimmed);
                let db_hit: Option<(String, String, String, i64)> = conn
                    .query_row(
                        "SELECT tenant_id, user_id, role, expires_at
                         FROM daemon_sessions
                         WHERE token_sha256 = ?1 AND revoked_at IS NULL AND expires_at >= ?2",
                        params![token_hash, now],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()
                    .ok()
                    .flatten();
                if let Some((tenant_id, user_id, role, exp)) = db_hit {
                    let principal = FlowPrincipal {
                        tenant_id,
                        user_id,
                        role,
                    };
                    let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
                    sessions.insert(
                        trimmed.to_string(),
                        SessionEntry {
                            principal: principal.clone(),
                            expires_at: exp,
                        },
                    );
                    return Some(principal);
                }
                if cached.is_some() {
                    let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
                    sessions.remove(trimmed);
                }
                return None;
            }
            // Fail closed if database revocation check cannot run while DB pool is configured
            return None;
        }
        cached
    }

    pub fn issue_human_workbench_session(
        &self,
        principal: FlowPrincipal,
        expires_at: i64,
    ) -> String {
        let token = format!("cwk_human_{}", generate_csprng_hex_32());
        let now = Utc::now().timestamp();
        if let Some(ref pool) = self.pool {
            if let Ok(conn) = pool.connection() {
                let _ = conn.execute(
                    "DELETE FROM daemon_sessions WHERE expires_at < ?1",
                    params![now],
                );
                let token_hash = sha256_hex_token(token.trim());
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO daemon_sessions (token_sha256, tenant_id, user_id, role, expires_at, created_at, revoked_at, actor_kind)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 'HUMAN_WORKBENCH_SESSION')",
                    params![
                        token_hash,
                        principal.tenant_id,
                        principal.user_id,
                        principal.role,
                        expires_at,
                        now
                    ],
                );
            }
        }
        let mut sessions = self
            .human_sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        sessions.retain(|_, entry| entry.expires_at >= now);
        while sessions.len() >= MAX_SESSIONS_CAPACITY {
            let Some(oldest) = sessions
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            sessions.remove(&oldest);
        }
        sessions.insert(
            token.clone(),
            SessionEntry {
                principal,
                expires_at,
            },
        );
        token
    }

    pub fn get_valid_human_session(&self, token: &str) -> Option<FlowPrincipal> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return None;
        }
        let now = Utc::now().timestamp();
        let cached = {
            let mut sessions = self
                .human_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match sessions.get(trimmed).cloned() {
                Some(entry) if entry.expires_at >= now => Some(entry.principal),
                Some(_) => {
                    sessions.remove(trimmed);
                    None
                }
                None => None,
            }
        };
        if let Some(ref pool) = self.pool {
            if let Ok(conn) = pool.connection() {
                let token_hash = sha256_hex_token(trimmed);
                let db_hit: Option<(String, String, String, i64)> = conn
                    .query_row(
                        "SELECT tenant_id, user_id, role, expires_at
                         FROM daemon_sessions
                         WHERE token_sha256 = ?1 AND actor_kind = 'HUMAN_WORKBENCH_SESSION'
                           AND revoked_at IS NULL AND expires_at >= ?2",
                        params![token_hash, now],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()
                    .ok()
                    .flatten();
                if let Some((tenant_id, user_id, role, exp)) = db_hit {
                    let principal = FlowPrincipal {
                        tenant_id,
                        user_id,
                        role,
                    };
                    let mut sessions = self
                        .human_sessions
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    sessions.insert(
                        trimmed.to_string(),
                        SessionEntry {
                            principal: principal.clone(),
                            expires_at: exp,
                        },
                    );
                    return Some(principal);
                }
                if cached.is_some() {
                    let mut sessions = self
                        .human_sessions
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    sessions.remove(trimmed);
                }
                return None;
            }
            // Fail closed if DB revocation check cannot run while DB pool is configured.
            return None;
        }
        cached
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }
}

pub fn is_allowed_loopback_host_with_port(host: &str, expected_port: Option<u16>) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let (hostname, port_opt) = match h.split_once(':') {
        Some((name, port_str)) => {
            if port_str.is_empty() || !port_str.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            let Ok(port) = port_str.parse::<u16>() else {
                return false;
            };
            if port == 0 {
                return false;
            }
            (name, Some(port))
        }
        None => (h.as_str(), None),
    };
    if hostname != "127.0.0.1" && hostname != "localhost" {
        return false;
    }
    match expected_port {
        Some(pinned) if pinned > 0 => port_opt == Some(pinned),
        _ => true,
    }
}

pub fn is_allowed_loopback_host(host: &str) -> bool {
    is_allowed_loopback_host_with_port(host, None)
}

fn extract_loopback_origin_host(origin: &str) -> Option<&str> {
    let trimmed = origin.trim();
    let after_scheme = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))?;
    let host_part = after_scheme.strip_suffix('/').unwrap_or(after_scheme);
    if is_allowed_loopback_host(host_part) {
        Some(host_part)
    } else {
        None
    }
}

fn verify_loopback_headers(
    headers: &HeaderMap,
    expected_port: Option<u16>,
) -> Result<(), (StatusCode, &'static str, &'static str)> {
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or((
            StatusCode::FORBIDDEN,
            "ORIGIN_REJECTED",
            "Missing Host header",
        ))?;
    if !is_allowed_loopback_host_with_port(host, expected_port) {
        return Err((
            StatusCode::FORBIDDEN,
            "ORIGIN_REJECTED",
            "Non-loopback or cross-port Host rejected",
        ));
    }
    let mut _sec_fetch_site_val: Option<&str> = None;
    if let Some(sfs) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        let sfs_trimmed = sfs.trim();
        if sfs_trimmed != "same-origin" && sfs_trimmed != "none" {
            return Err((
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Cross-site Sec-Fetch-Site rejected",
            ));
        }
        _sec_fetch_site_val = Some(sfs_trimmed);
    }
    if let Some(orig) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let Some(orig_host) = extract_loopback_origin_host(orig) else {
            return Err((
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Non-loopback Origin rejected",
            ));
        };
        if !orig_host.eq_ignore_ascii_case(host) {
            return Err((
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Cross-port or mismatched Origin rejected",
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
    let expected_port = (state.port > 0).then_some(state.port);
    if let Err((st, code, msg)) = verify_loopback_headers(&headers, expected_port) {
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

    let is_same_origin = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .is_some_and(|s| s == "same-origin");
    if !is_superadmin && !has_daemon_nonce && !is_same_origin {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_ROLE",
            "Local session issuance requires daemon nonce proof, superadmin authorization, or same-origin browser context",
        ));
    }
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
    auth.insert_session(token.clone(), principal.clone(), expires_at);
    Ok(Json(LocalSessionResponse {
        session_token: token,
        tenant_id: principal.tenant_id,
        user_id: principal.user_id,
        role,
        expires_at,
    }))
}

/// Interactive `HUMAN_ONLY` workbench session issuer (`POST /api/v1/auth/workbench-session`).
/// Strictly refuses `daemon.lock` (`daemon_nonce`) alone; requires `COWOK_SUPERADMIN_SECRET` or
/// interactive `COWOK_WORKBENCH_HUMAN_SECRET` held outside `daemon.lock`.
pub async fn handle_issue_human_workbench_session(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    body_opt: Option<Json<LocalSessionRequest>>,
) -> Result<Json<LocalSessionResponse>, Response> {
    let expected_port = (state.port > 0).then_some(state.port);
    if let Err((st, code, msg)) = verify_loopback_headers(&headers, expected_port) {
        return Err(auth_err_resp(st, code, msg));
    }
    let has_superadmin = is_valid_superadmin_bearer(&headers);
    let supplied_secret = headers
        .get("x-cowok-workbench-secret")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let env_secret = std::env::var("COWOK_WORKBENCH_HUMAN_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let file_secret = if env_secret.is_none() {
        crate::vault::VaultStore::from_workspace_root(state.registry.pool().workspace_root())
            .read_existing_workbench_human_secret()
            .ok()
            .flatten()
            .map(|z| z.as_str().to_string())
    } else {
        None
    };
    let has_human_secret = supplied_secret.is_some_and(|actual| {
        if let Some(exp) = env_secret.as_deref() {
            constant_time_eq(exp.as_bytes(), actual.as_bytes())
        } else {
            file_secret
                .as_deref()
                .is_some_and(|exp| constant_time_eq(exp.as_bytes(), actual.as_bytes()))
        }
    });
    if !has_superadmin && !has_human_secret {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            "HumanWorkbenchSession requires interactive workbench secret or superadmin bearer; daemon.lock credentials cannot mint HUMAN_ONLY sessions",
        ));
    }
    let req = body_opt.map(|Json(v)| v).unwrap_or_default();
    let role = req.role.unwrap_or_else(default_role);
    if !matches!(role.as_str(), "editor" | "admin") {
        return Err(auth_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Human workbench session role must be editor or admin",
        ));
    }
    if role == "admin" && !has_superadmin {
        return Err(auth_err_resp(
            StatusCode::FORBIDDEN,
            "INSUFFICIENT_ROLE",
            "Admin human workbench session requires COWOK_SUPERADMIN_SECRET Bearer",
        ));
    }
    let principal = FlowPrincipal {
        tenant_id: req
            .tenant_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_tenant_id),
        user_id: req
            .user_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_user_id),
        role: role.clone(),
    };
    let auth = state_auth(&state);
    let expires_at = Utc::now().timestamp() + SESSION_TTL_SECS;
    let token = auth.issue_human_workbench_session(principal.clone(), expires_at);
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
    let expected_port = (state.port > 0).then_some(state.port);
    if let Err((st, code, msg)) = verify_loopback_headers(req.headers(), expected_port) {
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
    let (principal, actor_ctx) = match authenticate_request_with_actor(
        &auth,
        &parts.method,
        &parts.uri,
        &parts.headers,
        &raw_bytes,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };
    if parts.method != Method::GET && parts.method != Method::HEAD {
        if let Err(resp) = require_editor_role(&principal) {
            return resp;
        }
    }
    let mut rebuilt = Request::from_parts(parts, Body::from(raw_bytes));
    rebuilt.extensions_mut().insert(principal);
    rebuilt.extensions_mut().insert(actor_ctx);
    next.run(rebuilt).await
}

fn verify_hmac_v2(
    auth: &DaemonAuthState,
    method: &Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    raw_body: &[u8],
) -> Result<FlowPrincipal, (StatusCode, &'static str, &'static str)> {
    let body_sha_hex = format!("{:x}", Sha256::digest(raw_body));
    verify_hmac_v2_with_sha(auth, method, uri, headers, &body_sha_hex)
}

fn verify_hmac_v2_with_sha(
    auth: &DaemonAuthState,
    method: &Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body_sha_hex: &str,
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
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        auth.daemon_nonce,
        ts_str,
        req_nonce,
        method.as_str().to_ascii_uppercase(),
        uri.path(),
        uri.query().unwrap_or(""),
        principal_b64,
        body_sha_hex
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
    let mut principal: FlowPrincipal = serde_json::from_slice(&decoded).map_err(|_| {
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
    if principal.role == "admin" && !is_valid_superadmin_bearer(headers) {
        principal.role = "editor".to_string();
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

pub async fn cors_and_loopback_middleware(
    State(state): State<DaemonAppState>,
    req: Request,
    next: Next,
) -> Response {
    let expected_port = (state.port > 0).then_some(state.port);
    if let Err((st, code, msg)) = verify_loopback_headers(req.headers(), expected_port) {
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
        if extract_loopback_origin_host(orig).is_some() {
            if let Ok(val) = HeaderValue::from_str(orig) {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, val);
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
                );
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_static(
                        "Authorization, Content-Type, X-Cowok-Session-Token, X-Cowok-Human-Session, X-Cowok-Client-Kind, X-Cowok-Workspace-Root, X-Cowok-Actor-Id, X-Cowok-Portal, X-Correlation-Id",
                    ),
                );
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CasIngressTempGuard {
    pub path: std::path::PathBuf,
}

impl Drop for CasIngressTempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn legacy_project_auth_middleware(
    State(state): State<DaemonAppState>,
    req: Request,
    next: Next,
) -> Response {
    let expected_port = (state.port > 0).then_some(state.port);
    if let Err((st, code, msg)) = verify_loopback_headers(req.headers(), expected_port) {
        return auth_err_resp(st, code, msg);
    }
    let (parts, body) = req.into_parts();
    let is_cas_upload = parts.method == Method::POST && parts.uri.path() == "/api/v1/cas/upload";
    if is_cas_upload {
        const MAX_CAS_CONTENT_LEN: usize = 26_214_400 + 262_144;
        let has_auth_hdr = parts.headers.contains_key("x-cowok-hmac-sig")
            || parts.headers.contains_key("x-cowok-session-token")
            || parts.headers.contains_key("authorization");
        if !has_auth_hdr {
            return auth_err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Missing authentication for legacy project endpoint",
            );
        }
        if parts
            .headers
            .get("x-cowok-role")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|r| r.trim().eq_ignore_ascii_case("viewer"))
        {
            return auth_err_resp(
                StatusCode::FORBIDDEN,
                "AUTH_FORBIDDEN",
                "Viewer role cannot upload attachments",
            );
        }
        if let Some(clen) = parts
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<usize>().ok())
        {
            if clen > MAX_CAS_CONTENT_LEN {
                return crate::routes::chat_cas_storage::safe_api_error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "CAS_QUOTA_EXCEEDED",
                    "Request upload size limit exceeded",
                    false,
                    Some("REQUEST_BYTES"),
                );
            }
        }
    }
    let (raw_bytes, body_sha_hex, staged_cas) = if is_cas_upload {
        let cas_dir = state
            .registry
            .pool()
            .workspace_root()
            .join(".cowok-design/cas");
        match stream_cas_body_to_temp(&cas_dir, body).await {
            Ok((sha, guard)) => (axum::body::Bytes::new(), sha, Some(Arc::new(guard))),
            Err(resp) => return resp,
        }
    } else {
        let b = match to_bytes(body, MAX_BODY_BYTES * 2).await {
            Ok(b) => b,
            Err(_) => {
                return auth_err_resp(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "PAYLOAD_TOO_LARGE",
                    "Body exceeds 2 MiB",
                )
            }
        };
        let sha = format!("{:x}", Sha256::digest(&b));
        (b, sha, None)
    };
    let auth = state_auth(&state);
    let verified: Option<VerifiedLegacyIngress> = if parts.headers.contains_key("x-cowok-hmac-sig")
    {
        match verify_hmac_v2_with_sha(
            &auth,
            &parts.method,
            &parts.uri,
            &parts.headers,
            &body_sha_hex,
        ) {
            Ok(mut principal) => {
                // Honor explicit header downgrade to viewer/editor while preserving HMAC-verified admin
                if let Some(hdr_role) = parts
                    .headers
                    .get("x-cowok-role")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.trim().to_ascii_lowercase())
                {
                    if hdr_role == "viewer" {
                        principal.role = "viewer".to_string();
                    } else if hdr_role == "editor" && principal.role == "admin" {
                        principal.role = "editor".to_string();
                    } else if hdr_role != "editor" && hdr_role != "admin" {
                        // Fail closed on unknown x-cowok-role header values.
                        principal.role = "viewer".to_string();
                    }
                }
                Some(VerifiedLegacyIngress {
                    principal,
                    kind: LegacyAuthKind::HmacV2,
                    bearer_token: None,
                })
            }
            Err((st, code, msg)) => return auth_err_resp(st, code, msg),
        }
    } else if let Some(tok) = parts
        .headers
        .get("x-cowok-session-token")
        .and_then(|v| v.to_str().ok())
    {
        match auth
            .get_valid_human_session(tok)
            .or_else(|| auth.get_valid_session(tok))
        {
            Some(mut principal) => {
                let role = resolve_role(&parts.headers, Some(tok));
                if role == "viewer" {
                    principal.role = "viewer".to_string();
                }
                Some(VerifiedLegacyIngress {
                    principal,
                    kind: LegacyAuthKind::SessionToken,
                    bearer_token: None,
                })
            }
            None => {
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
        if let Some(tok) = auth_hdr
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let is_superadmin = is_valid_superadmin_bearer(&parts.headers);
            let role = if is_superadmin {
                "admin".to_string()
            } else {
                resolve_role(&parts.headers, Some(tok))
            };
            Some(VerifiedLegacyIngress {
                principal: FlowPrincipal {
                    tenant_id: default_tenant_id(),
                    user_id: default_user_id(),
                    role,
                },
                kind: LegacyAuthKind::Bearer,
                bearer_token: Some(tok.to_string()),
            })
        } else {
            return auth_err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authorization header must provide a non-empty Bearer token",
            );
        }
    } else {
        None
    };

    if let Some(ref caller) = verified {
        let is_readonly_post =
            parts.method == Method::POST && is_anchored_readonly_post_path(parts.uri.path());
        if parts.method != Method::GET && parts.method != Method::HEAD && !is_readonly_post {
            if let Err(resp) = require_editor_role(&caller.principal) {
                return resp;
            }
        }
    } else {
        return auth_err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHENTICATED",
            "Missing authentication for legacy project endpoint",
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

    if parts.uri.path() == "/api/v1/fs/browse" {
        if let Some(ref caller) = verified {
            if caller.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(&parts.headers)
            {
                return auth_err_resp(
                    StatusCode::FORBIDDEN,
                    "AUTH_SCOPE_MISMATCH",
                    "Browsing host filesystem requires superadmin Bearer, session token, or verified loopback HMAC",
                );
            }
        }
    }

    let mut rebuilt = Request::from_parts(parts, Body::from(raw_bytes));
    if let Some(staged) = staged_cas {
        rebuilt.extensions_mut().insert(staged);
    }
    if let Some(caller) = verified {
        rebuilt.extensions_mut().insert(caller);
    }
    next.run(rebuilt).await
}

async fn stream_cas_body_to_temp(
    cas_dir: &std::path::Path,
    body: Body,
) -> Result<(String, CasIngressTempGuard), Response> {
    use futures_util::StreamExt;
    use std::io::Write;
    const MAX_INGRESS_BYTES: usize = 26_214_400 + 262_144;
    let (tmp_path, mut tmp_file) = crate::routes::chat_cas_archive::open_secure_temp_file(
        cas_dir,
        &format!(".ingress-{}", Uuid::new_v4()),
    )?;
    let guard = CasIngressTempGuard { path: tmp_path };
    let mut hasher = Sha256::new();
    let mut total: usize = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk_res) = stream.next().await {
        let Ok(chunk) = chunk_res else {
            return Err(crate::routes::chat_cas_storage::safe_api_error_response(
                StatusCode::BAD_REQUEST,
                "CAS_INVALID_STREAM",
                "Failed to read upload stream",
                false,
                None,
            ));
        };
        total = total.saturating_add(chunk.len());
        if total > MAX_INGRESS_BYTES {
            return Err(crate::routes::chat_cas_storage::safe_api_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "CAS_QUOTA_EXCEEDED",
                "Request upload size limit exceeded",
                false,
                Some("REQUEST_BYTES"),
            ));
        }
        hasher.update(&chunk);
        let write_res = tokio::task::spawn_blocking(move || {
            let res = tmp_file.write_all(&chunk);
            (tmp_file, res)
        })
        .await;
        match write_res {
            Ok((returned_file, Ok(()))) => {
                tmp_file = returned_file;
            }
            _ => {
                return Err(crate::routes::chat_cas_storage::safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CAS_STORAGE_ERROR",
                    "Failed to stage upload stream",
                    false,
                    None,
                ));
            }
        }
    }
    let _ = tokio::task::spawn_blocking(move || {
        let _ = tmp_file.flush();
    })
    .await;
    Ok((format!("{:x}", hasher.finalize()), guard))
}

fn is_anchored_readonly_post_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix('/') else {
        return false;
    };
    let segs: Vec<&str> = rest.split('/').collect();
    if segs.iter().any(|s| s.is_empty() || *s == "." || *s == "..") {
        return false;
    }
    matches!(
        segs.as_slice(),
        ["api", "v1", "decks", _, "slides", _, "charts", "validate"]
            | [
                "api",
                "v1",
                "projects",
                _,
                "decks",
                _,
                "slides",
                _,
                "charts",
                "validate"
            ]
            | ["api", "v1", "decks", _, "presenter", "handshake"]
            | ["api", "v1", "skills", "route"]
            | [
                "api",
                "v1",
                "projects",
                _,
                "decks",
                _,
                "presenter",
                "handshake"
            ]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_host_pins_port_and_rejects_malformed_ports() {
        assert!(is_allowed_loopback_host_with_port(
            "127.0.0.1:4170",
            Some(4170)
        ));
        assert!(is_allowed_loopback_host_with_port(
            "localhost:4170",
            Some(4170)
        ));
        assert!(!is_allowed_loopback_host_with_port("127.0.0.1", Some(4170)));
        assert!(!is_allowed_loopback_host_with_port(
            "127.0.0.1:9999",
            Some(4170)
        ));
        assert!(!is_allowed_loopback_host_with_port("127.0.0.1:0", None));
        assert!(!is_allowed_loopback_host_with_port("127.0.0.1:+80", None));
        assert!(!is_allowed_loopback_host_with_port(
            "evil.example:4170",
            Some(4170)
        ));
    }

    #[test]
    fn session_store_evicts_expired_and_caps_capacity() {
        let auth = DaemonAuthState::with_secrets("nonce".to_string(), vec![7u8; 32]);
        let now = Utc::now().timestamp();
        let principal = FlowPrincipal {
            tenant_id: "local-default".into(),
            user_id: "u1".into(),
            role: "editor".into(),
        };

        auth.insert_session_with_capacity("expired".into(), principal.clone(), now - 10, 2);
        assert!(auth.get_valid_session("expired").is_none());
        assert_eq!(auth.active_session_count(), 0);

        auth.insert_session_with_capacity("s1".into(), principal.clone(), now + 100, 2);
        auth.insert_session_with_capacity("s2".into(), principal.clone(), now + 200, 2);
        auth.insert_session_with_capacity("s3".into(), principal, now + 300, 2);
        assert_eq!(auth.active_session_count(), 2);
        assert!(auth.get_valid_session("s1").is_none());
        assert!(auth.get_valid_session("s2").is_some());
        assert!(auth.get_valid_session("s3").is_some());
    }

    #[test]
    fn test_viewer_role_rejected_by_require_editor_role() {
        let viewer = AuthPrincipal {
            tenant_id: "local-default".to_string(),
            user_id: "u-viewer".to_string(),
            role: "viewer".to_string(),
        };
        let res = require_editor_role(&viewer);
        assert!(res.is_err());
        let err_response = res.unwrap_err();
        assert_eq!(err_response.status(), StatusCode::FORBIDDEN);

        let editor = AuthPrincipal {
            tenant_id: "local-default".to_string(),
            user_id: "u-editor".to_string(),
            role: "editor".to_string(),
        };
        assert!(require_editor_role(&editor).is_ok());

        let admin = AuthPrincipal {
            tenant_id: "local-default".to_string(),
            user_id: "u-admin".to_string(),
            role: "admin".to_string(),
        };
        assert!(require_editor_role(&admin).is_ok());
    }

    #[test]
    fn test_resolve_role_scoping() {
        let mut headers = HeaderMap::new();
        assert_eq!(resolve_role(&headers, None), "editor");
        assert_eq!(resolve_role(&headers, Some("token:viewer")), "viewer");

        headers.insert("x-cowok-role", HeaderValue::from_static("viewer"));
        assert_eq!(resolve_role(&headers, None), "viewer");

        headers.insert("x-cowok-role", HeaderValue::from_static("editor"));
        assert_eq!(resolve_role(&headers, None), "editor");
    }

    #[test]
    fn test_admin_role_cannot_be_escalated_via_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-cowok-role", HeaderValue::from_static("admin"));
        // resolve_role must fail closed on unknown/admin header -> "viewer"
        assert_ne!(resolve_role(&headers, None), "admin");
        assert_eq!(resolve_role(&headers, None), "viewer");

        // Even with a token, header "admin" does not escalate
        assert_ne!(resolve_role(&headers, Some("token123")), "admin");
        assert_eq!(resolve_role(&headers, Some("token123")), "viewer");

        // Arbitrary unknown role header also fails closed to "viewer"
        headers.insert("x-cowok-role", HeaderValue::from_static("superuser"));
        assert_eq!(resolve_role(&headers, Some("token123")), "viewer");
    }

    #[test]
    fn test_arbitrary_bearer_rejected_by_authenticate_request() {
        let auth = DaemonAuthState::with_secrets("nonce".to_string(), vec![7u8; 32]);
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer untrusted_token"),
        );
        let uri = "/api/v1/projects/p1/flow/stream".parse().unwrap();
        let res = authenticate_request(&auth, &Method::POST, &uri, &headers, b"{}");
        assert!(res.is_err());
        let err_resp = res.unwrap_err();
        assert_eq!(err_resp.status(), StatusCode::UNAUTHORIZED);
    }
}
