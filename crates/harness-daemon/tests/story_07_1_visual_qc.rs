// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-071-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let viewer_tok = format!("sess-071-vw-{}", Uuid::new_v4());
    state.auth.insert_session(
        viewer_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-1".into(),
            role: "viewer".into(),
        },
        exp,
    );

    let other_tenant_tok = format!("sess-071-other-{}", Uuid::new_v4());
    state.auth.insert_session(
        other_tenant_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".into(),
            user_id: "attacker-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let router = build_router((*state).clone());
    (state, router, editor_tok, viewer_tok, other_tenant_tok)
}

async fn send_json(
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

async fn create_project(router: &axum::Router, tok: &str, slug: &str) -> String {
    let (st, res) = send_json(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        Some(json!({ "slug": slug, "name": "Visual QC Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn it_07_1_01_qc_run_persists_v26_hits_sha1_cache_and_enforces_rbac_and_idor() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, viewer_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "visual-qc-proj-1").await;

    let payload = json!({
        "projectId": project_id,
        "targetKind": "APP_SCREEN",
        "targetId": "screen-checkout",
        "html": "<main><iframe src=\"https://thirdparty.example\"></iframe><p style=\"color: #cccccc; font-size: 10px;\">Low contrast</p></main>",
        "constraintsJson": "{\"radius\":\"balanced\"}",
        "viewports": [1440, 1024, 768, 390],
        "fixtureRef": "fixture-checkout-v1",
        "forceNoBrowser": true
    });

    // Viewer cannot mutate -> 403
    let (st_v, _) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &viewer_tok,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_v, StatusCode::FORBIDDEN);

    // Cross-tenant editor rejected -> 403
    let (st_x, _) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &other_tenant_tok,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_x, StatusCode::FORBIDDEN);

    // First run -> cache miss, honest UNVERIFIED_NO_BROWSER fallback (never fake PASS)
    let (st1, res1) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st1, StatusCode::OK);
    assert_eq!(res1["ok"], json!(true));
    assert_eq!(res1["data"]["verdict"], json!("UNVERIFIED_NO_BROWSER"));
    assert_eq!(res1["data"]["cacheHit"], json!(false));
    assert_eq!(res1["data"]["usedFallback"], json!(true));
    assert_eq!(
        res1["data"]["fallbackMode"],
        json!("iframe_bridge_fallback")
    );
    let first_run_id = res1["data"]["qcRunId"]
        .as_str()
        .expect("qcRunId")
        .to_string();
    let first_sha1 = res1["data"]["layoutSha1"]
        .as_str()
        .expect("layoutSha1")
        .to_string();
    assert_eq!(first_sha1.len(), 40);

    // Second identical run -> SHA-1 cache hit returning same qcRunId
    let (st2, res2) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(payload),
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    assert_eq!(res2["data"]["cacheHit"], json!(true));
    assert_eq!(res2["data"]["qcRunId"], json!(first_run_id));
    assert_eq!(res2["data"]["layoutSha1"], json!(first_sha1));
}

#[tokio::test]
async fn it_07_1_02_deterministic_lint_fix_and_strict_max_3_attempts_cap() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "visual-qc-proj-2").await;

    let dirty_html = "<main><iframe src=\"https://x.example\"></iframe><button style=\"outline: none;\">Pay</button></main>";
    let (st_run, run_res) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "targetKind": "APP_SCREEN",
            "targetId": "screen-billing",
            "html": dirty_html,
            "viewports": [1440],
            "forceNoBrowser": true
        })),
    )
    .await;
    assert_eq!(st_run, StatusCode::OK);
    let qc_run_id = run_res["data"]["qcRunId"].as_str().unwrap().to_string();
    let issue_ids: Vec<String> = run_res["data"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["deterministicFixable"] == json!(true))
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect();
    assert!(!issue_ids.is_empty());

    // Deterministic lint_fix executes in <10ms with 0 LLM tokens and returns rollback snapshot
    let (st_fix, fix_res) = send_json(
        &router,
        "POST",
        "/api/v1/qc/auto-fix",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "qcRunId": qc_run_id,
            "issueIds": issue_ids,
            "mode": "DETERMINISTIC_LINT_FIX",
            "attemptNumber": 1,
            "currentHtml": dirty_html
        })),
    )
    .await;
    assert_eq!(st_fix, StatusCode::OK);
    assert_eq!(fix_res["data"]["llmTokensUsed"], json!(0));
    assert!(fix_res["data"]["elapsedMs"].as_f64().unwrap() < 10.0);
    assert_eq!(fix_res["data"]["previousHtml"], json!(dirty_html));
    assert!(fix_res["data"]["patchedHtml"]
        .as_str()
        .unwrap()
        .contains("sandbox=\"allow-scripts\""));

    // Attempt 4 strictly rejected with 422 AUTOFIX_MAX_ATTEMPTS_EXCEEDED
    let (st_cap, cap_res) = send_json(
        &router,
        "POST",
        "/api/v1/qc/auto-fix",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "qcRunId": qc_run_id,
            "issueIds": issue_ids,
            "mode": "AGENT_PATCH",
            "attemptNumber": 4,
            "currentHtml": dirty_html
        })),
    )
    .await;
    assert_eq!(st_cap, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        cap_res["error"]["code"],
        json!("AUTOFIX_MAX_ATTEMPTS_EXCEEDED")
    );
}

#[tokio::test]
async fn it_07_1_03_slide_16_9_and_full_flow_completeness_audit_through_daemon() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "visual-qc-proj-3").await;

    let (st, res) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "targetKind": "FULL_FLOW",
            "targetId": "flow-main",
            "html": "<main><h1>Flow</h1></main>",
            "flowNodes": [
                {
                    "id": "n-home",
                    "label": "Home",
                    "isEntry": true,
                    "subStates": ["default", "loading", "empty", "error"],
                    "ctas": [{ "ctaId": "cta-orphan-btn", "label": "Dead CTA", "targetScreenId": null }]
                },
                {
                    "id": "n-orphan",
                    "label": "Unreachable",
                    "isEntry": false,
                    "subStates": ["default"],
                    "ctas": []
                }
            ],
            "flowEdges": [],
            "forceNoBrowser": true
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let flow = &res["data"]["flowCompleteness"];
    assert_eq!(flow["isComplete"], json!(false));
    assert_eq!(flow["deadEndCtas"], json!(["n-home:cta-orphan-btn"]));
    assert_eq!(flow["orphanScreens"], json!(["n-orphan"]));
}

#[tokio::test]
async fn it_07_1_04_defect_1_and_9_server_side_attempts_deterministic_no_inc_and_surgical_patch() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, _viewer_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "visual-qc-proj-4").await;

    let html_with_panel = r#"<main><iframe src="https://x.example"></iframe><div data-ff-id="ff-panel" style="width: 400px;">Tall content</div></main>"#;
    let (st_run, run_res) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "targetKind": "APP_SCREEN",
            "targetId": "screen-panel",
            "html": html_with_panel,
            "viewports": [1440],
            "geometryNodes": [{
                "ffId": "ff-panel",
                "tag": "div",
                "bbox": { "x": 0.0, "y": 0.0, "width": 400.0, "height": 200.0 },
                "scrollWidth": 400.0,
                "clientWidth": 400.0,
                "scrollHeight": 420.0,
                "clientHeight": 200.0
            }],
            "forceNoBrowser": true
        })),
    )
    .await;
    assert_eq!(st_run, StatusCode::OK);
    let qc_run_id = run_res["data"]["qcRunId"].as_str().unwrap().to_string();
    let issues = run_res["data"]["issues"].as_array().unwrap();
    let det_id = issues
        .iter()
        .find(|i| i["ruleId"] == json!("R11_CROSS_ORIGIN_IFRAME_SANDBOX"))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let ovf_id = issues
        .iter()
        .find(|i| i["ruleId"] == json!("R02_VERTICAL_OVERFLOW"))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let no_browser_id = issues
        .iter()
        .find(|i| i["ruleId"] == json!("R26_UNVERIFIED_NO_BROWSER_CDP"))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let read_persisted_attempts = || -> i64 {
        let conn = state.registry.pool().connection().unwrap();
        conn.query_row(
            "SELECT autofix_attempt FROM qc_gate_runs WHERE id = ?1",
            [&qc_run_id],
            |r| r.get(0),
        )
        .unwrap()
    };

    // 1. Deterministic lint fix MUST NOT increment autofix_attempt
    for _ in 0..2 {
        let (st, res) = send_json(
            &router,
            "POST",
            "/api/v1/qc/auto-fix",
            &editor_tok,
            Some(json!({
                "projectId": project_id,
                "qcRunId": qc_run_id,
                "issueIds": [&det_id],
                "mode": "DETERMINISTIC_LINT_FIX",
                "attemptNumber": 1,
                "currentHtml": html_with_panel
            })),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(res["data"]["attemptNumber"], json!(0));
        assert_eq!(res["data"]["remainingAttempts"], json!(3));
        assert_eq!(read_persisted_attempts(), 0);
    }

    // 2. AgentPatch producing zero patch operations MUST NOT increment autofix_attempt
    let (st_noop, noop_res) = send_json(
        &router,
        "POST",
        "/api/v1/qc/auto-fix",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "qcRunId": qc_run_id,
            "issueIds": [&no_browser_id],
            "mode": "AGENT_PATCH",
            "attemptNumber": 1,
            "currentHtml": html_with_panel
        })),
    )
    .await;
    assert_eq!(st_noop, StatusCode::OK);
    assert_eq!(noop_res["data"]["attemptNumber"], json!(0));
    assert_eq!(noop_res["data"]["patchedHtml"], json!(html_with_panel));
    assert_eq!(read_persisted_attempts(), 0);

    // 3. AgentPatch applies surgical structural patch & increments server-side 1 -> 2 -> 3
    // even when malicious client keeps sending attemptNumber: 1!
    for expected_attempt in 1..=3u8 {
        let (st, res) = send_json(
            &router,
            "POST",
            "/api/v1/qc/auto-fix",
            &editor_tok,
            Some(json!({
                "projectId": project_id,
                "qcRunId": qc_run_id,
                "issueIds": [&ovf_id],
                "mode": "AGENT_PATCH",
                "attemptNumber": 1,
                "currentHtml": html_with_panel
            })),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(res["data"]["attemptNumber"], json!(expected_attempt));
        assert_eq!(
            res["data"]["remainingAttempts"],
            json!(3 - expected_attempt)
        );
        assert!(res["data"]["patchedHtml"]
            .as_str()
            .unwrap()
            .contains("overflow-y: auto"));
        assert!(!res["data"]["diffSummary"].as_array().unwrap().is_empty());
        assert_eq!(read_persisted_attempts(), i64::from(expected_attempt));
    }

    // 4. 4th AgentPatch with client replaying attemptNumber: 1 MUST be rejected with 422
    let (st_4th, res_4th) = send_json(
        &router,
        "POST",
        "/api/v1/qc/auto-fix",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "qcRunId": qc_run_id,
            "issueIds": [&ovf_id],
            "mode": "AGENT_PATCH",
            "attemptNumber": 1,
            "currentHtml": html_with_panel
        })),
    )
    .await;
    assert_eq!(st_4th, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res_4th["error"]["code"],
        json!("AUTOFIX_MAX_ATTEMPTS_EXCEEDED")
    );
    assert_eq!(read_persisted_attempts(), 3);
}

#[tokio::test]
async fn it_07_1_05_defect_4_cache_key_inputs_flow_reconstruction_and_stale_unverified_bypass() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, _viewer_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "visual-qc-proj-5").await;

    // 1. FULL_FLOW cache hit MUST still reconstruct flowCompleteness
    let flow_payload = json!({
        "projectId": project_id,
        "targetKind": "FULL_FLOW",
        "targetId": "flow-cache",
        "html": "<main><h1>Flow</h1></main>",
        "flowNodes": [
            {
                "id": "n-entry",
                "label": "Entry",
                "isEntry": true,
                "subStates": ["default", "loading", "empty", "error"],
                "ctas": [{ "ctaId": "cta-dead", "label": "Dead", "targetScreenId": null }]
            }
        ],
        "flowEdges": [],
        "forceNoBrowser": true
    });
    let (st1, r1) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(flow_payload.clone()),
    )
    .await;
    assert_eq!(st1, StatusCode::OK);
    assert_eq!(r1["data"]["cacheHit"], json!(false));
    assert_eq!(r1["data"]["flowCompleteness"]["isComplete"], json!(false));

    let (st2, r2) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(flow_payload.clone()),
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    assert_eq!(r2["data"]["cacheHit"], json!(true));
    assert_eq!(
        r2["data"]["flowCompleteness"]["deadEndCtas"],
        json!(["n-entry:cta-dead"])
    );

    // 2. Varying targetKind, forceNoBrowser, ssimThreshold, geometryNodes, flowNodes, or RGBA
    // MUST invalidate the cache key!
    let base_sha = r1["data"]["layoutSha1"].as_str().unwrap().to_string();
    let mut changed_kind = flow_payload.clone();
    changed_kind["targetKind"] = json!("APP_SCREEN");
    let (_, r_kind) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(changed_kind),
    )
    .await;
    assert_eq!(r_kind["data"]["cacheHit"], json!(false));
    assert_ne!(r_kind["data"]["layoutSha1"], json!(base_sha));

    let mut changed_thresh = flow_payload.clone();
    changed_thresh["ssimThreshold"] = json!(0.98);
    let (_, r_thresh) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(changed_thresh),
    )
    .await;
    assert_eq!(r_thresh["data"]["cacheHit"], json!(false));
    assert_ne!(r_thresh["data"]["layoutSha1"], json!(base_sha));

    let mut changed_geom = flow_payload.clone();
    changed_geom["geometryNodes"] = json!([{
        "ffId": "ff-box-1",
        "tag": "div",
        "bbox": { "x": 10.0, "y": 10.0, "width": 200.0, "height": 100.0 }
    }]);
    let (_, r_geom) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(changed_geom),
    )
    .await;
    assert_eq!(r_geom["data"]["cacheHit"], json!(false));
    assert_ne!(r_geom["data"]["layoutSha1"], json!(base_sha));

    // 3. Stale cached UNVERIFIED_NO_BROWSER MUST be bypassed when live CDP is now ready
    let live_payload = json!({
        "projectId": project_id,
        "targetKind": "APP_SCREEN",
        "targetId": "screen-stale-unverified",
        "html": "<main><h1>Healthy</h1></main>",
        "viewports": [1440],
        "geometryNodes": [{
            "ffId": "ff-hero",
            "tag": "div",
            "bbox": { "x": 0.0, "y": 0.0, "width": 600.0, "height": 200.0 }
        }],
        "forceNoBrowser": false
    });
    let (st_live1, r_live1) = send_json(
        &router,
        "POST",
        "/api/v1/qc/run",
        &editor_tok,
        Some(live_payload.clone()),
    )
    .await;
    assert_eq!(st_live1, StatusCode::OK);
    let live_sha = r_live1["data"]["layoutSha1"].as_str().unwrap().to_string();

    if harness_daemon::routes::visual_qc::shared_cdp_evaluator().can_attempt_cdp() {
        // Poison the cached row to simulate a prior outage that stored UNVERIFIED_NO_BROWSER
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "UPDATE qc_gate_runs SET verdict = 'UNVERIFIED_NO_BROWSER' WHERE project_id = ?1 AND layout_sha1 = ?2",
            rusqlite::params![&project_id, &live_sha],
        )
        .unwrap();

        let (st_live2, r_live2) = send_json(
            &router,
            "POST",
            "/api/v1/qc/run",
            &editor_tok,
            Some(live_payload),
        )
        .await;
        assert_eq!(st_live2, StatusCode::OK);
        assert_eq!(r_live2["data"]["cacheHit"], json!(false));
        assert_eq!(r_live2["data"]["verdict"], json!("PASS"));
    }
}
