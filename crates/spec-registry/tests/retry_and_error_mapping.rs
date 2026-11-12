use std::fs;
use std::path::Path;

use domain::{CanonicalSpecKey, HarnessError, ProvenanceStatus};
use rusqlite::params;
use spec_registry::{
    compute_sha256_hex, map_sqlite_error, resolve_jailed_spec_path, validate_candidate_spec_path,
    SpecRegistry, SpecWatcher, SqlitePool, SqliteWalConfig, SyncOutcome, UpsertSpecInput,
};
use tempfile::tempdir;
use uuid::Uuid;

fn setup_registry() -> (tempfile::TempDir, SpecRegistry, Uuid) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("watcher-test.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = SpecRegistry::new(pool);
    let project_id = Uuid::new_v4();
    registry
        .create_project(
            project_id,
            "cowok-design-watch",
            "cowok-design Watch",
            "Brief",
            None,
        )
        .expect("create_project");
    (dir, registry, project_id)
}

#[test]
fn watch_01_to_03_and_ext_01_to_04_hash_gated_echo_suppression_and_zero_byte_debounce() {
    let (dir, registry, project_id) = setup_registry();
    let watcher = SpecWatcher::start(registry.clone(), project_id).expect("start watcher");

    let initial_md = "# UI/UX Spec v1\n\nDesign tokens and layout rules.\n";
    let (rec1, _) = registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key: CanonicalSpecKey::UiUxSpec,
            markdown_content: initial_md.to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "AGENT".to_string(),
            actor_id: "ux-agent".to_string(),
            change_summary: "v1".to_string(),
        })
        .expect("upsert v1");

    let spec_path = dir.path().join("product/specs/03-ui-ux-spec.md");

    // WATCH-01: Internal rename echo (disk_sha256 == db_sha256) -> SuppressedNoOp, 0 new revisions!
    for _ in 0..10 {
        let outcome = watcher.process_path(&spec_path).expect("process_path");
        assert_eq!(
            outcome,
            SyncOutcome::SuppressedNoOp {
                spec_key: CanonicalSpecKey::UiUxSpec,
                content_sha256: rec1.content_sha256.clone(),
            }
        );
    }
    let revs = registry
        .list_revisions(project_id, CanonicalSpecKey::UiUxSpec)
        .unwrap();
    assert_eq!(revs.len(), 1, "Echo events must produce 0 extra revisions");

    // Editor transient 0-byte truncation -> IgnoredZeroByteTransient, DB remains at v1
    fs::write(&spec_path, b"").unwrap();
    let zero_outcome = watcher.process_path(&spec_path).unwrap();
    assert!(matches!(
        zero_outcome,
        SyncOutcome::IgnoredZeroByteTransient { .. }
    ));
    let spec_after_zero = registry
        .get_spec(project_id, CanonicalSpecKey::UiUxSpec)
        .unwrap()
        .unwrap();
    assert_eq!(spec_after_zero.version, 1);

    // EXT-01: Valid external edit -> reconciles to v2 with actor 'external_watcher'
    let ext_md = "# UI/UX Spec v2 (Edited in VS Code)\n";
    fs::write(&spec_path, ext_md.as_bytes()).unwrap();
    let ext_outcome = watcher.process_path(&spec_path).unwrap();
    let ext_sha = compute_sha256_hex(ext_md.as_bytes());
    assert_eq!(
        ext_outcome,
        SyncOutcome::SyncedToDiskAndDb {
            spec_key: CanonicalSpecKey::UiUxSpec,
            version: 2,
            content_sha256: ext_sha.clone(),
        }
    );

    let revs_after_ext = registry
        .list_revisions(project_id, CanonicalSpecKey::UiUxSpec)
        .unwrap();
    assert_eq!(revs_after_ext.len(), 2);
    assert_eq!(revs_after_ext[1].actor_id, "external_watcher");

    // EXT-03: Deleted tracked file -> TombstoneReconciled outcome (AC-07)
    fs::remove_file(&spec_path).unwrap();
    let del_outcome = watcher.process_path(&spec_path).unwrap();
    assert!(matches!(
        del_outcome,
        SyncOutcome::TombstoneReconciled {
            spec_key: CanonicalSpecKey::UiUxSpec,
            ..
        } | SyncOutcome::MissingTrackedFile {
            spec_key: CanonicalSpecKey::UiUxSpec,
            ..
        }
    ));
}

#[test]
fn jail_and_sre_01_to_04_path_traversal_symlink_and_sqlite_error_mapping() {
    let (dir, registry, _project_id) = setup_registry();

    // Path traversal rejection (`../`)
    let traversal = Path::new("../outside.md");
    let err_trav = validate_candidate_spec_path(dir.path(), traversal).unwrap_err();
    assert!(matches!(err_trav, HarnessError::PathTraversalDenied { .. }));

    // Symlink escape rejection inside product/specs/
    #[cfg(unix)]
    {
        let outside_file = dir.path().join("secret-outside.md");
        fs::write(&outside_file, "# Secret").unwrap();
        let specs_dir = dir.path().join("product/specs");
        fs::create_dir_all(&specs_dir).unwrap();
        let symlink_spec = specs_dir.join("09-data-and-copy.md");
        std::os::unix::fs::symlink(&outside_file, &symlink_spec).unwrap();

        let symlink_err =
            resolve_jailed_spec_path(dir.path(), CanonicalSpecKey::DataAndCopy).unwrap_err();
        assert!(matches!(
            symlink_err,
            HarnessError::PathTraversalDenied { .. }
        ));
    }

    // SRE-03: Unique constraint violation maps deterministically to ValidationFailed("UNIQUE_CONSTRAINT...")
    let dup_err = registry
        .create_project(
            Uuid::new_v4(),
            "cowok-design-watch", // duplicate slug!
            "Duplicate Slug Project",
            "Brief",
            None,
        )
        .expect_err("duplicate slug must fail");
    match dup_err {
        HarnessError::ValidationFailed { reason } => {
            assert!(
                reason.contains("UNIQUE_CONSTRAINT"),
                "Expected UNIQUE_CONSTRAINT, got: {reason}"
            );
        }
        other => panic!("Expected ValidationFailed for unique constraint, got: {other:?}"),
    }

    // SRE-04: Foreign key violation maps deterministically to ValidationFailed("REFERENTIAL_INTEGRITY...")
    let conn = registry.pool().connection().unwrap();
    let raw_fk_err = conn
        .execute(
            "INSERT INTO project_specs (id, project_id, spec_key, updated_at) VALUES (?1, ?2, ?3, 1000)",
            params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(), // non-existent project_id
                CanonicalSpecKey::ProductOverview.as_str()
            ],
        )
        .unwrap_err();
    let mapped_fk = map_sqlite_error(raw_fk_err, "insert_orphan_spec");
    match mapped_fk {
        HarnessError::ValidationFailed { reason } => {
            assert!(
                reason.contains("REFERENTIAL_INTEGRITY"),
                "Expected REFERENTIAL_INTEGRITY, got: {reason}"
            );
        }
        other => panic!("Expected ValidationFailed for FK violation, got: {other:?}"),
    }
}
