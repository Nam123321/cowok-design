// @story 01.2, 04.1
// @implements AC-08, AC-04, AC-06
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::Utc;
use domain::HarnessError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::errors::{map_sqlite_error, retry_on_busy};

pub const V1_MIGRATION_SQL: &str = include_str!("../migrations/V1__initial_16_tables.sql");
pub const V2_MIGRATION_SQL: &str =
    include_str!("../migrations/V2__project_boundaries_and_chat_sessions.sql");
pub const V3_MIGRATION_SQL: &str = include_str!("../migrations/V3__flow_workbench_dag.sql");
pub const V3_MIGRATION_NAME: &str = "V3__flow_workbench_dag.sql";

use std::sync::OnceLock;

fn cached_migration_sha256(cache: &OnceLock<String>, sql: &str) -> String {
    cache
        .get_or_init(|| {
            let mut hasher = Sha256::new();
            hasher.update(sql.as_bytes());
            format!("{:x}", hasher.finalize())
        })
        .clone()
}

pub fn v3_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V3_MIGRATION_SQL)
}

pub const V4_MIGRATION_SQL: &str =
    include_str!("../migrations/V4__storyboard_chat_and_execution_engine.sql");
pub const V4_MIGRATION_NAME: &str = "V4__storyboard_chat_and_execution_engine.sql";

pub fn v4_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V4_MIGRATION_SQL)
}

pub const V5_MIGRATION_SQL: &str =
    include_str!("../migrations/V5__proposals_snapshots_runtime_columns.sql");
pub const V5_MIGRATION_NAME: &str = "V5__proposals_snapshots_runtime_columns.sql";

pub fn v5_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V5_MIGRATION_SQL)
}

pub const V6_MIGRATION_SQL: &str = include_str!("../migrations/V6__epic04_complete_contracts.sql");
pub const V6_MIGRATION_NAME: &str = "V6__epic04_complete_contracts.sql";

pub fn v6_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V6_MIGRATION_SQL)
}

pub const V7_MIGRATION_SQL: &str =
    include_str!("../migrations/V7__epic04_cas_seq_and_vault_convergence.sql");
pub const V7_MIGRATION_NAME: &str = "V7__epic04_cas_seq_and_vault_convergence.sql";

pub fn v7_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V7_MIGRATION_SQL)
}

pub const V8_MIGRATION_SQL: &str =
    include_str!("../migrations/V8__daemon_auth_sessions_and_nonces.sql");
pub const V8_MIGRATION_NAME: &str = "V8__daemon_auth_sessions_and_nonces.sql";

pub fn v8_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V8_MIGRATION_SQL)
}

pub const V9_MIGRATION_SQL: &str =
    include_str!("../migrations/V9__epic05_storyboard_proposals_handoff.sql");
pub const V9_MIGRATION_NAME: &str = "V9__epic05_storyboard_proposals_handoff.sql";

pub fn v9_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V9_MIGRATION_SQL)
}

pub const V10_MIGRATION_SQL: &str =
    include_str!("../migrations/V10__story_05_2_proposals_receipts_tradeoffs.sql");
pub const V10_MIGRATION_NAME: &str = "V10__story_05_2_proposals_receipts_tradeoffs.sql";

pub fn v10_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V10_MIGRATION_SQL)
}

pub const V11_MIGRATION_SQL: &str =
    include_str!("../migrations/V11__story_05_3_snapshots_fork_handoff.sql");
pub const V11_MIGRATION_NAME: &str = "V11__story_05_3_snapshots_fork_handoff.sql";

pub fn v11_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V11_MIGRATION_SQL)
}

pub const V12_MIGRATION_SQL: &str =
    include_str!("../migrations/V12__epic08_plugins_learn_cron.sql");
pub const V12_MIGRATION_NAME: &str = "V12__epic08_plugins_learn_cron.sql";

pub fn v12_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V12_MIGRATION_SQL)
}

pub const V13_MIGRATION_SQL: &str = include_str!("../migrations/V13__epic08_audit_remediation.sql");
pub const V13_MIGRATION_NAME: &str = "V13__epic08_audit_remediation.sql";

pub fn v13_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V13_MIGRATION_SQL)
}

pub const V14_MIGRATION_SQL: &str = include_str!("../migrations/V14__epic08_party_remediation.sql");
pub const V14_MIGRATION_NAME: &str = "V14__epic08_party_remediation.sql";

pub fn v14_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V14_MIGRATION_SQL)
}

pub const V15_MIGRATION_SQL: &str =
    include_str!("../migrations/V15__epic06_presentation_cms_and_charts.sql");
pub const V15_MIGRATION_NAME: &str = "V15__epic06_presentation_cms_and_charts.sql";
const LEGACY_EPIC06_V12_SHA256: &str =
    "7b8d9c63dec95c85ff0239728c6670da1cedfbcde82adbae29c6b1c2d7718b9d";

pub fn v15_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V15_MIGRATION_SQL)
}

pub const V16_MIGRATION_SQL: &str = include_str!("../migrations/V16__story_04_6_cas_blobs.sql");
pub const V16_MIGRATION_NAME: &str = "V16__story_04_6_cas_blobs.sql";

pub fn v16_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V16_MIGRATION_SQL)
}

pub const V17_MIGRATION_SQL: &str =
    include_str!("../migrations/V17__story_02_1_context_telemetry.sql");
pub const V17_MIGRATION_NAME: &str = "V17__story_02_1_context_telemetry.sql";

pub fn v17_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V17_MIGRATION_SQL)
}

pub const V18_MIGRATION_SQL: &str = include_str!("../migrations/V18__story_02_4_hidden_ssot.sql");
pub const V18_MIGRATION_NAME: &str = "V18__story_02_4_hidden_ssot.sql";

pub fn v18_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V18_MIGRATION_SQL)
}

pub const V19_MIGRATION_SQL: &str =
    include_str!("../migrations/V19__story_05_4_html_forge_and_sketch_polish.sql");
pub const V19_MIGRATION_NAME: &str = "V19__story_05_4_html_forge_and_sketch_polish.sql";

pub fn v19_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V19_MIGRATION_SQL)
}

pub const V20_MIGRATION_SQL: &str =
    include_str!("../migrations/V20__story_08_5_de_dr_learn_ledger.sql");
pub const V20_MIGRATION_NAME: &str = "V20__story_08_5_de_dr_learn_ledger.sql";

pub fn v20_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V20_MIGRATION_SQL)
}

pub const V21_MIGRATION_SQL: &str =
    include_str!("../migrations/V21__story_09_2_decision_audit_log.sql");
pub const V21_MIGRATION_NAME: &str = "V21__story_09_2_decision_audit_log.sql";

pub fn v21_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V21_MIGRATION_SQL)
}

pub const V22_MIGRATION_SQL: &str =
    include_str!("../migrations/V22__story_05_5_flow_node_substates.sql");
pub const V22_MIGRATION_NAME: &str = "V22__story_05_5_flow_node_substates.sql";

pub fn v22_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V22_MIGRATION_SQL)
}

pub const V23_MIGRATION_SQL: &str =
    include_str!("../migrations/V23__story_14_1_support_telemetry_outbox.sql");
pub const V23_MIGRATION_NAME: &str = "V23__story_14_1_support_telemetry_outbox.sql";

pub fn v23_migration_checksum() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    cached_migration_sha256(&CACHE, V23_MIGRATION_SQL)
}
/// Configuration for the embedded SQLite WAL connection pool.
#[derive(Debug, Clone)]
pub struct SqliteWalConfig {
    pub db_path: PathBuf,
    pub workspace_root: PathBuf,
    pub busy_timeout_ms: u32,
}

impl SqliteWalConfig {
    pub fn new(db_path: impl Into<PathBuf>, workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
            workspace_root: workspace_root.into(),
            busy_timeout_ms: 5_000,
        }
    }
}

struct MigrateLockGuard {
    #[cfg(unix)]
    file: fs::File,
}

impl MigrateLockGuard {
    #[allow(clippy::suspicious_open_options)]
    fn acquire(lock_path: &Path) -> Result<Self, HarnessError> {
        if let Ok(meta) = fs::symlink_metadata(lock_path) {
            if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                return Err(HarnessError::PathTraversalDenied {
                    path: lock_path.display().to_string(),
                });
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::os::unix::io::AsRawFd;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(lock_path)
                .map_err(|e| {
                    if e.raw_os_error() == Some(libc::ELOOP) {
                        return HarnessError::PathTraversalDenied {
                            path: lock_path.display().to_string(),
                        };
                    }
                    HarnessError::ValidationFailed {
                        reason: format!(
                            "DB_MIGRATE_LOCK_OPEN_FAILED [{}]: {e}",
                            lock_path.display()
                        ),
                    }
                })?;
            enforce_unix_mode(lock_path, 0o600)?;
            let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            if rc != 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "DB_MIGRATE_LOCK_FLOCK_FAILED [{}]: {}",
                        lock_path.display(),
                        std::io::Error::last_os_error()
                    ),
                });
            }
            Ok(Self { file })
        }
        #[cfg(not(unix))]
        {
            let _ = lock_path;
            Err(HarnessError::ValidationFailed {
                reason: "DB_MIGRATE_LOCK_UNSUPPORTED: advisory file locking is required for migrations on non-Unix targets (fail-closed)".to_string(),
            })
        }
    }
}

#[cfg(unix)]
impl Drop for MigrateLockGuard {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(unix)]
fn enforce_unix_mode(path: &Path, mode: u32) -> Result<(), HarnessError> {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(HarnessError::PathTraversalDenied {
                path: path.display().to_string(),
            });
        }
        let mut perms = meta.permissions();
        if (perms.mode() & 0o777) != mode {
            perms.set_mode(mode);
            let _ = fs::set_permissions(path, perms);
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn enforce_unix_mode(_path: &Path, _mode: u32) -> Result<(), HarnessError> {
    Ok(())
}

fn harden_sqlite_files(db_path: &Path) -> Result<(), HarnessError> {
    enforce_unix_mode(db_path, 0o600)?;
    let wal_path = PathBuf::from(format!("{}-wal", db_path.display()));
    let shm_path = PathBuf::from(format!("{}-shm", db_path.display()));
    enforce_unix_mode(&wal_path, 0o600)?;
    enforce_unix_mode(&shm_path, 0o600)?;
    Ok(())
}

/// Embedded SQLite WAL pool enforcing mandatory PRAGMAs and checksum-gated V1 -> V2 -> V3 migrations.
#[derive(Debug, Clone)]
pub struct SqlitePool {
    config: SqliteWalConfig,
    migration_elapsed_ms: u128,
}

impl SqlitePool {
    /// Opens the SQLite WAL pool, enforces `.cowok-design` (`0700`) / `.db*` (`0600`) permissions,
    /// serializes DDL upgrades via `db-migrate.lock`, and executes V1 + V2 + 12-step V3 rebuild.
    pub fn open_wal(config: SqliteWalConfig) -> Result<Self, HarnessError> {
        let parent_dir = config
            .db_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| config.workspace_root.clone());

        fs::create_dir_all(&parent_dir).map_err(|e| HarnessError::ValidationFailed {
            reason: format!("Failed to create SQLite parent directory: {e}"),
        })?;
        let parent_meta =
            fs::symlink_metadata(&parent_dir).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("Failed to stat SQLite parent directory: {e}"),
            })?;
        if parent_meta.file_type().is_symlink() || !parent_meta.file_type().is_dir() {
            return Err(HarnessError::PathTraversalDenied {
                path: parent_dir.display().to_string(),
            });
        }
        let is_workspace_root = parent_dir
            .canonicalize()
            .ok()
            .zip(config.workspace_root.canonicalize().ok())
            .map(|(p, w)| p == w)
            .unwrap_or(false);
        if !is_workspace_root {
            enforce_unix_mode(&parent_dir, 0o700)?;
        }

        let start = Instant::now();
        let lock_path = parent_dir.join("db-migrate.lock");
        let _migrate_guard = MigrateLockGuard::acquire(&lock_path)?;
        let is_genesis_empty = match fs::symlink_metadata(&config.db_path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(HarnessError::PathTraversalDenied {
                        path: config.db_path.display().to_string(),
                    });
                }
                meta.len() == 0
            }
            Err(_) => true,
        };

        if is_genesis_empty {
            let db_bytes = Self::cached_genesis_db_bytes(&parent_dir)?;
            let tmp_path = parent_dir.join(format!(".genesis-init-{}.tmp", uuid::Uuid::new_v4()));
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let write_res = (|| -> Result<(), std::io::Error> {
                    let mut f = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .custom_flags(libc::O_NOFOLLOW)
                        .mode(0o600)
                        .open(&tmp_path)?;
                    f.write_all(db_bytes)?;
                    f.sync_all()?;
                    Ok(())
                })();
                if let Err(e) = write_res {
                    let _ = fs::remove_file(&tmp_path);
                    return Err(HarnessError::ValidationFailed {
                        reason: format!("GENESIS_WRITE_FAILED: {e}"),
                    });
                }
            }
            #[cfg(not(unix))]
            {
                if let Err(e) = fs::write(&tmp_path, db_bytes) {
                    let _ = fs::remove_file(&tmp_path);
                    return Err(HarnessError::ValidationFailed {
                        reason: format!("GENESIS_WRITE_FAILED: {e}"),
                    });
                }
            }
            if let Err(e) = fs::rename(&tmp_path, &config.db_path) {
                let _ = fs::remove_file(&tmp_path);
                return Err(HarnessError::ValidationFailed {
                    reason: format!("GENESIS_WRITE_FAILED: {e}"),
                });
            }
            let _conn = Self::connect_with_pragmas_opts(&config, true)?;
        } else {
            let mut conn = Self::connect_with_pragmas_opts(&config, true)?;
            harden_sqlite_files(&config.db_path)?;

            retry_on_busy("ensure_schema_migrations_ledger", || {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS schema_migrations (
                        version INTEGER PRIMARY KEY NOT NULL,
                        name TEXT NOT NULL,
                        checksum_sha256 TEXT NOT NULL,
                        applied_at BIGINT NOT NULL
                    );",
                )
            })?;

            let expected_v3_sha = v3_migration_checksum();
            let existing_v3_sha: Option<String> = conn
                .query_row(
                    "SELECT checksum_sha256 FROM schema_migrations WHERE version = 3",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| map_sqlite_error(e, "read_v3_migration_ledger"))?;

            if let Some(actual_sha) = existing_v3_sha {
                if actual_sha != expected_v3_sha {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "MIGRATION_CHECKSUM_MISMATCH [V3]: expected {expected_v3_sha}, found {actual_sha}"
                        ),
                    });
                }
            } else {
                retry_on_busy("genesis_v1_v2_migration", || {
                    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                    tx.execute_batch(V1_MIGRATION_SQL)?;
                    tx.execute_batch(V2_MIGRATION_SQL)?;
                    tx.commit()
                })?;

                Self::apply_v3_rebuild_if_needed(&mut conn, &config.db_path)?;
            }
            Self::apply_v4_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v5_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v6_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v7_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v8_migration_if_needed(&mut conn, &config.db_path)?;
            Self::reconcile_legacy_epic08_v9_to_v11_slots(&mut conn, &config.db_path)?;
            Self::reconcile_legacy_epic06_v12_slot(&mut conn, &config.db_path)?;
            Self::apply_v9_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v10_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v11_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v12_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v13_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v14_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v15_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v16_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v17_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v18_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v19_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v20_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v21_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v22_migration_if_needed(&mut conn, &config.db_path)?;
            Self::apply_v23_migration_if_needed(&mut conn, &config.db_path)?;
        }
        harden_sqlite_files(&config.db_path)?;

        let migration_elapsed_ms = start.elapsed().as_millis();
        Ok(Self {
            config,
            migration_elapsed_ms,
        })
    }

    fn apply_v3_rebuild_if_needed(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v3_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 3",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v3_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V3]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        // Conditional backup ONLY when upgrading a populated pre-V3 database (preserves <15ms cold start on empty DBs)
        let populated_rows: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM projects)
                      + (SELECT COUNT(*) FROM flow_graphs)
                      + (SELECT COUNT(*) FROM flow_nodes)
                      + (SELECT COUNT(*) FROM screen_artifacts)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if populated_rows > 0 {
            Self::create_pre_v3_backup(conn, db_path)?;
        }

        // Idempotent projects.tenant_id column addition before V3 rebuild joins p.tenant_id
        let has_tenant_col: bool = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(projects)")
                .map_err(|e| map_sqlite_error(e, "pragma_projects_info"))?;
            let cols: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| map_sqlite_error(e, "query_projects_cols"))?
                .filter_map(Result::ok)
                .collect();
            cols.iter().any(|c| c == "tenant_id")
        };
        if !has_tenant_col {
            conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';",
            )
            .map_err(|e| map_sqlite_error(e, "alter_projects_add_tenant_id"))?;
        }

        let pre_artifacts_digest = if populated_rows > 0 {
            Some(Self::compute_screen_artifacts_digest(conn)?)
        } else {
            None
        };

        // SQLite 12-Step Rule: PRAGMA foreign_keys = OFF & legacy_alter_table = OFF MUST run OUTSIDE transaction
        if !conn.is_autocommit() {
            return Err(HarnessError::ValidationFailed {
                reason: "V3_MIGRATION_PRECONDITION_FAILED: connection must be in autocommit mode"
                    .to_string(),
            });
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = OFF;",
        )
        .map_err(|e| map_sqlite_error(e, "disable_fk_for_v3_rebuild"))?;

        let fk_off: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_off"))?;
        if fk_off != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: "V3_MIGRATION_PRECONDITION_FAILED: PRAGMA foreign_keys remained ON"
                    .to_string(),
            });
        }

        let rebuild_res = (|| -> Result<(), HarnessError> {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(|e| map_sqlite_error(e, "begin_exclusive_v3"))?;
            tx.execute_batch(V3_MIGRATION_SQL)
                .map_err(|e| map_sqlite_error(e, "execute_v3_sql"))?;

            if let Some(ref expected_digest) = pre_artifacts_digest {
                let post_artifacts_digest = Self::compute_screen_artifacts_digest(&tx)?;
                if *expected_digest != post_artifacts_digest {
                    return Err(HarnessError::ValidationFailed {
                        reason:
                            "V3_MIGRATION_SCREEN_ARTIFACTS_DRIFT: screen_artifacts digest changed"
                                .to_string(),
                    });
                }
            }

            let fk_violation_count: usize = {
                let mut stmt = tx
                    .prepare("PRAGMA foreign_key_check;")
                    .map_err(|e| map_sqlite_error(e, "prepare_fk_check"))?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(|e| map_sqlite_error(e, "run_fk_check"))?;
                rows.count()
            };
            if fk_violation_count > 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "V3_MIGRATION_FK_CHECK_FAILED: {fk_violation_count} foreign key violation(s)"
                    ),
                });
            }

            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (3, ?1, ?2, ?3)",
                params![
                    V3_MIGRATION_NAME,
                    expected_sha,
                    Utc::now().timestamp_millis()
                ],
            )
            .map_err(|e| map_sqlite_error(e, "insert_v3_schema_migration"))?;

            tx.commit()
                .map_err(|e| map_sqlite_error(e, "commit_v3_migration"))?;
            Ok(())
        })();

        // Always restore foreign_keys = ON before propagating result
        let restore_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
        rebuild_res?;
        restore_res.map_err(|e| map_sqlite_error(e, "restore_fk_on"))?;

        let fk_on: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_on"))?;
        if fk_on != 1 {
            return Err(HarnessError::ValidationFailed {
                reason:
                    "V3_MIGRATION_POSTCONDITION_FAILED: PRAGMA foreign_keys did not restore to 1"
                        .to_string(),
            });
        }
        Ok(())
    }

    fn apply_v4_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v4_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 4",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v4_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V4]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v4"))?;

        // 1. Idempotent columns on projects (project_code, trash_deleted_at, provenance)
        let pragma_cols: Vec<String> = {
            let mut stmt = tx
                .prepare("PRAGMA table_info(projects)")
                .map_err(|e| map_sqlite_error(e, "pragma_projects_info_v4"))?;
            let cols = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| map_sqlite_error(e, "query_projects_cols_v4"))?
                .filter_map(Result::ok)
                .collect();
            cols
        };

        if !pragma_cols.iter().any(|c| c == "project_code") {
            tx.execute_batch(
                "ALTER TABLE projects ADD COLUMN project_code TEXT;
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_code ON projects(project_code) WHERE project_code IS NOT NULL;
                 UPDATE projects SET project_code = 'CWK-' || SUBSTR(UPPER(HEX(RANDOMBLOB(3))), 1, 6) WHERE project_code IS NULL;"
            )
            .map_err(|e| map_sqlite_error(e, "alter_projects_add_project_code"))?;
        }

        if !pragma_cols.iter().any(|c| c == "trash_deleted_at") {
            tx.execute_batch("ALTER TABLE projects ADD COLUMN trash_deleted_at BIGINT;")
                .map_err(|e| map_sqlite_error(e, "alter_projects_add_trash_deleted_at"))?;
        }

        if !pragma_cols.iter().any(|c| c == "provenance") {
            tx.execute_batch(
                "ALTER TABLE projects ADD COLUMN provenance TEXT NOT NULL DEFAULT 'scratch';",
            )
            .map_err(|e| map_sqlite_error(e, "alter_projects_add_provenance"))?;
        }

        // Ensure V2-created chat_sessions table receives archived_at required by V4
        let chat_cols: Vec<String> = {
            let mut stmt = tx
                .prepare("PRAGMA table_info(chat_sessions)")
                .map_err(|e| map_sqlite_error(e, "pragma_chat_sessions_info_v4"))?;
            let cols = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| map_sqlite_error(e, "query_chat_sessions_cols_v4"))?
                .filter_map(Result::ok)
                .collect();
            cols
        };
        if !chat_cols.is_empty() && !chat_cols.iter().any(|c| c == "archived_at") {
            tx.execute_batch("ALTER TABLE chat_sessions ADD COLUMN archived_at BIGINT;")
                .map_err(|e| map_sqlite_error(e, "alter_chat_sessions_add_archived_at"))?;
        }

        // 2. Execute V4 SQL batch and insert migration receipt
        tx.execute_batch(V4_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v4_sql"))?;

        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (4, ?1, ?2, ?3)",
            params![
                V4_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v4_schema_migration"))?;

        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v4_migration"))?;

        Ok(())
    }

    fn ensure_columns_idempotent(
        conn: &Connection,
        table: &str,
        columns: &[(&str, &str)],
    ) -> Result<(), HarnessError> {
        let existing: Vec<String> = {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .map_err(|e| map_sqlite_error(e, "pragma_table_info_v5"))?;
            let cols = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| map_sqlite_error(e, "query_table_cols_v5"))?
                .filter_map(Result::ok)
                .collect();
            cols
        };
        if existing.is_empty() {
            return Ok(());
        }
        let mut batch = String::new();
        for &(col_name, col_def) in columns {
            if !existing.iter().any(|c| c == col_name) {
                batch.push_str(&format!(
                    "ALTER TABLE {table} ADD COLUMN {col_name} {col_def};\n"
                ));
            }
        }
        if !batch.is_empty() {
            conn.execute_batch(&batch)
                .map_err(|e| map_sqlite_error(e, "alter_table_add_col_v5"))?;
        }
        Ok(())
    }

    fn apply_v5_columns(tx: &Connection) -> Result<(), HarnessError> {
        for (table, cols) in [
            (
                "change_proposals",
                &[
                    ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                    ("intent_id", "TEXT"),
                    ("title", "TEXT NOT NULL DEFAULT ''"),
                    ("description", "TEXT NOT NULL DEFAULT ''"),
                    ("files_json", "TEXT NOT NULL DEFAULT '[]'"),
                    ("validation_status", "TEXT NOT NULL DEFAULT 'unverified'"),
                    ("validation_checks_json", "TEXT NOT NULL DEFAULT '[]'"),
                ][..],
            ),
            (
                "version_snapshots",
                &[
                    ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                    ("version_tag", "TEXT NOT NULL DEFAULT ''"),
                    ("label", "TEXT NOT NULL DEFAULT ''"),
                    ("description", "TEXT NOT NULL DEFAULT ''"),
                    ("trigger", "TEXT NOT NULL DEFAULT 'manual'"),
                    ("tree_sha256", "TEXT NOT NULL DEFAULT ''"),
                    ("files_json", "TEXT NOT NULL DEFAULT '[]'"),
                ][..],
            ),
            (
                "project_workspace_files",
                &[("content", "TEXT NOT NULL DEFAULT ''")][..],
            ),
            (
                "github_pull_requests",
                &[
                    ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                    ("pr_url", "TEXT NOT NULL DEFAULT ''"),
                ][..],
            ),
        ] {
            Self::ensure_columns_idempotent(tx, table, cols)?;
        }
        Ok(())
    }

    fn apply_v6_columns(tx: &Connection) -> Result<(), HarnessError> {
        for (table, cols) in [
            (
                "chat_sessions",
                &[
                    ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                    ("archived_at", "BIGINT"),
                ][..],
            ),
            (
                "chat_messages",
                &[("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'")][..],
            ),
            (
                "node_execution_jobs",
                &[("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'")][..],
            ),
            (
                "chat_artifact_snapshots",
                &[("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'")][..],
            ),
        ] {
            Self::ensure_columns_idempotent(tx, table, cols)?;
        }
        Ok(())
    }

    fn apply_v7_columns(tx: &Connection) -> Result<(), HarnessError> {
        for (table, cols) in [
            (
                "projects",
                &[
                    ("project_code", "TEXT"),
                    ("trash_deleted_at", "BIGINT"),
                    ("provenance", "TEXT NOT NULL DEFAULT 'scratch'"),
                ][..],
            ),
            (
                "chat_sessions",
                &[
                    ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                    ("archived_at", "BIGINT"),
                ][..],
            ),
            (
                "chat_message_node_scopes",
                &[("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'")][..],
            ),
            (
                "execution_events",
                &[
                    ("project_seq", "INTEGER"),
                    ("composite_scope_key", "TEXT NOT NULL DEFAULT ''"),
                ][..],
            ),
            (
                "element_comments",
                &[
                    ("artifact_sha256", "TEXT NOT NULL DEFAULT ''"),
                    ("anchor_kind", "TEXT NOT NULL DEFAULT 'html_element'"),
                ][..],
            ),
            (
                "agent_settings",
                &[
                    ("permission_mode", "TEXT NOT NULL DEFAULT 'ask-every-time'"),
                    ("vault_mode", "TEXT NOT NULL DEFAULT 'keychain'"),
                    ("onboarding_done", "INTEGER NOT NULL DEFAULT 0"),
                ][..],
            ),
            (
                "node_chat_sessions",
                &[
                    ("composite_scope_key", "TEXT NOT NULL DEFAULT ''"),
                    ("unread_count", "INTEGER NOT NULL DEFAULT 0"),
                    ("last_message_id", "TEXT"),
                ][..],
            ),
        ] {
            Self::ensure_columns_idempotent(tx, table, cols)?;
        }
        Ok(())
    }

    fn apply_v9_columns(tx: &Connection) -> Result<(), HarnessError> {
        for (table, cols) in [
            (
                "sdi_intents",
                &[
                    ("routing_mode", "TEXT NOT NULL DEFAULT 'direct_edit'"),
                    ("gap_questions_json", "TEXT"),
                    ("revision", "INTEGER NOT NULL DEFAULT 1"),
                ][..],
            ),
            (
                "flow_nodes",
                &[
                    ("portal_group", "TEXT"),
                    ("shared_app_shell_id", "TEXT"),
                    ("mini_skeleton_html", "TEXT"),
                    ("cta_pills_json", "TEXT NOT NULL DEFAULT '[]'"),
                    ("revision", "INTEGER NOT NULL DEFAULT 1"),
                ][..],
            ),
            (
                "screen_fixtures",
                &[
                    ("preset_key", "TEXT"),
                    ("sample_rows_json", "TEXT NOT NULL DEFAULT '[]'"),
                    ("revision", "INTEGER NOT NULL DEFAULT 1"),
                ][..],
            ),
            (
                "flow_graphs",
                &[("revision", "INTEGER NOT NULL DEFAULT 1")][..],
            ),
        ] {
            Self::ensure_columns_idempotent(tx, table, cols)?;
        }
        Ok(())
    }

    fn apply_v10_columns(tx: &Connection) -> Result<(), HarnessError> {
        Self::ensure_columns_idempotent(
            tx,
            "change_proposals",
            &[
                ("node_id", "TEXT"),
                ("batch_id", "TEXT"),
                ("composite_scope_key", "TEXT NOT NULL DEFAULT ''"),
                ("content_mode", "TEXT NOT NULL DEFAULT 'full_file'"),
                ("base_file_hashes_json", "TEXT NOT NULL DEFAULT '{}'"),
                ("visual_diff_json", "TEXT NOT NULL DEFAULT '{}'"),
                ("gate_receipt_id", "TEXT"),
            ],
        )
    }

    fn apply_v11_columns(tx: &Connection) -> Result<(), HarnessError> {
        for (table, cols) in [
            (
                "chat_sessions",
                &[
                    (
                        "forked_from_session_id",
                        "TEXT REFERENCES chat_sessions(id) ON DELETE SET NULL",
                    ),
                    (
                        "forked_from_message_id",
                        "TEXT REFERENCES chat_messages(id) ON DELETE SET NULL",
                    ),
                ][..],
            ),
            (
                "chat_artifact_snapshots",
                &[
                    (
                        "proposal_id",
                        "TEXT REFERENCES change_proposals(id) ON DELETE SET NULL",
                    ),
                    (
                        "parent_snapshot_id",
                        "TEXT REFERENCES chat_artifact_snapshots(id) ON DELETE SET NULL",
                    ),
                    ("node_snapshot_json", "TEXT NOT NULL DEFAULT '{}'"),
                ][..],
            ),
            (
                "version_snapshots",
                &[
                    (
                        "proposal_id",
                        "TEXT REFERENCES change_proposals(id) ON DELETE SET NULL",
                    ),
                    (
                        "parent_snapshot_id",
                        "TEXT REFERENCES version_snapshots(id) ON DELETE SET NULL",
                    ),
                    (
                        "chat_artifact_snapshot_id",
                        "TEXT REFERENCES chat_artifact_snapshots(id) ON DELETE SET NULL",
                    ),
                ][..],
            ),
            (
                "github_pull_requests",
                &[
                    (
                        "snapshot_id",
                        "TEXT REFERENCES version_snapshots(id) ON DELETE SET NULL",
                    ),
                    ("handoff_manifest_sha256", "TEXT NOT NULL DEFAULT ''"),
                    ("visual_diff_summary_json", "TEXT NOT NULL DEFAULT '{}'"),
                ][..],
            ),
        ] {
            Self::ensure_columns_idempotent(tx, table, cols)?;
        }
        Ok(())
    }

    fn apply_v12_columns(tx: &Connection) -> Result<(), HarnessError> {
        Self::ensure_columns_idempotent(
            tx,
            "skill_plugins",
            &[
                ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                ("source_type", "TEXT NOT NULL DEFAULT 'local'"),
                ("upstream_repo_url", "TEXT"),
                ("upstream_commit_sha", "TEXT"),
                ("cron_schedule", "TEXT"),
                ("skill_md_path", "TEXT NOT NULL DEFAULT 'SKILL.md'"),
                ("plugin_json_path", "TEXT NOT NULL DEFAULT 'plugin.json'"),
                ("last_synced_at", "BIGINT"),
                ("revision", "BIGINT NOT NULL DEFAULT 1"),
                ("staging_version", "TEXT"),
                ("staging_manifest_json", "TEXT"),
                ("staging_diff_reasons_json", "TEXT"),
                ("staging_bundle_dir", "TEXT"),
                ("staging_package_sha256", "TEXT"),
                ("breaking_change_isolated", "INTEGER NOT NULL DEFAULT 0"),
                ("quarantined", "INTEGER NOT NULL DEFAULT 0"),
                ("quarantine_reason", "TEXT"),
            ][..],
        )
    }

    fn apply_v14_columns(tx: &Connection) -> Result<(), HarnessError> {
        Self::ensure_columns_idempotent(
            tx,
            "gate_reports",
            &[
                ("tenant_id", "TEXT NOT NULL DEFAULT 'local-default'"),
                ("correlation_id", "TEXT NOT NULL DEFAULT ''"),
                ("target_id", "TEXT NOT NULL DEFAULT ''"),
                ("action", "TEXT NOT NULL DEFAULT ''"),
                ("revision", "BIGINT NOT NULL DEFAULT 0"),
                ("fencing_token", "BIGINT NOT NULL DEFAULT 0"),
                ("reason_code", "TEXT NOT NULL DEFAULT ''"),
            ][..],
        )?;
        Self::ensure_columns_idempotent(
            tx,
            "daemon_sessions",
            &[("actor_kind", "TEXT NOT NULL DEFAULT 'AGENT_HMAC'")][..],
        )
    }

    fn apply_v15_columns(tx: &Connection) -> Result<(), HarnessError> {
        for (table, cols) in [
            (
                "presentation_decks",
                &[
                    ("aspect_ratio", "TEXT NOT NULL DEFAULT '16:9'"),
                    ("deck_protocol_version", "INTEGER NOT NULL DEFAULT 1"),
                    ("deck_mode", "TEXT NOT NULL DEFAULT 'multi-page'"),
                    ("revision", "INTEGER NOT NULL DEFAULT 0"),
                ][..],
            ),
            (
                "deck_slides",
                &[
                    ("slide_uid", "TEXT"),
                    ("speaker_notes", "TEXT NOT NULL DEFAULT ''"),
                    ("narrative_arc", "TEXT NOT NULL DEFAULT ''"),
                    ("eyebrow_tag", "TEXT NOT NULL DEFAULT ''"),
                    ("revision", "INTEGER NOT NULL DEFAULT 0"),
                ][..],
            ),
            (
                "wording_slots",
                &[
                    ("slot_kind", "TEXT NOT NULL DEFAULT 'text'"),
                    ("charts_json", "TEXT NOT NULL DEFAULT '[]'"),
                    ("safe_styles_json", "TEXT NOT NULL DEFAULT '{}'"),
                    ("updated_by", "TEXT NOT NULL DEFAULT 'human'"),
                    ("ast_revision", "INTEGER NOT NULL DEFAULT 0"),
                ][..],
            ),
            ("change_proposals", &[("source_comment_id", "TEXT")][..]),
        ] {
            Self::ensure_columns_idempotent(tx, table, cols)?;
        }
        Ok(())
    }

    fn apply_v16_columns(tx: &Connection) -> Result<(), HarnessError> {
        Self::ensure_columns_idempotent(tx, "chat_messages", &[("seq_no", "BIGINT")][..])
    }

    fn cached_genesis_db_bytes(parent_dir: &Path) -> Result<&'static [u8], HarnessError> {
        static GENESIS_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        static GENESIS_INIT_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        if let Some(cached) = GENESIS_BYTES.get() {
            return Ok(cached.as_slice());
        }
        let _init_guard = GENESIS_INIT_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .map_err(|_| HarnessError::ValidationFailed {
                reason: "GENESIS_LOCK_POISONED".to_string(),
            })?;
        if let Some(cached) = GENESIS_BYTES.get() {
            return Ok(cached.as_slice());
        }
        let built = (|| -> Result<Vec<u8>, HarnessError> {
            let mut mem_conn = Connection::open_in_memory()
                .map_err(|e| map_sqlite_error(e, "open_genesis_memory"))?;
            Self::apply_genesis_single_tx(&mut mem_conn)?;
            let tmp_path = parent_dir.join(format!(".genesis-tpl-{}.tmp", uuid::Uuid::new_v4()));
            let _ = fs::remove_file(&tmp_path);
            let target_str = tmp_path.to_string_lossy().into_owned();
            let vac_res = mem_conn.execute("VACUUM INTO ?1;", params![target_str]);
            let read_res = vac_res
                .map_err(|e| map_sqlite_error(e, "vacuum_genesis"))
                .and_then(|_| {
                    use std::io::Read;
                    let mut opts = OpenOptions::new();
                    opts.read(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        opts.custom_flags(libc::O_NOFOLLOW);
                    }
                    let mut f =
                        opts.open(&tmp_path)
                            .map_err(|e| HarnessError::ValidationFailed {
                                reason: format!("GENESIS_READ_FAILED: {e}"),
                            })?;
                    let mut buf = Vec::new();
                    f.read_to_end(&mut buf)
                        .map_err(|e| HarnessError::ValidationFailed {
                            reason: format!("GENESIS_READ_FAILED: {e}"),
                        })?;
                    Ok(buf)
                });
            let _ = fs::remove_file(&tmp_path);
            read_res
        })()?;
        let _ = GENESIS_BYTES.set(built);
        GENESIS_BYTES
            .get()
            .map(|v| v.as_slice())
            .ok_or_else(|| HarnessError::ValidationFailed {
                reason: "GENESIS_CACHE_INIT_FAILED".to_string(),
            })
    }

    fn apply_genesis_single_tx(conn: &mut Connection) -> Result<(), HarnessError> {
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = OFF;",
        )
        .map_err(|e| map_sqlite_error(e, "disable_fk_for_genesis"))?;

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(|e| map_sqlite_error(e, "begin_genesis_single_tx"))?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                checksum_sha256 TEXT NOT NULL,
                applied_at BIGINT NOT NULL
            );",
        )
        .map_err(|e| map_sqlite_error(e, "genesis_schema_migrations"))?;
        tx.execute_batch(V1_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v1"))?;
        tx.execute_batch(V2_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v2"))?;
        tx.execute_batch(
            "ALTER TABLE projects ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';",
        )
        .map_err(|e| map_sqlite_error(e, "genesis_v3_tenant_col"))?;
        tx.execute_batch(V3_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v3"))?;
        tx.execute_batch(
            "ALTER TABLE projects ADD COLUMN project_code TEXT;
             CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_code ON projects(project_code) WHERE project_code IS NOT NULL;
             ALTER TABLE projects ADD COLUMN trash_deleted_at BIGINT;
             ALTER TABLE projects ADD COLUMN provenance TEXT NOT NULL DEFAULT 'scratch';
             ALTER TABLE chat_sessions ADD COLUMN archived_at BIGINT;",
        )
        .map_err(|e| map_sqlite_error(e, "genesis_v4_cols"))?;
        tx.execute_batch(V4_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v4"))?;
        Self::apply_v5_columns(&tx)?;
        tx.execute_batch(V5_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v5"))?;
        Self::apply_v6_columns(&tx)?;
        tx.execute_batch(V6_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v6"))?;
        Self::apply_v7_columns(&tx)?;
        tx.execute_batch(V7_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v7"))?;
        tx.execute_batch(V8_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v8"))?;
        Self::apply_v9_columns(&tx)?;
        tx.execute_batch(V9_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v9"))?;
        Self::apply_v10_columns(&tx)?;
        tx.execute_batch(V10_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v10"))?;
        Self::apply_v11_columns(&tx)?;
        tx.execute_batch(V11_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v11"))?;
        Self::apply_v12_columns(&tx)?;
        tx.execute_batch(V12_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v12"))?;
        tx.execute_batch(V13_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v13"))?;
        Self::apply_v14_columns(&tx)?;
        tx.execute_batch(V14_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v14"))?;
        Self::apply_v15_columns(&tx)?;
        crate::presentation_repo::backfill_missing_slide_uids(&tx)?;
        tx.execute_batch(V15_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v15"))?;
        Self::apply_v16_columns(&tx)?;
        tx.execute_batch(V16_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v16"))?;
        tx.execute_batch(V17_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v17"))?;
        tx.execute_batch(V18_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v18"))?;
        tx.execute_batch(V19_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v19"))?;
        tx.execute_batch(V20_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v20"))?;
        tx.execute_batch(V21_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v21"))?;
        tx.execute_batch(V22_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v22"))?;
        tx.execute_batch(V23_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "genesis_v23"))?;

        let now = Utc::now().timestamp_millis();
        for (version, name, sha) in [
            (3i64, V3_MIGRATION_NAME, v3_migration_checksum()),
            (4i64, V4_MIGRATION_NAME, v4_migration_checksum()),
            (5i64, V5_MIGRATION_NAME, v5_migration_checksum()),
            (6i64, V6_MIGRATION_NAME, v6_migration_checksum()),
            (7i64, V7_MIGRATION_NAME, v7_migration_checksum()),
            (8i64, V8_MIGRATION_NAME, v8_migration_checksum()),
            (9i64, V9_MIGRATION_NAME, v9_migration_checksum()),
            (10i64, V10_MIGRATION_NAME, v10_migration_checksum()),
            (11i64, V11_MIGRATION_NAME, v11_migration_checksum()),
            (12i64, V12_MIGRATION_NAME, v12_migration_checksum()),
            (13i64, V13_MIGRATION_NAME, v13_migration_checksum()),
            (14i64, V14_MIGRATION_NAME, v14_migration_checksum()),
            (15i64, V15_MIGRATION_NAME, v15_migration_checksum()),
            (16i64, V16_MIGRATION_NAME, v16_migration_checksum()),
            (17i64, V17_MIGRATION_NAME, v17_migration_checksum()),
            (18i64, V18_MIGRATION_NAME, v18_migration_checksum()),
            (19i64, V19_MIGRATION_NAME, v19_migration_checksum()),
            (20i64, V20_MIGRATION_NAME, v20_migration_checksum()),
            (21i64, V21_MIGRATION_NAME, v21_migration_checksum()),
            (22i64, V22_MIGRATION_NAME, v22_migration_checksum()),
            (23i64, V23_MIGRATION_NAME, v23_migration_checksum()),
        ] {
            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![version, name, sha, now],
            )
            .map_err(|e| map_sqlite_error(e, "genesis_insert_migrations"))?;
        }
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_genesis_single_tx"))?;
        Ok(())
    }

    fn apply_v5_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v5_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 5",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v5_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V5]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v5"))?;
        Self::apply_v5_columns(&tx)?;
        tx.execute_batch(V5_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v5_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (5, ?1, ?2, ?3)",
            params![
                V5_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v5_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v5_migration"))?;

        Ok(())
    }

    fn apply_v6_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v6_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 6",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v6_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V6]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v6"))?;
        // Idempotent guards repairing V2->V4->V6 columns if upgraded from earlier schemas
        Self::apply_v6_columns(&tx)?;
        tx.execute_batch(V6_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v6_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (6, ?1, ?2, ?3)",
            params![
                V6_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v6_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v6_migration"))?;

        Ok(())
    }

    fn apply_v7_migration_if_needed(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v7_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 7",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v7_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V7]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let populated_rows: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM projects)
                      + (SELECT COUNT(*) FROM execution_events)
                      + (SELECT COUNT(*) FROM node_execution_jobs)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if populated_rows > 0 {
            Self::create_versioned_backup(conn, db_path, "v7")?;
        }

        // SQLite 12-Step Rule: PRAGMA foreign_keys = OFF & legacy_alter_table = OFF MUST run OUTSIDE transaction
        if !conn.is_autocommit() {
            return Err(HarnessError::ValidationFailed {
                reason: "V7_MIGRATION_PRECONDITION_FAILED: connection must be in autocommit mode"
                    .to_string(),
            });
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = OFF;",
        )
        .map_err(|e| map_sqlite_error(e, "disable_fk_for_v7_rebuild"))?;

        let fk_off: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_off_v7"))?;
        if fk_off != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: "V7_MIGRATION_PRECONDITION_FAILED: PRAGMA foreign_keys remained ON"
                    .to_string(),
            });
        }

        let rebuild_res = (|| -> Result<(), HarnessError> {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(|e| map_sqlite_error(e, "begin_exclusive_v7"))?;
            Self::apply_v7_columns(&tx)?;
            tx.execute_batch(V7_MIGRATION_SQL)
                .map_err(|e| map_sqlite_error(e, "execute_v7_sql"))?;

            let fk_violation_count: usize = {
                let mut stmt = tx
                    .prepare("PRAGMA foreign_key_check;")
                    .map_err(|e| map_sqlite_error(e, "prepare_fk_check_v7"))?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(|e| map_sqlite_error(e, "run_fk_check_v7"))?;
                rows.count()
            };
            if fk_violation_count > 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "V7_MIGRATION_FK_CHECK_FAILED: {fk_violation_count} foreign key violation(s)"
                    ),
                });
            }

            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (7, ?1, ?2, ?3)",
                params![
                    V7_MIGRATION_NAME,
                    expected_sha,
                    Utc::now().timestamp_millis()
                ],
            )
            .map_err(|e| map_sqlite_error(e, "insert_v7_schema_migration"))?;

            tx.commit()
                .map_err(|e| map_sqlite_error(e, "commit_v7_migration"))?;
            Ok(())
        })();

        let restore_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
        rebuild_res?;
        restore_res.map_err(|e| map_sqlite_error(e, "restore_fk_on_v7"))?;

        let fk_on: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_on_v7"))?;
        if fk_on != 1 {
            return Err(HarnessError::ValidationFailed {
                reason:
                    "V7_MIGRATION_POSTCONDITION_FAILED: PRAGMA foreign_keys did not restore to 1"
                        .to_string(),
            });
        }
        Ok(())
    }

    fn apply_v8_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v8_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 8",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v8_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V8]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v8"))?;
        tx.execute_batch(V8_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v8_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (8, ?1, ?2, ?3)",
            params![
                V8_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v8_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v8_migration"))?;
        Ok(())
    }

    fn reconcile_legacy_epic08_v9_to_v11_slots(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let slots = [
            (
                "V9__epic08_plugins_learn_cron.sql",
                12i64,
                V12_MIGRATION_NAME,
                v12_migration_checksum(),
            ),
            (
                "V10__epic08_audit_remediation.sql",
                13i64,
                V13_MIGRATION_NAME,
                v13_migration_checksum(),
            ),
            (
                "V11__epic08_party_remediation.sql",
                14i64,
                V14_MIGRATION_NAME,
                v14_migration_checksum(),
            ),
        ];

        let mut legacy_hits: Vec<(i64, &str, i64, &str)> = Vec::new();
        for (old_name, target_ver, new_name, expected_sha) in slots {
            let row: Option<(i64, String)> = conn
                .query_row(
                    "SELECT version, checksum_sha256 FROM schema_migrations WHERE name = ?1",
                    params![old_name],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(|e| map_sqlite_error(e, "read_legacy_epic08_migration_slot"))?;
            if let Some((old_ver, actual_sha)) = row {
                if actual_sha != expected_sha {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "MIGRATION_CHECKSUM_MISMATCH [{old_name}]: expected {expected_sha}, found {actual_sha}"
                        ),
                    });
                }
                legacy_hits.push((old_ver, old_name, target_ver, new_name));
            }
        }

        if legacy_hits.is_empty() {
            return Ok(());
        }

        for (_, _, target_ver, new_name) in &legacy_hits {
            let occupant: Option<String> = conn
                .query_row(
                    "SELECT name FROM schema_migrations WHERE version = ?1",
                    params![target_ver],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| map_sqlite_error(e, "check_target_migration_slot"))?;
            if let Some(occ_name) = occupant {
                if occ_name != *new_name {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "LEGACY_MIGRATION_REKEY_CONFLICT: version {target_ver} already occupied by {occ_name}"
                        ),
                    });
                }
            }
        }

        Self::create_versioned_backup(conn, db_path, "pre-epic08-rekey")?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_legacy_epic08_rekey"))?;
        for (old_ver, old_name, target_ver, new_name) in legacy_hits.iter().rev() {
            tx.execute(
                "UPDATE schema_migrations SET version = ?1, name = ?2 WHERE version = ?3 AND name = ?4",
                params![target_ver, new_name, old_ver, old_name],
            )
            .map_err(|e| map_sqlite_error(e, "update_legacy_epic08_slot"))?;
        }
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_legacy_epic08_rekey"))?;
        Ok(())
    }

    fn reconcile_legacy_epic06_v12_slot(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let legacy_name = "V12__epic06_presentation_cms_and_charts.sql";
        let row: Option<(i64, String)> = conn
            .query_row(
                "SELECT version, checksum_sha256 FROM schema_migrations WHERE name = ?1",
                params![legacy_name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_legacy_epic06_migration_slot"))?;
        let Some((old_ver, actual_sha)) = row else {
            return Ok(());
        };
        if actual_sha != LEGACY_EPIC06_V12_SHA256 {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "MIGRATION_CHECKSUM_MISMATCH [{legacy_name}]: expected {LEGACY_EPIC06_V12_SHA256}, found {actual_sha}"
                ),
            });
        }
        let occupant: Option<String> = conn
            .query_row(
                "SELECT name FROM schema_migrations WHERE version = 15",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "check_target_v15_migration_slot"))?;
        if let Some(occ_name) = occupant {
            if occ_name != V15_MIGRATION_NAME {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "LEGACY_MIGRATION_REKEY_CONFLICT: version 15 already occupied by {occ_name}"
                    ),
                });
            }
        }
        Self::create_versioned_backup(conn, db_path, "pre-epic06-rekey")?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_legacy_epic06_rekey"))?;
        tx.execute(
            "UPDATE schema_migrations SET version = 15, name = ?1, checksum_sha256 = ?2 WHERE version = ?3 AND name = ?4",
            params![V15_MIGRATION_NAME, v15_migration_checksum(), old_ver, legacy_name],
        )
        .map_err(|e| map_sqlite_error(e, "update_legacy_epic06_slot"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_legacy_epic06_rekey"))?;
        Ok(())
    }

    fn apply_v9_migration_if_needed(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v9_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 9",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v9_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V9]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        // Enforce pre-transaction PRAGMA foreign_keys = ON read-back = 1
        let pre_fk: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_pre_fk_on_v9"))?;
        if pre_fk != 1 {
            return Err(HarnessError::ValidationFailed {
                reason: "V9_MIGRATION_PRECONDITION_FAILED: PRAGMA foreign_keys must be ON before migration"
                    .to_string(),
            });
        }

        let populated_rows: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM projects)
                      + (SELECT COUNT(*) FROM flow_graphs)
                      + (SELECT COUNT(*) FROM flow_nodes)
                      + (SELECT COUNT(*) FROM flow_edges)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if populated_rows > 0 {
            Self::create_versioned_backup(conn, db_path, "v9")?;
        }

        let src_edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM flow_edges", [], |r| r.get(0))
            .unwrap_or(0);

        // SQLite 12-Step Rule: PRAGMA foreign_keys = OFF & legacy_alter_table = OFF MUST run OUTSIDE transaction
        if !conn.is_autocommit() {
            return Err(HarnessError::ValidationFailed {
                reason: "V9_MIGRATION_PRECONDITION_FAILED: connection must be in autocommit mode"
                    .to_string(),
            });
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = OFF;",
        )
        .map_err(|e| map_sqlite_error(e, "disable_fk_for_v9_rebuild"))?;

        let fk_off: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_off_v9"))?;
        if fk_off != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: "V9_MIGRATION_PRECONDITION_FAILED: PRAGMA foreign_keys remained ON"
                    .to_string(),
            });
        }

        let rebuild_res = (|| -> Result<(), HarnessError> {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(|e| map_sqlite_error(e, "begin_exclusive_v9"))?;
            Self::apply_v9_columns(&tx)?;

            if let Some((sql_part1, sql_part2)) =
                V9_MIGRATION_SQL.split_once("DROP TABLE flow_edges;")
            {
                tx.execute_batch(sql_part1)
                    .map_err(|e| map_sqlite_error(e, "execute_v9_sql_part1"))?;

                let mid_edge_count: i64 = tx
                    .query_row("SELECT COUNT(*) FROM flow_edges_v9", [], |r| r.get(0))
                    .map_err(|e| map_sqlite_error(e, "read_v9_mid_edge_count"))?;
                if src_edge_count != mid_edge_count {
                    return Err(HarnessError::ValidationFailed {
                        reason: format!(
                            "V9_MIGRATION_ROW_COUNT_MISMATCH: expected {src_edge_count}, found {mid_edge_count}"
                        ),
                    });
                }

                tx.execute_batch("DROP TABLE flow_edges;")
                    .map_err(|e| map_sqlite_error(e, "execute_v9_drop_flow_edges"))?;
                tx.execute_batch(sql_part2)
                    .map_err(|e| map_sqlite_error(e, "execute_v9_sql_part2"))?;
            } else {
                tx.execute_batch(V9_MIGRATION_SQL)
                    .map_err(|e| map_sqlite_error(e, "execute_v9_sql"))?;
            }

            let dst_edge_count: i64 = tx
                .query_row("SELECT COUNT(*) FROM flow_edges", [], |r| r.get(0))
                .map_err(|e| map_sqlite_error(e, "read_v9_post_swap_count"))?;
            if src_edge_count != dst_edge_count {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "V9_MIGRATION_ROW_COUNT_MISMATCH: expected {src_edge_count}, found {dst_edge_count}"
                    ),
                });
            }

            let fk_violation_count: usize = {
                let mut stmt = tx
                    .prepare("PRAGMA foreign_key_check;")
                    .map_err(|e| map_sqlite_error(e, "prepare_fk_check_v9"))?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(|e| map_sqlite_error(e, "run_fk_check_v9"))?;
                rows.count()
            };
            if fk_violation_count > 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "V9_MIGRATION_FK_CHECK_FAILED: {fk_violation_count} foreign key violation(s)"
                    ),
                });
            }

            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (9, ?1, ?2, ?3)",
                params![
                    V9_MIGRATION_NAME,
                    expected_sha,
                    Utc::now().timestamp_millis()
                ],
            )
            .map_err(|e| map_sqlite_error(e, "insert_v9_schema_migration"))?;

            tx.commit()
                .map_err(|e| map_sqlite_error(e, "commit_v9_migration"))?;
            Ok(())
        })();

        let restore_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
        rebuild_res?;
        restore_res.map_err(|e| map_sqlite_error(e, "restore_fk_on_v9"))?;

        let fk_on: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_on_v9"))?;
        if fk_on != 1 {
            return Err(HarnessError::ValidationFailed {
                reason:
                    "V9_MIGRATION_POSTCONDITION_FAILED: PRAGMA foreign_keys did not restore to 1"
                        .to_string(),
            });
        }
        Ok(())
    }

    fn apply_v10_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v10_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 10",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v10_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V10]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v10"))?;
        Self::apply_v10_columns(&tx)?;
        tx.execute_batch(V10_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v10_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (10, ?1, ?2, ?3)",
            params![
                V10_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v10_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v10_migration"))?;
        Ok(())
    }

    fn apply_v11_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v11_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 11",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v11_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V11]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v11"))?;
        Self::apply_v11_columns(&tx)?;
        tx.execute_batch(V11_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v11_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (11, ?1, ?2, ?3)",
            params![
                V11_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v11_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v11_migration"))?;
        Ok(())
    }

    fn apply_v12_migration_if_needed(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v12_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 12",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v12_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V12]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let populated_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM skill_plugins", [], |r| r.get(0))
            .unwrap_or(0);
        if populated_rows > 0 {
            Self::create_versioned_backup(conn, db_path, "v12")?;
        }

        if !conn.is_autocommit() {
            return Err(HarnessError::ValidationFailed {
                reason: "V12_MIGRATION_PRECONDITION_FAILED: connection must be in autocommit mode"
                    .to_string(),
            });
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = OFF;",
        )
        .map_err(|e| map_sqlite_error(e, "disable_fk_for_v12_rebuild"))?;

        let fk_off: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_off_v12"))?;
        if fk_off != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: "V12_MIGRATION_PRECONDITION_FAILED: PRAGMA foreign_keys remained ON"
                    .to_string(),
            });
        }

        let rebuild_res = (|| -> Result<(), HarnessError> {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(|e| map_sqlite_error(e, "begin_exclusive_v12"))?;
            Self::apply_v12_columns(&tx)?;
            tx.execute_batch(V12_MIGRATION_SQL)
                .map_err(|e| map_sqlite_error(e, "execute_v12_sql"))?;

            let fk_violation_count: usize = {
                let mut stmt = tx
                    .prepare("PRAGMA foreign_key_check;")
                    .map_err(|e| map_sqlite_error(e, "prepare_fk_check_v12"))?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(|e| map_sqlite_error(e, "run_fk_check_v12"))?;
                rows.count()
            };
            if fk_violation_count > 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "V12_MIGRATION_FK_CHECK_FAILED: {fk_violation_count} foreign key violation(s)"
                    ),
                });
            }

            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (12, ?1, ?2, ?3)",
                params![
                    V12_MIGRATION_NAME,
                    expected_sha,
                    Utc::now().timestamp_millis()
                ],
            )
            .map_err(|e| map_sqlite_error(e, "insert_v12_schema_migration"))?;

            tx.commit()
                .map_err(|e| map_sqlite_error(e, "commit_v12_migration"))?;
            Ok(())
        })();

        let restore_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
        rebuild_res?;
        restore_res.map_err(|e| map_sqlite_error(e, "restore_fk_on_v12"))?;

        let fk_on: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_on_v12"))?;
        if fk_on != 1 {
            return Err(HarnessError::ValidationFailed {
                reason:
                    "V12_MIGRATION_POSTCONDITION_FAILED: PRAGMA foreign_keys did not restore to 1"
                        .to_string(),
            });
        }
        Ok(())
    }

    fn apply_v13_migration_if_needed(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v13_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 13",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v13_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V13]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let populated_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM knowledge_cards", [], |r| r.get(0))
            .unwrap_or(0);
        if populated_rows > 0 {
            Self::create_versioned_backup(conn, db_path, "v13")?;
        }
        if !conn.is_autocommit() {
            return Err(HarnessError::ValidationFailed {
                reason: "V13_MIGRATION_PRECONDITION_FAILED: connection must be in autocommit mode"
                    .to_string(),
            });
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             PRAGMA legacy_alter_table = OFF;",
        )
        .map_err(|e| map_sqlite_error(e, "disable_fk_for_v13_rebuild"))?;
        let fk_off: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_off_v13"))?;
        if fk_off != 0 {
            return Err(HarnessError::ValidationFailed {
                reason: "V13_MIGRATION_PRECONDITION_FAILED: PRAGMA foreign_keys remained ON"
                    .to_string(),
            });
        }
        let tx_res = (|| -> Result<(), HarnessError> {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(|e| map_sqlite_error(e, "begin_exclusive_v13"))?;
            tx.execute_batch(V13_MIGRATION_SQL)
                .map_err(|e| map_sqlite_error(e, "execute_v13_sql"))?;
            let fk_violation_count: usize = {
                let mut stmt = tx
                    .prepare("PRAGMA foreign_key_check;")
                    .map_err(|e| map_sqlite_error(e, "prepare_fk_check_v13"))?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(|e| map_sqlite_error(e, "run_fk_check_v13"))?;
                rows.count()
            };
            if fk_violation_count > 0 {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "V13_MIGRATION_FK_CHECK_FAILED: {fk_violation_count} foreign key violation(s)"
                    ),
                });
            }
            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (13, ?1, ?2, ?3)",
                params![
                    V13_MIGRATION_NAME,
                    expected_sha,
                    Utc::now().timestamp_millis()
                ],
            )
            .map_err(|e| map_sqlite_error(e, "insert_v13_schema_migration"))?;
            tx.commit()
                .map_err(|e| map_sqlite_error(e, "commit_v13_migration"))?;
            Ok(())
        })();
        let restore_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
        tx_res?;
        restore_res.map_err(|e| map_sqlite_error(e, "restore_fk_on_v13"))?;
        let fk_on: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
            .map_err(|e| map_sqlite_error(e, "verify_fk_on_v13"))?;
        if fk_on != 1 {
            return Err(HarnessError::ValidationFailed {
                reason:
                    "V13_MIGRATION_POSTCONDITION_FAILED: PRAGMA foreign_keys did not restore to 1"
                        .to_string(),
            });
        }
        Ok(())
    }

    fn apply_v14_migration_if_needed(
        conn: &mut Connection,
        db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v14_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 14",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v14_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V14]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }
        let populated_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM knowledge_cards", [], |r| r.get(0))
            .unwrap_or(0);
        if populated_rows > 0 {
            Self::create_versioned_backup(conn, db_path, "v14")?;
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v14"))?;
        Self::apply_v14_columns(&tx)?;
        tx.execute_batch(V14_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v14_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (14, ?1, ?2, ?3)",
            params![
                V14_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v14_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v14_migration"))?;
        Ok(())
    }

    fn apply_v15_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v15_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 15",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v15_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V15]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            Self::apply_v15_columns(conn)?;
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v15"))?;
        Self::apply_v15_columns(&tx)?;
        crate::presentation_repo::backfill_missing_slide_uids(&tx)?;
        tx.execute_batch(V15_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v15_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (15, ?1, ?2, ?3)",
            params![
                V15_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v15_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v15_migration"))?;
        Ok(())
    }

    fn apply_v16_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v16_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 16",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v16_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V16]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            Self::apply_v16_columns(conn)?;
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v16"))?;
        Self::apply_v16_columns(&tx)?;
        tx.execute_batch(V16_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v16_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (16, ?1, ?2, ?3)",
            params![
                V16_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v16_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v16_migration"))?;
        Ok(())
    }

    fn apply_v17_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v17_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 17",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v17_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V17]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v17"))?;
        tx.execute_batch(V17_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v17_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (17, ?1, ?2, ?3)",
            params![
                V17_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v17_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v17_migration"))?;
        Ok(())
    }

    fn apply_v18_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v18_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 18",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v18_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V18]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v18"))?;
        tx.execute_batch(V18_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v18_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (18, ?1, ?2, ?3)",
            params![
                V18_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v18_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v18_migration"))?;
        Ok(())
    }

    fn apply_v19_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v19_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 19",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v19_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V19]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v19"))?;
        tx.execute_batch(V19_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v19_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (19, ?1, ?2, ?3)",
            params![
                V19_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v19_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v19_migration"))?;
        Ok(())
    }

    fn apply_v20_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v20_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 20",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v20_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V20]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v20"))?;
        tx.execute_batch(V20_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v20_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (20, ?1, ?2, ?3)",
            params![
                V20_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v20_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v20_migration"))?;
        Ok(())
    }

    fn apply_v21_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v21_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 21",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v21_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V21]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v21"))?;
        tx.execute_batch(V21_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v21_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (21, ?1, ?2, ?3)",
            params![
                V21_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v21_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v21_migration"))?;
        Ok(())
    }

    fn apply_v22_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v22_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 22",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v22_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V22]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v22"))?;
        tx.execute_batch(V22_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v22_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (22, ?1, ?2, ?3)",
            params![
                V22_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v22_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v22_migration"))?;
        Ok(())
    }

    fn apply_v23_migration_if_needed(
        conn: &mut Connection,
        _db_path: &Path,
    ) -> Result<(), HarnessError> {
        let expected_sha = v23_migration_checksum();
        let recorded_sha: Option<String> = conn
            .query_row(
                "SELECT checksum_sha256 FROM schema_migrations WHERE version = 23",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "read_v23_migration_ledger"))?;

        if let Some(actual_sha) = recorded_sha {
            if actual_sha != expected_sha {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "MIGRATION_CHECKSUM_MISMATCH [V23]: expected {expected_sha}, found {actual_sha}"
                    ),
                });
            }
            return Ok(());
        }

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| map_sqlite_error(e, "begin_immediate_v23"))?;
        tx.execute_batch(V23_MIGRATION_SQL)
            .map_err(|e| map_sqlite_error(e, "execute_v23_sql"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (23, ?1, ?2, ?3)",
            params![
                V23_MIGRATION_NAME,
                expected_sha,
                Utc::now().timestamp_millis()
            ],
        )
        .map_err(|e| map_sqlite_error(e, "insert_v23_schema_migration"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_v23_migration"))?;
        Ok(())
    }

    fn compute_screen_artifacts_digest(conn: &Connection) -> Result<(usize, String), HarnessError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, node_id, content_hash, artifact_path, anchor_status, created_at
                 FROM screen_artifacts ORDER BY id ASC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_screen_artifacts_digest"))?;
        let mut hasher = Sha256::new();
        let mut count = 0usize;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(|e| map_sqlite_error(e, "query_screen_artifacts_digest"))?;
        for row in rows {
            let (id, node_id, hash, path, status, created_at) =
                row.map_err(|e| map_sqlite_error(e, "read_screen_artifact_row"))?;
            hasher
                .update(format!("{id}|{node_id}|{hash}|{path}|{status}|{created_at}\n").as_bytes());
            count += 1;
        }
        Ok((count, format!("{:x}", hasher.finalize())))
    }

    fn create_pre_v3_backup(conn: &Connection, db_path: &Path) -> Result<(), HarnessError> {
        Self::create_versioned_backup(conn, db_path, "v3")
    }

    fn create_versioned_backup(
        conn: &Connection,
        db_path: &Path,
        version_tag: &str,
    ) -> Result<(), HarnessError> {
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        let ts = Utc::now().timestamp_millis();
        let bak_path = PathBuf::from(format!("{}.pre-{version_tag}.{ts}.bak", db_path.display()));
        let bak_str = bak_path.to_string_lossy().into_owned();
        conn.execute("VACUUM INTO ?1;", params![bak_str])
            .or_else(|_| fs::copy(db_path, &bak_path).map(|_| 0))
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!(
                    "{}_BACKUP_FAILED [{}]: {e}",
                    version_tag.to_uppercase(),
                    bak_path.display()
                ),
            })?;
        enforce_unix_mode(&bak_path, 0o600)?;

        // Retain at most the 2 newest .pre-<tag>.*.bak files
        if let (Some(parent), Some(stem)) = (
            db_path.parent(),
            db_path.file_name().and_then(|s| s.to_str()),
        ) {
            let prefix = format!("{stem}.pre-{version_tag}.");
            if let Ok(entries) = fs::read_dir(parent) {
                let mut backups: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with(&prefix) && n.ends_with(".bak"))
                            .unwrap_or(false)
                    })
                    .collect();
                backups.sort();
                if backups.len() > 2 {
                    let remove_count = backups.len() - 2;
                    for old in backups.into_iter().take(remove_count) {
                        let _ = fs::remove_file(old);
                    }
                }
            }
        }
        Ok(())
    }

    /// Acquires a freshly configured SQLite connection with all 4 mandatory PRAGMAs applied.
    pub fn connection(&self) -> Result<Connection, HarnessError> {
        Self::connect_with_pragmas(&self.config)
    }

    pub fn config(&self) -> &SqliteWalConfig {
        &self.config
    }

    pub fn workspace_root(&self) -> &Path {
        &self.config.workspace_root
    }

    pub fn db_path(&self) -> &Path {
        &self.config.db_path
    }

    pub fn migration_elapsed_ms(&self) -> u128 {
        self.migration_elapsed_ms
    }

    #[allow(clippy::suspicious_open_options)]
    fn connect_with_pragmas(config: &SqliteWalConfig) -> Result<Connection, HarnessError> {
        Self::connect_with_pragmas_opts(config, true)
    }

    #[allow(clippy::suspicious_open_options)]
    fn connect_with_pragmas_opts(
        config: &SqliteWalConfig,
        enable_wal: bool,
    ) -> Result<Connection, HarnessError> {
        #[cfg(unix)]
        let conn = {
            use std::os::unix::fs::OpenOptionsExt;
            let _f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&config.db_path)
                .map_err(|e| {
                    if e.raw_os_error() == Some(libc::ELOOP) {
                        HarnessError::PathTraversalDenied {
                            path: config.db_path.display().to_string(),
                        }
                    } else {
                        map_sqlite_error(
                            rusqlite::Error::SqliteFailure(
                                rusqlite::ffi::Error::new(libc::EIO),
                                Some(e.to_string()),
                            ),
                            "open_connection_nofollow",
                        )
                    }
                })?;
            Connection::open(&config.db_path).map_err(|e| map_sqlite_error(e, "open_connection"))?
        };
        #[cfg(not(unix))]
        let conn = Connection::open(&config.db_path)
            .map_err(|e| map_sqlite_error(e, "open_connection"))?;

        conn.busy_timeout(Duration::from_millis(u64::from(config.busy_timeout_ms)))
            .map_err(|e| map_sqlite_error(e, "set_busy_timeout"))?;
        let pragma_sql = if enable_wal {
            format!(
                "PRAGMA busy_timeout = {};
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;",
                config.busy_timeout_ms
            )
        } else {
            format!(
                "PRAGMA busy_timeout = {};
                 PRAGMA synchronous = OFF;
                 PRAGMA journal_mode = WAL;
                 PRAGMA temp_store = MEMORY;
                 PRAGMA cache_size = -16000;
                 PRAGMA foreign_keys = ON;",
                config.busy_timeout_ms
            )
        };
        retry_on_busy("apply_pragmas", || conn.execute_batch(&pragma_sql))?;
        Ok(conn)
    }
}
