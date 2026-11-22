// @story 02.4
// @implements AC-02, AC-04
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::errors::{is_sqlite_busy_or_locked, map_sqlite_error};

pub const MIN_LEASE_DURATION_MS: i64 = 5_000;
pub const MAX_LEASE_DURATION_MS: i64 = 3_600_000;
pub const DEFAULT_LEASE_DURATION_MS: i64 = 30_000;

pub const ALLOWED_SSOT_PORTALS: [&str; 6] = [
    "SuperAdmin",
    "TenantAdmin",
    "SalesApp",
    "MarketingWeb",
    "DeveloperPortal",
    "Presentation",
];

pub fn is_valid_ssot_portal(portal_id: &str) -> bool {
    ALLOWED_SSOT_PORTALS.contains(&portal_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppShellLockAction {
    Acquire,
    Renew,
    Release,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppShellLockConflictDetails {
    pub current_revision: i64,
    pub current_fencing_token: i64,
    pub expires_at: i64,
    pub is_expired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppShellLockDto {
    pub id: String,
    pub project_id: String,
    pub portal_id: String,
    pub lock_owner_id: String,
    pub fencing_token: i64,
    pub locked_slots: Vec<String>,
    pub expires_at: i64,
    pub is_expired: bool,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppShellLockMutationResult {
    pub lock: AppShellLockDto,
}

#[derive(Debug, Clone)]
pub struct AppShellLockCommand {
    pub tenant_id: String,
    pub project_id: String,
    pub portal_id: String,
    pub caller_actor_id: String,
    pub is_privileged_role: bool,
    pub action: AppShellLockAction,
    pub lock_owner_id: String,
    pub expected_revision: i64,
    pub expected_fencing_token: Option<i64>,
    pub locked_slots: Vec<String>,
    pub ttl_ms: i64,
    pub force_transfer_reason: Option<String>,
    pub now_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsotLockError {
    Validation { code: &'static str, message: String },
    Forbidden { code: &'static str, message: String },
    NotFound { code: &'static str, message: String },
    FencingConflict(AppShellLockConflictDetails),
    StorageBusy { details: String, retry_after_secs: u64 },
    Internal { code: &'static str, message: String },
}

impl std::fmt::Display for SsotLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsotLockError::Validation { code, message }
            | SsotLockError::Forbidden { code, message }
            | SsotLockError::NotFound { code, message }
            | SsotLockError::Internal { code, message } => write!(f, "{code}: {message}"),
            SsotLockError::FencingConflict(c) => write!(
                f,
                "LOCK_FENCING_CONFLICT: currentRevision={}, currentFencingToken={}",
                c.current_revision, c.current_fencing_token
            ),
            SsotLockError::StorageBusy { details, .. } => write!(f, "STORAGE_BUSY: {details}"),
        }
    }
}

impl std::error::Error for SsotLockError {}

pub fn current_epoch_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 1,
    }
}

/// Computes deterministic per-(tenant, project, portal, attempt) SHA-256 jitter backoff in ms.
/// Attempt 0 -> `15 + (seed % 5)`
/// Attempt 1 -> `45 + (seed % 15)`
/// Attempt 2 -> `105 + (seed % 30)`
pub fn compute_deterministic_jitter_backoff_ms(
    tenant_id: &str,
    project_id: &str,
    portal_id: &str,
    attempt: u8,
) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(tenant_id.as_bytes());
    hasher.update([0x00]);
    hasher.update(project_id.as_bytes());
    hasher.update([0x00]);
    hasher.update(portal_id.as_bytes());
    hasher.update([0x00]);
    hasher.update([attempt]);
    let digest = hasher.finalize();
    let seed = u64::from(u32::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3],
    ]));

    match attempt {
        0 => 15 + (seed % 5),
        1 => 45 + (seed % 15),
        _ => 105 + (seed % 30),
    }
}

/// Retries ONLY `SQLITE_BUSY`/`SQLITE_LOCKED` before transaction lock acquisition (max 3 retries).
/// On exhaustion returns `SsotLockError::StorageBusy` (`503 STORAGE_BUSY`, `Retry-After: 1`).
pub fn retry_on_busy_with_jitter<T, F>(
    tenant_id: &str,
    project_id: &str,
    portal_id: &str,
    context: &str,
    mut op: F,
) -> Result<T, SsotLockError>
where
    F: FnMut() -> Result<T, rusqlite::Error>,
{
    let mut attempt: u8 = 0;
    loop {
        match op() {
            Ok(val) => return Ok(val),
            Err(err) if is_sqlite_busy_or_locked(&err) => {
                if attempt >= 3 {
                    return Err(SsotLockError::StorageBusy {
                        details: format!("{context}: SQLite busy after 3 jittered retries ({err})"),
                        retry_after_secs: 1,
                    });
                }
                let sleep_ms = compute_deterministic_jitter_backoff_ms(
                    tenant_id,
                    project_id,
                    portal_id,
                    attempt,
                );
                attempt = attempt.saturating_add(1);
                std::thread::sleep(Duration::from_millis(sleep_ms));
            }
            Err(err) => {
                let is_constraint = matches!(
                    err,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ffi::ErrorCode::ConstraintViolation
                                | rusqlite::ffi::ErrorCode::TypeMismatch,
                            ..
                        },
                        _
                    )
                );
                let mapped = map_sqlite_error(err, context);
                if is_constraint {
                    return Err(SsotLockError::Validation {
                        code: "INVALID_REQUEST",
                        message: mapped.to_string(),
                    });
                }
                return Err(SsotLockError::Internal {
                    code: "INTERNAL_ERROR",
                    message: mapped.to_string(),
                });
            }
        }
    }
}

fn map_lock_row(row: &rusqlite::Row<'_>, now_ms: i64) -> rusqlite::Result<AppShellLockDto> {
    let lock_owner_id: String = row.get(3)?;
    let slots_raw: String = row.get(5)?;
    let expires_at: i64 = row.get(6)?;
    let locked_slots: Vec<String> = serde_json::from_str(&slots_raw).unwrap_or_default();
    let is_expired = lock_owner_id.trim().is_empty() || expires_at <= now_ms;
    Ok(AppShellLockDto {
        id: row.get(0)?,
        project_id: row.get(1)?,
        portal_id: row.get(2)?,
        lock_owner_id,
        fencing_token: row.get(4)?,
        locked_slots,
        expires_at,
        is_expired,
        revision: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn select_lock_row(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    portal_id: &str,
    now_ms: i64,
) -> Result<Option<AppShellLockDto>, rusqlite::Error> {
    tx.query_row(
        "SELECT id, project_id, portal_id, lock_owner_id, fencing_token, locked_slots_json, \
                expires_at, revision, created_at, updated_at \
         FROM app_shell_locks \
         WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3",
        params![tenant_id, project_id, portal_id],
        |row| map_lock_row(row, now_ms),
    )
    .optional()
}

fn build_conflict(existing: &AppShellLockDto) -> AppShellLockConflictDetails {
    AppShellLockConflictDetails {
        current_revision: existing.revision,
        current_fencing_token: existing.fencing_token,
        expires_at: existing.expires_at,
        is_expired: existing.is_expired,
    }
}

fn verify_project_exists_in_tenant(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
) -> Result<bool, rusqlite::Error> {
    let found: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Executes an atomic `BEGIN IMMEDIATE` AppShell lock mutation (`ACQUIRE` | `RENEW` | `RELEASE` | `TRANSFER`).
/// Uses pre-transaction deterministic SHA-256 jittered retry for transient SQLite contention.
pub fn mutate_appshell_lock(
    conn: &mut Connection,
    cmd: &AppShellLockCommand,
) -> Result<AppShellLockMutationResult, SsotLockError> {
    if cmd.tenant_id.trim().is_empty() || cmd.project_id.trim().is_empty() {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "tenantId and projectId must be non-empty".to_string(),
        });
    }
    if !is_valid_ssot_portal(&cmd.portal_id) {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: format!("Unsupported portalId: {}", cmd.portal_id),
        });
    }
    if cmd.lock_owner_id.trim().is_empty() || cmd.caller_actor_id.trim().is_empty() {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "lockOwnerId and callerActorId must be non-empty".to_string(),
        });
    }
    if cmd.expected_revision < 0 {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "expectedRevision must be >= 0".to_string(),
        });
    }
    if !matches!(cmd.action, AppShellLockAction::Release)
        && !(MIN_LEASE_DURATION_MS..=MAX_LEASE_DURATION_MS).contains(&cmd.ttl_ms)
    {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: format!(
                "ttlMs must be between {MIN_LEASE_DURATION_MS} and {MAX_LEASE_DURATION_MS}"
            ),
        });
    }
    if cmd.locked_slots.len() > 32
        || cmd
            .locked_slots
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 64)
    {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "lockedSlots entries must be 1..=64 chars (max 32 entries)".to_string(),
        });
    }

    if matches!(cmd.action, AppShellLockAction::Transfer) {
        let reason_ok = cmd
            .force_transfer_reason
            .as_deref()
            .map(|s| !s.trim().is_empty() && s.trim().len() <= 256)
            .unwrap_or(false);
        if !cmd.is_privileged_role || !reason_ok {
            return Err(SsotLockError::Forbidden {
                code: "LOCK_TRANSFER_FORBIDDEN",
                message: "TRANSFER requires ADMIN/OWNER role and non-empty forceTransferReason"
                    .to_string(),
            });
        }
    } else if !cmd.is_privileged_role && cmd.lock_owner_id != cmd.caller_actor_id {
        return Err(SsotLockError::Forbidden {
            code: "AUTH_FORBIDDEN",
            message: "lockOwnerId must match authenticated caller unless transferring via admin role"
                .to_string(),
        });
    }

    if matches!(
        cmd.action,
        AppShellLockAction::Renew | AppShellLockAction::Release | AppShellLockAction::Transfer
    ) && cmd.expected_fencing_token.is_none()
    {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: "expectedFencingToken is required for RENEW, RELEASE, and TRANSFER"
                .to_string(),
        });
    }

    let _ = conn.busy_timeout(Duration::from_millis(250));
    let locked_slots_json =
        serde_json::to_string(&cmd.locked_slots).unwrap_or_else(|_| "[]".to_string());

    enum TxOutcome {
        Ok(AppShellLockMutationResult),
        Conflict(AppShellLockConflictDetails),
        NotFound(&'static str, String),
    }

    let outcome = retry_on_busy_with_jitter(
        &cmd.tenant_id,
        &cmd.project_id,
        &cmd.portal_id,
        "appshell_lock_begin_immediate",
        || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if !verify_project_exists_in_tenant(&tx, &cmd.tenant_id, &cmd.project_id)? {
                return Ok(TxOutcome::NotFound(
                    "PROJECT_NOT_FOUND",
                    format!("Project `{}` not found in tenant", cmd.project_id),
                ));
            }

            let now_ms = cmd.now_ms.unwrap_or_else(current_epoch_ms);
            let expires_at = now_ms.saturating_add(cmd.ttl_ms);

            match cmd.action {
                AppShellLockAction::Acquire => {
                    if cmd.expected_revision == 0 {
                        let new_id = Uuid::new_v4().to_string();
                        let inserted = tx.execute(
                            "INSERT OR IGNORE INTO app_shell_locks \
                             (id, tenant_id, project_id, portal_id, lock_owner_id, fencing_token, \
                              locked_slots_json, expires_at, revision, created_at, updated_at) \
                             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, 1, ?8, ?8)",
                            params![
                                new_id,
                                cmd.tenant_id,
                                cmd.project_id,
                                cmd.portal_id,
                                cmd.lock_owner_id,
                                locked_slots_json,
                                expires_at,
                                now_ms
                            ],
                        )?;
                        if inserted == 1 {
                            let row = select_lock_row(
                                &tx,
                                &cmd.tenant_id,
                                &cmd.project_id,
                                &cmd.portal_id,
                                now_ms,
                            )?
                            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                            tx.commit()?;
                            return Ok(TxOutcome::Ok(AppShellLockMutationResult { lock: row }));
                        }
                    }

                    let Some(existing) = select_lock_row(
                        &tx,
                        &cmd.tenant_id,
                        &cmd.project_id,
                        &cmd.portal_id,
                        now_ms,
                    )?
                    else {
                        return Ok(TxOutcome::Conflict(AppShellLockConflictDetails {
                            current_revision: 0,
                            current_fencing_token: 0,
                            expires_at: 0,
                            is_expired: true,
                        }));
                    };

                    let is_tombstone_or_expired = existing.is_expired;
                    if is_tombstone_or_expired && cmd.expected_revision == existing.revision {
                        let updated = tx.execute(
                            "UPDATE app_shell_locks \
                             SET lock_owner_id = ?1, fencing_token = fencing_token + 1, \
                                 revision = revision + 1, locked_slots_json = ?2, \
                                 expires_at = ?3, updated_at = ?4 \
                             WHERE tenant_id = ?5 AND project_id = ?6 AND portal_id = ?7 AND revision = ?8",
                            params![
                                cmd.lock_owner_id,
                                locked_slots_json,
                                expires_at,
                                now_ms,
                                cmd.tenant_id,
                                cmd.project_id,
                                cmd.portal_id,
                                cmd.expected_revision
                            ],
                        )?;
                        if updated == 1 {
                            let row = select_lock_row(
                                &tx,
                                &cmd.tenant_id,
                                &cmd.project_id,
                                &cmd.portal_id,
                                now_ms,
                            )?
                            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                            tx.commit()?;
                            return Ok(TxOutcome::Ok(AppShellLockMutationResult { lock: row }));
                        }
                    }

                    Ok(TxOutcome::Conflict(build_conflict(&existing)))
                }
                AppShellLockAction::Renew => {
                    let expected_tok = cmd.expected_fencing_token.unwrap_or(-1);
                    let updated = tx.execute(
                        "UPDATE app_shell_locks \
                         SET expires_at = ?1, \
                             locked_slots_json = ?2, \
                             revision = revision + 1, \
                             updated_at = ?3 \
                         WHERE tenant_id = ?4 AND project_id = ?5 AND portal_id = ?6 \
                           AND lock_owner_id = ?7 AND fencing_token = ?8 AND revision = ?9 \
                           AND expires_at > ?10",
                        params![
                            expires_at,
                            locked_slots_json,
                            now_ms,
                            cmd.tenant_id,
                            cmd.project_id,
                            cmd.portal_id,
                            cmd.lock_owner_id,
                            expected_tok,
                            cmd.expected_revision,
                            now_ms
                        ],
                    )?;
                    if updated == 1 {
                        let row = select_lock_row(
                            &tx,
                            &cmd.tenant_id,
                            &cmd.project_id,
                            &cmd.portal_id,
                            now_ms,
                        )?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                        tx.commit()?;
                        return Ok(TxOutcome::Ok(AppShellLockMutationResult { lock: row }));
                    }
                    match select_lock_row(
                        &tx,
                        &cmd.tenant_id,
                        &cmd.project_id,
                        &cmd.portal_id,
                        now_ms,
                    )? {
                        Some(existing) => Ok(TxOutcome::Conflict(build_conflict(&existing))),
                        None => Ok(TxOutcome::NotFound(
                            "PROJECT_NOT_FOUND",
                            "AppShell lock does not exist for this portal".to_string(),
                        )),
                    }
                }
                AppShellLockAction::Release => {
                    let expected_tok = cmd.expected_fencing_token.unwrap_or(-1);
                    let updated = tx.execute(
                        "UPDATE app_shell_locks \
                         SET lock_owner_id = '', locked_slots_json = '[]', expires_at = 0, revision = revision + 1, updated_at = ?1 \
                         WHERE tenant_id = ?2 AND project_id = ?3 AND portal_id = ?4 \
                           AND lock_owner_id = ?5 AND fencing_token = ?6 AND revision = ?7 AND expires_at > ?1",
                        params![
                            now_ms,
                            cmd.tenant_id,
                            cmd.project_id,
                            cmd.portal_id,
                            cmd.lock_owner_id,
                            expected_tok,
                            cmd.expected_revision
                        ],
                    )?;
                    if updated == 1 {
                        let row = select_lock_row(
                            &tx,
                            &cmd.tenant_id,
                            &cmd.project_id,
                            &cmd.portal_id,
                            now_ms,
                        )?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                        tx.commit()?;
                        return Ok(TxOutcome::Ok(AppShellLockMutationResult { lock: row }));
                    }
                    match select_lock_row(
                        &tx,
                        &cmd.tenant_id,
                        &cmd.project_id,
                        &cmd.portal_id,
                        now_ms,
                    )? {
                        Some(existing) => Ok(TxOutcome::Conflict(build_conflict(&existing))),
                        None => Ok(TxOutcome::NotFound(
                            "PROJECT_NOT_FOUND",
                            "AppShell lock does not exist for this portal".to_string(),
                        )),
                    }
                }
                AppShellLockAction::Transfer => {
                    let expected_tok = cmd.expected_fencing_token.unwrap_or(-1);
                    let updated = tx.execute(
                        "UPDATE app_shell_locks \
                         SET lock_owner_id = ?1, \
                             fencing_token = fencing_token + 1, \
                             revision = revision + 1, \
                             locked_slots_json = ?2, \
                             expires_at = ?3, \
                             updated_at = ?4 \
                         WHERE tenant_id = ?5 AND project_id = ?6 AND portal_id = ?7 \
                           AND revision = ?8 AND fencing_token = ?9",
                        params![
                            cmd.lock_owner_id,
                            locked_slots_json,
                            expires_at,
                            now_ms,
                            cmd.tenant_id,
                            cmd.project_id,
                            cmd.portal_id,
                            cmd.expected_revision,
                            expected_tok
                        ],
                    )?;
                    if updated == 1 {
                        let row = select_lock_row(
                            &tx,
                            &cmd.tenant_id,
                            &cmd.project_id,
                            &cmd.portal_id,
                            now_ms,
                        )?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                        tx.commit()?;
                        return Ok(TxOutcome::Ok(AppShellLockMutationResult { lock: row }));
                    }
                    match select_lock_row(
                        &tx,
                        &cmd.tenant_id,
                        &cmd.project_id,
                        &cmd.portal_id,
                        now_ms,
                    )? {
                        Some(existing) => Ok(TxOutcome::Conflict(build_conflict(&existing))),
                        None => Ok(TxOutcome::NotFound(
                            "PROJECT_NOT_FOUND",
                            "AppShell lock does not exist for this portal".to_string(),
                        )),
                    }
                }
            }
        },
    );

    let _ = conn.busy_timeout(Duration::from_millis(5000));
    let outcome = outcome?;

    match outcome {
        TxOutcome::Ok(res) => Ok(res),
        TxOutcome::Conflict(details) => Err(SsotLockError::FencingConflict(details)),
        TxOutcome::NotFound(code, message) => Err(SsotLockError::NotFound { code, message }),
    }
}

pub fn get_appshell_lock_for_portal(
    conn: &Connection,
    tenant_id: &str,
    project_id: &str,
    portal_id: &str,
    now_ms: Option<i64>,
) -> Result<Option<AppShellLockDto>, SsotLockError> {
    if !is_valid_ssot_portal(portal_id) {
        return Err(SsotLockError::Validation {
            code: "INVALID_REQUEST",
            message: format!("Unsupported portalId: {portal_id}"),
        });
    }
    let now = now_ms.unwrap_or_else(current_epoch_ms);
    conn.query_row(
        "SELECT id, project_id, portal_id, lock_owner_id, fencing_token, locked_slots_json, \
                expires_at, revision, created_at, updated_at \
         FROM app_shell_locks \
         WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3",
        params![tenant_id, project_id, portal_id],
        |row| map_lock_row(row, now),
    )
    .optional()
    .map_err(|e| SsotLockError::Internal {
        code: "INTERNAL_ERROR",
        message: map_sqlite_error(e, "get_appshell_lock_for_portal").to_string(),
    })
}
