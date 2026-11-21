// @story 04.6
// @implements AC-02, AC-05 (CAS Streaming Multipart/Raw Upload, Quotas & Idempotency)
use std::path::Path;
use std::sync::Arc;

use axum::{
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::daemon_auth::{CasIngressTempGuard, VerifiedLegacyIngress};
use crate::routes::chat_cas_archive::{
    cleanup_uncommitted_final_blob, validate_cas_magic_and_ext, write_atomic_cas_blob,
};
use crate::routes::chat_cas_storage::{
    compute_cas_payload_digest, ensure_session_for_tenant, execute_immediate_with_retry,
    purge_expired_idempotency_receipts, resolve_idempotency_key, safe_api_error_response,
    ChatCasTxError, CAS_GC_LEASE_MS, CAS_MAX_SESSION_ATTACHMENTS,
    CAS_MAX_TENANT_INGRESS_24H_BYTES, CAS_MAX_TENANT_PHYSICAL_BYTES, IDEMPOTENCY_TTL_MS,
};
use crate::routes::DaemonAppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CasAttachmentDto {
    pub id: String,
    pub blob_sha256: String,
    pub session_id: String,
    pub project_id: Option<String>,
    pub filename: String,
    pub file_ext: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub storage_relpath: String,
    pub created_at: i64,
    pub replayed: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CasRawQueryParams {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub filename: Option<String>,
    pub idempotency_key: Option<String>,
}

struct ParsedCasInput {
    session_id: String,
    project_id: Option<String>,
    filename: String,
    body_idempotency_key: Option<String>,
    file_bytes: Vec<u8>,
}

fn sanitize_upload_filename(raw: &str) -> Result<String, Response> {
    let base = Path::new(raw.trim())
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::trim)
        .unwrap_or("");
    if base.is_empty()
        || base == "."
        || base == ".."
        || base.len() > 255
        || base.bytes().any(|b| b < 0x20 || b == 0x7f || b == b'/' || b == b'\\')
    {
        return Err(safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "CAS_INVALID_FILENAME",
            "Invalid attachment filename",
            false,
            None,
        ));
    }
    Ok(base.to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_attr(header_str: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=\"");
    let start = header_str.find(&pat)? + pat.len();
    let rest = &header_str[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_multipart_bytes(boundary: &str, raw: &[u8]) -> Result<ParsedCasInput, Response> {
    let delim = format!("--{boundary}").into_bytes();
    let mut parts_bounds = Vec::new();
    let mut pos = 0usize;
    while let Some(idx) = find_bytes(&raw[pos..], &delim) {
        let abs = pos + idx;
        let line_start_ok =
            abs == 0 || raw[..abs].ends_with(b"\r\n") || raw[..abs].ends_with(b"\n");
        let after = &raw[abs + delim.len()..];
        let line_end_ok =
            after.starts_with(b"\r\n") || after.starts_with(b"\n") || after.starts_with(b"--");
        if line_start_ok && line_end_ok {
            parts_bounds.push(abs);
            pos = abs + delim.len();
        } else {
            pos = abs + 1;
        }
    }
    if parts_bounds.len() < 2 {
        return Err(safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "CAS_INVALID_MULTIPART",
            "Malformed multipart upload payload",
            false,
            None,
        ));
    }
    let mut session_id = None;
    let mut project_id = None;
    let mut body_key = None;
    let mut filename = None;
    let mut file_bytes = None;

    for w in parts_bounds.windows(2) {
        let mut part = &raw[w[0] + delim.len()..w[1]];
        if part.starts_with(b"--") {
            break;
        }
        if part.starts_with(b"\r\n") {
            part = &part[2..];
        } else if part.starts_with(b"\n") {
            part = &part[1..];
        }
        let Some((hdr_end, sep_len)) = find_bytes(part, b"\r\n\r\n")
            .map(|i| (i, 4))
            .or_else(|| find_bytes(part, b"\n\n").map(|i| (i, 2)))
        else {
            continue;
        };
        let hdr_str = std::str::from_utf8(&part[..hdr_end]).unwrap_or("");
        let mut body = &part[hdr_end + sep_len..];
        if body.ends_with(b"\r\n") {
            body = &body[..body.len() - 2];
        } else if body.ends_with(b"\n") {
            body = &body[..body.len() - 1];
        }
        match extract_attr(hdr_str, "name").as_deref() {
            Some("sessionId") => session_id = String::from_utf8(body.to_vec()).ok(),
            Some("projectId") => {
                let s = String::from_utf8_lossy(body).trim().to_string();
                if !s.is_empty() {
                    project_id = Some(s);
                }
            }
            Some("idempotencyKey") => body_key = String::from_utf8(body.to_vec()).ok(),
            Some("file") => {
                filename = extract_attr(hdr_str, "filename");
                file_bytes = Some(body.to_vec());
            }
            _ => {}
        }
    }

    let sid = session_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).ok_or_else(|| {
        safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "CAS_INVALID_REQUEST",
            "Missing sessionId field",
            false,
            None,
        )
    })?;
    let fname = sanitize_upload_filename(&filename.unwrap_or_default())?;
    let bytes = file_bytes.ok_or_else(|| {
        safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "CAS_INVALID_FILE",
            "Missing file payload",
            false,
            None,
        )
    })?;
    Ok(ParsedCasInput {
        session_id: sid,
        project_id,
        filename: fname,
        body_idempotency_key: body_key,
        file_bytes: bytes,
    })
}

fn parse_cas_input(
    headers: &HeaderMap,
    query: &CasRawQueryParams,
    raw: &[u8],
) -> Result<ParsedCasInput, Response> {
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if let Some(idx) = ct.find("boundary=") {
        let b = ct[idx + 9..].split(';').next().unwrap_or("").trim().trim_matches('"');
        if !b.is_empty() {
            return parse_multipart_bytes(b, raw);
        }
    }
    let sid = query
        .session_id
        .as_deref()
        .or_else(|| headers.get("x-session-id").and_then(|v| v.to_str().ok()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            safe_api_error_response(
                StatusCode::BAD_REQUEST,
                "CAS_INVALID_REQUEST",
                "Missing sessionId parameter",
                false,
                None,
            )
        })?
        .to_string();
    let project_id = query
        .project_id
        .as_deref()
        .or_else(|| headers.get("x-project-id").and_then(|v| v.to_str().ok()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let fname_raw = query
        .filename
        .as_deref()
        .or_else(|| headers.get("x-filename").and_then(|v| v.to_str().ok()))
        .unwrap_or("");
    Ok(ParsedCasInput {
        session_id: sid,
        project_id,
        filename: sanitize_upload_filename(fname_raw)?,
        body_idempotency_key: query.idempotency_key.clone(),
        file_bytes: raw.to_vec(),
    })
}

pub async fn handle_cas_upload(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    Extension(staged): Extension<Arc<CasIngressTempGuard>>,
    Query(query): Query<CasRawQueryParams>,
    headers: HeaderMap,
) -> Response {
    if caller.principal.role == "viewer" {
        return safe_api_error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot upload attachments",
            false,
            None,
        );
    }
    let staged_path = staged.path.clone();
    let raw_ingress = match tokio::task::spawn_blocking(move || std::fs::read(&staged_path)).await {
        Ok(Ok(b)) => b,
        _ => {
            return safe_api_error_response(
                StatusCode::BAD_REQUEST,
                "CAS_INVALID_STREAM",
                "Failed to read staged upload",
                false,
                None,
            );
        }
    };
    let mut parsed = match parse_cas_input(&headers, &query, &raw_ingress) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    drop(raw_ingress);
    let idem_key = match resolve_idempotency_key(&headers, parsed.body_idempotency_key.as_deref()) {
        Ok(Some(k)) => k,
        Ok(None) => Uuid::new_v4().to_string(),
        Err(resp) => return resp,
    };
    let validated = match validate_cas_magic_and_ext(&parsed.filename, &parsed.file_bytes) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if matches!(validated.file_ext, "csv" | "tsv") {
        parsed.file_bytes = super::chat_cas_archive::neutralize_csv_tsv_formulas(
            &parsed.file_bytes,
            validated.file_ext,
        );
    }
    let blob_sha = format!("{:x}", Sha256::digest(&parsed.file_bytes));
    let payload_sha = compute_cas_payload_digest(
        &parsed.file_bytes,
        &parsed.session_id,
        parsed.project_id.as_deref(),
        validated.file_ext,
        validated.mime_type,
    );
    let now_ms = Utc::now().timestamp_millis();
    execute_cas_upload_core(
        &state,
        &caller.principal.tenant_id,
        parsed,
        &idem_key,
        &blob_sha,
        &payload_sha,
        validated.file_ext,
        validated.mime_type,
        now_ms,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn execute_cas_upload_core(
    state: &DaemonAppState,
    tenant_id: &str,
    parsed: ParsedCasInput,
    idem_key: &str,
    blob_sha: &str,
    payload_sha: &str,
    file_ext: &'static str,
    mime_type: &'static str,
    now_ms: i64,
) -> Response {
    let sha_lock = state.cas_sha_lock(blob_sha).await;
    let _sha_guard = sha_lock.lock().await;
    let cas_root = state.registry.pool().workspace_root().join(".cowok-design/cas");
    let byte_size = parsed.file_bytes.len() as i64;
    let relpath = format!(".cowok-design/cas/{blob_sha}");

    let precheck = execute_immediate_with_retry(
        || {
            state.registry.pool().connection().map_err(|_| {
                safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Database unavailable",
                    false,
                    None,
                )
            })
        },
        |tx| {
            purge_expired_idempotency_receipts(tx, tenant_id, now_ms)?;
            let existing: Option<(String, String)> = tx
                .query_row(
                    "SELECT payload_sha256, attachment_id FROM cas_command_idempotency
                     WHERE tenant_id = ?1 AND session_id = ?2 AND idempotency_key = ?3",
                    params![tenant_id, parsed.session_id, idem_key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((saved_digest, att_id)) = existing {
                if saved_digest != payload_sha {
                    return Err(ChatCasTxError::Http(safe_api_error_response(
                        StatusCode::CONFLICT,
                        "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH",
                        "Idempotency key already used with different payload",
                        false,
                        None,
                    )));
                }
                let dto = load_cas_attachment_dto(tx, tenant_id, &att_id, true)?;
                return Ok(Some(dto));
            }
            ensure_session_for_tenant(
                tx,
                tenant_id,
                &parsed.session_id,
                parsed.project_id.as_deref(),
                now_ms,
            )?;
            enforce_cas_quotas(tx, tenant_id, &parsed.session_id, blob_sha, byte_size, now_ms)?;
            Ok(None)
        },
    )
    .await;

    match precheck {
        Err(resp) => return resp,
        Ok(Some(replayed_dto)) => return (StatusCode::OK, Json(replayed_dto)).into_response(),
        Ok(None) => {}
    }

    let cas_root_io = cas_root.clone();
    let blob_sha_io = blob_sha.to_string();
    let bytes_io = parsed.file_bytes.clone();
    let write_res = tokio::task::spawn_blocking(move || {
        write_atomic_cas_blob(&cas_root_io, &blob_sha_io, &bytes_io)
    })
    .await;
    let created_new_file = match write_res {
        Ok(Ok((_, created))) => created,
        Ok(Err(resp)) => return resp,
        Err(_) => {
            return safe_api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CAS_STORAGE_ERROR",
                "Failed to persist attachment content",
                false,
                None,
            );
        }
    };

    let res = execute_immediate_with_retry(
        || {
            state.registry.pool().connection().map_err(|_| {
                safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Database unavailable",
                    false,
                    None,
                )
            })
        },
        |tx| {
            let existing: Option<(String, String)> = tx
                .query_row(
                    "SELECT payload_sha256, attachment_id FROM cas_command_idempotency
                     WHERE tenant_id = ?1 AND session_id = ?2 AND idempotency_key = ?3",
                    params![tenant_id, parsed.session_id, idem_key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((saved_digest, att_id)) = existing {
                if saved_digest != payload_sha {
                    return Err(ChatCasTxError::Http(safe_api_error_response(
                        StatusCode::CONFLICT,
                        "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH",
                        "Idempotency key already used with different payload",
                        false,
                        None,
                    )));
                }
                let dto = load_cas_attachment_dto(tx, tenant_id, &att_id, true)?;
                return Ok((StatusCode::OK, dto));
            }
            ensure_session_for_tenant(
                tx,
                tenant_id,
                &parsed.session_id,
                parsed.project_id.as_deref(),
                now_ms,
            )?;
            enforce_cas_quotas(tx, tenant_id, &parsed.session_id, blob_sha, byte_size, now_ms)?;
            let att_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO cas_blobs (sha256, tenant_id, byte_size, mime_type, file_ext, storage_relpath, ref_count, gc_lease_until, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7)
                 ON CONFLICT(sha256) DO UPDATE SET ref_count = cas_blobs.ref_count + 1",
                params![blob_sha, tenant_id, byte_size, mime_type, file_ext, relpath, now_ms],
            )?;
            tx.execute(
                "INSERT INTO cas_attachments (id, blob_sha256, tenant_id, project_id, session_id, filename, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![att_id, blob_sha, tenant_id, parsed.project_id, parsed.session_id, parsed.filename, now_ms],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO tenant_cas_blob_usage (tenant_id, blob_sha256, byte_size, first_attached_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![tenant_id, blob_sha, byte_size, now_ms],
            )?;
            tx.execute(
                "INSERT INTO cas_upload_ingress (id, tenant_id, session_id, byte_size, accepted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![Uuid::new_v4().to_string(), tenant_id, parsed.session_id, byte_size, now_ms],
            )?;
            tx.execute(
                "INSERT INTO cas_command_idempotency (id, tenant_id, session_id, idempotency_key, payload_sha256, attachment_id, accepted_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    Uuid::new_v4().to_string(),
                    tenant_id,
                    parsed.session_id,
                    idem_key,
                    payload_sha,
                    att_id,
                    now_ms,
                    now_ms.saturating_add(IDEMPOTENCY_TTL_MS),
                ],
            )?;
            Ok((
                StatusCode::CREATED,
                CasAttachmentDto {
                    id: att_id,
                    blob_sha256: blob_sha.to_string(),
                    session_id: parsed.session_id.clone(),
                    project_id: parsed.project_id.clone(),
                    filename: parsed.filename.clone(),
                    file_ext: file_ext.to_string(),
                    mime_type: mime_type.to_string(),
                    byte_size,
                    storage_relpath: relpath.clone(),
                    created_at: now_ms,
                    replayed: false,
                },
            ))
        },
    )
    .await;

    match res {
        Ok((status, dto)) => (status, Json(dto)).into_response(),
        Err(resp) => {
            if created_new_file {
                let pool = state.registry.pool().clone();
                let cas_rb = cas_root.clone();
                let sha_rb = blob_sha.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(mut c) = pool.connection() {
                        cleanup_uncommitted_final_blob(&mut c, &cas_rb, &sha_rb);
                    }
                })
                .await;
            }
            resp
        }
    }
}

fn enforce_cas_quotas(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    session_id: &str,
    blob_sha: &str,
    byte_size: i64,
    now_ms: i64,
) -> Result<(), ChatCasTxError> {
    let sess_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM cas_attachments WHERE tenant_id = ?1 AND session_id = ?2",
        params![tenant_id, session_id],
        |r| r.get(0),
    )?;
    if sess_count >= CAS_MAX_SESSION_ATTACHMENTS {
        return Err(ChatCasTxError::Http(safe_api_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "CAS_QUOTA_EXCEEDED",
            "Session attachment limit reached",
            false,
            Some("SESSION_ATTACHMENT_COUNT"),
        )));
    }
    let window_start = now_ms.saturating_sub(IDEMPOTENCY_TTL_MS);
    let ingress_24h: i64 = tx.query_row(
        "SELECT COALESCE(SUM(byte_size), 0) FROM cas_upload_ingress WHERE tenant_id = ?1 AND accepted_at >= ?2",
        params![tenant_id, window_start],
        |r| r.get(0),
    )?;
    if ingress_24h.saturating_add(byte_size) > CAS_MAX_TENANT_INGRESS_24H_BYTES {
        return Err(ChatCasTxError::Http(safe_api_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "CAS_QUOTA_EXCEEDED",
            "Rolling 24h tenant upload quota exceeded",
            false,
            Some("TENANT_INGRESS_24H"),
        )));
    }
    let already_billed: Option<i64> = tx
        .query_row(
            "SELECT byte_size FROM tenant_cas_blob_usage WHERE tenant_id = ?1 AND blob_sha256 = ?2",
            params![tenant_id, blob_sha],
            |r| r.get(0),
        )
        .optional()?;
    if already_billed.is_none() {
        let physical_used: i64 = tx.query_row(
            "SELECT COALESCE(SUM(byte_size), 0) FROM tenant_cas_blob_usage WHERE tenant_id = ?1",
            params![tenant_id],
            |r| r.get(0),
        )?;
        if physical_used.saturating_add(byte_size) > CAS_MAX_TENANT_PHYSICAL_BYTES {
            return Err(ChatCasTxError::Http(safe_api_error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "CAS_QUOTA_EXCEEDED",
                "Tenant physical CAS storage quota exceeded",
                false,
                Some("TENANT_PHYSICAL_BYTES"),
            )));
        }
    }
    Ok(())
}

fn load_cas_attachment_dto(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    attachment_id: &str,
    replayed: bool,
) -> Result<CasAttachmentDto, ChatCasTxError> {
    tx.query_row(
        "SELECT a.id, a.blob_sha256, a.session_id, a.project_id, a.filename, b.file_ext, b.mime_type, b.byte_size, b.storage_relpath, a.created_at
         FROM cas_attachments a
         JOIN cas_blobs b ON b.sha256 = a.blob_sha256
         WHERE a.id = ?1 AND a.tenant_id = ?2",
        params![attachment_id, tenant_id],
        |r| {
            Ok(CasAttachmentDto {
                id: r.get(0)?,
                blob_sha256: r.get(1)?,
                session_id: r.get(2)?,
                project_id: r.get(3)?,
                filename: r.get(4)?,
                file_ext: r.get(5)?,
                mime_type: r.get(6)?,
                byte_size: r.get(7)?,
                storage_relpath: r.get(8)?,
                created_at: r.get(9)?,
                replayed,
            })
        },
    )
    .map_err(ChatCasTxError::Sqlite)
}

pub async fn delete_cas_attachment_scoped(
    state: &DaemonAppState,
    tenant_id: &str,
    attachment_id: &str,
    now_ms: i64,
) -> Result<bool, Response> {
    let conn = state.registry.pool().connection().map_err(|_| {
        safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Database unavailable",
            false,
            None,
        )
    })?;
    let sha_opt: Option<String> = conn
        .query_row(
            "SELECT blob_sha256 FROM cas_attachments WHERE id = ?1 AND tenant_id = ?2",
            params![attachment_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .unwrap_or(None);
    let Some(sha) = sha_opt else {
        return Ok(false);
    };
    let lock = state.cas_sha_lock(&sha).await;
    let _guard = lock.lock().await;
    let mut c: Connection = state.registry.pool().connection().map_err(|_| {
        safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Database unavailable",
            false,
            None,
        )
    })?;
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| {
            safe_api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to begin attachment delete",
                false,
                None,
            )
        })?;
    let deleted = tx
        .execute(
            "DELETE FROM cas_attachments WHERE id = ?1 AND tenant_id = ?2",
            params![attachment_id, tenant_id],
        )
        .unwrap_or(0);
    if deleted > 0 {
        let _ = tx.execute(
            "UPDATE cas_blobs
             SET ref_count = MAX(ref_count - 1, 0),
                 gc_lease_until = CASE WHEN ref_count <= 1 THEN ?2 ELSE gc_lease_until END
             WHERE sha256 = ?1",
            params![sha, now_ms.saturating_add(CAS_GC_LEASE_MS)],
        );
        let remaining_for_tenant: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM cas_attachments WHERE tenant_id = ?1 AND blob_sha256 = ?2",
                params![tenant_id, sha],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if remaining_for_tenant == 0 {
            let _ = tx.execute(
                "DELETE FROM tenant_cas_blob_usage WHERE tenant_id = ?1 AND blob_sha256 = ?2",
                params![tenant_id, sha],
            );
        }
    }
    tx.commit().map_err(|_| {
        safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Failed to commit attachment deletion",
            false,
            None,
        )
    })?;
    Ok(deleted > 0)
}
