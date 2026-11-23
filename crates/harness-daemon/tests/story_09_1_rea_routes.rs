// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (Full build_router Integration Tests for /api/v1/rea/*)
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
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct TestHarness {
    _dir: tempfile::TempDir,
    app: Router,
    editor_a_tok: String,
    viewer_a_tok: String,
    editor_b_tok: String,
}

fn setup_router() -> TestHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-091.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_a_tok = "cwk_sess_091_editor_a".to_string();
    auth.insert_session(
        editor_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_a_tok = "cwk_sess_091_viewer_a".to_string();
    auth.insert_session(
        viewer_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".to_string(),
            user_id: "viewer-a".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let editor_b_tok = "cwk_sess_091_editor_b".to_string();
    auth.insert_session(
        editor_b_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".to_string(),
            user_id: "user-b".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let state = DaemonAppState::with_auth(registry, "127.0.0.1".to_string(), 4170, auth);
    let app = build_router(state);

    TestHarness {
        _dir: dir,
        app,
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
async fn it_09_1_01_auth_rbac_ssrf_firewall_fts5_tenant_isolation_and_honest_proof() {
    let h = setup_router();

    // 1. Unauthenticated & legacy Bearer rejected with 401
    let (st_unauth, _) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/search?q=fintech",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    let (st_bearer, _) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/search?q=fintech",
        None,
        Some("arbitrary-legacy-project-token"),
        None,
    )
    .await;
    assert_eq!(st_bearer, StatusCode::UNAUTHORIZED);

    // 2. Viewer cannot mutate via POST /api/v1/rea/ingest (403)
    let (st_viewer, _) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/ingest",
        Some(&h.viewer_a_tok),
        None,
        Some(serde_json::json!({
            "slug": "viewer-attempt",
            "sourceUrl": "https://github.com/open-design/ui-kit",
            "sourceFormat": "github",
            "licenseSpdx": "MIT"
        })),
    )
    .await;
    assert_eq!(st_viewer, StatusCode::FORBIDDEN);

    // 3. SSRF private IP blocked (422 SSRF_URL_BLOCKED)
    let (st_ssrf, body_ssrf) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/ingest",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "slug": "ssrf-attempt",
            "sourceUrl": "http://169.254.169.254/latest/meta-data/",
            "sourceFormat": "url",
            "licenseSpdx": "MIT"
        })),
    )
    .await;
    assert_eq!(st_ssrf, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_ssrf["error"]["code"], "SSRF_URL_BLOCKED");

    // 4. GPL-3.0 rejected by License Firewall (422 FIREWALL_REJECTED)
    let (st_gpl, body_gpl) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/ingest",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "slug": "gpl-blocked",
            "sourceUrl": "https://github.com/example/gpl-app",
            "sourceFormat": "github",
            "licenseSpdx": "GPL-3.0"
        })),
    )
    .await;
    assert_eq!(st_gpl, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_gpl["error"]["code"], "FIREWALL_REJECTED");

    // 5. MIT source with synthetic SQL/secret payload sanitized & indexed in Two-Store FTS5
    let synthetic_secret = ["sk", "proj", "0123456789abcdefghij0123456789"].join("-");
    let (st_ok, body_ok) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/ingest",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({
            "slug": "alpha-fintech-kit",
            "sourceUrl": "https://github.com/open-design/fintech-kit",
            "sourceFormat": "github",
            "licenseSpdx": "MIT",
            "payloadText": format!("<main style='color:#0f766e'>Fintech Ledger</main> SELECT * FROM users; Authorization: Bearer {synthetic_secret}")
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "ingest body: {body_ok}");
    assert_eq!(body_ok["ok"], true);
    assert_eq!(body_ok["data"]["firewall"]["verdict"], "ADOPT_PRIOR_ART");
    assert!(
        body_ok["data"]["record"]["sanitizedSecretsCount"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert!(
        body_ok["data"]["record"]["tier1aTokens"]
            .as_u64()
            .unwrap_or(999)
            <= 250
    );
    let ref_id = body_ok["data"]["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 6. Tenant A finds it via FTS5; Tenant B sees 0 hits (strict tenant isolation)
    let (st_search_a, body_search_a) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/search?q=fintech&limit=10&tier=1a",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_search_a, StatusCode::OK);
    assert_eq!(body_search_a["data"]["total"], 1);
    assert_eq!(
        body_search_a["data"]["items"][0]["slug"],
        "alpha-fintech-kit"
    );

    let (st_search_b, body_search_b) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/search?q=fintech&limit=10&tier=1a",
        Some(&h.editor_b_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_search_b, StatusCode::OK);
    assert_eq!(body_search_b["data"]["total"], 0);

    // 7. Honest Proof Exporter: awaiting-verification -> proof-verified on exact SHA-256 match
    let (st_p1, body_p1) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/export-proof",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({ "referenceId": ref_id })),
    )
    .await;
    assert_eq!(st_p1, StatusCode::OK);
    assert_eq!(body_p1["data"]["proofStatus"], "awaiting-verification");
    let sha = body_p1["data"]["computedSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // Cross-tenant export-proof denied with 404 REFERENCE_NOT_FOUND
    let (st_p_b, body_p_b) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/export-proof",
        Some(&h.editor_b_tok),
        None,
        Some(serde_json::json!({ "referenceId": ref_id, "roundtripChecksum": sha })),
    )
    .await;
    assert_eq!(st_p_b, StatusCode::NOT_FOUND);
    assert_eq!(body_p_b["error"]["code"], "REFERENCE_NOT_FOUND");

    // Same tenant with matching checksum verifies honestly
    let (st_p2, body_p2) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/rea/export-proof",
        Some(&h.editor_a_tok),
        None,
        Some(serde_json::json!({ "referenceId": ref_id, "roundtripChecksum": sha })),
    )
    .await;
    assert_eq!(st_p2, StatusCode::OK);
    assert_eq!(body_p2["data"]["proofStatus"], "proof-verified");
    assert_eq!(body_p2["data"]["verified"], true);
}

#[tokio::test]
async fn it_09_1_02_auth_before_rbac_rejects_fake_legacy_bearer_with_viewer_role_as_401() {
    let h = setup_router();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/rea/ingest")
        .header("host", "127.0.0.1:4170")
        .header("authorization", "Bearer fake-legacy")
        .header("x-cowok-role", "viewer")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "slug": "auth-before-rbac-probe",
                "sourceUrl": "https://github.com/open-design/ui-kit",
                "sourceFormat": "github",
                "licenseSpdx": "MIT"
            })
            .to_string(),
        ))
        .expect("req");

    let resp = h.app.clone().oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Unauthenticated fake-legacy bearer + x-cowok-role: viewer MUST return 401 UNAUTHORIZED, not 403"
    );
}

#[tokio::test]
async fn it_09_1_03_awesome_re_catalog_top_5_live_fts5_and_auth() {
    let h = setup_router();

    // Unauthenticated & legacy Bearer rejected with 401
    let (st_unauth, _) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=&limit=5",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    let (st_bearer, _) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=figma&limit=5",
        None,
        Some("legacy-project-token"),
        None,
    )
    .await;
    assert_eq!(st_bearer, StatusCode::UNAUTHORIZED);

    // Default empty query returns Top-5 real apps from alexgaoth/awesome-reverse-engineered
    let (st_top5, body_top5) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=&limit=5",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_top5, StatusCode::OK, "catalog top5 body: {body_top5}");
    assert_eq!(body_top5["ok"], true);
    assert_eq!(
        body_top5["data"]["upstreamRepo"],
        "https://github.com/alexgaoth/awesome-reverse-engineered"
    );
    assert!(
        body_top5["data"]["totalCatalogEntries"]
            .as_u64()
            .unwrap_or(0)
            >= 200
    );
    assert_eq!(body_top5["data"]["total"], 5);
    assert_eq!(body_top5["data"]["items"][0]["slug"], "photocraft");
    assert_eq!(
        body_top5["data"]["items"][0]["layoutArchetype"],
        "canvas-editor"
    );

    // Search "figma" returns FigCraft
    let (st_figma, body_figma) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=figma&limit=5",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_figma, StatusCode::OK);
    let figma_items = body_figma["data"]["items"].as_array().expect("items array");
    assert!(
        figma_items.iter().any(|item| item["slug"] == "figcraft"),
        "expected figcraft in figma search results: {body_figma}"
    );

    // Search "canvas" returns canvas-editor apps & "music" returns Top-5 media apps
    let (st_canvas, body_canvas) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=canvas&limit=5",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_canvas, StatusCode::OK);
    assert!(body_canvas["data"]["total"].as_u64().unwrap_or(0) >= 1);

    let (st_music, body_music) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=music&limit=5",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_music, StatusCode::OK);
    assert_eq!(body_music["data"]["total"], 5);

    // Search "webapp" surfaces non-GitHub Alternative Frontends / Web Apps (Codeberg/SourceHut/GitLab)
    let (st_web, body_web) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/rea/catalog?q=webapp&limit=5",
        Some(&h.viewer_a_tok),
        None,
        None,
    )
    .await;
    assert_eq!(st_web, StatusCode::OK);
    assert_eq!(body_web["data"]["total"], 5);
}
