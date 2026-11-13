// @story 01.2
// @cover AC-06, AC-07, AC-08
use domain::{CanonicalSpecKey, ProvenanceStatus};
use spec_registry::{
    lock::SpecLockGuard,
    registry::{SpecRegistry, SyncOutcome, UpsertSpecInput},
    SqlitePool, SqliteWalConfig,
};
use std::fs;
use std::time::Duration;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn test_tombstone_reconciliation_on_physical_deletion() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("tombstone.db");
    let root = dir.path().to_path_buf();

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &root)).expect("open_wal");
    let registry = SpecRegistry::new(pool);

    let project_id = Uuid::new_v4();
    registry
        .create_project(
            project_id,
            "tombstone-test",
            "Tombstone Test",
            "Brief",
            None,
        )
        .expect("create_project");

    let spec_key = CanonicalSpecKey::ProductOverview;
    let input = UpsertSpecInput {
        project_id,
        spec_key,
        actor_kind: "USER".to_string(),
        actor_id: "dev1".to_string(),
        change_summary: "Initial overview".to_string(),
        status: ProvenanceStatus::Confirmed,
        markdown_content: "# Product Overview\n\nInitial content".to_string(),
        expected_sha256: None,
        source_project_id: None,
        source_revision_sha256: None,
        provenance_summary_json: "{}".to_string(),
    };

    let (_rec, outcome) = registry.upsert_spec(input).expect("upsert_spec");
    assert!(matches!(outcome, SyncOutcome::SyncedToDiskAndDb { .. }));

    // Verify spec is tracked and NOT tombstoned
    assert!(!registry
        .is_spec_tombstoned(project_id, spec_key)
        .expect("is_spec_tombstoned"));

    let spec_path = root.join("product/specs").join(spec_key.as_str());
    assert!(spec_path.exists());

    // Physically delete file from disk (simulating external deletion)
    fs::remove_file(&spec_path).expect("remove_file");
    assert!(!spec_path.exists());

    // Process event via watcher path handler
    let sync_res = registry
        .handle_watcher_path_event(project_id, &spec_path)
        .expect("handle_watcher_path_event");
    assert!(
        matches!(sync_res, SyncOutcome::TombstoneReconciled { .. }),
        "Expected TombstoneReconciled, got {sync_res:?}"
    );

    // Verify spec is now tombstoned
    assert!(registry
        .is_spec_tombstoned(project_id, spec_key)
        .expect("is_spec_tombstoned"));

    // Verify row is removed from project_specs
    let spec_opt = registry.get_spec(project_id, spec_key).expect("get_spec");
    assert!(spec_opt.is_none());

    // Verify revision history records FILE_DELETED
    let revs = registry
        .list_revisions(project_id, spec_key)
        .expect("list_revisions");
    assert!(revs.iter().any(|r| r.content_hash == "FILE_DELETED"));
}

#[test]
fn test_explicit_delete_spec_records_tombstone() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("explicit-delete.db");
    let root = dir.path().to_path_buf();

    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &root)).expect("open_wal");
    let registry = SpecRegistry::new(pool);

    let project_id = Uuid::new_v4();
    registry
        .create_project(project_id, "explicit-del", "Delete Test", "Brief", None)
        .expect("create_project");

    let spec_key = CanonicalSpecKey::SecurityTenancy;
    let input = UpsertSpecInput {
        project_id,
        spec_key,
        actor_kind: "USER".to_string(),
        actor_id: "admin".to_string(),
        change_summary: "Security spec".to_string(),
        status: ProvenanceStatus::Confirmed,
        markdown_content: "# Security Spec".to_string(),
        expected_sha256: None,
        source_project_id: None,
        source_revision_sha256: None,
        provenance_summary_json: "{}".to_string(),
    };

    registry.upsert_spec(input).expect("upsert_spec");
    let spec_path = root.join("product/specs").join(spec_key.as_str());
    assert!(spec_path.exists());

    // Explicit deletion
    let del_res = registry
        .delete_spec(project_id, spec_key, "admin_user")
        .expect("delete_spec");
    assert!(matches!(del_res, SyncOutcome::TombstoneReconciled { .. }));

    // File should be physically removed
    assert!(!spec_path.exists());

    // Tombstone check
    assert!(registry
        .is_spec_tombstoned(project_id, spec_key)
        .expect("is_spec_tombstoned"));
}

#[test]
fn test_cross_platform_lock_exclusive_mutual_exclusion() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let project_id = Uuid::new_v4();
    let spec_key = CanonicalSpecKey::Architecture;

    // Acquire lock on thread 1
    let guard1 = SpecLockGuard::acquire(&root, project_id, spec_key, Duration::from_millis(500))
        .expect("guard1 acquire");

    // Thread 2 should fail within short timeout
    let root_clone = root.clone();
    let handle = std::thread::spawn(move || {
        SpecLockGuard::acquire(&root_clone, project_id, spec_key, Duration::from_millis(50))
    });

    let res2 = handle.join().expect("join");
    assert!(res2.is_err(), "Concurrent lock acquisition must fail");

    // Drop guard1
    drop(guard1);

    // Now acquisition succeeds
    let guard3 = SpecLockGuard::acquire(&root, project_id, spec_key, Duration::from_millis(500));
    assert!(
        guard3.is_ok(),
        "Lock must be acquirable after previous guard dropped"
    );
}

#[test]
fn test_post_tombstone_recreation_and_occ_sequence() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("recreate.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = SpecRegistry::new(pool);

    let project_id = Uuid::new_v4();
    registry
        .create_project(project_id, "recreate", "Recreate", "Brief", None)
        .expect("create_project");

    let spec_key = CanonicalSpecKey::ProductOverview;
    let (v1, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            actor_kind: "USER".to_string(),
            actor_id: "u1".to_string(),
            change_summary: "v1".to_string(),
            status: ProvenanceStatus::Confirmed,
            markdown_content: "# V1".to_string(),
            expected_sha256: None,
            source_project_id: None,
            source_revision_sha256: None,
            provenance_summary_json: "{}".to_string(),
        })
        .expect("upsert v1");

    let (v2, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            actor_kind: "USER".to_string(),
            actor_id: "u1".to_string(),
            change_summary: "v2".to_string(),
            status: ProvenanceStatus::Confirmed,
            markdown_content: "# V2".to_string(),
            expected_sha256: Some(v1.content_sha256),
            source_project_id: None,
            source_revision_sha256: None,
            provenance_summary_json: "{}".to_string(),
        })
        .expect("upsert v2");

    registry
        .delete_spec_checked(project_id, spec_key, Some(&v2.content_sha256), "u1")
        .expect("delete v2");

    let (v4, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            actor_kind: "USER".to_string(),
            actor_id: "u1".to_string(),
            change_summary: "recreated".to_string(),
            status: ProvenanceStatus::Confirmed,
            markdown_content: "# Recreated".to_string(),
            expected_sha256: None,
            source_project_id: None,
            source_revision_sha256: None,
            provenance_summary_json: "{}".to_string(),
        })
        .expect("recreate after tombstone");

    assert!(v4.version >= 4);
    let revs = registry
        .list_revisions(project_id, spec_key)
        .expect("list_revisions");
    assert_eq!(revs.len(), 4);
    assert_eq!(revs[2].content_hash, "FILE_DELETED");
    assert_eq!(revs[3].content_hash, v4.content_sha256);
}

#[test]
fn test_spec_watcher_external_write_and_echo_suppression() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("watcher.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = SpecRegistry::new(pool);

    let project_id = Uuid::new_v4();
    registry
        .create_project(project_id, "watch", "Watch", "Brief", None)
        .expect("create_project");

    let spec_key = CanonicalSpecKey::Architecture;
    let (v1, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            actor_kind: "USER".to_string(),
            actor_id: "u1".to_string(),
            change_summary: "seed".to_string(),
            status: ProvenanceStatus::Confirmed,
            markdown_content: "# Initial Arch".to_string(),
            expected_sha256: None,
            source_project_id: None,
            source_revision_sha256: None,
            provenance_summary_json: "{}".to_string(),
        })
        .expect("upsert v1");

    let watcher = spec_registry::SpecWatcher::start(registry.clone(), project_id).expect("start watcher");
    let spec_path = spec_registry::resolve_jailed_spec_path(dir.path(), spec_key).expect("spec_path");

    // Identical content echo must be suppressed
    let echo = watcher.process_path(&spec_path).expect("echo");
    assert!(matches!(echo, SyncOutcome::SuppressedNoOp { .. }));

    // External disk modification must reconcile to version 2
    std::fs::write(&spec_path, "# Updated External Arch").expect("write external edit");
    let synced = watcher.process_path(&spec_path).expect("sync external edit");
    assert!(matches!(synced, SyncOutcome::SyncedToDiskAndDb { version: 2, .. }));

    let latest = registry.get_spec(project_id, spec_key).expect("get_spec").expect("exists");
    assert_eq!(latest.version, 2);
    assert_ne!(latest.content_sha256, v1.content_sha256);
}
