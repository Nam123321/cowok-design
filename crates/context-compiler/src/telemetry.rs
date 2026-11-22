// @story 02.1
// @implements AC-02, AC-04
use crate::types::{
    AdapterMode, ContextAssembleDataDto, ContextAssemblyCommand, ContextCompilerError,
    ContextReasonCode, ContextTelemetryItemDto, ContextTelemetryPageDataDto,
    ContextTelemetryPageMetaDto, ContextTelemetryQueryDto, ContextTelemetryTokenUsageDto, Horizon,
    ModelProfile, PortalId,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::SqlitePool;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::sync::{Mutex, OnceLock};
use std::thread::sleep;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_TELEMETRY_LIMIT: usize = 25;
const MAX_TELEMETRY_LIMIT: usize = 100;
const MAX_SQLITE_BUSY_RETRIES: usize = 5;
const BUSY_BACKOFF_BASE_MS: u64 = 15;
const MAX_CURSOR_LEN: usize = 512;

static CURSOR_HMAC_SECRET: OnceLock<[u8; 32]> = OnceLock::new();
static WORKSPACE_CURSOR_SECRET_CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();

fn process_cursor_secret() -> &'static [u8; 32] {
    CURSOR_HMAC_SECRET.get_or_init(|| {
        let mut h = Sha256::new();
        h.update(Uuid::new_v4().as_bytes());
        h.update(Uuid::new_v4().as_bytes());
        h.update(std::process::id().to_le_bytes());
        let digest = h.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest[..32]);
        key
    })
}

fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    let mut k_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = Sha256::digest(key);
        k_block[..32].copy_from_slice(&hashed);
    } else {
        k_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k_block[i];
        opad[i] ^= k_block[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    format!("{:x}", outer.finalize())
}

fn constant_time_str_eq(a: &str, b: &str) -> bool {
    let ba = a.as_bytes();
    let bb = b.as_bytes();
    if ba.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in ba.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorPayload {
    created_at: i64,
    id: String,
    project_id: String,
    tenant_id: String,
    filters_hash: String,
    sig: String,
}

fn is_busy_error(err: &rusqlite::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("locked") || msg.contains("busy")
}

fn map_storage_err(err: rusqlite::Error) -> ContextCompilerError {
    if is_busy_error(&err) {
        ContextCompilerError::TelemetryWriteBusy
    } else {
        ContextCompilerError::TelemetryStorageUnavailable {
            reason: err.to_string(),
        }
    }
}

fn compute_filters_hash(query: &ContextTelemetryQueryDto, allow_superadmin: bool) -> String {
    let portal = query.portal_id.map(|p| p.as_str()).unwrap_or("*");
    let horizon = query.horizon.map(|h| h.as_str()).unwrap_or("*");
    let profile = query.model_profile.map(|m| m.as_str()).unwrap_or("*");
    let raw = format!(
        "{}:{portal}:{horizon}:{profile}:{allow_superadmin}",
        query.project_id
    );
    format!("{:x}", Sha256::digest(raw.as_bytes()))
}

fn read_verified_hex_key_nofollow(key_path: &std::path::Path) -> Option<[u8; 32]> {
    let meta = fs::symlink_metadata(key_path).ok()?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return None;
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NOFOLLOW);
    let mut file = opts.open(key_path).ok()?;
    let mut raw = String::new();
    file.read_to_string(&mut raw).ok()?;
    let trimmed = raw.trim();
    if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        let digest = Sha256::digest(trimmed.as_bytes());
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest[..32]);
        return Some(key);
    }
    None
}

fn load_or_create_workspace_cursor_secret(db_root: &str) -> [u8; 32] {
    if let Ok(env_secret) = std::env::var("COWOK_CURSOR_HMAC_SECRET") {
        let trimmed = env_secret.trim();
        if !trimmed.is_empty() {
            let digest = Sha256::digest(trimmed.as_bytes());
            let mut key = [0u8; 32];
            key.copy_from_slice(&digest[..32]);
            return key;
        }
    }
    let cache = WORKSPACE_CURSOR_SECRET_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get(db_root) {
            return *cached;
        }
    }
    let root_path = std::path::Path::new(db_root);
    if !db_root.trim().is_empty() && root_path.is_dir() {
        let state_dir = root_path.join(".cowok-design");
        let state_is_symlink = fs::symlink_metadata(&state_dir)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if !state_is_symlink {
            let key_path = state_dir.join(".cursor-hmac.key");
            if let Some(key) = read_verified_hex_key_nofollow(&key_path) {
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(db_root.to_string(), key);
                }
                return key;
            }
            if fs::create_dir_all(&state_dir).is_ok() {
                let fresh_hex = format!(
                    "{:x}",
                    Sha256::digest(format!("{}{}", Uuid::new_v4(), Uuid::new_v4()).as_bytes())
                );
                let tmp_path = state_dir.join(format!(".cursor-hmac.key.tmp.{}", Uuid::new_v4()));
                let mut opts = OpenOptions::new();
                opts.write(true).create_new(true);
                #[cfg(unix)]
                opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
                if let Ok(mut f) = opts.open(&tmp_path) {
                    if f.write_all(fresh_hex.as_bytes()).is_ok() && f.sync_all().is_ok() {
                        drop(f);
                        let _ = fs::hard_link(&tmp_path, &key_path);
                    }
                    let _ = fs::remove_file(&tmp_path);
                }
                if let Some(key) = read_verified_hex_key_nofollow(&key_path) {
                    if let Ok(mut guard) = cache.lock() {
                        guard.insert(db_root.to_string(), key);
                    }
                    return key;
                }
            }
        }
    }
    *process_cursor_secret()
}

fn sign_cursor(
    db_root: &str,
    created_at: i64,
    id: &str,
    project_id: &str,
    tenant_id: &str,
    filters_hash: &str,
) -> String {
    let secret = load_or_create_workspace_cursor_secret(db_root);
    let msg = format!("{db_root}|{created_at}|{id}|{project_id}|{tenant_id}|{filters_hash}");
    hmac_sha256_hex(&secret, msg.as_bytes())
}

fn decode_and_verify_cursor(
    db_root: &str,
    tenant_id: &str,
    query: &ContextTelemetryQueryDto,
    filters_hash: &str,
) -> Result<Option<(i64, String)>, ContextCompilerError> {
    let raw_cursor = match &query.cursor {
        Some(c) if !c.trim().is_empty() => c.trim(),
        _ => return Ok(None),
    };
    if raw_cursor.len() > MAX_CURSOR_LEN {
        return Err(ContextCompilerError::InvalidCursor);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(raw_cursor.as_bytes())
        .map_err(|_| ContextCompilerError::InvalidCursor)?;
    let parsed: CursorPayload =
        serde_json::from_slice(&decoded).map_err(|_| ContextCompilerError::InvalidCursor)?;
    let expected_sig = sign_cursor(
        db_root,
        parsed.created_at,
        &parsed.id,
        &parsed.project_id,
        &parsed.tenant_id,
        &parsed.filters_hash,
    );
    if !constant_time_str_eq(&parsed.sig, &expected_sig)
        || parsed.tenant_id != tenant_id
        || parsed.project_id != query.project_id.to_string()
        || parsed.filters_hash != filters_hash
    {
        return Err(ContextCompilerError::InvalidCursor);
    }
    Ok(Some((parsed.created_at, parsed.id)))
}

fn encode_cursor(
    db_root: &str,
    created_at: i64,
    id: &str,
    project_id: &str,
    tenant_id: &str,
    filters_hash: &str,
) -> String {
    let payload = CursorPayload {
        created_at,
        id: id.to_string(),
        project_id: project_id.to_string(),
        tenant_id: tenant_id.to_string(),
        filters_hash: filters_hash.to_string(),
        sig: sign_cursor(db_root, created_at, id, project_id, tenant_id, filters_hash),
    };
    let json_bytes = serde_json::to_vec(&payload).expect("CursorPayload serialization is infallible");
    URL_SAFE_NO_PAD.encode(json_bytes)
}

pub fn compute_request_fingerprint_sha256(cmd: &ContextAssemblyCommand) -> String {
    let nfc_intent = crate::portal_guard::normalize_intent_nfc(&cmd.intent)
        .unwrap_or_else(|_| cmd.intent.trim().to_string());
    let mut ids: Vec<String> = cmd
        .source_ids
        .as_ref()
        .map(|v| v.iter().map(|u| u.to_string()).collect())
        .unwrap_or_default();
    ids.sort();
    let mut constraints = cmd.extra_constraints.clone();
    constraints.sort();
    let mut outline_sigs: Vec<(&str, &str, &str, &str)> = cmd
        .dom_outline_nodes
        .iter()
        .map(|n| {
            (
                n.ff_id.as_str(),
                n.tag.as_str(),
                n.role.as_deref().unwrap_or(""),
                n.summary.as_deref().unwrap_or(""),
            )
        })
        .collect();
    outline_sigs.sort();
    let mut blocks: Vec<(u8, &str, &str, &str)> = cmd
        .optional_blocks
        .iter()
        .map(|b| (b.priority, b.format.as_str(), b.id.as_str(), b.content.as_str()))
        .collect();
    blocks.sort();
    let encoded = serde_json::to_vec(&(
        cmd.project_id.to_string(),
        cmd.portal_id.as_str(),
        cmd.horizon.as_str(),
        cmd.model_profile.as_str(),
        nfc_intent,
        cmd.requested_image_tokens,
        cmd.max_image_tokens_per_turn,
        cmd.inspect_only,
        cmd.brand_tokens_prefix.as_str(),
        cmd.active_skill_prefix.as_str(),
        constraints,
        ids,
        outline_sigs,
        blocks,
    ))
    .expect("request fingerprint serialization is infallible");
    format!("{:x}", Sha256::digest(&encoded))
}

fn verify_project_tenant_in_conn(
    conn: &Connection,
    project_id: &str,
    tenant_id: &str,
) -> Result<(), ContextCompilerError> {
    let owner: Option<String> = conn
        .query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![project_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_storage_err)?;
    match owner {
        None => Err(ContextCompilerError::TelemetryProjectReferenceInvalid),
        Some(t) if t != tenant_id => Err(ContextCompilerError::ProjectTenantMismatch),
        Some(_) => Ok(()),
    }
}

pub fn verify_project_tenant_for_pool(
    pool: &SqlitePool,
    project_id: Uuid,
    tenant_id: &str,
) -> Result<(), ContextCompilerError> {
    let conn =
        pool.connection()
            .map_err(|e| ContextCompilerError::TelemetryStorageUnavailable {
                reason: e.to_string(),
            })?;
    verify_project_tenant_in_conn(&conn, &project_id.to_string(), tenant_id)
}

fn check_idempotency_replay_tx(
    conn: &Connection,
    cmd: &ContextAssemblyCommand,
    fingerprint: &str,
) -> Result<Option<ContextAssembleDataDto>, ContextCompilerError> {
    let Some(idem_key) = cmd.idempotency_key.as_deref() else {
        return Ok(None);
    };
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT request_fingerprint_sha256, response_json
             FROM context_assembly_telemetry
             WHERE tenant_id = ?1 AND project_id = ?2 AND idempotency_key = ?3",
            params![cmd.tenant_id, cmd.project_id.to_string(), idem_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(map_storage_err)?;
    let Some((saved_fp, saved_json)) = existing else {
        return Ok(None);
    };
    if saved_fp != fingerprint {
        return Err(ContextCompilerError::IdempotencyKeyReused);
    }
    let replayed: ContextAssembleDataDto = serde_json::from_str(&saved_json).map_err(|err| {
        ContextCompilerError::TelemetryStorageUnavailable {
            reason: err.to_string(),
        }
    })?;
    Ok(Some(replayed))
}

fn write_telemetry_row_tx(
    conn: &Connection,
    cmd: &ContextAssemblyCommand,
    dto: &ContextAssembleDataDto,
    fingerprint: &str,
) -> Result<(), ContextCompilerError> {
    let response_json = serde_json::to_string(dto).map_err(|err| {
        ContextCompilerError::TelemetryStorageUnavailable {
            reason: err.to_string(),
        }
    })?;
    let now_ms = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO context_assembly_telemetry (
            id, tenant_id, project_id, idempotency_key, request_fingerprint_sha256,
            portal_id, horizon, model_profile, adapter_mode, prefix_sha256,
            static_prefix_tokens, dynamic_tail_tokens, outline_tokens, total_text_tokens,
            image_tokens, ssot_dom_tokens, non_ssot_tokens, ssot_dom_ratio,
            sources_requested, sources_included, sources_pruned,
            reason_code, response_json, latency_ms, created_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25)",
        params![
            dto.telemetry_id, cmd.tenant_id, dto.project_id, cmd.idempotency_key, fingerprint,
            dto.portal_id.as_str(), dto.horizon.as_str(), dto.model_profile.as_str(),
            dto.adapter_mode.as_str(), dto.static_prefix_sha256,
            dto.token_usage.static_prefix as i64, dto.token_usage.dynamic_tail as i64,
            dto.token_usage.outline as i64, dto.token_usage.text_total as i64,
            dto.token_usage.image as i64, dto.token_usage.ssot_dom as i64,
            dto.token_usage.non_ssot_text as i64, dto.token_usage.ssot_dom_ratio,
            dto.sources.requested as i64, dto.sources.included as i64,
            dto.sources.pruned as i64, dto.reason_code.as_str(),
            response_json, dto.latency_ms as i64, now_ms,
        ],
    )
    .map_err(|err| {
        if is_busy_error(&err) {
            ContextCompilerError::TelemetryWriteBusy
        } else if matches!(
            err,
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    extended_code: rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                        | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY,
                    ..
                },
                _
            )
        ) {
            ContextCompilerError::TelemetryConflict
        } else {
            ContextCompilerError::TelemetryStorageUnavailable {
                reason: err.to_string(),
            }
        }
    })?;
    Ok(())
}

pub fn lookup_idempotent_replay(
    pool: &SqlitePool,
    cmd: &ContextAssemblyCommand,
) -> Result<Option<ContextAssembleDataDto>, ContextCompilerError> {
    if cmd.idempotency_key.is_none() {
        return Ok(None);
    }
    let fingerprint = compute_request_fingerprint_sha256(cmd);
    let conn =
        pool.connection()
            .map_err(|e| ContextCompilerError::TelemetryStorageUnavailable {
                reason: e.to_string(),
            })?;
    verify_project_tenant_in_conn(&conn, &cmd.project_id.to_string(), &cmd.tenant_id)?;
    check_idempotency_replay_tx(&conn, cmd, &fingerprint)
}

fn insert_telemetry_tx(
    conn: &mut Connection,
    cmd: &ContextAssemblyCommand,
    dto: &ContextAssembleDataDto,
    fingerprint: &str,
) -> Result<ContextAssembleDataDto, ContextCompilerError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_storage_err)?;
    verify_project_tenant_in_conn(&tx, &cmd.project_id.to_string(), &cmd.tenant_id)?;
    if let Some(replayed) = check_idempotency_replay_tx(&tx, cmd, fingerprint)? {
        return Ok(replayed);
    }
    if cmd.inspect_only {
        return Ok(dto.clone());
    }
    write_telemetry_row_tx(&tx, cmd, dto, fingerprint)?;
    tx.commit().map_err(map_storage_err)?;
    Ok(dto.clone())
}

pub fn record_idempotent_or_replay(
    pool: &SqlitePool,
    cmd: &ContextAssemblyCommand,
    dto: &ContextAssembleDataDto,
) -> Result<ContextAssembleDataDto, ContextCompilerError> {
    let fingerprint = compute_request_fingerprint_sha256(cmd);
    let mut attempt = 0usize;
    loop {
        let mut conn =
            pool.connection()
                .map_err(|e| ContextCompilerError::TelemetryStorageUnavailable {
                    reason: e.to_string(),
                })?;
        match insert_telemetry_tx(&mut conn, cmd, dto, &fingerprint) {
            Ok(res) => return Ok(res),
            Err(ContextCompilerError::TelemetryWriteBusy) if attempt < MAX_SQLITE_BUSY_RETRIES => {
                attempt += 1;
                sleep(Duration::from_millis(
                    BUSY_BACKOFF_BASE_MS * (attempt as u64),
                ));
            }
            Err(err) => return Err(err),
        }
    }
}

fn parse_enum<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, ContextCompilerError> {
    serde_json::from_str(&format!("\"{raw}\"")).map_err(|e| {
        ContextCompilerError::TelemetryStorageUnavailable {
            reason: e.to_string(),
        }
    })
}

fn ms_to_rfc3339(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

type RawTelemetryRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    f64,
    i64,
    i64,
);
type TelemetryRowBatch = (Vec<ContextTelemetryItemDto>, Vec<(i64, String)>);

fn map_telemetry_row(
    r: RawTelemetryRow,
) -> Result<(ContextTelemetryItemDto, (i64, String)), ContextCompilerError> {
    let ts = (r.13, r.0.clone());
    let item = ContextTelemetryItemDto {
        telemetry_id: r.0,
        project_id: r.1,
        portal_id: parse_enum::<PortalId>(&r.2)?,
        horizon: parse_enum::<Horizon>(&r.3)?,
        model_profile: parse_enum::<ModelProfile>(&r.4)?,
        adapter_mode: parse_enum::<AdapterMode>(&r.5)?,
        reason_code: parse_enum::<ContextReasonCode>(&r.6)?,
        static_prefix_sha256: r.7,
        token_usage: ContextTelemetryTokenUsageDto {
            outline: r.8.max(0) as usize,
            text_total: r.9.max(0) as usize,
            image: r.10.max(0) as usize,
            ssot_dom_ratio: r.11,
        },
        latency_ms: r.12.max(0) as u64,
        status: "success".to_string(),
        created_at: ms_to_rfc3339(r.13),
    };
    Ok((item, ts))
}

fn scan_telemetry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RawTelemetryRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
        r.get(12)?,
        r.get(13)?,
    ))
}

fn query_telemetry_rows(
    conn: &Connection,
    tenant_id: &str,
    query: &ContextTelemetryQueryDto,
    c_ts: i64,
    c_id: &str,
    limit: usize,
    allow_superadmin: bool,
) -> Result<TelemetryRowBatch, ContextCompilerError> {
    let portal_filter = query.portal_id.map(|p| p.as_str().to_string());
    let horizon_filter = query.horizon.map(|h| h.as_str().to_string());
    let profile_filter = query.model_profile.map(|m| m.as_str().to_string());
    let allow_sa_flag: i64 = i64::from(allow_superadmin);
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, portal_id, horizon, model_profile, adapter_mode,
                    reason_code, prefix_sha256, outline_tokens, total_text_tokens,
                    image_tokens, ssot_dom_ratio, latency_ms, created_at
             FROM context_assembly_telemetry
             WHERE tenant_id = ?1 AND project_id = ?2
               AND (?3 IS NULL OR portal_id = ?3)
               AND (?4 IS NULL OR model_profile = ?4)
               AND (?9 IS NULL OR horizon = ?9)
               AND (?8 = 1 OR portal_id != 'SuperAdmin')
               AND (created_at < ?5 OR (created_at = ?5 AND id < ?6))
             ORDER BY created_at DESC, id DESC
             LIMIT ?7",
        )
        .map_err(map_storage_err)?;
    let rows = stmt
        .query_map(
            params![
                tenant_id,
                query.project_id.to_string(),
                portal_filter,
                profile_filter,
                c_ts,
                c_id,
                (limit + 1) as i64,
                allow_sa_flag,
                horizon_filter,
            ],
            scan_telemetry_row,
        )
        .map_err(map_storage_err)?;
    let mut items = Vec::new();
    let mut timestamps = Vec::new();
    for row in rows {
        let (item, ts) = map_telemetry_row(row.map_err(map_storage_err)?)?;
        timestamps.push(ts);
        items.push(item);
    }
    Ok((items, timestamps))
}

pub fn list_context_telemetry_scoped(
    pool: &SqlitePool,
    tenant_id: &str,
    query: &ContextTelemetryQueryDto,
    allow_superadmin: bool,
) -> Result<ContextTelemetryPageDataDto, ContextCompilerError> {
    if query.portal_id == Some(PortalId::SuperAdmin) && !allow_superadmin {
        return Err(ContextCompilerError::CrossPortalLeakBlocked {
            reason: "SuperAdmin telemetry access requires admin privileges".to_string(),
        });
    }
    let limit = query.limit.unwrap_or(DEFAULT_TELEMETRY_LIMIT);
    if limit == 0 || limit > MAX_TELEMETRY_LIMIT {
        return Err(ContextCompilerError::InvalidRequest {
            message: "limit must be between 1 and 100".to_string(),
            field: Some("limit".to_string()),
        });
    }
    let db_root = pool.workspace_root().display().to_string();
    let filters_hash = compute_filters_hash(query, allow_superadmin);
    let cursor_tuple = decode_and_verify_cursor(&db_root, tenant_id, query, &filters_hash)?;
    let conn =
        pool.connection()
            .map_err(|e| ContextCompilerError::TelemetryStorageUnavailable {
                reason: e.to_string(),
            })?;
    match verify_project_tenant_in_conn(&conn, &query.project_id.to_string(), tenant_id) {
        Ok(()) => {}
        Err(
            ContextCompilerError::TelemetryProjectReferenceInvalid
            | ContextCompilerError::ProjectTenantMismatch,
        ) => {
            return Err(ContextCompilerError::ProjectTenantMismatch);
        }
        Err(err) => return Err(err),
    }
    let (c_ts, c_id) = cursor_tuple.unwrap_or((i64::MAX, String::from("~")));
    let (mut items, timestamps) = query_telemetry_rows(
        &conn,
        tenant_id,
        query,
        c_ts,
        &c_id,
        limit,
        allow_superadmin,
    )?;
    let next_cursor = if items.len() > limit {
        items.truncate(limit);
        let (last_ts, ref last_id) = timestamps[limit - 1];
        Some(encode_cursor(
            &db_root,
            last_ts,
            last_id,
            &query.project_id.to_string(),
            tenant_id,
            &filters_hash,
        ))
    } else {
        None
    };
    Ok(ContextTelemetryPageDataDto {
        items,
        page: ContextTelemetryPageMetaDto {
            limit,
            next_cursor,
            order: "created_at_desc_id_desc".to_string(),
        },
    })
}

pub fn list_context_telemetry(
    pool: &SqlitePool,
    tenant_id: &str,
    query: &ContextTelemetryQueryDto,
) -> Result<ContextTelemetryPageDataDto, ContextCompilerError> {
    list_context_telemetry_scoped(pool, tenant_id, query, false)
}
