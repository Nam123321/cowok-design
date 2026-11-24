// @story 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::fs;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::presentation_copilot::run_pending_slide_copilot_jobs;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use presentation_cms::{
    compute_sha256_hex, count_words, is_valid_slide_uid, MAX_SLIDE_HTML_BYTES,
    STAGESHOW_MAX_WORDS_PER_SLIDE,
};
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
    let tok = format!("sess-065-{}", uuid::Uuid::new_v4());
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
         VALUES (?1, ?1, ?2, ?3, ?4, 'Story 06.5 test', 'ACTIVE', ?5, ?5)",
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

#[tokio::test]
async fn it_01_retires_copilot_echo_stub_and_synthetic_q1_q3() {
    // Static zero-stub guarantee on presentation_copilot.rs
    let src = include_str!("../src/routes/presentation_copilot.rs");
    assert!(
        !src.contains("prompt.chars().take(160)"),
        "Echo stub prompt.chars().take(160) must be completely retired"
    );
    assert!(
        !src.contains("\"label\": \"Q1\"") && !src.contains("\"value\": 34.0"),
        "Synthetic Q1..Q3 chart stub must be completely retired"
    );

    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    let (st_create, deck_body) = send_req(
        &router,
        "POST",
        "/api/v1/projects/proj-a/decks",
        Some(&editor_tok),
        Some(json!({
            "title": "Copilot Verification Deck",
            "themePreset": "night",
            "aspectRatio": "16:9",
            "deckMode": "multi-page",
            "initialSlides": [{
                "layoutKind": "hero_cover",
                "eyebrowTag": "INTRO",
                "narrativeArc": "Opening",
                "headline": "Legacy Headline"
            }]
        })),
    )
    .await;
    assert_eq!(st_create, StatusCode::CREATED, "create failed: {deck_body}");
    let deck_id = deck_body["id"].as_str().unwrap();
    let slide = &deck_body["slides"][0];
    let uid = slide["slideUid"].as_str().unwrap();
    let sha = slide["contentSha256"].as_str().unwrap();
    let rev = slide["revision"].as_i64().unwrap();

    let instruction_prompt = "Rewrite: Accelerate enterprise retention across core workflows";
    let (st_disp, lease) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/proj-a/decks/{deck_id}/slides/{uid}/copilot/dispatch"),
        Some(&editor_tok),
        Some(json!({
            "sessionId": "sess-065-1",
            "targetFfId": "ff-hero_cover-headline",
            "prompt": instruction_prompt,
            "expectedRevision": rev,
            "preFlushedSha256": sha
        })),
    )
    .await;
    assert_eq!(st_disp, StatusCode::CREATED, "dispatch failed: {lease}");

    let processed = run_pending_slide_copilot_jobs(&state).await;
    assert_eq!(processed, 1);

    let conn = state.registry.pool().connection().unwrap();
    let committed_text: String = conn
        .query_row(
            "SELECT w.text_value
             FROM wording_slots w
             JOIN deck_slides s ON s.id = w.slide_id
             WHERE s.slide_uid = ?1 AND w.ff_id = 'ff-hero_cover-headline'",
            params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(
        committed_text, instruction_prompt,
        "Committed text must not echo raw instruction prompt verbatim"
    );
    assert!(
        count_words(&committed_text) <= STAGESHOW_MAX_WORDS_PER_SLIDE,
        "Committed text exceeded 30-word stageshow budget: {committed_text}"
    );
}

#[tokio::test]
async fn it_02_normalizes_csv_tsv_pdf_cas_and_rejects_malformed_or_scanned_sources() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    // 1. US + EU/VN locale CSV -> Pie chart with largest-remainder sum 100.00 ± 0.05 & preserved raw units
    let locale_csv =
        "Channel,Revenue\nDirect,\"$1,234.56\"\nEMEA,\"1.234,56 €\"\nAPAC,\"42,5M VND\"\n";
    let (st, norm_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineContent": locale_csv,
            "sourceFormat": "csv",
            "title": "Global Revenue Share",
            "chartKind": "pie"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "csv normalize failed: {norm_body}");
    let prop_sum = norm_body["proportionalSum"].as_f64().unwrap();
    assert!((prop_sum - 100.0).abs() <= 0.05, "prop_sum={prop_sum}");
    let pts = norm_body["spec"]["series"][0]["points"].as_array().unwrap();
    assert!(pts[0]["label"].as_str().unwrap().contains("$1,234.56"));
    assert!(pts[1]["label"].as_str().unwrap().contains("1.234,56 €"));
    assert!(pts[2]["label"].as_str().unwrap().contains("42,5M VND"));

    // 2. CAS SHA-256 TSV attachment -> Bar chart
    let tsv_bytes = b"Stage\tConversion\nVisitor\t1200\nSignup\t480\nPaid\t165\n";
    let tsv_sha = compute_sha256_hex(tsv_bytes);
    let cas_dir = tmp.path().join(".cowok-design/cas");
    fs::create_dir_all(&cas_dir).unwrap();
    fs::write(cas_dir.join(&tsv_sha), tsv_bytes).unwrap();
    {
        let conn = state.registry.pool().connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO cas_blobs (sha256, tenant_id, byte_size, mime_type, file_ext, storage_relpath, ref_count, gc_lease_until, created_at)
             VALUES (?1, 'tenant-alpha', ?2, 'text/tab-separated-values', 'tsv', ?3, 1, 0, ?4)",
            params![tsv_sha, tsv_bytes.len() as i64, format!(".cowok-design/cas/{tsv_sha}"), now],
        )
        .unwrap();
    }
    let (st_cas, cas_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "casSha256": tsv_sha,
            "chartKind": "funnel"
        })),
    )
    .await;
    assert_eq!(st_cas, StatusCode::OK, "cas tsv failed: {cas_body}");
    assert!((cas_body["proportionalSum"].as_f64().unwrap() - 100.0).abs() <= 0.05);

    // 3. Text-extractable PDF table -> Donut chart
    let text_pdf = b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj\n2 0 obj << /Length 120 >> stream\nBT\n(Plan) Tj (ARR) Tj T*\n(Enterprise) Tj (1500) Tj T*\n(Team) Tj (900) Tj T*\n(Starter) Tj (600) Tj T*\nET\nendstream endobj\n%%EOF";
    let (st_pdf, pdf_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineBase64": BASE64_STANDARD.encode(text_pdf),
            "sourceFormat": "pdf",
            "chartKind": "donut"
        })),
    )
    .await;
    assert_eq!(st_pdf, StatusCode::OK, "text pdf failed: {pdf_body}");
    assert_eq!(pdf_body["rowCount"], 3);

    // 4. Scanned / image-only PDF -> 422 SCANNED_PDF_NO_TEXT_TABLES with non-tech guidance
    let scanned_pdf = b"%PDF-1.4\n1 0 obj << /Type /Page /Resources << /XObject << /Im0 2 0 R >> >> >> endobj\n2 0 obj << /Subtype /Image >> endobj\n%%EOF";
    let (st_scan, scan_err) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineBase64": BASE64_STANDARD.encode(scanned_pdf),
            "sourceFormat": "pdf",
            "chartKind": "bar"
        })),
    )
    .await;
    assert_eq!(st_scan, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(scan_err["error"]["code"], "SCANNED_PDF_NO_TEXT_TABLES");
    assert!(scan_err["error"]["guidance"]
        .as_str()
        .unwrap_or("")
        .contains("CSV"));

    // 5. Edge cases: NaN, negative proportional, sum==0, >12 series, >200 points
    let (st_nan, nan_err) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineContent": "K,V\nA,NaN\nB,10\n",
            "chartKind": "bar"
        })),
    )
    .await;
    assert_eq!(st_nan, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(nan_err["error"]["code"], "E_CHART_NON_FINITE");

    let (st_neg, neg_err) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineContent": "K,V\nA,-10\nB,50\n",
            "chartKind": "pie"
        })),
    )
    .await;
    assert_eq!(st_neg, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(neg_err["error"]["code"], "E_CHART_NEGATIVE_PROPORTIONAL");

    let (st_zero, zero_err) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineContent": "K,V\nA,0\nB,0\n",
            "chartKind": "donut"
        })),
    )
    .await;
    assert_eq!(st_zero, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(zero_err["error"]["code"], "E_CHART_SUM_NON_POSITIVE");

    let mut too_many_series = String::from("Cat");
    for i in 1..=13 {
        too_many_series.push_str(&format!(",S{i}"));
    }
    too_many_series.push_str("\nR1");
    for _ in 1..=13 {
        too_many_series.push_str(",10");
    }
    let (st_ms, ms_err) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineContent": too_many_series,
            "chartKind": "bar"
        })),
    )
    .await;
    assert_eq!(st_ms, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(ms_err["error"]["code"], "E_CHART_MAX_SERIES_EXCEEDED");

    let mut too_many_points = String::from("Cat,Val\n");
    for i in 0..201 {
        too_many_points.push_str(&format!("P{i},1\n"));
    }
    let (st_mp, mp_err) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineContent": too_many_points,
            "chartKind": "line"
        })),
    )
    .await;
    assert_eq!(st_mp, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(mp_err["error"]["code"], "E_CHART_MAX_POINTS_EXCEEDED");
}

#[tokio::test]
async fn it_03_generates_stageshow_deck_with_frames_auto_split_and_v15_persistence() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());
    let foreign_tok = add_session(&state, "tenant-beta", "attacker", "editor");

    let long_brief = "Unified AI design studio streamlines quarterly reviews across engineering and product leadership. \
        Enterprise adoption grew strongly across North America and Southeast Asia throughout the year. \
        Automated validation eliminated manual spreadsheet formatting drift before executive presentations. \
        Every slide preserves strict thirty-word density budgets with zero visual frame overflow. \
        Atomic SQLite staging protects concurrent human edits and copilot updates without stale overwrites. \
        Next quarter expands interactive HTML exports and automated accessibility verification across portals.";

    for frame in ["Stage", "Window", "Phone"] {
        let (st, gen_body) = send_req(
            &router,
            "POST",
            "/api/v1/decks/generate",
            Some(&editor_tok),
            Some(json!({
                "projectId": "proj-a",
                "title": format!("Executive Deck ({frame})"),
                "brief": long_brief,
                "framePreset": frame,
                "themePreset": "night",
                "aspectRatio": "16:9",
                "useByok": false,
                "chartSource": {
                    "inlineContent": "Region,ARR\nUS,\"$1,200.50\"\nEU,\"800,50 €\"\nVN,\"500M VND\"\n",
                    "sourceFormat": "csv",
                    "chartKind": "donut"
                }
            })),
        )
        .await;
        assert_eq!(
            st,
            StatusCode::CREATED,
            "generate ({frame}) failed: {gen_body}"
        );
        assert_eq!(gen_body["framePreset"], frame);
        assert!(
            gen_body["autoSplitCount"].as_u64().unwrap() >= 1,
            "expected auto-split for >30 word brief"
        );
        assert!(
            gen_body["maxWordsPerSlide"].as_u64().unwrap() <= STAGESHOW_MAX_WORDS_PER_SLIDE as u64
        );

        let slides = gen_body["deck"]["slides"].as_array().unwrap();
        assert!(
            slides.len() >= 4,
            "expected cover + split insights + chart slide"
        );

        let proj_ws = tmp.path().join("proj-a");
        for s in slides {
            let uid = s["slideUid"].as_str().unwrap();
            assert!(is_valid_slide_uid(uid), "invalid ULID: {uid}");
            let rel = s["htmlPath"].as_str().unwrap();
            let html = fs::read_to_string(proj_ws.join(rel)).expect("staged html exists");
            assert!(html.len() <= MAX_SLIDE_HTML_BYTES);
            assert_eq!(
                compute_sha256_hex(html.as_bytes()),
                s["contentSha256"].as_str().unwrap()
            );
            assert!(html.contains(&format!("data-stageshow-frame=\"{frame}\"")));
        }
    }

    // Anti-IDOR check for foreign tenant
    let (st_idor, err_idor) = send_req(
        &router,
        "POST",
        "/api/v1/decks/generate",
        Some(&foreign_tok),
        Some(json!({
            "projectId": "proj-a",
            "title": "Unauthorized",
            "brief": "Blocked cross tenant brief"
        })),
    )
    .await;
    assert_eq!(st_idor, StatusCode::NOT_FOUND);
    assert_eq!(err_idor["error"]["code"], "E_DECK_NOT_FOUND");
}

#[tokio::test]
async fn it_04_copilot_rewrite_endpoint_enforces_budget_and_splits_atomically() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    let (st_gen, gen_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/generate",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "title": "Rewrite Flow Deck",
            "brief": "Core launch metrics confirm steady growth.",
            "framePreset": "Window",
            "useByok": false
        })),
    )
    .await;
    assert_eq!(st_gen, StatusCode::CREATED, "generate failed: {gen_body}");
    let deck_id = gen_body["deck"]["id"].as_str().unwrap();
    let slide0 = &gen_body["deck"]["slides"][0];
    let slide_uid = slide0["slideUid"].as_str().unwrap();
    let rev = slide0["revision"].as_i64().unwrap();
    let initial_count = gen_body["deck"]["slidesCount"].as_i64().unwrap();

    let overflow_prompt = "Rewrite: Platform telemetry demonstrates sustained enterprise growth across forty markets. \
        Customer onboarding speed improved dramatically after unified design tokens launched globally. \
        Automated security scanners verified zero cross-tenant data leakage across every workspace endpoint. \
        Interactive browser playback maintained sixty frames per second under peak presentation load. \
        Executive stakeholders approved full phase two rollout across all regional hubs.";

    let (st_rw, rw_body) = send_req(
        &router,
        "POST",
        "/api/v1/slides/copilot/rewrite",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "deckId": deck_id,
            "slideUid": slide_uid,
            "targetFfId": "ff-s1-b1",
            "prompt": overflow_prompt,
            "expectedRevision": rev,
            "framePreset": "Window",
            "autoSplitOnOverflow": true
        })),
    )
    .await;
    assert_eq!(st_rw, StatusCode::OK, "rewrite failed: {rw_body}");
    assert!(rw_body["wordCount"].as_u64().unwrap() <= STAGESHOW_MAX_WORDS_PER_SLIDE as u64);
    let conts = rw_body["continuationSlides"].as_array().unwrap();
    assert!(
        !conts.is_empty(),
        "expected atomic continuation slide on >30 word overflow"
    );
    for c in conts {
        assert!(is_valid_slide_uid(c["slideUid"].as_str().unwrap()));
        assert!(c["wordCount"].as_u64().unwrap() <= STAGESHOW_MAX_WORDS_PER_SLIDE as u64);
    }

    let conn = state.registry.pool().connection().unwrap();
    let updated_count: i64 = conn
        .query_row(
            "SELECT slides_count FROM presentation_decks WHERE id = ?1",
            params![deck_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(updated_count, initial_count + (conts.len() as i64));
}

#[tokio::test]
async fn it_05_one_page_deck_generation_and_rewrite_preserves_sibling_slides() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    let (st_gen, gen_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/generate",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "title": "One Page Narrative",
            "brief": "Enterprise adoption expanded across three regions. Automated QA cut regressions by half.",
            "framePreset": "Stage",
            "deckMode": "one-page",
            "useByok": false
        })),
    )
    .await;
    assert_eq!(
        st_gen,
        StatusCode::CREATED,
        "one-page gen failed: {gen_body}"
    );
    let deck_id = gen_body["deck"]["id"].as_str().unwrap();
    let slides = gen_body["deck"]["slides"].as_array().unwrap();
    assert!(slides.len() >= 2, "expected >= 2 slides in one-page deck");

    let shared_path = slides[0]["htmlPath"].as_str().unwrap();
    let shared_sha = slides[0]["contentSha256"].as_str().unwrap();
    for s in slides {
        assert_eq!(s["htmlPath"].as_str().unwrap(), shared_path);
        assert_eq!(s["contentSha256"].as_str().unwrap(), shared_sha);
    }

    let proj_ws = tmp.path().join("proj-a");
    let initial_html = fs::read_to_string(proj_ws.join(shared_path)).expect("one-page html");
    for s in slides {
        let uid = s["slideUid"].as_str().unwrap();
        assert!(
            initial_html.contains(&format!("data-slide-uid=\"{uid}\"")),
            "initial one-page HTML missing slide {uid}"
        );
    }

    // Rewrite slide 0 in one-page mode -> must preserve sibling slide sections!
    let slide0_uid = slides[0]["slideUid"].as_str().unwrap();
    let slide1_uid = slides[1]["slideUid"].as_str().unwrap();
    let (st_rw, rw_body) = send_req(
        &router,
        "POST",
        "/api/v1/slides/copilot/rewrite",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "deckId": deck_id,
            "slideUid": slide0_uid,
            "targetFfId": "ff-s1-headline",
            "prompt": "Rewrite: Unified Q4 Growth Highlights",
            "expectedRevision": 1,
            "framePreset": "Stage",
            "autoSplitOnOverflow": false
        })),
    )
    .await;
    assert_eq!(st_rw, StatusCode::OK, "one-page rewrite failed: {rw_body}");
    assert_eq!(rw_body["newRevision"], 2);

    let conn = state.registry.pool().connection().unwrap();
    let updated_path: String = conn
        .query_row(
            "SELECT html_path FROM deck_slides WHERE slide_uid = ?1",
            params![slide1_uid],
            |r| r.get(0),
        )
        .unwrap();
    let updated_html = fs::read_to_string(proj_ws.join(&updated_path)).expect("updated one-page");
    assert!(
        updated_html.contains(&format!("data-slide-uid=\"{slide0_uid}\"")),
        "rewritten one-page HTML must retain slide 0"
    );
    assert!(
        updated_html.contains(&format!("data-slide-uid=\"{slide1_uid}\"")),
        "rewritten one-page HTML must preserve sibling slide 1"
    );
}

#[tokio::test]
async fn it_06_copilot_rewrite_returns_409_on_stale_revision() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    let (st_gen, gen_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/generate",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "title": "OCC Revision Deck",
            "brief": "Deterministic OCC protects concurrent edits.",
            "framePreset": "Stage",
            "useByok": false
        })),
    )
    .await;
    assert_eq!(st_gen, StatusCode::CREATED);
    let deck_id = gen_body["deck"]["id"].as_str().unwrap();
    let slide_uid = gen_body["deck"]["slides"][0]["slideUid"].as_str().unwrap();

    let (st_stale, err_stale) = send_req(
        &router,
        "POST",
        "/api/v1/slides/copilot/rewrite",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "deckId": deck_id,
            "slideUid": slide_uid,
            "targetFfId": "ff-s1-b1",
            "prompt": "Rewrite bullet",
            "expectedRevision": 99,
            "framePreset": "Stage"
        })),
    )
    .await;
    assert_eq!(st_stale, StatusCode::CONFLICT);
    assert_eq!(err_stale["error"]["code"], "E_STALE_REVISION");
}

#[tokio::test]
async fn it_07_cas_upload_to_cas_sha256_generate_deck_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    let csv_payload = b"Quarter,Revenue\nQ1,\"2,500,000 VND\"\nQ2,\"3,500,000 VND\"\n";
    let upload_req = Request::builder()
        .method("POST")
        .uri("/api/v1/cas/upload?sessionId=sess-cas-065&projectId=proj-a&filename=revenue.csv")
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", &editor_tok)
        .header("content-type", "text/csv")
        .body(Body::from(csv_payload.to_vec()))
        .unwrap();
    let upload_resp = router.clone().oneshot(upload_req).await.unwrap();
    assert_eq!(upload_resp.status(), StatusCode::CREATED);
    let upload_bytes = upload_resp.into_body().collect().await.unwrap().to_bytes();
    let upload_json: serde_json::Value = serde_json::from_slice(&upload_bytes).unwrap();
    let cas_sha = upload_json["blobSha256"].as_str().unwrap().to_string();

    let (st_gen, gen_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/generate",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "title": "CAS Revenue Deck",
            "brief": "Quarterly revenue verified from CAS upload.",
            "framePreset": "Window",
            "useByok": false,
            "chartSource": {
                "casSha256": cas_sha,
                "chartKind": "bar"
            }
        })),
    )
    .await;
    assert_eq!(
        st_gen,
        StatusCode::CREATED,
        "CAS generate failed: {gen_body}"
    );
    assert_eq!(gen_body["normalizedChart"]["spec"]["unit"], "M VND");
    let pts = gen_body["normalizedChart"]["spec"]["series"][0]["points"]
        .as_array()
        .unwrap();
    assert!((pts[0]["value"].as_f64().unwrap() - 2.5).abs() < 1e-6);
}

#[tokio::test]
async fn it_08_flatedecode_compressed_pdf_extraction_via_endpoint() {
    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    const FLATE_TABLE: &[u8] = &[
        120, 218, 115, 10, 225, 210, 8, 78, 77, 207, 77, 205, 43, 209, 84, 8, 201, 82, 208, 8, 74,
        45, 75, 205, 43, 77, 5, 115, 66, 180, 184, 52, 92, 243, 74, 82, 139, 10, 138, 50, 139, 33,
        66, 26, 134, 70, 6, 112, 185, 96, 95, 39, 136, 160, 5, 146, 88, 106, 78, 90, 112, 106, 81,
        25, 84, 185, 41, 92, 198, 53, 132, 11, 0, 174, 193, 29, 123,
    ];
    let mut flate_pdf = Vec::new();
    flate_pdf.extend_from_slice(
        b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj\n2 0 obj << /Filter /FlateDecode /Length 77 >>\nstream\n",
    );
    flate_pdf.extend_from_slice(FLATE_TABLE);
    flate_pdf.extend_from_slice(b"\nendstream\nendobj\n%%EOF");

    let (st, norm_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/normalize-chart-source",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "inlineBase64": BASE64_STANDARD.encode(&flate_pdf),
            "sourceFormat": "pdf",
            "chartKind": "pie"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "FlateDecode PDF failed: {norm_body}");
    assert_eq!(norm_body["rowCount"], 3);
    assert!((norm_body["proportionalSum"].as_f64().unwrap() - 100.0).abs() <= 0.05);
}

#[tokio::test]
async fn it_09_rejects_65_35_chart_stub_and_fails_closed_without_numeric_data() {
    let src = include_str!("../src/routes/presentation_copilot.rs");
    assert!(
        !src.contains("Primary Focus (65%)") && !src.contains("\"value\": 65.0"),
        "Hardcoded 65/35 chart stub must be removed from presentation_copilot.rs"
    );
    assert!(
        !src.contains("\"label\": \"Baseline\"") && !src.contains("\"value\": 40.0"),
        "Hardcoded 40/60 chart stub must be removed from presentation_copilot.rs"
    );

    let tmp = TempDir::new().unwrap();
    let (state, router, editor_tok) = setup_daemon(&tmp);
    seed_project(&state, "proj-a", "tenant-alpha", tmp.path());

    // Qualitative slide with zero numeric slots
    let (st_gen, gen_body) = send_req(
        &router,
        "POST",
        "/api/v1/decks/generate",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "title": "Qualitative Vision Deck",
            "brief": "Customer empathy and design clarity guide every release.",
            "framePreset": "Stage",
            "useByok": false
        })),
    )
    .await;
    assert_eq!(st_gen, StatusCode::CREATED);
    let deck_id = gen_body["deck"]["id"].as_str().unwrap();
    let slide_uid = gen_body["deck"]["slides"][0]["slideUid"].as_str().unwrap();

    // Requesting a chart rewrite with zero real numeric data MUST fail closed with E_CHART_DATA_REQUIRED
    let (st_rw, err_rw) = send_req(
        &router,
        "POST",
        "/api/v1/slides/copilot/rewrite",
        Some(&editor_tok),
        Some(json!({
            "projectId": "proj-a",
            "deckId": deck_id,
            "slideUid": slide_uid,
            "targetFfId": "ff-s1-b1",
            "prompt": "Render donut chart mix",
            "expectedRevision": 1,
            "requestedChartKind": "donut"
        })),
    )
    .await;
    assert_eq!(st_rw, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err_rw["error"]["code"], "E_CHART_DATA_REQUIRED");
}
