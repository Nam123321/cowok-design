// @story 07.3
// @implements AC-03
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState};
use html_forge::pptx_layout::decode_base64;
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-073-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-073".into(),
            role: "editor".into(),
        },
        exp,
    );

    let router = build_router((*state).clone());
    (state, router, editor_tok)
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

async fn create_project(router: &axum::Router, tok: &str, slug: &str) -> String {
    let (st, res) = send_json(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        Some(json!({ "slug": slug, "name": "Story 07.3 PPTX Project", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn it_07_3_01_builtin_active_pptx_editable_and_visual_exact_returns_real_zip_and_v30_audit() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "pptx-builtin-1").await;

    let sample_html = r##"
    <style>
      @keyframes pulse { from { opacity: 0.5; } to { opacity: 1; } }
      .cta:hover { color: #2563eb; }
    </style>
    <section data-slide="1" data-transition="morph">
      <h1>Kế hoạch Quý 4 &amp; Chiến lược</h1>
      <div style="backdrop-filter: blur(12px);">Glass highlight</div>
      <table>
        <tr><th rowspan="2">Hạng mục</th><th colspan="2">Mục tiêu</th></tr>
        <tr><td>Q3</td><td>Q4</td></tr>
      </table>
      <svg width="100" height="60" viewBox="0 0 100 60"><rect width="100" height="60" fill="#2563eb"/></svg>
    </section>
    <section data-slide="2" data-transition="fade">
      <h2>Tổng kết</h2>
      <p>Sẵn sàng trình chiếu trên PowerPoint, Keynote và Google Slides.</p>
    </section>"##;

    // 1. Editable mode (default)
    let (st_ed, res_ed) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Kế hoạch Quý 4",
            "html": sample_html,
            "exportMode": "editable",
            "locale": "vi-VN"
        })),
    )
    .await;
    assert_eq!(st_ed, StatusCode::OK);
    assert_eq!(res_ed["ok"], json!(true));
    assert_eq!(res_ed["pluginInstalled"], json!(true));
    assert_eq!(res_ed["data"]["pluginStatus"], json!("BUILTIN_ACTIVE"));
    assert_eq!(res_ed["data"]["exportMode"], json!("editable"));
    assert_eq!(res_ed["data"]["fileName"], json!("ke-hoach-quy-4.pptx"));
    assert_eq!(res_ed["data"]["slideCount"], json!(2));
    assert_eq!(res_ed["data"]["sha256"].as_str().unwrap().len(), 64);

    let b64 = res_ed["data"]["pptxBase64"].as_str().expect("pptxBase64");
    let decoded = decode_base64(b64).expect("valid base64");
    assert_eq!(
        &decoded[0..4],
        b"PK\x03\x04",
        "must decode to real OOXML ZIP"
    );
    assert_eq!(
        decoded.len(),
        res_ed["data"]["byteSize"].as_u64().unwrap() as usize
    );

    let warnings = res_ed["data"]["fidelityWarnings"]
        .as_array()
        .expect("fidelityWarnings array");
    assert!(warnings
        .iter()
        .any(|w| w["code"] == json!("BACKDROP_FILTER_RASTERIZED")));
    assert!(warnings
        .iter()
        .any(|w| w["code"] == json!("CSS_ANIMATION_OMITTED")));

    // 2. Visual-Exact mode (without CDP snapshot -> preserves real editable content + fallback warning)
    let (st_ve, res_ve) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Kế hoạch Quý 4",
            "html": sample_html,
            "exportMode": "visual_exact",
            "locale": "en-US"
        })),
    )
    .await;
    assert_eq!(st_ve, StatusCode::OK);
    assert_eq!(res_ve["data"]["exportMode"], json!("visual_exact"));
    let ve_bytes = decode_base64(res_ve["data"]["pptxBase64"].as_str().unwrap()).unwrap();
    assert_eq!(&ve_bytes[0..4], b"PK\x03\x04");
    let ve_warnings = res_ve["data"]["fidelityWarnings"]
        .as_array()
        .expect("visual_exact fidelityWarnings array");
    assert!(
        ve_warnings
            .iter()
            .any(|w| w["code"] == json!("VISUAL_EXACT_RASTER_UNAVAILABLE")),
        "visual_exact without headless Chromium snapshot must emit VISUAL_EXACT_RASTER_UNAVAILABLE"
    );
    // Verify the generated slide1.xml is non-blank and preserves real content (not a 64x36 solid box)
    assert!(
        ve_bytes.len() > 2_000,
        "visual_exact fallback PPTX must contain real slide XML content"
    );

    // 3. Verify V30 export_bundle_runs persistence & GET /api/v1/export/history
    {
        let conn = state.registry.pool().connection().unwrap();
        let (fmt, plugin_status, byte_size): (String, String, i64) = conn
            .query_row(
                "SELECT format, plugin_status, byte_size FROM export_bundle_runs
                 WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 1",
                params![&project_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(fmt, "PLUGIN_PPTX");
        assert_eq!(plugin_status, "BUILTIN_ACTIVE");
        assert!(byte_size > 100);
    }

    let (st_hist, hist_res) = send_json(
        &router,
        "GET",
        &format!("/api/v1/export/history?projectId={project_id}"),
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_hist, StatusCode::OK);
    assert_eq!(hist_res["data"]["pptxPluginInstalled"], json!(true));
    assert_eq!(hist_res["data"]["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn it_07_3_02_pptx_enforces_p0_qc_gate_block_and_explicit_override() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "pptx-qc-gate-2").await;

    {
        let conn = state.registry.pool().connection().unwrap();
        harness_daemon::routes::visual_qc::ensure_v26_tables(&conn).unwrap();
        conn.execute(
            "INSERT INTO qc_gate_runs (
                id, project_id, target_kind, target_id, layout_sha1, viewports_json,
                verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
             ) VALUES (?1, ?2, 'PRESENTATION_SLIDE', 'slide-1', ?3, '[1920]', 'FAIL', 3, 1, 0.71, '[]', 0, ?4)",
            params![
                Uuid::new_v4().to_string(),
                &project_id,
                "a".repeat(40),
                Utc::now().timestamp_millis(),
            ],
        )
        .unwrap();
    }

    // Without override -> 409 QC_GATE_P0_BLOCKED
    let (st_block, block_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Blocked Deck",
            "html": "<section data-slide=\"1\"><h1>Test</h1></section>",
            "exportMode": "editable",
            "overrideQcWarning": false
        })),
    )
    .await;
    assert_eq!(st_block, StatusCode::CONFLICT);
    assert_eq!(block_res["error"], json!("QC_GATE_P0_BLOCKED"));
    assert_eq!(block_res["p0Count"], json!(3));

    // With explicit override -> 200 OK with FAIL_OVERRIDDEN recorded
    let (st_ok, ok_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Override Deck",
            "html": "<section data-slide=\"1\"><h1>Test</h1></section>",
            "exportMode": "editable",
            "overrideQcWarning": true
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert_eq!(ok_res["data"]["qcGateStatus"], json!("FAIL_OVERRIDDEN"));
    assert_eq!(ok_res["data"]["qcOverrideAcknowledged"], json!(true));
    assert_eq!(ok_res["data"]["pluginStatus"], json!("BUILTIN_ACTIVE"));

    // ADV-03 HTTP check: >200 slides fails closed with 422 INVALID_REQUEST
    let mut too_many = String::new();
    for i in 1..=201 {
        too_many.push_str(&format!("<section><h1>S{i}</h1></section>"));
    }
    let (st_limit, limit_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Over Slide Cap",
            "html": too_many,
            "exportMode": "editable",
            "overrideQcWarning": true
        })),
    )
    .await;
    assert_eq!(st_limit, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(limit_res["error"], json!("INVALID_REQUEST"));
}
