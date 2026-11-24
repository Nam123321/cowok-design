// @story 10.1
// @implements AC-01, AC-02, AC-03, AC-04 (50-Writer Concurrent MPSC Zero SQLITE_BUSY, 429 Backpressure, 3-Lane Router, $UIUX_LIVE_SPEC O_NOFOLLOW/512KB/Malformed/Cmd+Z & CAS Provenance Backlinks)
use std::fs;
use std::sync::{Arc, Barrier};

use axum::{
    body::Body,
    extract::Extension,
    http::{HeaderMap, Method, Request, StatusCode},
    Router,
};
use bot_fleet::{
    build_lane2_pty_env, read_live_spec_file_nofollow, IsolatedSessionDir, OrchestratorError,
    LIVE_SPEC_ENV_VAR, LIVE_SPEC_MAX_BYTES,
};
use chrono::Utc;
use harness_daemon::daemon_auth::{DaemonAuthState, FlowPrincipal};
use harness_daemon::routes::orchestrator::build_orchestrator_router_with_actor;
use harness_daemon::routes::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use spec_registry::{
    SpecRegistry, SqliteActorError, SqlitePool, SqliteWalConfig, SqliteWriteActor,
};
use tempfile::tempdir;
use tower::ServiceExt;

struct OrchestratorHarness {
    dir: tempfile::TempDir,
    db_path: std::path::PathBuf,
    registry: Arc<SpecRegistry>,
    state: DaemonAppState,
    app: Router,
    project_a: String,
    editor_a_tok: String,
    viewer_a_tok: String,
    editor_b_tok: String,
}

fn setup_harness() -> OrchestratorHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-101.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let project_a = uuid::Uuid::new_v4().to_string();
    {
        let conn = registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES (?1, 'tenant-a', 'proj-101', 'Orchestrator Proj', ?2, '', 'ACTIVE', 1000, 1000)",
            rusqlite::params![project_a, dir.path().to_string_lossy().as_ref()],
        )
        .expect("seed project");
    }

    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_a_tok = format!("cwk_sess_101_ed_a_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        editor_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_a_tok = format!("cwk_sess_101_vw_a_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        viewer_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "viewer-a".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let editor_b_tok = format!("cwk_sess_101_ed_b_{}", uuid::Uuid::new_v4().simple());
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
        DaemonAppState::with_auth(Arc::clone(&registry), "127.0.0.1".to_string(), 4170, auth);
    let _ = harness_daemon::routes::orchestrator::shared_write_actor_for_pool(registry.pool());
    let app = build_router(state.clone());

    OrchestratorHarness {
        dir,
        db_path,
        registry,
        state,
        app,
        project_a,
        editor_a_tok,
        viewer_a_tok,
        editor_b_tok,
    }
}

async fn send_req(
    app: &Router,
    method: Method,
    uri: &str,
    session_token: Option<&str>,
    bearer_token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170");
    if let Some(tok) = session_token {
        builder = builder.header("x-cowok-session-token", tok);
    }
    if let Some(b) = bearer_token {
        builder = builder.header("authorization", format!("Bearer {b}"));
    }
    let req = if let Some(json) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(json.to_string()))
            .expect("req")
    } else {
        builder.body(Body::empty()).expect("req")
    };

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, parsed)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn assert_zero_floats_in_json(val: &Value) {
    match val {
        Value::Number(n) => {
            assert!(
                n.is_i64() || n.is_u64(),
                "Leaked floating point number into VerdictShield/Orchestrator JSON: {n}"
            );
        }
        Value::Array(arr) => {
            for item in arr {
                assert_zero_floats_in_json(item);
            }
        }
        Value::Object(map) => {
            for (_, v) in map {
                assert_zero_floats_in_json(v);
            }
        }
        _ => {}
    }
}

#[tokio::test]
async fn it_10_1_01_mpsc_actor_50_parallel_writers_zero_sqlite_busy() {
    let h = setup_harness();
    let actor = SqliteWriteActor::from_pool(h.registry.pool()).expect("spawn actor");

    let mut handles = Vec::with_capacity(50);
    for idx in 0..50usize {
        let actor_c = actor.clone();
        let pid = h.project_a.clone();
        handles.push(tokio::spawn(async move {
            let receipt = actor_c.try_enqueue_immediate(move |tx| {
                tx.execute(
                    "INSERT INTO decision_audit_log (
                        id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                        verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                        stale_file_sha256, latency_ms, created_at
                     ) VALUES (?1, 'tenant-a', ?2, 'DECISION_1', 'LOCAL_FTS5_BM25', 'ExactMatch', ?3, NULL, 1, 0, NULL, 1, ?4)",
                    rusqlite::params![
                        format!("mpsc-stress-{idx}"),
                        pid,
                        format!("STRESS_WRITE_{idx}"),
                        10_000 + idx as i64,
                    ],
                )?;
                Ok(())
            })?;
            tokio::task::spawn_blocking(move || receipt.wait())
                .await
                .expect("join blocking")
        }));
    }

    let mut busy_errors = 0usize;
    let mut other_errors = Vec::new();
    let mut ok_count = 0usize;
    for handle in handles {
        match handle.await.expect("task join") {
            Ok(()) => ok_count += 1,
            Err(err) => {
                if err.to_string().contains("SQLITE_BUSY") || err.to_string().contains("locked") {
                    busy_errors += 1;
                } else {
                    other_errors.push(err.to_string());
                }
            }
        }
    }

    assert_eq!(busy_errors, 0, "Expected 0 SQLITE_BUSY errors");
    assert!(
        other_errors.is_empty(),
        "Unexpected MPSC actor write failures: {other_errors:?}"
    );
    assert_eq!(ok_count, 50, "Expected 50/50 parallel writes committed");

    let conn = h.registry.pool().connection().expect("read conn");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decision_audit_log WHERE project_id = ?1",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count rows");
    assert_eq!(count, 50);
}

#[tokio::test]
async fn it_10_1_02_mpsc_actor_backpressure_returns_deterministic_429_retry_after() {
    let h = setup_harness();
    let tiny_actor =
        SqliteWriteActor::spawn_with_capacity(&h.db_path, 1).expect("spawn capacity=1 actor");

    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let entered_c = Arc::clone(&entered);
    let release_c = Arc::clone(&release);
    let r1 = tiny_actor
        .try_enqueue_immediate(move |_tx| {
            entered_c.wait();
            release_c.wait();
            Ok(())
        })
        .expect("job 1 accepted");

    // Wait until actor OS thread is actively executing job 1.
    entered.wait();

    // Job 2 saturates the capacity=1 bounded channel buffer.
    let r2 = tiny_actor
        .try_enqueue_immediate(|_tx| Ok(()))
        .expect("job 2 fills buffer");

    // Direct API check: job 3 MUST fail-fast with Backpressure { retry_after_secs: 1, queue_capacity: 1 }.
    let direct_err = tiny_actor.try_enqueue_immediate(|_tx| Ok(()));
    assert!(matches!(
        direct_err,
        Err(SqliteActorError::Backpressure {
            retry_after_secs: 1,
            queue_capacity: 1
        })
    ));

    // HTTP route check: POST /api/v1/orchestrator/chat MUST return 429 Too Many Requests + Retry-After: 1.
    let bp_app = build_orchestrator_router_with_actor(tiny_actor.clone())
        .layer(Extension(FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        }))
        .with_state(h.state.clone());

    let payload = serde_json::json!({
        "projectId": h.project_a,
        "sessionId": "sess-bp-1",
        "message": "lint_fix",
        "commandKind": "lint_fix"
    });
    let (status, headers, body) = send_req(
        &bp_app,
        Method::POST,
        "/api/v1/orchestrator/chat",
        None,
        None,
        Some(payload),
    )
    .await;

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "body: {body}");
    assert_eq!(
        headers.get("retry-after").and_then(|v| v.to_str().ok()),
        Some("1"),
        "Missing Retry-After: 1 header"
    );
    assert_eq!(body["error"]["code"], "ACTOR_BACKPRESSURE");

    // Release barrier so actor finishes cleanly with zero dropped jobs.
    release.wait();
    r1.wait().expect("r1 committed");
    r2.wait().expect("r2 committed");
    assert_eq!(tiny_actor.queue_depth(), 0);
}

#[tokio::test]
async fn it_10_1_03_three_lane_router_latency_zero_cost_zero_floats_and_pty_env() {
    let h = setup_harness();

    // Lane 0: Deterministic CSS token update (<50ms, tokenCost == 0)
    let lane0_payload = serde_json::json!({
        "projectId": h.project_a,
        "sessionId": "sess-lane-0",
        "nodeId": "hero-card",
        "message": "update css token --bg-surface: #0F172A",
        "commandKind": "token_update"
    });
    let (st0, _, body0) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/chat",
        Some(&h.editor_a_tok),
        None,
        Some(lane0_payload),
    )
    .await;
    assert_eq!(st0, StatusCode::OK, "lane0 body: {body0}");
    assert_eq!(body0["data"]["lane"], "LANE_0_DETERMINISTIC");
    assert_eq!(body0["data"]["tokenCost"], 0);
    assert!(
        body0["data"]["latencyMs"].as_u64().unwrap_or(999) < 50,
        "Lane 0 exceeded 50ms budget: {body0}"
    );
    assert_eq!(body0["data"]["canUndo"], true);

    // Lane 1: Decision Cascade delegation (0 floats, no logprobs)
    let lane1_payload = serde_json::json!({
        "projectId": h.project_a,
        "sessionId": "sess-lane-1",
        "message": "Should we optimize contrast and accessibility polish on the pricing section?",
        "commandKind": "auto"
    });
    let (st1, _, body1) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/chat",
        Some(&h.editor_a_tok),
        None,
        Some(lane1_payload),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "lane1 body: {body1}");
    assert_eq!(body1["data"]["lane"], "LANE_1_DECISION_CASCADE");
    assert_zero_floats_in_json(&body1);
    assert!(
        !body1.to_string().contains("logprob"),
        "Lane 1 leaked logprob"
    );

    // Lane 2: OPC Execution Fleet with jailed $UIUX_LIVE_SPEC path & portable-pty env injection
    let lane2_payload = serde_json::json!({
        "projectId": h.project_a,
        "sessionId": "sess-lane-2",
        "message": "Spawn worker swarm to build full multi-step fintech onboarding dashboard",
        "commandKind": "auto"
    });
    let (st2, _, body2) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/chat",
        Some(&h.editor_a_tok),
        None,
        Some(lane2_payload),
    )
    .await;
    assert_eq!(st2, StatusCode::OK, "lane2 body: {body2}");
    assert_eq!(body2["data"]["lane"], "LANE_2_OPC_FLEET");
    let live_path = body2["data"]["liveSpecPath"]
        .as_str()
        .expect("liveSpecPath must be present for Lane 2");
    assert!(
        live_path.contains(".cowok-design/live-spec/") && live_path.ends_with("doc-patch.jsonl"),
        "Unexpected liveSpecPath: {live_path}"
    );

    // Verify PTY env injection helper injects $UIUX_LIVE_SPEC cleanly alongside $COWOK_SESSION_DIR
    let isolated = IsolatedSessionDir::create_ephemeral("sess-lane-2").expect("session dir");
    let pty_env = build_lane2_pty_env(&isolated, std::path::Path::new(live_path));
    assert!(pty_env
        .iter()
        .any(|(k, v)| k == LIVE_SPEC_ENV_VAR && v == live_path));
    assert!(pty_env.iter().any(|(k, _)| k == "COWOK_SESSION_DIR"));
}

#[tokio::test]
async fn it_10_1_04_live_spec_security_symlink_oversize_malformed_hot_apply_and_cmd_z_undo() {
    let h = setup_harness();
    let live_dir = h.dir.path().join(".cowok-design").join("live-spec");
    fs::create_dir_all(&live_dir).expect("create live-spec dir");

    // 1. O_NOFOLLOW symlink rejection
    #[cfg(unix)]
    {
        let secret_target = h.dir.path().join("outside-secret.txt");
        fs::write(&secret_target, "TOP_SECRET").expect("write target");
        let symlink_path = live_dir.join("evil-link.jsonl");
        std::os::unix::fs::symlink(&secret_target, &symlink_path).expect("create symlink");

        let direct_err = read_live_spec_file_nofollow(h.dir.path(), &symlink_path);
        assert!(
            matches!(direct_err, Err(OrchestratorError::SymlinkViolation(_))),
            "Expected SymlinkViolation, got {direct_err:?}"
        );

        let sym_payload = serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-sec-1",
            "action": "apply",
            "liveSpecFilePath": ".cowok-design/live-spec/evil-link.jsonl"
        });
        let (st_sym, _, body_sym) = send_req(
            &h.app,
            Method::POST,
            "/api/v1/orchestrator/live-patch",
            Some(&h.editor_a_tok),
            None,
            Some(sym_payload),
        )
        .await;
        assert_eq!(st_sym, StatusCode::FORBIDDEN, "sym body: {body_sym}");
        assert_eq!(body_sym["error"]["code"], "SYMLINK_VIOLATION");
    }

    // 2. > 512 KB payload rejection (both file & inline)
    let oversize_file = live_dir.join("oversize.jsonl");
    fs::write(&oversize_file, "a".repeat(LIVE_SPEC_MAX_BYTES + 1024)).expect("write oversize");
    let direct_size_err = read_live_spec_file_nofollow(h.dir.path(), &oversize_file);
    assert!(matches!(
        direct_size_err,
        Err(OrchestratorError::PayloadTooLarge { .. })
    ));

    let oversize_req = serde_json::json!({
        "projectId": h.project_a,
        "sessionId": "sess-sec-1",
        "action": "apply",
        "liveSpecFilePath": ".cowok-design/live-spec/oversize.jsonl"
    });
    let (st_big, _, body_big) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(oversize_req),
    )
    .await;
    assert_eq!(
        st_big,
        StatusCode::PAYLOAD_TOO_LARGE,
        "big body: {body_big}"
    );
    assert_eq!(body_big["error"]["code"], "LIVE_SPEC_PAYLOAD_TOO_LARGE");

    // 3. Valid patch hot-apply (<50ms)
    let valid_jsonl_1 = serde_json::json!({
        "screenId": "checkout-screen",
        "ffId": "hero-title",
        "opKind": "TEXT_EDIT",
        "payload": { "text": "Stable Title V1" }
    })
    .to_string();
    let (st_v1, _, body_v1) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-sec-1",
            "nodeId": "node-checkout",
            "screenId": "checkout-screen",
            "action": "apply",
            "liveSpecPayload": valid_jsonl_1
        })),
    )
    .await;
    assert_eq!(st_v1, StatusCode::OK, "v1 body: {body_v1}");
    assert!(body_v1["data"]["latencyMs"].as_u64().unwrap_or(999) < 50);
    assert_eq!(body_v1["data"]["canUndo"], true);
    let sha_v1 = body_v1["data"]["canvasSha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(body_v1["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Stable Title V1"));

    // 4. Malformed JSONL & unknown-field JSONL rejected cleanly WITHOUT altering stable V1 state
    let malformed_jsonl = "{not-valid-json\n{\"screenId\":\"checkout-screen\"}";
    let (st_bad, _, body_bad) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-sec-1",
            "screenId": "checkout-screen",
            "action": "apply",
            "liveSpecPayload": malformed_jsonl
        })),
    )
    .await;
    assert_eq!(
        st_bad,
        StatusCode::UNPROCESSABLE_ENTITY,
        "bad body: {body_bad}"
    );
    assert_eq!(body_bad["error"]["code"], "INVALID_LIVE_SPEC_PATCH");

    let unknown_field_jsonl = serde_json::json!({
        "screenId": "checkout-screen",
        "ffId": "hero-title",
        "opKind": "TEXT_EDIT",
        "payload": { "text": "Evil Override" },
        "injectedField": true
    })
    .to_string();
    let (st_unk, _, body_unk) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-sec-1",
            "screenId": "checkout-screen",
            "action": "apply",
            "liveSpecPayload": unknown_field_jsonl
        })),
    )
    .await;
    assert_eq!(
        st_unk,
        StatusCode::UNPROCESSABLE_ENTITY,
        "unk body: {body_unk}"
    );
    assert_eq!(body_unk["error"]["code"], "INVALID_LIVE_SPEC_PATCH");

    // 5. Apply V2, then 1-step Cmd+Z undo -> must restore exact V1 canvasSha256 in <50ms!
    let valid_jsonl_2 = serde_json::json!({
        "screenId": "checkout-screen",
        "ffId": "hero-title",
        "opKind": "TEXT_EDIT",
        "payload": { "text": "Title V2" }
    })
    .to_string();
    let (st_v2, _, body_v2) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-sec-1",
            "screenId": "checkout-screen",
            "action": "apply",
            "liveSpecPayload": valid_jsonl_2
        })),
    )
    .await;
    assert_eq!(st_v2, StatusCode::OK, "v2 body: {body_v2}");
    let sha_v2 = body_v2["data"]["canvasSha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(sha_v1, sha_v2);

    let (st_undo, _, body_undo) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-sec-1",
            "screenId": "checkout-screen",
            "action": "undo"
        })),
    )
    .await;
    assert_eq!(st_undo, StatusCode::OK, "undo body: {body_undo}");
    assert!(body_undo["data"]["latencyMs"].as_u64().unwrap_or(999) < 50);
    assert_eq!(
        body_undo["data"]["canvasSha256"].as_str(),
        Some(sha_v1.as_str()),
        "1-step Cmd+Z must restore exact V1 Canvas SHA-256"
    );
    assert!(body_undo["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Stable Title V1"));
}

#[tokio::test]
async fn it_10_1_05_unified_asset_library_cas_blobs_dedup_and_auth_isolation() {
    let h = setup_harness();
    let shared_hash = hex_sha256(b"hero-brand-logo-svg-bytes");
    let second_hash = hex_sha256(b"dashboard-illustration-png");

    // Seed V16 cas_blobs + duplicate cas_attachments referencing the same content_hash
    {
        let conn = h.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO chat_sessions (id, project_id, title, model_provider, created_at, updated_at)
             VALUES ('sess-asset-1', ?1, 'Asset Chat', 'default', 1000, 1000)",
            rusqlite::params![h.project_a],
        )
        .expect("seed chat_session");
        conn.execute(
            "INSERT INTO cas_blobs (sha256, tenant_id, byte_size, mime_type, file_ext, storage_relpath, ref_count, gc_lease_until, created_at)
             VALUES (?1, 'tenant-a', 512, 'image/svg+xml', 'svg', 'cas/hero.svg', 2, 0, 2000)",
            rusqlite::params![shared_hash],
        )
        .expect("seed blob 1");
        conn.execute(
            "INSERT INTO cas_attachments (id, blob_sha256, tenant_id, project_id, session_id, filename, created_at)
             VALUES ('att-1', ?1, 'tenant-a', ?2, 'sess-asset-1', 'hero-a.svg', 2001),
                    ('att-2', ?1, 'tenant-a', ?2, 'sess-asset-1', 'hero-b.svg', 2002)",
            rusqlite::params![shared_hash, h.project_a],
        )
        .expect("seed duplicate attachments");
    }

    // Apply live-patch referencing both shared_hash (enriches with nodeId + patchId) and second_hash
    let patch_jsonl = format!(
        "{}\n{}",
        serde_json::json!({
            "patchId": "patch-asset-link-1",
            "screenId": "brand-screen",
            "ffId": "hero-title",
            "opKind": "TEXT_EDIT",
            "payload": { "text": "Brand Header" },
            "assetContentHash": shared_hash,
            "assetMimeType": "image/svg+xml"
        }),
        serde_json::json!({
            "patchId": "patch-asset-link-1",
            "screenId": "brand-screen",
            "ffId": "stage-root",
            "opKind": "TOKEN_SWAP",
            "payload": { "cssVars": { "--brand-primary": "#1D4ED8" } },
            "assetContentHash": second_hash,
            "assetMimeType": "image/png"
        })
    );
    let (st_patch, _, body_patch) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-asset-1",
            "nodeId": "node-brand-hero",
            "screenId": "brand-screen",
            "action": "apply",
            "liveSpecPayload": patch_jsonl
        })),
    )
    .await;
    assert_eq!(st_patch, StatusCode::OK, "patch body: {body_patch}");

    // GET /api/v1/orchestrator/assets deduplicates by contentHash and preserves provenance backlinks
    let (st_assets, _, body_assets) = send_req(
        &h.app,
        Method::GET,
        &format!("/api/v1/orchestrator/assets?projectId={}", h.project_a),
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_assets, StatusCode::OK, "assets body: {body_assets}");
    assert_eq!(body_assets["data"]["totalCount"], 2);
    let items = body_assets["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);

    let shared_item = items
        .iter()
        .find(|i| i["contentHash"].as_str() == Some(shared_hash.as_str()))
        .expect("shared_hash item");
    assert_eq!(shared_item["sessionId"], "sess-asset-1");
    assert_eq!(shared_item["nodeId"], "node-brand-hero");
    assert_eq!(shared_item["patchId"], "patch-asset-link-1");
    assert_eq!(shared_item["mimeType"], "image/svg+xml");

    // Auth & anti-IDOR checks
    let (st_unauth, _, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/chat",
        None,
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-1",
            "message": "lint_fix",
            "commandKind": "lint_fix"
        })),
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    let (st_viewer_post, _, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/chat",
        Some(&h.viewer_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-1",
            "message": "lint_fix",
            "commandKind": "lint_fix"
        })),
    )
    .await;
    assert_eq!(st_viewer_post, StatusCode::FORBIDDEN);

    let (st_idor, _, _) = send_req(
        &h.app,
        Method::GET,
        &format!("/api/v1/orchestrator/assets?projectId={}", h.project_a),
        Some(&h.editor_b_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_idor, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn it_10_1_06_fail_closed_mxss_path_jail_hardlink_enoent_resize_and_query_errors() {
    let h = setup_harness();
    let live_dir = h.dir.path().join(".cowok-design").join("live-spec");
    fs::create_dir_all(&live_dir).expect("create live-spec dir");

    // 1. Fail-closed mXSS (<script> & onerror=) in baseHtml -> HTTP 422 E_MXSS_BLOCKED, never persisted
    let valid_line = serde_json::json!({
        "screenId": "mxss-screen",
        "ffId": "hero-title",
        "opKind": "TEXT_EDIT",
        "payload": { "text": "Safe Text" }
    })
    .to_string();
    let (st_script, _, body_script) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-mxss",
            "screenId": "mxss-screen",
            "action": "apply",
            "baseHtml": "<!DOCTYPE html><html><body><script>alert(1)</script><h1 data-ff-id=\"hero-title\">Hi</h1></body></html>",
            "liveSpecPayload": valid_line
        })),
    )
    .await;
    assert_eq!(
        st_script,
        StatusCode::UNPROCESSABLE_ENTITY,
        "script body: {body_script}"
    );
    assert_eq!(body_script["error"]["code"], "E_MXSS_BLOCKED");

    let (st_onerror, _, body_onerror) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-mxss",
            "screenId": "mxss-screen",
            "action": "apply",
            "baseHtml": "<!DOCTYPE html><html><body><img src=\"/assets/a.png\" onerror=\"alert(1)\"><h1 data-ff-id=\"hero-title\">Hi</h1></body></html>",
            "liveSpecPayload": valid_line
        })),
    )
    .await;
    assert_eq!(
        st_onerror,
        StatusCode::UNPROCESSABLE_ENTITY,
        "onerror body: {body_onerror}"
    );
    assert_eq!(body_onerror["error"]["code"], "E_MXSS_BLOCKED");

    {
        let conn = h.registry.pool().connection().expect("conn");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM screen_artifact_patches WHERE project_id = ?1 AND screen_id = 'mxss-screen'",
                rusqlite::params![h.project_a],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(count, 0, "Blocked mXSS HTML must never be persisted");
    }

    // 2. Strict .cowok-design/live-spec/ path jail -> 403 PATH_TRAVERSAL for daemon.lock, vault.enc, .env, ..
    fs::write(h.dir.path().join(".cowok-design").join("daemon.lock"), "{}").expect("daemon.lock");
    for forbidden_path in [
        ".cowok-design/daemon.lock",
        "vault.enc",
        ".env",
        ".cowok-design/live-spec/../daemon.lock",
        ".cowok-design/live-spec/not-jsonl.txt",
    ] {
        let (st_jail, _, body_jail) = send_req(
            &h.app,
            Method::POST,
            "/api/v1/orchestrator/live-patch",
            Some(&h.editor_a_tok),
            None,
            Some(serde_json::json!({
                "projectId": h.project_a,
                "sessionId": "sess-jail",
                "action": "apply",
                "liveSpecFilePath": forbidden_path
            })),
        )
        .await;
        assert_eq!(
            st_jail,
            StatusCode::FORBIDDEN,
            "path={forbidden_path} body={body_jail}"
        );
        assert_eq!(body_jail["error"]["code"], "PATH_TRAVERSAL");
    }

    // 3. Hardlink rejection (nlink > 1) via openat + fstat
    #[cfg(unix)]
    {
        let secret_src = h.dir.path().join("vault-secret.jsonl");
        fs::write(&secret_src, &valid_line).expect("write secret");
        let hardlink_dst = live_dir.join("hardlinked.jsonl");
        fs::hard_link(&secret_src, &hardlink_dst).expect("hard_link");

        let direct_hl = read_live_spec_file_nofollow(h.dir.path(), &hardlink_dst);
        assert!(
            matches!(direct_hl, Err(OrchestratorError::SymlinkViolation(_))),
            "Expected hardlink SymlinkViolation, got {direct_hl:?}"
        );

        let (st_hl, _, body_hl) = send_req(
            &h.app,
            Method::POST,
            "/api/v1/orchestrator/live-patch",
            Some(&h.editor_a_tok),
            None,
            Some(serde_json::json!({
                "projectId": h.project_a,
                "sessionId": "sess-hl",
                "action": "apply",
                "liveSpecFilePath": ".cowok-design/live-spec/hardlinked.jsonl"
            })),
        )
        .await;
        assert_eq!(st_hl, StatusCode::FORBIDDEN, "hl body: {body_hl}");
        assert_eq!(body_hl["error"]["code"], "SYMLINK_VIOLATION");
    }

    // 4. ENOENT -> 404 LIVE_SPEC_NOT_FOUND without leaking workspace host path
    let (st_enoent, _, body_enoent) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-missing",
            "action": "apply",
            "liveSpecFilePath": ".cowok-design/live-spec/does-not-exist.jsonl"
        })),
    )
    .await;
    assert_eq!(st_enoent, StatusCode::NOT_FOUND, "enoent: {body_enoent}");
    assert_eq!(body_enoent["error"]["code"], "LIVE_SPEC_NOT_FOUND");
    assert!(
        !body_enoent
            .to_string()
            .contains(&h.dir.path().to_string_lossy().to_string()),
        "404 error leaked host workspace path: {body_enoent}"
    );

    // 5. Invalid assets query returns structured JSON 400
    let (st_q, _, body_q) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/orchestrator/assets",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_q, StatusCode::BAD_REQUEST, "query body: {body_q}");
    assert_eq!(body_q["error"]["code"], "INVALID_REQUEST");

    // 6. RESIZE patch genuinely mutates htmlSnapshot with bounded px dimensions
    let resize_jsonl = serde_json::json!({
        "screenId": "resize-screen",
        "ffId": "hero-title",
        "opKind": "RESIZE",
        "payload": { "width": 640, "height": 320 }
    })
    .to_string();
    let (st_rz, _, body_rz) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-rz",
            "screenId": "resize-screen",
            "action": "apply",
            "liveSpecPayload": resize_jsonl
        })),
    )
    .await;
    assert_eq!(st_rz, StatusCode::OK, "resize body: {body_rz}");
    let rz_html = body_rz["data"]["htmlSnapshot"].as_str().unwrap_or("");
    assert!(
        rz_html.contains("width: 640px") && rz_html.contains("height: 320px"),
        "RESIZE must mutate htmlSnapshot: {rz_html}"
    );
}

#[tokio::test]
async fn it_10_1_07_concurrent_chat_and_live_patch_no_lost_updates_and_two_step_cmd_z_undo() {
    let h = setup_harness();

    // 1. Consecutive 2-step Cmd+Z undo parity: V1 -> V2 -> V3 -> Undo(V2) -> Undo(V1)
    let apply_title = |title: &str| {
        serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-undo-multi",
            "screenId": "multi-undo-screen",
            "action": "apply",
            "liveSpecPayload": serde_json::json!({
                "screenId": "multi-undo-screen",
                "ffId": "hero-title",
                "opKind": "TEXT_EDIT",
                "payload": { "text": title }
            }).to_string()
        })
    };

    let (st1, _, b1) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(apply_title("Title V1")),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "v1: {b1}");
    let sha1 = b1["data"]["canvasSha256"].as_str().unwrap().to_string();

    let (st2, _, b2) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(apply_title("Title V2")),
    )
    .await;
    assert_eq!(st2, StatusCode::OK, "v2: {b2}");
    let sha2 = b2["data"]["canvasSha256"].as_str().unwrap().to_string();

    let (st3, _, b3) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(apply_title("Title V3")),
    )
    .await;
    assert_eq!(st3, StatusCode::OK, "v3: {b3}");
    assert!(b3["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Title V3"));

    let undo_req = || {
        serde_json::json!({
            "projectId": h.project_a,
            "sessionId": "sess-undo-multi",
            "screenId": "multi-undo-screen",
            "action": "undo"
        })
    };

    // Undo #1: V3 -> V2
    let (su1, _, bu1) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(undo_req()),
    )
    .await;
    assert_eq!(su1, StatusCode::OK, "undo1: {bu1}");
    assert_eq!(bu1["data"]["canvasSha256"].as_str(), Some(sha2.as_str()));
    assert!(
        bu1["data"]["htmlSnapshot"]
            .as_str()
            .unwrap()
            .contains("Title V2"),
        "Undo #1 must restore V2 HTML: {bu1}"
    );

    // Undo #2: V2 -> V1
    let (su2, _, bu2) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/orchestrator/live-patch",
        Some(&h.editor_a_tok),
        None,
        Some(undo_req()),
    )
    .await;
    assert_eq!(su2, StatusCode::OK, "undo2: {bu2}");
    assert_eq!(bu2["data"]["canvasSha256"].as_str(), Some(sha1.as_str()));
    assert!(
        bu2["data"]["htmlSnapshot"]
            .as_str()
            .unwrap()
            .contains("Title V1"),
        "Undo #2 must restore V1 HTML: {bu2}"
    );

    // 2. Concurrent /chat (Lane 0 + read-only Lane 1) + /live-patch on same screen -> 0 lost updates
    let mut tasks = Vec::new();
    for i in 0..4usize {
        let app_c = h.app.clone();
        let tok_c = h.editor_a_tok.clone();
        let pid_c = h.project_a.clone();
        tasks.push(tokio::spawn(async move {
            let line = serde_json::json!({
                "screenId": "concurrent-screen",
                "ffId": "stage-root",
                "opKind": "TOKEN_SWAP",
                "payload": { "cssVars": { format!("--lp-{i}"): "#2563EB" } }
            })
            .to_string();
            send_req(
                &app_c,
                Method::POST,
                "/api/v1/orchestrator/live-patch",
                Some(&tok_c),
                None,
                Some(serde_json::json!({
                    "projectId": pid_c,
                    "sessionId": format!("sess-lp-{i}"),
                    "screenId": "concurrent-screen",
                    "action": "apply",
                    "liveSpecPayload": line
                })),
            )
            .await
        }));
    }
    for i in 0..4usize {
        let app_c = h.app.clone();
        let tok_c = h.editor_a_tok.clone();
        let pid_c = h.project_a.clone();
        tasks.push(tokio::spawn(async move {
            let line = serde_json::json!({
                "screenId": "concurrent-screen",
                "ffId": "stage-root",
                "opKind": "TOKEN_SWAP",
                "payload": { "cssVars": { format!("--chat-{i}"): "#0F766E" } }
            })
            .to_string();
            send_req(
                &app_c,
                Method::POST,
                "/api/v1/orchestrator/chat",
                Some(&tok_c),
                None,
                Some(serde_json::json!({
                    "projectId": pid_c,
                    "sessionId": format!("sess-chat-{i}"),
                    "message": "apply live spec",
                    "commandKind": "apply_live_spec",
                    "liveSpecPayload": line
                })),
            )
            .await
        }));
    }
    for t in tasks {
        let (st, _, body) = t.await.expect("join");
        assert_eq!(st, StatusCode::OK, "concurrent body: {body}");
    }

    let conn = h.registry.pool().connection().expect("conn");
    let persisted_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM screen_artifact_patches WHERE project_id = ?1 AND screen_id = 'concurrent-screen'",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count concurrent rows");
    assert_eq!(
        persisted_count, 8,
        "All 4 /live-patch + 4 /chat Lane 0 mutations must persist without lost updates"
    );
}

#[tokio::test]
async fn it_10_1_08_write_actor_closure_panic_survival_and_adjacent_writer_routing() {
    let h = setup_harness();
    let actor = SqliteWriteActor::from_pool(h.registry.pool()).expect("spawn actor");
    assert!(actor.is_alive());

    let project_a_for_panic = h.project_a.clone();
    let panic_res: Result<(), SqliteActorError> = actor.execute_immediate(move |tx| {
        tx.execute(
            "INSERT INTO decision_audit_log (
                id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                stale_file_sha256, latency_ms, created_at
             ) VALUES ('must-rollback-id', 'tenant-a', ?1, 'DECISION_1', 'LOCAL_FTS5_BM25', 'ExactMatch', 'PANIC', NULL, 1, 0, NULL, 1, 1000)",
            rusqlite::params![project_a_for_panic],
        )?;
        panic!("intentional closure panic inside write actor");
    });
    assert_eq!(
        panic_res,
        Err(SqliteActorError::OperationFailed(
            "WRITE_ACTOR_PANICKED".to_string()
        ))
    );
    assert!(
        actor.is_alive(),
        "Write actor thread must survive closure panic"
    );
    assert_eq!(
        actor.queue_depth(),
        0,
        "queue_depth must return to 0 after panic"
    );

    // Subsequent write through the same actor succeeds and rolled-back row is absent.
    actor
        .execute_immediate({
            let pid = h.project_a.clone();
            move |tx| {
                tx.execute(
                    "INSERT INTO decision_audit_log (
                        id, tenant_id, project_id, cascade_stage, tier_used, confidence_kind,
                        verdict_label, rubric_swap_consistent, attempt_number, breaker_tripped,
                        stale_file_sha256, latency_ms, created_at
                     ) VALUES ('post-panic-id', 'tenant-a', ?1, 'DECISION_1', 'LOCAL_FTS5_BM25', 'ExactMatch', 'SURVIVED', NULL, 1, 0, NULL, 1, 1001)",
                    rusqlite::params![pid],
                )?;
                Ok(())
            }
        })
        .expect("post-panic write committed");

    let conn = h.registry.pool().connection().expect("conn");
    let rolled_back_cnt: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decision_audit_log WHERE id = 'must-rollback-id'",
            [],
            |r| r.get(0),
        )
        .expect("count rollback");
    assert_eq!(rolled_back_cnt, 0);
    let survived_cnt: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decision_audit_log WHERE id = 'post-panic-id'",
            [],
            |r| r.get(0),
        )
        .expect("count survived");
    assert_eq!(survived_cnt, 1);

    // Adjacent writers (/api/v1/decision/route & /api/v1/sdi/clarify) route through shared write actor cleanly.
    let (st_dec, _, body_dec) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "stage": "DECISION_1",
            "prompt": "Compare contrast ratios for hero CTA",
            "context": "screen=home",
            "untrustedArtifactSha256": hex_sha256(b"artifact-101")
        })),
    )
    .await;
    assert_eq!(st_dec, StatusCode::OK, "decision route: {body_dec}");

    let (st_sdi, _, body_sdi) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/sdi/clarify",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "projectId": h.project_a,
            "goalPrompt": "Build fintech executive summary layout",
            "locale": "en",
            "portalId": "SalesApp"
        })),
    )
    .await;
    assert_eq!(st_sdi, StatusCode::OK, "sdi clarify: {body_sdi}");
}
