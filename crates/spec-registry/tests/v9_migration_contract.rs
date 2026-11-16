// @story 05.1
// @implements AC-01, AC-03, AC-06
use rusqlite::{params, Connection};
use spec_registry::pool::{
    v3_migration_checksum, v4_migration_checksum, v5_migration_checksum, v6_migration_checksum,
    v7_migration_checksum, v8_migration_checksum, v9_migration_checksum, SqlitePool,
    SqliteWalConfig, V1_MIGRATION_SQL, V2_MIGRATION_SQL, V3_MIGRATION_SQL, V4_MIGRATION_SQL,
    V5_MIGRATION_SQL, V6_MIGRATION_SQL, V7_MIGRATION_SQL, V8_MIGRATION_SQL,
};
use tempfile::tempdir;

#[test]
fn v9_migration_upgrades_populated_v8_preserves_edges_and_records_checksum() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir .cowok-design");
    let db_path = cowok_dir.join("cowok-design.sqlite3");

    // Seed a real V1..V8 database with flow nodes and edges without V9
    {
        let conn = Connection::open(&db_path).expect("open raw v8 db");
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
        conn.execute_batch(V8_MIGRATION_SQL).expect("v8");

        for (ver, name, sha) in [
            (3i64, "V3", v3_migration_checksum()),
            (4, "V4", v4_migration_checksum()),
            (5, "V5", v5_migration_checksum()),
            (6, "V6", v6_migration_checksum()),
            (7, "V7", v7_migration_checksum()),
            (8, "V8", v8_migration_checksum()),
        ] {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (?1, ?2, ?3, 1000)",
                params![ver, name, sha],
            )
            .expect("seed ledger");
        }

        // Insert project, graph, 2 nodes, 1 edge
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES ('proj-v9', 'tenant-alpha', 'alpha-app', 'Alpha', '/tmp/alpha', '', 'ACTIVE', 1000, 1000)",
            [],
        )
        .expect("insert project");

        conn.execute(
            "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, updated_at)
             VALUES ('graph-v9', 'tenant-alpha', 'proj-v9', 'default', 'Main Flow', 1, 1, 1000)",
            [],
        )
        .expect("insert graph");

        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, created_at, updated_at)
             VALUES ('node-1', 'graph-v9', 'tenant-alpha', 'proj-v9', 'default', 'home', 'Home', 'screen', '/', 1000, 1000)",
            [],
        )
        .expect("insert node-1");

        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, created_at, updated_at)
             VALUES ('node-2', 'graph-v9', 'tenant-alpha', 'proj-v9', 'default', 'settings', 'Settings', 'screen', '/settings', 1000, 1000)",
            [],
        )
        .expect("insert node-2");

        conn.execute(
            "INSERT INTO flow_edges (id, graph_id, tenant_id, project_id, source_node_id, target_node_id, flow_type, trigger_event, version, created_at, updated_at)
             VALUES ('edge-1', 'graph-v9', 'tenant-alpha', 'proj-v9', 'node-1', 'node-2', 'main', 'onClick', 1, 1000, 1000)",
            [],
        )
        .expect("insert edge-1");

        // Verify initial edge count is 1
        let pre_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM flow_edges", [], |r| r.get(0))
            .expect("pre count");
        assert_eq!(pre_count, 1);
    }

    // Open WAL: this triggers apply_v9_migration_if_needed
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect("open_wal should upgrade V8->V9");
    let conn = pool.connection().expect("conn");

    // 1. Verify V9 migration checksum recorded
    let v9_sha: String = conn
        .query_row(
            "SELECT checksum_sha256 FROM schema_migrations WHERE version = 9",
            [],
            |r| r.get(0),
        )
        .expect("v9 row");
    assert_eq!(v9_sha, v9_migration_checksum());

    // 2. Verify row-count preservation across 12-step rebuild
    let post_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM flow_edges", [], |r| r.get(0))
        .expect("post count");
    assert_eq!(post_count, 1);

    // 3. Verify V9 default columns on flow_edges
    let (max_iter, rev): (i64, i64) = conn
        .query_row(
            "SELECT max_loop_iterations, revision FROM flow_edges WHERE id = 'edge-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query edge-1 v9 columns");
    assert_eq!(max_iter, 3);
    assert_eq!(rev, 1);

    // 4. Verify idempotency_records table exists and allows insertion
    conn.execute(
        "INSERT INTO idempotency_records (
            tenant_id, actor_id, idempotency_key, request_sha256, status, lease_expires_at, response_status, response_body_json, expires_at
         ) VALUES ('tenant-alpha', 'user-1', 'key-123', 'sha-abc', 'COMPLETED', 2000, 201, '{\"ok\":true}', 5000)",
        [],
    )
    .expect("insert idempotency record");

    let rec_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM idempotency_records", [], |r| r.get(0))
        .expect("idempotency count");
    assert_eq!(rec_count, 1);

    // 5. Verify PRAGMA foreign_keys is ON after migration
    let fk_status: i64 = conn
        .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
        .expect("fk status");
    assert_eq!(fk_status, 1);
}
