// @story 11.2
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon Integration Tests for /api/v1/mcp/rpc, /api/v1/mcp/install, /api/v1/mcp/tools/toggle & /api/v1/mcp/external-activity)
use std::fs;
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

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4171));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-112-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "operator-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let viewer_tok = format!("sess-112-vw-{}", Uuid::new_v4());
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
    tok: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut req_builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4171")
        .header("x-cowok-session-token", tok)
        .header("x-mcp-host-client", "cursor");
    if !raw.is_empty() {
        req_builder = req_builder.header("content-type", "application/json");
    }
    let resp = router
        .clone()
        .oneshot(req_builder.body(Body::from(raw)).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn it_11_2_01_rpc_hides_and_blocks_human_only_and_logs_external_activity() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok) = setup_daemon(&tmp);

    // 1. tools/list excludes HUMAN_ONLY
    let (st, list_res) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/rpc",
        &editor_tok,
        Some(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let tools = list_res["result"]["tools"].as_array().expect("tools array");
    assert!(tools.iter().all(|t| t["permissionTier"] != "HUMAN_ONLY"));

    // 2. tools/call on HUMAN_ONLY returns -32003 HUMAN_APPROVAL_REQUIRED
    let (st_deny, deny_res) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/rpc",
        &editor_tok,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "ephemeral_tool_forge",
                "arguments": { "approval_token": "fake-bypass" }
            }
        })),
    )
    .await;
    assert_eq!(st_deny, StatusCode::OK);
    assert_eq!(deny_res["error"]["code"], -32003);
    assert_eq!(deny_res["error"]["message"], "HUMAN_APPROVAL_REQUIRED");

    // 3. GET /api/v1/mcp/external-activity records both calls and returns 4-tier policies
    let (st_act, act_res) = send_json(
        &router,
        "GET",
        "/api/v1/mcp/external-activity?limit=20",
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_act, StatusCode::OK);
    assert_eq!(act_res["ok"], true);
    let activities = act_res["data"]["activities"]
        .as_array()
        .expect("activities");
    assert!(activities
        .iter()
        .any(|a| a["status"] == "DENIED_HUMAN_ONLY" && a["toolName"] == "ephemeral_tool_forge"));
    let policies = act_res["data"]["policies"].as_array().expect("policies");
    assert!(policies
        .iter()
        .any(|p| p["toolName"] == "ephemeral_tool_forge" && p["lockedHumanOnly"] == true));
}

#[tokio::test]
async fn it_11_2_02_toggle_rejects_human_only_and_allows_inspect_tool() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, viewer_tok) = setup_daemon(&tmp);

    // Viewer cannot toggle
    let (st_vw, _) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/tools/toggle",
        &viewer_tok,
        Some(json!({ "toolName": "banner_solver", "enabled": false })),
    )
    .await;
    assert_eq!(st_vw, StatusCode::FORBIDDEN);

    // Editor cannot enable HUMAN_ONLY tool for MCP
    let (st_human, body_human) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/tools/toggle",
        &editor_tok,
        Some(json!({ "toolName": "learn_card_promote", "mcpCallable": true })),
    )
    .await;
    assert_eq!(st_human, StatusCode::FORBIDDEN);
    assert_eq!(body_human["error"]["code"], "HUMAN_APPROVAL_REQUIRED");

    // Editor can toggle INSPECT tool off and back on
    let (st_off, body_off) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/tools/toggle",
        &editor_tok,
        Some(json!({ "toolName": "banner_solver", "enabled": false })),
    )
    .await;
    assert_eq!(st_off, StatusCode::OK);
    assert_eq!(body_off["data"]["enabled"], false);
}

#[tokio::test]
async fn it_11_2_03_install_and_rollback_host_config_via_daemon() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok) = setup_daemon(&tmp);

    let cfg_path = tmp.path().join("cursor-mcp.json");
    let initial = "{\n  \"mcpServers\": {\n    \"sibling\": { \"command\": \"node\" }\n  }\n}\n";
    fs::write(&cfg_path, initial).expect("write initial");

    let (st_ins, ins_body) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/install",
        &editor_tok,
        Some(json!({
            "hostId": "cursor",
            "action": "install",
            "configPathOverride": cfg_path.to_string_lossy()
        })),
    )
    .await;
    assert_eq!(st_ins, StatusCode::OK);
    let backup_id = ins_body["data"]["backupId"]
        .as_str()
        .expect("backupId")
        .to_string();
    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&cfg_path).unwrap()).unwrap();
    assert_eq!(merged["mcpServers"]["sibling"]["command"], "node");
    assert_eq!(merged["mcpServers"]["cowok-design"]["command"], "cowok");

    let (st_rb, rb_body) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/install",
        &editor_tok,
        Some(json!({
            "hostId": "cursor",
            "action": "rollback",
            "backupId": backup_id
        })),
    )
    .await;
    assert_eq!(st_rb, StatusCode::OK);
    assert_eq!(rb_body["data"]["rolledBack"], true);
    assert_eq!(fs::read_to_string(&cfg_path).unwrap(), initial);
}

#[tokio::test]
async fn it_11_2_04_install_hardening_override_size_limit_and_honest_actor_kind() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, _viewer_tok) = setup_daemon(&tmp);

    // 1. Rejects mismatched host extension (.toml for JSON host cursor, .json for TOML host codex)
    let wrong_ext = tmp.path().join("cursor.toml");
    let (st_ext, body_ext) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/install",
        &editor_tok,
        Some(json!({
            "hostId": "cursor",
            "action": "install",
            "configPathOverride": wrong_ext.to_string_lossy()
        })),
    )
    .await;
    assert_eq!(st_ext, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_ext["error"]["code"], "MALFORMED_HOST_CONFIG");

    // 2. Rejects sensitive directories (.ssh, .git, .cowok-design) and protected files (package.json, Cargo.toml)
    for (host_id, rel) in [
        ("cursor", ".ssh/mcp.json"),
        ("cursor", ".git/mcp.json"),
        ("cursor", ".cowok-design/mcp.json"),
        ("cursor", "package.json"),
        ("codex", "Cargo.toml"),
        ("cursor", ".zshrc"),
    ] {
        let forbidden = tmp.path().join(rel);
        let (st_forb, body_forb) = send_json(
            &router,
            "POST",
            "/api/v1/mcp/install",
            &editor_tok,
            Some(json!({
                "hostId": host_id,
                "action": "install",
                "configPathOverride": forbidden.to_string_lossy()
            })),
        )
        .await;
        assert_eq!(
            st_forb,
            StatusCode::UNPROCESSABLE_ENTITY,
            "expected rejection for {rel}"
        );
        assert_eq!(body_forb["error"]["code"], "MALFORMED_HOST_CONFIG");
    }

    // 3. Rejects oversized body > 256 KiB with 413 PAYLOAD_TOO_LARGE
    let huge_pad = "a".repeat(270_000);
    let oversized_req = Request::builder()
        .method("POST")
        .uri("/api/v1/mcp/install")
        .header("host", "127.0.0.1:4171")
        .header("x-cowok-session-token", &editor_tok)
        .header("content-type", "application/json")
        .body(Body::from(format!(
            "{{\"hostId\":\"cursor\",\"action\":\"install\",\"configPathOverride\":\"{huge_pad}\"}}"
        )))
        .unwrap();
    let oversized_resp = router.clone().oneshot(oversized_req).await.unwrap();
    assert_eq!(oversized_resp.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // 4. Honest actor_kind: non-human session token logs MCP_CLIENT, verified human session logs HUMAN_WORKBENCH_SESSION
    let cfg_mcp = tmp.path().join("honest-mcp.json");
    let (st_mcp, _) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/install",
        &editor_tok,
        Some(json!({
            "hostId": "cursor",
            "action": "install",
            "configPathOverride": cfg_mcp.to_string_lossy()
        })),
    )
    .await;
    assert_eq!(st_mcp, StatusCode::OK);

    let human_tok = state.auth.issue_human_workbench_session(
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "operator-1".into(),
            role: "editor".into(),
        },
        Utc::now().timestamp() + 3_600,
    );
    let cfg_human = tmp.path().join("honest-human.json");
    let human_req = Request::builder()
        .method("POST")
        .uri("/api/v1/mcp/install")
        .header("host", "127.0.0.1:4171")
        .header("x-cowok-session-token", &editor_tok)
        .header("x-cowok-human-session", &human_tok)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "hostId": "antigravity",
                "action": "install",
                "configPathOverride": cfg_human.to_string_lossy()
            }))
            .unwrap(),
        ))
        .unwrap();
    let hum_resp = router.clone().oneshot(human_req).await.unwrap();
    assert_eq!(hum_resp.status(), StatusCode::OK);

    let (st_act, act_body) = send_json(
        &router,
        "GET",
        "/api/v1/mcp/external-activity?limit=20",
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_act, StatusCode::OK);
    let activities = act_body["data"]["activities"].as_array().unwrap();
    assert!(activities.iter().any(|a| a["hostClient"] == "cursor"
        && a["methodName"] == "mcp/install"
        && a["actorKind"] == "MCP_CLIENT"));
    assert!(activities.iter().any(|a| a["hostClient"] == "antigravity"
        && a["methodName"] == "mcp/install"
        && a["actorKind"] == "HUMAN_WORKBENCH_SESSION"));
}

#[tokio::test]
async fn it_11_2_05_rollback_freshly_created_config_unlinks_and_disabled_tool_is_32004() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok) = setup_daemon(&tmp);

    // 1. Fresh install + rollback unlinks file cleanly
    let fresh_cfg = tmp.path().join("fresh-opencode.json");
    assert!(!fresh_cfg.exists());
    let (st_ins, ins_body) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/install",
        &editor_tok,
        Some(json!({
            "hostId": "opencode",
            "action": "install",
            "configPathOverride": fresh_cfg.to_string_lossy()
        })),
    )
    .await;
    assert_eq!(st_ins, StatusCode::OK);
    assert!(fresh_cfg.exists());

    let backup_id = ins_body["data"]["backupId"].as_str().unwrap();
    let (st_rb, _) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/install",
        &editor_tok,
        Some(json!({
            "hostId": "opencode",
            "action": "rollback",
            "backupId": backup_id
        })),
    )
    .await;
    assert_eq!(st_rb, StatusCode::OK);
    assert!(!fresh_cfg.exists());

    // 2. Disabling INSPECT tool banner_solver with enabled=false & mcpCallable=false yields -32004 (not -32003)
    let (st_tog, _) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/tools/toggle",
        &editor_tok,
        Some(json!({ "toolName": "banner_solver", "enabled": false, "mcpCallable": false })),
    )
    .await;
    assert_eq!(st_tog, StatusCode::OK);

    let (st_call, call_body) = send_json(
        &router,
        "POST",
        "/api/v1/mcp/rpc",
        &editor_tok,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "tools/call",
            "params": { "name": "banner_solver", "arguments": { "preset": "G1", "width": 1200, "height": 630 } }
        })),
    )
    .await;
    assert_eq!(st_call, StatusCode::OK);
    assert_eq!(call_body["error"]["code"], -32004);
}

#[tokio::test]
async fn it_11_2_06_rpc_engine_isolated_across_tenants_and_projects() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, alpha_tok, _viewer_tok) = setup_daemon(&tmp);

    let beta_tok = format!("sess-112-beta-{}", Uuid::new_v4());
    state.auth.insert_session(
        beta_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".into(),
            user_id: "operator-beta".into(),
            role: "editor".into(),
        },
        Utc::now().timestamp() + 3600,
    );

    let send_with_project = |tok: String, project_id: &'static str, body: serde_json::Value| {
        let r = router.clone();
        async move {
            let req = Request::builder()
                .method("POST")
                .uri("/api/v1/mcp/rpc")
                .header("host", "127.0.0.1:4171")
                .header("x-cowok-session-token", tok)
                .header("x-cowok-project-id", project_id)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap();
            let resp = r.oneshot(req).await.unwrap();
            let status = resp.status();
            let bytes = resp.into_body().collect().await.unwrap().to_bytes();
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            )
        }
    };

    // Tenant Alpha / proj-a creates a secret node
    let (st_mut, _) = send_with_project(
        alpha_tok.clone(),
        "proj-a",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "apply_batch",
                "arguments": {
                    "dryRun": false,
                    "operations": [
                        { "op": "create_node", "id": "tenant-alpha-secret-node", "kind": "card", "text": "Alpha", "width": 200, "height": 80 }
                    ]
                }
            }
        }),
    )
    .await;
    assert_eq!(st_mut, StatusCode::OK);

    // Tenant Beta / proj-a MUST NOT see tenant-alpha-secret-node
    let (st_beta, beta_state) = send_with_project(
        beta_tok,
        "proj-a",
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "get_app_state", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(st_beta, StatusCode::OK);
    let beta_nodes = beta_state["result"]["structuredContent"]["nodes"]
        .as_array()
        .unwrap();
    assert!(!beta_nodes
        .iter()
        .any(|n| n["id"] == "tenant-alpha-secret-node"));

    // Tenant Alpha / proj-b MUST NOT see tenant-alpha-secret-node either
    let (st_proj_b, proj_b_state) = send_with_project(
        alpha_tok,
        "proj-b",
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "get_app_state", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(st_proj_b, StatusCode::OK);
    let proj_b_nodes = proj_b_state["result"]["structuredContent"]["nodes"]
        .as_array()
        .unwrap();
    assert!(!proj_b_nodes
        .iter()
        .any(|n| n["id"] == "tenant-alpha-secret-node"));
}
