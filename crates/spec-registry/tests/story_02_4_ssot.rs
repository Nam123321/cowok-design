// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use domain::token_budget::count_exact_bpe_tokens;
use rusqlite::ffi;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use spec_registry::exports::{
    acquire_export_reader_lease, allocate_export_snapshot, commit_export_snapshot,
    export_hidden_ssot_bundle, prune_export_generations, read_and_verify_latest_export,
    resolve_jailed_export_root, SsotExportError, SsotExportRequest,
};
use spec_registry::locks::{
    compute_deterministic_jitter_backoff_ms, mutate_appshell_lock, retry_on_busy_with_jitter,
    AppShellLockAction, AppShellLockCommand, SsotLockError,
};
use spec_registry::reverse_sync::{
    list_component_patterns, reverse_sync_tsx_components, ReverseSyncDisposition,
    ReverseSyncRequest,
};
use spec_registry::skill_router::{
    CachePreservingSkillRouter, SkillRouteCommand, COWOK_DESIGN_OPT_SPINE,
    COWOK_DESIGN_OPT_SPINE_MAX_TOKENS,
};
use spec_registry::{SqlitePool, SqliteWalConfig};
use tempfile::TempDir;

fn setup_test_db() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("create tempdir");
    let db_path = dir.path().join("ssot_02_4.sqlite");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path()))
        .expect("init V18 SQLite WAL pool");
    let conn = pool.connection().expect("pool connection");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, name, slug, workspace_root, product_brief, status, created_at, updated_at) \
         VALUES ('proj-1', 'tenant-a', 'Alpha Project', 'alpha-project', '.', '', 'ACTIVE', 1700000000000, 1700000000000)",
        [],
    )
    .expect("seed project");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, name, slug, workspace_root, product_brief, status, created_at, updated_at) \
         VALUES ('proj-b', 'tenant-b', 'Beta Project', 'beta-project', '.', '', 'ACTIVE', 1700000000000, 1700000000000)",
        [],
    )
    .expect("seed tenant-b project");
    (dir, db_path)
}

#[test]
fn ut_retry_01_deterministic_sha256_jitter_bounds_and_busy_exhaustion() {
    for attempt in 0u8..3u8 {
        let mut hasher = Sha256::new();
        hasher.update(b"tenant-a\x00proj-1\x00DeveloperPortal\x00");
        hasher.update([attempt]);
        let digest = hasher.finalize();
        let seed = u64::from(u32::from_be_bytes([
            digest[0], digest[1], digest[2], digest[3],
        ]));
        let expected = match attempt {
            0 => 15 + (seed % 5),
            1 => 45 + (seed % 15),
            _ => 105 + (seed % 30),
        };
        let actual = compute_deterministic_jitter_backoff_ms(
            "tenant-a",
            "proj-1",
            "DeveloperPortal",
            attempt,
        );
        assert_eq!(actual, expected);
    }

    let mut calls = 0usize;
    let err = retry_on_busy_with_jitter::<(), _>(
        "tenant-a",
        "proj-1",
        "DeveloperPortal",
        "ut_retry_01_busy",
        || {
            calls += 1;
            Err(rusqlite::Error::SqliteFailure(
                ffi::Error::new(ffi::SQLITE_BUSY),
                Some("busy".to_string()),
            ))
        },
    )
    .expect_err("should exhaust after 3 retries (4 calls)");

    assert_eq!(calls, 4);
    assert!(matches!(
        err,
        SsotLockError::StorageBusy {
            retry_after_secs: 1,
            ..
        }
    ));

    let mut non_busy_calls = 0usize;
    let non_busy = retry_on_busy_with_jitter::<(), _>(
        "tenant-a",
        "proj-1",
        "DeveloperPortal",
        "ut_retry_01_non_busy",
        || {
            non_busy_calls += 1;
            Err(rusqlite::Error::SqliteFailure(
                ffi::Error::new(ffi::SQLITE_CONSTRAINT_UNIQUE),
                Some("unique".to_string()),
            ))
        },
    )
    .expect_err("non-busy error fails immediately");
    assert_eq!(non_busy_calls, 1);
    assert!(matches!(non_busy, SsotLockError::Validation { .. }));
}

#[test]
fn it_lock_07_and_08_cas_fencing_concurrency_expiry_and_transfer_rbac() {
    let (_dir, db_path) = setup_test_db();

    let barrier = Arc::new(Barrier::new(6));
    let handles: Vec<_> = (0..6)
        .map(|i| {
            let b = barrier.clone();
            let path = db_path.clone();
            thread::spawn(move || {
                let mut conn = Connection::open(&path).expect("open conn");
                let owner = format!("worker-{i}");
                b.wait();
                mutate_appshell_lock(
                    &mut conn,
                    &AppShellLockCommand {
                        tenant_id: "tenant-a".to_string(),
                        project_id: "proj-1".to_string(),
                        portal_id: "DeveloperPortal".to_string(),
                        caller_actor_id: owner.clone(),
                        is_privileged_role: false,
                        action: AppShellLockAction::Acquire,
                        lock_owner_id: owner,
                        expected_revision: 0,
                        expected_fencing_token: None,
                        locked_slots: vec!["TopBar".to_string(), "Sidebar".to_string()],
                        ttl_ms: 60_000,
                        force_transfer_reason: None,
                        now_ms: Some(1_000_000),
                    },
                )
            })
        })
        .collect();

    let mut acquired = None;
    let mut conflicts = 0usize;
    for h in handles {
        match h.join().expect("thread join") {
            Ok(res) => acquired = Some(res),
            Err(SsotLockError::FencingConflict(c)) => {
                conflicts += 1;
                assert_eq!(c.current_revision, 1);
                assert_eq!(c.current_fencing_token, 1);
                assert!(!c.is_expired);
            }
            Err(other) => panic!("Unexpected lock error: {other}"),
        }
    }
    let winner = acquired.expect("strictly 1 worker acquires initial lease");
    assert_eq!(conflicts, 5);
    assert_eq!(winner.lock.fencing_token, 1);
    assert_eq!(winner.lock.revision, 1);
    assert_eq!(winner.lock.locked_slots, vec!["TopBar", "Sidebar"]);

    let mut conn = Connection::open(&db_path).expect("open conn");

    // Non-admin TRANSFER rejected with LOCK_TRANSFER_FORBIDDEN
    let non_admin_err = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "intruder".to_string(),
            is_privileged_role: false,
            action: AppShellLockAction::Transfer,
            lock_owner_id: "intruder".to_string(),
            expected_revision: 1,
            expected_fencing_token: None,
            locked_slots: vec!["TopBar".to_string()],
            ttl_ms: 60_000,
            force_transfer_reason: Some("handover".to_string()),
            now_ms: Some(1_010_000),
        },
    )
    .expect_err("non-admin transfer forbidden");
    assert!(matches!(
        non_admin_err,
        SsotLockError::Forbidden {
            code: "LOCK_TRANSFER_FORBIDDEN",
            ..
        }
    ));

    // Admin TRANSFER without reason rejected
    let empty_reason_err = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "admin-1".to_string(),
            is_privileged_role: true,
            action: AppShellLockAction::Transfer,
            lock_owner_id: "admin-1".to_string(),
            expected_revision: 1,
            expected_fencing_token: None,
            locked_slots: vec!["TopBar".to_string()],
            ttl_ms: 60_000,
            force_transfer_reason: Some("   ".to_string()),
            now_ms: Some(1_010_000),
        },
    )
    .expect_err("empty forceTransferReason forbidden");
    assert!(matches!(
        empty_reason_err,
        SsotLockError::Forbidden {
            code: "LOCK_TRANSFER_FORBIDDEN",
            ..
        }
    ));

    // Admin TRANSFER without expectedFencingToken rejected with INVALID_REQUEST
    let missing_tok_transfer = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "admin-1".to_string(),
            is_privileged_role: true,
            action: AppShellLockAction::Transfer,
            lock_owner_id: "admin-1".to_string(),
            expected_revision: 1,
            expected_fencing_token: None,
            locked_slots: vec!["TopBar".to_string()],
            ttl_ms: 60_000,
            force_transfer_reason: Some("on-call incident takeover".to_string()),
            now_ms: Some(1_010_000),
        },
    )
    .expect_err("transfer without fencing token must fail");
    assert!(matches!(
        missing_tok_transfer,
        SsotLockError::Validation {
            code: "INVALID_REQUEST",
            ..
        }
    ));

    // Admin TRANSFER with stale expectedFencingToken=99 rejected with LOCK_FENCING_CONFLICT
    let stale_tok_transfer = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "admin-1".to_string(),
            is_privileged_role: true,
            action: AppShellLockAction::Transfer,
            lock_owner_id: "admin-1".to_string(),
            expected_revision: 1,
            expected_fencing_token: Some(99),
            locked_slots: vec!["TopBar".to_string()],
            ttl_ms: 60_000,
            force_transfer_reason: Some("on-call incident takeover".to_string()),
            now_ms: Some(1_010_000),
        },
    )
    .expect_err("transfer with stale fencing token must conflict");
    assert!(matches!(
        stale_tok_transfer,
        SsotLockError::FencingConflict(_)
    ));

    // Admin TRANSFER with matching expectedFencingToken=1 succeeds -> bumps both fencing_token (2) & revision (2)
    let transferred = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "admin-1".to_string(),
            is_privileged_role: true,
            action: AppShellLockAction::Transfer,
            lock_owner_id: "admin-1".to_string(),
            expected_revision: 1,
            expected_fencing_token: Some(1),
            locked_slots: vec!["TopBar".to_string()],
            ttl_ms: 30_000,
            force_transfer_reason: Some("on-call incident takeover".to_string()),
            now_ms: Some(1_010_000),
        },
    )
    .expect("admin transfer succeeds");
    assert_eq!(transferred.lock.fencing_token, 2);
    assert_eq!(transferred.lock.revision, 2);
    assert_eq!(transferred.lock.lock_owner_id, "admin-1");

    // Stale owner RENEW with old fencing_token=1 conflicts
    let stale_renew = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: winner.lock.lock_owner_id.clone(),
            is_privileged_role: false,
            action: AppShellLockAction::Renew,
            lock_owner_id: winner.lock.lock_owner_id,
            expected_revision: 2,
            expected_fencing_token: Some(1),
            locked_slots: vec!["TopBar".to_string()],
            ttl_ms: 60_000,
            force_transfer_reason: None,
            now_ms: Some(1_020_000),
        },
    )
    .expect_err("stale fencing token must fail renew");
    assert!(matches!(stale_renew, SsotLockError::FencingConflict(_)));

    // Expired lease takeover at t = 1_050_000 (> 1_040_000) succeeds with expected_revision = 2
    let takeover = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "worker-next".to_string(),
            is_privileged_role: false,
            action: AppShellLockAction::Acquire,
            lock_owner_id: "worker-next".to_string(),
            expected_revision: 2,
            expected_fencing_token: None,
            locked_slots: vec!["Footer".to_string()],
            ttl_ms: 45_000,
            force_transfer_reason: None,
            now_ms: Some(1_050_000),
        },
    )
    .expect("expired lock takeover succeeds");
    assert_eq!(takeover.lock.fencing_token, 3);
    assert_eq!(takeover.lock.revision, 3);

    // RELEASE with matching fencing_token clears lock_owner_id tombstoning revision=4
    let released = mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor_id: "worker-next".to_string(),
            is_privileged_role: false,
            action: AppShellLockAction::Release,
            lock_owner_id: "worker-next".to_string(),
            expected_revision: 3,
            expected_fencing_token: Some(3),
            locked_slots: vec![],
            ttl_ms: 45_000,
            force_transfer_reason: None,
            now_ms: Some(1_055_000),
        },
    )
    .expect("release succeeds");
    assert_eq!(released.lock.lock_owner_id, "");
    assert!(released.lock.is_expired);
    assert_eq!(released.lock.revision, 4);
}

#[test]
fn it_export_06_07_atomic_manifest_superseded_retention_leases_and_tamper_detection() {
    let (dir, db_path) = setup_test_db();
    let workspace_root = dir.path();

    let traversal = resolve_jailed_export_root(workspace_root, Some("../escape"));
    assert!(matches!(
        traversal,
        Err(SsotExportError::PathTraversal { .. })
    ));

    let mut conn = Connection::open(&db_path).expect("open conn");

    let synced = reverse_sync_tsx_components(
        &mut conn,
        &ReverseSyncRequest {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "DeveloperPortal".to_string(),
            caller_actor: "dev-1".to_string(),
            source_file_path: "src/pages/workbench/WorkbenchShell.tsx".to_string(),
            source_code: r#"
                export function WorkbenchShell({ portalId }: { portalId: string }) {
                    return <div data-ff-id="appshell-lock-panel" style={{ background: "var(--bg-card)" }}>{portalId}</div>;
                }
            "#
            .to_string(),
            now_ms: Some(100_000),
        },
    )
    .expect("sync tsx");
    assert_eq!(synced.len(), 1);
    assert_eq!(synced[0].disposition, ReverseSyncDisposition::Synced);

    let req_base = SsotExportRequest {
        tenant_id: "tenant-a".to_string(),
        project_id: "proj-1".to_string(),
        portal_id: "DeveloperPortal".to_string(),
        formats: vec!["markdown".to_string(), "json".to_string()],
        include_quarantined: false,
        relative_export_subdir: None,
        now_ms: Some(200_000),
    };
    let snap1 = allocate_export_snapshot(
        &mut conn,
        "tenant-a",
        "proj-1",
        "DeveloperPortal",
        false,
        200_000,
    )
    .expect("snap1");
    let snap2 = allocate_export_snapshot(
        &mut conn,
        "tenant-a",
        "proj-1",
        "DeveloperPortal",
        false,
        200_000,
    )
    .expect("snap2");
    assert_eq!(snap1.export_revision, 1);
    assert_eq!(snap2.export_revision, 2);

    let res2 = commit_export_snapshot(workspace_root, &req_base, snap2).expect("rev 2 commits");
    assert_eq!(res2.revision, 2);
    assert_eq!(res2.markdown_sha256.len(), 64);
    assert_eq!(res2.json_sha256.len(), 64);

    let stale_err =
        commit_export_snapshot(workspace_root, &req_base, snap1).expect_err("stale rev 1 rejected");
    assert!(matches!(
        stale_err,
        SsotExportError::ExportSuperseded {
            attempted_revision: 1,
            latest_revision: 2,
        }
    ));

    for idx in 0..7 {
        let mut req_i = req_base.clone();
        req_i.now_ms = Some(210_000 + idx * 100_000);
        let res = export_hidden_ssot_bundle(&mut conn, workspace_root, &req_i).expect("export");
        assert_eq!(res.revision, 3 + idx);
    }

    let export_root = resolve_jailed_export_root(workspace_root, None).expect("export root");
    let gens_dir = export_root.join("generations");
    let remaining: Vec<String> = fs::read_dir(&gens_dir)
        .expect("read gens")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        remaining.len(),
        5,
        "retention keeps newest 5 generations when older >60s"
    );

    let mut oldest_gen = remaining.clone();
    oldest_gen.sort();
    let protected_gen = oldest_gen[0].clone();
    let _lease = acquire_export_reader_lease(&export_root, &protected_gen, 600_000, 1_500_000)
        .expect("acquire lease");
    let _pruned_after = prune_export_generations(&export_root, 1_500_000).expect("prune");
    assert!(
        gens_dir.join(&protected_gen).exists(),
        "reader lease must protect generation from pruning"
    );

    let verified = read_and_verify_latest_export(workspace_root, None, Some(1_500_000))
        .expect("valid manifest");
    assert_eq!(verified.export_revision, 9);

    let tampered_bundle = gens_dir
        .join(&verified.generation_id)
        .join("ssot-bundle.json");
    fs::write(&tampered_bundle, b"{\"tampered\":true}").expect("tamper bundle");
    let tamper_err = read_and_verify_latest_export(workspace_root, None, Some(1_500_000))
        .expect_err("tampered bundle must fail hash verification");
    assert!(matches!(
        tamper_err,
        SsotExportError::ManifestInvalid { .. }
    ));
}

#[test]
fn reverse_sync_suppresses_duplicates_and_quarantines_drift_when_locked_by_another_actor() {
    let (_dir, db_path) = setup_test_db();
    let mut conn = Connection::open(&db_path).expect("open conn");

    let initial_tsx = r#"
        export function PatternCard({ title }: { title: string }) {
            return <div data-ff-id="pattern-card" style={{ color: "var(--text-primary)" }}>{title}</div>;
        }
    "#;

    let first = reverse_sync_tsx_components(
        &mut conn,
        &ReverseSyncRequest {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "SalesApp".to_string(),
            caller_actor: "designer-a".to_string(),
            source_file_path: "src/components/PatternCard.tsx".to_string(),
            source_code: initial_tsx.to_string(),
            now_ms: Some(1_000),
        },
    )
    .expect("initial sync");
    assert_eq!(first[0].disposition, ReverseSyncDisposition::Synced);
    let canonical_ast = first[0].pattern.ast_signature_sha256.clone();

    let second = reverse_sync_tsx_components(
        &mut conn,
        &ReverseSyncRequest {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "SalesApp".to_string(),
            caller_actor: "designer-a".to_string(),
            source_file_path: "src/components/PatternCard.tsx".to_string(),
            source_code: initial_tsx.to_string(),
            now_ms: Some(2_000),
        },
    )
    .expect("dedup sync");
    assert_eq!(
        second[0].disposition,
        ReverseSyncDisposition::SuppressedNoOp
    );

    mutate_appshell_lock(
        &mut conn,
        &AppShellLockCommand {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "SalesApp".to_string(),
            caller_actor_id: "designer-a".to_string(),
            is_privileged_role: false,
            action: AppShellLockAction::Acquire,
            lock_owner_id: "designer-a".to_string(),
            expected_revision: 0,
            expected_fencing_token: None,
            locked_slots: vec!["ShellHeader".to_string()],
            ttl_ms: 60_000,
            force_transfer_reason: None,
            now_ms: Some(3_000),
        },
    )
    .expect("lock SalesApp");

    let modified_tsx = r#"
        export function PatternCard({ title, subtitle }: { title: string; subtitle: string }) {
            return <section data-ff-id="pattern-card" style={{ color: "var(--accent-primary)" }}>{title}{subtitle}</section>;
        }
    "#;
    let quarantined = reverse_sync_tsx_components(
        &mut conn,
        &ReverseSyncRequest {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "SalesApp".to_string(),
            caller_actor: "bot-sync".to_string(),
            source_file_path: "src/components/PatternCard.tsx".to_string(),
            source_code: modified_tsx.to_string(),
            now_ms: Some(4_000),
        },
    )
    .expect("quarantined sync");
    assert_eq!(
        quarantined[0].disposition,
        ReverseSyncDisposition::QuarantinedDrift
    );
    assert_eq!(quarantined[0].pattern.drift_status, "quarantined_drift");
    assert_eq!(
        quarantined[0].pattern.ast_signature_sha256, canonical_ast,
        "Canonical locked AST signature must NOT be overwritten during quarantine"
    );
    assert_ne!(quarantined[0].pattern.quarantined_diff_json, "{}");

    let page_while_quarantined =
        list_component_patterns(&conn, "tenant-a", "proj-1", "SalesApp", None, 50)
            .expect("list SalesApp quarantined");
    assert_eq!(page_while_quarantined.quarantined_count, 1);

    // Once lock expires (t = 70_000 > 63_000), re-running reverse sync on the SAME modified TSX
    // MUST reconcile from 'quarantined_drift' -> 'synced' cleanly (not stay stranded in SuppressedNoOp).
    let reconciled = reverse_sync_tsx_components(
        &mut conn,
        &ReverseSyncRequest {
            tenant_id: "tenant-a".to_string(),
            project_id: "proj-1".to_string(),
            portal_id: "SalesApp".to_string(),
            caller_actor: "bot-sync".to_string(),
            source_file_path: "src/components/PatternCard.tsx".to_string(),
            source_code: modified_tsx.to_string(),
            now_ms: Some(70_000),
        },
    )
    .expect("reconcile after expiry");
    assert_eq!(reconciled[0].disposition, ReverseSyncDisposition::Synced);
    assert_eq!(reconciled[0].pattern.drift_status, "synced");
    assert_ne!(
        reconciled[0].pattern.ast_signature_sha256, canonical_ast,
        "Unlocked reconciliation updates canonical AST signature"
    );

    let page = list_component_patterns(&conn, "tenant-a", "proj-1", "SalesApp", None, 50)
        .expect("list SalesApp");
    assert_eq!(page.patterns.len(), 1);
    assert_eq!(page.quarantined_count, 0);

    let b_patterns =
        list_component_patterns(&conn, "tenant-b", "proj-b", "SalesApp", None, 50).expect("list b");
    assert!(b_patterns.patterns.is_empty());
    let wrong_proj = list_component_patterns(&conn, "tenant-b", "proj-1", "SalesApp", None, 50);
    assert!(matches!(wrong_proj, Err(SsotLockError::NotFound { .. })));
}

#[test]
fn bench_01_02_router_spine_under_250_bpe_lexicographic_sort_and_kv_prefix_invariance() {
    let spine_tokens = count_exact_bpe_tokens(COWOK_DESIGN_OPT_SPINE);
    assert!(
        spine_tokens <= COWOK_DESIGN_OPT_SPINE_MAX_TOKENS,
        "BENCH-01 failed: cowok-design-opt spine tokens = {spine_tokens} > 250"
    );

    let r1 = CachePreservingSkillRouter::route(&SkillRouteCommand {
        project_id: "proj-1".to_string(),
        intent_summary: "Build Billing KPI Bento grid".to_string(),
        portal_id: "DeveloperPortal".to_string(),
        requested_slugs: Some(vec![
            "typography".to_string(),
            "oklch-palette".to_string(),
            "dashboard-ux".to_string(),
            "typography".to_string(),
        ]),
        max_spine_tokens: 250,
    })
    .expect("route 1");

    let r2 = CachePreservingSkillRouter::route(&SkillRouteCommand {
        project_id: "proj-1".to_string(),
        intent_summary: "Completely different user prompt about drawer states and modals"
            .to_string(),
        portal_id: "DeveloperPortal".to_string(),
        requested_slugs: Some(vec![
            "dashboard-ux".to_string(),
            "typography".to_string(),
            "oklch-palette".to_string(),
        ]),
        max_spine_tokens: 250,
    })
    .expect("route 2");

    assert_eq!(
        r1.selected_slugs,
        vec!["dashboard-ux", "oklch-palette", "typography"]
    );
    assert_eq!(r1.selected_slugs, r2.selected_slugs);
    assert_eq!(
        r1.prefix_sha256, r2.prefix_sha256,
        "KV cache prefix hash must stay 100% invariant when skills set matches"
    );

    let r3 = CachePreservingSkillRouter::route(&SkillRouteCommand {
        project_id: "proj-1".to_string(),
        intent_summary: "Build Billing KPI Bento grid".to_string(),
        portal_id: "DeveloperPortal".to_string(),
        requested_slugs: Some(vec![
            "dashboard-ux".to_string(),
            "typography".to_string(),
            "oklch-palette".to_string(),
            "impeccable-polish".to_string(),
        ]),
        max_spine_tokens: 250,
    })
    .expect("route 3");
    assert_ne!(r1.prefix_sha256, r3.prefix_sha256);

    let mut samples_us = Vec::with_capacity(50);
    for idx in 0..50 {
        let t0 = Instant::now();
        let out = CachePreservingSkillRouter::route(&SkillRouteCommand {
            project_id: "proj-1".to_string(),
            intent_summary: format!("Render settings card #{idx}"),
            portal_id: "SalesApp".to_string(),
            requested_slugs: Some(vec!["typography".to_string(), "oklch-palette".to_string()]),
            max_spine_tokens: 250,
        })
        .expect("route bench");
        let _ = out.prefix_sha256;
        samples_us.push(t0.elapsed().as_micros());
    }
    samples_us.sort_unstable();
    let p95_us = samples_us[47];
    assert!(
        p95_us < 15_000,
        "BENCH-02 failed: p95 routing latency = {p95_us}us >= 15000us"
    );
}
