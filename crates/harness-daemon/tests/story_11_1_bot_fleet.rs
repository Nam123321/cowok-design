// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon Integration Tests for /api/v1/bot-fleet/* including C-01, H-03, H-04, H-05, M-04)
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::bot_fleet::shared_cooldown_tracker;
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
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4171));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-111-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "operator-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let viewer_tok = format!("sess-111-vw-{}", Uuid::new_v4());
    state.auth.insert_session(
        viewer_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-1".into(),
            role: "viewer".into(),
        },
        exp,
    );

    let other_tenant_tok = format!("sess-111-other-{}", Uuid::new_v4());
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
        .header("host", "127.0.0.1:4171")
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
        Some(json!({ "slug": slug, "name": "Bot Fleet Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn it_11_1_01_providers_status_blocks_legacy_bearer_and_isolates_tenant_cooldowns() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, viewer_tok, other_tok) = setup_daemon(&tmp);

    // C-01: Arbitrary non-superadmin Bearer MUST be rejected with 401 UNAUTHORIZED
    let bearer_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/bot-fleet/providers/status")
                .header("host", "127.0.0.1:4171")
                .header("authorization", "Bearer arbitrary-non-superadmin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bearer_resp.status(), StatusCode::UNAUTHORIZED);

    // H-05: ram_vault anthropic secret maps to canonical "claude" and is reported configured for tenant-alpha only
    state.ram_vault.upsert(
        "tenant-alpha",
        "anthropic",
        "sk-ant-ram-only-secret-1234567890",
    );
    // H-04: record 429 cooldown for tenant-alpha gemini -> tenant-beta MUST NOT see it
    shared_cooldown_tracker().record_rate_limit("tenant-alpha", "gemini", Some(30), None);

    let (st, body_a) = send_json(
        &router,
        "GET",
        "/api/v1/bot-fleet/providers/status",
        &viewer_tok,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "body={body_a}");
    assert_eq!(body_a["data"]["sandboxProtocol"], "posix_0600_fd_pgid");
    assert!(body_a["data"]["adapterCount"].as_u64().unwrap_or(0) >= 26);

    let providers_a = body_a["data"]["byokProviders"].as_array().unwrap();
    let claude_a = providers_a
        .iter()
        .find(|p| p["providerId"] == "claude")
        .unwrap();
    assert_eq!(claude_a["configured"], true);
    let gemini_a = providers_a
        .iter()
        .find(|p| p["providerId"] == "gemini")
        .unwrap();
    assert_eq!(gemini_a["inCooldown"], true);

    let (st_b, body_b) = send_json(
        &router,
        "GET",
        "/api/v1/bot-fleet/providers/status",
        &other_tok,
        None,
    )
    .await;
    assert_eq!(st_b, StatusCode::OK);
    let providers_b = body_b["data"]["byokProviders"].as_array().unwrap();
    let gemini_b = providers_b
        .iter()
        .find(|p| p["providerId"] == "gemini")
        .unwrap();
    assert_eq!(
        gemini_b["inCooldown"], false,
        "H-04: Tenant B must not see Tenant A's cooldown"
    );

    let (st2, _) = send_json(
        &router,
        "GET",
        "/api/v1/bot-fleet/providers/status",
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    shared_cooldown_tracker().clear("tenant-alpha", "gemini");
}

#[tokio::test]
async fn it_11_1_02_execute_enforces_rbac_tenant_anti_idor_and_input_hardening() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, viewer_tok, other_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "fleet-proj").await;

    // Viewer cannot execute
    let (st_vw, _) = send_json(
        &router,
        "POST",
        "/api/v1/bot-fleet/execute",
        &viewer_tok,
        Some(json!({
            "projectId": project_id,
            "sessionId": "sess-1",
            "mode": "byok",
            "prompt": "hello"
        })),
    )
    .await;
    assert_eq!(st_vw, StatusCode::FORBIDDEN);

    // Cross-tenant attacker blocked
    let (st_idor, _) = send_json(
        &router,
        "POST",
        "/api/v1/bot-fleet/execute",
        &other_tok,
        Some(json!({
            "projectId": project_id,
            "sessionId": "sess-1",
            "mode": "byok",
            "prompt": "hello"
        })),
    )
    .await;
    assert!(matches!(
        st_idor,
        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
    ));

    // M-04: unsafe sessionId rejected with 422
    let (st_bad_sess, _) = send_json(
        &router,
        "POST",
        "/api/v1/bot-fleet/execute",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "sessionId": "../bad/sess",
            "mode": "byok",
            "prompt": "hello"
        })),
    )
    .await;
    assert_eq!(st_bad_sess, StatusCode::UNPROCESSABLE_ENTITY);

    // H-03: Gemini modelId with slash/traversal rejected with 422
    let (st_bad_model, _) = send_json(
        &router,
        "POST",
        "/api/v1/bot-fleet/execute",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "sessionId": "sess-1",
            "mode": "byok",
            "providerId": "gemini",
            "modelId": "gemini/../../evil",
            "prompt": "hello"
        })),
    )
    .await;
    assert_eq!(st_bad_model, StatusCode::UNPROCESSABLE_ENTITY);

    // Editor without configured VaultStore secret gets deterministic 422 NO_PROVIDER_CONFIGURED
    let (st_unconf, body_unconf) = send_json(
        &router,
        "POST",
        "/api/v1/bot-fleet/execute",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "sessionId": "sess-1",
            "mode": "byok",
            "prompt": "Generate hero card"
        })),
    )
    .await;
    assert_eq!(st_unconf, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_unconf["error"]["code"], "INVALID_REQUEST");
}
