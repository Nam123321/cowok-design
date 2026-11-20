// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04, IT-01, IT-02, IT-03, IT-04, IT-05
use std::fs;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::execution_sweeper::sweep_expired_leases;
use harness_daemon::routes::presentation::sweep_orphan_slide_artifacts;
use harness_daemon::routes::presentation_copilot::{
    commit_slide_copilot_turn, run_pending_slide_copilot_jobs, CopilotSlotMutation,
};
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use spec_registry::presentation_repo::list_active_slide_html_paths;
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
    let tok = format!("sess-063-{}", uuid::Uuid::new_v4());
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
         VALUES (?1, ?1, ?2, ?3, ?4, 'Story 06.3 test', 'ACTIVE', ?5, ?5)",
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

async fn create_deck_with_slides(
    router: &axum::Router,
    tok: &str,
    project_id: &str,
    deck_mode: &str,
    slide_count: usize,
) -> (String, Vec<(String, String, i64)>) {
    let initial_slides: Vec<serde_json::Value> = (0..slide_count)
        .map(|i| {
            json!({
                "layoutKind": "kpi_metric_grid",
                "eyebrowTag": format!("S{}", i + 1),
                "narrativeArc": "KPI",
                "headline": format!("Slide Headline {}", i + 1)
            })
        })
        .collect();
    let (status, body) = send_req(
        router,
        "POST",
        &format!("/api/v1/projects/{project_id}/decks"),
        Some(tok),
        &[],
        Some(json!({
            "title": "Story 06.3 Deck",
            "themePreset": "night",
            "aspectRatio": "16:9",
            "deckMode": deck_mode,
            "initialSlides": initial_slides,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create deck failed: {body}");
    let deck_id = body["id"].as_str().unwrap().to_string();
    let slides = body["slides"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["slideUid"].as_str().unwrap().to_string(),
                s["contentSha256"].as_str().unwrap().to_string(),
                s["revision"].as_i64().unwrap(),
            )
        })
        .collect();
    (deck_id, slides)
}

#[tokio::test]
async fn it_01_auth_rbac_alias_anti_idor_and_preflush_cas() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    seed_project(&state, "proj-b", "tenant-beta", tmp.path());
    let viewer_tok = add_session(&state, "tenant-alpha", "viewer-1", "viewer");
    let foreign_tok = add_session(&state, "tenant-beta", "attacker-1", "editor");

    let (deck_id, slides) =
        create_deck_with_slides(&router, &editor_tok, "proj-a", "multi-page", 2).await;
    let (uid, sha, rev) = &slides[0];

    let dispatch_body = json!({
        "sessionId": "sess-1",
        "targetFfId": "ff-slide-headline",
        "prompt": "Refine headline",
        "expectedRevision": rev,
        "preFlushedSha256": sha,
        "snapshotReadOnly": false
    });

    // 401 without token
    let (st, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        None,
        &[],
        Some(dispatch_body.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // 403 viewer on dispatch
    let (st, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&viewer_tok),
        &[],
        Some(dispatch_body.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // 200 viewer on read-only charts/validate
    let (st, val_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/charts/validate"),
        Some(&viewer_tok),
        &[],
        Some(json!({
            "ffId": "ff-kpi-card",
            "spec": {
                "kind": "pie",
                "title": "Mix",
                "series": [{"name": "S1", "points": [{"label": "A", "value": 40}, {"label": "B", "value": 60}]}]
            }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "validate failed: {val_body}");
    assert_eq!(val_body["valid"], true);

    // Direct alias without X-Cowok-Project-Id or cross-tenant -> 404 E_DECK_NOT_FOUND
    let (st, err_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/slides/{uid}/copilot"),
        Some(&editor_tok),
        &[],
        Some(dispatch_body.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(err_body["error"]["code"], "E_DECK_NOT_FOUND");

    let (st, err_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/decks/{deck_id}/slides/{uid}/copilot"),
        Some(&foreign_tok),
        &[("x-cowok-project-id", "proj-a")],
        Some(dispatch_body.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(err_body["error"]["code"], "E_DECK_NOT_FOUND");

    // Pre-flush mismatch -> 409 E_PREFLUSH_CONFLICT
    let (st, stale_body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "Refine headline",
            "expectedRevision": rev + 9,
            "preFlushedSha256": sha,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(stale_body["error"]["code"], "E_PREFLUSH_CONFLICT");
}

#[tokio::test]
async fn it_02_parallel_cap4_fifo_lease_guard_idempotency_and_comment_cas() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    seed_project(&state, "proj-b", "tenant-beta", tmp.path());

    let (deck_id, slides) =
        create_deck_with_slides(&router, &editor_tok, "proj-a", "multi-page", 5).await;

    // Seed active comment on slide 0 + foreign comment on proj-b
    {
        let conn = state.registry.pool().connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO element_comments (id, tenant_id, project_id, node_id, element_ff_id, author, content, dom_hash, artifact_sha256, anchor_kind, anchor_status, created_at, updated_at)
             VALUES ('cmt-ok', 'tenant-alpha', 'proj-a', ?1, 'ff-slide-headline', 'designer-1', 'Tighten copy', 'domhash-1', ?2, 'html_element', 'active', ?3, ?3)",
            params![slides[0].0, slides[0].1, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO element_comments (id, tenant_id, project_id, node_id, element_ff_id, author, content, dom_hash, artifact_sha256, anchor_kind, anchor_status, created_at, updated_at)
             VALUES ('cmt-foreign', 'tenant-beta', 'proj-b', 'n-1', 'ff-slide-headline', 'other', 'x', 'domhash-x', 'shax', 'html_element', 'active', ?1, ?1)",
            params![now],
        )
        .unwrap();
    }

    // Foreign comment -> 404 E_COMMENT_NOT_FOUND
    let (st, err_b) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "Apply comment",
            "expectedRevision": slides[0].2,
            "preFlushedSha256": slides[0].1,
            "snapshotReadOnly": false,
            "structuredComment": {
                "commentId": "cmt-foreign",
                "slideUid": slides[0].0,
                "elementFfId": "ff-slide-headline",
                "domHash": "domhash-x",
                "artifactSha256": "shax",
                "commentBody": "x"
            }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(err_b["error"]["code"], "E_COMMENT_NOT_FOUND");

    // Stale comment CAS -> 409 E_COMMENT_ANCHOR_STALE
    let (st, err_stale) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "Apply comment",
            "expectedRevision": slides[0].2,
            "preFlushedSha256": slides[0].1,
            "snapshotReadOnly": false,
            "structuredComment": {
                "commentId": "cmt-ok",
                "slideUid": slides[0].0,
                "elementFfId": "ff-slide-headline",
                "domHash": "stale-domhash",
                "artifactSha256": slides[0].1,
                "commentBody": "Tighten copy"
            }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(err_stale["error"]["code"], "E_COMMENT_ANCHOR_STALE");

    // Dispatch slides 0..3 -> 4 distinct RUNNING slots + idempotency replay/mismatch
    let req0 = json!({
        "sessionId": "sess-1",
        "targetFfId": "ff-slide-headline",
        "prompt": "Tighten headline",
        "expectedRevision": slides[0].2,
        "preFlushedSha256": slides[0].1,
        "snapshotReadOnly": false,
        "structuredComment": {
            "commentId": "cmt-ok",
            "slideUid": slides[0].0,
            "elementFfId": "ff-slide-headline",
            "domHash": "domhash-1",
            "artifactSha256": slides[0].1,
            "commentBody": "Tighten copy"
        }
    });
    let (st, lease0) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[("idempotency-key", "idem-s0-1")],
        Some(req0.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "dispatch 0 failed: {lease0}");
    assert_eq!(lease0["status"], "RUNNING");
    assert_eq!(lease0["leaseTtlMs"], 60_000);
    assert!(lease0["leaseExpiresAtMs"].as_i64().is_some());

    // Idempotency replay returns exact lease with 0 new jobs
    let (st_rep, lease0_rep) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[("idempotency-key", "idem-s0-1")],
        Some(req0.clone()),
    )
    .await;
    assert_eq!(st_rep, StatusCode::CREATED);
    assert_eq!(lease0_rep["jobId"], lease0["jobId"]);

    // Idempotency payload mismatch -> 422 E_IDEMPOTENCY_PAYLOAD_MISMATCH
    let mut diff_req = req0.clone();
    diff_req["prompt"] = json!("Different prompt");
    let (st_mis, err_mis) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[("idempotency-key", "idem-s0-1")],
        Some(diff_req),
    )
    .await;
    assert_eq!(st_mis, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err_mis["error"]["code"], "E_IDEMPOTENCY_PAYLOAD_MISMATCH");

    // 2nd prompt on slide 0 (even with different sessionId) MUST queue FIFO!
    let (st_fifo, lease0_q) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-other",
            "targetFfId": "ff-slide-headline",
            "prompt": "Second turn on slide 0",
            "expectedRevision": slides[0].2,
            "preFlushedSha256": slides[0].1,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(st_fifo, StatusCode::CREATED);
    assert_eq!(lease0_q["status"], "QUEUED");
    assert_eq!(lease0_q["queuePosition"], 1);

    // Slides 1, 2, 3 run in parallel (cap 4)
    for (idx, slide) in slides.iter().enumerate().take(4).skip(1) {
        let (s, l) = send_req(
            &router,
            "POST",
            &format!(
                "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
                slide.0
            ),
            Some(&editor_tok),
            &[],
            Some(json!({
                "sessionId": "sess-1",
                "targetFfId": "ff-slide-headline",
                "prompt": format!("Parallel {idx}"),
                "expectedRevision": slide.2,
                "preFlushedSha256": slide.1,
                "snapshotReadOnly": false
            })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        assert_eq!(l["status"], "RUNNING");
    }

    // Slide 4 exceeds global cap 4 -> QUEUED
    let (s4, l4) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[4].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "5th slide",
            "expectedRevision": slides[4].2,
            "preFlushedSha256": slides[4].1,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(s4, StatusCode::CREATED);
    assert_eq!(l4["status"], "QUEUED");

    // Human CMS slots/batch on leased slide 0 -> 409 E_COPILOT_LEASE_HELD
    let (st_held, err_held) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/slots/batch", slides[0].0),
        Some(&editor_tok),
        &[],
        Some(json!({
            "expectedRevision": slides[0].2,
            "patches": [{ "ffId": "ff-slide-headline", "slotKind": "heading", "textValue": "Human Conflict" }]
        })),
    )
    .await;
    assert_eq!(st_held, StatusCode::CONFLICT);
    assert_eq!(err_held["error"]["code"], "E_COPILOT_LEASE_HELD");

    // Slide 4 (QUEUED, not RUNNING) still allows human slots/batch
    let (st_ok, ok_body) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/slots/batch",
            slides[4].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "expectedRevision": slides[4].2,
            "patches": [{ "ffId": "ff-kpi-1", "slotKind": "metric", "textValue": "Human OK" }]
        })),
    )
    .await;
    assert_eq!(
        st_ok,
        StatusCode::OK,
        "slide 4 human batch failed: {ok_body}"
    );

    // Complete job 0 -> updates wording_slots updated_by='copilot', resolves comment 'cmt-ok', promotes FIFO job
    let job0_id = lease0["jobId"].as_str().unwrap();
    let fence0 = lease0["fencingToken"].as_i64().unwrap();
    let outcome = commit_slide_copilot_turn(
        &state,
        "tenant-alpha",
        "proj-a",
        &deck_id,
        &slides[0].0,
        job0_id,
        fence0,
        slides[0].2,
        &[CopilotSlotMutation {
            ff_id: "ff-kpi-1".into(),
            slot_kind: "metric".into(),
            text_value: "Copilot Tightened Metric".into(),
            charts_json: None,
        }],
        Some("cmt-ok"),
    )
    .expect("commit copilot turn");
    assert_eq!(outcome.new_revision, slides[0].2 + 1);
    assert!(outcome.resolved_comment);

    let conn = state.registry.pool().connection().unwrap();
    let anchor_status: String = conn
        .query_row(
            "SELECT anchor_status FROM element_comments WHERE id = 'cmt-ok'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(anchor_status, "resolved");
    let updated_by: String = conn
        .query_row(
            "SELECT w.updated_by FROM wording_slots w JOIN deck_slides s ON s.id = w.slide_id WHERE s.slide_uid = ?1 AND w.ff_id = 'ff-kpi-1'",
            params![slides[0].0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(updated_by, "copilot");
}

#[tokio::test]
async fn it_03_charts_validate_and_invalid_chart_preserves_prior_dom_and_db() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, slides) =
        create_deck_with_slides(&router, &editor_tok, "proj-a", "multi-page", 1).await;
    let (uid, sha, rev) = &slides[0];

    // Invalid pie (94%) -> 422 E_CHART_SUM_NOT_100
    let (st, body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/charts/validate"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "ffId": "ff-kpi-card",
            "spec": {
                "kind": "pie",
                "title": "Share",
                "series": [{"name": "S", "points": [{"label": "A", "value": 44}, {"label": "B", "value": 50}]}]
            }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "E_CHART_SUM_NOT_100");

    // Negative proportional -> 422 E_CHART_NEGATIVE_PROPORTIONAL
    let (st, body) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/charts/validate"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "ffId": "ff-kpi-card",
            "spec": {
                "kind": "funnel",
                "title": "Funnel",
                "series": [{"name": "S", "points": [{"label": "A", "value": -5}, {"label": "B", "value": 105}]}]
            }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "E_CHART_NEGATIVE_PROPORTIONAL");

    // Signed cartesian bar succeeds
    let (st, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/charts/validate"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "ffId": "ff-kpi-card",
            "spec": {
                "kind": "bar",
                "title": "Net Variance",
                "series": [{"name": "S", "points": [{"label": "Q1", "value": -12}, {"label": "Q2", "value": 45}]}]
            }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // Copilot dispatch then invalid chart commit fails closed preserving prior slide sha/rev & releasing lease
    let (st_d, lease) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-kpi-card",
            "prompt": "Update chart",
            "expectedRevision": rev,
            "preFlushedSha256": sha,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(st_d, StatusCode::CREATED);

    let err = commit_slide_copilot_turn(
        &state,
        "tenant-alpha",
        "proj-a",
        &deck_id,
        uid,
        lease["jobId"].as_str().unwrap(),
        lease["fencingToken"].as_i64().unwrap(),
        *rev,
        &[CopilotSlotMutation {
            ff_id: "ff-kpi-card".into(),
            slot_kind: "chart_json".into(),
            text_value: "Bad Chart".into(),
            charts_json: Some(r#"{"kind":"pie","title":"Bad","series":[{"name":"S","points":[{"label":"A","value":94}]}]}"#.into()),
        }],
        None,
    )
    .unwrap_err();
    assert!(err.contains("E_CHART_SUM_NOT_100"), "got {err}");

    let conn = state.registry.pool().connection().unwrap();
    let (cur_rev, cur_sha): (i64, String) = conn
        .query_row(
            "SELECT revision, content_sha256 FROM deck_slides WHERE slide_uid = ?1",
            params![uid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(cur_rev, *rev);
    assert_eq!(cur_sha, *sha);
    let job_status: String = conn
        .query_row(
            "SELECT status FROM node_execution_jobs WHERE id = ?1",
            params![lease["jobId"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(job_status, "FAILED");
}

#[tokio::test]
async fn it_04_one_page_two_phase_commit_and_orphan_sweep() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, slides) =
        create_deck_with_slides(&router, &editor_tok, "proj-a", "one-page", 2).await;

    let (st_d, lease) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "Update one-page slide 0",
            "expectedRevision": slides[0].2,
            "preFlushedSha256": slides[0].1,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(st_d, StatusCode::CREATED);

    let outcome = commit_slide_copilot_turn(
        &state,
        "tenant-alpha",
        "proj-a",
        &deck_id,
        &slides[0].0,
        lease["jobId"].as_str().unwrap(),
        lease["fencingToken"].as_i64().unwrap(),
        slides[0].2,
        &[CopilotSlotMutation {
            ff_id: "ff-kpi-1".into(),
            slot_kind: "metric".into(),
            text_value: "One-Page Copilot Update".into(),
            charts_json: None,
        }],
        None,
    )
    .expect("one-page commit");
    assert_eq!(outcome.new_revision, slides[0].2 + 1);

    // Dispatch & commit a 2nd turn so `.r2.{uuid}.html` becomes superseded and swept by orphan cleaner
    let (st_d2, lease2) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/proj-a/decks/{deck_id}/slides/{}/copilot/dispatch",
            slides[0].0
        ),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-kpi-1",
            "prompt": "Second one-page update",
            "expectedRevision": outcome.new_revision,
            "preFlushedSha256": outcome.content_sha256,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(st_d2, StatusCode::CREATED);
    let outcome2 = commit_slide_copilot_turn(
        &state,
        "tenant-alpha",
        "proj-a",
        &deck_id,
        &slides[0].0,
        lease2["jobId"].as_str().unwrap(),
        lease2["fencingToken"].as_i64().unwrap(),
        outcome.new_revision,
        &[CopilotSlotMutation {
            ff_id: "ff-kpi-1".into(),
            slot_kind: "metric".into(),
            text_value: "One-Page Copilot Update 2".into(),
            charts_json: None,
        }],
        None,
    )
    .expect("one-page commit 2");

    let conn = state.registry.pool().connection().unwrap();
    let sibling_rev: i64 = conn
        .query_row(
            "SELECT revision FROM deck_slides WHERE slide_uid = ?1",
            params![slides[1].0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sibling_rev, outcome2.new_revision);
    let active = list_active_slide_html_paths(&conn).unwrap();
    let swept = sweep_orphan_slide_artifacts(&tmp.path().join("proj-a"), &active, 0, true);
    assert!(swept >= 1, "superseded artifact should be swept");
}

#[tokio::test]
async fn it_05_60s_lease_sweeper_expiry_and_cancel_cas_promotion() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, slides) =
        create_deck_with_slides(&router, &editor_tok, "proj-a", "multi-page", 1).await;
    let (uid, sha, rev) = &slides[0];

    let (s1, l1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "Job 1",
            "expectedRevision": rev,
            "preFlushedSha256": sha,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(s1, StatusCode::CREATED);
    assert_eq!(l1["status"], "RUNNING");

    let (s2, l2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "sess-1",
            "targetFfId": "ff-slide-headline",
            "prompt": "Job 2 FIFO",
            "expectedRevision": rev,
            "preFlushedSha256": sha,
            "snapshotReadOnly": false
        })),
    )
    .await;
    assert_eq!(s2, StatusCode::CREATED);
    assert_eq!(l2["status"], "QUEUED");

    // Force job 1 lease expiry past 60s TTL -> sweeper marks INTERRUPTED and promotes job 2 with 60s TTL
    {
        let conn = state.registry.pool().connection().unwrap();
        let expired_at = Utc::now().timestamp_millis() - 5_000;
        conn.execute(
            "UPDATE node_execution_jobs SET stale_after_timestamp = ?1 WHERE id = ?2",
            params![expired_at, l1["jobId"].as_str().unwrap()],
        )
        .unwrap();
    }
    let reaped = sweep_expired_leases(state.registry.pool(), &state.sse_tx)
        .await
        .expect("sweep");
    assert_eq!(reaped, 1);

    let (j1_st, j2_st, j2_fence, j2_stale): (String, String, i64, Option<i64>) = {
        let conn = state.registry.pool().connection().unwrap();
        let j1_st = conn
            .query_row(
                "SELECT status FROM node_execution_jobs WHERE id = ?1",
                params![l1["jobId"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let (j2_st, j2_fence, j2_stale) = conn
            .query_row(
                "SELECT status, fencing_token, stale_after_timestamp FROM node_execution_jobs WHERE id = ?1",
                params![l2["jobId"].as_str().unwrap()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        (j1_st, j2_st, j2_fence, j2_stale)
    };
    assert_eq!(j1_st, "INTERRUPTED");
    assert_eq!(j2_st, "RUNNING");
    assert!(j2_stale.unwrap() > Utc::now().timestamp_millis() + 45_000);

    // Stale fencing token cancel fails 409
    let (st_bad_cancel, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/cancel"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "jobId": l2["jobId"],
            "expectedFencingToken": j2_fence + 99,
            "reason": "stale"
        })),
    )
    .await;
    assert_eq!(st_bad_cancel, StatusCode::CONFLICT);

    // Valid cancel CAS transitions to CANCELLED and releases CMS lock
    let (st_cancel, canceled) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/cancel"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "jobId": l2["jobId"],
            "expectedFencingToken": j2_fence,
            "reason": "user_abort"
        })),
    )
    .await;
    assert_eq!(st_cancel, StatusCode::OK, "cancel failed: {canceled}");
    assert_eq!(canceled["status"], "CANCELLED");
    assert_eq!(canceled["targetFfId"], "ff-slide-headline");
    assert!(canceled["leaseExpiresAtMs"].is_null());

    // Now human CMS slots/batch succeeds on the unlocked slide!
    let (st_human, body_human) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots/batch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "expectedRevision": rev,
            "patches": [{ "ffId": "ff-kpi-1", "slotKind": "metric", "textValue": "Unlocked Human Edit" }]
        })),
    )
    .await;
    assert_eq!(
        st_human,
        StatusCode::OK,
        "unlocked human edit failed: {body_human}"
    );
}

#[tokio::test]
async fn it_06_production_copilot_worker_executes_and_rejects_invalid_charts() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, slides) =
        create_deck_with_slides(&router, &editor_tok, "proj-a", "multi-page", 1).await;
    let (uid, sha, rev) = (&slides[0].0, &slides[0].1, slides[0].2);

    // 1. Valid chart copilot dispatch executed asynchronously by production worker
    let (st, lease) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "worker-sess",
            "targetFfId": "ff-chart-1",
            "prompt": "Render donut chart mix",
            "expectedRevision": rev,
            "preFlushedSha256": sha,
            "requestedChartKind": "donut"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "dispatch failed: {lease}");

    let processed = run_pending_slide_copilot_jobs(&state).await;
    assert_eq!(processed, 1);

    let (st_snap, snap_after) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&editor_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_snap, StatusCode::OK);
    assert_eq!(snap_after["revision"], rev + 1);
    let updated_sha = snap_after["contentSha256"].as_str().unwrap();

    // 2. Invalid proportional chart trigger marks job FAILED and emits targetFfId on chart.validation.rejected
    let (st2, lease2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        &[],
        Some(json!({
            "sessionId": "worker-sess",
            "targetFfId": "ff-chart-1",
            "prompt": "pie distribution A 45 and B 45",
            "expectedRevision": rev + 1,
            "preFlushedSha256": updated_sha,
            "requestedChartKind": "pie"
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::CREATED);

    let processed2 = run_pending_slide_copilot_jobs(&state).await;
    assert_eq!(processed2, 0);

    let conn = state.registry.pool().connection().unwrap();
    let (ev_kind, ev_payload): (String, String) = conn
        .query_row(
            "SELECT event_kind, payload_text FROM execution_events WHERE job_id = ?1 ORDER BY seq_no DESC LIMIT 1",
            params![lease2["jobId"].as_str().unwrap()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ev_kind, "chart.validation.rejected");
    let ev_json: serde_json::Value = serde_json::from_str(&ev_payload).unwrap();
    assert_eq!(ev_json["code"], "E_CHART_SUM_NOT_100");
    assert_eq!(ev_json["targetFfId"], "ff-chart-1");
}
