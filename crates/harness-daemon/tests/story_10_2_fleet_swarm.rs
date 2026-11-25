// @story 10.2
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon Integration Tests: V27 worker_file_leases, Disjoint Overlap 409, 429 Clamp 3->1, Builder != Scorer 403, E1..E4 >= 95%, Generation Fence 409 & V18a Ledger Handoff)
use std::fs;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use bot_fleet::design_jury::{canonical_e1_e4_benchmark_input, REQUIRED_E1_E4_CATCH_RATE};
use bot_fleet::swarm_leases::{
    current_process_pgid, INITIAL_CONCURRENCY, RECOVERY_SUCCESS_THRESHOLD,
};
use chrono::Utc;
use harness_daemon::daemon_auth::{DaemonAuthState, FlowPrincipal};
use harness_daemon::routes::execution_reaper::{
    spawn_pgid_child, ActiveWorkerEntry, ActiveWorkerRegistry,
};
use harness_daemon::routes::fleet_swarm::shared_project_rate_semaphore;
use harness_daemon::routes::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::{json, Value};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct FleetSwarmHarness {
    dir: tempfile::TempDir,
    registry: Arc<SpecRegistry>,
    active_workers: ActiveWorkerRegistry,
    app: Router,
    project_id: String,
    editor_a_tok: String,
    viewer_a_tok: String,
    editor_b_tok: String,
}

fn setup_harness() -> FleetSwarmHarness {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("src/pages")).expect("mkdir src/pages");
    fs::write(
        dir.path().join("src/pages/Home.tsx"),
        "export const Home = () => null;",
    )
    .expect("seed Home.tsx");

    let db_path = dir.path().join(".cowok-design").join("daemon-102.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let project_id = uuid::Uuid::new_v4().to_string();

    {
        let conn = registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES (?1, 'tenant-a', 'proj-102', 'Fleet Swarm Proj', ?2, '', 'ACTIVE', 1000, 1000)",
            params![project_id, dir.path().to_string_lossy().as_ref()],
        )
        .expect("seed project");
    }

    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_a_tok = format!("cwk_sess_102_ed_a_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        editor_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_a_tok = format!("cwk_sess_102_vw_a_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        viewer_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "viewer-a".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let editor_b_tok = format!("cwk_sess_102_ed_b_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        editor_b_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-b".to_string(),
            user_id: "user-b".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let state =
        DaemonAppState::with_auth(Arc::clone(&registry), "127.0.0.1".to_string(), 4172, auth);
    let active_workers = state.active_workers.clone();
    let _ = harness_daemon::routes::orchestrator::shared_write_actor_for_pool(registry.pool());
    let app = build_router(state);

    FleetSwarmHarness {
        dir,
        registry,
        active_workers,
        app,
        project_id,
        editor_a_tok,
        viewer_a_tok,
        editor_b_tok,
    }
}

async fn send_post(
    app: &Router,
    uri: &str,
    session_token: &str,
    payload: Value,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("host", "127.0.0.1:4172")
        .header("content-type", "application/json")
        .header("x-cowok-session-token", session_token)
        .body(Body::from(payload.to_string()))
        .expect("build request");

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let json_val: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
    (status, json_val)
}

#[tokio::test]
async fn leases_acquire_enforces_disjoint_canonical_prefix_and_monotonic_fencing_token() {
    let h = setup_harness();

    // 1. Worker A acquires directory lease src/pages/ -> fencing_token = 1
    let (status1, body1) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "claude-worker-1",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/pages/"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(status1, StatusCode::OK, "body={body1}");
    assert_eq!(body1["ok"], true);
    assert_eq!(body1["data"]["fencingToken"], 1);
    assert_eq!(body1["data"]["ownedPaths"], json!(["src/pages"]));

    // 2. Worker B requests descendant src/pages/Home.tsx -> 409 LEASE_PATH_CONFLICT
    let (status_conflict, body_conflict) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "codex-worker-2",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/pages/Home.tsx"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(
        status_conflict,
        StatusCode::CONFLICT,
        "body={body_conflict}"
    );
    assert_eq!(body_conflict["error"]["code"], "LEASE_PATH_CONFLICT");

    // 3. Worker B requests disjoint path src/pages-v2/Home.tsx -> 200 OK, fencing_token = 2
    let (status2, body2) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "codex-worker-2",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/pages-v2/Home.tsx"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(status2, StatusCode::OK, "body={body2}");
    assert_eq!(body2["data"]["fencingToken"], 2);

    // 4. Parent traversal rejected (400 PATH_TRAVERSAL)
    let (status_trav, body_trav) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "rogue-worker",
            "workerRole": "BUILDER",
            "ownedPaths": ["../outside.ts"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(status_trav, StatusCode::BAD_REQUEST, "body={body_trav}");
    assert_eq!(body_trav["error"]["code"], "PATH_TRAVERSAL");

    // 5. Viewer role blocked (403), cross-tenant editor blocked (404)
    let (status_viewer, _) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.viewer_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "viewer-worker",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/viewer.ts"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(status_viewer, StatusCode::FORBIDDEN);

    let (status_idor, _) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_b_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "other-tenant-worker",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/other.ts"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(status_idor, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn fleet_dispatch_writes_one_job_never_envelopes_and_clamps_3_to_1_on_429() {
    let h = setup_harness();

    // 1. Overlapping batch rejected with 409 LEASE_PATH_CONFLICT
    let (status_ov, body_ov) = send_post(
        &h.app,
        "/api/v1/fleet/dispatch",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "sessionId": "sess-10-2-overlap",
            "tasks": [
                {
                    "workerId": "builder-a",
                    "workerRole": "BUILDER",
                    "oneJob": "Build page A",
                    "neverRules": ["NEVER touch outside src/a"],
                    "ownedPaths": ["src/a"],
                    "ttlMs": 60000
                },
                {
                    "workerId": "builder-b",
                    "workerRole": "BUILDER",
                    "oneJob": "Build file b.ts",
                    "neverRules": ["NEVER self-PASS"],
                    "ownedPaths": ["src/a/b.ts"],
                    "ttlMs": 60000
                }
            ]
        }),
    )
    .await;
    assert_eq!(status_ov, StatusCode::CONFLICT, "body={body_ov}");
    assert_eq!(body_ov["error"]["code"], "LEASE_PATH_CONFLICT");

    // 2. Disjoint 3-worker dispatch under HTTP 429 clamps concurrency 3 -> 1
    let (status_ok, body_ok) = send_post(
        &h.app,
        "/api/v1/fleet/dispatch",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "sessionId": "sess-10-2-429",
            "upstreamHttpStatus": 429,
            "retryAfterMs": 1200,
            "tasks": [
                {
                    "workerId": "claude-1",
                    "workerRole": "BUILDER",
                    "oneJob": "Implement header",
                    "neverRules": ["NEVER edit footer", "NEVER self-PASS"],
                    "ownedPaths": ["src/components/Header.tsx"],
                    "ttlMs": 60000
                },
                {
                    "workerId": "codex-2",
                    "workerRole": "BUILDER",
                    "oneJob": "Implement footer",
                    "neverRules": ["NEVER edit header", "NEVER self-PASS"],
                    "ownedPaths": ["src/components/Footer.tsx"],
                    "ttlMs": 60000
                },
                {
                    "workerId": "agy-3",
                    "workerRole": "VERIFIER_QC",
                    "oneJob": "Run E1..E4 audit",
                    "neverRules": ["NEVER mutate builder files"],
                    "ownedPaths": ["src/qa/report.json"],
                    "ttlMs": 60000
                }
            ]
        }),
    )
    .await;
    assert_eq!(status_ok, StatusCode::OK, "body={body_ok}");
    assert_eq!(body_ok["data"]["concurrencyLimit"], 1);
    assert_eq!(body_ok["data"]["clampedOn429"], true);
    assert!(body_ok["data"]["backoffMs"].as_u64().unwrap_or(0) >= 1200);

    let assignments = body_ok["data"]["assignments"]
        .as_array()
        .expect("assignments");
    assert_eq!(assignments.len(), 3);
    assert_eq!(assignments[0]["dispatchState"], "DISPATCHED");
    assert_eq!(assignments[1]["dispatchState"], "QUEUED_BY_CLAMP");
    assert_eq!(assignments[2]["dispatchState"], "QUEUED_BY_CLAMP");

    // Verify coordination envelopes exist physically under .cowok-design/coordination/tasks/
    for a in assignments {
        let rel = a["envelopeRelPath"].as_str().expect("rel path");
        let full = h.dir.path().join(rel);
        assert!(full.exists(), "expected envelope at {}", full.display());
    }
}

#[tokio::test]
async fn jury_evaluate_blocks_builder_self_pass_enforces_fence_verifies_e1_e4_and_escalates_v18a_ledger(
) {
    let h = setup_harness();
    let freeze_sha = "b".repeat(64);

    // 1. Builder self-PASS spoof blocked with 403 BUILDER_SELF_PASS_FORBIDDEN (same worker ID)
    let (status_self, body_self) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": "screen-home",
            "builderWorkerId": "claude-builder-1",
            "scorerWorkerId": "claude-builder-1",
            "scorerRole": "VERIFIER_QC",
            "generationFence": 1,
            "freezeSnapshotSha256": freeze_sha
        }),
    )
    .await;
    assert_eq!(status_self, StatusCode::FORBIDDEN, "body={body_self}");
    assert_eq!(body_self["error"]["code"], "BUILDER_SELF_PASS_FORBIDDEN");

    // 2. Builder role trying to issue PASS under alias blocked with 403 BUILDER_SELF_PASS_FORBIDDEN
    let (status_role, body_role) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": "screen-home",
            "builderWorkerId": "claude-builder-1",
            "scorerWorkerId": "codex-builder-2",
            "scorerRole": "BUILDER",
            "generationFence": 1,
            "freezeSnapshotSha256": freeze_sha
        }),
    )
    .await;
    assert_eq!(status_role, StatusCode::FORBIDDEN, "body={body_role}");
    assert_eq!(body_role["error"]["code"], "BUILDER_SELF_PASS_FORBIDDEN");

    // 3. DB-level CHECK (builder_worker_id <> scorer_worker_id) rejects direct SQL spoofing
    {
        let conn = h.registry.pool().connection().expect("conn");
        let db_check_err = conn.execute(
            "INSERT INTO fleet_jury_verdicts (
                id, project_id, builder_worker_id, scorer_worker_id,
                generation_fence, freeze_snapshot_sha256, qc_run_id,
                e1_e4_catch_rate, verdict, violations_json, ledger_entry_id, created_at
             ) VALUES ('spoof-1', ?1, 'w-same', 'w-same', 1, ?2, NULL, 1.0, 'PASS', '[]', NULL, 1000)",
            params![h.project_id, freeze_sha],
        );
        assert!(
            db_check_err.is_err(),
            "SQLite CHECK must block builder_worker_id = scorer_worker_id"
        );
    }

    // 4. First E1..E4 benchmark run (generationFence = 2) catches >= 95% and returns FAIL (hit 1)
    let bench = canonical_e1_e4_benchmark_input();
    let (status_run1, body_run1) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": bench.screen_id,
            "builderWorkerId": "claude-builder-1",
            "scorerWorkerId": "independent-jury-1",
            "scorerRole": "DESIGN_JURY",
            "generationFence": 2,
            "freezeSnapshotSha256": freeze_sha,
            "frozenHtml": bench.html_snapshot,
            "qcRunId": bench.qc_run_id,
            "reportedIssues": bench.reported_issues,
            "expectedDefectIds": bench.expected_defect_ids,
            "contrastRatios": bench.contrast_ratios,
            "boundingBoxes": bench.bounding_boxes,
            "brandTokensExpected": bench.brand_tokens_expected
        }),
    )
    .await;
    assert_eq!(status_run1, StatusCode::OK, "body={body_run1}");
    let catch_rate = body_run1["data"]["e1E4CatchRate"]
        .as_f64()
        .expect("catch rate");
    assert!(
        catch_rate >= REQUIRED_E1_E4_CATCH_RATE,
        "expected catch rate >= 0.95, got {catch_rate}"
    );
    assert_eq!(body_run1["data"]["verdict"], "FAIL");
    assert_eq!(body_run1["data"]["escalatedToLedger"], false);
    assert_eq!(body_run1["data"]["lensScores"].as_array().unwrap().len(), 4);

    // 5. Stale generation_fence (1 < latest persisted 2) rejected with 409 STALE_GENERATION_FENCE
    let (status_stale, body_stale) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": "screen-home",
            "builderWorkerId": "claude-builder-1",
            "scorerWorkerId": "independent-jury-1",
            "scorerRole": "DESIGN_JURY",
            "generationFence": 1,
            "freezeSnapshotSha256": freeze_sha
        }),
    )
    .await;
    assert_eq!(status_stale, StatusCode::CONFLICT, "body={body_stale}");
    assert_eq!(body_stale["error"]["code"], "STALE_GENERATION_FENCE");

    // 6. Second occurrence of same violations (generationFence = 3) escalates to V18a gate_failure_ledger (HUMAN_ONLY)
    let (status_run2, body_run2) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": bench.screen_id,
            "builderWorkerId": "claude-builder-1",
            "scorerWorkerId": "independent-jury-1",
            "scorerRole": "DESIGN_JURY",
            "generationFence": 3,
            "freezeSnapshotSha256": freeze_sha,
            "frozenHtml": bench.html_snapshot,
            "qcRunId": "qc-run-second",
            "reportedIssues": bench.reported_issues,
            "contrastRatios": bench.contrast_ratios,
            "boundingBoxes": bench.bounding_boxes,
            "brandTokensExpected": bench.brand_tokens_expected
        }),
    )
    .await;
    assert_eq!(status_run2, StatusCode::OK, "body={body_run2}");
    assert_eq!(body_run2["data"]["verdict"], "ESCALATED_TO_LEDGER");
    assert_eq!(body_run2["data"]["escalatedToLedger"], true);
    assert_eq!(body_run2["data"]["approvalPolicy"], "HUMAN_ONLY");
    let ledger_id = body_run2["data"]["ledgerEntryId"]
        .as_str()
        .expect("ledgerEntryId must be populated");

    // Verify persisted gate_failure_ledger row carries status='PROPOSED' and approval_policy=HUMAN_ONLY
    {
        let conn = h.registry.pool().connection().expect("conn");
        let (status_str, sample_msg, hit_count): (String, String, i64) = conn
            .query_row(
                "SELECT status, sample_message, hit_count FROM gate_failure_ledger WHERE id = ?1",
                params![ledger_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("ledger row must exist");
        assert_eq!(status_str, "PROPOSED");
        assert!(sample_msg.contains("approval_policy=HUMAN_ONLY"));
        assert!(hit_count >= 2);
    }
}

#[tokio::test]
async fn unsafe_worker_pgid_rejected_422_and_reaper_never_kills_unowned_external_pgid() {
    let h = setup_harness();
    let self_pgid = current_process_pgid().unwrap_or(1);

    // 1. Unsafe PGIDs rejected with 422 INVALID_REQUEST on /api/v1/fleet/leases/acquire
    for bad_pgid in [-1, 0, 1, self_pgid] {
        let (status, body) = send_post(
            &h.app,
            "/api/v1/fleet/leases/acquire",
            &h.editor_a_tok,
            json!({
                "projectId": h.project_id,
                "workerId": "pgid-probe",
                "workerRole": "BUILDER",
                "ownedPaths": ["src/pgid-test.tsx"],
                "ttlMs": 60000,
                "workerPgid": bad_pgid
            }),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "bad_pgid={bad_pgid} body={body}"
        );
        assert_eq!(body["error"]["code"], "INVALID_REQUEST");
    }

    // 2. Unsafe PGIDs rejected with 422 INVALID_REQUEST on /api/v1/fleet/dispatch
    for bad_pgid in [0, 1, self_pgid] {
        let (status, body) = send_post(
            &h.app,
            "/api/v1/fleet/dispatch",
            &h.editor_a_tok,
            json!({
                "projectId": h.project_id,
                "sessionId": "sess-unsafe-pgid",
                "tasks": [
                    {
                        "workerId": "pgid-worker",
                        "workerRole": "BUILDER",
                        "oneJob": "Check unsafe pgid",
                        "neverRules": ["NEVER touch other paths"],
                        "ownedPaths": ["src/pgid-dispatch.tsx"],
                        "ttlMs": 60000,
                        "workerPgid": bad_pgid
                    }
                ]
            }),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "bad_pgid={bad_pgid} body={body}"
        );
        assert_eq!(body["error"]["code"], "INVALID_REQUEST");
    }

    // 3. Unowned external PGID MUST NOT be signalled when its expired lease is reaped
    #[cfg(unix)]
    {
        let mut ext_cmd = std::process::Command::new("sleep");
        ext_cmd.arg("30");
        let (mut ext_child, _ext_pid, ext_pgid, _ext_start) =
            spawn_pgid_child(ext_cmd).expect("spawn external pgid child");

        {
            let conn = h.registry.pool().connection().expect("conn");
            conn.execute(
                "INSERT INTO worker_file_leases (
                    lease_id, project_id, worker_id, worker_role, worker_pgid,
                    fencing_token, owned_paths_json, expires_at, released, created_at
                 ) VALUES ('expired-unowned', ?1, 'external-worker', 'BUILDER', ?2, 1, '[\"src/reap-unowned.tsx\"]', 1000, 0, 500)",
                params![h.project_id, ext_pgid],
            )
            .expect("seed expired unowned lease");
        }

        let (status, body) = send_post(
            &h.app,
            "/api/v1/fleet/leases/acquire",
            &h.editor_a_tok,
            json!({
                "projectId": h.project_id,
                "workerId": "new-owner",
                "workerRole": "BUILDER",
                "ownedPaths": ["src/reap-unowned.tsx"],
                "ttlMs": 60000
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "body={body}");
        assert_eq!(body["data"]["reapedExpiredCount"], 1);

        // External process MUST still be alive (try_wait is None and kill(-pgid, 0) == 0).
        let still_running = ext_child.try_wait().expect("try_wait").is_none();
        // Clean up external child before asserting so we never leak processes.
        let _ = ext_child.kill();
        let _ = ext_child.wait();
        assert!(
            still_running,
            "reaper MUST NOT send SIGKILL to unowned external PGID"
        );

        // 4. Owned daemon child registered in state.active_workers IS terminated cleanly on reap
        let mut owned_cmd = std::process::Command::new("sleep");
        owned_cmd.arg("30");
        let (mut owned_child, owned_pid, owned_pgid, owned_start) =
            spawn_pgid_child(owned_cmd).expect("spawn owned child");

        h.active_workers.register(ActiveWorkerEntry {
            job_id: "job-owned-reap".to_string(),
            project_id: h.project_id.clone(),
            fencing_token: 10,
            pid: owned_pid,
            pgid: owned_pgid,
            start_time: owned_start,
            shadow_dir: h.dir.path().join(".cowok-design/shadow/owned"),
        });

        {
            let conn = h.registry.pool().connection().expect("conn");
            conn.execute(
                "INSERT INTO worker_file_leases (
                    lease_id, project_id, worker_id, worker_role, worker_pgid,
                    fencing_token, owned_paths_json, expires_at, released, created_at
                 ) VALUES ('expired-owned', ?1, 'owned-worker', 'BUILDER', ?2, 10, '[\"src/reap-owned.tsx\"]', 1000, 0, 500)",
                params![h.project_id, owned_pgid],
            )
            .expect("seed expired owned lease");
        }

        let (status2, body2) = send_post(
            &h.app,
            "/api/v1/fleet/leases/acquire",
            &h.editor_a_tok,
            json!({
                "projectId": h.project_id,
                "workerId": "successor-worker",
                "workerRole": "BUILDER",
                "ownedPaths": ["src/reap-owned.tsx"],
                "ttlMs": 60000
            }),
        )
        .await;
        assert_eq!(status2, StatusCode::OK, "body={body2}");
        assert_eq!(body2["data"]["reapedExpiredCount"], 1);

        // Owned child process group must now be terminated (probe returns -1 / ESRCH).
        let alive_after = unsafe { libc::kill(-owned_pgid, 0) == 0 };
        let _ = owned_child.kill();
        let _ = owned_child.wait();
        assert!(
            !alive_after,
            "daemon-owned expired worker PGID must be terminated via terminate_pgid_checked"
        );
    }
}

#[tokio::test]
async fn jury_evaluate_blocks_builder_role_spoofing_via_db_lease_history_and_invalid_worker_ids() {
    let h = setup_harness();
    let freeze_sha = "c".repeat(64);

    // 1. Invalid worker IDs ("." / "..") rejected with 400 VALIDATION_ERROR
    let (bad_b_status, bad_b_body) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": "screen-home",
            "builderWorkerId": ".",
            "scorerWorkerId": "independent-jury-1",
            "scorerRole": "DESIGN_JURY",
            "generationFence": 1,
            "freezeSnapshotSha256": freeze_sha
        }),
    )
    .await;
    assert_eq!(bad_b_status, StatusCode::BAD_REQUEST, "body={bad_b_body}");
    assert_eq!(bad_b_body["error"]["code"], "VALIDATION_ERROR");

    let (bad_s_status, bad_s_body) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": "screen-home",
            "builderWorkerId": "builder-1",
            "scorerWorkerId": "../evil",
            "scorerRole": "DESIGN_JURY",
            "generationFence": 1,
            "freezeSnapshotSha256": freeze_sha
        }),
    )
    .await;
    assert_eq!(bad_s_status, StatusCode::BAD_REQUEST, "body={bad_s_body}");
    assert_eq!(bad_s_body["error"]["code"], "VALIDATION_ERROR");

    // 2. Worker acquires a BUILDER lease in this project
    let (lease_status, lease_body) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "spoofing-builder-x",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/spoof.tsx"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(lease_status, StatusCode::OK, "body={lease_body}");

    // 3. That BUILDER attempts to spoof scorerRole="DESIGN_JURY" against a different builderWorkerId
    let (spoof_status, spoof_body) = send_post(
        &h.app,
        "/api/v1/fleet/jury/evaluate",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "screenId": "screen-home",
            "builderWorkerId": "other-builder-y",
            "scorerWorkerId": "SPOOFING-BUILDER-X",
            "scorerRole": "DESIGN_JURY",
            "generationFence": 1,
            "freezeSnapshotSha256": freeze_sha
        }),
    )
    .await;
    assert_eq!(spoof_status, StatusCode::FORBIDDEN, "body={spoof_body}");
    assert_eq!(spoof_body["error"]["code"], "BUILDER_SELF_PASS_FORBIDDEN");

    let verdict_count: i64 = {
        let conn = h.registry.pool().connection().expect("conn");
        conn.query_row(
            "SELECT COUNT(*) FROM fleet_jury_verdicts WHERE project_id = ?1",
            params![h.project_id],
            |r| r.get(0),
        )
        .expect("count verdicts")
    };
    assert_eq!(verdict_count, 0);
}

#[tokio::test]
async fn dispatch_prevents_orphaned_leases_on_blank_never_rules_mailbox_open_or_envelope_write_failure(
) {
    let h = setup_harness();

    // 1. Whitespace-only neverRules rejected BEFORE DB commit -> zero rows inserted
    let (blank_status, blank_body) = send_post(
        &h.app,
        "/api/v1/fleet/dispatch",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "sessionId": "sess-blank-never",
            "tasks": [
                {
                    "workerId": "builder-blank",
                    "workerRole": "BUILDER",
                    "oneJob": "Build hero",
                    "neverRules": ["   ", "\t\n"],
                    "ownedPaths": ["src/blank.tsx"],
                    "ttlMs": 60000
                }
            ]
        }),
    )
    .await;
    assert_eq!(blank_status, StatusCode::BAD_REQUEST, "body={blank_body}");
    assert_eq!(blank_body["error"]["code"], "INVALID_CONTRACT");

    let count_after_blank: i64 = {
        let conn = h.registry.pool().connection().expect("conn");
        conn.query_row(
            "SELECT COUNT(*) FROM worker_file_leases WHERE project_id = ?1",
            params![h.project_id],
            |r| r.get(0),
        )
        .expect("count leases")
    };
    assert_eq!(count_after_blank, 0);

    // 2. Symlinked .cowok-design/coordination fails CoordinationMailbox::open BEFORE DB commit
    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let outside = tempdir().expect("outside");
        let coord_path = h.dir.path().join(".cowok-design/coordination");
        fs::create_dir_all(h.dir.path().join(".cowok-design")).expect("mkdir .cowok-design");
        symlink(outside.path(), &coord_path).expect("symlink coordination");

        let (sym_status, sym_body) = send_post(
            &h.app,
            "/api/v1/fleet/dispatch",
            &h.editor_a_tok,
            json!({
                "projectId": h.project_id,
                "sessionId": "sess-sym-coord",
                "tasks": [
                    {
                        "workerId": "builder-sym",
                        "workerRole": "BUILDER",
                        "oneJob": "Build hero",
                        "neverRules": ["NEVER escape sandbox"],
                        "ownedPaths": ["src/sym.tsx"],
                        "ttlMs": 60000
                    }
                ]
            }),
        )
        .await;
        assert_eq!(sym_status, StatusCode::BAD_REQUEST, "body={sym_body}");
        assert_eq!(sym_body["error"]["code"], "SYMLINK_VIOLATION");

        let count_after_sym: i64 = {
            let conn = h.registry.pool().connection().expect("conn");
            conn.query_row(
                "SELECT COUNT(*) FROM worker_file_leases WHERE project_id = ?1",
                params![h.project_id],
                |r| r.get(0),
            )
            .expect("count leases")
        };
        assert_eq!(count_after_sym, 0);

        fs::remove_file(&coord_path).expect("remove coord symlink");

        // 3. Post-TX envelope write failure triggers compensating rollback (released = 1)
        //    so no active leases remain orphaned in SQLite (skip permission test if running as uid 0).
        if unsafe { libc::geteuid() } != 0 {
            let tasks_dir = coord_path.join("tasks");
            let staging_dir = coord_path.join("staging");
            fs::create_dir_all(&tasks_dir).expect("mkdir tasks");
            fs::create_dir_all(&staging_dir).expect("mkdir staging");
            fs::set_permissions(&tasks_dir, fs::Permissions::from_mode(0o500))
                .expect("chmod 500 tasks");

            let (io_status, _io_body) = send_post(
                &h.app,
                "/api/v1/fleet/dispatch",
                &h.editor_a_tok,
                json!({
                    "projectId": h.project_id,
                    "sessionId": "sess-rollback",
                    "tasks": [
                        {
                            "workerId": "builder-rb-1",
                            "workerRole": "BUILDER",
                            "oneJob": "Build header",
                            "neverRules": ["NEVER edit footer"],
                            "ownedPaths": ["src/rb-header.tsx"],
                            "ttlMs": 60000
                        },
                        {
                            "workerId": "builder-rb-2",
                            "workerRole": "BUILDER",
                            "oneJob": "Build footer",
                            "neverRules": ["NEVER edit header"],
                            "ownedPaths": ["src/rb-footer.tsx"],
                            "ttlMs": 60000
                        }
                    ]
                }),
            )
            .await;

            // Always restore permissions before asserting so tempdir cleans up cleanly.
            fs::set_permissions(&tasks_dir, fs::Permissions::from_mode(0o700))
                .expect("restore 700 tasks");

            assert_eq!(io_status, StatusCode::BAD_REQUEST);

            let active_orphans: i64 = {
                let conn = h.registry.pool().connection().expect("conn");
                conn.query_row(
                    "SELECT COUNT(*) FROM worker_file_leases WHERE project_id = ?1 AND released = 0",
                    params![h.project_id],
                    |r| r.get(0),
                )
                .expect("count active orphans")
            };
            assert_eq!(
                active_orphans, 0,
                "compensating rollback must release all leases issued in failed dispatch batch"
            );

            // Re-dispatching the exact same paths must now succeed cleanly.
            let (retry_status, retry_body) = send_post(
                &h.app,
                "/api/v1/fleet/dispatch",
                &h.editor_a_tok,
                json!({
                    "projectId": h.project_id,
                    "sessionId": "sess-rollback-retry",
                    "tasks": [
                        {
                            "workerId": "builder-rb-1",
                            "workerRole": "BUILDER",
                            "oneJob": "Build header",
                            "neverRules": ["NEVER edit footer"],
                            "ownedPaths": ["src/rb-header.tsx"],
                            "ttlMs": 60000
                        }
                    ]
                }),
            )
            .await;
            assert_eq!(retry_status, StatusCode::OK, "body={retry_body}");
        }
    }
}

#[tokio::test]
async fn reap_expired_leases_persists_released_for_healthy_leases_when_another_owned_kill_fails() {
    let h = setup_harness();

    // Register a worker entry with missing start_time so terminate_pgid_checked returns Err
    // for lease-fail-kill, while lease-healthy-expired can be reaped cleanly.
    h.active_workers.register(ActiveWorkerEntry {
        job_id: "job-bad-start".to_string(),
        project_id: h.project_id.clone(),
        fencing_token: 1,
        pid: 66_001,
        pgid: 66_001,
        start_time: String::new(),
        shadow_dir: h.dir.path().join(".cowok-design/shadow/bad-start"),
    });

    {
        let conn = h.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO worker_file_leases (
                lease_id, project_id, worker_id, worker_role, worker_pgid,
                fencing_token, owned_paths_json, expires_at, released, created_at
             ) VALUES ('lease-fail-kill', ?1, 'w-fail', 'BUILDER', 66001, 1, '[\"src/stuck.tsx\"]', 1000, 0, 500)",
            params![h.project_id],
        )
        .expect("seed failing expired lease");
        conn.execute(
            "INSERT INTO worker_file_leases (
                lease_id, project_id, worker_id, worker_role, worker_pgid,
                fencing_token, owned_paths_json, expires_at, released, created_at
             ) VALUES ('lease-healthy-expired', ?1, 'w-ok', 'BUILDER', NULL, 2, '[\"src/reusable.tsx\"]', 1000, 0, 500)",
            params![h.project_id],
        )
        .expect("seed healthy expired lease");
    }

    let (status, body) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "w-next",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/reusable.tsx"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["data"]["reapedExpiredCount"], 1);

    let (fail_released, healthy_released): (i64, i64) = {
        let conn = h.registry.pool().connection().expect("conn");
        let r1: i64 = conn
            .query_row(
                "SELECT released FROM worker_file_leases WHERE lease_id = 'lease-fail-kill'",
                [],
                |r| r.get(0),
            )
            .expect("query lease-fail-kill");
        let r2: i64 = conn
            .query_row(
                "SELECT released FROM worker_file_leases WHERE lease_id = 'lease-healthy-expired'",
                [],
                |r| r.get(0),
            )
            .expect("query lease-healthy-expired");
        (r1, r2)
    };
    assert_eq!(
        fail_released, 0,
        "lease whose kill failed must remain unreleased"
    );
    assert_eq!(
        healthy_released, 1,
        "other expired leases must still be reaped and persisted as released = 1"
    );
}

#[tokio::test]
async fn dispatch_semaphore_unmutated_on_conflict_and_recovers_after_clean_dispatches() {
    let h = setup_harness();

    // 1. Acquire active lease on src/locked.tsx
    let (acq_status, _) = send_post(
        &h.app,
        "/api/v1/fleet/leases/acquire",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "workerId": "holder-1",
            "workerRole": "BUILDER",
            "ownedPaths": ["src/locked.tsx"],
            "ttlMs": 60000
        }),
    )
    .await;
    assert_eq!(acq_status, StatusCode::OK);

    // 2. Dispatch conflicting with src/locked.tsx AND carrying upstreamHttpStatus=429 -> 409 CONFLICT
    let (conflict_status, conflict_body) = send_post(
        &h.app,
        "/api/v1/fleet/dispatch",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "sessionId": "sess-conflict-429",
            "upstreamHttpStatus": 429,
            "tasks": [
                {
                    "workerId": "conflicting-worker",
                    "workerRole": "BUILDER",
                    "oneJob": "Conflict test",
                    "neverRules": ["NEVER collide"],
                    "ownedPaths": ["src/locked.tsx"],
                    "ttlMs": 60000
                }
            ]
        }),
    )
    .await;
    assert_eq!(
        conflict_status,
        StatusCode::CONFLICT,
        "body={conflict_body}"
    );

    // Semaphore MUST remain completely untouched (limit = 3, not clamped)
    let sem = shared_project_rate_semaphore("tenant-a", &h.project_id);
    assert_eq!(sem.current_limit(), INITIAL_CONCURRENCY);
    assert!(!sem.is_clamped());

    // 3. Valid dispatch with upstreamHttpStatus=429 clamps 3 -> 1
    let (clamp_status, clamp_body) = send_post(
        &h.app,
        "/api/v1/fleet/dispatch",
        &h.editor_a_tok,
        json!({
            "projectId": h.project_id,
            "sessionId": "sess-clamp-ok",
            "upstreamHttpStatus": 429,
            "tasks": [
                {
                    "workerId": "w-clamp",
                    "workerRole": "BUILDER",
                    "oneJob": "Trigger clamp",
                    "neverRules": ["NEVER exceed limit"],
                    "ownedPaths": ["src/clamp-0.tsx"],
                    "ttlMs": 60000
                }
            ]
        }),
    )
    .await;
    assert_eq!(clamp_status, StatusCode::OK, "body={clamp_body}");
    assert_eq!(clamp_body["data"]["concurrencyLimit"], 1);
    assert_eq!(clamp_body["data"]["clampedOn429"], true);

    // 4. Clean dispatches (upstreamHttpStatus=200 or None) recover 1 -> 2 -> 3
    let total_steps = RECOVERY_SUCCESS_THRESHOLD * 2;
    let mut last_body = json!({});
    for step in 1..=total_steps {
        let payload = if step % 2 == 0 {
            json!({
                "projectId": h.project_id,
                "sessionId": format!("sess-recover-{step}"),
                "upstreamHttpStatus": 200,
                "tasks": [
                    {
                        "workerId": format!("w-rec-{step}"),
                        "workerRole": "BUILDER",
                        "oneJob": "Clean recovery step",
                        "neverRules": ["NEVER drift"],
                        "ownedPaths": [format!("src/rec-{step}.tsx")],
                        "ttlMs": 60000
                    }
                ]
            })
        } else {
            json!({
                "projectId": h.project_id,
                "sessionId": format!("sess-recover-{step}"),
                "tasks": [
                    {
                        "workerId": format!("w-rec-{step}"),
                        "workerRole": "BUILDER",
                        "oneJob": "Clean recovery step",
                        "neverRules": ["NEVER drift"],
                        "ownedPaths": [format!("src/rec-{step}.tsx")],
                        "ttlMs": 60000
                    }
                ]
            })
        };
        let (st, bd) = send_post(&h.app, "/api/v1/fleet/dispatch", &h.editor_a_tok, payload).await;
        assert_eq!(st, StatusCode::OK, "step={step} body={bd}");
        if step == RECOVERY_SUCCESS_THRESHOLD {
            assert_eq!(bd["data"]["concurrencyLimit"], 2);
            assert_eq!(bd["data"]["clampedOn429"], true);
        }
        last_body = bd;
    }

    assert_eq!(last_body["data"]["concurrencyLimit"], INITIAL_CONCURRENCY);
    assert_eq!(last_body["data"]["clampedOn429"], false);
    assert_eq!(last_body["data"]["backoffMs"], 0);
    assert!(!sem.is_clamped());
}
