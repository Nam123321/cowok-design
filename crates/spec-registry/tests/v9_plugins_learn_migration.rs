// @story 08.4
// @implements AC-01, AC-04
use std::collections::BTreeSet;
use std::path::Path;

use rusqlite::{params, Connection};
use spec_registry::plugin_store::{
    claim_cron_lease, commit_cron_fenced, commit_learn_card_decision, insert_learn_card_tx,
    list_plugins_for_tenant, reap_stale_cron_leases, update_plugin_cas, CasFailure,
    ConsumedNonceRow, FencedCronCommit, KnowledgeCardRow, SkillPluginRow,
};
use spec_registry::pool::{
    v12_migration_checksum, v13_migration_checksum, v14_migration_checksum, v3_migration_checksum,
    v4_migration_checksum, v5_migration_checksum, v6_migration_checksum, v7_migration_checksum,
    v8_migration_checksum, v9_migration_checksum, SqlitePool, SqliteWalConfig, V12_MIGRATION_NAME,
    V12_MIGRATION_SQL, V13_MIGRATION_NAME, V13_MIGRATION_SQL, V14_MIGRATION_NAME,
    V14_MIGRATION_SQL, V1_MIGRATION_SQL, V2_MIGRATION_SQL, V3_MIGRATION_SQL, V4_MIGRATION_SQL,
    V5_MIGRATION_SQL, V6_MIGRATION_SQL, V7_MIGRATION_SQL, V8_MIGRATION_NAME, V8_MIGRATION_SQL,
    V9_MIGRATION_NAME,
};
use tempfile::tempdir;

fn seed_v8_database(db_path: &Path) {
    let conn = Connection::open(db_path).expect("open raw v8 db");
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
    .expect("ledger");

    conn.execute_batch(V1_MIGRATION_SQL).expect("v1");
    conn.execute_batch(V2_MIGRATION_SQL).expect("v2");
    conn.execute_batch(
        "ALTER TABLE projects ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';",
    )
    .expect("proj tenant");
    conn.execute_batch(V3_MIGRATION_SQL).expect("v3");
    conn.execute_batch(
        "ALTER TABLE projects ADD COLUMN project_code TEXT;
         ALTER TABLE projects ADD COLUMN trash_deleted_at BIGINT;
         ALTER TABLE projects ADD COLUMN provenance TEXT NOT NULL DEFAULT 'scratch';
         ALTER TABLE chat_sessions ADD COLUMN archived_at BIGINT;",
    )
    .expect("v4 proj cols");
    conn.execute_batch(V4_MIGRATION_SQL).expect("v4");
    conn.execute_batch(V5_MIGRATION_SQL).expect("v5");
    conn.execute_batch(
        "ALTER TABLE chat_sessions ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
         ALTER TABLE chat_messages ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
         ALTER TABLE node_execution_jobs ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
         ALTER TABLE chat_artifact_snapshots ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';",
    )
    .expect("v6 cols");
    conn.execute_batch(V6_MIGRATION_SQL).expect("v6");
    conn.execute_batch("ALTER TABLE sdi_intents ADD COLUMN pr_url TEXT NOT NULL DEFAULT '';")
        .expect("v6 sdi pr_url");
    conn.execute_batch(
        "ALTER TABLE chat_message_node_scopes ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
         ALTER TABLE execution_events ADD COLUMN project_seq INTEGER;
         ALTER TABLE execution_events ADD COLUMN composite_scope_key TEXT NOT NULL DEFAULT '';
         ALTER TABLE element_comments ADD COLUMN artifact_sha256 TEXT NOT NULL DEFAULT '';
         ALTER TABLE element_comments ADD COLUMN anchor_kind TEXT NOT NULL DEFAULT 'html_element';
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
        (8, V8_MIGRATION_NAME, v8_migration_checksum()),
    ] {
        conn.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
             VALUES (?1, ?2, ?3, 1000)",
            params![ver, name, sha],
        )
        .expect("insert ledger");
    }
}

#[test]
fn v9_upgrades_populated_v8_dedups_enabled_and_preserves_discovered() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir");
    let db_path = cowok_dir.join("cowok-design.sqlite3");
    seed_v8_database(&db_path);

    {
        let conn = Connection::open(&db_path).expect("open seeded v8");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, product_brief, status, created_at, updated_at)
             VALUES ('proj-t1', 'tenant-alpha', 'proj-t1', 'Proj T1', '', 'ACTIVE', 100, 100)",
            [],
        )
        .expect("proj");
        // Older ENABLED (should demote to DEPRECATED), newer ENABLED (should become ACTIVE),
        // even newer DISCOVERED on the same slug (must stay DISCOVERED and NOT block ENABLED->ACTIVE!),
        // and legacy DISABLED (should normalize to DEPRECATED).
        for (id, ver, state, ts) in [
            ("p-old-en", "1.0.0", "ENABLED", 1_000i64),
            ("p-new-en", "1.1.0", "ENABLED", 2_000i64),
            ("p-disc", "1.2.0", "DISCOVERED", 3_000i64),
            ("p-dis", "0.9.0", "DISABLED", 500i64),
        ] {
            conn.execute(
                "INSERT INTO skill_plugins (id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
                 VALUES (?1, 'proj-t1', 'ux-helper', ?2, ?3, 'https://github.com/org/repo', '{}', ?4)",
                params![id, ver, state, ts],
            )
            .expect("seed skill_plugins");
        }
    }

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open v9");
    let conn = pool.connection().expect("conn");

    let mut stmt = conn
        .prepare("SELECT id, tenant_id, lifecycle_state FROM skill_plugins ORDER BY id ASC")
        .expect("stmt");
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .expect("query")
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        rows,
        vec![
            ("p-dis".into(), "tenant-alpha".into(), "DEPRECATED".into()),
            ("p-disc".into(), "tenant-alpha".into(), "DISCOVERED".into()),
            ("p-new-en".into(), "tenant-alpha".into(), "ACTIVE".into()),
            (
                "p-old-en".into(),
                "tenant-alpha".into(),
                "DEPRECATED".into()
            ),
        ]
    );

    // Partial unique index forbids inserting a second ACTIVE row for same (tenant, project, slug)
    let dup_active = conn.execute(
        "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
         VALUES ('p-dup', 'tenant-alpha', 'proj-t1', 'ux-helper', '2.0.0', 'ACTIVE', 'local://x', '{}', 4000)",
        [],
    );
    assert!(
        dup_active.is_err(),
        "partial unique ACTIVE index must reject duplicate ACTIVE"
    );

    // CHECK constraint rejects legacy 'ENABLED'
    let legacy_enabled = conn.execute(
        "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
         VALUES ('p-leg', 'tenant-alpha', 'proj-t1', 'other-slug', '1.0.0', 'ENABLED', 'local://x', '{}', 4000)",
        [],
    );
    assert!(
        legacy_enabled.is_err(),
        "CHECK constraint must reject legacy ENABLED"
    );
}

#[test]
fn v12_through_v14_apply_deterministically_after_v11_and_fail_closed_on_tamper() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir");
    let db_path = cowok_dir.join("cowok-design.sqlite3");
    seed_v8_database(&db_path);

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open");
    let conn = pool.connection().expect("conn");
    for (expected_ver, name, expected_sha) in [
        (9i64, V9_MIGRATION_NAME, v9_migration_checksum()),
        (12i64, V12_MIGRATION_NAME, v12_migration_checksum()),
        (13i64, V13_MIGRATION_NAME, v13_migration_checksum()),
        (14i64, V14_MIGRATION_NAME, v14_migration_checksum()),
    ] {
        let (ver, sha): (i64, String) = conn
            .query_row(
                "SELECT version, checksum_sha256 FROM schema_migrations WHERE name = ?1",
                params![name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("migration row");
        assert_eq!(ver, expected_ver, "deterministic slot for {name}");
        assert_eq!(sha, expected_sha, "checksum match for {name}");
    }

    // Tampering with V12 checksum must fail closed
    drop(conn);
    drop(pool);
    {
        let conn = Connection::open(&db_path).expect("tamper open");
        conn.execute(
            "UPDATE schema_migrations SET checksum_sha256 = 'tampered' WHERE name = ?1",
            params![V12_MIGRATION_NAME],
        )
        .expect("tamper");
    }
    let err = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect_err("must fail closed on V12 checksum mismatch");
    assert!(format!("{err:?}").contains("MIGRATION_CHECKSUM_MISMATCH"));
}

#[test]
fn legacy_epic08_v9_to_v11_slots_rekey_to_v12_to_v14_and_backfill_epic05() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir");
    let db_path = cowok_dir.join("cowok-design.sqlite3");
    seed_v8_database(&db_path);

    {
        let conn = Connection::open(&db_path).expect("open legacy epic08");
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("fk off");
        conn.execute_batch(
            "ALTER TABLE skill_plugins ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
             ALTER TABLE skill_plugins ADD COLUMN source_type TEXT NOT NULL DEFAULT 'local';
             ALTER TABLE skill_plugins ADD COLUMN upstream_repo_url TEXT;
             ALTER TABLE skill_plugins ADD COLUMN upstream_commit_sha TEXT;
             ALTER TABLE skill_plugins ADD COLUMN cron_schedule TEXT;
             ALTER TABLE skill_plugins ADD COLUMN skill_md_path TEXT NOT NULL DEFAULT 'SKILL.md';
             ALTER TABLE skill_plugins ADD COLUMN plugin_json_path TEXT NOT NULL DEFAULT 'plugin.json';
             ALTER TABLE skill_plugins ADD COLUMN last_synced_at BIGINT;
             ALTER TABLE skill_plugins ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
             ALTER TABLE skill_plugins ADD COLUMN staging_version TEXT;
             ALTER TABLE skill_plugins ADD COLUMN staging_manifest_json TEXT;
             ALTER TABLE skill_plugins ADD COLUMN staging_diff_reasons_json TEXT;
             ALTER TABLE skill_plugins ADD COLUMN staging_bundle_dir TEXT;
             ALTER TABLE skill_plugins ADD COLUMN staging_package_sha256 TEXT;
             ALTER TABLE skill_plugins ADD COLUMN breaking_change_isolated INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE skill_plugins ADD COLUMN quarantined INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE skill_plugins ADD COLUMN quarantine_reason TEXT;",
        )
        .expect("v12 pre-cols");
        conn.execute_batch(V12_MIGRATION_SQL).expect("legacy v9");
        conn.execute_batch(V13_MIGRATION_SQL).expect("legacy v10");
        conn.execute_batch(
            "ALTER TABLE gate_reports ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default';
             ALTER TABLE gate_reports ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';
             ALTER TABLE gate_reports ADD COLUMN target_id TEXT NOT NULL DEFAULT '';
             ALTER TABLE gate_reports ADD COLUMN action TEXT NOT NULL DEFAULT '';
             ALTER TABLE gate_reports ADD COLUMN revision BIGINT NOT NULL DEFAULT 0;
             ALTER TABLE gate_reports ADD COLUMN fencing_token BIGINT NOT NULL DEFAULT 0;
             ALTER TABLE gate_reports ADD COLUMN reason_code TEXT NOT NULL DEFAULT '';
             ALTER TABLE daemon_sessions ADD COLUMN actor_kind TEXT NOT NULL DEFAULT 'AGENT_HMAC';",
        )
        .expect("v14 pre-cols");
        conn.execute_batch(V14_MIGRATION_SQL).expect("legacy v11");

        for (ver, legacy_name, sha) in [
            (
                9i64,
                "V9__epic08_plugins_learn_cron.sql",
                v12_migration_checksum(),
            ),
            (
                10i64,
                "V10__epic08_audit_remediation.sql",
                v13_migration_checksum(),
            ),
            (
                11i64,
                "V11__epic08_party_remediation.sql",
                v14_migration_checksum(),
            ),
        ] {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum_sha256, applied_at)
                 VALUES (?1, ?2, ?3, 1500)",
                params![ver, legacy_name, sha],
            )
            .expect("insert legacy epic08 slot");
        }
    }

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect("rekey legacy epic08 slots and backfill V9..V11");
    let conn = pool.connection().expect("conn");
    let entries: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT version, name FROM schema_migrations WHERE version >= 9 ORDER BY version ASC")
            .expect("prep");
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        entries,
        vec![
            (9, V9_MIGRATION_NAME.to_string()),
            (10, spec_registry::pool::V10_MIGRATION_NAME.to_string()),
            (11, spec_registry::pool::V11_MIGRATION_NAME.to_string()),
            (12, V12_MIGRATION_NAME.to_string()),
            (13, V13_MIGRATION_NAME.to_string()),
            (14, V14_MIGRATION_NAME.to_string()),
            (15, spec_registry::pool::V15_MIGRATION_NAME.to_string()),
            (16, spec_registry::pool::V16_MIGRATION_NAME.to_string()),
            (17, spec_registry::pool::V17_MIGRATION_NAME.to_string()),
            (18, spec_registry::pool::V18_MIGRATION_NAME.to_string()),
        ]
    );
}

#[test]
fn sqlite_master_matches_prisma_schema_for_story_08_4_tables() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("genesis");
    let conn = pool.connection().expect("conn");

    let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("prisma/schema.prisma");
    let prisma_text = std::fs::read_to_string(&schema_path).expect("read prisma schema");

    for (model_name, table_name) in [
        ("SkillPlugin", "skill_plugins"),
        ("KnowledgeCard", "knowledge_cards"),
        ("LearnConsumedNonce", "learn_consumed_nonces"),
        ("PluginCronSchedule", "plugin_cron_schedules"),
    ] {
        assert!(
            prisma_text.contains(&format!("model {model_name} {{")),
            "prisma missing model {model_name}"
        );
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table_name})"))
            .expect("table info");
        let cols: BTreeSet<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .expect("cols")
            .map(Result::unwrap)
            .collect();
        assert!(!cols.is_empty(), "sqlite missing table {table_name}");
        for col in cols {
            let mapped = format!("@map(\"{col}\")");
            assert!(
                prisma_text.contains(&mapped) || prisma_text.contains(&format!("  {col} ")),
                "prisma model {model_name} missing SQLite column '{col}' for {table_name}"
            );
        }
    }
}

#[test]
fn store_enforces_cas_nonce_replay_and_fenced_cron_lease_reaper() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("genesis");
    {
        let conn = pool.connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, product_brief, status, created_at, updated_at)
             VALUES ('proj-1', 't-a', 'proj-1', 'P1', '', 'ACTIVE', 100, 100)",
            [],
        )
        .expect("proj");
        conn.execute(
            "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, revision, updated_at)
             VALUES ('plug-1', 't-a', 'proj-1', 'demo', '1.0.0', 'INSTALLED', 'https://github.com/a/b', '{}', 1, 100)",
            [],
        )
        .expect("plug");
    }

    // Tenant B cannot see Tenant A plugin (strict tenant isolation)
    {
        let conn = pool.connection().expect("conn");
        assert!(list_plugins_for_tenant(&conn, "t-b", None)
            .unwrap()
            .is_empty());
        assert_eq!(
            list_plugins_for_tenant(&conn, "t-a", Some("proj-1"))
                .unwrap()
                .len(),
            1
        );
    }

    // CAS mismatch returns RevisionConflict
    let updated_row = SkillPluginRow {
        id: "plug-1".into(),
        tenant_id: "t-a".into(),
        project_id: "proj-1".into(),
        plugin_slug: "demo".into(),
        version: "1.0.0".into(),
        lifecycle_state: "ACTIVE".into(),
        source_type: "github".into(),
        source_repo_url: "https://github.com/a/b".into(),
        upstream_repo_url: Some("https://github.com/a/b".into()),
        upstream_commit_sha: None,
        cron_schedule: Some("0 * * * *".into()),
        skill_md_path: "SKILL.md".into(),
        plugin_json_path: "plugin.json".into(),
        manifest_json: "{}".into(),
        last_synced_at: None,
        revision: 2,
        staging_version: None,
        staging_manifest_json: None,
        staging_diff_reasons_json: None,
        staging_bundle_dir: None,
        staging_package_sha256: None,
        breaking_change_isolated: false,
        quarantined: false,
        quarantine_reason: None,
        updated_at: 200,
    };
    let conflict = update_plugin_cas(&pool, 99, &updated_row, None).unwrap();
    assert_eq!(
        conflict,
        Err(CasFailure::RevisionConflict {
            current_revision: 1
        })
    );
    assert!(update_plugin_cas(&pool, 1, &updated_row, None)
        .unwrap()
        .is_ok());

    // Learn card insert + atomic nonce consumption & replay block
    let card = KnowledgeCardRow {
        id: "card-1".into(),
        tenant_id: "t-a".into(),
        project_id: "proj-1".into(),
        idempotency_key: Some("idem-1".into()),
        category: "RULE".into(),
        title: "Use tokens.css".into(),
        content: "Avoid raw utility classes".into(),
        content_sha256: "abc123".into(),
        source_evidence: "gate:tailwind".into(),
        status: "PROPOSED".into(),
        revision: 1,
        reviewed_by: None,
        decision_reason: None,
        created_at: 1_000,
        accepted_at: None,
    };
    insert_learn_card_tx(&pool, &card, None).expect("insert card");
    let mut accepted = card.clone();
    accepted.status = "ACCEPTED".into();
    accepted.revision = 2;
    accepted.reviewed_by = Some("human-op".into());
    accepted.decision_reason = Some("Verified rule".into());
    accepted.accepted_at = Some(1_050);
    let nonce = ConsumedNonceRow {
        tenant_id: "t-a".into(),
        project_id: "proj-1".into(),
        nonce: "nonce-1".into(),
        expires_at_s: 2_000,
        consumed_at: 1_050,
    };
    assert!(
        commit_learn_card_decision(&pool, 1, &accepted, &nonce, None)
            .unwrap()
            .is_ok()
    );
    assert!(commit_learn_card_decision(&pool, 2, &accepted, &nonce, None).is_err());

    // Cron LeaseReaper invalidates zombie fencing_token so stale commit aborts
    spec_registry::plugin_store::upsert_cron_schedule(
        &pool,
        "t-a",
        "proj-1",
        "plug-1",
        "0 * * * *",
        "https://github.com/a/b",
        None,
    )
    .expect("upsert sched");
    let claimed = claim_cron_lease(
        &pool, "t-a", "proj-1", "plug-1", "worker-1", false, 5_000, 30_000,
    )
    .unwrap()
    .expect("lease");
    assert_eq!(reap_stale_cron_leases(&pool, 40_000).unwrap(), 1);
    let zombie_commit = commit_cron_fenced(
        &pool,
        &FencedCronCommit {
            tenant_id: "t-a",
            project_id: "proj-1",
            plugin_id: "plug-1",
            fencing_token: claimed.fencing_token,
            sync_status: "SUCCEEDED",
            last_etag: Some("\"etag1\""),
            last_commit_sha: Some("deadbeef"),
            consecutive_failures: 0,
            next_retry_at: None,
            last_error_code: None,
            last_error_message: None,
            updated_at: 41_000,
        },
    )
    .unwrap();
    assert!(
        !zombie_commit,
        "stale fencing_token commit MUST abort cleanly"
    );
}
