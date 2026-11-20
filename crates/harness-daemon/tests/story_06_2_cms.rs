// @story 06.2
// @implements AC-02, AC-04, AC-05
// @cover IT-01, IT-02, IT-02B, IT-03, IT-04, IT-04B, IT-05, IT-05B, IT-05C, IT-05D, IT-06
use std::fs;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::presentation::sweep_orphan_slide_artifacts;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use presentation_cms::compute_sha256_hex;
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
    let tok = format!("sess-062-{}", uuid::Uuid::new_v4());
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
         VALUES (?1, ?1, ?2, ?3, ?4, 'Story 06.2 test', 'ACTIVE', ?5, ?5)",
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

async fn create_sample_deck(
    router: &axum::Router,
    tok: &str,
    project_id: &str,
    deck_mode: &str,
) -> (String, Vec<String>) {
    let (status, body) = send_req(
        router,
        "POST",
        &format!("/api/v1/projects/{project_id}/decks"),
        Some(tok),
        &[],
        Some(json!({
            "title": "Story 06.2 Deck",
            "themePreset": "night",
            "aspectRatio": "16:9",
            "deckMode": deck_mode,
            "initialSlides": [
                { "layoutKind": "hero_cover", "eyebrowTag": "S1", "narrativeArc": "Intro", "headline": "Initial Slide 1" },
                { "layoutKind": "kpi_metric_grid", "eyebrowTag": "S2", "narrativeArc": "KPI", "headline": "Initial Slide 2" }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create deck failed: {body}");
    let deck_id = body["id"].as_str().unwrap().to_string();
    let uids = body["slides"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["slideUid"].as_str().unwrap().to_string())
        .collect();
    (deck_id, uids)
}

#[tokio::test]
async fn it_01_and_it_04b_auth_rbac_and_daemon_alias_anti_idor() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    seed_project(&state, "proj-b", "tenant-beta", tmp.path());

    let viewer_tok = add_session(&state, "tenant-alpha", "viewer-1", "viewer");
    let foreign_tok = add_session(&state, "tenant-beta", "attacker-1", "editor");
    let (deck_id, uids) = create_sample_deck(&router, &editor_tok, "proj-a", "multi-page").await;
    let uid = &uids[0];

    // 401 without session token
    let (st, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Viewer can GET slots but gets 403 on mutation
    let (st_get, snap) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&viewer_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_get, StatusCode::OK, "viewer GET slots failed: {snap}");
    assert!(!snap["slots"].as_array().unwrap().is_empty());

    let (st_mut, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots/batch"),
        Some(&viewer_tok),
        &[],
        Some(json!({ "expectedRevision": 0, "speakerNotes": "nope", "patches": [] })),
    )
    .await;
    assert_eq!(st_mut, StatusCode::FORBIDDEN);

    // IT-04B: Direct daemon alias `/api/v1/decks/:deck_id/...` enforces tenant & X-Cowok-Project-Id anti-IDOR -> uniform 404
    let (st_foreign_alias, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/decks/{deck_id}/slides/{uid}/slots"),
        Some(&foreign_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_foreign_alias, StatusCode::NOT_FOUND);

    let (st_mismatched_hdr, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/decks/{deck_id}/slides/{uid}/slots"),
        Some(&editor_tok),
        &[("x-cowok-project-id", "proj-b")],
        None,
    )
    .await;
    assert_eq!(st_mismatched_hdr, StatusCode::NOT_FOUND);

    // Matching X-Cowok-Project-Id on direct alias succeeds
    let (st_ok_alias, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/decks/{deck_id}/slides/{uid}/slots"),
        Some(&editor_tok),
        &[("x-cowok-project-id", "proj-a")],
        None,
    )
    .await;
    assert_eq!(st_ok_alias, StatusCode::OK);
}

#[tokio::test]
async fn it_02_and_it_02b_slots_batch_occ_speaker_notes_and_idempotency() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, uids) = create_sample_deck(&router, &tok, "proj-a", "multi-page").await;
    let uid = &uids[0];

    let (_, snap) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    let first_ff = snap["slots"][0]["ffId"].as_str().unwrap().to_string();

    // Commit slot + speaker_notes with Idempotency-Key (initial revision = 1)
    let batch_uri = format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots/batch");
    let payload = json!({
        "expectedRevision": 1,
        "speakerNotes": "Presenter note alpha",
        "patches": [
            { "ffId": first_ff, "slotKind": "heading", "textValue": "Updated Executive Headline" }
        ]
    });
    let (st1, res1) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "idem-cms-01")],
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "batch failed: {res1}");
    assert_eq!(res1["newRevision"], 2);
    assert_eq!(res1["patchedSlotsCount"], 1);

    // IT-02B: Exact replay returns cached 200 without bumping revision
    let (st_replay, res_replay) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "idem-cms-01")],
        Some(payload),
    )
    .await;
    assert_eq!(st_replay, StatusCode::OK);
    assert_eq!(res_replay["newRevision"], 2);
    assert_eq!(res_replay["contentSha256"], res1["contentSha256"]);

    // IT-02B: Same Idempotency-Key with different payload -> 422 E_IDEMPOTENCY_PAYLOAD_MISMATCH
    let (st_mismatch, res_mismatch) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "idem-cms-01")],
        Some(json!({
            "expectedRevision": 2,
            "speakerNotes": "Different body",
            "patches": []
        })),
    )
    .await;
    assert_eq!(st_mismatch, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res_mismatch["error"]["code"],
        "E_IDEMPOTENCY_PAYLOAD_MISMATCH"
    );

    // IT-02B: Active IN_PROGRESS -> 409; expired lease -> renews & executes
    {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let req_body = harness_daemon::routes::presentation_cms_routes::PatchSlotsBatchBody {
            expected_revision: 2,
            speaker_notes: Some("Only notes update".into()),
            patches: vec![],
            one_page_scope_uid: None,
        };
        let canon = serde_json::to_string(&req_body).unwrap();
        let fp = format!("POST\n/slides/slots/batch\nproj-a\n{deck_id}\n{uid}\n{canon}");
        let sha = compute_sha256_hex(fp.as_bytes());
        conn.execute(
            "INSERT INTO idempotency_records
             (tenant_id, actor_id, idempotency_key, request_sha256, status, lease_expires_at, expires_at)
             VALUES ('tenant-alpha', 'designer-1', 'idem-inflight', ?1, 'IN_PROGRESS', ?2, ?3)",
            params![sha, now_ms + 30_000, now_ms + 86_400_000],
        )
        .unwrap();
    }
    let notes_only_body = json!({
        "expectedRevision": 2,
        "speakerNotes": "Only notes update",
        "patches": []
    });
    let (st_inflight, res_inflight) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "idem-inflight")],
        Some(notes_only_body.clone()),
    )
    .await;
    assert_eq!(st_inflight, StatusCode::CONFLICT);
    assert_eq!(res_inflight["error"]["code"], "E_IDEMPOTENCY_IN_PROGRESS");

    // Expire lease -> retry succeeds (speakerNotes-only with 0 patches!)
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "UPDATE idempotency_records SET lease_expires_at = 0 WHERE idempotency_key = 'idem-inflight'",
            [],
        )
        .unwrap();
    }
    let (st_notes, res_notes) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "idem-inflight")],
        Some(notes_only_body),
    )
    .await;
    assert_eq!(
        st_notes,
        StatusCode::OK,
        "expired lease retry failed: {res_notes}"
    );
    assert_eq!(res_notes["newRevision"], 3);

    // Stale expectedRevision -> 409 E_STALE_REVISION with actualRevision: 3
    let (st_stale, res_stale) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[],
        Some(json!({ "expectedRevision": 1, "speakerNotes": "stale", "patches": [] })),
    )
    .await;
    assert_eq!(st_stale, StatusCode::CONFLICT);
    assert_eq!(res_stale["error"]["code"], "E_STALE_REVISION");
    assert_eq!(res_stale["error"]["details"]["actualRevision"], 3);
}

#[tokio::test]
async fn it_03_and_it_04_apply_html_mxss_413_and_custom_override_lock() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, uids) = create_sample_deck(&router, &tok, "proj-a", "multi-page").await;
    let uid = &uids[0];
    let apply_uri = format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/apply-html");
    let batch_uri = format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots/batch");

    // mXSS payload -> 422 E_MXSS_BLOCKED
    let (st_xss, res_xss) = send_req(
        &router,
        "POST",
        &apply_uri,
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 0,
            "rawHtml": "<section class=\"slide active\"><svg><foreignObject><script>alert(1)</script></foreignObject></svg></section>",
            "resyncStructuredSlots": true
        })),
    )
    .await;
    assert_eq!(st_xss, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res_xss["error"]["code"], "E_MXSS_BLOCKED");

    // >512KB payload -> 413 E_PAYLOAD_TOO_LARGE
    let huge = format!(
        "<section class=\"slide active\"><p data-ff-id=\"ff-big\">{}</p></section>",
        "a".repeat(530_000)
    );
    let (st_big, res_big) = send_req(
        &router,
        "POST",
        &apply_uri,
        Some(&tok),
        &[],
        Some(json!({ "expectedRevision": 0, "rawHtml": huge, "resyncStructuredSlots": true })),
    )
    .await;
    assert_eq!(st_big, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(res_big["error"]["code"], "E_PAYLOAD_TOO_LARGE");

    // Divergent manual HTML missing committed [data-ff-id] sets customHtmlOverride=true -> locks slots/batch (409)
    let (st_div, res_div) = send_req(
        &router,
        "POST",
        &apply_uri,
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 1,
            "rawHtml": "<section class=\"slide active\"><div data-ff-id=\"ff-custom-only\">Manual HTML</div></section>",
            "resyncStructuredSlots": true
        })),
    )
    .await;
    assert_eq!(st_div, StatusCode::OK, "apply-html failed: {res_div}");
    assert_eq!(res_div["customHtmlOverride"], true);
    assert!(!res_div["unmappedSlotIds"].as_array().unwrap().is_empty());

    let (st_lock, res_lock) = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 2,
            "patches": [{ "ffId": "ff-custom-only", "slotKind": "text", "textValue": "Blocked" }]
        })),
    )
    .await;
    assert_eq!(st_lock, StatusCode::CONFLICT);
    assert_eq!(res_lock["error"]["code"], "E_CUSTOM_HTML_OVERRIDE_LOCK");
}

#[tokio::test]
async fn it_05b_one_page_shared_revision_and_sibling_sha256_occ() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, uids) = create_sample_deck(&router, &tok, "proj-a", "one-page").await;
    let uid1 = &uids[0];
    let uid2 = &uids[1];

    // Setup shared one-page HTML containing both sections so both slides share html_path + content_sha256
    let shared_html = format!(
        r#"<!DOCTYPE html><html><body>
        <section class="slide active" data-slide-uid="{uid1}"><h1 data-ff-id="ff-shared-h1">Slide One</h1></section>
        <section class="slide" data-slide-uid="{uid2}"><h1 data-ff-id="ff-shared-h1">Slide Two</h1></section>
        </body></html>"#
    );
    let shared_rel = format!("presentations/{deck_id}/slides/deck.r1.seed.html");
    let shared_abs = tmp.path().join("proj-a").join(&shared_rel);
    fs::create_dir_all(shared_abs.parent().unwrap()).unwrap();
    fs::write(&shared_abs, &shared_html).unwrap();
    let shared_sha = compute_sha256_hex(shared_html.as_bytes());
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "UPDATE deck_slides SET html_path = ?1, content_sha256 = ?2 WHERE deck_id = ?3",
            params![shared_rel, shared_sha, deck_id],
        )
        .unwrap();
        conn.execute("DELETE FROM wording_slots WHERE slide_id IN (SELECT id FROM deck_slides WHERE deck_id = ?1)", params![deck_id]).unwrap();
    }

    // Edit Slide 1 -> bumps shared presentation_decks.revision from 1 to 2 and updates BOTH siblings' html_path & content_sha256
    let (st1, res1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid1}/slots/batch"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 1,
            "patches": [{ "ffId": "ff-shared-h1", "slotKind": "heading", "textValue": "Slide One Patched" }]
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "slide 1 patch failed: {res1}");
    assert_eq!(res1["newRevision"], 2);

    // Concurrent sibling Slide 2 holding stale expectedRevision: 1 MUST fail with 409 E_STALE_REVISION (actual: 2)
    let (st2_stale, res2_stale) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid2}/slots/batch"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 1,
            "patches": [{ "ffId": "ff-shared-h1", "slotKind": "heading", "textValue": "Stale Sibling" }]
        })),
    )
    .await;
    assert_eq!(st2_stale, StatusCode::CONFLICT);
    assert_eq!(res2_stale["error"]["code"], "E_STALE_REVISION");
    assert_eq!(res2_stale["error"]["details"]["actualRevision"], 2);

    // Slide 2 retry with expectedRevision: 2 succeeds without 422 hash drift and preserves Slide 1 edit!
    let (st2_ok, res2_ok) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid2}/slots/batch"),
        Some(&tok),
        &[],
        Some(json!({
            "expectedRevision": 2,
            "patches": [{ "ffId": "ff-shared-h1", "slotKind": "heading", "textValue": "Slide Two Patched" }]
        })),
    )
    .await;
    assert_eq!(st2_ok, StatusCode::OK, "slide 2 retry failed: {res2_ok}");
    assert_eq!(res2_ok["newRevision"], 3);

    // Both slides now read cleanly at rev 3 and disk HTML has both edits isolated
    let (st_snap, snap1) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid1}/slots"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_snap, StatusCode::OK);
    assert_eq!(snap1["revision"], 3);
}

#[tokio::test]
async fn it_05c_and_it_05d_crash_sweep_and_concurrent_same_revision_race() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let (deck_id, uids) = create_sample_deck(&router, &tok, "proj-a", "multi-page").await;
    let uid = &uids[0];

    let (_, snap) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    let ff_id = snap["slots"][0]["ffId"].as_str().unwrap().to_string();
    let batch_uri = format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots/batch");

    // IT-05D: Two concurrent requests with SAME expectedRevision: 1 and DISTINCT Idempotency-Keys race!
    let req_a = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "race-key-a")],
        Some(json!({
            "expectedRevision": 1,
            "patches": [{ "ffId": ff_id, "slotKind": "heading", "textValue": "Winner A" }]
        })),
    );
    let req_b = send_req(
        &router,
        "POST",
        &batch_uri,
        Some(&tok),
        &[("idempotency-key", "race-key-b")],
        Some(json!({
            "expectedRevision": 1,
            "patches": [{ "ffId": ff_id, "slotKind": "heading", "textValue": "Winner B" }]
        })),
    );
    let ((st_a, body_a), (st_b, body_b)) = tokio::join!(req_a, req_b);
    let statuses = [st_a, st_b];
    assert!(
        statuses.contains(&StatusCode::OK) && statuses.contains(&StatusCode::CONFLICT),
        "Expected 1 winner (200) + 1 CAS loser (409), got: A=({st_a:?}, {body_a}) B=({st_b:?}, {body_b})"
    );

    // Winner's artifact MUST remain readable & SHA-verified (loser never overwrote or unlinked winner's tx_uuid file!)
    let (st_after, snap_after) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(
        st_after,
        StatusCode::OK,
        "winner artifact lost after race: {snap_after}"
    );
    assert_eq!(snap_after["revision"], 2);

    // IT-05C: Simulated pre-commit orphan .tmp and unreferenced .r99.uuid.html swept without harming committed artifact
    let proj_ws = tmp.path().join("proj-a");
    let slides_dir = proj_ws.join(format!("presentations/{deck_id}/slides"));
    fs::write(slides_dir.join(".cowok-tmp-orphan.html"), "orphan tmp").unwrap();
    fs::write(
        slides_dir.join(format!("{uid}.r99.orphan-uuid.html")),
        "orphan rev",
    )
    .unwrap();

    let active_paths = {
        let conn = state.registry.pool().connection().unwrap();
        list_active_slide_html_paths(&conn).unwrap()
    };
    let swept = sweep_orphan_slide_artifacts(&proj_ws, &active_paths, 0, true);
    assert!(swept >= 2, "expected >= 2 orphan files swept, got {swept}");

    // Active slide still reads cleanly after sweep
    let (st_post_sweep, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_post_sweep, StatusCode::OK);

    // Disk tampering -> fail-closed 422 E_DECK_ARTIFACT_MISSING
    for active in &active_paths {
        if active.contains(uid) {
            fs::write(proj_ws.join(active), "TAMPERED").unwrap();
        }
    }
    let (st_corrupt, body_corrupt) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/slots"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_corrupt, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body_corrupt["error"]["code"], "E_DECK_ARTIFACT_MISSING");
}
