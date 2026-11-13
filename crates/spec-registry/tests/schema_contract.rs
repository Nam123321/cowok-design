// @story 01.2
// @cover AC-01, AC-02, AC-03, AC-04, AC-05
use rusqlite::Connection;
use spec_registry::{SqlitePool, SqliteWalConfig};
use std::collections::BTreeSet;
use tempfile::tempdir;

const EXPECTED_16_TABLES: &[&str] = &[
    "projects",
    "project_specs",
    "spec_revisions",
    "constraint_profiles",
    "constraint_rules",
    "flow_graphs",
    "flow_nodes",
    "flow_edges",
    "screen_artifacts",
    "presentation_decks",
    "deck_slides",
    "wording_slots",
    "skill_plugins",
    "model_tournaments",
    "bot_jobs",
    "gate_reports",
];

const EXPECTED_V2_TABLES: &[&str] = &[
    "spec_tombstones",
    "project_boundaries",
    "chat_sessions",
    "change_proposals",
    "version_snapshots",
    "project_workspace_files",
    "github_pull_requests",
    "recovery_commits",
];

fn get_table_columns(conn: &Connection, table: &str) -> BTreeSet<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("PRAGMA table_info failed");
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query_map failed")
        .map(|r| r.expect("col name"))
        .collect();
    cols
}

fn get_foreign_keys(conn: &Connection, table: &str) -> Vec<(String, String, String)> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA foreign_key_list({table})"))
        .expect("PRAGMA foreign_key_list failed");
    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(2)?, // target table
            row.get::<_, String>(3)?, // from col
            row.get::<_, String>(6)?, // on_delete
        ))
    })
    .expect("fk query failed")
    .map(|r| r.expect("fk row"))
    .collect()
}

#[test]
fn schema_01_to_16_verifies_wal_pragmas_and_exact_16_table_ledger() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("cowok-design.db");
    let config = SqliteWalConfig::new(&db_path, dir.path());

    let pool = SqlitePool::open_wal(config).expect("open_wal must succeed");
    assert!(
        pool.migration_elapsed_ms() < 500,
        "Cold-start migration should be fast, got {}ms",
        pool.migration_elapsed_ms()
    );

    let conn = pool.connection().expect("connection");

    // Verify all 4 mandatory PRAGMAs (AC-01)
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .expect("journal_mode");
    assert_eq!(journal_mode.to_lowercase(), "wal");

    let synchronous: i64 = conn
        .query_row("PRAGMA synchronous;", [], |r| r.get(0))
        .expect("synchronous");
    assert_eq!(synchronous, 1, "PRAGMA synchronous = NORMAL must equal 1");

    let busy_timeout: i64 = conn
        .query_row("PRAGMA busy_timeout;", [], |r| r.get(0))
        .expect("busy_timeout");
    assert_eq!(busy_timeout, 5000);

    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
        .expect("foreign_keys");
    assert_eq!(foreign_keys, 1);

    // Verify all 16 business tables + spec_write_intents infrastructure table exist
    for &table in EXPECTED_16_TABLES {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .expect("table count");
        assert_eq!(count, 1, "Missing mandatory table: {table}");
    }

    // Verify exact columns for projects, project_specs, spec_revisions
    let proj_cols = get_table_columns(&conn, "projects");
    for col in [
        "id",
        "slug",
        "name",
        "workspace_root",
        "product_brief",
        "inherit_brand_from_project_id",
        "status",
        "created_at",
        "updated_at",
    ] {
        assert!(proj_cols.contains(col), "projects missing column {col}");
    }

    let spec_cols = get_table_columns(&conn, "project_specs");
    for col in [
        "id",
        "project_id",
        "spec_key",
        "file_path",
        "status",
        "version",
        "content_sha256",
        "summary_digest",
        "provenance_summary_json",
        "source_project_id",
        "source_revision_sha256",
        "created_at",
        "updated_at",
    ] {
        assert!(
            spec_cols.contains(col),
            "project_specs missing column {col}"
        );
    }

    let rev_cols = get_table_columns(&conn, "spec_revisions");
    for col in [
        "id",
        "spec_id",
        "project_id",
        "spec_key",
        "revision_number",
        "operation_id",
        "previous_sha256",
        "content_hash",
        "actor_kind",
        "actor_id",
        "change_summary",
        "source_project_id",
        "source_revision_sha256",
        "created_at",
    ] {
        assert!(
            rev_cols.contains(col),
            "spec_revisions missing column {col}"
        );
    }

    // Verify foreign keys and ON DELETE behavior
    let proj_fks = get_foreign_keys(&conn, "projects");
    assert!(proj_fks
        .iter()
        .any(|(target, from, on_del)| target == "projects"
            && from == "inherit_brand_from_project_id"
            && on_del == "SET NULL"));

    let spec_fks = get_foreign_keys(&conn, "project_specs");
    assert!(spec_fks
        .iter()
        .any(|(target, from, on_del)| target == "projects"
            && from == "project_id"
            && on_del == "CASCADE"));
    assert!(spec_fks
        .iter()
        .any(|(target, from, on_del)| target == "projects"
            && from == "source_project_id"
            && on_del == "SET NULL"));

    let rev_fks = get_foreign_keys(&conn, "spec_revisions");
    assert!(rev_fks
        .iter()
        .any(|(target, from, on_del)| target == "project_specs"
            && from == "spec_id"
            && on_del == "CASCADE"));
    assert!(rev_fks
        .iter()
        .any(|(target, from, on_del)| target == "projects"
            && from == "project_id"
            && on_del == "CASCADE"));
}

#[test]
fn mig_01_concurrent_open_wal_is_idempotent() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("concurrent-wal.db");
    let root = dir.path().to_path_buf();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let db = db_path.clone();
            let r = root.clone();
            std::thread::spawn(move || {
                SqlitePool::open_wal(SqliteWalConfig::new(db, r)).expect("concurrent open_wal")
            })
        })
        .collect();

    for h in handles {
        let pool = h.join().expect("thread join");
        let conn = pool.connection().expect("conn");
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(
            table_count, 39,
            "17 V1 tables + 8 V2 tables + flow_draft_deployments + schema_migrations + 4 V4 tables + 8 V6 tables = 39"
        );
    }
}

#[test]
fn it_mig_01_populated_v1_v2_upgrade_preserves_ids_and_screen_artifacts_fks() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir .cowok-design");
    let db_path = cowok_dir.join("cowok-design.sqlite3");

    // Seed a real pre-V3 (V1 + V2) database WITHOUT projects.tenant_id and WITH screen_artifacts
    {
        let conn = Connection::open(&db_path).expect("open raw v1 db");
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .expect("pragmas");
        conn.execute_batch(spec_registry::pool::V1_MIGRATION_SQL)
            .expect("v1");
        conn.execute_batch(spec_registry::pool::V2_MIGRATION_SQL)
            .expect("v2");
        conn.execute(
            "INSERT INTO projects (id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES ('11111111-1111-4111-8111-111111111111', 'legacy-proj', 'Legacy Project', '/tmp/legacy', '', 'ACTIVE', 1000, 1000)",
            [],
        )
        .expect("insert project");
        conn.execute(
            "INSERT INTO flow_graphs (id, project_id, portal_key, title, updated_at)
             VALUES ('22222222-2222-4222-8222-222222222222', '11111111-1111-4111-8111-111111111111', 'default', 'Main Flow', 1000)",
            [],
        )
        .expect("insert graph");
        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, project_id, screen_key, title, route_path, fixture_ref, viewport_preset, updated_at)
             VALUES ('33333333-3333-4333-8333-333333333331', '22222222-2222-4222-8222-222222222222', '11111111-1111-4111-8111-111111111111', 'home', 'Home', '/home', 'Default', 'desktop-1440', 1000),
                    ('33333333-3333-4333-8333-333333333332', '22222222-2222-4222-8222-222222222222', '11111111-1111-4111-8111-111111111111', 'checkout', 'Checkout', '/checkout', 'Default', 'desktop-1440', 1000)",
            [],
        )
        .expect("insert nodes");
        conn.execute(
            "INSERT INTO flow_edges (id, graph_id, from_node_id, to_node_id, trigger_action, updated_at)
             VALUES ('44444444-4444-4444-8444-444444444444', '22222222-2222-4222-8222-222222222222', '33333333-3333-4333-8333-333333333331', '33333333-3333-4333-8333-333333333332', 'submitOrder', 1000)",
            [],
        )
        .expect("insert edge");
        conn.execute(
            "INSERT INTO screen_artifacts (id, node_id, content_hash, artifact_path, anchor_status, created_at)
             VALUES ('55555555-5555-4555-8555-555555555555', '33333333-3333-4333-8333-333333333331', 'sha256-abc', 'screens/home.html', 'ANCHORED', 1000)",
            [],
        )
        .expect("insert screen_artifact");
    }

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect("V3 12-step rebuild must succeed on populated V1/V2 DB");
    let conn = pool.connection().expect("conn");

    // Verify conditional backup .pre-v3.*.bak exists
    let has_bak = std::fs::read_dir(&cowok_dir)
        .expect("read_dir")
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().contains(".pre-v3."));
    assert!(
        has_bak,
        "Populated pre-V3 DB must create a .pre-v3.<ts>.bak backup"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = std::fs::metadata(&cowok_dir).unwrap().permissions().mode() & 0o777;
        let db_mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, ".cowok-design directory must be 0700");
        assert_eq!(db_mode, 0o600, "SQLite DB file must be 0600");
    }

    let (graph_ver, graph_layout_ver, tenant_id): (i64, i64, String) = conn
        .query_row(
            "SELECT version, layout_version, tenant_id FROM flow_graphs WHERE id = '22222222-2222-4222-8222-222222222222'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("upgraded graph");
    assert_eq!((graph_ver, graph_layout_ver), (1, 1));
    assert_eq!(tenant_id, "local-default");

    let (src_id, dst_id, trigger_event, edge_ver): (String, String, String, i64) = conn
        .query_row(
            "SELECT source_node_id, target_node_id, trigger_event, version FROM flow_edges WHERE id = '44444444-4444-4444-8444-444444444444'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("upgraded edge");
    assert_eq!(src_id, "33333333-3333-4333-8333-333333333331");
    assert_eq!(dst_id, "33333333-3333-4333-8333-333333333332");
    assert_eq!(trigger_event, "submitOrder");
    assert_eq!(edge_ver, 1);

    let artifact_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM screen_artifacts WHERE id = '55555555-5555-4555-8555-555555555555'", [], |r| r.get(0))
        .expect("artifact count");
    assert_eq!(artifact_count, 1);
}

#[test]
fn it_mig_02_self_loop_and_cross_tenant_composite_fk_enforced() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("fk-and-selfloop.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let conn = pool.connection().expect("conn");

    conn.execute(
        "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
         VALUES ('p1', 'tenant-a', 'proj-a', 'Proj A', '/tmp/a', '', 'ACTIVE', 1, 1),
                ('p2', 'tenant-b', 'proj-b', 'Proj B', '/tmp/b', '', 'ACTIVE', 1, 1)",
        [],
    )
    .expect("projects");
    conn.execute(
        "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, updated_at)
         VALUES ('g1', 'tenant-a', 'p1', 'default', 'Flow A', 1, 1, 1),
                ('g2', 'tenant-b', 'p2', 'default', 'Flow B', 1, 1, 1)",
        [],
    )
    .expect("graphs");
    conn.execute(
        "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, screen_key, title, created_at, updated_at)
         VALUES ('n1', 'g1', 'tenant-a', 'p1', 's1', 'S1', 1, 1),
                ('n2', 'g2', 'tenant-b', 'p2', 's2', 'S2', 1, 1)",
        [],
    )
    .expect("nodes");

    // Self-loop must fail CHECK (source_node_id != target_node_id)
    let self_loop = conn.execute(
        "INSERT INTO flow_edges (id, graph_id, tenant_id, project_id, source_node_id, target_node_id, created_at, updated_at)
         VALUES ('e-self', 'g1', 'tenant-a', 'p1', 'n1', 'n1', 1, 1)",
        [],
    );
    assert!(
        self_loop.is_err(),
        "Self-loop edge must be rejected by SQLite CHECK"
    );

    // Cross-tenant/project edge must fail composite FK
    let cross_tenant = conn.execute(
        "INSERT INTO flow_edges (id, graph_id, tenant_id, project_id, source_node_id, target_node_id, created_at, updated_at)
         VALUES ('e-cross', 'g1', 'tenant-a', 'p1', 'n1', 'n2', 1, 1)",
        [],
    );
    assert!(
        cross_tenant.is_err(),
        "Cross-tenant edge must be rejected by composite FK"
    );
}

#[test]
fn it_mig_03_tampered_v3_migration_checksum_fails_closed() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("tampered-checksum.db");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("initial open_wal");
    let conn = pool.connection().expect("conn");
    conn.execute(
        "UPDATE schema_migrations SET checksum_sha256 = 'deadbeef' WHERE version = 3",
        [],
    )
    .expect("tamper checksum");
    drop(conn);

    let reopened = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()));
    assert!(
        reopened.is_err(),
        "Tampered V3 checksum must fail closed on startup"
    );
    let err_msg = format!("{:?}", reopened.unwrap_err());
    assert!(err_msg.contains("MIGRATION_CHECKSUM_MISMATCH"));
}

#[test]
fn schema_02_v2_migration_adds_8_tables_and_workspace_root_unique_index() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("v2-migration.db");
    let config = SqliteWalConfig::new(&db_path, dir.path());

    let pool = SqlitePool::open_wal(config).expect("open_wal must succeed");
    let conn = pool.connection().expect("connection");

    // Verify all 8 V2 extension tables exist (AC-08)
    for &table in EXPECTED_V2_TABLES {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .expect("v2 table count");
        assert_eq!(count, 1, "Missing mandatory V2 table: {table}");
    }

    // Total domain + extension tables = 24
    let mut total_domain_v2 = 0;
    for &table in EXPECTED_16_TABLES.iter().chain(EXPECTED_V2_TABLES.iter()) {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .expect("table count");
        if count == 1 {
            total_domain_v2 += 1;
        }
    }
    assert_eq!(
        total_domain_v2, 24,
        "Must have exactly 24 domain + V2 tables"
    );

    // Verify unique index on projects(workspace_root)
    let idx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='projects_workspace_root_unique'",
            [],
            |r| r.get(0),
        )
        .expect("index count");
    assert_eq!(
        idx_count, 1,
        "Must have projects_workspace_root_unique index"
    );

    // Verify spec_tombstones columns
    let tombstone_cols = get_table_columns(&conn, "spec_tombstones");
    for col in [
        "id",
        "project_id",
        "spec_key",
        "deleted_at",
        "deleted_by",
        "content_sha256_at_deletion",
    ] {
        assert!(
            tombstone_cols.contains(col),
            "spec_tombstones missing column {col}"
        );
    }

    // Verify recovery_commits columns
    let rec_cols = get_table_columns(&conn, "recovery_commits");
    for col in [
        "journal_id",
        "project_id",
        "spec_key",
        "next_rev",
        "applied_at",
        "status",
    ] {
        assert!(
            rec_cols.contains(col),
            "recovery_commits missing column {col}"
        );
    }
}
