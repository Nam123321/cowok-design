// @story 04.6
// @implements AC-02, AC-05 (Magic-Byte Validation, ZIP-Bomb Guard, Atomic CAS Write & GC)
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use axum::{http::StatusCode, response::Response};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

use crate::routes::chat_cas_storage::{safe_api_error_response, CAS_MAX_REQUEST_BYTES};
use crate::routes::DaemonAppState;

const ZIP_MAX_ENTRIES: usize = 1024;
const ZIP_MAX_UNCOMPRESSED_BYTES: u64 = 67_108_864;
const ZIP_MAX_COMPRESSION_RATIO: f64 = 20.0;

#[derive(Debug, Clone)]
pub struct ValidatedCasUpload {
    pub file_ext: &'static str,
    pub mime_type: &'static str,
}

fn le_u16(b: &[u8]) -> Option<u16> {
    b.get(0..2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn le_u32(b: &[u8]) -> Option<u32> {
    b.get(0..4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub fn validate_zip_archive_limits(bytes: &[u8]) -> Result<(), Response> {
    let eocd_pos = bytes
        .windows(4)
        .rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])
        .ok_or_else(|| {
            safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "Invalid or truncated ZIP archive",
                false,
                None,
            )
        })?;
    if eocd_pos + 22 > bytes.len() {
        return Err(safe_api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CAS_ARCHIVE_REJECTED",
            "Truncated archive directory",
            false,
            None,
        ));
    }
    let eocd = &bytes[eocd_pos..];
    let total_entries = le_u16(&eocd[10..12]).unwrap_or(0) as usize;
    let cd_size = le_u32(&eocd[12..16]).unwrap_or(0) as usize;
    let cd_offset = le_u32(&eocd[16..20]).unwrap_or(0) as usize;
    if total_entries == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF {
        return Err(safe_api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CAS_ARCHIVE_REJECTED",
            "Archive exceeds safety limits",
            false,
            None,
        ));
    }
    if total_entries > ZIP_MAX_ENTRIES || cd_offset.saturating_add(cd_size) > eocd_pos {
        return Err(safe_api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CAS_ARCHIVE_REJECTED",
            "Archive entry count or layout exceeds limits",
            false,
            None,
        ));
    }

    let mut cursor = cd_offset;
    let cd_end = cd_offset + cd_size;
    let mut count = 0usize;
    let mut sum_uncomp = 0u64;
    let mut sum_comp = 0u64;

    while cursor + 46 <= cd_end {
        let hdr = &bytes[cursor..];
        if hdr[0..4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "Corrupt archive entry header",
                false,
                None,
            ));
        }
        let comp = le_u32(&hdr[20..24]).unwrap_or(0) as u64;
        let uncomp = le_u32(&hdr[24..28]).unwrap_or(0) as u64;
        let fname_len = le_u16(&hdr[28..30]).unwrap_or(0) as usize;
        let extra_len = le_u16(&hdr[30..32]).unwrap_or(0) as usize;
        let comment_len = le_u16(&hdr[32..34]).unwrap_or(0) as usize;
        let total_rec = 46 + fname_len + extra_len + comment_len;
        if cursor + total_rec > cd_end {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "Truncated archive entry metadata",
                false,
                None,
            ));
        }
        let name = std::str::from_utf8(&bytes[cursor + 46..cursor + 46 + fname_len]).unwrap_or("");
        if name.starts_with('/') || name.split(['/', '\\']).any(|s| s == "..") {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "Archive path traversal denied",
                false,
                None,
            ));
        }
        if comp == 0xFFFF_FFFF || uncomp == 0xFFFF_FFFF {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "ZIP64 archives exceed allowed limits",
                false,
                None,
            ));
        }
        count += 1;
        sum_comp = sum_comp.saturating_add(comp);
        sum_uncomp = sum_uncomp.saturating_add(uncomp);
        if count > ZIP_MAX_ENTRIES || sum_uncomp > ZIP_MAX_UNCOMPRESSED_BYTES {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "Archive uncompressed size or entry limit exceeded",
                false,
                None,
            ));
        }
        if uncomp > 0 && (comp == 0 || (uncomp as f64) / (comp as f64) > ZIP_MAX_COMPRESSION_RATIO) {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_ARCHIVE_REJECTED",
                "Archive compression ratio exceeds limit",
                false,
                None,
            ));
        }
        cursor += total_rec;
    }

    if count != total_entries {
        return Err(safe_api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CAS_ARCHIVE_REJECTED",
            "Archive directory mismatch",
            false,
            None,
        ));
    }
    if sum_uncomp > 0
        && (sum_comp == 0 || (sum_uncomp as f64) / (sum_comp as f64) > ZIP_MAX_COMPRESSION_RATIO)
    {
        return Err(safe_api_error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CAS_ARCHIVE_REJECTED",
            "Archive compression ratio exceeds limit",
            false,
            None,
        ));
    }
    Ok(())
}

pub fn validate_cas_magic_and_ext(filename: &str, bytes: &[u8]) -> Result<ValidatedCasUpload, Response> {
    if bytes.is_empty() {
        return Err(safe_api_error_response(
            StatusCode::BAD_REQUEST,
            "CAS_INVALID_FILE",
            "Uploaded file cannot be empty",
            false,
            None,
        ));
    }
    if bytes.len() > CAS_MAX_REQUEST_BYTES {
        return Err(safe_api_error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "CAS_QUOTA_EXCEEDED",
            "Uploaded file exceeds 25 MiB limit",
            false,
            Some("REQUEST_BYTES"),
        ));
    }
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_default();

    let (file_ext, mime_type) = match ext.as_str() {
        "png" if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) => {
            ("png", "image/png")
        }
        "pdf" if bytes.starts_with(b"%PDF-") => ("pdf", "application/pdf"),
        "xlsx" if bytes.starts_with(b"PK\x03\x04") => {
            validate_zip_archive_limits(bytes)?;
            (
                "xlsx",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            )
        }
        "fig" if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"fig-kiwi") => {
            if bytes.starts_with(b"PK\x03\x04") {
                validate_zip_archive_limits(bytes)?;
            }
            ("fig", "application/x-figma")
        }
        "svg" if std::str::from_utf8(bytes)
            .map(|s| s.contains("<svg"))
            .unwrap_or(false) =>
        {
            ("svg", "image/svg+xml")
        }
        "csv" if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok() => ("csv", "text/csv"),
        "tsv" if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok() => {
            ("tsv", "text/tab-separated-values")
        }
        "txt" if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok() => ("txt", "text/plain"),
        _ => {
            return Err(safe_api_error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CAS_UNSUPPORTED_MEDIA",
                "Unsupported file type or invalid magic signature",
                false,
                None,
            ));
        }
    };
    Ok(ValidatedCasUpload {
        file_ext,
        mime_type,
    })
}

pub fn open_secure_temp_file(dir: &Path, name: &str) -> Result<(PathBuf, File), Response> {
    fs::create_dir_all(dir).map_err(|_| {
        safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CAS_STORAGE_ERROR",
            "Failed to initialize attachment storage",
            false,
            None,
        )
    })?;
    let path = dir.join(name);
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = opts.open(&path).map_err(|_| {
        safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CAS_STORAGE_ERROR",
            "Failed to allocate attachment temp file",
            false,
            None,
        )
    })?;
    Ok((path, file))
}

pub fn neutralize_csv_tsv_formulas(bytes: &[u8], file_ext: &str) -> Vec<u8> {
    if !matches!(file_ext, "csv" | "tsv") {
        return bytes.to_vec();
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    let delim = if file_ext == "tsv" { '\t' } else { ',' };
    let is_trigger = |c: char| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r');
    let mut out = String::with_capacity(text.len().saturating_add(16));
    for (li, line) in text.split('\n').enumerate() {
        if li > 0 {
            out.push('\n');
        }
        for (ci, cell) in line.split(delim).enumerate() {
            if ci > 0 {
                out.push(delim);
            }
            if let Some(rest) = cell.strip_prefix('"') {
                if rest.starts_with(is_trigger) {
                    out.push('"');
                    out.push('\'');
                    out.push_str(rest);
                } else {
                    out.push_str(cell);
                }
            } else if cell.starts_with(is_trigger) {
                out.push('\'');
                out.push_str(cell);
            } else {
                out.push_str(cell);
            }
        }
    }
    out.into_bytes()
}

pub fn write_atomic_cas_blob(cas_root: &Path, sha256: &str, bytes: &[u8]) -> Result<(PathBuf, bool), Response> {
    let final_path = cas_root.join(sha256);
    if fs::symlink_metadata(&final_path).is_ok() {
        return Ok((final_path, false));
    }
    let (tmp_path, mut tmp_file) = open_secure_temp_file(cas_root, &format!(".tmp-{}", Uuid::new_v4()))?;
    if tmp_file.write_all(bytes).is_err() || tmp_file.sync_all().is_err() {
        let _ = fs::remove_file(&tmp_path);
        return Err(safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CAS_STORAGE_ERROR",
            "Failed to persist attachment content",
            false,
            None,
        ));
    }
    drop(tmp_file);
    if fs::rename(&tmp_path, &final_path).is_err() {
        let _ = fs::remove_file(&tmp_path);
        return Err(safe_api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "CAS_STORAGE_ERROR",
            "Failed to finalize attachment content",
            false,
            None,
        ));
    }
    if let Ok(dir) = fs::File::open(cas_root) {
        let _ = dir.sync_all();
    }
    Ok((final_path, true))
}

pub fn cleanup_uncommitted_final_blob(conn: &mut Connection, cas_root: &Path, sha256: &str) {
    let Ok(tx) = conn.transaction_with_behavior(TransactionBehavior::Immediate) else {
        return;
    };
    let still_referenced: Option<i64> = tx
        .query_row(
            "SELECT ref_count FROM cas_blobs WHERE sha256 = ?1",
            params![sha256],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    if still_referenced.is_none() {
        let _ = fs::remove_file(cas_root.join(sha256));
    }
}

pub async fn run_cas_gc_sweep(state: &DaemonAppState, now_ms: i64) -> usize {
    let Ok(conn) = state.registry.pool().connection() else {
        return 0;
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT sha256 FROM cas_blobs WHERE ref_count = 0 AND gc_lease_until < ?1",
    ) else {
        return 0;
    };
    let candidates: Vec<String> = stmt
        .query_map(params![now_ms], |r| r.get::<_, String>(0))
        .ok()
        .map(|it| it.filter_map(Result::ok).collect())
        .unwrap_or_default();

    let cas_root = state.registry.pool().workspace_root().join(".cowok-design/cas");
    let mut swept = 0usize;
    for sha in candidates {
        let lock = state.cas_sha_lock(&sha).await;
        let _guard = lock.lock().await;
        let Ok(mut c) = state.registry.pool().connection() else {
            continue;
        };
        let Ok(tx) = c.transaction_with_behavior(TransactionBehavior::Immediate) else {
            continue;
        };
        let deleted = tx
            .execute(
                "DELETE FROM cas_blobs WHERE sha256 = ?1 AND ref_count = 0 AND gc_lease_until < ?2",
                params![sha, now_ms],
            )
            .unwrap_or(0);
        if deleted > 0 && tx.commit().is_ok() {
            let _ = fs::remove_file(cas_root.join(&sha));
            swept += 1;
        }
    }
    swept
}
