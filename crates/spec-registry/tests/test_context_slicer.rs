// @story 02.1
// @implements AC-01, AC-02, AC-03
//! Integration Tests for Context Slicer and Token Budget Governor (Story 02.1)

use std::time::Duration;
use tempfile::tempdir;
use uuid::Uuid;

use domain::{
    CanonicalSpecKey, ContextSliceRequest, Department, HarnessError, Portal, ProvenanceStatus,
    ReasonCode, TaskHorizon,
};
use spec_registry::{
    ContextSlicer, SpecLockGuard, SpecRegistry, SqlitePool, SqliteWalConfig, UpsertSpecInput,
};

fn setup_test_registry() -> (tempfile::TempDir, SpecRegistry, Uuid) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("test_registry.db");
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

fn upsert_test_spec(
    registry: &SpecRegistry,
    project_id: Uuid,
    spec_key: CanonicalSpecKey,
    content: &str,
) {
    registry
        .upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            markdown_content: content.to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: Some("0".repeat(64)),
            actor_kind: "AGENT".to_string(),
            actor_id: "test-runner".to_string(),
            change_summary: format!("Seed {:?}", spec_key),
        })
        .expect("upsert_test_spec");
}

#[test]
fn it_01_slice_context_product_ui_department_tenant_admin() {
    let (_dir, registry, project_id) = setup_test_registry();

    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Product Overview\n\nCowok-design platform core overview.",
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::UiUxSpec,
        "# UI UX Spec\n\nDesign system tokens and layout rules.",
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::Architecture,
        "# Architecture\n\nClean Hexagonal Architecture with Rust domain.",
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Update UI button style and hover effects".to_string(),
        override_horizon: None,
        department: Department::ProductUi,
        portal: Portal::TenantAdmin,
        affected_artifacts: vec!["src/components/button.tsx".to_string()],
    };

    let slice = slicer
        .slice_context(project_id, &request)
        .expect("slice_context must succeed");

    assert_eq!(slice.project_id, project_id);
    assert_eq!(slice.decision.horizon, TaskHorizon::Short);
    assert_eq!(slice.decision.token_ceiling, 600);
    assert_eq!(slice.decision.primary_reason, ReasonCode::Success);
    assert!(slice.atomic_complete);

    // Verify allowed specs
    let keys: Vec<CanonicalSpecKey> = slice.slices.iter().map(|s| s.spec_key).collect();
    assert!(keys.contains(&CanonicalSpecKey::ProductOverview));
    assert!(keys.contains(&CanonicalSpecKey::UiUxSpec));
    assert!(keys.contains(&CanonicalSpecKey::Architecture));

    // Priority 1 (ProductOverview) should appear first in stable sort
    assert_eq!(slice.slices[0].spec_key, CanonicalSpecKey::ProductOverview);
    assert!(slice.total_estimated_tokens <= 600);
}

#[test]
fn it_02_token_budget_governor_pruning_under_short_horizon() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Priority 1 spec: small
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Core Vision\n\nCore product overview invariant.",
    );

    // Priority 2 spec: large content that pushes total tokens > 600
    let large_architecture_spec = format!(
        "# Architecture Spec\n\n{}\n",
        "This is an extensive architecture description with many modules and layers. ".repeat(60)
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::Architecture,
        &large_architecture_spec,
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Refactor architecture interfaces".to_string(),
        override_horizon: Some(TaskHorizon::Short), // Ceiling is 600 tokens
        department: Department::ProductUi,
        portal: Portal::TenantAdmin,
        affected_artifacts: vec![],
    };

    let slice = slicer
        .slice_context(project_id, &request)
        .expect("slice_context should succeed with pruning");

    // Total tokens must strictly obey the 600 token ceiling
    assert!(
        slice.total_estimated_tokens <= 600,
        "Total tokens {} exceeded ceiling 600",
        slice.total_estimated_tokens
    );

    // Priority 1 spec must NOT be truncated
    let p1 = slice
        .slices
        .iter()
        .find(|s| s.spec_key == CanonicalSpecKey::ProductOverview)
        .expect("ProductOverview must be present");
    assert!(!p1.content.contains("[TRUNCATED_BY_TOKEN_GOVERNOR]"));

    // Priority 2 spec should be truncated
    let p2 = slice
        .slices
        .iter()
        .find(|s| s.spec_key == CanonicalSpecKey::Architecture)
        .expect("Architecture must be present");
    assert!(p2.content.contains("[TRUNCATED_BY_TOKEN_GOVERNOR]"));
}

#[test]
fn it_03_ac03_boundary_guard_rejects_superadmin_leakage() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Upsert spec with sensitive SuperAdmin directives
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Overview\n\nPublic view with confidential superadmin master_key leaked here.",
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Build marketing landing page slide".to_string(),
        override_horizon: None,
        department: Department::MarketingDeck,
        portal: Portal::MarketingWeb, // Public portal
        affected_artifacts: vec![],
    };

    let res = slicer.slice_context(project_id, &request);
    assert!(
        res.is_err(),
        "Expected BoundaryViolation error for public portal leaking superadmin"
    );

    if let Err(HarnessError::BoundaryViolation { reason }) = res {
        assert!(reason.contains("AC-03 Boundary Violation"));
    } else {
        panic!("Expected BoundaryViolation, got {:?}", res);
    }
}

#[test]
fn it_04_sqlite_retry_backoff_on_busy() {
    let (dir, registry, project_id) = setup_test_registry();

    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Overview\n\nStandard content.",
    );

    // Acquire lock externally to simulate contention
    let _guard = SpecLockGuard::acquire(
        dir.path(),
        project_id,
        CanonicalSpecKey::ProductOverview,
        Duration::from_millis(200),
    )
    .expect("acquire external lock");

    // Slicer query on locked spec will attempt retry with backoff and return SqliteBusy if locked
    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Quick check".to_string(),
        override_horizon: Some(TaskHorizon::Short),
        department: Department::ProductUi,
        portal: Portal::TenantAdmin,
        affected_artifacts: vec![],
    };

    let res = slicer.slice_context(project_id, &request);
    assert!(res.is_err());
    assert!(matches!(
        res.err().unwrap(),
        HarnessError::SqliteBusy { .. }
    ));
}

#[test]
fn e2e_01_slice_dispatch_stable_sort_order() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Insert out of priority order: P3, P2, P1, P2
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::DataAndCopy,
        "# Data & Copy\n\nTypography, copy deck, and strings.",
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::UiUxSpec,
        "# UI UX Spec\n\nComponent specs.",
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Product Overview\n\nFoundation.",
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::Architecture,
        "# Architecture\n\nModular design.",
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Full review of UI and architectural specifications for entire platform"
            .to_string(),
        override_horizon: Some(TaskHorizon::Long), // 1400 token ceiling
        department: Department::ProductUi,
        portal: Portal::TenantAdmin,
        affected_artifacts: vec![],
    };

    let slice = slicer
        .slice_context(project_id, &request)
        .expect("slice_context should succeed");

    assert_eq!(slice.slices.len(), 4);

    // Priority 1 (ProductOverview)
    assert_eq!(slice.slices[0].spec_key, CanonicalSpecKey::ProductOverview);
    assert_eq!(slice.slices[0].priority_rank, 1);

    // Priority 2 (UiUxSpec "03-ui-ux-spec.md", Architecture "04-architecture.md") sorted alphabetically by spec_key filename
    assert_eq!(slice.slices[1].spec_key, CanonicalSpecKey::UiUxSpec);
    assert_eq!(slice.slices[1].priority_rank, 2);
    assert_eq!(slice.slices[2].spec_key, CanonicalSpecKey::Architecture);
    assert_eq!(slice.slices[2].priority_rank, 2);

    // Priority 3 (DataAndCopy)
    assert_eq!(slice.slices[3].spec_key, CanonicalSpecKey::DataAndCopy);
    assert_eq!(slice.slices[3].priority_rank, 3);
}

#[test]
fn it_05_public_portal_unconditionally_denies_database_and_security_specs() {
    let (_dir, _registry, _project_id) = setup_test_registry();

    // Even if department is ExecutiveReport (which normally allows DatabaseSpec),
    // a public facing portal like MarketingWeb must unconditionally deny DatabaseSpec and SecurityTenancy.
    let (allowed, denied) =
        domain::VisibilityPolicy::evaluate(Department::ExecutiveReport, Portal::MarketingWeb);
    assert!(!allowed.contains(&CanonicalSpecKey::DatabaseSpec));
    assert!(!allowed.contains(&CanonicalSpecKey::SecurityTenancy));

    let db_denial = denied
        .iter()
        .find(|(k, _)| *k == CanonicalSpecKey::DatabaseSpec)
        .expect("DatabaseSpec must be denied");
    assert_eq!(db_denial.1, ReasonCode::RedactedSecurityBoundary);
}

#[test]
fn it_06_super_admin_phrase_with_space_redacted_ac03() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Test F-02 fix: "Super Admin" phrase with space must be caught
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Public Overview\n\nConfidential Super Admin directives leaked here.",
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Generate slide deck".to_string(),
        override_horizon: None,
        department: Department::MarketingDeck,
        portal: Portal::MarketingWeb,
        affected_artifacts: vec![],
    };

    let res = slicer.slice_context(project_id, &request);
    assert!(res.is_err());
    if let Err(HarnessError::BoundaryViolation { reason }) = res {
        assert!(reason.contains("super admin"));
    } else {
        panic!("Expected BoundaryViolation, got {:?}", res);
    }
}

#[test]
fn it_07_priority_1_single_spec_pruning_under_tiny_ceiling() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Test F-03 fix: Even if P1 spec alone exceeds ceiling, governor prunes to <= ceiling
    let huge_p1 = format!(
        "# Product Overview\n\n{}\n",
        "Essential foundational vision statement with many repetitive sentences. ".repeat(70)
    );
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        &huge_p1,
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Quick check".to_string(),
        override_horizon: Some(TaskHorizon::Short), // Ceiling = 600
        department: Department::ProductUi,
        portal: Portal::TenantAdmin,
        affected_artifacts: vec![],
    };

    let slice = slicer
        .slice_context(project_id, &request)
        .expect("slice_context must succeed");

    assert!(
        slice.total_estimated_tokens <= 600,
        "Total tokens {} must not exceed 600",
        slice.total_estimated_tokens
    );
    assert!(slice.slices[0]
        .content
        .contains("[TRUNCATED_BY_TOKEN_GOVERNOR]"));
}

#[test]
fn it_08_missing_spec_file_returns_stale_file_error() {
    let (dir, registry, project_id) = setup_test_registry();

    // Upsert spec then delete disk file to simulate missing file
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "# Overview\n\nContent.",
    );

    let target_path =
        spec_registry::resolve_jailed_spec_path(dir.path(), CanonicalSpecKey::ProductOverview)
            .expect("resolve path");
    if target_path.exists() {
        std::fs::remove_file(&target_path).expect("remove file");
    }

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Check spec".to_string(),
        override_horizon: Some(TaskHorizon::Short),
        department: Department::ProductUi,
        portal: Portal::TenantAdmin,
        affected_artifacts: vec![],
    };

    let res = slicer.slice_context(project_id, &request);
    assert!(res.is_err());
    assert!(matches!(res.err().unwrap(), HarnessError::StaleFile { .. }));
}

#[test]
fn it_09_project_constraints_forbid_marketing_rule() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Insert a constraint profile and active FORBID_MARKETING rule into SQLite (F-07)
    {
        let conn = registry.pool().connection().expect("connection");
        let profile_id = Uuid::new_v4().to_string();
        let rule_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at)
             VALUES (?1, ?2, 'strict_b2b', 'tailwind', 'tokens.css', 'icons.json', '[]', '{}', ?3)",
            rusqlite::params![profile_id, project_id.to_string(), now],
        ).expect("insert profile");

        conn.execute(
            "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
             VALUES (?1, ?2, 'FORBID_MARKETING', 'BLOCKER', '{}', 1, ?3)",
            rusqlite::params![rule_id, profile_id, now],
        ).expect("insert rule");
    }

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Prepare marketing deck".to_string(),
        override_horizon: Some(TaskHorizon::Mid),
        department: Department::MarketingDeck,
        portal: Portal::MarketingWeb,
        affected_artifacts: vec![],
    };

    let res = slicer.slice_context(project_id, &request);
    assert!(res.is_err());
    if let Err(HarnessError::BoundaryViolation { reason }) = res {
        assert!(reason.contains("FORBID_MARKETING"));
    } else {
        panic!("Expected BoundaryViolation with FORBID_MARKETING");
    }
}

#[test]
fn it_10_anti_obfuscation_zero_width_space_in_slicer() {
    let (_dir, registry, project_id) = setup_test_registry();

    // Insert ProductOverview containing zero-width space evasion
    upsert_test_spec(
        &registry,
        project_id,
        CanonicalSpecKey::ProductOverview,
        "Super\u{200b}Admin master\u{200b}_key: secret_token_value",
    );

    let slicer = ContextSlicer::new(&registry);
    let request = ContextSliceRequest {
        task_input: "Review product".to_string(),
        override_horizon: Some(TaskHorizon::Short),
        department: Department::MarketingDeck,
        portal: Portal::MarketingWeb,
        affected_artifacts: vec![],
    };

    let res = slicer.slice_context(project_id, &request);
    assert!(res.is_err());
    assert!(matches!(
        res.err().unwrap(),
        HarnessError::BoundaryViolation { .. }
    ));
}
