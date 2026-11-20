// @story 06.1
// @implements AC-01, AC-02, AC-04, AC-05
use std::collections::HashSet;
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Component, Path};
use std::time::{Duration, SystemTime};

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use presentation_cms::{compute_sha256_hex, is_valid_slide_uid, MAX_SLIDE_HTML_BYTES};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::routes::fs_jail;
use crate::routes::{constant_time_eq, DaemonAppState};

pub(crate) const ORPHAN_TMP_AGE_SECS: u64 = 60;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_ff_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InnerError {
    code: String,
    message: String,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ErrorDetails>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: InnerError,
}

pub(crate) fn err_resp(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    retryable: bool,
    details: Option<ErrorDetails>,
) -> Response {
    (
        status,
        Json(ErrorEnvelope {
            error: InnerError {
                code: code.to_string(),
                message: message.into(),
                retryable,
                details,
            },
        }),
    )
        .into_response()
}

pub(crate) fn stage_slide_html_file(
    ws_root: &Path,
    deck_id: &str,
    slide_uid: &str,
    html: &str,
) -> Result<String, Response> {
    if !is_valid_slide_uid(slide_uid) {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_SLIDE_UID",
            "Invalid slideUid format",
            false,
            None,
        ));
    }

    let rel_path = format!("presentations/{deck_id}/slides/{slide_uid}.html");
    fs_jail::inspect_workspace_relative_jail(ws_root, &rel_path).map_err(|e| {
        err_resp(
            StatusCode::FORBIDDEN,
            "E_FS_JAIL_VIOLATION",
            format!("Slide stage path rejected: {e}"),
            false,
            None,
        )
    })?;

    let ws_fd = fs_jail::open_dir_abs_nofollow(ws_root).map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!("Failed opening workspace root cleanly: {e}"),
            false,
            None,
        )
    })?;

    let cur_fd = fs_jail::ensure_child_dir_fd(&ws_fd, OsStr::new("presentations"), 0o755)
        .and_then(|d| fs_jail::ensure_child_dir_fd(&d, OsStr::new(deck_id), 0o755))
        .and_then(|d| fs_jail::ensure_child_dir_fd(&d, OsStr::new("slides"), 0o755))
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "E_FS_STAGE_FAILED",
                format!("Failed creating deck directory cleanly: {e}"),
                false,
                None,
            )
        })?;

    let file_name = format!("{slide_uid}.html");
    fs_jail::write_file_in_dir_fd(&cur_fd, OsStr::new(&file_name), html.as_bytes(), 0o644)
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "E_FS_STAGE_FAILED",
                format!("Failed staging slide HTML atomically: {e}"),
                false,
                None,
            )
        })?;
    if unsafe { libc::fsync(cur_fd.as_raw_fd()) } != 0 {
        let err = std::io::Error::last_os_error();
        return Err(err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!("Failed directory fsync after staging slide HTML: {err}"),
            true,
            None,
        ));
    }
    drop(cur_fd);

    Ok(rel_path)
}

/// Stages an immutable, transaction-owned slide HTML artifact (`*.r{next_revision}.{tx_uuid}.html`)
/// via `openat(O_CREAT|O_EXCL|O_NOFOLLOW)` -> `fsync(file)` -> `renameat` -> `fsync(dir)` BEFORE SQLite commit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stage_tx_owned_slide_html_file(
    ws_root: &Path,
    deck_id: &str,
    slide_uid: &str,
    is_one_page: bool,
    next_revision: i64,
    tx_uuid: &str,
    html: &str,
) -> Result<String, Response> {
    if !is_valid_slide_uid(slide_uid) {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_SLIDE_UID",
            "Invalid slideUid format",
            false,
            None,
        ));
    }
    if tx_uuid.is_empty()
        || tx_uuid.len() > 64
        || !tx_uuid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            "Invalid transaction UUID for slide staging",
            false,
            None,
        ));
    }

    let final_name = if is_one_page {
        format!("deck.r{next_revision}.{tx_uuid}.html")
    } else {
        format!("{slide_uid}.r{next_revision}.{tx_uuid}.html")
    };
    let tmp_name = format!(".cowok-tmp-{slide_uid}-{tx_uuid}.html");
    let rel_path = format!("presentations/{deck_id}/slides/{final_name}");

    fs_jail::inspect_workspace_relative_jail(ws_root, &rel_path).map_err(|e| {
        err_resp(
            StatusCode::FORBIDDEN,
            "E_FS_JAIL_VIOLATION",
            format!("Slide stage path rejected: {e}"),
            false,
            None,
        )
    })?;

    let ws_fd = fs_jail::open_dir_abs_nofollow(ws_root).map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!("Failed opening workspace root cleanly: {e}"),
            false,
            None,
        )
    })?;

    let cur_fd = fs_jail::ensure_child_dir_fd(&ws_fd, OsStr::new("presentations"), 0o755)
        .and_then(|d| fs_jail::ensure_child_dir_fd(&d, OsStr::new(deck_id), 0o755))
        .and_then(|d| fs_jail::ensure_child_dir_fd(&d, OsStr::new("slides"), 0o755))
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "E_FS_STAGE_FAILED",
                format!("Failed creating deck directory cleanly: {e}"),
                false,
                None,
            )
        })?;

    let c_tmp = CString::new(tmp_name.as_bytes()).map_err(|_| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            "Invalid temp filename",
            false,
            None,
        )
    })?;
    let c_final = CString::new(final_name.as_bytes()).map_err(|_| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            "Invalid final filename",
            false,
            None,
        )
    })?;

    let raw_fd = unsafe {
        libc::openat(
            cur_fd.as_raw_fd(),
            c_tmp.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o644 as libc::c_uint,
        )
    };
    if raw_fd < 0 {
        return Err(err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!(
                "Failed creating exclusive staging temp file: {}",
                std::io::Error::last_os_error()
            ),
            true,
            None,
        ));
    }
    let mut file = unsafe { File::from_raw_fd(raw_fd) };
    if let Err(err) = file
        .write_all(html.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        let _ = unsafe { libc::unlinkat(cur_fd.as_raw_fd(), c_tmp.as_ptr(), 0) };
        return Err(err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!("Failed writing/fsyncing staging slide HTML: {err}"),
            true,
            None,
        ));
    }
    drop(file);

    let rc = unsafe {
        libc::renameat(
            cur_fd.as_raw_fd(),
            c_tmp.as_ptr(),
            cur_fd.as_raw_fd(),
            c_final.as_ptr(),
        )
    };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        let _ = unsafe { libc::unlinkat(cur_fd.as_raw_fd(), c_tmp.as_ptr(), 0) };
        return Err(err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!("Failed atomic renameat to immutable slide artifact: {err}"),
            true,
            None,
        ));
    }
    if unsafe { libc::fsync(cur_fd.as_raw_fd()) } != 0 {
        let err = std::io::Error::last_os_error();
        let _ = unsafe { libc::unlinkat(cur_fd.as_raw_fd(), c_final.as_ptr(), 0) };
        return Err(err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_FS_STAGE_FAILED",
            format!("Failed directory fsync after atomic renameat: {err}"),
            true,
            None,
        ));
    }
    drop(cur_fd);

    Ok(rel_path)
}

pub(crate) fn cleanup_staged_files(ws_root: &Path, rel_paths: &[String]) {
    let Ok(ws_fd) = fs_jail::open_dir_abs_nofollow(ws_root) else {
        return;
    };
    for rel in rel_paths {
        let _ = fs_jail::remove_rel_file_in_dir_fd(&ws_fd, Path::new(rel));
    }
}

pub(crate) fn read_and_verify_slide_html(
    ws_root: &Path,
    rel_path: &str,
    expected_sha256: &str,
    slide_uid: &str,
) -> Result<String, Response> {
    let anchor = fs_jail::inspect_workspace_relative_jail(ws_root, rel_path).map_err(|e| {
        err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Slide artifact failed jail verification: {e}"),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        )
    })?;

    if !anchor.target_exists {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Committed slide artifact missing on disk: {rel_path}"),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        ));
    }

    let (Some(parent), Some(file_name)) = (
        anchor.canonical_path.parent(),
        anchor.canonical_path.file_name(),
    ) else {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            "Invalid slide artifact directory or filename",
            false,
            None,
        ));
    };

    let parent_fd = fs_jail::open_dir_abs_nofollow(parent).map_err(|_| {
        err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Slide artifact parent directory missing: {rel_path}"),
            false,
            None,
        )
    })?;

    if !fs_jail::check_child_non_symlink_in_dir_fd(&parent_fd, file_name) {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Slide artifact missing or symlink-blocked: {rel_path}"),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        ));
    }

    let c_name = CString::new(file_name.to_string_lossy().as_bytes()).map_err(|_| {
        err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            "Invalid slide filename bytes",
            false,
            None,
        )
    })?;
    let raw_fd = unsafe {
        libc::openat(
            parent_fd.as_raw_fd(),
            c_name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if raw_fd < 0 {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Unable to open committed slide artifact: {rel_path}"),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        ));
    }
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(owned_fd.as_raw_fd(), &mut stat_buf) } != 0
        || (stat_buf.st_mode & libc::S_IFMT) != libc::S_IFREG
        || stat_buf.st_size < 0
        || (stat_buf.st_size as usize) > MAX_SLIDE_HTML_BYTES
    {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Committed slide artifact is not a bounded regular file: {rel_path}"),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        ));
    }
    let file = File::from(owned_fd);
    let mut contents = String::new();
    let max_read = (MAX_SLIDE_HTML_BYTES as u64) + 1;
    if file.take(max_read).read_to_string(&mut contents).is_err()
        || contents.len() > MAX_SLIDE_HTML_BYTES
    {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!("Unable to read bounded UTF-8 slide artifact: {rel_path}"),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        ));
    }

    let actual_sha = compute_sha256_hex(contents.as_bytes());
    if !constant_time_eq(actual_sha.as_bytes(), expected_sha256.as_bytes()) {
        return Err(err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_ARTIFACT_MISSING",
            format!(
                "Slide artifact integrity checksum mismatch on disk for {slide_uid} (expected {expected_sha256}, actual {actual_sha})"
            ),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid.to_string()),
                duplicate_ff_id: None,
            }),
        ));
    }

    Ok(contents)
}

pub(crate) enum IdemState {
    None,
    Proceed(String),
    Replay(u16, String),
}

pub(crate) fn build_scoped_idempotency_key(
    route_tag: &str,
    project_id: &str,
    deck_id: &str,
    slide_uid: &str,
    raw_key: &str,
) -> String {
    let candidate = format!("{route_tag}:{project_id}:{deck_id}:{slide_uid}:{raw_key}");
    if candidate.len() <= 128 {
        return candidate;
    }
    let mut hasher = Sha256::new();
    hasher.update(candidate.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("{route_tag}:{}", &digest[..48])
}

type ExistingIdempotencyRow = (String, String, i64, Option<i64>, Option<String>, i64);

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_and_reserve_scoped_idempotency(
    state: &DaemonAppState,
    tenant_id: &str,
    actor_id: &str,
    headers: &HeaderMap,
    route_tag: &str,
    project_id: &str,
    deck_id: &str,
    slide_uid: &str,
    payload_bytes: &[u8],
) -> Result<IdemState, Response> {
    let Some(raw_key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) else {
        return Ok(IdemState::None);
    };
    let trimmed = raw_key.trim();
    if trimmed.is_empty() {
        return Ok(IdemState::None);
    }
    if trimmed.len() > 128 {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_IDEMPOTENCY_KEY",
            "Idempotency-Key exceeds 128 characters",
            false,
            None,
        ));
    }
    let scoped = build_scoped_idempotency_key(route_tag, project_id, deck_id, slide_uid, trimmed);
    check_and_reserve_idempotency_with_resolved_key(
        state,
        tenant_id,
        actor_id,
        &scoped,
        payload_bytes,
    )
}

pub(crate) fn check_and_reserve_idempotency(
    state: &DaemonAppState,
    tenant_id: &str,
    actor_id: &str,
    headers: &HeaderMap,
    payload_bytes: &[u8],
) -> Result<IdemState, Response> {
    let Some(raw_key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) else {
        return Ok(IdemState::None);
    };
    let key = raw_key.trim();
    if key.is_empty() {
        return Ok(IdemState::None);
    }
    if key.len() > 128 {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_IDEMPOTENCY_KEY",
            "Idempotency-Key exceeds 128 characters",
            false,
            None,
        ));
    }
    check_and_reserve_idempotency_with_resolved_key(state, tenant_id, actor_id, key, payload_bytes)
}

fn check_and_reserve_idempotency_with_resolved_key(
    state: &DaemonAppState,
    tenant_id: &str,
    actor_id: &str,
    key: &str,
    payload_bytes: &[u8],
) -> Result<IdemState, Response> {
    let mut hasher = Sha256::new();
    hasher.update(payload_bytes);
    let req_sha = format!("{:x}", hasher.finalize());
    let now_ms = Utc::now().timestamp_millis();

    let mut conn = state.registry.pool().connection().map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("DB unavailable: {e}"),
            true,
            None,
        )
    })?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Idempotency TX failed: {e}"),
                true,
                None,
            )
        })?;

    let existing: Option<ExistingIdempotencyRow> = tx
        .query_row(
            "SELECT request_sha256, status, lease_expires_at, response_status, response_body_json, expires_at
             FROM idempotency_records
             WHERE tenant_id = ?1 AND actor_id = ?2 AND idempotency_key = ?3",
            params![tenant_id, actor_id, key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Idempotency lookup failed: {e}"),
                false,
                None,
            )
        })?;

    if let Some((saved_sha, status, lease_exp, resp_status, resp_body, expires_at)) = existing {
        if now_ms >= expires_at {
            tx.execute(
                "UPDATE idempotency_records
                 SET request_sha256 = ?1, status = 'IN_PROGRESS', lease_expires_at = ?2, response_status = NULL, response_body_json = NULL, expires_at = ?3
                 WHERE tenant_id = ?4 AND actor_id = ?5 AND idempotency_key = ?6",
                params![req_sha, now_ms + 30_000, now_ms + 86_400_000, tenant_id, actor_id, key],
            )
            .map_err(|e| {
                err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    format!("Idempotency expired-renew failed: {e}"),
                    true,
                    None,
                )
            })?;
        } else if saved_sha != req_sha {
            let _ = tx.rollback();
            return Err(err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "E_IDEMPOTENCY_PAYLOAD_MISMATCH",
                "Idempotency-Key reused with different request payload",
                false,
                None,
            ));
        } else if status == "COMPLETED" {
            let _ = tx.rollback();
            return Ok(IdemState::Replay(
                resp_status.unwrap_or(201) as u16,
                resp_body.unwrap_or_default(),
            ));
        } else if status == "IN_PROGRESS" && now_ms < lease_exp {
            let _ = tx.rollback();
            return Err(err_resp(
                StatusCode::CONFLICT,
                "E_IDEMPOTENCY_IN_PROGRESS",
                "Duplicate request with matching Idempotency-Key is currently in progress",
                true,
                None,
            ));
        } else {
            tx.execute(
                "UPDATE idempotency_records
                 SET status = 'IN_PROGRESS', lease_expires_at = ?1, response_status = NULL, response_body_json = NULL, expires_at = ?2
                 WHERE tenant_id = ?3 AND actor_id = ?4 AND idempotency_key = ?5",
                params![now_ms + 30_000, now_ms + 86_400_000, tenant_id, actor_id, key],
            )
            .map_err(|e| {
                err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    format!("Idempotency renew failed: {e}"),
                    true,
                    None,
                )
            })?;
        }
    } else {
        tx.execute(
            "INSERT INTO idempotency_records
             (tenant_id, actor_id, idempotency_key, request_sha256, status, lease_expires_at, response_status, response_body_json, expires_at)
             VALUES (?1, ?2, ?3, ?4, 'IN_PROGRESS', ?5, NULL, NULL, ?6)",
            params![
                tenant_id,
                actor_id,
                key,
                req_sha,
                now_ms + 30_000,
                now_ms + 86_400_000
            ],
        )
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Idempotency reserve failed: {e}"),
                true,
                None,
            )
        })?;
    }
    tx.commit().map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Idempotency commit failed: {e}"),
            true,
            None,
        )
    })?;
    Ok(IdemState::Proceed(key.to_string()))
}

pub(crate) fn finalize_idempotency(
    state: &DaemonAppState,
    tenant_id: &str,
    actor_id: &str,
    key: &str,
    status_code: u16,
    body_json: &str,
) {
    match state.registry.pool().connection() {
        Ok(conn) => {
            if let Err(err) = conn.execute(
                "UPDATE idempotency_records
                 SET status = 'COMPLETED', response_status = ?1, response_body_json = ?2
                 WHERE tenant_id = ?3 AND actor_id = ?4 AND idempotency_key = ?5",
                params![status_code as i64, body_json, tenant_id, actor_id, key],
            ) {
                eprintln!(
                    "[presentation-storage] WARN finalize_idempotency failed for key={key}: {err}"
                );
            }
        }
        Err(err) => {
            eprintln!(
                "[presentation-storage] WARN finalize_idempotency pool error for key={key}: {err}"
            );
        }
    }
}

pub(crate) fn release_idempotency(
    state: &DaemonAppState,
    tenant_id: &str,
    actor_id: &str,
    key: &str,
) {
    match state.registry.pool().connection() {
        Ok(conn) => {
            if let Err(err) = conn.execute(
                "DELETE FROM idempotency_records
                 WHERE tenant_id = ?1 AND actor_id = ?2 AND idempotency_key = ?3 AND status = 'IN_PROGRESS'",
                params![tenant_id, actor_id, key],
            ) {
                eprintln!("[presentation-storage] WARN release_idempotency failed for key={key}: {err}");
            }
        }
        Err(err) => {
            eprintln!("[presentation-storage] WARN release_idempotency pool error for key={key}: {err}");
        }
    }
}

pub(crate) fn resolve_constraint_tokens_css(
    conn: &rusqlite::Connection,
    project_id: &str,
    workspace_root: &Path,
) -> String {
    let rel_path: Option<String> = conn
        .query_row(
            "SELECT tokens_css_path FROM constraint_profiles WHERE project_id = ?1 ORDER BY updated_at DESC LIMIT 1",
            params![project_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    let Some(rel_path) = rel_path else {
        return String::new();
    };
    let trimmed = rel_path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(anchor) = fs_jail::inspect_workspace_relative_jail(workspace_root, trimmed) else {
        return String::new();
    };
    if !anchor.target_exists {
        return String::new();
    }
    let (Some(parent), Some(file_name)) = (
        anchor.canonical_path.parent(),
        anchor.canonical_path.file_name(),
    ) else {
        return String::new();
    };
    let Ok(parent_fd) = fs_jail::open_dir_abs_nofollow(parent) else {
        return String::new();
    };
    if !fs_jail::check_child_non_symlink_in_dir_fd(&parent_fd, file_name) {
        return String::new();
    }
    let Ok(c_leaf) = CString::new(file_name.to_string_lossy().as_bytes()) else {
        return String::new();
    };
    let raw_fd = unsafe {
        libc::openat(
            parent_fd.as_raw_fd(),
            c_leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if raw_fd < 0 {
        return String::new();
    }
    let mut file = unsafe { File::from_raw_fd(raw_fd) };
    let mut buf = Vec::new();
    if std::io::Read::by_ref(&mut file)
        .take(65_536)
        .read_to_end(&mut buf)
        .is_err()
    {
        return String::new();
    }
    String::from_utf8(buf).unwrap_or_default()
}

/// Sweeps orphaned `.cowok-tmp-*` / `.tmp*` slide staging artifacts older than 60s on startup without following symlinks.
pub fn sweep_orphan_tmp_slides(workspace_root: &Path) -> usize {
    sweep_orphan_slide_artifacts(workspace_root, &HashSet::new(), ORPHAN_TMP_AGE_SECS, false)
}

/// Deterministic crash-recovery sweeper: removes `.cowok-tmp-*`/`.tmp*` files AND unreferenced
/// immutable `*.r<rev>.<uuid>.html` artifacts that are NOT in `active_html_paths` and older than `min_age_secs`.
pub fn sweep_orphan_slide_artifacts(
    workspace_root: &Path,
    active_html_paths: &HashSet<String>,
    min_age_secs: u64,
    include_unreferenced_revisions: bool,
) -> usize {
    let base = workspace_root.join("presentations");
    let Ok(base_meta) = std::fs::symlink_metadata(&base) else {
        return 0;
    };
    if base_meta.file_type().is_symlink() || !base_meta.is_dir() {
        return 0;
    }
    let Ok(_base_fd) = fs_jail::open_dir_abs_nofollow(&base) else {
        return 0;
    };
    let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) else {
        return 0;
    };
    let cutoff = now.saturating_sub(Duration::from_secs(min_age_secs));
    let mut swept = 0;

    let Ok(decks) = std::fs::read_dir(&base) else {
        return 0;
    };
    for deck_entry in decks.flatten() {
        let deck_name = deck_entry.file_name().to_string_lossy().to_string();
        let deck_path = deck_entry.path();
        let Ok(deck_meta) = std::fs::symlink_metadata(&deck_path) else {
            continue;
        };
        if deck_meta.file_type().is_symlink() || !deck_meta.is_dir() {
            continue;
        }
        let slides_dir = deck_path.join("slides");
        let Ok(slides_meta) = std::fs::symlink_metadata(&slides_dir) else {
            continue;
        };
        if slides_meta.file_type().is_symlink() || !slides_meta.is_dir() {
            continue;
        }
        let Ok(slides_fd) = fs_jail::open_dir_abs_nofollow(&slides_dir) else {
            continue;
        };
        let Ok(files) = std::fs::read_dir(&slides_dir) else {
            continue;
        };
        for file_entry in files.flatten() {
            let name = file_entry.file_name();
            let name_str = name.to_string_lossy();
            let has_traversal = Path::new(&*name_str)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)));
            if has_traversal {
                continue;
            }
            let rel_path = format!("presentations/{deck_name}/slides/{name_str}");
            if active_html_paths.contains(&rel_path) {
                continue;
            }
            let is_orphan_tmp = name_str.starts_with(".cowok-tmp-") || name_str.contains(".tmp");
            let is_unreferenced_rev = include_unreferenced_revisions
                && name_str.ends_with(".html")
                && name_str.contains(".r");
            if !is_orphan_tmp && !is_unreferenced_rev {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(file_entry.path()) else {
                continue;
            };
            if meta.file_type().is_symlink() || !meta.is_file() {
                continue;
            }
            if let Ok(mtime) = meta.modified() {
                let age_epoch = mtime
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default();
                if min_age_secs == 0 || age_epoch <= cutoff {
                    if let Ok(c_name) = CString::new(&*name_str) {
                        let rc =
                            unsafe { libc::unlinkat(slides_fd.as_raw_fd(), c_name.as_ptr(), 0) };
                        if rc == 0 {
                            swept += 1;
                        }
                    }
                }
            }
        }
    }
    swept
}
