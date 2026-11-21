// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05 (Safe Errors, SQLite Retry Executor, Secret Redaction & TTL Allowlist)
use std::time::{Duration, Instant};

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rand::Rng;
use rusqlite::{
    params, Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CAS_MAX_REQUEST_BYTES: usize = 26_214_400;
pub const CAS_MAX_TENANT_PHYSICAL_BYTES: i64 = 536_870_912;
pub const CAS_MAX_TENANT_INGRESS_24H_BYTES: i64 = 1_073_741_824;
pub const CAS_MAX_SESSION_ATTACHMENTS: i64 = 500;
pub const IDEMPOTENCY_TTL_MS: i64 = 86_400_000;
pub const CAS_GC_LEASE_MS: i64 = 60_000;

const RETRY_MAX_ATTEMPTS: usize = 3;
const RETRY_INITIAL_BACKOFF_MS: u64 = 25;
const RETRY_MAX_BACKOFF_MS: u64 = 200;
const RETRY_JITTER_MAX_MS: u64 = 25;
const RETRY_DEADLINE_MS: u64 = 3_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorDto {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
}

pub fn safe_api_error_response(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    retryable: bool,
    limit: Option<&'static str>,
) -> Response {
    (
        status,
        Json(ApiErrorDto {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            limit: limit.map(str::to_string),
        }),
    )
        .into_response()
}

pub enum ChatCasTxError {
    Sqlite(rusqlite::Error),
    Http(Response),
}

impl From<rusqlite::Error> for ChatCasTxError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

fn is_sqlite_lock_contention(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(ffi_err, _)
            if ffi_err.code == ErrorCode::DatabaseBusy || ffi_err.code == ErrorCode::DatabaseLocked
    )
}

pub async fn execute_immediate_with_retry<T, F>(
    conn_factory: impl Fn() -> Result<Connection, Response>,
    mut op: F,
) -> Result<T, Response>
where
    F: FnMut(&Transaction<'_>) -> Result<T, ChatCasTxError>,
{
    let deadline = Instant::now() + Duration::from_millis(RETRY_DEADLINE_MS);
    let mut backoff_ms = RETRY_INITIAL_BACKOFF_MS;

    for attempt in 1..=RETRY_MAX_ATTEMPTS {
        let mut conn = conn_factory()?;
        let _ = conn.busy_timeout(Duration::from_millis(250));
        match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
            Ok(tx) => match op(&tx) {
                Ok(val) => match tx.commit() {
                    Ok(()) => return Ok(val),
                    Err(e) if is_sqlite_lock_contention(&e) => {
                        if attempt >= RETRY_MAX_ATTEMPTS || Instant::now() >= deadline {
                            return Err(safe_api_error_response(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "CHAT_WRITE_CONTENTION",
                                "Database is temporarily busy; please retry",
                                true,
                                None,
                            ));
                        }
                    }
                    Err(_) => {
                        return Err(safe_api_error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "INTERNAL_ERROR",
                            "Persistence commit failed",
                            false,
                            None,
                        ));
                    }
                },
                Err(ChatCasTxError::Http(resp)) => return Err(resp),
                Err(ChatCasTxError::Sqlite(e)) if is_sqlite_lock_contention(&e) => {
                    drop(tx);
                    if attempt >= RETRY_MAX_ATTEMPTS || Instant::now() >= deadline {
                        return Err(safe_api_error_response(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "CHAT_WRITE_CONTENTION",
                            "Database is temporarily busy; please retry",
                            true,
                            None,
                        ));
                    }
                }
                Err(ChatCasTxError::Sqlite(_)) => {
                    return Err(safe_api_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "INTERNAL_ERROR",
                        "Persistence transaction failed",
                        false,
                        None,
                    ));
                }
            },
            Err(e) if is_sqlite_lock_contention(&e) => {
                if attempt >= RETRY_MAX_ATTEMPTS || Instant::now() >= deadline {
                    return Err(safe_api_error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "CHAT_WRITE_CONTENTION",
                        "Database is temporarily busy; please retry",
                        true,
                        None,
                    ));
                }
            }
            Err(_) => {
                return Err(safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Failed to begin transaction",
                    false,
                    None,
                ));
            }
        }

        let jitter: u64 = rand::thread_rng().gen_range(0..=RETRY_JITTER_MAX_MS);
        let sleep_dur = Duration::from_millis(backoff_ms.saturating_add(jitter));
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(safe_api_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "CHAT_WRITE_CONTENTION",
                "Database is temporarily busy; please retry",
                true,
                None,
            ));
        }
        tokio::time::sleep(sleep_dur.min(remaining)).await;
        backoff_ms = (backoff_ms.saturating_mul(2)).min(RETRY_MAX_BACKOFF_MS);
    }

    Err(safe_api_error_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "CHAT_WRITE_CONTENTION",
        "Database is temporarily busy; please retry",
        true,
        None,
    ))
}

/// Strictly allowlisted tenant-scoped TTL cleanup for decoupled receipt tables ONLY.
/// Permanent tables (`cas_attachments`, `cas_blobs`, `cas_upload_ingress`) MUST NEVER be deleted here.
pub fn purge_expired_idempotency_receipts(
    tx: &Transaction<'_>,
    tenant_id: &str,
    now_ms: i64,
) -> Result<(usize, usize), rusqlite::Error> {
    let cas_purged = tx.execute(
        "DELETE FROM cas_command_idempotency WHERE tenant_id = ?1 AND expires_at < ?2",
        params![tenant_id, now_ms],
    )?;
    let vault_purged = tx.execute(
        "DELETE FROM vault_command_idempotency WHERE tenant_id = ?1 AND expires_at < ?2",
        params![tenant_id, now_ms],
    )?;
    Ok((cas_purged, vault_purged))
}

fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
}

fn match_secret_span(bytes: &[u8], i: usize) -> Option<(usize, &'static str)> {
    const PREFIXES: &[(&[u8], usize, &str)] = &[
        (b"AIza", 20, "[REDACTED:GOOGLE_AI_STUDIO]"),
        (b"jev_", 12, "[REDACTED:TYPESAFE_JEV]"),
        (b"sk-ant-", 16, "[REDACTED:ANTHROPIC]"),
        (b"sk-or-", 16, "[REDACTED:OPENROUTER]"),
        (b"sk-", 16, "[REDACTED:API_KEY]"),
        (b"ghp_", 16, "[REDACTED:GITHUB_PAT]"),
        (b"github_pat_", 16, "[REDACTED:GITHUB_PAT]"),
        (b"gho_", 16, "[REDACTED:GITHUB_TOKEN]"),
    ];
    for &(pfx, min_rest, label) in PREFIXES {
        if bytes[i..].starts_with(pfx) {
            let mut j = i + pfx.len();
            while j < bytes.len() && is_token_byte(bytes[j]) {
                j += 1;
            }
            if j - (i + pfx.len()) >= min_rest {
                return Some((j, label));
            }
        }
    }
    None
}

pub fn redact_secrets(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some((end, label)) = match_secret_span(bytes, i) {
            out.push_str(label);
            i = end;
            continue;
        }
        if let Some(ch) = input[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

pub fn is_valid_idempotency_key_str(raw: &str) -> bool {
    !raw.is_empty() && raw.len() <= 128 && raw.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

pub fn resolve_idempotency_key(
    headers: &HeaderMap,
    body_key: Option<&str>,
) -> Result<Option<String>, Response> {
    let header_key = match headers.get("x-idempotency-key") {
        Some(val) => {
            let s = val.to_str().map_err(|_| {
                safe_api_error_response(
                    StatusCode::BAD_REQUEST,
                    "INVALID_IDEMPOTENCY_KEY",
                    "Idempotency key must be visible ASCII (1..=128)",
                    false,
                    None,
                )
            })?;
            if !is_valid_idempotency_key_str(s) {
                return Err(safe_api_error_response(
                    StatusCode::BAD_REQUEST,
                    "INVALID_IDEMPOTENCY_KEY",
                    "Idempotency key must be visible ASCII (1..=128)",
                    false,
                    None,
                ));
            }
            Some(s.to_string())
        }
        None => None,
    };

    if let Some(bk) = body_key {
        if !is_valid_idempotency_key_str(bk) {
            return Err(safe_api_error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_IDEMPOTENCY_KEY",
                "Idempotency key must be visible ASCII (1..=128)",
                false,
                None,
            ));
        }
        if let Some(ref hk) = header_key {
            if hk != bk {
                return Err(safe_api_error_response(
                    StatusCode::BAD_REQUEST,
                    "IDEMPOTENCY_KEY_CONFLICT",
                    "Header and body idempotency keys do not match",
                    false,
                    None,
                ));
            }
        }
        return Ok(Some(bk.to_string()));
    }
    Ok(header_key)
}

pub fn compute_cas_payload_digest(
    raw_bytes: &[u8],
    session_id: &str,
    project_id: Option<&str>,
    file_ext: &str,
    mime_type: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(raw_bytes);
    h.update(b"\n");
    h.update(session_id.as_bytes());
    h.update(b"\n");
    h.update(project_id.unwrap_or("null").as_bytes());
    h.update(b"\n");
    h.update(file_ext.as_bytes());
    h.update(b"\n");
    h.update(mime_type.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn default_project_id_for_tenant(tenant_id: &str) -> (String, String) {
    if tenant_id == "local-default" {
        (
            "__workbench_default__".to_string(),
            "__workbench_default__".to_string(),
        )
    } else {
        let digest_hex = format!("{:x}", Sha256::digest(tenant_id.as_bytes()));
        let suffix = &digest_hex[..16];
        (
            format!("__workbench_default__{suffix}"),
            format!("__workbench_default__{suffix}"),
        )
    }
}

pub fn ensure_session_for_tenant(
    tx: &Transaction<'_>,
    tenant_id: &str,
    sid: &str,
    req_project_id: Option<&str>,
    now_ms: i64,
) -> Result<String, ChatCasTxError> {
    let sess: Option<(String, String)> = tx
        .query_row(
            "SELECT tenant_id, project_id FROM chat_sessions WHERE id = ?1",
            params![sid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((row_tenant, pid)) = sess {
        if row_tenant != tenant_id {
            return Err(ChatCasTxError::Http(safe_api_error_response(
                StatusCode::NOT_FOUND,
                "RESOURCE_NOT_FOUND",
                "Session not found",
                false,
                None,
            )));
        }
        if let Some(req_pid) = req_project_id {
            let p_tenant: Option<String> = tx
                .query_row(
                    "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
                    params![req_pid],
                    |r| r.get(0),
                )
                .optional()?;
            if p_tenant.as_deref() != Some(tenant_id) {
                return Err(ChatCasTxError::Http(safe_api_error_response(
                    StatusCode::NOT_FOUND,
                    "RESOURCE_NOT_FOUND",
                    "Project not found",
                    false,
                    None,
                )));
            }
        }
        return Ok(pid);
    }

    let pid = if let Some(req_pid) = req_project_id {
        let p_tenant: Option<String> = tx
            .query_row(
                "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
                params![req_pid],
                |r| r.get(0),
            )
            .optional()?;
        if p_tenant.as_deref() != Some(tenant_id) {
            return Err(ChatCasTxError::Http(safe_api_error_response(
                StatusCode::NOT_FOUND,
                "RESOURCE_NOT_FOUND",
                "Project not found",
                false,
                None,
            )));
        }
        req_pid.to_string()
    } else {
        let (def_id, def_slug) = default_project_id_for_tenant(tenant_id);
        tx.execute(
            "INSERT OR IGNORE INTO projects (id, slug, name, workspace_root, product_brief, tenant_id, provenance, status, created_at, updated_at)
             VALUES (?1, ?2, 'Workbench Default', '.', '', ?3, 'workbench', 'ACTIVE', ?4, ?4)",
            params![def_id, def_slug, tenant_id, now_ms],
        )?;
        def_id
    };
    tx.execute(
        "INSERT INTO chat_sessions (id, project_id, title, model_provider, tenant_id, created_at, updated_at)
         VALUES (?1, ?2, 'Workbench Chat', 'anthropic', ?3, ?4, ?4)",
        params![sid, pid, tenant_id, now_ms],
    )?;
    Ok(pid)
}

pub fn compute_vault_payload_digest(
    session_id: &str,
    project_id: Option<&str>,
    provider: &str,
    secret: &str,
    local_only: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(session_id.as_bytes());
    h.update(b"\n");
    h.update(project_id.unwrap_or("null").as_bytes());
    h.update(b"\n");
    h.update(provider.as_bytes());
    h.update(b"\n");
    h.update(if local_only { b"1" } else { b"0" });
    h.update(b"\n");
    h.update(secret.as_bytes());
    format!("{:x}", h.finalize())
}
