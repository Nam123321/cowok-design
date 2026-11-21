// @story 04.6
// @implements AC-01, AC-04, AC-05 (Persistent Chat Messages & Zero-Leak Key Vault Routes)
use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::daemon_auth::VerifiedLegacyIngress;
use crate::routes::chat_cas_storage::{
    compute_vault_payload_digest, ensure_session_for_tenant, execute_immediate_with_retry,
    purge_expired_idempotency_receipts, redact_secrets, resolve_idempotency_key,
    safe_api_error_response, ChatCasTxError, IDEMPOTENCY_TTL_MS,
};
use crate::routes::chat_cas_upload_handler::handle_cas_upload;
use crate::routes::DaemonAppState;
use crate::vault::VaultStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultReceiptDto {
    pub receipt_id: String,
    pub provider: String,
    pub presenter_mask: String,
    pub status: String,
    pub local_only: bool,
    pub high_entropy_warning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDto {
    pub id: String,
    pub session_id: String,
    pub project_id: String,
    pub role: String,
    pub prose_markdown: String,
    pub seq_no: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_receipt: Option<VaultReceiptDto>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagesListResponse {
    pub session_id: String,
    pub project_id: String,
    pub messages: Vec<ChatMessageDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatMessageBody {
    #[serde(default = "default_user_role")]
    pub role: String,
    #[serde(default)]
    pub prose_markdown: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub vault_receipt: Option<VaultReceiptDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatVaultIngestBody {
    pub session_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    pub provider: String,
    pub raw_secret_envelope: String,
    #[serde(default)]
    pub enterprise_local_only_lock: bool,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceVaultReceiptBody {
    #[serde(default = "default_custom_provider")]
    pub provider: String,
    pub raw_secret_envelope: String,
    #[serde(default)]
    pub enterprise_local_only_lock: bool,
}

fn default_user_role() -> String {
    "user".to_string()
}

fn default_custom_provider() -> String {
    "custom".to_string()
}

pub fn chat_cas_vault_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/chat-sessions/:sid/messages",
            get(handle_get_chat_messages).post(handle_post_chat_message),
        )
        .route("/api/v1/cas/upload", post(handle_cas_upload))
        .route("/api/v1/vault/chat-ingest", post(handle_vault_chat_ingest))
        .route(
            "/api/v1/vault/receipts/:receipt_id",
            post(handle_replace_vault_receipt).delete(handle_revoke_vault_receipt),
        )
}

fn make_presenter_mask(secret: &str) -> String {
    let trimmed = secret.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 12 {
        let head: String = chars.iter().take(2).collect();
        let tail: String = chars[chars.len().saturating_sub(2)..].iter().collect();
        return format!("{head}...{tail}");
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars[chars.len().saturating_sub(4)..].iter().collect();
    format!("{head}...{tail}")
}

fn is_valid_provider(p: &str) -> bool {
    matches!(
        p,
        "google_ai_studio" | "typesafe_jev" | "anthropic" | "openai" | "custom"
    )
}

fn evaluate_vault_key_status(provider: &str, secret: &str) -> &'static str {
    let trimmed = secret.trim();
    let ok = match provider {
        "google_ai_studio" => trimmed.starts_with("AIza") && trimmed.len() >= 24,
        "typesafe_jev" => trimmed.starts_with("jev_") && trimmed.len() >= 16,
        "anthropic" => trimmed.starts_with("sk-ant-") && trimmed.len() >= 24,
        "openai" => trimmed.starts_with("sk-") && trimmed.len() >= 20,
        "custom" => trimmed.len() >= 8,
        _ => false,
    };
    if ok {
        "VERIFIED"
    } else {
        "INVALID"
    }
}

fn sanitize_vault_receipt(r: &VaultReceiptDto) -> VaultReceiptDto {
    let provider = if is_valid_provider(&r.provider) {
        r.provider.clone()
    } else {
        "custom".to_string()
    };
    let status = match r.status.as_str() {
        "VERIFIED" | "RATE_LIMITED_429" | "INVALID" | "REVOKED" => r.status.clone(),
        _ => "INVALID".to_string(),
    };
    VaultReceiptDto {
        receipt_id: redact_secrets(&r.receipt_id),
        provider,
        presenter_mask: redact_secrets(&r.presenter_mask),
        status,
        local_only: r.local_only,
        high_entropy_warning: r.high_entropy_warning,
    }
}

fn shannon_entropy_bits(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    let bytes = s.as_bytes();
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let len = bytes.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = (c as f64) / len;
            -p * p.log2()
        })
        .sum()
}

pub async fn handle_get_chat_messages(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(sid): AxumPath<String>,
) -> Response {
    let Ok(conn) = state.registry.pool().connection() else {
        return safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Database unavailable",
            false,
            None,
        );
    };
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT tenant_id, project_id FROM chat_sessions WHERE id = ?1",
            params![sid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .ok()
        .flatten();
    let Some((sess_tenant, project_id)) = row else {
        return safe_api_error_response(
            StatusCode::NOT_FOUND,
            "RESOURCE_NOT_FOUND",
            "Session not found",
            false,
            None,
        );
    };
    if sess_tenant != caller.principal.tenant_id {
        return safe_api_error_response(
            StatusCode::NOT_FOUND,
            "RESOURCE_NOT_FOUND",
            "Session not found",
            false,
            None,
        );
    }
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, role, prose_markdown, COALESCE(seq_no, 0), turn_receipt_json, created_at
         FROM chat_messages
         WHERE session_id = ?1 AND tenant_id = ?2
         ORDER BY COALESCE(seq_no, 0) ASC, created_at ASC, id ASC",
    ) {
        Ok(s) => s,
        Err(_) => {
            return safe_api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Failed to load messages",
                false,
                None,
            );
        }
    };
    let messages: Vec<ChatMessageDto> = stmt
        .query_map(params![sid, caller.principal.tenant_id], |r| {
            let raw_prose: String = r.get(3)?;
            let raw_receipt: Option<String> = r.get(5)?;
            let vault_receipt = raw_receipt
                .as_deref()
                .and_then(|s| serde_json::from_str::<VaultReceiptDto>(s).ok());
            Ok(ChatMessageDto {
                id: r.get(0)?,
                session_id: r.get(1)?,
                project_id: project_id.clone(),
                role: r.get(2)?,
                prose_markdown: redact_secrets(&raw_prose),
                seq_no: r.get(4)?,
                vault_receipt,
                created_at: r.get(6)?,
            })
        })
        .ok()
        .map(|it| it.filter_map(Result::ok).collect())
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(ChatMessagesListResponse {
            session_id: sid,
            project_id,
            messages,
        }),
    )
        .into_response()
}

pub async fn handle_post_chat_message(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(sid): AxumPath<String>,
    Json(body): Json<CreateChatMessageBody>,
) -> Response {
    if caller.principal.role == "viewer" {
        return safe_api_error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot persist messages",
            false,
            None,
        );
    }
    if sid.trim().is_empty()
        || !matches!(body.role.as_str(), "user" | "assistant" | "system")
        || body.prose_markdown.trim().is_empty()
    {
        return safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid chat message payload",
            false,
            None,
        );
    }
    let clean_prose = redact_secrets(&body.prose_markdown);
    let sanitized_receipt = body.vault_receipt.as_ref().map(sanitize_vault_receipt);
    let receipt_json = sanitized_receipt
        .as_ref()
        .and_then(|r| serde_json::to_string(r).ok());
    let now_ms = Utc::now().timestamp_millis();
    let tenant_id = caller.principal.tenant_id.clone();

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
            let project_id = ensure_session_for_tenant(
                tx,
                &tenant_id,
                &sid,
                body.project_id.as_deref(),
                now_ms,
            )?;
            let next_seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(seq_no), 0) + 1 FROM chat_messages WHERE session_id = ?1",
                params![sid],
                |r| r.get(0),
            )?;
            let msg_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO chat_messages (id, tenant_id, session_id, role, prose_markdown, turn_receipt_json, seq_no, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![msg_id, tenant_id, sid, body.role, clean_prose, receipt_json, next_seq, now_ms],
            )?;
            Ok(ChatMessageDto {
                id: msg_id,
                session_id: sid.clone(),
                project_id,
                role: body.role.clone(),
                prose_markdown: clean_prose.clone(),
                seq_no: next_seq,
                vault_receipt: sanitized_receipt.clone(),
                created_at: now_ms,
            })
        },
    )
    .await;
    match res {
        Ok(dto) => {
            let _ = state.sse_tx.send(dto.project_id.clone());
            (StatusCode::CREATED, Json(dto)).into_response()
        }
        Err(resp) => resp,
    }
}

pub async fn handle_vault_chat_ingest(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    headers: HeaderMap,
    Json(body): Json<ChatVaultIngestBody>,
) -> Response {
    let secret = Zeroizing::new(body.raw_secret_envelope);
    if caller.principal.role == "viewer" {
        return safe_api_error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot ingest credentials",
            false,
            None,
        );
    }
    if !is_valid_provider(&body.provider) || secret.trim().len() < 8 || secret.len() > 512 {
        return safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid credential ingest payload",
            false,
            None,
        );
    }
    let idem_key = match resolve_idempotency_key(&headers, body.idempotency_key.as_deref()) {
        Ok(Some(k)) => k,
        Ok(None) => Uuid::new_v4().to_string(),
        Err(resp) => return resp,
    };
    let payload_sha = compute_vault_payload_digest(
        &body.session_id,
        body.project_id.as_deref(),
        &body.provider,
        &secret,
        body.enterprise_local_only_lock,
    );
    let now_ms = Utc::now().timestamp_millis();
    let tenant_id = caller.principal.tenant_id.clone();
    let vault_lock = state
        .cas_sha_lock(&format!("vault:{tenant_id}:{}:{idem_key}", body.session_id))
        .await;
    let _vault_guard = vault_lock.lock().await;
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
            purge_expired_idempotency_receipts(tx, &tenant_id, now_ms)?;
            let existing: Option<(String, String)> = tx
                .query_row(
                    "SELECT payload_sha256, receipt_json FROM vault_command_idempotency
                     WHERE tenant_id = ?1 AND session_id = ?2 AND idempotency_key = ?3",
                    params![tenant_id, body.session_id, idem_key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((saved_sha, saved_json)) = existing {
                if saved_sha != payload_sha {
                    return Err(ChatCasTxError::Http(safe_api_error_response(
                        StatusCode::CONFLICT,
                        "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH",
                        "Idempotency key already used with different payload",
                        false,
                        None,
                    )));
                }
                let parsed: VaultReceiptDto = serde_json::from_str(&saved_json).map_err(|_| {
                    ChatCasTxError::Http(safe_api_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "INTERNAL_ERROR",
                        "Corrupt vault receipt",
                        false,
                        None,
                    ))
                })?;
                return Ok(Some(parsed));
            }
            ensure_session_for_tenant(
                tx,
                &tenant_id,
                &body.session_id,
                body.project_id.as_deref(),
                now_ms,
            )?;
            Ok(None)
        },
    )
    .await;

    match precheck {
        Err(resp) => return resp,
        Ok(Some(replayed_dto)) => return (StatusCode::OK, Json(replayed_dto)).into_response(),
        Ok(None) => {}
    }

    let receipt_id = format!("rcpt-{}", Uuid::new_v4());
    let key_status = evaluate_vault_key_status(&body.provider, &secret);
    let dto = VaultReceiptDto {
        receipt_id: receipt_id.clone(),
        provider: body.provider.clone(),
        presenter_mask: make_presenter_mask(&secret),
        status: key_status.to_string(),
        local_only: body.enterprise_local_only_lock,
        high_entropy_warning: shannon_entropy_bits(secret.trim()) >= 3.5,
    };
    let vkey = format!("receipt:{}", dto.receipt_id);
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let tid_seal = tenant_id.clone();
    let vkey_seal = vkey.clone();
    let sec_seal = secret.clone();
    let sealed_ok = tokio::task::spawn_blocking(move || {
        let store = VaultStore::from_workspace_root(&ws_root);
        store.upsert_secret_scoped(&tid_seal, &vkey_seal, &sec_seal, None).is_ok()
    })
    .await
    .unwrap_or(false);
    if !sealed_ok {
        return safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "VAULT_CUSTODY_ERROR",
            "Failed to seal credential into vault",
            false,
            None,
        );
    }

    let dto_db = dto.clone();
    let commit_res = execute_immediate_with_retry(
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
            let r_json = serde_json::to_string(&dto_db).map_err(|_| {
                ChatCasTxError::Http(safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Failed to encode vault receipt",
                    false,
                    None,
                ))
            })?;
            tx.execute(
                "INSERT INTO vault_command_idempotency (id, tenant_id, session_id, idempotency_key, payload_sha256, receipt_id, receipt_json, accepted_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    Uuid::new_v4().to_string(),
                    tenant_id,
                    body.session_id,
                    idem_key,
                    payload_sha,
                    dto_db.receipt_id,
                    r_json,
                    now_ms,
                    now_ms.saturating_add(IDEMPOTENCY_TTL_MS),
                ],
            )?;
            Ok(())
        },
    )
    .await;

    if let Err(resp) = commit_res {
        let ws_rb = state.registry.pool().workspace_root().to_path_buf();
        let tid_rb = tenant_id.clone();
        let vkey_rb = vkey;
        let _ = tokio::task::spawn_blocking(move || {
            let store = VaultStore::from_workspace_root(&ws_rb);
            let _ = store.delete_secret_scoped(&tid_rb, &vkey_rb, None);
        })
        .await;
        return resp;
    }
    state.ram_vault.upsert(&tenant_id, &vkey, &secret);
    (StatusCode::CREATED, Json(dto)).into_response()
}

pub async fn handle_replace_vault_receipt(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(receipt_id): AxumPath<String>,
    Json(body): Json<ReplaceVaultReceiptBody>,
) -> Response {
    let secret = Zeroizing::new(body.raw_secret_envelope);
    if caller.principal.role == "viewer" {
        return safe_api_error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot rotate credentials",
            false,
            None,
        );
    }
    if !is_valid_provider(&body.provider) || secret.trim().len() < 8 || secret.len() > 512 {
        return safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid credential replacement payload",
            false,
            None,
        );
    }
    let tenant_id = caller.principal.tenant_id.clone();
    let rcpt_lock = state
        .cas_sha_lock(&format!("vault-rcpt:{tenant_id}:{receipt_id}"))
        .await;
    let _rcpt_guard = rcpt_lock.lock().await;
    let vkey = format!("receipt:{receipt_id}");
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let tid_chk = tenant_id.clone();
    let vkey_chk = vkey.clone();
    let exists_disk = tokio::task::spawn_blocking(move || {
        VaultStore::from_workspace_root(&ws_root)
            .contains_secret_scoped(&tid_chk, &vkey_chk, None)
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false);
    let exists_ram = state.ram_vault.contains(&tenant_id, &vkey);
    if !exists_disk && !exists_ram {
        return safe_api_error_response(
            StatusCode::NOT_FOUND,
            "RESOURCE_NOT_FOUND",
            "Vault receipt not found",
            false,
            None,
        );
    }
    let ws_up = state.registry.pool().workspace_root().to_path_buf();
    let tid_up = tenant_id.clone();
    let vkey_up = vkey.clone();
    let sec_up = secret.clone();
    let up_ok = tokio::task::spawn_blocking(move || {
        VaultStore::from_workspace_root(&ws_up)
            .upsert_secret_scoped(&tid_up, &vkey_up, &sec_up, None)
            .is_ok()
    })
    .await
    .unwrap_or(false);
    if !up_ok {
        return safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "VAULT_CUSTODY_ERROR",
            "Failed to rotate credential in vault",
            false,
            None,
        );
    }
    state.ram_vault.upsert(&tenant_id, &vkey, &secret);
    let status_val = evaluate_vault_key_status(&body.provider, &secret);
    let updated = VaultReceiptDto {
        receipt_id,
        provider: body.provider,
        presenter_mask: make_presenter_mask(&secret),
        status: status_val.to_string(),
        local_only: body.enterprise_local_only_lock,
        high_entropy_warning: shannon_entropy_bits(secret.trim()) >= 3.5,
    };
    let updated_db = updated.clone();
    let tid_db = tenant_id.clone();
    let db_res = execute_immediate_with_retry(
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
            let r_json = serde_json::to_string(&updated_db).map_err(|_| {
                ChatCasTxError::Http(safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Failed to encode vault receipt",
                    false,
                    None,
                ))
            })?;
            tx.execute(
                "UPDATE vault_command_idempotency SET receipt_json = ?1 WHERE tenant_id = ?2 AND receipt_id = ?3",
                params![r_json, tid_db, updated_db.receipt_id],
            )?;
            tx.execute(
                "UPDATE chat_messages SET turn_receipt_json = ?1
                 WHERE tenant_id = ?2 AND turn_receipt_json IS NOT NULL
                   AND json_extract(turn_receipt_json, '$.receiptId') = ?3",
                params![r_json, tid_db, updated_db.receipt_id],
            )?;
            Ok(())
        },
    )
    .await;
    if let Err(resp) = db_res {
        return resp;
    }
    (StatusCode::OK, Json(updated)).into_response()
}

pub async fn handle_revoke_vault_receipt(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(receipt_id): AxumPath<String>,
) -> Response {
    if caller.principal.role == "viewer" {
        return safe_api_error_response(
            StatusCode::FORBIDDEN,
            "AUTH_FORBIDDEN",
            "Viewer role cannot revoke credentials",
            false,
            None,
        );
    }
    let tenant_id = caller.principal.tenant_id.clone();
    let rcpt_lock = state
        .cas_sha_lock(&format!("vault-rcpt:{tenant_id}:{receipt_id}"))
        .await;
    let _rcpt_guard = rcpt_lock.lock().await;
    let vkey = format!("receipt:{receipt_id}");
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let tid_chk = tenant_id.clone();
    let vkey_chk = vkey.clone();
    let exists_disk = tokio::task::spawn_blocking(move || {
        VaultStore::from_workspace_root(&ws_root)
            .contains_secret_scoped(&tid_chk, &vkey_chk, None)
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false);
    let exists_ram = state.ram_vault.contains(&tenant_id, &vkey);
    if !exists_disk && !exists_ram {
        return safe_api_error_response(
            StatusCode::NOT_FOUND,
            "RESOURCE_NOT_FOUND",
            "Vault receipt not found",
            false,
            None,
        );
    }
    let ws_del = state.registry.pool().workspace_root().to_path_buf();
    let tid_del = tenant_id.clone();
    let vkey_del = vkey.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let _ = VaultStore::from_workspace_root(&ws_del).delete_secret_scoped(&tid_del, &vkey_del, None);
    })
    .await;
    state.ram_vault.remove(&tenant_id, &vkey);
    let tid_db = tenant_id.clone();
    let rid_db = receipt_id.clone();
    let db_res = execute_immediate_with_retry(
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
            tx.execute(
                "DELETE FROM vault_command_idempotency WHERE tenant_id = ?1 AND receipt_id = ?2",
                params![tid_db, rid_db],
            )?;
            tx.execute(
                "UPDATE chat_messages SET turn_receipt_json = NULL
                 WHERE tenant_id = ?1 AND turn_receipt_json IS NOT NULL
                   AND json_extract(turn_receipt_json, '$.receiptId') = ?2",
                params![tid_db, rid_db],
            )?;
            Ok(())
        },
    )
    .await;
    if let Err(resp) = db_res {
        return resp;
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "receiptId": receipt_id,
            "status": "REVOKED"
        })),
    )
        .into_response()
}
