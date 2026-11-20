// @story 06.4
// @implements AC-01, AC-02, AC-04, IT-01, IT-02, IT-03, IT-04, IT-05
use std::fs;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use rusqlite::params;
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
    let tok = add_session(&state, "tenant-alpha", "designer-1", "editor");
    let router = build_router((*state).clone());
    (state, router, tok)
}

fn add_session(state: &Arc<DaemonAppState>, tenant_id: &str, user_id: &str, role: &str) -> String {
    let tok = format!("sess-064-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            role: role.into(),
        },
        exp,
    );
    tok
}

fn seed_project(state: &Arc<DaemonAppState>, id: &str, tenant_id: &str, ws_root: &std::path::Path) {
    let proj_ws = ws_root.join(id);
    fs::create_dir_all(&proj_ws).expect("mkdir proj ws");
    let now = Utc::now().timestamp_millis();
    let conn = state.registry.pool().connection().expect("conn");
    conn.execute(
        "INSERT INTO projects (id, slug, name, tenant_id, workspace_root, product_brief, status, created_at, updated_at)
         VALUES (?1, ?1, ?2, ?3, ?4, 'Story 06.4 presenter test', 'ACTIVE', ?5, ?5)",
        params![
            id,
            format!("Project {id}"),
            tenant_id,
            proj_ws.to_string_lossy().to_string(),
            now,
        ],
    )
    .expect("seed project");
}

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170");
    if let Some(t) = tok {
        b = b.header("x-cowok-session-token", t);
    }
    for (k, v) in extra_headers {
        b = b.header(*k, *v);
    }
    if !raw.is_empty() {
        b = b.header("content-type", "application/json");
    }
    let resp = router
        .clone()
        .oneshot(b.body(Body::from(raw)).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
    (status, json)
}

async fn create_deck_with_notes(
    router: &axum::Router,
    tok: &str,
    project_id: &str,
) -> (String, Vec<String>) {
    let (status, body) = send_req(
        router,
        "POST",
        &format!("/api/v1/projects/{project_id}/decks"),
        Some(tok),
        &[],
        Some(json!({
            "title": "Story 06.4 Presenter Deck",
            "themePreset": "night",
            "aspectRatio": "16:9",
            "deckMode": "multi-page",
            "initialSlides": [
                {
                    "layoutKind": "hero_cover",
                    "eyebrowTag": "SLD-01",
                    "narrativeArc": "Intro",
                    "headline": "Opening Keynote"
                },
                {
                    "layoutKind": "kpi_metric_grid",
                    "eyebrowTag": "SLD-02",
                    "narrativeArc": "Metrics",
                    "headline": "Q4 KPI Growth"
                }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create deck failed: {body}");
    let deck_id = body["id"].as_str().unwrap().to_string();
    let slide_uids: Vec<String> = body["slides"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["slideUid"].as_str().unwrap().to_string())
        .collect();
    let slide0_rev = body["slides"][0]["revision"].as_i64().unwrap_or(1);

    // Update speaker_notes via Story 06.2 batch endpoint on slide 0
    let (patch_status, patch_body) = send_req(
        router,
        "POST",
        &format!(
            "/api/v1/projects/{project_id}/decks/{deck_id}/slides/{}/slots/batch",
            slide_uids[0]
        ),
        Some(tok),
        &[],
        Some(json!({
            "expectedRevision": slide0_rev,
            "speakerNotes": "Pause 2 seconds; <script>alert(1)</script> plain note",
            "patches": []
        })),
    )
    .await;
    assert_eq!(
        patch_status,
        StatusCode::OK,
        "patch notes failed: {patch_body}"
    );

    (deck_id, slide_uids)
}

#[tokio::test]
async fn it_01_presenter_handshake_returns_ordered_slides_and_speaker_notes() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let viewer_tok = add_session(&state, "tenant-alpha", "viewer-1", "viewer");

    let (deck_id, slide_uids) = create_deck_with_notes(&router, &editor_tok, "proj-a").await;

    // Project-scoped handshake as viewer
    let (status_proj, body_proj) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/presenter/handshake"),
        Some(&viewer_tok),
        &[],
        Some(json!({
            "transportMode": "broadcast_channel",
            "activeSlideIndex": 1
        })),
    )
    .await;
    assert_eq!(
        status_proj,
        StatusCode::OK,
        "project handshake failed: {body_proj}"
    );
    assert_eq!(body_proj["deckId"], deck_id);
    assert_eq!(body_proj["projectId"], "proj-a");
    assert_eq!(body_proj["aspectRatio"], "16:9");
    assert_eq!(body_proj["activeSlideIndex"], 1);
    assert_eq!(body_proj["isHistoricalReadOnly"], false);
    assert!(body_proj["sessionId"].as_str().unwrap().len() >= 16);
    let slides = body_proj["slides"].as_array().unwrap();
    assert_eq!(slides.len(), 2);
    assert_eq!(slides[0]["slideUid"], slide_uids[0]);
    assert_eq!(slides[0]["slideIndex"], 0);
    assert_eq!(
        slides[0]["speakerNotes"],
        "Pause 2 seconds; <script>alert(1)</script> plain note"
    );
    assert_eq!(slides[1]["slideUid"], slide_uids[1]);
    assert_eq!(slides[1]["slideIndex"], 1);

    // Direct handshake route with X-Cowok-Project-Id
    let (status_direct, body_direct) = send_req(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/presenter/handshake"),
        Some(&viewer_tok),
        &[("x-cowok-project-id", "proj-a")],
        Some(json!({
            "transportMode": "tauri_ipc",
            "activeSlideIndex": 99
        })),
    )
    .await;
    assert_eq!(
        status_direct,
        StatusCode::OK,
        "direct handshake failed: {body_direct}"
    );
    // Clamped to slides.len() - 1 = 1
    assert_eq!(body_direct["activeSlideIndex"], 1);
}

#[tokio::test]
async fn it_02_anti_idor_cross_tenant_and_mismatched_project_returns_404() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    seed_project(&state, "proj-b", "tenant-beta", tmp.path());
    let foreign_tok = add_session(&state, "tenant-beta", "attacker-1", "editor");

    let (deck_id, _) = create_deck_with_notes(&router, &editor_tok, "proj-a").await;

    // Cross-tenant attacker on direct route
    let (s1, b1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/presenter/handshake"),
        Some(&foreign_tok),
        &[],
        Some(json!({ "transportMode": "broadcast_channel", "activeSlideIndex": 0 })),
    )
    .await;
    assert_eq!(s1, StatusCode::NOT_FOUND);
    assert_eq!(b1["error"]["code"], "E_DECK_NOT_FOUND");

    // Mismatched X-Cowok-Project-Id header on direct route
    let (s2, b2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/presenter/handshake"),
        Some(&editor_tok),
        &[("x-cowok-project-id", "proj-b")],
        Some(json!({ "transportMode": "broadcast_channel", "activeSlideIndex": 0 })),
    )
    .await;
    assert_eq!(s2, StatusCode::NOT_FOUND);
    assert_eq!(b2["error"]["code"], "E_DECK_NOT_FOUND");

    // Mismatched projectId in body on project-scoped route
    let (s3, b3) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/presenter/handshake"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "projectId": "proj-b",
            "transportMode": "broadcast_channel",
            "activeSlideIndex": 0
        })),
    )
    .await;
    assert_eq!(s3, StatusCode::NOT_FOUND);
    assert_eq!(b3["error"]["code"], "E_DECK_NOT_FOUND");
}

#[tokio::test]
async fn it_03_snapshot_tag_stamps_historical_readonly_and_blocks_foreign_snapshot() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    seed_project(&state, "proj-b", "tenant-beta", tmp.path());

    let (deck_id, _) = create_deck_with_notes(&router, &editor_tok, "proj-a").await;
    {
        let conn = state.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO version_snapshots (id, tenant_id, project_id, snapshot_tag, version_tag, label, manifest_sha256)
             VALUES (?1, 'tenant-alpha', 'proj-a', 'SNAP-064-ALPHA', 'v1.0.0', 'Release Cut', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
            params![uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert alpha snapshot");
        conn.execute(
            "INSERT INTO version_snapshots (id, tenant_id, project_id, snapshot_tag, version_tag, label, manifest_sha256)
             VALUES (?1, 'tenant-beta', 'proj-b', 'SNAP-064-FOREIGN', 'v1.0.0', 'Foreign Cut', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
            params![uuid::Uuid::new_v4().to_string()],
        )
        .expect("insert foreign snapshot");
    }

    // Valid project snapshot stamps isHistoricalReadOnly: true
    let (ok_status, ok_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/presenter/handshake"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "snapshotTag": "SNAP-064-ALPHA",
            "transportMode": "broadcast_channel",
            "activeSlideIndex": 0
        })),
    )
    .await;
    assert_eq!(
        ok_status,
        StatusCode::OK,
        "snapshot handshake failed: {ok_body}"
    );
    assert_eq!(ok_body["isHistoricalReadOnly"], true);
    assert_eq!(ok_body["snapshotTag"], "SNAP-064-ALPHA");

    // Foreign snapshot tag returns uniform 404
    let (bad_status, bad_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/presenter/handshake"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "snapshotTag": "SNAP-064-FOREIGN",
            "transportMode": "broadcast_channel",
            "activeSlideIndex": 0
        })),
    )
    .await;
    assert_eq!(bad_status, StatusCode::NOT_FOUND);
    assert_eq!(bad_body["error"]["code"], "E_DECK_NOT_FOUND");
}

#[tokio::test]
async fn it_04_tampered_slide_artifact_fails_closed_422() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, slide_uids) = create_deck_with_notes(&router, &editor_tok, "proj-a").await;

    let html_path: String = {
        let conn = state.registry.pool().connection().expect("conn");
        conn.query_row(
            "SELECT html_path FROM deck_slides WHERE deck_id = ?1 AND slide_uid = ?2",
            params![deck_id, slide_uids[0]],
            |r| r.get(0),
        )
        .expect("query html_path")
    };
    let full_path = tmp.path().join("proj-a").join(html_path);
    fs::write(&full_path, "<html><body>TAMPERED</body></html>").expect("tamper slide");

    let (status, body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/presenter/handshake"),
        Some(&editor_tok),
        &[],
        Some(json!({ "transportMode": "broadcast_channel", "activeSlideIndex": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "E_DECK_ARTIFACT_MISSING");
}

#[tokio::test]
async fn it_05_unauthenticated_and_invalid_payload_checks() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, _) = create_deck_with_notes(&router, &editor_tok, "proj-a").await;

    // Missing session token -> 401
    let (s_unauth, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/presenter/handshake"),
        None,
        &[],
        Some(json!({ "transportMode": "broadcast_channel", "activeSlideIndex": 0 })),
    )
    .await;
    assert_eq!(s_unauth, StatusCode::UNAUTHORIZED);

    // Invalid snapshotTag format -> 400
    let (s_bad, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/presenter/handshake"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "snapshotTag": "INVALID-TAG",
            "transportMode": "broadcast_channel",
            "activeSlideIndex": 0
        })),
    )
    .await;
    assert_eq!(s_bad, StatusCode::BAD_REQUEST);
}
