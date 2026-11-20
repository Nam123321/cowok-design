// @story 06.1
// @implements AC-01, AC-02, AC-04, AC-05
// @cover IT-01..IT-05
use std::fs;
use std::path::Path;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState, ProjectDto};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::presentation_repo::is_valid_slide_uid;
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
    let tok = format!("sess-061-{}", uuid::Uuid::new_v4());
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

async fn send_req_with_headers(
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
    let st = resp.status();
    let out = resp.into_body().collect().await.unwrap().to_bytes();
    (
        st,
        serde_json::from_slice(&out).unwrap_or(serde_json::Value::Null),
    )
}

async fn create_project(
    router: &axum::Router,
    tok: &str,
    slug: &str,
    ws_root: Option<&Path>,
) -> ProjectDto {
    let (st, val) = send_req_with_headers(
        router,
        "POST",
        "/api/v1/projects",
        Some(tok),
        &[],
        Some(json!({
            "slug": slug,
            "name": format!("Project {slug}"),
            "workspace_root": ws_root.map(|p| p.to_string_lossy().to_string()),
            "product_brief": "Story 06.1 presentation deck integration test"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "project create failed: {val}");
    serde_json::from_value(val).unwrap()
}

fn sample_create_deck_body(title: &str) -> serde_json::Value {
    json!({
        "title": title,
        "themePreset": "night",
        "aspectRatio": "16:9",
        "deckMode": "multi-page",
        "initialSlides": [
            {
                "layoutKind": "hero_cover",
                "eyebrowTag": "Q4 BRIEFING",
                "narrativeArc": "Establish executive thesis",
                "headline": "Architecture Overview"
            },
            {
                "layoutKind": "kpi_metric_grid",
                "eyebrowTag": "METRICS",
                "narrativeArc": "Highlight reliability improvements",
                "headline": "Operational Health"
            }
        ]
    })
}

#[tokio::test]
async fn it_01_auth_rbac_and_idempotency_create_and_replay() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    let project_ws = tmp.path().join("ws-it01");
    fs::create_dir_all(&project_ws).unwrap();
    let project = create_project(&router, &editor_tok, "p-it01", Some(&project_ws)).await;

    // 1. Missing token -> 401
    let (st_unauth, _) = send_req_with_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/decks", project.id),
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    // 2. Viewer cannot mutate -> 403
    let viewer_tok = add_session(&state, "tenant-alpha", "viewer-1", "viewer");
    let (st_viewer, err_viewer) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&viewer_tok),
        &[],
        Some(sample_create_deck_body("Forbidden Deck")),
    )
    .await;
    assert_eq!(st_viewer, StatusCode::FORBIDDEN, "{err_viewer}");

    // 3. Editor creates deck with Idempotency-Key -> 201
    let body = sample_create_deck_body("Executive Review");
    let (st_created, created) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&editor_tok),
        &[("idempotency-key", "idem-deck-create-1")],
        Some(body.clone()),
    )
    .await;
    assert_eq!(st_created, StatusCode::CREATED, "{created}");
    let deck_id = created["id"].as_str().unwrap().to_string();
    let slides = created["slides"].as_array().unwrap();
    assert_eq!(slides.len(), 2);
    for s in slides {
        let suid = s["slideUid"].as_str().unwrap();
        assert!(is_valid_slide_uid(suid), "invalid slideUid {suid}");
        let html_path = project_ws.join(s["htmlPath"].as_str().unwrap());
        assert!(html_path.is_file(), "staged file missing: {html_path:?}");
    }

    // 4. Replay exact Idempotency-Key returns same deck id
    let (st_replay, replayed) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&editor_tok),
        &[("idempotency-key", "idem-deck-create-1")],
        Some(body),
    )
    .await;
    assert_eq!(st_replay, StatusCode::CREATED);
    assert_eq!(replayed["id"].as_str().unwrap(), deck_id);

    // 5. Reusing Idempotency-Key with mismatched body -> 422
    let (st_mismatch, mismatch_body) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&editor_tok),
        &[("idempotency-key", "idem-deck-create-1")],
        Some(sample_create_deck_body("Different Payload")),
    )
    .await;
    assert_eq!(
        st_mismatch,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{mismatch_body}"
    );
}

#[tokio::test]
async fn it_02_reorder_occ_conflict_and_atomic_swap() {
    let tmp = TempDir::new().unwrap();
    let (_state, router, tok) = setup_daemon(&tmp);
    let project_ws = tmp.path().join("ws-it02");
    fs::create_dir_all(&project_ws).unwrap();
    let project = create_project(&router, &tok, "p-it02", Some(&project_ws)).await;

    let (st, created) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&tok),
        &[],
        Some(sample_create_deck_body("Reorder Deck")),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let deck_id = created["id"].as_str().unwrap();
    let s0 = created["slides"][0]["slideUid"]
        .as_str()
        .unwrap()
        .to_string();
    let s1 = created["slides"][1]["slideUid"]
        .as_str()
        .unwrap()
        .to_string();

    // Wrong revision -> 409 E_STALE_REVISION with actualRevision
    let (st_stale, stale_body) = send_req_with_headers(
        &router,
        "PUT",
        &format!("/api/v1/decks/{deck_id}/slides/reorder"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 99,
            "orderedSlideUids": [&s1, &s0]
        })),
    )
    .await;
    assert_eq!(st_stale, StatusCode::CONFLICT, "{stale_body}");
    assert_eq!(stale_body["error"]["code"], "E_STALE_REVISION");
    assert_eq!(stale_body["error"]["details"]["actualRevision"], 1);

    // Valid revision -> 200 OK with swapped order and revision 2
    let (st_ok, reordered) = send_req_with_headers(
        &router,
        "PUT",
        &format!("/api/v1/decks/{deck_id}/slides/reorder"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 1,
            "orderedSlideUids": [&s1, &s0]
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "{reordered}");
    assert_eq!(reordered["revision"], 2);
    assert_eq!(reordered["slides"][0]["slideUid"], s1);
    assert_eq!(reordered["slides"][1]["slideUid"], s0);

    // Duplicate UID in orderedSlideUids must return 409 E_SLIDE_INDEX_CONFLICT (not 500)
    let (st_dup_uid, dup_uid_body) = send_req_with_headers(
        &router,
        "PUT",
        &format!("/api/v1/decks/{deck_id}/slides/reorder"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 2,
            "orderedSlideUids": [&s1, &s1]
        })),
    )
    .await;
    assert_eq!(st_dup_uid, StatusCode::CONFLICT, "{dup_uid_body}");
    assert_eq!(dup_uid_body["error"]["code"], "E_SLIDE_INDEX_CONFLICT");
}

#[tokio::test]
async fn it_03_duplicate_slide_creates_fresh_uid_and_rewrites_ff_ids() {
    let tmp = TempDir::new().unwrap();
    let (_state, router, tok) = setup_daemon(&tmp);
    let project_ws = tmp.path().join("ws-it03");
    fs::create_dir_all(&project_ws).unwrap();
    let project = create_project(&router, &tok, "p-it03", Some(&project_ws)).await;

    let (st, created) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&tok),
        &[],
        Some(sample_create_deck_body("Dup Deck")),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let deck_id = created["id"].as_str().unwrap();
    let s0_uid = created["slides"][0]["slideUid"].as_str().unwrap();
    let s0_rel = created["slides"][0]["htmlPath"].as_str().unwrap();

    let (st_dup, dup_body) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/slides/{s0_uid}/duplicate"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedDeckRevision": 1,
            "insertAfterSlideUid": s0_uid
        })),
    )
    .await;
    assert_eq!(st_dup, StatusCode::CREATED, "{dup_body}");
    assert_eq!(dup_body["deckRevision"], 2);
    let new_uid = dup_body["slide"]["slideUid"].as_str().unwrap();
    assert_ne!(new_uid, s0_uid);
    assert!(is_valid_slide_uid(new_uid));
    assert_eq!(dup_body["slide"]["slideIndex"], 1);

    let orig_html = fs::read_to_string(project_ws.join(s0_rel)).unwrap();
    let dup_html =
        fs::read_to_string(project_ws.join(dup_body["slide"]["htmlPath"].as_str().unwrap()))
            .unwrap();
    let orig_val = presentation_cms::validate_slide_protocol_structure(&orig_html).unwrap();
    let dup_val = presentation_cms::validate_slide_protocol_structure(&dup_html).unwrap();
    assert_eq!(orig_val.unique_ff_ids.len(), dup_val.unique_ff_ids.len());
    for ff in &dup_val.unique_ff_ids {
        assert!(
            !orig_val.unique_ff_ids.contains(ff),
            "duplicated ff-id {ff} collided with original"
        );
    }
}

#[tokio::test]
async fn it_04_anti_idor_cross_tenant_returns_uniform_404() {
    let tmp = TempDir::new().unwrap();
    let (state, router, owner_tok) = setup_daemon(&tmp);
    let project_ws = tmp.path().join("ws-it04");
    fs::create_dir_all(&project_ws).unwrap();
    let project = create_project(&router, &owner_tok, "p-it04", Some(&project_ws)).await;

    let (st, created) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&owner_tok),
        &[],
        Some(sample_create_deck_body("Private Deck")),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let deck_id = created["id"].as_str().unwrap();
    let s0_uid = created["slides"][0]["slideUid"].as_str().unwrap();

    let foreign_tok = add_session(&state, "tenant-beta", "attacker", "editor");

    let (st_get, b_get) = send_req_with_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&foreign_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_get, StatusCode::NOT_FOUND, "{b_get}");
    assert_eq!(b_get["error"]["code"], "E_DECK_NOT_FOUND");

    let (st_put, b_put) = send_req_with_headers(
        &router,
        "PUT",
        &format!("/api/v1/decks/{deck_id}/slides/reorder"),
        Some(&foreign_tok),
        &[],
        Some(json!({ "expectedRevision": 1, "orderedSlideUids": [s0_uid] })),
    )
    .await;
    assert_eq!(st_put, StatusCode::NOT_FOUND, "{b_put}");
    assert_eq!(b_put["error"]["code"], "E_DECK_NOT_FOUND");
}

#[tokio::test]
async fn it_05_fail_closed_422_on_missing_or_corrupted_slide_artifact_and_startup_sweep() {
    let tmp = TempDir::new().unwrap();
    let (_state, router, tok) = setup_daemon(&tmp);
    let project_ws = tmp.path().join("ws-it05");
    fs::create_dir_all(&project_ws).unwrap();
    let project = create_project(&router, &tok, "p-it05", Some(&project_ws)).await;

    let (st, created) = send_req_with_headers(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&tok),
        &[],
        Some(sample_create_deck_body("Integrity Deck")),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let s0_rel = created["slides"][0]["htmlPath"].as_str().unwrap();
    let abs_slide = project_ws.join(s0_rel);

    // 1. Corrupt slide HTML on disk -> SHA mismatch must fail closed with 422 E_DECK_ARTIFACT_MISSING
    fs::write(&abs_slide, "<html><body>tampered</body></html>").unwrap();
    let (st_corrupt, corrupt_body) = send_req_with_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(
        st_corrupt,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{corrupt_body}"
    );
    assert_eq!(corrupt_body["error"]["code"], "E_DECK_ARTIFACT_MISSING");

    // 2. Delete slide HTML -> missing file must also fail closed with 422 E_DECK_ARTIFACT_MISSING
    fs::remove_file(&abs_slide).unwrap();
    let (st_missing, missing_body) = send_req_with_headers(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/decks", project.id),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(
        st_missing,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{missing_body}"
    );
    assert_eq!(missing_body["error"]["code"], "E_DECK_ARTIFACT_MISSING");

    // 3. Startup orphan sweep cleans stale .cowok-tmp-* and .tmp* staging files older than 60s
    let stale_tmp = project_ws.join("presentations/deck-orphan/slides/SLD-ORPHAN.tmp.123");
    let stale_cowok_tmp = project_ws.join("presentations/deck-orphan/slides/.cowok-tmp-abc123");
    fs::create_dir_all(stale_tmp.parent().unwrap()).unwrap();
    fs::write(&stale_tmp, "orphaned").unwrap();
    fs::write(&stale_cowok_tmp, "orphaned-cowok").unwrap();
    let stale_ts = std::process::Command::new("touch")
        .args([
            "-t",
            "202001010000",
            stale_tmp.to_str().unwrap(),
            stale_cowok_tmp.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(stale_ts.success());
    let swept = harness_daemon::routes::presentation::sweep_orphan_tmp_slides(&project_ws);
    assert_eq!(swept, 2);
    assert!(!stale_tmp.exists());
    assert!(!stale_cowok_tmp.exists());
}
