// @story 04.1, 04.2
// @implements AC-01, AC-06
use rusqlite::{params, Connection};
use spec_registry::pool::{
    v3_migration_checksum, v4_migration_checksum, v5_migration_checksum, v6_migration_checksum,
    v7_migration_checksum, v8_migration_checksum, SqlitePool, SqliteWalConfig, V1_MIGRATION_SQL,
    V2_MIGRATION_SQL, V3_MIGRATION_SQL, V4_MIGRATION_SQL, V5_MIGRATION_SQL, V6_MIGRATION_SQL,
    V7_MIGRATION_SQL,
};
use tempfile::tempdir;

#[test]
fn v8_migration_upgrades_populated_v7_and_records_checksum() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir .cowok-design");
    let db_path = cowok_dir.join("cowok-design.sqlite3");

    // Seed a real V1..V7 database without V8
    {
        let conn = Connection::open(&db_path).expect("open raw v7 db");
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = OFF;
             CREATE TABLE IF NOT EXISTS schema_migrations (
                 version INTEGER PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL,
                 checksum_sha256 TEXT NOT NULL,
                 applied_at BIGINT NOT NULL
             );",
        )
        .expect("pragmas + ledger");

        conn.execute_batch(V1_MIGRATION_SQL).expect("v1");
        conn.execute_batch(V2_MIGRATION_SQL).expect("v2");
        conn.execute_batch(
            "ALTER TABLE projects ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';",
        )
        .expect("v3 col");
        conn.execute_batch(V3_MIGRATION_SQL).expect("v3");
        conn.execute_batch(
            "ALTER TABLE projects ADD COLUMN project_code TEXT;
             ALTER TABLE projects ADD COLUMN trash_deleted_at BIGINT;
             ALTER TABLE projects ADD COLUMN provenance TEXT NOT NULL DEFAULT 'scratch';",
        )
        .expect("v4 cols");
        conn.execute_batch(V4_MIGRATION_SQL).expect("v4");
        conn.execute_batch(V5_MIGRATION_SQL).expect("v5");
        conn.execute_batch(
            "ALTER TABLE chat_sessions ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
             ALTER TABLE chat_sessions ADD COLUMN archived_at BIGINT;
             ALTER TABLE chat_messages ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
             ALTER TABLE node_execution_jobs ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
             ALTER TABLE chat_artifact_snapshots ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';",
        )
        .expect("v6 cols");
        conn.execute_batch(V6_MIGRATION_SQL).expect("v6");
        conn.execute_batch(
            "ALTER TABLE chat_message_node_scopes ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
             ALTER TABLE element_comments ADD COLUMN artifact_sha256 TEXT NOT NULL DEFAULT '';
             ALTER TABLE element_comments ADD COLUMN anchor_kind TEXT NOT NULL DEFAULT 'html_element';
             ALTER TABLE execution_events ADD COLUMN project_seq INTEGER;
             ALTER TABLE execution_events ADD COLUMN composite_scope_key TEXT NOT NULL DEFAULT '';
             ALTER TABLE agent_settings ADD COLUMN permission_mode TEXT NOT NULL DEFAULT 'ask-every-time';
             ALTER TABLE agent_settings ADD COLUMN vault_mode TEXT NOT NULL DEFAULT 'keychain';
             ALTER TABLE agent_settings ADD COLUMN onboarding_done INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE node_chat_sessions ADD COLUMN composite_scope_key TEXT NOT NULL DEFAULT '';
             ALTER TABLE node_chat_sessions ADD COLUMN unread_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE node_chat_sessions ADD COLUMN last_message_id TEXT;",
        )
        .expect("v7 cols");
        conn.execute_batch(V7_MIGRATION_SQL).expect("v7");

        for (ver, name, sha) in [
            (3i64, "V3", v3_migration_checksum()),
            (4, "V4", v4_migration_checksum()),
            (5, "V5", v5_migration_checksum()),
            (6, "V6", v6_migration_checksum()),
            (7, "V7", v7_migration_checksum()),
        ] {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (?1, ?2, ?3, 1000)",
                params![ver, name, sha],
            )
            .expect("seed ledger");
        }
    }

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect("open_wal should upgrade V7->V8");
    let conn = pool.connection().expect("conn");
    let v8_sha: String = conn
        .query_row(
            "SELECT checksum_sha256 FROM schema_migrations WHERE version = 8",
            [],
            |r| r.get(0),
        )
        .expect("v8 row");
    assert_eq!(v8_sha, v8_migration_checksum());

    for table in [
        "daemon_sessions",
        "daemon_replay_nonces",
        "workspace_sync_intents",
    ] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                params![table],
                |r| r.get(0),
            )
            .expect("table exists");
        assert_eq!(count, 1, "expected table {table} from V8");
    }
}
