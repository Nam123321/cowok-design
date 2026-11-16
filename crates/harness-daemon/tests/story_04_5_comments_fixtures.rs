// @story 04.5
// @implements AC-01, AC-03 (SQLite V7 ElementComment CommitAllowed CAS 409 STALE_MAPPING & Uppercase ScreenFixture)
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState, ProjectDto};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok = format!("sess-045-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok)
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

async fn create_project(router: &axum::Router, tok: &str) -> ProjectDto {
    let (st, val) = send_req(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        Some(json!({ "slug": "story045-cas", "name": "Story 04.5 Test", "product_brief": "live preview" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "project create: {val}");
    serde_json::from_value(val).unwrap()
}

#[tokio::test]
async fn test_element_comment_cas_stale_mapping_and_reanchor() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok).await;
    let node_id = "node-scr-01";

    // Seed flow_graph + flow_node + active ScreenArtifact digest for node_id.
    {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO flow_graphs (id, project_id, portal_key, title, updated_at)
             VALUES (?1, ?2, 'default', 'Story 04.5 Graph', ?3)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), proj.id.to_string(), now_ms],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, project_id, screen_key, title, route_path, fixture_ref, viewport_preset, created_at, updated_at)
             SELECT ?1, id, ?2, 'SCR-01', 'Home', '/home', 'DEFAULT', 'desktop-1440', ?3, ?3
             FROM flow_graphs WHERE project_id = ?2 LIMIT 1",
            rusqlite::params![node_id, proj.id.to_string(), now_ms],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO screen_artifacts (id, node_id, content_hash, artifact_path, anchor_status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'LINKED', ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                node_id,
                "sha256-aaa111",
                ".cowok-design/screens/scr01.html",
                now_ms
            ],
        )
        .unwrap();
    }

    // 1. Mismatched artifactSha256 on create fails closed with 409 STALE_MAPPING.
    let (st_stale_create, val_stale_create) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/nodes/{}/comments", proj.id, node_id),
        &tok,
        Some(json!({
            "elementFfId": "sec-header",
            "author": "Designer",
            "content": "Adjust contrast",
            "domHash": "11223344",
            "artifactSha256": "sha256-outdated",
            "anchorKind": "html_element"
        })),
    )
    .await;
    assert_eq!(st_stale_create, StatusCode::CONFLICT, "{val_stale_create}");
    assert_eq!(val_stale_create["code"], "STALE_MAPPING");

    // 2. Matching artifactSha256 + anchorKind creates active comment cleanly.
    let (st_ok, val_ok) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/nodes/{}/comments", proj.id, node_id),
        &tok,
        Some(json!({
            "elementFfId": "sec-header",
            "author": "Designer",
            "content": "Tighten CTA spacing",
            "domHash": "aabbccdd",
            "artifactSha256": "sha256-aaa111",
            "anchorKind": "marquee_box"
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::CREATED, "{val_ok}");
    assert_eq!(val_ok["artifactSha256"], "sha256-aaa111");
    assert_eq!(val_ok["anchorKind"], "marquee_box");
    let cid = val_ok["id"].as_str().unwrap().to_string();

    // 3. Simulate disk artifact drift in screen_artifacts -> PATCH without reanchor triggers 409 STALE_MAPPING & marks stale.
    {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis() + 100;
        conn.execute(
            "INSERT INTO screen_artifacts (id, node_id, content_hash, artifact_path, anchor_status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'LINKED', ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                node_id,
                "sha256-bbb222",
                ".cowok-design/screens/scr01.html",
                now_ms
            ],
        )
        .unwrap();
    }

    let (st_conflict, val_conflict) = send_req(
        &router,
        "PATCH",
        &format!(
            "/api/v1/projects/{}/flow/nodes/{}/comments/{}",
            proj.id, node_id, cid
        ),
        &tok,
        Some(json!({ "content": "Updated while drifted" })),
    )
    .await;
    assert_eq!(st_conflict, StatusCode::CONFLICT, "{val_conflict}");
    assert_eq!(val_conflict["code"], "STALE_MAPPING");

    // Verify GET reflects anchorStatus == "stale".
    let (st_list, val_list) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/flow/nodes/{}/comments", proj.id, node_id),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_list, StatusCode::OK);
    assert_eq!(val_list[0]["anchorStatus"], "stale");

    // 4. Explicit reanchor: true restores anchorStatus -> "active" with fresh SHA-256 & DOM hash.
    let (st_reanchor, val_reanchor) = send_req(
        &router,
        "PATCH",
        &format!(
            "/api/v1/projects/{}/flow/nodes/{}/comments/{}",
            proj.id, node_id, cid
        ),
        &tok,
        Some(json!({
            "reanchor": true,
            "elementFfId": "sec-header-v2",
            "domHash": "eeff0011",
            "artifactSha256": "sha256-bbb222"
        })),
    )
    .await;
    assert_eq!(st_reanchor, StatusCode::OK, "{val_reanchor}");
    assert_eq!(val_reanchor["anchorStatus"], "active");
    assert_eq!(val_reanchor["elementFfId"], "sec-header-v2");
    assert_eq!(val_reanchor["artifactSha256"], "sha256-bbb222");
}

#[tokio::test]
async fn test_screen_fixtures_uppercase_variants() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok).await;
    let node_id = "node-scr-02";

    // Lowercase variant fails fail-closed validation (only DEFAULT | EMPTY | LOADING | ERROR accepted).
    let (st_bad, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/flow/nodes/{}/fixtures", proj.id, node_id),
        &tok,
        Some(json!({ "variant": "loading", "payloadJson": "{}", "isDefault": false })),
    )
    .await;
    assert_eq!(st_bad, StatusCode::BAD_REQUEST);

    for v in ["DEFAULT", "LOADING", "EMPTY", "ERROR"] {
        let (st, val) = send_req(
            &router,
            "POST",
            &format!("/api/v1/projects/{}/flow/nodes/{}/fixtures", proj.id, node_id),
            &tok,
            Some(json!({
                "variant": v,
                "payloadJson": json!({ "state": v }).to_string(),
                "isDefault": v == "DEFAULT"
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED, "fixture {v}: {val}");
        assert_eq!(val["variant"], v);
    }
}
