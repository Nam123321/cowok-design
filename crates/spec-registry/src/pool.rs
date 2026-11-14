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
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let mut f = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&config.db_path)
                    .map_err(|e| HarnessError::ValidationFailed {
                        reason: format!("GENESIS_WRITE_FAILED: {e}"),
                    })?;
                f.write_all(db_bytes)
                    .map_err(|e| HarnessError::ValidationFailed {
                        reason: format!("GENESIS_WRITE_FAILED: {e}"),
                    })?;
            }
            #[cfg(not(unix))]
            {
                fs::write(&config.db_path, db_bytes).map_err(|e| {
                    HarnessError::ValidationFailed {
                        reason: format!("GENESIS_WRITE_FAILED: {e}"),
                    }
                })?;
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

    fn cached_genesis_db_bytes(parent_dir: &Path) -> Result<&'static [u8], HarnessError> {
        static GENESIS_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
        let res = GENESIS_BYTES.get_or_init(|| {
            let mut mem_conn =
                Connection::open_in_memory().map_err(|e| format!("open_genesis_memory: {e}"))?;
            Self::apply_genesis_single_tx(&mut mem_conn).map_err(|e| format!("{e:?}"))?;
            let tmp_path = parent_dir.join(format!(".genesis-tpl-{}.tmp", uuid::Uuid::new_v4()));
            let _ = fs::remove_file(&tmp_path);
            let target_str = tmp_path.to_string_lossy().into_owned();
            let vac_res = mem_conn.execute("VACUUM INTO ?1;", params![target_str]);
            let bytes = vac_res
                .map_err(|e| format!("vacuum_genesis: {e}"))
                .and_then(|_| fs::read(&tmp_path).map_err(|e| format!("read_genesis: {e}")));
            let _ = fs::remove_file(&tmp_path);
            bytes
        });
        res.as_deref()
            .map_err(|reason| HarnessError::ValidationFailed {
                reason: reason.clone(),
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

        let now = Utc::now().timestamp_millis();
        for (version, name, sha) in [
            (3i64, V3_MIGRATION_NAME, v3_migration_checksum()),
            (4i64, V4_MIGRATION_NAME, v4_migration_checksum()),
            (5i64, V5_MIGRATION_NAME, v5_migration_checksum()),
            (6i64, V6_MIGRATION_NAME, v6_migration_checksum()),
            (7i64, V7_MIGRATION_NAME, v7_migration_checksum()),
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
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
        let ts = Utc::now().timestamp_millis();
        let bak_path = PathBuf::from(format!("{}.pre-{version_tag}.{ts}.bak", db_path.display()));
        fs::copy(db_path, &bak_path).map_err(|e| HarnessError::ValidationFailed {
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
            let _ = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .mode(0o600)
                .open(&config.db_path);
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
