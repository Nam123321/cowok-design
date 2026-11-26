// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (End-to-End Sealed Anti-Goodhart Benchmark Runner & 25-Case /simulate-user M1..M5 Certification Integration Tests)
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::cli::{commands::execute_cli_command, parse_cli_args};
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::eval_runner::{
    acquire_eval_single_flight, list_eval_history, seal_sealed_eval_dir,
    validate_candidate_dir_in_workspace, verify_sealed_eval_integrity, EVAL_RUN_LOCK_FILENAME,
    GATE_HISTORY_REL_PATH, MAX_SEALED_FILE_BYTES, PINNED_SEALED_MANIFEST_SHA256,
    SEALED_EVAL_REL_DIR, SEALED_MANIFEST_ENV_VAR,
};
use harness_daemon::eval_runner_suites::{
    canonical_sealed_fixture_files, compute_eval_verdict, evaluate_sim_user_25_cases,
    inspect_design_craft_package, parse_semver_v1, run_independent_scorer_detector, sha256_hex,
    DecisionCalibrationSummary, DecisionProviderCalibration, DecisionProviderId, EvalMilestoneId,
    EvalTier, EvalVerdict, EvalVerdictInput, FaultInjectionSummary, PairedAblationSummary,
    SimCaseResult, SimCaseStatus, SimEvalContext, MANDATORY_SECURITY_GATE_CASE_IDS,
    MAX_PLUGIN_JSON_BYTES, PR_TIER_MAX_DURATION_MS,
};
use harness_daemon::lockfile::CURRENT_SCHEMA_VERSION;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

/// Serializes tests that mutate process-global `EVAL_RUN_IN_FLIGHT` or `COWOK_SEALED_MANIFEST_SHA256`.
fn eval_test_mutex() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn materialize_canonical_sealed_dir(workspace_root: &Path) {
    let sealed_dir = workspace_root.join(SEALED_EVAL_REL_DIR);
    fs::create_dir_all(&sealed_dir).expect("mkdir sealed dir");
    for (name, contents) in canonical_sealed_fixture_files() {
        fs::write(sealed_dir.join(name), contents).expect("write sealed fixture");
    }
}

fn materialize_valid_design_craft(workspace_root: &Path, version: &str, include_dataviz: bool) {
    let dc_dir = workspace_root.join("packages/design-craft");
    fs::create_dir_all(&dc_dir).expect("mkdir design-craft");
    let phase3 = if include_dataviz {
        json!(["impeccable-polish", "dashboard-ux", "data-viz-storyteller"])
    } else {
        json!(["impeccable-polish"])
    };
    let peers = if include_dataviz {
        json!([{ "id": "dashboard-design-upstream" }])
    } else {
        json!([])
    };
    let plugin = json!({
        "name": "design-craft",
        "version": version,
        "phaseSkillMap": {
            "phase-3": phase3
        },
        "optionalPeers": peers
    });
    fs::write(
        dc_dir.join("plugin.json"),
        serde_json::to_string_pretty(&plugin).unwrap(),
    )
    .expect("write plugin.json");
}

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let editor_tok = format!("sess-131-ed-{}", Uuid::new_v4());
    let viewer_tok = format!("sess-131-vw-{}", Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "certifier-1".into(),
            role: "editor".into(),
        },
        exp,
    );
    state.auth.insert_session(
        viewer_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-1".into(),
            role: "viewer".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, editor_tok, viewer_tok)
}

async fn send_json(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170");
    if let Some(t) = tok {
        b = b.header("x-cowok-session-token", t);
    }
    if !raw.is_empty() {
        b = b.header("content-type", "application/json");
    }
    let resp = router
        .clone()
        .oneshot(b.body(Body::from(raw)).unwrap())
        .await
        .unwrap();
    let st = resp.status();
    let out = resp.into_body().collect().await.unwrap().to_bytes();
    (
        st,
        serde_json::from_slice(&out).unwrap_or(serde_json::Value::Null),
    )
}

#[test]
fn ac01_pinned_manifest_sha256_matches_canonical_bundle_and_blocks_all_tamper_vectors() {
    let _guard = eval_test_mutex().blocking_lock();
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    assert_eq!(CURRENT_SCHEMA_VERSION, 33);

    // 1. Canonical manifest digest MUST match compile-time out-of-band pin
    let files = canonical_sealed_fixture_files();
    let canonical_manifest_sha = sha256_hex(files["manifest.json"].as_bytes());
    assert_eq!(
        PINNED_SEALED_MANIFEST_SHA256, canonical_manifest_sha,
        "PINNED_SEALED_MANIFEST_SHA256 must match canonical manifest.json SHA-256 ({canonical_manifest_sha})"
    );

    // 2. Missing sealed dir fails closed
    let empty_tmp = TempDir::new().unwrap();
    let missing_err = verify_sealed_eval_integrity(empty_tmp.path()).unwrap_err();
    assert_eq!(missing_err.reason, "SEALED_DIR_MISSING");

    // 3. Fresh 0644 checkout fails closed on WRITABLE_SEALED_FILE before sealing
    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    #[cfg(unix)]
    {
        let writable_err = verify_sealed_eval_integrity(tmp.path()).unwrap_err();
        assert_eq!(writable_err.reason, "WRITABLE_SEALED_FILE");
    }

    // 4. seal_sealed_eval_dir verifies hashes FIRST then chmod 0444s cleanly
    let sealed_snap = seal_sealed_eval_dir(tmp.path()).expect("clean seal must succeed");
    assert!(sealed_snap.sealed_integrity_ok);
    assert_eq!(sealed_snap.sealed_manifest_sha256, canonical_manifest_sha);
    assert_eq!(sealed_snap.sealed_files_count, 5);
    let verified = verify_sealed_eval_integrity(tmp.path()).expect("post-seal verify passes");
    assert_eq!(verified.briefs.briefs.len(), 14);
    assert_eq!(verified.faults.faults.len(), 22);
    assert_eq!(verified.sim_cases.cases.len(), 25);

    // 5. Tampering extra unlisted file fails closed
    let extra_path = tmp.path().join(SEALED_EVAL_REL_DIR).join("extra.json");
    fs::write(&extra_path, "{}").unwrap();
    let extra_err = verify_sealed_eval_integrity(tmp.path()).unwrap_err();
    assert_eq!(extra_err.reason, "UNEXPECTED_FILE_IN_SEALED_DIR");
    fs::remove_file(&extra_path).unwrap();

    // 6. Symlink inside sealed dir fails closed
    #[cfg(unix)]
    {
        let sym_path = tmp.path().join(SEALED_EVAL_REL_DIR).join("evil-link.json");
        std::os::unix::fs::symlink(
            tmp.path().join(SEALED_EVAL_REL_DIR).join("manifest.json"),
            &sym_path,
        )
        .unwrap();
        let sym_err = verify_sealed_eval_integrity(tmp.path()).unwrap_err();
        assert_eq!(sym_err.reason, "SYMLINK_DETECTED");
        fs::remove_file(&sym_path).unwrap();
    }

    // 7. Modifying a fixture byte (even if re-chmodded 0444) fails closed
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let briefs_path = tmp.path().join(SEALED_EVAL_REL_DIR).join("briefs.json");
        fs::set_permissions(&briefs_path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut content = fs::read_to_string(&briefs_path).unwrap();
        content.push(' ');
        fs::write(&briefs_path, content).unwrap();
        fs::set_permissions(&briefs_path, fs::Permissions::from_mode(0o444)).unwrap();
        let mod_err = verify_sealed_eval_integrity(tmp.path()).unwrap_err();
        assert_eq!(mod_err.reason, "FIXTURE_SHA256_MISMATCH");

        // Even attempting `seal` on tampered dir refuses to seal!
        let seal_err = seal_sealed_eval_dir(tmp.path()).unwrap_err();
        assert_eq!(seal_err.reason, "FIXTURE_SHA256_MISMATCH");
    }

    // 8. Malformed COWOK_SEALED_MANIFEST_SHA256 env override fails closed
    std::env::set_var(SEALED_MANIFEST_ENV_VAR, "not-a-valid-64-hex");
    let env_err = verify_sealed_eval_integrity(tmp.path()).unwrap_err();
    assert_eq!(env_err.reason, "MALFORMED_PINNED_MANIFEST_SHA256");
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);
}

#[tokio::test]
async fn ac02_ac03_ac04_http_run_history_single_flight_and_tamper_logging() {
    let _guard = eval_test_mutex().lock().await;
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    materialize_valid_design_craft(tmp.path(), "1.0.0", true);
    seal_sealed_eval_dir(tmp.path()).expect("seal");

    let (state, router, editor_tok, viewer_tok) = setup_daemon(&tmp);

    // 1. Unauthenticated rejected (401); viewer POST rejected (403)
    let (st_unauth, _) = send_json(&router, "GET", "/api/v1/eval/history", None, None).await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);
    let (st_viewer, _) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&viewer_tok),
        Some(json!({ "tier": "pr" })),
    )
    .await;
    assert_eq!(st_viewer, StatusCode::FORBIDDEN);

    // 2. PR tier run passes <15s, $0.00, 14 briefs (>=3 Data Storytelling), 3-provider 0 float leaks, 22/22 faults (>=95%), 25/25 sim cases
    let (st_run, run_body) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr", "runSimulatedUsers": true })),
    )
    .await;
    assert_eq!(st_run, StatusCode::OK, "body={run_body}");
    assert_eq!(run_body["ok"], true);
    let data = &run_body["data"];
    assert_eq!(data["tier"], "pr");
    assert_eq!(data["offlineEnforced"], true);
    assert_eq!(data["verdict"], "PASS");
    assert_eq!(data["manifestPinSource"], "compiled_binary");
    assert_eq!(data["tamperDetected"], false);
    assert!(data["tamperReason"].is_null());
    assert!(data["durationMs"].as_u64().unwrap() < 15_000);
    assert_eq!(data["estimatedCostUsd"].as_f64().unwrap(), 0.0);
    assert_eq!(data["simCasesPassed"], 25);
    assert_eq!(data["simCasesBlocked"], 0);
    assert_eq!(data["simCasesGap"], 0);
    assert_eq!(data["simCasesTotal"], 25);
    assert_eq!(data["securityGatesPassed"], true);
    assert_eq!(data["securityBlockers"].as_array().unwrap().len(), 0);
    assert_eq!(data["milestones"].as_array().unwrap().len(), 5);
    assert_eq!(data["simCases"].as_array().unwrap().len(), 25);
    assert!(data["ablation"]["briefsEvaluated"].as_u64().unwrap() >= 12);
    assert!(
        data["ablation"]["dataStorytellingBriefsCount"]
            .as_u64()
            .unwrap()
            >= 3
    );
    assert!(data["ablation"]["liftPercent"].as_f64().unwrap() > 0.0);
    assert_eq!(data["ablation"]["regressionsCount"], 0);
    assert_eq!(data["ablation"]["designCraftSemverPassed"], true);
    assert_eq!(
        data["calibration"]["providers"].as_array().unwrap().len(),
        3
    );
    assert_eq!(data["calibration"]["totalFloatLeaks"], 0);
    assert_eq!(data["calibration"]["passed"], true);
    assert!(data["faultInjection"]["injectedCount"].as_u64().unwrap() >= 20);
    assert!(data["faultInjection"]["catchRatePercent"].as_f64().unwrap() >= 95.0);
    assert_eq!(data["faultInjection"]["builderScorerIsolated"], true);

    // 3. Verify V33 SQLite persistence + .cowok-design/evals/gate-history.jsonl atomic append
    let conn = state.registry.pool().connection().unwrap();
    let db_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM eval_benchmark_runs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(db_count, 1);

    let ledger = fs::read_to_string(tmp.path().join(GATE_HISTORY_REL_PATH)).expect("jsonl ledger");
    let lines: Vec<&str> = ledger.lines().collect();
    assert_eq!(lines.len(), 1);
    let parsed_line: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed_line["runId"], data["runId"]);
    assert_eq!(parsed_line["verdict"], "PASS");

    // 4. GET /api/v1/eval/history returns live integrity + persisted run
    let (st_hist, hist_body) = send_json(
        &router,
        "GET",
        "/api/v1/eval/history?limit=10&tier=pr",
        Some(&editor_tok),
        None,
    )
    .await;
    assert_eq!(st_hist, StatusCode::OK);
    assert_eq!(hist_body["data"]["sealedIntegrityOk"], true);
    assert_eq!(hist_body["data"]["totalRuns"], 1);
    assert_eq!(hist_body["data"]["runs"].as_array().unwrap().len(), 1);
    assert_eq!(hist_body["data"]["latestRun"]["runId"], data["runId"]);

    // 5. Single-flight lock returns 409 EVAL_ALREADY_RUNNING when held
    let sf_guard = acquire_eval_single_flight().expect("acquire lock");
    let (st_409, conflict_body) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr" })),
    )
    .await;
    assert_eq!(st_409, StatusCode::CONFLICT);
    assert_eq!(conflict_body["error"]["code"], "EVAL_ALREADY_RUNNING");
    drop(sf_guard);

    // 6. Candidate design-craft dropping data-viz-storyteller triggers ADR-009 regression FAIL
    let bad_dc = tmp.path().join("candidate-bad-dc");
    fs::create_dir_all(&bad_dc).unwrap();
    fs::write(
        bad_dc.join("plugin.json"),
        serde_json::to_string_pretty(&json!({
            "name": "design-craft",
            "version": "2.0.0",
            "phaseSkillMap": { "phase-3": ["impeccable-polish"] },
            "optionalPeers": []
        }))
        .unwrap(),
    )
    .unwrap();
    let (st_reg, reg_body) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({
            "tier": "stage",
            "designCraftCandidateDir": "candidate-bad-dc"
        })),
    )
    .await;
    assert_eq!(st_reg, StatusCode::OK);
    assert_eq!(reg_body["data"]["verdict"], "FAIL");
    assert_eq!(
        reg_body["data"]["ablation"]["designCraftSemverPassed"],
        false
    );
    assert!(
        reg_body["data"]["ablation"]["regressionsCount"]
            .as_u64()
            .unwrap()
            > 0
    );

    // 7. Path traversal on designCraftCandidateDir rejected with 400 VALIDATION_ERROR
    let (st_trav, trav_body) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({
            "tier": "stage",
            "designCraftCandidateDir": "../../etc"
        })),
    )
    .await;
    assert_eq!(st_trav, StatusCode::BAD_REQUEST);
    assert_eq!(trav_body["error"]["code"], "VALIDATION_ERROR");

    // 8. Tampering sealed file returns 422 SEALED_EVAL_TAMPER_ERROR and logs tamper run to V33 & JSONL
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mf = tmp.path().join(SEALED_EVAL_REL_DIR).join("manifest.json");
        fs::set_permissions(&mf, fs::Permissions::from_mode(0o644)).unwrap();
        let (st_tamp, tamp_body) = send_json(
            &router,
            "POST",
            "/api/v1/eval/run",
            Some(&editor_tok),
            Some(json!({ "tier": "release" })),
        )
        .await;
        assert_eq!(st_tamp, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(tamp_body["error"]["code"], "SEALED_EVAL_TAMPER_ERROR");
        assert_eq!(
            tamp_body["error"]["details"]["tamperReason"],
            "WRITABLE_SEALED_FILE"
        );

        let db_tamper_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM eval_benchmark_runs WHERE verdict = 'SEALED_EVAL_TAMPER_ERROR'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(db_tamper_count, 1);
    }
}

#[test]
fn ac05_mandatory_security_gates_cannot_be_masked_by_24_of_25_score() {
    let ablation = PairedAblationSummary {
        briefs_evaluated: 14,
        data_storytelling_briefs_count: 4,
        baseline_mean_score: 20.0,
        with_skill_mean_score: 96.0,
        lift_percent: 380.0,
        regressions_count: 0,
        design_craft_semver_passed: true,
        design_craft_version: "1.0.0".to_string(),
    };
    let calibration = DecisionCalibrationSummary {
        providers: vec![
            DecisionProviderCalibration {
                provider_id: DecisionProviderId::LocalFts5,
                cases_evaluated: 3,
                accuracy_percent: 100.0,
                float_leaks_count: 0,
                passed: true,
            },
            DecisionProviderCalibration {
                provider_id: DecisionProviderId::GeminiDecisionJev,
                cases_evaluated: 3,
                accuracy_percent: 100.0,
                float_leaks_count: 0,
                passed: true,
            },
            DecisionProviderCalibration {
                provider_id: DecisionProviderId::DeterministicMcda,
                cases_evaluated: 3,
                accuracy_percent: 100.0,
                float_leaks_count: 0,
                passed: true,
            },
        ],
        total_float_leaks: 0,
        passed: true,
    };
    let fault_injection = FaultInjectionSummary {
        injected_count: 22,
        caught_count: 22,
        catch_rate_percent: 100.0,
        builder_scorer_isolated: true,
        passed: true,
    };

    // Build 25 passing cases, then fail ONLY UC-03 (M1_SECRET_INTERCEPT) -> 24/25 passed!
    let mut cases: Vec<SimCaseResult> = (1..=25)
        .map(|i| {
            let milestone = match i {
                1..=5 => EvalMilestoneId::M1,
                6..=10 => EvalMilestoneId::M2,
                11..=15 => EvalMilestoneId::M3,
                16..=20 => EvalMilestoneId::M4,
                _ => EvalMilestoneId::M5,
            };
            let (mandatory, key) = match i {
                3 => (true, Some("M1_SECRET_INTERCEPT".to_string())),
                9 => (true, Some("M2_CSP_XSS_ZERO_SPLIT_BRAIN".to_string())),
                14 => (true, Some("M3_RCE_TOOL_JAIL".to_string())),
                20 => (true, Some("M4_ZERO_FLOAT_LEAK".to_string())),
                23 => (true, Some("M5_ZERO_OFFLINE_404".to_string())),
                25 => (true, Some("M5_SEALED_TAMPER_GUARD".to_string())),
                _ => (false, None),
            };
            SimCaseResult {
                case_id: format!("UC-{i:02}"),
                milestone,
                title_key: format!("evalCertification.cases.UC_{i:02}"),
                mandatory_security_gate: mandatory,
                security_gate_key: key,
                status: SimCaseStatus::Pass,
                duration_ms: 1,
                evidence_summary: "ok".to_string(),
            }
        })
        .collect();

    // Case A: 24/25 pass with UC-03 failed -> MUST FAIL
    cases[2].status = SimCaseStatus::Fail;
    let (verdict_uc03, sec_ok_uc03, blockers_uc03) = compute_eval_verdict(&EvalVerdictInput {
        tier: EvalTier::Release,
        offline_enforced: false,
        duration_ms: 120,
        estimated_cost_usd: 0.0,
        ablation: &ablation,
        calibration: &calibration,
        fault_injection: &fault_injection,
        sim_cases: &cases,
        run_simulated_users: true,
    });
    assert_eq!(verdict_uc03, EvalVerdict::Fail);
    assert!(!sec_ok_uc03);
    assert!(blockers_uc03.contains(&"UC-03:M1_SECRET_INTERCEPT".to_string()));

    // Case B: 24/25 pass with UC-14 (M3_RCE_TOOL_JAIL) failed -> MUST FAIL
    cases[2].status = SimCaseStatus::Pass;
    cases[13].status = SimCaseStatus::Fail;
    let (verdict_uc14, sec_ok_uc14, blockers_uc14) = compute_eval_verdict(&EvalVerdictInput {
        tier: EvalTier::Release,
        offline_enforced: false,
        duration_ms: 120,
        estimated_cost_usd: 0.0,
        ablation: &ablation,
        calibration: &calibration,
        fault_injection: &fault_injection,
        sim_cases: &cases,
        run_simulated_users: true,
    });
    assert_eq!(verdict_uc14, EvalVerdict::Fail);
    assert!(!sec_ok_uc14);
    assert!(blockers_uc14.contains(&"UC-14:M3_RCE_TOOL_JAIL".to_string()));

    // Case C: 24/25 pass with non-security case UC-04 failed -> PASS (>=22/25 & all security gates pass)
    cases[13].status = SimCaseStatus::Pass;
    cases[3].status = SimCaseStatus::Fail;
    let (verdict_non_sec, sec_ok_non_sec, blockers_non_sec) =
        compute_eval_verdict(&EvalVerdictInput {
            tier: EvalTier::Release,
            offline_enforced: false,
            duration_ms: 120,
            estimated_cost_usd: 0.0,
            ablation: &ablation,
            calibration: &calibration,
            fault_injection: &fault_injection,
            sim_cases: &cases,
            run_simulated_users: true,
        });
    assert_eq!(verdict_non_sec, EvalVerdict::Pass);
    assert!(sec_ok_non_sec);
    assert!(blockers_non_sec.is_empty());
}

#[test]
fn cli_eval_seal_run_history_exit_codes_and_json_parity() {
    let _guard = eval_test_mutex().blocking_lock();
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    materialize_valid_design_craft(tmp.path(), "1.1.0", true);
    let ws = tmp.path().to_string_lossy().into_owned();

    // 1. Before seal -> eval run fails closed with exit code 3 (SEALED_EVAL_TAMPER_ERROR)
    #[cfg(unix)]
    {
        let cmd_unsealed = parse_cli_args(&[
            "eval".into(),
            "run".into(),
            "--tier".into(),
            "pr".into(),
            "--workspace".into(),
            ws.clone(),
            "--json".into(),
        ])
        .unwrap();
        let out_unsealed = execute_cli_command(cmd_unsealed);
        assert_eq!(out_unsealed.exit_code, 3);
        let parsed: serde_json::Value = serde_json::from_str(&out_unsealed.stdout).unwrap();
        assert_eq!(parsed["error"]["code"], "SEALED_EVAL_TAMPER_ERROR");
    }

    // 2. `cowok-design eval seal --json` -> exit 0
    let cmd_seal = parse_cli_args(&[
        "eval".into(),
        "seal".into(),
        "--workspace".into(),
        ws.clone(),
        "--json".into(),
    ])
    .unwrap();
    let out_seal = execute_cli_command(cmd_seal);
    assert_eq!(out_seal.exit_code, 0, "stderr={}", out_seal.stderr);
    let seal_json: serde_json::Value = serde_json::from_str(&out_seal.stdout).unwrap();
    assert_eq!(seal_json["ok"], true);
    assert_eq!(seal_json["data"]["sealedIntegrityOk"], true);

    // 3. `cowok-design eval run --tier pr --json` -> exit 0
    let cmd_run = parse_cli_args(&[
        "eval".into(),
        "run".into(),
        "--tier".into(),
        "pr".into(),
        "--workspace".into(),
        ws.clone(),
        "--json".into(),
    ])
    .unwrap();
    let out_run = execute_cli_command(cmd_run);
    assert_eq!(out_run.exit_code, 0, "stdout={}", out_run.stdout);
    let run_json: serde_json::Value = serde_json::from_str(&out_run.stdout).unwrap();
    assert_eq!(run_json["ok"], true);
    assert_eq!(run_json["data"]["verdict"], "PASS");
    assert_eq!(run_json["data"]["offlineEnforced"], true);

    // 4. Single-flight lock -> CLI exits 4 (EVAL_ALREADY_RUNNING)
    let sf = acquire_eval_single_flight().unwrap();
    let cmd_locked = parse_cli_args(&[
        "eval".into(),
        "run".into(),
        "--tier".into(),
        "pr".into(),
        "--workspace".into(),
        ws.clone(),
        "--json".into(),
    ])
    .unwrap();
    let out_locked = execute_cli_command(cmd_locked);
    assert_eq!(out_locked.exit_code, 4);
    drop(sf);

    // 5. `cowok-design eval history --limit 10 --json` -> exit 0 with parity envelope
    let cmd_hist = parse_cli_args(&[
        "eval".into(),
        "history".into(),
        "--limit".into(),
        "10".into(),
        "--workspace".into(),
        ws,
        "--json".into(),
    ])
    .unwrap();
    let out_hist = execute_cli_command(cmd_hist);
    assert_eq!(out_hist.exit_code, 0);
    let hist_json: serde_json::Value = serde_json::from_str(&out_hist.stdout).unwrap();
    assert_eq!(hist_json["ok"], true);
    assert_eq!(hist_json["data"]["sealedIntegrityOk"], true);
    assert!(hist_json["data"]["totalRuns"].as_u64().unwrap() >= 1);
}

fn passing_summaries() -> (
    PairedAblationSummary,
    DecisionCalibrationSummary,
    FaultInjectionSummary,
) {
    let ablation = PairedAblationSummary {
        briefs_evaluated: 14,
        data_storytelling_briefs_count: 4,
        baseline_mean_score: 20.0,
        with_skill_mean_score: 96.0,
        lift_percent: 380.0,
        regressions_count: 0,
        design_craft_semver_passed: true,
        design_craft_version: "1.0.0".to_string(),
    };
    let calibration = DecisionCalibrationSummary {
        providers: vec![
            DecisionProviderCalibration {
                provider_id: DecisionProviderId::LocalFts5,
                cases_evaluated: 3,
                accuracy_percent: 100.0,
                float_leaks_count: 0,
                passed: true,
            },
            DecisionProviderCalibration {
                provider_id: DecisionProviderId::GeminiDecisionJev,
                cases_evaluated: 3,
                accuracy_percent: 100.0,
                float_leaks_count: 0,
                passed: true,
            },
            DecisionProviderCalibration {
                provider_id: DecisionProviderId::DeterministicMcda,
                cases_evaluated: 3,
                accuracy_percent: 100.0,
                float_leaks_count: 0,
                passed: true,
            },
        ],
        total_float_leaks: 0,
        passed: true,
    };
    let fault_injection = FaultInjectionSummary {
        injected_count: 22,
        caught_count: 22,
        catch_rate_percent: 100.0,
        builder_scorer_isolated: true,
        passed: true,
    };
    (ablation, calibration, fault_injection)
}

#[cfg(unix)]
fn create_unix_fifo(path: &Path) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).expect("cstring");
    let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    assert_eq!(rc, 0, "mkfifo failed: {}", std::io::Error::last_os_error());
}

#[tokio::test]
async fn p0_1_skip_sim_rejected_on_stage_release_and_mandatory_security_gates_enforced() {
    let _guard = eval_test_mutex().lock().await;
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    materialize_valid_design_craft(tmp.path(), "1.0.0", true);
    seal_sealed_eval_dir(tmp.path()).expect("seal");
    let (_state, router, editor_tok, _viewer_tok) = setup_daemon(&tmp);

    // 1. HTTP rejects runSimulatedUsers=false on stage and release with 400 VALIDATION_ERROR
    for tier in ["stage", "release"] {
        let (st, body) = send_json(
            &router,
            "POST",
            "/api/v1/eval/run",
            Some(&editor_tok),
            Some(json!({ "tier": tier, "runSimulatedUsers": false })),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "tier={tier} body={body}");
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
        assert_eq!(body["error"]["field"], "runSimulatedUsers");
    }

    // 2. CLI parser rejects --skip-sim on stage and release (exit 2)
    for tier in ["stage", "release"] {
        let err = parse_cli_args(&[
            "eval".into(),
            "run".into(),
            "--tier".into(),
            tier.into(),
            "--skip-sim".into(),
        ])
        .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    // 3. On PR tier, runSimulatedUsers=false STILL executes all 6 canonical mandatory security gates
    let (st_pr, pr_body) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr", "runSimulatedUsers": false })),
    )
    .await;
    assert_eq!(st_pr, StatusCode::OK, "body={pr_body}");
    assert_eq!(pr_body["data"]["verdict"], "PASS");
    assert_eq!(pr_body["data"]["securityGatesPassed"], true);
    let sim_cases = pr_body["data"]["simCases"].as_array().unwrap();
    assert_eq!(sim_cases.len(), MANDATORY_SECURITY_GATE_CASE_IDS.len());
    for req_id in MANDATORY_SECURITY_GATE_CASE_IDS {
        assert!(
            sim_cases
                .iter()
                .any(|c| c["caseId"] == *req_id && c["status"] == "PASS"),
            "missing mandatory security case {req_id} in PR skip-sim run"
        );
    }

    // 4. compute_eval_verdict fails closed when sim_cases is empty or missing UC-03
    let (ablation, calibration, fault_injection) = passing_summaries();
    let (v_empty, sec_empty, blockers_empty) = compute_eval_verdict(&EvalVerdictInput {
        tier: EvalTier::Pr,
        offline_enforced: true,
        duration_ms: 50,
        estimated_cost_usd: 0.0,
        ablation: &ablation,
        calibration: &calibration,
        fault_injection: &fault_injection,
        sim_cases: &[],
        run_simulated_users: false,
    });
    assert_eq!(v_empty, EvalVerdict::Fail);
    assert!(!sec_empty);
    assert_eq!(blockers_empty.len(), MANDATORY_SECURITY_GATE_CASE_IDS.len());
    assert!(blockers_empty.contains(&"UC-03:MISSING_MANDATORY_SECURITY_GATE".to_string()));

    // Missing UC-03 even when other 5 mandatory cases pass (with spoofed mandatory_security_gate=false)
    let partial_cases: Vec<SimCaseResult> = MANDATORY_SECURITY_GATE_CASE_IDS
        .iter()
        .filter(|&&id| id != "UC-03")
        .map(|&id| SimCaseResult {
            case_id: id.to_string(),
            milestone: EvalMilestoneId::M5,
            title_key: format!("evalCertification.cases.{id}"),
            mandatory_security_gate: false,
            security_gate_key: None,
            status: SimCaseStatus::Pass,
            duration_ms: 1,
            evidence_summary: "ok".to_string(),
        })
        .collect();
    let (v_missing, sec_missing, blockers_missing) = compute_eval_verdict(&EvalVerdictInput {
        tier: EvalTier::Pr,
        offline_enforced: true,
        duration_ms: 50,
        estimated_cost_usd: 0.0,
        ablation: &ablation,
        calibration: &calibration,
        fault_injection: &fault_injection,
        sim_cases: &partial_cases,
        run_simulated_users: false,
    });
    assert_eq!(v_missing, EvalVerdict::Fail);
    assert!(!sec_missing);
    assert!(blockers_missing.contains(&"UC-03:MISSING_MANDATORY_SECURITY_GATE".to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn p1_1_and_p1_2_fifo_nonblocking_oversized_cap_and_fd_seal() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = eval_test_mutex().lock().await;
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    // 1. FIFO in tests/eval/sealed/ rejected non-blocking with SEALED_EVAL_TAMPER_ERROR
    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    materialize_valid_design_craft(tmp.path(), "1.0.0", true);
    seal_sealed_eval_dir(tmp.path()).expect("seal");

    let fifo_path = tmp.path().join(SEALED_EVAL_REL_DIR).join("briefs.json");
    fs::set_permissions(&fifo_path, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&fifo_path).unwrap();
    create_unix_fifo(&fifo_path);

    let (_state, router, editor_tok, _) = setup_daemon(&tmp);
    let (st_fifo, body_fifo) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr" })),
    )
    .await;
    assert_eq!(st_fifo, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_fifo["error"]["code"], "SEALED_EVAL_TAMPER_ERROR");
    assert_eq!(
        body_fifo["error"]["details"]["tamperReason"],
        "UNEXPECTED_ENTRY_IN_SEALED_DIR"
    );

    // 2. Oversized file > MAX_SEALED_FILE_BYTES fails closed with SEALED_FILE_TOO_LARGE
    let tmp_big = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp_big.path());
    let mf_big = tmp_big
        .path()
        .join(SEALED_EVAL_REL_DIR)
        .join("manifest.json");
    let big_bytes = vec![b' '; (MAX_SEALED_FILE_BYTES + 128) as usize];
    fs::write(&mf_big, big_bytes).unwrap();
    fs::set_permissions(&mf_big, fs::Permissions::from_mode(0o444)).unwrap();
    let big_err = verify_sealed_eval_integrity(tmp_big.path()).unwrap_err();
    assert_eq!(big_err.reason, "SEALED_FILE_TOO_LARGE");

    // 3. Hardlink inside sealed dir fails closed on seal_sealed_eval_dir
    let tmp_hl = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp_hl.path());
    let orig = tmp_hl.path().join(SEALED_EVAL_REL_DIR).join("briefs.json");
    let hl_outside = tmp_hl.path().join("briefs-hardlink.json");
    fs::hard_link(&orig, &hl_outside).unwrap();
    let hl_err = seal_sealed_eval_dir(tmp_hl.path()).unwrap_err();
    assert_eq!(hl_err.reason, "HARDLINK_DETECTED");
}

#[tokio::test]
async fn p1_3_design_craft_symlink_fifo_oom_and_missing_default_fail_closed() {
    let _guard = eval_test_mutex().lock().await;
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    seal_sealed_eval_dir(tmp.path()).expect("seal");

    // 1. Missing default packages/design-craft/plugin.json fails closed (never fabricates 1.0.0)
    let missing_dc = inspect_design_craft_package(None, tmp.path());
    assert!(!missing_dc.semver_v1_valid);
    assert_eq!(missing_dc.version, "missing");

    let (_state, router, editor_tok, _) = setup_daemon(&tmp);
    let (st_miss, body_miss) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr" })),
    )
    .await;
    assert_eq!(st_miss, StatusCode::OK);
    assert_eq!(body_miss["data"]["verdict"], "FAIL");
    assert_eq!(
        body_miss["data"]["ablation"]["designCraftSemverPassed"],
        false
    );

    // 2. validate_candidate_dir_in_workspace rejects regular files (must be a directory)
    let reg_file = tmp.path().join("not-a-dir.txt");
    fs::write(&reg_file, "hello").unwrap();
    assert!(validate_candidate_dir_in_workspace(tmp.path(), "not-a-dir.txt").is_err());

    // 3. Symlinked plugin.json fails closed
    #[cfg(unix)]
    {
        let sym_dir = tmp.path().join("cand-sym");
        fs::create_dir_all(&sym_dir).unwrap();
        let outside = tmp.path().join("outside-plugin.json");
        fs::write(&outside, r#"{"version":"1.0.0"}"#).unwrap();
        std::os::unix::fs::symlink(&outside, sym_dir.join("plugin.json")).unwrap();
        let sym_res = inspect_design_craft_package(Some(&sym_dir), tmp.path());
        assert!(!sym_res.semver_v1_valid);
        assert_eq!(sym_res.version, "symlink-rejected");
    }

    // 4. FIFO plugin.json fails closed non-blocking
    #[cfg(unix)]
    {
        let fifo_dir = tmp.path().join("cand-fifo");
        fs::create_dir_all(&fifo_dir).unwrap();
        create_unix_fifo(&fifo_dir.join("plugin.json"));
        let fifo_res = inspect_design_craft_package(Some(&fifo_dir), tmp.path());
        assert!(!fifo_res.semver_v1_valid);
        assert_eq!(fifo_res.version, "non-regular-file");
    }

    // 5. Oversized plugin.json (> 256 KiB) fails closed
    let big_dir = tmp.path().join("cand-big");
    fs::create_dir_all(&big_dir).unwrap();
    let big_json = format!(
        "{{\"version\":\"1.0.0\",\"pad\":\"{}\"}}",
        "a".repeat((MAX_PLUGIN_JSON_BYTES + 64) as usize)
    );
    fs::write(big_dir.join("plugin.json"), big_json).unwrap();
    let big_res = inspect_design_craft_package(Some(&big_dir), tmp.path());
    assert!(!big_res.semver_v1_valid);
    assert_eq!(big_res.version, "oversized");
}

#[test]
fn p1_4_list_eval_history_is_pure_read_no_ddl() {
    let _guard = eval_test_mutex().blocking_lock();
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).unwrap();

    // Drop eval_benchmark_runs so we can prove list_eval_history executes ZERO DDL on read conn
    {
        let conn = pool.connection().unwrap();
        conn.execute("DROP TABLE IF EXISTS eval_benchmark_runs", [])
            .unwrap();
    }

    let hist = list_eval_history(&pool, tmp.path(), "tenant-alpha", 20, None, None)
        .expect("list_eval_history should succeed read-only");
    assert_eq!(hist.total_runs, 0);
    assert!(hist.runs.is_empty());
    assert!(hist.latest_run.is_none());

    // Confirm table STILL does not exist (no DDL was run)
    let conn = pool.connection().unwrap();
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'eval_benchmark_runs'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exists, 0);
}

#[tokio::test]
async fn p1_5_strict_bearer_middleware_rejects_untrusted_bearer_before_role_check() {
    let _guard = eval_test_mutex().lock().await;
    let tmp = TempDir::new().unwrap();
    let (_state, router, _editor_tok, _viewer_tok) = setup_daemon(&tmp);

    // Untrusted Bearer + viewer role header MUST get 401 UNAUTHORIZED from middleware (not 403)
    for (method, uri, body) in [
        ("POST", "/api/v1/eval/run", r#"{"tier":"pr"}"#),
        ("GET", "/api/v1/eval/history", ""),
    ] {
        let mut req_builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:4170")
            .header("authorization", "Bearer untrusted-legacy-token")
            .header("x-cowok-role", "viewer");
        if !body.is_empty() {
            req_builder = req_builder.header("content-type", "application/json");
        }
        let resp = router
            .clone()
            .oneshot(req_builder.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "method={method} uri={uri} must be rejected 401 by strict bearer middleware"
        );
    }
}

#[cfg(unix)]
#[test]
fn p1_6_ledger_fifo_rejection_and_readonly_workspace_split() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = eval_test_mutex().blocking_lock();
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    // 1. FIFO at gate-history.jsonl fails closed non-blocking
    let tmp_fifo = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp_fifo.path());
    materialize_valid_design_craft(tmp_fifo.path(), "1.0.0", true);
    seal_sealed_eval_dir(tmp_fifo.path()).expect("seal");
    let evals_dir = tmp_fifo.path().join(".cowok-design/evals");
    fs::create_dir_all(&evals_dir).unwrap();
    create_unix_fifo(&evals_dir.join("gate-history.jsonl"));

    let cmd_fifo = parse_cli_args(&[
        "eval".into(),
        "run".into(),
        "--tier".into(),
        "pr".into(),
        "--workspace".into(),
        tmp_fifo.path().to_string_lossy().into_owned(),
        "--json".into(),
    ])
    .unwrap();
    let out_fifo = execute_cli_command(cmd_fifo);
    assert_eq!(out_fifo.exit_code, 1, "stdout={}", out_fifo.stdout);
    let parsed_fifo: serde_json::Value = serde_json::from_str(&out_fifo.stdout).unwrap();
    assert_eq!(parsed_fifo["error"]["code"], "STORAGE_ERROR");

    // 2. Read-only workspace splits state cleanly to XDG_DATA_HOME without STORAGE_ERROR
    let ro_ws = TempDir::new().unwrap();
    let xdg_dir = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(ro_ws.path());
    materialize_valid_design_craft(ro_ws.path(), "1.0.0", true);
    seal_sealed_eval_dir(ro_ws.path()).expect("seal");

    let prev_xdg = std::env::var_os("XDG_DATA_HOME");
    std::env::set_var("XDG_DATA_HOME", xdg_dir.path());
    fs::set_permissions(ro_ws.path(), fs::Permissions::from_mode(0o555)).unwrap();

    let cmd_ro = parse_cli_args(&[
        "eval".into(),
        "run".into(),
        "--tier".into(),
        "pr".into(),
        "--workspace".into(),
        ro_ws.path().to_string_lossy().into_owned(),
        "--json".into(),
    ])
    .unwrap();
    let out_ro = execute_cli_command(cmd_ro);

    // Restore permissions & env before asserting so cleanup always succeeds
    fs::set_permissions(ro_ws.path(), fs::Permissions::from_mode(0o755)).unwrap();
    match prev_xdg {
        Some(v) => std::env::set_var("XDG_DATA_HOME", v),
        None => std::env::remove_var("XDG_DATA_HOME"),
    }

    assert_eq!(
        out_ro.exit_code, 0,
        "ro workspace failed: stdout={} stderr={}",
        out_ro.stdout, out_ro.stderr
    );
    let ro_json: serde_json::Value = serde_json::from_str(&out_ro.stdout).unwrap();
    assert_eq!(ro_json["ok"], true);
    assert_eq!(ro_json["data"]["verdict"], "PASS");
}

#[test]
fn p2_1_p2_2_p2_5_real_probes_live_uc24_strict_semver_and_css4_hex() {
    let _guard = eval_test_mutex().blocking_lock();
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    // 1. Strict SemVer 2.0.0 tests (P2-5)
    assert!(parse_semver_v1("1.0.0"));
    assert!(parse_semver_v1("1.2.3+build.42"));
    assert!(parse_semver_v1("1.2.3-rc.1+sha.deadbeef"));
    assert!(parse_semver_v1("1.0.0-alpha01"));
    assert!(!parse_semver_v1("01.0.0"));
    assert!(!parse_semver_v1("1.02.0"));
    assert!(!parse_semver_v1("1.0.03"));
    assert!(!parse_semver_v1("1.0.0-"));
    assert!(!parse_semver_v1("1.0.0+"));
    assert!(!parse_semver_v1("1.0.0-01"));
    assert!(!parse_semver_v1("1.0.0-rc..1"));
    assert!(!parse_semver_v1("2.0.0"));
    assert!(!parse_semver_v1("0.9.9"));

    // 2. CSS4 #rgba / #rrggbbaa hex detection (P2-5)
    assert!(run_independent_scorer_detector(
        "token_hex_guard",
        "color: #f00f;"
    ));
    assert!(run_independent_scorer_detector(
        "token_hex_guard",
        "background: #112233aa;"
    ));
    assert!(!run_independent_scorer_detector(
        "token_hex_guard",
        "color: var(--text-primary);"
    ));

    // 3. Real probes (P2-1) & live UC-24 Instant check (P2-2)
    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    seal_sealed_eval_dir(tmp.path()).expect("seal");
    let verified = verify_sealed_eval_integrity(tmp.path()).unwrap();
    let (ablation, calibration, fault_injection) = passing_summaries();

    let ctx_live = SimEvalContext {
        workspace_root: tmp.path(),
        tier: EvalTier::Pr,
        offline_enforced: true,
        sealed_integrity_ok: true,
        write_actor_alive: true,
        ablation: &ablation,
        calibration: &calibration,
        fault_injection: &fault_injection,
        started: Instant::now(),
        estimated_cost_usd: 0.0,
    };
    let all_ok = evaluate_sim_user_25_cases(&verified.sim_cases, &ctx_live);
    assert_eq!(all_ok.len(), 25);
    assert!(
        all_ok.iter().all(|c| c.status == SimCaseStatus::Pass),
        "failed probes: {:?}",
        all_ok
            .iter()
            .filter(|c| c.status != SimCaseStatus::Pass)
            .collect::<Vec<_>>()
    );

    // Proves UC-16 checks real actor liveness (not hardcoded true) and UC-24 checks live elapsed time
    let backdated = Instant::now()
        .checked_sub(Duration::from_millis(PR_TIER_MAX_DURATION_MS + 1_000))
        .expect("backdated instant");
    let ctx_fail = SimEvalContext {
        write_actor_alive: false,
        started: backdated,
        ..ctx_live
    };
    let cases_fail = evaluate_sim_user_25_cases(&verified.sim_cases, &ctx_fail);
    let uc16 = cases_fail.iter().find(|c| c.case_id == "UC-16").unwrap();
    assert_eq!(uc16.status, SimCaseStatus::Fail);
    let uc24 = cases_fail.iter().find(|c| c.case_id == "UC-24").unwrap();
    assert_eq!(uc24.status, SimCaseStatus::Fail);
}

#[cfg(unix)]
#[tokio::test]
async fn p2_3_cross_process_flock_blocks_overlapping_run() {
    use std::os::unix::io::AsRawFd;

    let _guard = eval_test_mutex().lock().await;
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    materialize_valid_design_craft(tmp.path(), "1.0.0", true);
    seal_sealed_eval_dir(tmp.path()).expect("seal");
    let (_state, router, editor_tok, _) = setup_daemon(&tmp);

    let evals_dir = tmp.path().join(".cowok-design/evals");
    fs::create_dir_all(&evals_dir).unwrap();
    let lock_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(evals_dir.join(EVAL_RUN_LOCK_FILENAME))
        .unwrap();
    let rc = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(rc, 0, "external flock must succeed");

    // Even with EVAL_RUN_IN_FLIGHT == false, external flock forces 409 EVAL_ALREADY_RUNNING
    let (st_409, body_409) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr" })),
    )
    .await;
    assert_eq!(st_409, StatusCode::CONFLICT, "body={body_409}");
    assert_eq!(body_409["error"]["code"], "EVAL_ALREADY_RUNNING");

    // Release external flock -> run succeeds
    unsafe {
        libc::flock(lock_file.as_raw_fd(), libc::LOCK_UN);
    }
    drop(lock_file);

    let (st_ok, body_ok) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr" })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "body={body_ok}");
    assert_eq!(body_ok["data"]["verdict"], "PASS");
}

#[tokio::test]
async fn p2_4_cli_project_prevalidation_and_http_uniform_404_tenant_isolation() {
    let _guard = eval_test_mutex().lock().await;
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    materialize_canonical_sealed_dir(tmp.path());
    materialize_valid_design_craft(tmp.path(), "1.0.0", true);
    seal_sealed_eval_dir(tmp.path()).expect("seal");

    // 1. CLI --project pre-validates missing project with exit code 2
    let cmd_bad_proj = parse_cli_args(&[
        "eval".into(),
        "run".into(),
        "--tier".into(),
        "pr".into(),
        "--project".into(),
        "non-existent-project-id".into(),
        "--workspace".into(),
        tmp.path().to_string_lossy().into_owned(),
        "--json".into(),
    ])
    .unwrap();
    let out_bad_proj = execute_cli_command(cmd_bad_proj);
    assert_eq!(out_bad_proj.exit_code, 2, "stdout={}", out_bad_proj.stdout);
    let parsed_cli: serde_json::Value = serde_json::from_str(&out_bad_proj.stdout).unwrap();
    assert_eq!(parsed_cli["error"]["code"], "PROJECT_NOT_FOUND");

    // 2. HTTP returns uniform 404 PROJECT_NOT_FOUND on BOTH missing project and cross-tenant project
    let (state, router, editor_tok, _) = setup_daemon(&tmp);
    let other_tenant_project_id = Uuid::new_v4().to_string();
    {
        let conn = state.registry.pool().connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO projects (id, slug, name, tenant_id, workspace_root, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                other_tenant_project_id,
                format!("other-tenant-{}", Uuid::new_v4().simple()),
                "Other Tenant Project",
                "tenant-beta",
                tmp.path().to_string_lossy().into_owned(),
                now,
                now
            ],
        )
        .unwrap();
    }

    let (st_xtenant, body_xtenant) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr", "projectId": other_tenant_project_id })),
    )
    .await;
    let (st_missing, body_missing) = send_json(
        &router,
        "POST",
        "/api/v1/eval/run",
        Some(&editor_tok),
        Some(json!({ "tier": "pr", "projectId": Uuid::new_v4().to_string() })),
    )
    .await;
    assert_eq!(st_xtenant, StatusCode::NOT_FOUND);
    assert_eq!(st_missing, StatusCode::NOT_FOUND);
    assert_eq!(body_xtenant["error"]["code"], "PROJECT_NOT_FOUND");
    assert_eq!(body_missing["error"]["code"], "PROJECT_NOT_FOUND");
}

#[test]
fn cli_eval_init_sealed_materializes_seals_and_refuses_overwrite() {
    let _guard = eval_test_mutex().blocking_lock();
    std::env::remove_var(SEALED_MANIFEST_ENV_VAR);

    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().to_string_lossy().into_owned();

    // 1. Fresh workspace -> init-sealed writes 5 canonical files & seals at 0444
    let cmd_init = parse_cli_args(&[
        "eval".into(),
        "init-sealed".into(),
        "--workspace".into(),
        ws.clone(),
        "--json".into(),
    ])
    .unwrap();
    let out_init = execute_cli_command(cmd_init);
    assert_eq!(
        out_init.exit_code, 0,
        "stdout={} stderr={}",
        out_init.stdout, out_init.stderr
    );
    let init_json: serde_json::Value = serde_json::from_str(&out_init.stdout).unwrap();
    assert_eq!(init_json["ok"], true);
    assert_eq!(init_json["data"]["sealedIntegrityOk"], true);
    assert_eq!(init_json["data"]["sealedFilesCount"], 5);
    assert_eq!(
        init_json["data"]["sealedManifestSha256"],
        PINNED_SEALED_MANIFEST_SHA256
    );

    // 2. Running init-sealed again refuses to overwrite existing sealed files
    let cmd_again = parse_cli_args(&[
        "eval".into(),
        "init-sealed".into(),
        "--workspace".into(),
        ws,
        "--json".into(),
    ])
    .unwrap();
    let out_again = execute_cli_command(cmd_again);
    assert_eq!(out_again.exit_code, 3);
    let again_json: serde_json::Value = serde_json::from_str(&out_again.stdout).unwrap();
    assert_eq!(again_json["error"]["code"], "SEALED_DIR_NOT_EMPTY");
}
