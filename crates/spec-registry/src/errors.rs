use domain::HarnessError;
use rusqlite::ffi;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Determines if a `rusqlite::Error` represents a transient SQLite lock/busy condition.
pub fn is_sqlite_busy_or_locked(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(ffi_err, _) => matches!(
            ffi_err.code,
            ffi::ErrorCode::DatabaseBusy | ffi::ErrorCode::DatabaseLocked
        ),
        _ => false,
    }
}

/// Deterministically maps a `rusqlite::Error` into `HarnessError`.
/// - `SQLITE_BUSY` / `SQLITE_LOCKED` -> `HarnessError::SqliteBusy`
/// - `SQLITE_CONSTRAINT_UNIQUE` (2067) / `SQLITE_CONSTRAINT_PRIMARYKEY` (1555) -> `UNIQUE_CONSTRAINT`
/// - `SQLITE_CONSTRAINT_FOREIGNKEY` (787) -> `REFERENTIAL_INTEGRITY`
/// - `SQLITE_CONSTRAINT_CHECK` (275) -> `CHECK_CONSTRAINT`
pub fn map_sqlite_error(err: rusqlite::Error, context: &str) -> HarnessError {
    if is_sqlite_busy_or_locked(&err) {
        return HarnessError::SqliteBusy {
            details: format!("{context}: {err}"),
        };
    }

    if let rusqlite::Error::SqliteFailure(ffi_err, ref msg_opt) = err {
        let msg = msg_opt.as_deref().unwrap_or("constraint violation");
        match ffi_err.extended_code {
            ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                return HarnessError::ValidationFailed {
                    reason: format!("UNIQUE_CONSTRAINT [{context}]: {msg}"),
                };
            }
            ffi::SQLITE_CONSTRAINT_FOREIGNKEY => {
                return HarnessError::ValidationFailed {
                    reason: format!("REFERENTIAL_INTEGRITY [{context}]: {msg}"),
                };
            }
            ffi::SQLITE_CONSTRAINT_CHECK => {
                return HarnessError::ValidationFailed {
                    reason: format!("CHECK_CONSTRAINT [{context}]: {msg}"),
                };
            }
            _ => {}
        }
    }

    HarnessError::ValidationFailed {
        reason: format!("SQLITE_ERROR [{context}]: {err}"),
    }
}

/// Bounded retry policy (`25ms`, `50ms`, `100ms` + `0..15ms` jitter, total wall-clock cap `<= 5,000ms`)
/// for SQLite operations encountering `SQLITE_BUSY` / `SQLITE_LOCKED`.
pub fn retry_on_busy<T, F>(context: &str, mut op: F) -> Result<T, HarnessError>
where
    F: FnMut() -> Result<T, rusqlite::Error>,
{
    const BACKOFF_MS: [u64; 3] = [25, 50, 100];
    const MAX_WALL_MS: u128 = 5_000;
    let started = Instant::now();
    let mut attempt = 0usize;

    loop {
        match op() {
            Ok(val) => return Ok(val),
            Err(err) => {
                let elapsed = started.elapsed().as_millis();
                if is_sqlite_busy_or_locked(&err)
                    && attempt < BACKOFF_MS.len()
                    && elapsed < MAX_WALL_MS
                {
                    let jitter_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| u64::from(d.subsec_nanos() % 16))
                        .unwrap_or(0);
                    let remaining = (MAX_WALL_MS.saturating_sub(elapsed)) as u64;
                    let sleep_ms = (BACKOFF_MS[attempt] + jitter_ms).min(remaining);
                    attempt += 1;
                    if sleep_ms > 0 {
                        std::thread::sleep(Duration::from_millis(sleep_ms));
                    }
                    continue;
                }
                return Err(map_sqlite_error(err, context));
            }
        }
    }
}
