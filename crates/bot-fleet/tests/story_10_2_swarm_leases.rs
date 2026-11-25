// @story 10.2
// @implements AC-01, AC-02, AC-03, AC-04 (Disjoint Leases, Ancestor/Prefix Overlap, Symlink O_NOFOLLOW, Stale Fencing Token, 429 Clamp 3->1, Expired -PGID Kill, Builder != Scorer, E1..E4 >= 95%, Generation Fence CAS & V18a Ledger Escalation)
use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, Mutex};

use bot_fleet::design_jury::{
    canonical_e1_e4_benchmark_input, compute_violation_fingerprint, enforce_builder_neq_scorer,
    evaluate_design_jury_session, evaluate_e1_e4_evidence, DesignJuryError, DesignJurySessionInput,
    EvidenceTier, JuryLens, JuryVerdictStatus, QcIssueInput, QcRunInput, ReviewFreezeBundle,
    HUMAN_ONLY_APPROVAL_POLICY, REQUIRED_E1_E4_CATCH_RATE,
};
use bot_fleet::swarm_leases::{
    canonicalize_owned_path, check_conflict_against_active_leases, current_process_pgid,
    paths_overlap_canonical, reap_expired_leases_and_kill_pgid, validate_disjoint_write_sets,
    validate_safe_worker_id, CoordinationMailbox, OneJobContract, PgidKiller, RateLimitSemaphore,
    SwarmLeaseError, WorkerLeaseRecord, WorkerRole, CLAMPED_CONCURRENCY, INITIAL_CONCURRENCY,
    RECOVERY_SUCCESS_THRESHOLD,
};
use tempfile::tempdir;

#[derive(Default, Clone)]
struct RecordingPgidKiller {
    killed: Arc<Mutex<Vec<i32>>>,
}

impl RecordingPgidKiller {
    fn killed_pgids(&self) -> Vec<i32> {
        self.killed.lock().unwrap().clone()
    }
}

impl PgidKiller for RecordingPgidKiller {
    fn kill_pgid(&self, pgid: i32) -> Result<(), SwarmLeaseError> {
        self.killed.lock().unwrap().push(pgid);
        Ok(())
    }
}

#[test]
fn canonical_ancestor_and_prefix_overlap_blocks_nested_and_traversal_paths() {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("src/pages")).expect("mkdir");
    fs::write(dir.path().join("src/pages/Home.tsx"), "export default 1;").expect("write");

    // Ancestor vs descendant collision: src/pages/ vs src/pages/Home.tsx
    assert!(paths_overlap_canonical("src/pages/", "src/pages/Home.tsx"));
    assert!(paths_overlap_canonical("src/a/b.ts", "src/a"));
    assert!(paths_overlap_canonical("./src/pages", "src/pages/Home.tsx"));

    // Sibling prefix with shared string prefix must NOT collide
    assert!(!paths_overlap_canonical(
        "src/pages",
        "src/pages-v2/Home.tsx"
    ));
    assert!(!paths_overlap_canonical("src/a.ts", "src/ab.ts"));

    // Reject parent traversal & absolute paths outside workspace root
    assert!(matches!(
        canonicalize_owned_path(Some(dir.path()), "../etc/passwd"),
        Err(SwarmLeaseError::PathTraversal(_))
    ));
    assert!(matches!(
        canonicalize_owned_path(Some(dir.path()), "src/../../outside.ts"),
        Err(SwarmLeaseError::PathTraversal(_))
    ));
    assert!(matches!(
        canonicalize_owned_path(Some(dir.path()), "/etc/hosts"),
        Err(SwarmLeaseError::PathTraversal(_))
    ));
    assert!(matches!(
        canonicalize_owned_path(None, "/tmp/escape.ts"),
        Err(SwarmLeaseError::PathTraversal(_))
    ));

    // Disjoint write-set validator blocks overlapping workers in a batch
    let batch_conflict = validate_disjoint_write_sets(
        Some(dir.path()),
        &[
            ("worker-a".to_string(), vec!["src/pages/".to_string()]),
            (
                "worker-b".to_string(),
                vec!["src/pages/Home.tsx".to_string()],
            ),
        ],
    );
    assert!(matches!(
        batch_conflict,
        Err(SwarmLeaseError::LeasePathConflict { .. })
    ));

    // Disjoint workers succeed with canonical normalized paths
    let batch_ok = validate_disjoint_write_sets(
        Some(dir.path()),
        &[
            (
                "worker-a".to_string(),
                vec!["./src/pages/Home.tsx".to_string()],
            ),
            (
                "worker-b".to_string(),
                vec!["src/components/Header.tsx".to_string()],
            ),
        ],
    )
    .expect("disjoint batch should pass");
    assert_eq!(batch_ok[0].1, vec!["src/pages/Home.tsx"]);
    assert_eq!(batch_ok[1].1, vec!["src/components/Header.tsx"]);
}

#[cfg(unix)]
#[test]
fn canonicalize_owned_path_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().expect("tempdir");
    let outside = tempdir().expect("outside");
    let outside_secret = outside.path().join("secret.txt");
    fs::write(&outside_secret, "top-secret").expect("write secret");

    fs::create_dir_all(dir.path().join("src")).expect("mkdir src");
    let link_path = dir.path().join("src/escaped");
    symlink(outside.path(), &link_path).expect("symlink");

    let err = canonicalize_owned_path(Some(dir.path()), "src/escaped/secret.txt")
        .expect_err("symlink escape must be blocked");
    assert!(
        matches!(err, SwarmLeaseError::SymlinkViolation(_)),
        "expected SymlinkViolation, got {err:?}"
    );
}

#[test]
fn coordination_mailbox_writes_one_job_never_and_rejects_stale_fencing_token() {
    let dir = tempdir().expect("tempdir");
    let mailbox = CoordinationMailbox::open(dir.path()).expect("open mailbox");

    let lease = WorkerLeaseRecord {
        lease_id: "lease-10-2-a".to_string(),
        project_id: "proj-10-2".to_string(),
        worker_id: "claude-builder-1".to_string(),
        worker_role: WorkerRole::Builder,
        worker_pgid: Some(42_000),
        fencing_token: 7,
        owned_paths: vec!["src/pages/Home.tsx".to_string()],
        expires_at: 20_000,
        released: false,
        created_at: 10_000,
    };

    let contract = OneJobContract {
        task_id: "task-10-2-1".to_string(),
        project_id: lease.project_id.clone(),
        worker_id: lease.worker_id.clone(),
        worker_role: WorkerRole::Builder,
        lease_id: lease.lease_id.clone(),
        fencing_token: lease.fencing_token,
        one_job: "Implement responsive Home hero layout".to_string(),
        never_rules: vec![
            "NEVER edit files outside owned_paths".to_string(),
            "NEVER self-issue PASS verdict".to_string(),
        ],
        owned_paths: lease.owned_paths.clone(),
        staging_rel_dir: ".cowok-design/coordination/staging/lease-10-2-a".to_string(),
        expires_at: lease.expires_at,
        issued_at: 10_000,
    };

    let envelope_path = mailbox
        .write_task_envelope(&contract)
        .expect("write envelope");
    assert!(envelope_path.exists());
    let envelope_json = fs::read_to_string(&envelope_path).expect("read envelope");
    assert!(envelope_json.contains("NEVER self-issue PASS verdict"));

    // Stage file within owned path
    mailbox
        .stage_file(
            &lease,
            "src/pages/Home.tsx",
            b"export const Home = () => null;\n",
        )
        .expect("stage file");

    // Attempting to stage outside owned paths fails
    let unowned = mailbox.stage_file(&lease, "src/pages/Other.tsx", b"oops");
    assert!(matches!(
        unowned,
        Err(SwarmLeaseError::PathNotOwnedByLease { .. })
    ));

    // Stale fencing token (6 != 7) MUST be rejected before touching workspace
    let stale_err = mailbox
        .commit_staged_file(&lease, 6, "src/pages/Home.tsx", 15_000)
        .expect_err("stale fencing token must fail");
    assert!(matches!(
        stale_err,
        SwarmLeaseError::StaleFencingToken {
            expected: 7,
            provided: 6
        }
    ));
    assert!(!dir.path().join("src/pages/Home.tsx").exists());

    // Expired lease MUST be rejected
    let expired_err = mailbox
        .commit_staged_file(&lease, 7, "src/pages/Home.tsx", 25_000)
        .expect_err("expired lease must fail");
    assert!(matches!(expired_err, SwarmLeaseError::LeaseExpired { .. }));

    // Active lease with matching fencing_token commits cleanly
    let receipt = mailbox
        .commit_staged_file(&lease, 7, "src/pages/Home.tsx", 15_000)
        .expect("valid commit must succeed");
    assert_eq!(receipt.fencing_token, 7);
    assert_eq!(receipt.canonical_path, "src/pages/Home.tsx");
    assert_eq!(
        fs::read_to_string(dir.path().join("src/pages/Home.tsx")).expect("committed file"),
        "export const Home = () => null;\n"
    );
}

#[test]
fn rate_limit_semaphore_clamps_3_to_1_on_429_and_recovers_after_clean_runs() {
    let sem = RateLimitSemaphore::new();
    assert_eq!(sem.current_limit(), INITIAL_CONCURRENCY);
    assert_eq!(INITIAL_CONCURRENCY, 3);

    // Can hold 3 concurrent permits initially, 4th blocked
    let p1 = sem.try_acquire().expect("permit 1");
    let p2 = sem.try_acquire().expect("permit 2");
    let p3 = sem.try_acquire().expect("permit 3");
    assert!(
        sem.try_acquire().is_none(),
        "4th permit must be denied at limit 3"
    );
    drop((p1, p2, p3));

    // HTTP 429 clamps 3 -> 1 immediately with bounded jittered exponential backoff
    let t1 = sem.on_http_429(None, 42);
    assert_eq!(t1.previous_limit, 3);
    assert_eq!(t1.current_limit, CLAMPED_CONCURRENCY);
    assert_eq!(CLAMPED_CONCURRENCY, 1);
    assert!(t1.backoff_ms >= 250 && t1.backoff_ms <= 30_000);
    assert!(sem.is_clamped());

    // While clamped, only 1 permit can be held at a time
    let only_one = sem.try_acquire().expect("permit 1 while clamped");
    assert!(
        sem.try_acquire().is_none(),
        "2nd permit must be blocked while clamped to 1"
    );
    drop(only_one);

    // Second consecutive 429 increases backoff monotonically (exponential)
    let t2 = sem.on_http_429(None, 42);
    assert_eq!(t2.current_limit, 1);
    assert!(t2.backoff_ms > t1.backoff_ms);

    // Clean successes recover 1 -> 2 -> 3
    for _ in 0..RECOVERY_SUCCESS_THRESHOLD {
        sem.on_success();
    }
    assert_eq!(sem.current_limit(), 2);
    for _ in 0..RECOVERY_SUCCESS_THRESHOLD {
        sem.on_success();
    }
    assert_eq!(sem.current_limit(), 3);
}

#[test]
fn expired_lease_reaper_kills_safe_worker_pgid_and_skips_unsafe_pgids() {
    let self_pgid = current_process_pgid().unwrap_or(99_999);
    let mut leases = vec![
        WorkerLeaseRecord {
            lease_id: "lease-expired-safe".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-slow".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: Some(77_777),
            fencing_token: 1,
            owned_paths: vec!["src/a.tsx".to_string()],
            expires_at: 1_000,
            released: false,
            created_at: 500,
        },
        WorkerLeaseRecord {
            lease_id: "lease-expired-unsafe-root".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-bad-pgid".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: Some(1),
            fencing_token: 2,
            owned_paths: vec!["src/b.tsx".to_string()],
            expires_at: 1_000,
            released: false,
            created_at: 500,
        },
        WorkerLeaseRecord {
            lease_id: "lease-expired-self-pgid".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-self-pgid".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: Some(self_pgid),
            fencing_token: 3,
            owned_paths: vec!["src/c.tsx".to_string()],
            expires_at: 1_000,
            released: false,
            created_at: 500,
        },
        WorkerLeaseRecord {
            lease_id: "lease-active".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-active".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: Some(88_888),
            fencing_token: 4,
            owned_paths: vec!["src/d.tsx".to_string()],
            expires_at: 9_000,
            released: false,
            created_at: 500,
        },
    ];

    let killer = RecordingPgidKiller::default();
    let outcomes = reap_expired_leases_and_kill_pgid(&mut leases, 2_000, &killer);

    assert_eq!(outcomes.len(), 3);
    assert_eq!(killer.killed_pgids(), vec![77_777]);
    assert_eq!(outcomes[0].killed_pgid, Some(77_777));
    assert!(!outcomes[0].skipped_unsafe_pgid);
    assert!(outcomes[0].released);
    assert!(outcomes[0].kill_error.is_none());
    assert_eq!(outcomes[1].killed_pgid, None);
    assert!(outcomes[1].skipped_unsafe_pgid);
    assert!(outcomes[1].released);
    assert_eq!(outcomes[2].killed_pgid, None);
    assert!(outcomes[2].skipped_unsafe_pgid);
    assert!(outcomes[2].released);

    assert!(leases[0].released);
    assert!(leases[1].released);
    assert!(leases[2].released);
    assert!(!leases[3].released);

    // After reaping expired lease-0, src/a.tsx is free again, while src/d.tsx is still locked
    assert!(
        check_conflict_against_active_leases(&["src/a.tsx".to_string()], &leases, 2_000).is_ok()
    );
    assert!(matches!(
        check_conflict_against_active_leases(&["src/d.tsx".to_string()], &leases, 2_000),
        Err(SwarmLeaseError::LeasePathConflict { .. })
    ));
}

#[test]
fn builder_neq_scorer_blocks_self_pass_spoofing() {
    // Same worker ID (case-insensitive)
    assert!(matches!(
        enforce_builder_neq_scorer("worker-claude-1", "WORKER-CLAUDE-1", WorkerRole::VerifierQc),
        Err(DesignJuryError::BuilderSelfPassForbidden { .. })
    ));
    // Different worker ID, but role is BUILDER trying to spoof PASS
    assert!(matches!(
        enforce_builder_neq_scorer("worker-claude-1", "worker-codex-2", WorkerRole::Builder),
        Err(DesignJuryError::BuilderSelfPassForbidden { .. })
    ));
    // Distinct worker ID with VERIFIER_QC or DESIGN_JURY succeeds
    assert!(
        enforce_builder_neq_scorer("worker-claude-1", "qc-verifier-1", WorkerRole::VerifierQc)
            .is_ok()
    );
    assert!(enforce_builder_neq_scorer(
        "worker-claude-1",
        "jury-session-1",
        WorkerRole::DesignJury
    )
    .is_ok());
}

#[test]
fn e1_e4_independent_verifier_achieves_at_least_95_percent_catch_rate() {
    let bench = canonical_e1_e4_benchmark_input();
    let eval = evaluate_e1_e4_evidence(&bench);

    assert!(
        eval.total_expected_defects >= 20,
        "canonical benchmark should cover >= 20 E1..E4 defects"
    );
    assert!(
        eval.catch_rate >= REQUIRED_E1_E4_CATCH_RATE,
        "expected E1..E4 catch rate >= 95%, got {:.2}% ({}/{})",
        eval.catch_rate * 100.0,
        eval.matched_expected_defects,
        eval.total_expected_defects
    );
    assert!(*eval.tier_violation_counts.get("E1").unwrap_or(&0) >= 5);
    assert!(*eval.tier_violation_counts.get("E2").unwrap_or(&0) >= 5);
    assert!(*eval.tier_violation_counts.get("E3").unwrap_or(&0) >= 4);
    assert!(*eval.tier_violation_counts.get("E4").unwrap_or(&0) >= 4);
}

#[test]
fn tier3_design_jury_enforces_generation_fence_and_escalates_recurring_violations_to_ledger() {
    let freeze_sha = "a".repeat(64);
    let session_input = DesignJurySessionInput {
        builder_worker_id: "builder-claude-1".to_string(),
        scorer_worker_id: "jury-verifier-1".to_string(),
        scorer_role: WorkerRole::DesignJury,
        freeze_bundle: ReviewFreezeBundle {
            project_id: "proj-10-2".to_string(),
            screen_id: "screen-checkout".to_string(),
            generation_fence: 5,
            freeze_snapshot_sha256: freeze_sha.clone(),
            frozen_html: Some(
                "<html><body><main><h1>Checkout</h1><p>Lorem ipsum</p></main></body></html>"
                    .to_string(),
            ),
            frozen_at: 10_000,
        },
        qc_input: QcRunInput {
            qc_run_id: Some("qc-run-55".to_string()),
            screen_id: "screen-checkout".to_string(),
            html_snapshot: None,
            reported_issues: vec![QcIssueInput {
                rule_id: "E3_WCAG_AA_CONTRAST_FAIL".to_string(),
                tier: EvidenceTier::E3,
                severity: "CRITICAL".to_string(),
                selector: Some(".cta-btn".to_string()),
                message: "Contrast 2.1:1 below 4.5:1".to_string(),
            }],
            expected_defect_ids: vec![],
            contrast_ratios: vec![],
            bounding_boxes: vec![],
            brand_tokens_expected: vec![],
        },
    };

    // Stale generation fence (provided 5 vs expected 6) MUST be rejected
    let stale = evaluate_design_jury_session(&session_input, 6, &BTreeMap::new())
        .expect_err("stale generation fence must fail");
    assert!(matches!(
        stale,
        DesignJuryError::StaleGenerationFence {
            expected: 6,
            provided: 5
        }
    ));

    // First occurrence -> FAIL with all 4 lenses evaluated, not yet escalated
    let first = evaluate_design_jury_session(&session_input, 5, &BTreeMap::new())
        .expect("first jury evaluation");
    assert_eq!(first.report.verdict, JuryVerdictStatus::Fail);
    assert!(!first.report.escalated_to_ledger);
    assert!(first.ledger_escalations.is_empty());
    assert_eq!(first.report.lens_scores.len(), JuryLens::ALL.len());

    // Second occurrence (prior = 1 -> recurrence_count = 2 >= 2) -> ESCALATED_TO_LEDGER (HUMAN_ONLY)
    let contrast_fp = compute_violation_fingerprint(
        EvidenceTier::E3,
        "E3_WCAG_AA_CONTRAST_FAIL",
        Some(".cta-btn"),
    );
    let mut prior = BTreeMap::new();
    prior.insert(contrast_fp.clone(), 1usize);

    let second =
        evaluate_design_jury_session(&session_input, 5, &prior).expect("second jury evaluation");
    assert_eq!(second.report.verdict, JuryVerdictStatus::EscalatedToLedger);
    assert!(second.report.escalated_to_ledger);
    assert_eq!(second.ledger_escalations.len(), 1);
    assert_eq!(second.ledger_escalations[0].fingerprint_sha256, contrast_fp);
    assert_eq!(second.ledger_escalations[0].hit_count, 2);
    assert_eq!(
        second.ledger_escalations[0].approval_policy,
        HUMAN_ONLY_APPROVAL_POLICY
    );
    assert_eq!(second.ledger_escalations[0].status, "PROPOSED");
}

#[cfg(unix)]
#[test]
fn stage_and_commit_block_dot_lease_id_and_intermediate_staging_symlink_escape() {
    use std::os::unix::fs::symlink;

    // HIGH-02: reject "." in validate_safe_worker_id and bad lease_id at top of commit_staged_file.
    assert!(matches!(
        validate_safe_worker_id(".", "leaseId"),
        Err(SwarmLeaseError::InvalidContract(_))
    ));
    assert!(matches!(
        validate_safe_worker_id("..", "leaseId"),
        Err(SwarmLeaseError::InvalidContract(_))
    ));

    let dir = tempdir().expect("tempdir");
    let outside = tempdir().expect("outside");
    let outside_secret = outside.path().join("secret.txt");
    fs::write(&outside_secret, "TOP_SECRET_DATA").expect("write outside secret");

    let mailbox = CoordinationMailbox::open(dir.path()).expect("open mailbox");

    let bad_lease = WorkerLeaseRecord {
        lease_id: ".".to_string(),
        project_id: "p1".to_string(),
        worker_id: "w1".to_string(),
        worker_role: WorkerRole::Builder,
        worker_pgid: None,
        fencing_token: 1,
        owned_paths: vec!["src/pages/Home.tsx".to_string()],
        expires_at: 20_000,
        released: false,
        created_at: 10_000,
    };
    assert!(matches!(
        mailbox.stage_file(&bad_lease, "src/pages/Home.tsx", b"x"),
        Err(SwarmLeaseError::InvalidContract(_))
    ));
    assert!(matches!(
        mailbox.commit_staged_file(&bad_lease, 1, "src/pages/Home.tsx", 15_000),
        Err(SwarmLeaseError::InvalidContract(_))
    ));

    let lease = WorkerLeaseRecord {
        lease_id: "lease-symlink-test".to_string(),
        project_id: "p1".to_string(),
        worker_id: "w1".to_string(),
        worker_role: WorkerRole::Builder,
        worker_pgid: None,
        fencing_token: 2,
        owned_paths: vec!["src/pages/Home.tsx".to_string()],
        expires_at: 20_000,
        released: false,
        created_at: 10_000,
    };

    // Intermediate symlink inside staging/<lease_id>/src -> outside_dir must be blocked BEFORE mkdir/write.
    let lease_stage_dir = dir
        .path()
        .join(".cowok-design/coordination/staging")
        .join(&lease.lease_id);
    fs::create_dir_all(&lease_stage_dir).expect("mkdir lease stage dir");
    symlink(outside.path(), lease_stage_dir.join("src")).expect("symlink intermediate staging dir");

    let stage_err = mailbox
        .stage_file(&lease, "src/pages/Home.tsx", b"evil")
        .expect_err("intermediate staging symlink must be blocked");
    assert!(
        matches!(stage_err, SwarmLeaseError::SymlinkViolation(_)),
        "expected SymlinkViolation on stage_file, got {stage_err:?}"
    );
    assert!(
        !outside.path().join("pages").exists(),
        "stage_file must not create directories through a symlink"
    );

    let commit_inter_err = mailbox
        .commit_staged_file(&lease, 2, "src/pages/Home.tsx", 15_000)
        .expect_err("intermediate staging symlink must be blocked on commit");
    assert!(
        matches!(commit_inter_err, SwarmLeaseError::SymlinkViolation(_)),
        "expected SymlinkViolation on commit_staged_file, got {commit_inter_err:?}"
    );

    // Replace intermediate symlink with real dirs, then put a symlink at the staged leaf file itself.
    fs::remove_file(lease_stage_dir.join("src")).expect("remove dir symlink");
    fs::create_dir_all(lease_stage_dir.join("src/pages")).expect("mkdir real staging pages");
    symlink(&outside_secret, lease_stage_dir.join("src/pages/Home.tsx"))
        .expect("symlink leaf staged file");

    let commit_leaf_err = mailbox
        .commit_staged_file(&lease, 2, "src/pages/Home.tsx", 15_000)
        .expect_err("leaf staging symlink must be blocked by O_NOFOLLOW");
    assert!(
        matches!(commit_leaf_err, SwarmLeaseError::SymlinkViolation(_)),
        "expected SymlinkViolation on leaf staged symlink, got {commit_leaf_err:?}"
    );
    assert!(
        !dir.path().join("src/pages/Home.tsx").exists(),
        "commit_staged_file must not copy symlinked secret into workspace"
    );
}

struct SelectiveFailingPgidKiller {
    fail_pgid: i32,
    killed: Arc<Mutex<Vec<i32>>>,
}

impl PgidKiller for SelectiveFailingPgidKiller {
    fn kill_pgid(&self, pgid: i32) -> Result<(), SwarmLeaseError> {
        if pgid == self.fail_pgid {
            return Err(SwarmLeaseError::Io(format!("EPERM signalling -{pgid}")));
        }
        self.killed.lock().unwrap().push(pgid);
        Ok(())
    }
}

#[test]
fn expired_lease_reaper_continues_after_kill_pgid_error_and_only_skips_failed_lease() {
    let mut leases = vec![
        WorkerLeaseRecord {
            lease_id: "lease-kill-fail".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-fail".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: Some(55_001),
            fencing_token: 1,
            owned_paths: vec!["src/fail.tsx".to_string()],
            expires_at: 1_000,
            released: false,
            created_at: 500,
        },
        WorkerLeaseRecord {
            lease_id: "lease-kill-ok".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-ok".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: Some(55_002),
            fencing_token: 2,
            owned_paths: vec!["src/ok.tsx".to_string()],
            expires_at: 1_000,
            released: false,
            created_at: 500,
        },
        WorkerLeaseRecord {
            lease_id: "lease-no-pgid".to_string(),
            project_id: "p1".to_string(),
            worker_id: "w-nopgid".to_string(),
            worker_role: WorkerRole::Builder,
            worker_pgid: None,
            fencing_token: 3,
            owned_paths: vec!["src/nopgid.tsx".to_string()],
            expires_at: 1_000,
            released: false,
            created_at: 500,
        },
    ];

    let killer = SelectiveFailingPgidKiller {
        fail_pgid: 55_001,
        killed: Arc::new(Mutex::new(Vec::new())),
    };
    let outcomes = reap_expired_leases_and_kill_pgid(&mut leases, 2_000, &killer);

    assert_eq!(outcomes.len(), 3);
    assert!(!outcomes[0].released);
    assert_eq!(outcomes[0].killed_pgid, None);
    assert!(outcomes[0]
        .kill_error
        .as_deref()
        .unwrap_or("")
        .contains("EPERM"));
    assert!(!leases[0].released);

    assert!(outcomes[1].released);
    assert_eq!(outcomes[1].killed_pgid, Some(55_002));
    assert!(outcomes[1].kill_error.is_none());
    assert!(leases[1].released);

    assert!(outcomes[2].released);
    assert_eq!(outcomes[2].killed_pgid, None);
    assert!(outcomes[2].kill_error.is_none());
    assert!(leases[2].released);

    assert_eq!(*killer.killed.lock().unwrap(), vec![55_002]);
}

#[test]
fn rate_limit_semaphore_is_clamped_matches_snapshot_during_partial_recovery() {
    let sem = RateLimitSemaphore::new();
    assert!(!sem.is_clamped());
    assert_eq!(sem.is_clamped(), sem.snapshot().clamped);

    let _ = sem.on_http_429(None, 7);
    assert!(sem.is_clamped());
    assert_eq!(sem.is_clamped(), sem.snapshot().clamped);

    // After 1 clean success, consecutive_429 resets to 0 while limit is still 1 (< 3).
    sem.on_success();
    assert_eq!(sem.current_limit(), 1);
    assert!(sem.is_clamped());
    assert_eq!(sem.is_clamped(), sem.snapshot().clamped);

    // After reaching threshold, limit steps 1 -> 2 (< 3): still clamped!
    for _ in 1..RECOVERY_SUCCESS_THRESHOLD {
        sem.on_success();
    }
    assert_eq!(sem.current_limit(), 2);
    assert!(sem.is_clamped());
    assert_eq!(sem.is_clamped(), sem.snapshot().clamped);

    // After another threshold, limit recovers 2 -> 3 (== initial_limit): no longer clamped.
    for _ in 0..RECOVERY_SUCCESS_THRESHOLD {
        sem.on_success();
    }
    assert_eq!(sem.current_limit(), INITIAL_CONCURRENCY);
    assert!(!sem.is_clamped());
    assert_eq!(sem.is_clamped(), sem.snapshot().clamped);
}
