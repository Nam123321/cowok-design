// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (Full build_router Integration Tests for /api/v1/flows/synthesize & /api/v1/flows/substates)
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use chrono::Utc;
use flow_graph::synthesizer::count_json_floats;
use harness_daemon::daemon_auth::{DaemonAuthState, FlowPrincipal};
use harness_daemon::routes::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct FlowSynthHarness {
    _dir: tempfile::TempDir,
    app: Router,
    registry: Arc<SpecRegistry>,
    project_a: String,
    editor_a_tok: String,
    viewer_a_tok: String,
    editor_b_tok: String,
}

fn setup_router() -> FlowSynthHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-055.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let project_a = "55555555-5555-4555-8555-555555555555".to_string();
    {
        let conn = registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES (?1, 'tenant-a', 'proj-055', 'Flow Synth Proj', ?2, '', 'ACTIVE', 1000, 1000)",
            rusqlite::params![project_a, dir.path().to_string_lossy().as_ref()],
        )
        .expect("seed project");
    }

    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_a_tok = "cwk_sess_055_editor_a".to_string();
    auth.insert_session(
        editor_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_a_tok = "cwk_sess_055_viewer_a".to_string();
    auth.insert_session(
        viewer_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "viewer-a".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let editor_b_tok = "cwk_sess_055_editor_b".to_string();
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
        DaemonAppState::with_auth(Arc::clone(&registry), "127.0.0.1".to_string(), 4175, auth);
    let app = build_router(state);

    FlowSynthHarness {
        _dir: dir,
        app,
        registry,
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
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4175");
    if let Some(tok) = session_token {
        builder = builder.header("x-cowok-session-token", tok);
    }
    if let Some(b) = bearer_token {
        builder = builder.header("authorization", format!("Bearer {b}"));
    }
    let req = if let Some(json_body) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(json_body.to_string()))
            .expect("req")
    } else {
        builder.body(Body::empty()).expect("req")
    };

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

#[tokio::test]
async fn it_05_5_01_twelve_screen_synthesis_clustering_substates_and_auth_isolation() {
    let h = setup_router();
    let screens: Vec<Value> = (1..=12)
        .map(|idx| {
            json!({
                "screenKey": format!("app-screen-{idx:02}"),
                "title": format!("App Screen {idx}"),
                "routePath": format!("/screen-{idx}"),
                "fmeaFailureMode": if idx == 9 { "Payment timeout" } else { "Network stall" }
            })
        })
        .collect();

    let payload = json!({
        "projectId": h.project_a,
        "briefSummary": "12-screen commerce workbench with checkout and offline recovery",
        "maxDefaultClusterNodes": 8,
        "includeFmeaBranches": true,
        "screens": screens
    });

    // 1. Auth & RBAC isolation checks
    let (st_unauth, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        None,
        None,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    let (st_bearer, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        None,
        Some("legacy-project-token"),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_bearer, StatusCode::UNAUTHORIZED);

    let (st_viewer, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.viewer_a_tok),
        None,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_viewer, StatusCode::FORBIDDEN);

    let (st_cross, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_b_tok),
        None,
        Some(payload.clone()),
    )
    .await;
    assert!(matches!(
        st_cross,
        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
    ));

    // 2. Authorized synthesis persists <= 8 PRIMARY cluster nodes + 100% 4-SubState bindings
    let (st_ok, body) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(payload),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "body={body}");
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["data"]["summary"]["primaryClusterCount"], json!(8));
    assert!(
        body["data"]["summary"]["edgeCaseBranchCount"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );
    assert_eq!(
        body["data"]["summary"]["substateCoveragePercent"],
        json!(100)
    );
    assert_eq!(body["data"]["summary"]["floatLeakCount"], json!(0));
    assert_eq!(
        count_json_floats(&body),
        0,
        "VerdictShield forbids any raw float in JSON"
    );

    // 3. Verify SQLite V22 persistence via GET /api/v1/flows/substates and direct table count
    let list_uri = format!("/api/v1/flows/substates?projectId={}", h.project_a);
    let (st_list, list_body) = send_req(
        &h.app,
        Method::GET,
        &list_uri,
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_list, StatusCode::OK, "list_body={list_body}");
    let total_substates = list_body["data"]["totalCount"].as_u64().unwrap_or(0);
    let total_nodes = body["data"]["nodes"]
        .as_array()
        .map(|a| a.len() as u64)
        .unwrap_or(0);
    assert_eq!(total_substates, total_nodes * 4);

    let conn = h.registry.pool().connection().expect("conn");
    let persisted_substates: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_node_substates WHERE project_id = ?1",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count substates");
    assert_eq!(persisted_substates as u64, total_nodes * 4);

    // 4. Verify real DecisionCascadeClient audit row persisted in V21 decision_audit_log
    let audit_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decision_audit_log WHERE tenant_id = 'tenant-a' AND project_id = ?1 AND cascade_stage = 'DECISION_1'",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count decision_audit_log");
    assert_eq!(audit_rows, 1);
}

#[tokio::test]
async fn it_05_5_02_dfs_cycle_orphan_guard_preserves_loopbacks_and_writes_decision_audit() {
    let h = setup_router();
    let payload = json!({
        "projectId": h.project_a,
        "briefSummary": "Unrelated opaque prompt xyz123",
        "maxDefaultClusterNodes": 4,
        "includeFmeaBranches": false,
        "screens": [
            { "screenKey": "alpha", "title": "Alpha View", "routePath": "/alpha" },
            { "screenKey": "beta", "title": "Beta View", "routePath": "/beta" },
            { "screenKey": "gamma", "title": "Gamma View", "routePath": "/gamma" }
        ],
        "candidateEdges": [
            {
                "sourceNodeId": "alpha",
                "targetNodeId": "non-existent-orphan",
                "flowType": "main",
                "triggerEvent": "onOrphanJump"
            },
            {
                "sourceNodeId": "gamma",
                "targetNodeId": "alpha",
                "flowType": "main",
                "triggerEvent": "onUnguardedAutoCycle"
            },
            {
                "sourceNodeId": "gamma",
                "targetNodeId": "alpha",
                "flowType": "loopback",
                "triggerEvent": "onEditCartLoopback",
                "guardCondition": "user_clicked_edit_cart"
            }
        ]
    });

    let (st_ok, body) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(payload),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "body={body}");
    assert_eq!(
        body["data"]["summary"]["decisionKind"],
        json!("DeterministicFallback")
    );
    assert_eq!(body["data"]["summary"]["strippedOrphanEdgeCount"], json!(1));
    assert_eq!(body["data"]["summary"]["rejectedCycleEdgeCount"], json!(1));
    assert_eq!(
        body["data"]["summary"]["preservedLoopbackEdgeCount"],
        json!(1)
    );
    assert_eq!(count_json_floats(&body), 0);

    let edges = body["data"]["edges"].as_array().expect("edges array");
    assert!(edges
        .iter()
        .any(|e| e["flowType"] == json!("loopback")
            && e["triggerEvent"] == json!("onEditCartLoopback")));
    assert!(!edges
        .iter()
        .any(|e| e["triggerEvent"] == json!("onUnguardedAutoCycle")));
    assert!(!edges
        .iter()
        .any(|e| e["triggerEvent"] == json!("onOrphanJump")));

    let conn = h.registry.pool().connection().expect("conn");
    let audit_kind: String = conn
        .query_row(
            "SELECT confidence_kind FROM decision_audit_log WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 1",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("audit kind");
    assert_eq!(audit_kind, "DeterministicFallback");
}

#[tokio::test]
async fn it_05_5_03_second_synthesis_run_is_collision_free_and_reclustering_prunes_stale_nodes() {
    let h = setup_router();
    let screens: Vec<Value> = (1..=12)
        .map(|idx| {
            json!({
                "screenKey": format!("app-screen-{idx:02}"),
                "title": format!("App Screen {idx}"),
                "routePath": format!("/screen-{idx}")
            })
        })
        .collect();

    let payload_8 = json!({
        "projectId": h.project_a,
        "briefSummary": "12-screen commerce flow with checkout and recovery",
        "maxDefaultClusterNodes": 8,
        "includeFmeaBranches": true,
        "screens": screens.clone()
    });

    // Run 1: 8 primary clusters
    let (st_1, body_1) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(payload_8.clone()),
    )
    .await;
    assert_eq!(st_1, StatusCode::OK, "run1 body={body_1}");
    assert_eq!(body_1["data"]["summary"]["primaryClusterCount"], json!(8));

    // Run 2 (identical 2nd run): MUST return 200 OK with zero 409 PK/UNIQUE collisions!
    let (st_2, body_2) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(payload_8),
    )
    .await;
    assert_eq!(st_2, StatusCode::OK, "run2 body={body_2}");
    assert_eq!(body_2["data"]["summary"]["primaryClusterCount"], json!(8));

    // Run 3 (re-cluster 8 -> 4): MUST prune stale previously-synthesized nodes & substates!
    let payload_4 = json!({
        "projectId": h.project_a,
        "briefSummary": "Re-clustered 4-cluster commerce flow",
        "maxDefaultClusterNodes": 4,
        "includeFmeaBranches": true,
        "screens": screens
    });
    let (st_3, body_3) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(payload_4),
    )
    .await;
    assert_eq!(st_3, StatusCode::OK, "run3 body={body_3}");
    assert_eq!(body_3["data"]["summary"]["primaryClusterCount"], json!(4));
    let total_nodes_3 = body_3["data"]["nodes"].as_array().expect("nodes").len() as i64;

    let conn = h.registry.pool().connection().expect("conn");
    let persisted_nodes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_nodes WHERE project_id = ?1",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count flow_nodes");
    assert_eq!(
        persisted_nodes, total_nodes_3,
        "Stale nodes from 8-cluster run must be pruned"
    );

    let persisted_primary_substates: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM flow_node_substates WHERE project_id = ?1 AND flow_tier = 'PRIMARY'",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count primary substates");
    assert_eq!(persisted_primary_substates, 4 * 4);

    let audit_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decision_audit_log WHERE project_id = ?1",
            rusqlite::params![h.project_a],
            |r| r.get(0),
        )
        .expect("count audit rows");
    assert_eq!(audit_count, 3);
}

#[tokio::test]
async fn it_05_5_04_substates_filter_edge_case_invalid_tier_and_rejects_spoofed_or_dos_payloads() {
    let h = setup_router();
    let seed_payload = json!({
        "projectId": h.project_a,
        "briefSummary": "FMEA edge-case filter verification checkout flow",
        "maxDefaultClusterNodes": 4,
        "includeFmeaBranches": true
    });
    let (st_seed, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(seed_payload),
    )
    .await;
    assert_eq!(st_seed, StatusCode::OK);

    // 1. Filter GET /api/v1/flows/substates?flowTier=EDGE_CASE
    let edge_uri = format!(
        "/api/v1/flows/substates?projectId={}&flowTier=EDGE_CASE",
        h.project_a
    );
    let (st_edge, edge_body) = send_req(
        &h.app,
        Method::GET,
        &edge_uri,
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_edge, StatusCode::OK, "edge_body={edge_body}");
    let items = edge_body["data"]["items"].as_array().expect("items");
    assert!(!items.is_empty());
    assert!(items.iter().all(|it| it["flowTier"] == json!("EDGE_CASE")));

    // 2. Invalid flowTier returns 422 UNPROCESSABLE_ENTITY
    let bad_tier_uri = format!(
        "/api/v1/flows/substates?projectId={}&flowTier=BOGUS",
        h.project_a
    );
    let (st_bad_tier, bad_tier_body) = send_req(
        &h.app,
        Method::GET,
        &bad_tier_uri,
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(
        st_bad_tier,
        StatusCode::UNPROCESSABLE_ENTITY,
        "bad_tier_body={bad_tier_body}"
    );

    // 3. Spoofed simulateDecisionTimeout field is rejected with 422
    let spoofed_payload = json!({
        "projectId": h.project_a,
        "briefSummary": "Spoof attempt",
        "simulateDecisionTimeout": true
    });
    let (st_spoof, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(spoofed_payload),
    )
    .await;
    assert_eq!(st_spoof, StatusCode::UNPROCESSABLE_ENTITY);

    // 4. DoS bound: >64 screens rejected with 422
    let screens_65: Vec<Value> = (1..=65)
        .map(|i| json!({ "screenKey": format!("s-{i}"), "title": format!("S {i}") }))
        .collect();
    let dos_payload = json!({
        "projectId": h.project_a,
        "briefSummary": "DoS screen bound check",
        "screens": screens_65
    });
    let (st_dos, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/flows/synthesize",
        Some(&h.editor_a_tok),
        None,
        Some(dos_payload),
    )
    .await;
    assert_eq!(st_dos, StatusCode::UNPROCESSABLE_ENTITY);
}
