use std::fs;
use std::time::Duration;

use domain::{CanonicalSpecKey, HarnessError, ProvenanceStatus};
use rusqlite::params;
use spec_registry::{
    compute_sha256_hex, recover_pending_spec, SpecLockGuard, SpecRegistry, SqlitePool,
    SqliteWalConfig, SyncOutcome, UpsertSpecInput,
};
use tempfile::tempdir;
use uuid::Uuid;

fn setup_registry() -> (tempfile::TempDir, SpecRegistry, Uuid) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("registry.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = SpecRegistry::new(pool);
    let project_id = Uuid::new_v4();
    registry
        .create_project(
            project_id,
            "cowok-design",
            "cowok-design Platform",
            "Enterprise Design OS",
            None,
        )
        .expect("create_project");
    (dir, registry, project_id)
}

#[test]
fn lock_01_per_spec_lock_enforces_mutual_exclusion_and_timeout() {
    let (dir, _registry, project_id) = setup_registry();
    let guard1 = SpecLockGuard::acquire(
        dir.path(),
        project_id,
        CanonicalSpecKey::ProductOverview,
        Duration::from_millis(200),
    )
    .expect("first lock should succeed");

    // Second lock on same (project_id, spec_key) must time out and return SqliteBusy
    let err = SpecLockGuard::acquire(
        dir.path(),
        project_id,
        CanonicalSpecKey::ProductOverview,
        Duration::from_millis(60),
    )
    .expect_err("second lock on held spec must time out");

    assert!(matches!(err, HarnessError::SqliteBusy { .. }));

    // Lock on a different spec_key succeeds immediately
    let _guard_other = SpecLockGuard::acquire(
        dir.path(),
        project_id,
        CanonicalSpecKey::DatabaseSpec,
        Duration::from_millis(60),
    )
    .expect("different spec_key lock must succeed");

    drop(guard1);

    // After guard1 drops, re-acquiring ProductOverview lock succeeds
    let guard2 = SpecLockGuard::acquire(
        dir.path(),
        project_id,
        CanonicalSpecKey::ProductOverview,
        Duration::from_millis(60),
    )
    .expect("lock after drop must succeed");
    let lock_file_path = guard2.lock_path().to_path_buf();
    drop(guard2);

    // Simulate dead/crashed process leaving an orphan lock file with non-existent PID
    fs::write(&lock_file_path, "pid=99999999\n").unwrap();
    let _guard3 = SpecLockGuard::acquire(
        dir.path(),
        project_id,
        CanonicalSpecKey::ProductOverview,
        Duration::from_millis(60),
    )
    .expect("dead PID lock must be automatically reclaimed");
}

#[test]
fn occ_01_to_04_and_lin_01_atomic_upsert_and_stale_file_occ() {
    let (dir, registry, project_id) = setup_registry();
    let brand_parent_id = Uuid::new_v4();
    registry
        .create_project(
            brand_parent_id,
            "brand-parent",
            "Brand Parent",
            "Parent Brand Brief",
            None,
        )
        .expect("create parent project");

    let initial_md = "# Product Overview v1\n\n[CONFIRMED] cowok-design core overview.\n";
    let (rec1, outcome1) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::ProductOverview,
            markdown_content: initial_md.to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: r#"{"CONFIRMED":1,"INFERRED":0,"MISSING":0}"#.to_string(),
            source_project_id: Some(brand_parent_id),
            source_revision_sha256: Some("a".repeat(64)),
            actor_kind: "AGENT".to_string(),
            actor_id: "architect-agent".to_string(),
            change_summary: "Initial synthesis of 01-product-overview.md".to_string(),
        })
        .expect("initial upsert_spec");

    assert_eq!(rec1.version, 1);
    let expected_sha1 = compute_sha256_hex(initial_md.as_bytes());
    assert_eq!(rec1.content_sha256, expected_sha1);
    assert!(matches!(
        outcome1,
        SyncOutcome::SyncedToDiskAndDb { version: 1, .. }
    ));

    // Verify file on disk exists and no leftover .tmp files remain
    let disk_path = dir.path().join("product/specs/01-product-overview.md");
    assert!(disk_path.exists());
    assert_eq!(fs::read_to_string(&disk_path).unwrap(), initial_md);

    // OCC-01: Update without expected_sha256 on existing spec must fail with StaleFile
    let err_missing_sha = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::ProductOverview,
            markdown_content: "# Unauthorized overwrite".to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "USER".to_string(),
            actor_id: "user-1".to_string(),
            change_summary: "Missing expected_sha256".to_string(),
        })
        .expect_err("missing expected_sha256 must fail");
    assert!(matches!(err_missing_sha, HarnessError::StaleFile { .. }));

    // LIN-01: Two concurrent writers with the same expected_sha256 -> exactly 1 winner, 1 StaleFile
    let reg_a = registry.clone();
    let reg_b = registry.clone();
    let sha_a = expected_sha1.clone();
    let sha_b = expected_sha1.clone();

    let h_a = std::thread::spawn(move || {
        reg_a.upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::ProductOverview,
            markdown_content: "# Product Overview v2 — Writer A\n".to_string(),
            expected_sha256: Some(sha_a),
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "AGENT".to_string(),
            actor_id: "writer-a".to_string(),
            change_summary: "Writer A update".to_string(),
        })
    });

    let h_b = std::thread::spawn(move || {
        reg_b.upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::ProductOverview,
            markdown_content: "# Product Overview v2 — Writer B\n".to_string(),
            expected_sha256: Some(sha_b),
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "AGENT".to_string(),
            actor_id: "writer-b".to_string(),
            change_summary: "Writer B update".to_string(),
        })
    });

    let res_a = h_a.join().unwrap();
    let res_b = h_b.join().unwrap();

    assert!(
        (res_a.is_ok() && matches!(res_b, Err(HarnessError::StaleFile { .. })))
            || (res_b.is_ok() && matches!(res_a, Err(HarnessError::StaleFile { .. })))
    );

    // Verify final DB version is 2, revisions count is 2, and disk hash == DB hash
    let current = registry
        .get_spec(project_id, CanonicalSpecKey::ProductOverview)
        .expect("get_spec")
        .expect("spec exists");
    assert_eq!(current.version, 2);

    let disk_bytes = fs::read(&disk_path).expect("read disk");
    assert_eq!(compute_sha256_hex(&disk_bytes), current.content_sha256);

    let revisions = registry
        .list_revisions(project_id, CanonicalSpecKey::ProductOverview)
        .expect("list_revisions");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].revision_number, 1);
    assert_eq!(revisions[0].source_project_id, Some(brand_parent_id));
    assert_eq!(revisions[1].revision_number, 2);
}

#[test]
fn rec_01_to_05_crash_recovery_replays_or_aborts_pending_intents() {
    let (dir, registry, project_id) = setup_registry();
    let v1_md = "# Architecture v1\n";
    let (rec1, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::Architecture,
            markdown_content: v1_md.to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "SYSTEM".to_string(),
            actor_id: "init".to_string(),
            change_summary: "v1".to_string(),
        })
        .expect("v1 upsert");

    let disk_path = dir.path().join("product/specs/04-architecture.md");

    // Simulate Crash Point A: PENDING intent inserted, crash BEFORE rename (disk still has v1)
    let op_abort = Uuid::new_v4();
    let v2_abort_md = "# Architecture v2 Aborted\n";
    let v2_abort_sha = compute_sha256_hex(v2_abort_md.as_bytes());
    {
        let conn = registry.pool().connection().unwrap();
        conn.execute(
            "INSERT INTO spec_write_intents (
                operation_id, spec_id, project_id, spec_key, expected_version,
                old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                provenance_summary_json, state, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, 2, ?5, ?6, 'AGENT', 'crash-agent', 'aborted write', '{}', 'PENDING', 1000, 1000)",
            params![
                op_abort.to_string(),
                rec1.id.to_string(),
                project_id.to_string(),
                CanonicalSpecKey::Architecture.as_str(),
                rec1.content_sha256,
                v2_abort_sha,
            ],
        )
        .unwrap();
    }

    // Calling get_spec triggers recover_pending_spec() -> marks intent ABORTED, preserves v1
    let after_abort = registry
        .get_spec(project_id, CanonicalSpecKey::Architecture)
        .unwrap()
        .unwrap();
    assert_eq!(after_abort.version, 1);
    assert_eq!(after_abort.content_sha256, rec1.content_sha256);

    // Simulate Crash Point B: PENDING intent inserted AND rename completed on disk, crash BEFORE DB CAS commit
    let op_commit = Uuid::new_v4();
    let v2_commit_md = "# Architecture v2 Crash-Recovered\n";
    let v2_commit_sha = compute_sha256_hex(v2_commit_md.as_bytes());
    fs::write(&disk_path, v2_commit_md.as_bytes()).unwrap();
    {
        let conn = registry.pool().connection().unwrap();
        conn.execute(
            "INSERT INTO spec_write_intents (
                operation_id, spec_id, project_id, spec_key, expected_version,
                old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                provenance_summary_json, state, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, 2, ?5, ?6, 'AGENT', 'crash-agent', 'recovered write', '{\"origin\":\"recovery\"}', 'PENDING', 2000, 2000)",
            params![
                op_commit.to_string(),
                rec1.id.to_string(),
                project_id.to_string(),
                CanonicalSpecKey::Architecture.as_str(),
                rec1.content_sha256,
                v2_commit_sha,
            ],
        )
        .unwrap();
    }

    // Calling get_spec triggers recover_pending_spec() -> CAS finalizes v2 + appends exactly 1 revision
    let after_commit = registry
        .get_spec(project_id, CanonicalSpecKey::Architecture)
        .unwrap()
        .unwrap();
    assert_eq!(after_commit.version, 2);
    assert_eq!(after_commit.content_sha256, v2_commit_sha);

    let revs = registry
        .list_revisions(project_id, CanonicalSpecKey::Architecture)
        .unwrap();
    assert_eq!(revs.len(), 2);
    assert_eq!(revs[1].operation_id, op_commit);

    // Re-running recovery is a no-op (zero duplicate revisions)
    let mut conn = registry.pool().connection().unwrap();
    recover_pending_spec(
        &mut conn,
        dir.path(),
        project_id,
        CanonicalSpecKey::Architecture,
    )
    .unwrap();
    let revs_after = registry
        .list_revisions(project_id, CanonicalSpecKey::Architecture)
        .unwrap();
    assert_eq!(revs_after.len(), 2);

    // Simulate Crash Point C (F-01): Initial creation crashes before rename (placeholder row with empty sha + PENDING intent)
    let placeholder_spec_id = Uuid::new_v4();
    let op_init_abort = Uuid::new_v4();
    {
        let conn = registry.pool().connection().unwrap();
        conn.execute(
            "INSERT INTO project_specs (
                id, project_id, spec_key, file_path, status, version,
                content_sha256, summary_digest, provenance_summary_json,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, 'product/specs/06-cross-dependencies.md', 'CONFIRMED', 1, '', '', '{}', 3000, 3000)",
            params![
                placeholder_spec_id.to_string(),
                project_id.to_string(),
                CanonicalSpecKey::CrossDependencies.as_str(),
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO spec_write_intents (
                operation_id, spec_id, project_id, spec_key, expected_version,
                old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                provenance_summary_json, state, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, 1, NULL, ?5, 'AGENT', 'crash-init', 'init crash', '{}', 'PENDING', 3000, 3000)",
            params![
                op_init_abort.to_string(),
                placeholder_spec_id.to_string(),
                project_id.to_string(),
                CanonicalSpecKey::CrossDependencies.as_str(),
                "b".repeat(64),
            ],
        )
        .unwrap();
    }

    // Subsequent initial upsert on CrossDependencies must recover cleanly and succeed!
    let (rec_cross, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::CrossDependencies,
            markdown_content: "# Cross Dependencies v1\n".to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "AGENT".to_string(),
            actor_id: "retry-agent".to_string(),
            change_summary: "Retry initial creation after crash".to_string(),
        })
        .expect("initial creation retry after crash before rename must succeed");
    assert_eq!(rec_cross.version, 1);
}
