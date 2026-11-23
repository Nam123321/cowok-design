// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (Full build_router Integration Tests for /api/v1/decision/* & V21 Persistence)
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use chrono::Utc;
use harness_daemon::daemon_auth::{DaemonAuthState, FlowPrincipal};
use harness_daemon::routes::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::Value;
use sha2::{Digest, Sha256};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct DecisionHarness {
    _dir: tempfile::TempDir,
    app: Router,
    project_a: String,
    editor_a_tok: String,
    viewer_a_tok: String,
    editor_b_tok: String,
}

fn setup_router() -> DecisionHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-092.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let project_a = "22222222-2222-4222-8222-222222222222".to_string();
    {
        let conn = registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES (?1, 'tenant-a', 'proj-092', 'Decision Proj', ?2, '', 'ACTIVE', 1000, 1000)",
            rusqlite::params![project_a, dir.path().to_string_lossy().as_ref()],
        )
        .expect("seed project");
    }

    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_a_tok = "cwk_sess_092_editor_a".to_string();
    auth.insert_session(
        editor_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_a_tok = "cwk_sess_092_viewer_a".to_string();
    auth.insert_session(
        viewer_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "viewer-a".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let editor_b_tok = "cwk_sess_092_editor_b".to_string();
    auth.insert_session(
        editor_b_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-b".to_string(),
            user_id: "user-b".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let state = DaemonAppState::with_auth(registry, "127.0.0.1".to_string(), 4170, auth);
    let app = build_router(state);

    DecisionHarness {
        _dir: dir,
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
) -> (StatusCode, Value) {
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
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

#[tokio::test]
async fn it_09_2_01_auth_tenant_isolation_v21_persistence_and_stale_file_occ() {
    let h = setup_router();
    let artifact_context =
        "Fintech dashboard with OKLCH contrast tokens and accessibility guardrails";
    let expected_sha = hex_sha256(artifact_context.as_bytes());

    let route_payload = serde_json::json!({
        "projectId": h.project_a,
        "stage": "DECISION_1",
        "prompt": "Select the best OKLCH fintech design skill",
        "context": artifact_context,
        "untrustedArtifactSha256": expected_sha,
        "staleFileSha256": expected_sha,
        "currentFileSha256": expected_sha,
        "candidates": [
            { "id": "oklch-palette", "label": "OKLCH Contrast Palette", "text": "OKLCH semantic contrast tokens for fintech dashboard", "deScoreBps": 9100 },
            { "id": "slide-deck", "label": "Slide Deck Presenter", "text": "Markdown presentation stage transitions", "deScoreBps": 4200 }
        ],
        "attemptNumber": 1,
        "priorVerdicts": [],
        "preferExternalTier": false
    });

    // 1. Unauthenticated & legacy Bearer rejected with 401
    let (st_unauth, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        None,
        None,
        Some(route_payload.clone()),
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    let (st_bearer, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        None,
        Some("legacy-project-bearer"),
        Some(route_payload.clone()),
    )
    .await;
    assert_eq!(st_bearer, StatusCode::UNAUTHORIZED);

    // 2. Viewer forbidden on POST (403)
    let (st_viewer, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        Some(&h.viewer_a_tok),
        None,
        Some(route_payload.clone()),
    )
    .await;
    assert_eq!(st_viewer, StatusCode::FORBIDDEN);

    // 3. Cross-tenant anti-IDOR forbidden (403)
    let (st_idor, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        Some(&h.editor_b_tok),
        None,
        Some(route_payload.clone()),
    )
    .await;
    assert_eq!(st_idor, StatusCode::FORBIDDEN);

    // 4. StaleFile OCC mismatch returns 409 STALE_FILE_OCC
    let mut stale_payload = route_payload.clone();
    stale_payload["staleFileSha256"] = serde_json::json!("f".repeat(64));
    let (st_stale, body_stale) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        Some(&h.editor_a_tok),
        None,
        Some(stale_payload),
    )
    .await;
    assert_eq!(st_stale, StatusCode::CONFLICT);
    assert_eq!(body_stale["error"]["code"], "STALE_FILE_OCC");

    // 5. Valid offline route executes 5-stage cascade & persists to V21 decision_audit_log
    let (st_ok, body_ok) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/decision/route",
        Some(&h.editor_a_tok),
        None,
        Some(route_payload),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "route body: {body_ok}");
    assert_eq!(body_ok["ok"], true);
    assert_eq!(
        body_ok["data"]["output"]["verdictLabel"],
        "OKLCH Contrast Palette"
    );
    assert_eq!(body_ok["data"]["output"]["tierUsed"], "LOCAL_FTS5_BM25");
    assert_eq!(
        body_ok["data"]["output"]["stageTrace"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert!(
        !body_ok.to_string().contains("logprob"),
        "VerdictShield must never leak logprobs"
    );

    // 6. GET /api/v1/decision/audit returns persisted V21 row for tenant-a & blocks tenant-b
    let (st_audit, body_audit) = send_req(
        &h.app,
        Method::GET,
        &format!(
            "/api/v1/decision/audit?projectId={}&stage=DECISION_1&limit=10",
            h.project_a
        ),
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_audit, StatusCode::OK, "audit body: {body_audit}");
    assert_eq!(body_audit["data"]["totalCount"], 1);
    assert_eq!(
        body_audit["data"]["items"][0]["verdictLabel"],
        "OKLCH Contrast Palette"
    );

    let (st_audit_b, _) = send_req(
        &h.app,
        Method::GET,
        &format!("/api/v1/decision/audit?projectId={}", h.project_a),
        Some(&h.editor_b_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_audit_b, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn it_09_2_02_f07_legacy_bearer_viewer_returns_401_and_f08_uppercase_uuid_and_true_count() {
    let h = setup_router();
    let artifact_context = "Fintech dashboard tokens";
    let expected_sha = hex_sha256(artifact_context.as_bytes());
    let upper_project_id = h.project_a.to_ascii_uppercase();
    let upper_sha = expected_sha.to_ascii_uppercase();

    let payload = serde_json::json!({
        "projectId": upper_project_id,
        "stage": "DECISION_1",
        "prompt": "Design hero landing page craft typography color palette",
        "context": artifact_context,
        "untrustedArtifactSha256": expected_sha,
        "staleFileSha256": upper_sha,
        "currentFileSha256": expected_sha,
        "attemptNumber": 1,
        "priorVerdicts": [],
        "preferExternalTier": false
    });

    // F-07: Legacy Bearer + x-cowok-role: viewer MUST return 401 UNAUTHORIZED (not 403)
    let legacy_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/decision/route")
        .header("host", "127.0.0.1:4170")
        .header("authorization", "Bearer legacy-project-bearer")
        .header("x-cowok-role", "viewer")
        .header("x-cowok-tenant-id", "tenant-a")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("req");
    let legacy_resp = h.app.clone().oneshot(legacy_req).await.expect("oneshot");
    let legacy_status = legacy_resp.status();
    let legacy_bytes = legacy_resp
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let legacy_json: Value = serde_json::from_slice(&legacy_bytes).unwrap_or(Value::Null);
    assert_eq!(
        legacy_status,
        StatusCode::UNAUTHORIZED,
        "legacy Bearer + viewer must return 401 UNAUTHORIZED, got: {legacy_json}"
    );
    assert_eq!(legacy_json["code"], "UNAUTHORIZED");

    // F-02 + F-08: Uppercase UUID projectId + uppercase staleFileSha256 work & persist normalized lowercase
    for _ in 0..3 {
        let (st, body) = send_req(
            &h.app,
            Method::POST,
            "/api/v1/decision/route",
            Some(&h.editor_a_tok),
            None,
            Some(payload.clone()),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "uppercase UUID route failed: {body}");
    }

    // F-08: GET /api/v1/decision/audit with uppercase projectId & limit=1 returns 1 item and true totalCount=3
    let (st_audit, body_audit) = send_req(
        &h.app,
        Method::GET,
        &format!(
            "/api/v1/decision/audit?projectId={}&stage=DECISION_1&limit=1",
            upper_project_id
        ),
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_audit, StatusCode::OK, "audit body: {body_audit}");
    assert_eq!(body_audit["data"]["projectId"], h.project_a);
    assert_eq!(body_audit["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(body_audit["data"]["totalCount"], 3);
    assert_eq!(
        body_audit["data"]["items"][0]["staleFileSha256"], expected_sha,
        "stale_file_sha256 must be persisted in normalized lowercase"
    );
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}
