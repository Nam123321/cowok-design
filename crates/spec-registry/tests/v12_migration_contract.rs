// @story 06.1
// @implements AC-02, UT-01, IT-06
use rusqlite::{params, Connection};
use spec_registry::pool::{SqlitePool, SqliteWalConfig, V1_MIGRATION_SQL, V2_MIGRATION_SQL};
use spec_registry::presentation_repo::{
    assign_unique_slide_uid_with_retry, backfill_missing_slide_uids, generate_slide_uid,
    is_valid_slide_uid, reorder_slides, PresentationRepoError, MAX_ULID_COLLISION_RETRIES,
};
use tempfile::tempdir;

#[test]
fn ut_01_v12_backfills_valid_crockford_ulids_and_preserves_table_count() {
    let dir = tempdir().expect("tempdir");
    let cowok_dir = dir.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir .cowok-design");
    let db_path = cowok_dir.join("cowok-design.sqlite3");

    // Seed pre-V12 (V1 + V2) database with legacy presentation_decks + deck_slides (NULL slide_uid)
    {
        let conn = Connection::open(&db_path).expect("open legacy db");
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .expect("pragmas");
        conn.execute_batch(V1_MIGRATION_SQL).expect("v1");
        conn.execute_batch(V2_MIGRATION_SQL).expect("v2");
        conn.execute(
            "INSERT INTO projects (id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES ('proj-legacy', 'proj-legacy', 'Legacy', '/tmp/leg', '', 'ACTIVE', 1000, 1000)",
            [],
        )
        .expect("insert project");
        conn.execute(
            "INSERT INTO presentation_decks (id, project_id, title, theme_preset, slides_count, updated_at)
             VALUES ('deck-legacy', 'proj-legacy', 'Exec Deck', 'night', 2, 1000)",
            [],
        )
        .expect("insert deck");
        conn.execute(
            "INSERT INTO deck_slides (id, deck_id, slide_index, layout_kind, html_path, content_sha256, updated_at)
             VALUES ('slide-a', 'deck-legacy', 0, 'hero_cover', 'slides/a.html', 'sha-a', 1000),
                    ('slide-b', 'deck-legacy', 1, 'problem_solution', 'slides/b.html', 'sha-b', 1001)",
            [],
        )
        .expect("insert slides");
    }

    // Upgrade via open_wal -> runs V3..V12
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let conn = pool.connection().expect("conn");

    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .expect("table count");
    assert_eq!(
        table_count, 94,
        "V3..V33 migrations must yield 94 canonical tables"
    );

    let null_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deck_slides WHERE slide_uid IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("null count");
    assert_eq!(null_count, 0);

    let mut stmt = conn
        .prepare("SELECT id, slide_uid FROM deck_slides ORDER BY slide_index ASC")
        .expect("prepare");
    let uids: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .expect("query")
        .map(|r| r.expect("uid"))
        .collect();
    assert_eq!(uids.len(), 2);
    assert_ne!(uids[0], uids[1]);
    for uid in &uids {
        assert_eq!(
            uid.len(),
            30,
            "ULID with SLD- prefix must be 30 chars: {uid}"
        );
        assert!(
            is_valid_slide_uid(uid),
            "Invalid Crockford Base32 ULID: {uid}"
        );
    }

    // Also verify fresh genesis DB has schema_migrations version 12 & working deck_slides_slide_uid_key
    let fresh_dir = tempdir().expect("fresh dir");
    let fresh_db = fresh_dir
        .path()
        .join(".cowok-design")
        .join("cowok-design.sqlite3");
    std::fs::create_dir_all(fresh_db.parent().unwrap()).expect("mkdir");
    let fresh_pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&fresh_db, fresh_dir.path())).expect("genesis");
    let fresh_conn = fresh_pool.connection().expect("fresh conn");
    let has_v15: i64 = fresh_conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 15",
            [],
            |r| r.get(0),
        )
        .expect("v15 row");
    assert_eq!(has_v15, 1);
}

#[test]
fn it_06_ulid_collision_retry_exhaustion_and_reorder_occ_and_fk_mapping() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir
        .path()
        .join(".cowok-design")
        .join("cowok-design.sqlite3");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("mkdir");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("pool");
    let mut conn = pool.connection().expect("conn");

    conn.execute(
        "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
         VALUES ('p1', 't1', 'p1', 'P1', '/tmp/p1', '', 'ACTIVE', 1000, 1000)",
        [],
    )
    .expect("p1");
    conn.execute(
        "INSERT INTO presentation_decks (id, project_id, title, theme_preset, aspect_ratio, deck_protocol_version, deck_mode, slides_count, revision, updated_at)
         VALUES ('d1', 'p1', 'Deck 1', 'night', '16:9', 1, 'multi-page', 2, 1, 1000)",
        [],
    )
    .expect("d1");
    let uid1 = generate_slide_uid();
    let uid2 = generate_slide_uid();
    conn.execute(
        "INSERT INTO deck_slides (id, deck_id, slide_uid, slide_index, layout_kind, html_path, content_sha256, updated_at)
         VALUES ('s1', 'd1', ?1, 0, 'hero_cover', 'slides/1.html', 'h1', 1000),
                ('s2', 'd1', NULL, 1, 'problem_solution', 'slides/2.html', 'h2', 1001)",
        params![uid1],
    )
    .expect("insert slides");

    // Exhausted retries (> 5 collisions) fail closed with E_ULID_ENTROPY_EXHAUSTED
    let err_exhausted =
        assign_unique_slide_uid_with_retry(&conn, "s2", MAX_ULID_COLLISION_RETRIES, || {
            uid1.clone()
        })
        .expect_err("must exhaust");
    assert!(format!("{err_exhausted}").contains("E_ULID_ENTROPY_EXHAUSTED"));

    // Recover within 5 retries when 2 collisions happen first
    let mut calls = 0usize;
    let uid2_clone = uid2.clone();
    let uid1_clone = uid1.clone();
    let assigned =
        assign_unique_slide_uid_with_retry(&conn, "s2", MAX_ULID_COLLISION_RETRIES, move || {
            calls += 1;
            if calls <= 2 {
                uid1_clone.clone()
            } else {
                uid2_clone.clone()
            }
        })
        .expect("recover on 3rd attempt");
    assert_eq!(assigned, uid2);

    // Backfill is idempotent when 0 NULLs remain
    assert_eq!(
        backfill_missing_slide_uids(&conn).expect("idempotent backfill"),
        0
    );

    // Stale expectedRevision returns StaleRevision
    let occ_err = reorder_slides(
        &mut conn,
        "p1",
        "t1",
        "d1",
        99,
        &[uid2.clone(), uid1.clone()],
    )
    .expect_err("stale rev");
    assert!(matches!(
        occ_err,
        PresentationRepoError::StaleRevision {
            expected: 99,
            actual: 1
        }
    ));

    // Valid reorder swaps cleanly despite UNIQUE(deck_id, slide_index)
    let reordered = reorder_slides(
        &mut conn,
        "p1",
        "t1",
        "d1",
        1,
        &[uid2.clone(), uid1.clone()],
    )
    .expect("valid reorder");
    assert_eq!(reordered.revision, 2);
    assert_eq!(reordered.slides[0].slide_uid, uid2);
    assert_eq!(reordered.slides[1].slide_uid, uid1);

    // Cross-tenant / foreign project returns uniform NotFound
    let idor_err = reorder_slides(
        &mut conn,
        "other-proj",
        "t1",
        "d1",
        2,
        &[uid1.clone(), uid2.clone()],
    )
    .expect_err("idor");
    assert!(matches!(idor_err, PresentationRepoError::NotFound(_)));
}
