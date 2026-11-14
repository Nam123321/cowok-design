// @story 04.4
// @implements AC-01, AC-02, AC-03
use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::execution_reaper::{
    reap_orphan_jobs_on_startup, shadow_dir_for, spawn_pgid_child, sweep_expired_leases,
    terminate_pgid_checked, ActiveWorkerEntry,
};
use harness_daemon::{build_router, DaemonAppState, ProjectDto};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok_a = format!("sess-a-{}", uuid::Uuid::new_v4());
    let tok_b = format!("sess-b-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok_a.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".into(),
            user_id: "u1".into(),
            role: "editor".into(),
        },
        exp,
    );
    state.auth.insert_session(
        tok_b.clone(),
        FlowPrincipal {
            tenant_id: "tenant-b".into(),
            user_id: "u2".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok_a, tok_b)
}

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", tok);
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

async fn create_project(router: &axum::Router, tok: &str, slug: &str) -> ProjectDto {
    let (st, val) = send_req(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        Some(json!({ "slug": slug, "name": slug, "product_brief": "test" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{val}");
    serde_json::from_value(val).unwrap()
}

#[tokio::test]
async fn test_global_concurrency_cap_paused_slot_and_fifo_promotion() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, router, tok_a, _) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok_a, "cap-test").await;

    let (st, enq) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/enqueue", proj.id),
        &tok_a,
        Some(json!({
            "sessionId": "sess-cap-1",
            "nodeIds": ["n1", "n2", "n3", "n4", "n5"],
            "prompt": "Build screens",
            "mode": "parallel"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{enq}");
    let jobs = enq["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 5);
    assert_eq!(jobs[0]["status"], "RUNNING");
    assert_eq!(jobs[3]["status"], "RUNNING");
    assert_eq!(jobs[4]["status"], "QUEUED");
    assert_eq!(jobs[0]["daemonNonce"], state.auth.daemon_nonce);

    // Pause job 0 -> keeps active slot occupied (running + paused == 4), so job 4 stays QUEUED
    let job0_id = jobs[0]["id"].as_str().unwrap();
    let job0_tok = jobs[0]["fencingToken"].as_i64().unwrap();
    let (pst, paused) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/pause", proj.id),
        &tok_a,
        Some(json!({ "jobId": job0_id, "expectedFencingToken": job0_tok })),
    )
    .await;
    assert_eq!(pst, StatusCode::OK, "{paused}");
    assert_eq!(paused["status"], "PAUSED");
    assert_eq!(paused["fencingToken"], job0_tok + 1);

    let (nst, sessions) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/flow/node-sessions", proj.id),
        &tok_a,
        None,
    )
    .await;
    assert_eq!(nst, StatusCode::OK, "{sessions}");
    assert_eq!(sessions["activeSlotCount"], 4);

    // Cancel job 0 -> frees slot and promotes queued job 4 to RUNNING
    let (cst, cancelled) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/cancel", proj.id),
        &tok_a,
        Some(json!({ "jobId": job0_id, "expectedFencingToken": job0_tok + 1 })),
    )
    .await;
    assert_eq!(cst, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "CANCELLED");

    let (_, after_cancel) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/flow/node-sessions", proj.id),
        &tok_a,
        None,
    )
    .await;
    let job4 = after_cancel["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["nodeId"] == "n5")
        .unwrap();
    assert_eq!(job4["status"], "RUNNING");
}

#[tokio::test]
async fn test_monotonic_fencing_cas_and_409_across_endpoints() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, router, tok_a, tok_b) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok_a, "cas-test").await;

    // Reject /flow/jobs/events for non-enqueued jobs (404 JOB_NOT_FOUND)
    let (st_miss, err_miss) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", proj.id),
        &tok_a,
        Some(json!({
            "jobId": "job-never-enqueued",
            "sessionId": "sess-cas-1",
            "nodeId": "node-1",
            "eventKind": "started",
            "payloadText": "init",
            "jobStatus": "RUNNING",
            "expectedFencingToken": 1
        })),
    )
    .await;
    assert_eq!(st_miss, StatusCode::NOT_FOUND, "{err_miss}");
    assert_eq!(err_miss["code"], "JOB_NOT_FOUND");

    // Enqueue first, verify stamped daemon_nonce
    let (st1, enq1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/enqueue", proj.id),
        &tok_a,
        Some(json!({
            "sessionId": "sess-cas-1",
            "nodeIds": ["node-1"],
            "prompt": "init",
            "mode": "parallel"
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::CREATED, "{enq1}");
    let job_cas_id = enq1["jobs"][0]["id"].as_str().unwrap().to_string();
    let nonce_db: String = state
        .registry
        .pool()
        .connection()
        .unwrap()
        .query_row(
            "SELECT daemon_nonce FROM node_execution_jobs WHERE id = ?1",
            rusqlite::params![job_cas_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(nonce_db, state.auth.daemon_nonce);

    // Without expectedFencingToken -> 409 STALE_FENCING_TOKEN
    let (st_missing, err_missing) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", proj.id),
        &tok_a,
        Some(json!({
            "jobId": job_cas_id,
            "sessionId": "sess-cas-1",
            "nodeId": "node-1",
            "eventKind": "chunk",
            "payloadText": "stale",
            "jobStatus": "RUNNING"
        })),
    )
    .await;
    assert_eq!(st_missing, StatusCode::CONFLICT, "{err_missing}");
    assert_eq!(err_missing["code"], "STALE_FENCING_TOKEN");

    // Valid token succeeds and bumps fencing_token to 2
    let (st2, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", proj.id),
        &tok_a,
        Some(json!({
            "jobId": job_cas_id,
            "sessionId": "sess-cas-1",
            "nodeId": "node-1",
            "eventKind": "chunk",
            "payloadText": "valid",
            "jobStatus": "RUNNING",
            "expectedFencingToken": 1
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::CREATED);

    // Stale steer with token 1 -> 409 STALE_FENCING_TOKEN
    let (st_steer_stale, err_steer) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/steer", proj.id),
        &tok_a,
        Some(json!({
            "jobId": job_cas_id,
            "expectedFencingToken": 1,
            "instruction": "Switch palette"
        })),
    )
    .await;
    assert_eq!(st_steer_stale, StatusCode::CONFLICT, "{err_steer}");
    assert_eq!(err_steer["code"], "STALE_FENCING_TOKEN");

    // Terminal COMPLETED event then refuse reopening COMPLETED -> RUNNING
    let (st_done, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", proj.id),
        &tok_a,
        Some(json!({
            "jobId": job_cas_id,
            "sessionId": "sess-cas-1",
            "nodeId": "node-1",
            "eventKind": "completed",
            "payloadText": "done",
            "jobStatus": "COMPLETED",
            "expectedFencingToken": 2
        })),
    )
    .await;
    assert_eq!(st_done, StatusCode::CREATED);
    let (st_reopen, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", proj.id),
        &tok_a,
        Some(json!({
            "jobId": job_cas_id,
            "sessionId": "sess-cas-1",
            "nodeId": "node-1",
            "eventKind": "started",
            "payloadText": "reopen",
            "jobStatus": "RUNNING",
            "expectedFencingToken": 3
        })),
    )
    .await;
    assert_eq!(st_reopen, StatusCode::CONFLICT);

    // Cross-tenant node-sessions blocked with 403
    let (st_idor, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/flow/node-sessions", proj.id),
        &tok_b,
        None,
    )
    .await;
    assert_eq!(st_idor, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_pgid_reap_checked_and_shadow_discard_on_steer() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, router, tok_a, _) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok_a, "reap-test").await;

    let (_, enq) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/enqueue", proj.id),
        &tok_a,
        Some(json!({
            "sessionId": "sess-reap-1",
            "nodeIds": ["n-reap"],
            "prompt": "Run CLI",
            "mode": "parallel"
        })),
    )
    .await;
    let job = &enq["jobs"][0];
    let job_id = job["id"].as_str().unwrap().to_string();
    let tok = job["fencingToken"].as_i64().unwrap();

    // Spawn real child ignoring SIGTERM in its own PGID
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "trap '' TERM; sleep 30"]);
    let (mut child, pid, pgid, start_time) = spawn_pgid_child(cmd).expect("spawn pgid child");

    // Wrong start time must refuse to kill (PID reuse defense)
    let refused = terminate_pgid_checked(pid, pgid, "Thu Jan  1 00:00:00 1970").unwrap();
    assert!(!refused);
    assert!(child.try_wait().unwrap().is_none());

    let pid_str = proj.id.to_string();
    let shadow_dir = shadow_dir_for(tmp.path(), &pid_str, &job_id).unwrap();
    fs::create_dir_all(&shadow_dir).unwrap();
    fs::write(shadow_dir.join("dirty.tsx"), "unpromoted").unwrap();

    state.active_workers.register(ActiveWorkerEntry {
        job_id: job_id.clone(),
        project_id: pid_str,
        fencing_token: tok,
        pid,
        pgid,
        start_time,
        shadow_dir: shadow_dir.clone(),
    });

    let t0 = Instant::now();
    let (st, steered) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/steer", proj.id),
        &tok_a,
        Some(json!({
            "jobId": job_id,
            "expectedFencingToken": tok,
            "instruction": "Abort previous and regenerate",
            "pgidReapRequested": true
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{steered}");
    assert_eq!(steered["fencingToken"], tok + 1);
    assert!(!shadow_dir.exists(), "shadow dir must be discarded on steer");
    assert!(t0.elapsed() < Duration::from_millis(1500));
    let _ = child.wait();
    assert_eq!(unsafe { libc::kill(-pgid, 0) }, -1);
}

#[tokio::test]
async fn test_startup_orphan_reaper_5s_lease_and_persistent_sse() {
    let tmp = tempfile::tempdir().unwrap();
    let (state, router, tok_a, _) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok_a, "reaper-sse").await;
    let pool = state.registry.pool();

    let pid_str = proj.id.to_string();

    // Seed old-nonce orphan RUNNING & PAUSED jobs + expired lease job
    {
        let conn = pool.connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO chat_sessions (id, tenant_id, project_id, title, model_provider, created_at, updated_at)
             VALUES ('sess-orp', 'tenant-a', ?1, 'Orphan', 'default', ?2, ?2)",
            rusqlite::params![pid_str, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO node_execution_jobs (id, tenant_id, project_id, session_id, node_id, composite_scope_key, daemon_nonce, status, fencing_token, priority, stale_after_timestamp, created_at, updated_at)
             VALUES ('job-orp-1', 'tenant-a', ?1, 'sess-orp', 'n1', ?2, 'dead-nonce-000', 'RUNNING', 1, 10, ?3, ?4, ?4)",
            rusqlite::params![pid_str, format!("{pid_str}::sess-orp::n1"), now + 10_000, now],
        )
        .unwrap();
    }

    let reaped = reap_orphan_jobs_on_startup(pool, &state.auth.daemon_nonce)
        .await
        .unwrap();
    assert_eq!(reaped, 1);

    // Seed expired lease job with current nonce
    {
        let conn = pool.connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO node_execution_jobs (id, tenant_id, project_id, session_id, node_id, composite_scope_key, daemon_nonce, status, fencing_token, priority, stale_after_timestamp, created_at, updated_at)
             VALUES ('job-exp-1', 'tenant-a', ?1, 'sess-orp', 'n2', ?2, ?3, 'RUNNING', 1, 10, ?4, ?5, ?5)",
            rusqlite::params![pid_str, format!("{pid_str}::sess-orp::n2"), state.auth.daemon_nonce, now - 1000, now],
        )
        .unwrap();
    }
    let swept = sweep_expired_leases(pool, &state.sse_tx).await.unwrap();
    assert_eq!(swept, 1);

    // Persistent SSE stream stays open and receives live broadcast events after Last-Event-ID
    let sse_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/projects/{}/flow/jobs/stream", proj.id))
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", &tok_a)
        .header("last-event-id", "1")
        .body(Body::empty())
        .unwrap();
    let sse_resp = router.clone().oneshot(sse_req).await.unwrap();
    assert_eq!(sse_resp.status(), StatusCode::OK);
    let mut sse_body = sse_resp.into_body();
    let frame1 = tokio::time::timeout(Duration::from_secs(2), sse_body.frame())
        .await
        .expect("backlog frame timeout")
        .expect("stream closed")
        .expect("frame ok");
    let text1 = String::from_utf8_lossy(frame1.data_ref().unwrap()).into_owned();
    assert!(text1.contains("id: 2"), "got {text1}");

    // Enqueue then emit another event while stream stays open
    let (enq_live_st, enq_live) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/enqueue", proj.id),
        &tok_a,
        Some(json!({
            "sessionId": "sess-orp",
            "nodeIds": ["n3"],
            "prompt": "live",
            "mode": "parallel"
        })),
    )
    .await;
    assert_eq!(enq_live_st, StatusCode::CREATED, "{enq_live}");
    let live_jid = enq_live["jobs"][0]["id"].as_str().unwrap().to_string();
    let (ev_st, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/jobs/events", proj.id),
        &tok_a,
        Some(json!({
            "jobId": live_jid,
            "sessionId": "sess-orp",
            "nodeId": "n3",
            "eventKind": "chunk",
            "payloadText": "live-push",
            "jobStatus": "RUNNING",
            "expectedFencingToken": 1
        })),
    )
    .await;
    assert_eq!(ev_st, StatusCode::CREATED);

    let frame2 = tokio::time::timeout(Duration::from_secs(2), sse_body.frame())
        .await
        .expect("live frame timeout")
        .expect("stream closed unexpectedly")
        .expect("frame ok");
    let text2 = String::from_utf8_lossy(frame2.data_ref().unwrap()).into_owned();
    assert!(text2.contains("live-push"), "got {text2}");
}

#[tokio::test]
async fn test_stage4_post_commit_reap_interrupted_resume_and_sse_pagination() {
    use harness_daemon::routes::execution_reaper::{
        reap_orphan_jobs_with_workers, spawn_managed_worker_heartbeat, sweep_expired_leases_with_workers,
    };

    let tmp = tempfile::tempdir().unwrap();
    let (state, router, tok_a, _) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok_a, "stage4-hardening").await;
    let pid_str = proj.id.to_string();

    // 1. Spawn active worker + shadow dir; stale 409 steer/cancel must NOT reap or delete shadow
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "sleep 30"]);
    let (mut child, pid, pgid, start_time) = spawn_pgid_child(cmd).unwrap();
    assert!(terminate_pgid_checked(pid, pgid, "").is_err());

    let (_, enq) = send_req(
        &router, "POST", &format!("/api/v1/projects/{}/flow/jobs/enqueue", proj.id), &tok_a,
        Some(json!({ "sessionId": "sess-s4", "nodeIds": ["n1", "n2", "n3", "n4", "n5"], "prompt": "go", "mode": "parallel" })),
    ).await;
    let jobs = enq["jobs"].as_array().unwrap();
    let j0_id = jobs[0]["id"].as_str().unwrap().to_string();
    let j4_id = jobs[4]["id"].as_str().unwrap().to_string();
    assert_eq!(jobs[4]["status"], "QUEUED");

    let shadow_dir = shadow_dir_for(tmp.path(), &pid_str, &j0_id).unwrap();
    fs::create_dir_all(&shadow_dir).unwrap();
    fs::write(shadow_dir.join("keep.txt"), "live").unwrap();
    state.active_workers.register(ActiveWorkerEntry {
        job_id: j0_id.clone(), project_id: pid_str.clone(), fencing_token: 1,
        pid, pgid, start_time, shadow_dir: shadow_dir.clone(),
    });
    let _hb = spawn_managed_worker_heartbeat(state.registry.pool().clone(), state.active_workers.clone(), j0_id.clone(), 1);

    let (st_stale, _) = send_req(
        &router, "POST", &format!("/api/v1/projects/{}/flow/jobs/steer", proj.id), &tok_a,
        Some(json!({ "jobId": j0_id, "expectedFencingToken": 999, "instruction": "stale", "pgidReapRequested": true })),
    ).await;
    assert_eq!(st_stale, StatusCode::CONFLICT);
    assert!(shadow_dir.exists(), "409 steer must not discard shadow dir");
    assert!(child.try_wait().unwrap().is_none(), "409 steer must not kill PGID");

    // 2. Mark j1 INTERRUPTED and expire j0 lease -> sweep_expired_leases_with_workers promotes j4 + reaps j0 + cleans orphan shadow dir
    let j1_id = jobs[1]["id"].as_str().unwrap().to_string();
    {
        let conn = state.registry.pool().connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE node_execution_jobs SET status = 'INTERRUPTED', stale_after_timestamp = NULL WHERE id = ?1",
            rusqlite::params![j1_id],
        ).unwrap();
        conn.execute(
            "UPDATE node_execution_jobs SET stale_after_timestamp = ?1 WHERE id = ?2",
            rusqlite::params![now - 1000, j0_id],
        ).unwrap();
        let orphan_shadow = shadow_dir_for(tmp.path(), &pid_str, "orphan-disk-job").unwrap();
        fs::create_dir_all(&orphan_shadow).unwrap();
    }
    let swept = sweep_expired_leases_with_workers(
        state.registry.pool(), &state.active_workers, Some(tmp.path()), &state.auth.daemon_nonce, &state.sse_tx,
    ).await.unwrap();
    assert_eq!(swept, 1);
    let _ = child.wait();
    assert!(!shadow_dir.exists());

    // Now j4 was promoted (slots: n3, n4, j4 = 3 active; n1 INTERRUPTED; n2 INTERRUPTED). Verify INTERRUPTED -> QUEUED promotion when < 4
    let (res_st, resumed) = send_req(
        &router, "POST", &format!("/api/v1/projects/{}/flow/jobs/resume", proj.id), &tok_a,
        Some(json!({ "jobId": j1_id, "expectedFencingToken": 1 })),
    ).await;
    assert_eq!(res_st, StatusCode::OK, "{resumed}");
    assert_eq!(resumed["status"], "RUNNING");
    assert!(resumed["fencingToken"].as_i64().unwrap() >= 3);

    // Fill 4 slots and verify resuming j0 (INTERRUPTED) transitions to QUEUED (not RUNNING!)
    let (res0_st, res0) = send_req(
        &router, "POST", &format!("/api/v1/projects/{}/flow/jobs/resume", proj.id), &tok_a,
        Some(json!({ "jobId": j0_id, "expectedFencingToken": 2 })),
    ).await;
    assert_eq!(res0_st, StatusCode::OK, "{res0}");
    assert_eq!(res0["status"], "QUEUED");

    // 3. Startup orphan shadow cleanup removes non-active disk shadow dir
    reap_orphan_jobs_with_workers(state.registry.pool(), &state.active_workers, Some(tmp.path()), &state.auth.daemon_nonce).await.unwrap();
    assert!(!shadow_dir_for(tmp.path(), &pid_str, "orphan-disk-job").unwrap().exists());

    // 4. Test unread reset & >50 SSE events on COMPLETED job without truncation + nodeId/jobId filter + Last-Event-ID precedence
    for i in 0..55 {
        let is_last = i == 54;
        let (st_ev, _) = send_req(
            &router, "POST", &format!("/api/v1/projects/{}/flow/jobs/events", proj.id), &tok_a,
            Some(json!({
                "jobId": j4_id, "sessionId": "sess-s4", "nodeId": "n5",
                "eventKind": if is_last { "completed" } else { "chunk" },
                "payloadText": format!("chunk-{i}"),
                "jobStatus": if is_last { "COMPLETED" } else { "RUNNING" },
                "expectedFencingToken": 2 + i as i64
            })),
        ).await;
        assert_eq!(st_ev, StatusCode::CREATED);
    }

    let (read_st, read_res) = send_req(
        &router, "POST", &format!("/api/v1/projects/{}/flow/node-sessions/read", proj.id), &tok_a,
        Some(json!({ "sessionId": "sess-s4", "nodeId": "n5" })),
    ).await;
    assert_eq!(read_st, StatusCode::OK, "{read_res}");
    assert_eq!(read_res["ok"], true);

    // Last-Event-ID: 0 overrides afterSeq=99999 and combined jobId+nodeId returns all >50 events before closing
    let sse_req = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/projects/{}/flow/jobs/stream?jobId={}&nodeId=n5&afterSeq=99999", proj.id, j4_id))
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", &tok_a)
        .header("last-event-id", "0")
        .body(Body::empty())
        .unwrap();
    let sse_resp = router.clone().oneshot(sse_req).await.unwrap();
    let mut body = sse_resp.into_body();
    let mut combined = String::new();
    while let Ok(Some(Ok(frame))) = tokio::time::timeout(Duration::from_secs(2), body.frame()).await {
        if let Some(d) = frame.data_ref() {
            combined.push_str(&String::from_utf8_lossy(d));
        }
    }
    assert!(combined.contains("chunk-0"), "missing chunk-0");
    assert!(combined.contains("chunk-54"), "truncated >50 events: missing chunk-54");

    // 5. Plugins endpoint responds 200 JSON array
    let (pl_st, pl_body) = send_req(&router, "GET", "/api/v1/plugins?enabled=true", &tok_a, None).await;
    assert_eq!(pl_st, StatusCode::OK, "{pl_body}");
    assert!(pl_body.is_array());
}
