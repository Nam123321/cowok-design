// @story 04.2
// @implements AC-01, AC-04
use rusqlite::{params, Connection};
use spec_registry::pool::{
    v3_migration_checksum, v4_migration_checksum, v5_migration_checksum, v6_migration_checksum,
    v7_migration_checksum, SqlitePool, SqliteWalConfig, V1_MIGRATION_SQL, V2_MIGRATION_SQL,
    V3_MIGRATION_SQL, V4_MIGRATION_SQL, V5_MIGRATION_SQL, V6_MIGRATION_SQL,
};
use tempfile::tempdir;

#[test]
fn v7_migration_upgrades_populated_v6_preserves_fks_and_backfills_project_seq() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir .cowok-design");
    let db_path = cowok_dir.join("cowok-design.sqlite3");

    // Seed a real populated V1..V6 database without V7
    {
        let conn = Connection::open(&db_path).expect("open raw v6 db");
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
        .expect("projects tenant_id");
        conn.execute_batch(V3_MIGRATION_SQL).expect("v3");
        conn.execute_batch(
            "ALTER TABLE projects ADD COLUMN project_code TEXT;
             ALTER TABLE projects ADD COLUMN trash_deleted_at BIGINT;
             ALTER TABLE projects ADD COLUMN provenance TEXT NOT NULL DEFAULT 'scratch';",
        )
        .expect("v4 projects cols");
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

        for (ver, name, sha) in [
            (3i64, "V3", v3_migration_checksum()),
            (4, "V4", v4_migration_checksum()),
            (5, "V5", v5_migration_checksum()),
            (6, "V6", v6_migration_checksum()),
        ] {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (?1, ?2, ?3, 1000)",
                params![ver, name, sha],
            )
            .expect("seed ledger");
        }

        // Insert project with non-default tenant_id, session, message, scope, job, and execution_events
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES ('proj-1', 'tenant-alpha', 'alpha-app', 'Alpha', '/tmp/alpha', '', 'ACTIVE', 1000, 1000)",
            [],
        )
        .expect("insert project");
        conn.execute(
            "INSERT INTO chat_sessions (id, tenant_id, project_id, title, created_at, updated_at)
             VALUES ('sess-1', 'tenant-alpha', 'proj-1', 'Main', 1000, 1000)",
            [],
        )
        .expect("insert chat_session");
        conn.execute(
            "INSERT INTO chat_messages (id, tenant_id, session_id, role, prose_markdown, created_at)
             VALUES ('msg-1', 'tenant-alpha', 'sess-1', 'user', 'hi', 1000)",
            [],
        )
        .expect("insert chat_message");
        conn.execute(
            "INSERT INTO chat_message_node_scopes (id, message_id, project_id, session_id, node_id, composite_scope_key, created_at)
             VALUES ('scope-1', 'msg-1', 'proj-1', 'sess-1', 'node-1', 'proj-1::sess-1::node-1', 1000)",
            [],
        )
        .expect("insert scope");
        conn.execute(
            "INSERT INTO node_execution_jobs (id, tenant_id, project_id, session_id, node_id, status, fencing_token, priority, created_at)
             VALUES ('job-1', 'tenant-alpha', 'proj-1', 'sess-1', 'node-1', 'QUEUED', 1, 10, 1000)",
            [],
        )
        .expect("insert job");
        conn.execute_batch("ALTER TABLE execution_events ADD COLUMN project_seq INTEGER;")
            .expect("add partial project_seq");
        conn.execute(
            "INSERT INTO execution_events (id, tenant_id, project_id, job_id, node_id, event_kind, payload_text, seq_no, project_seq, created_at)
             VALUES
               ('ev-1', 'tenant-alpha', 'proj-1', 'job-1', 'node-1', 'log', 'a', 1, 5, 1001),
               ('ev-2', 'tenant-alpha', 'proj-1', 'job-1', 'node-1', 'log', 'b', 2, NULL, 1002),
               ('ev-3', 'tenant-alpha', 'proj-1', 'job-1', 'node-1', 'log', 'c', 3, NULL, 1003)",
            [],
        )
        .expect("insert partial-null execution_events");
    }

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect("open_wal should upgrade V6->V7");
    let conn = pool.connection().expect("conn");

    let fk_on: i64 = conn
        .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
        .expect("pragma fk");
    assert_eq!(fk_on, 1, "PRAGMA foreign_keys must be restored to 1");

    let v7_sha: String = conn
        .query_row(
            "SELECT checksum_sha256 FROM schema_migrations WHERE version = 7",
            [],
            |r| r.get(0),
        )
        .expect("v7 ledger row");
    assert_eq!(v7_sha, v7_migration_checksum());

    // Pre-v7 backup file created for populated DB
    let has_backup = std::fs::read_dir(&cowok_dir)
        .expect("readdir")
        .filter_map(Result::ok)
        .any(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.contains(".pre-v7.") && n.ends_with(".bak"))
                .unwrap_or(false)
        });
    assert!(
        has_backup,
        "Populated V6->V7 upgrade must write .pre-v7.*.bak"
    );

    // Scope tenant_id backfilled from project
    let scope_tenant: String = conn
        .query_row(
            "SELECT tenant_id FROM chat_message_node_scopes WHERE id = 'scope-1'",
            [],
            |r| r.get(0),
        )
        .expect("scope tenant");
    assert_eq!(scope_tenant, "tenant-alpha");

    // Partial-null project_seq backfilled monotonically above existing MAX=5
    let seqs: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare("SELECT id, project_seq FROM execution_events ORDER BY created_at ASC")
            .expect("prepare seq");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query seq")
            .filter_map(Result::ok)
            .collect()
    };
    assert_eq!(
        seqs,
        vec![
            ("ev-1".to_string(), 5),
            ("ev-2".to_string(), 6),
            ("ev-3".to_string(), 7),
        ]
    );

    // node_execution_jobs 12-step rebuild supports PAUSED & INTERRUPTED and preserves execution_events FKs
    let job_scope: String = conn
        .query_row(
            "SELECT composite_scope_key FROM node_execution_jobs WHERE id = 'job-1'",
            [],
            |r| r.get(0),
        )
        .expect("job composite_scope_key");
    assert_eq!(job_scope, "proj-1::sess-1::node-1");

    conn.execute(
        "UPDATE node_execution_jobs SET status = 'PAUSED' WHERE id = 'job-1'",
        [],
    )
    .expect("PAUSED status allowed");
    conn.execute(
        "UPDATE node_execution_jobs SET status = 'INTERRUPTED' WHERE id = 'job-1'",
        [],
    )
    .expect("INTERRUPTED status allowed");
}

#[test]
fn v7_partial_unique_indexes_allow_slug_and_workspace_reuse_after_soft_delete() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let conn = pool.connection().expect("conn");

    let shared_root = dir.path().join("reuse-root");
    let shared_root_str = shared_root.display().to_string();

    conn.execute(
        "INSERT INTO projects (id, slug, name, workspace_root, status, trash_deleted_at, provenance, created_at, updated_at)
         VALUES ('proj-trashed', 'reuse-slug', 'Trashed', ?1, 'ARCHIVED', 1700000000000, 'linked_local_git', 1000, 1000)",
        params![shared_root_str],
    )
    .expect("insert trashed project");

    // Creating a second active project with the SAME slug and workspace_root must succeed
    conn.execute(
        "INSERT INTO projects (id, slug, name, workspace_root, status, trash_deleted_at, provenance, created_at, updated_at)
         VALUES ('proj-active-1', 'reuse-slug', 'Active 1', ?1, 'ACTIVE', NULL, 'linked_local_git', 2000, 2000)",
        params![shared_root_str],
    )
    .expect("insert active project reusing trashed slug and root");

    // Creating a third active project with the SAME active slug must still fail unique constraint
    let duplicate_active = conn.execute(
        "INSERT INTO projects (id, slug, name, workspace_root, status, trash_deleted_at, provenance, created_at, updated_at)
         VALUES ('proj-active-2', 'reuse-slug', 'Active 2', '/tmp/other-root', 'ACTIVE', NULL, 'scratch', 3000, 3000)",
        [],
    );
    assert!(
        duplicate_active.is_err(),
        "Two active non-trashed projects with identical slug must fail UNIQUE constraint"
    );
}
