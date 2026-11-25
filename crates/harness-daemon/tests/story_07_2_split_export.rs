// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
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
use uuid::Uuid;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-072-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let other_tenant_tok = format!("sess-072-other-{}", Uuid::new_v4());
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
    (state, router, editor_tok, other_tenant_tok)
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
        Some(json!({ "slug": slug, "name": "Split Export Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn it_07_2_01_p0_qc_gate_blocks_until_explicit_override_and_persists_v30_audit() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "split-export-proj-1").await;

    // Seed a failing Story 07.1 qc_gate_runs row with p0_count = 2
    {
        let conn = state.registry.pool().connection().unwrap();
        harness_daemon::routes::visual_qc::ensure_v26_tables(&conn).unwrap();
        conn.execute(
            "INSERT INTO qc_gate_runs (
                id, project_id, target_kind, target_id, layout_sha1, viewports_json,
                verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
             ) VALUES (?1, ?2, 'APP_SCREEN', 'screen-1', ?3, '[1440]', 'FAIL', 2, 1, 0.72, '[]', 0, ?4)",
            params![
                Uuid::new_v4().to_string(),
                &project_id,
                "a".repeat(40),
                Utc::now().timestamp_millis(),
            ],
        )
        .unwrap();
    }

    let html_payload = json!({
        "projectId": project_id,
        "title": "Executive Pitch",
        "html": "<section data-slide=\"1\"><link rel=\"stylesheet\" href=\"https://cdn.example.com/a.css\"><h1>Hello</h1><svg viewBox=\"0 0 100 50\"><rect width=\"100\" height=\"50\"/></svg></section>",
        "enablePresenter": true,
        "overrideQcWarning": false
    });

    // Cross-tenant attempt rejected -> 403
    let (st_idor, _) = send_json(
        &router,
        "POST",
        "/api/v1/export/html-bundle",
        &other_tenant_tok,
        Some(html_payload.clone()),
    )
    .await;
    assert_eq!(st_idor, StatusCode::FORBIDDEN);

    // Without explicit override -> 409 QC_GATE_P0_BLOCKED
    let (st_block, block_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/html-bundle",
        &editor_tok,
        Some(html_payload.clone()),
    )
    .await;
    assert_eq!(st_block, StatusCode::CONFLICT);
    assert_eq!(block_res["error"], json!("QC_GATE_P0_BLOCKED"));
    assert_eq!(block_res["p0Count"], json!(2));

    // With explicit override -> 200 OK, zero CDN refs, V30 audit persisted
    let mut override_payload = html_payload;
    override_payload["overrideQcWarning"] = json!(true);
    let (st_ok, ok_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/html-bundle",
        &editor_tok,
        Some(override_payload),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert_eq!(ok_res["ok"], json!(true));
    assert_eq!(ok_res["data"]["externalCdnRefsCount"], json!(0));
    assert_eq!(ok_res["data"]["qcOverrideAcknowledged"], json!(true));
    assert_eq!(ok_res["data"]["presenterEnabled"], json!(true));

    // Verify GET /api/v1/export/history returns the persisted V30 receipt
    let (st_hist, hist_res) = send_json(
        &router,
        "GET",
        &format!("/api/v1/export/history?projectId={project_id}"),
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_hist, StatusCode::OK);
    let items = hist_res["data"]["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["format"], json!("OFFLINE_HTML"));
    assert_eq!(items[0]["qcOverrideAcknowledged"], json!(true));
    assert_eq!(hist_res["data"]["qcGate"]["p0Count"], json!(2));
}

#[tokio::test]
async fn it_07_2_02_vector_pdf_developer_handoff_and_plugin_isolated_pptx_fallback() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "split-export-proj-2").await;

    // 1. Vector PDF succeeds cleanly when project is UNSCANNED
    let (st_pdf, pdf_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/vector-pdf",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Board Deck",
            "slides": [
                { "heading": "Cover", "bodyLines": ["Key metrics"] },
                { "heading": "Next Steps", "bodyLines": ["Ship Q4"] }
            ]
        })),
    )
    .await;
    assert_eq!(st_pdf, StatusCode::OK);
    assert_eq!(pdf_res["data"]["pageCount"], json!(2));
    assert_eq!(pdf_res["data"]["engine"], json!("html_forge_vector"));
    assert!(pdf_res["data"]["pdfBase64"]
        .as_str()
        .unwrap()
        .starts_with("JVBERi0xLjQK"));

    // 2. PPTX plugin absent returns 200 with structured install CTA (zero crash, no LibreOffice)
    let (st_pptx, pptx_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "title": "Board Deck",
            "slideCount": 2
        })),
    )
    .await;
    assert_eq!(st_pptx, StatusCode::OK);
    assert_eq!(pptx_res["pluginInstalled"], json!(false));
    assert_eq!(pptx_res["pluginId"], json!("pptx-deck-exporter-plugin"));
    assert_eq!(
        pptx_res["installAction"],
        json!("/plugins?install=pptx-deck-exporter-plugin")
    );

    // 3. Developer Handoff returns canonical 5-file manifest
    let (st_dev, dev_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/developer-handoff",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "projectName": "Board Deck App"
        })),
    )
    .await;
    assert_eq!(st_dev, StatusCode::OK);
    assert_eq!(dev_res["data"]["audienceMode"], json!("DEVELOPER_HANDOFF"));
    assert_eq!(dev_res["data"]["files"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn it_07_2_03_multi_target_qc_gate_cannot_be_bypassed_by_newer_passing_target() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "split-export-proj-3").await;

    let now = Utc::now().timestamp_millis();
    {
        let conn = state.registry.pool().connection().unwrap();
        harness_daemon::routes::visual_qc::ensure_v26_tables(&conn).unwrap();
        // Target 1 failed with p0_count = 2 at t = now
        conn.execute(
            "INSERT INTO qc_gate_runs (
                id, project_id, target_kind, target_id, layout_sha1, viewports_json,
                verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
             ) VALUES (?1, ?2, 'APP_SCREEN', 'screen-1', ?3, '[1440]', 'FAIL', 2, 1, 0.70, '[]', 0, ?4)",
            params![Uuid::new_v4().to_string(), &project_id, "a".repeat(40), now],
        )
        .unwrap();
        // Target 2 passed LATER at t = now + 5000 — must NOT mask screen-1's unresolved P0s
        conn.execute(
            "INSERT INTO qc_gate_runs (
                id, project_id, target_kind, target_id, layout_sha1, viewports_json,
                verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
             ) VALUES (?1, ?2, 'APP_SCREEN', 'screen-2', ?3, '[1440]', 'PASS', 0, 0, 0.99, '[]', 0, ?4)",
            params![
                Uuid::new_v4().to_string(),
                &project_id,
                "b".repeat(40),
                now + 5000
            ],
        )
        .unwrap();
    }

    let payload = json!({
        "projectId": project_id,
        "title": "Multi-Target Deck",
        "html": "<section data-slide=\"1\"><h1>Test</h1></section>",
        "overrideQcWarning": false
    });

    let (st_block, block_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/html-bundle",
        &editor_tok,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_block, StatusCode::CONFLICT);
    assert_eq!(block_res["error"], json!("QC_GATE_P0_BLOCKED"));
    assert_eq!(block_res["p0Count"], json!(2));

    // Re-scan screen-1 as PASS at t = now + 10000 -> unresolved P0s drop to 0, export succeeds
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "INSERT INTO qc_gate_runs (
                id, project_id, target_kind, target_id, layout_sha1, viewports_json,
                verdict, p0_count, p1_count, ssim_score, issues_json, autofix_attempt, created_at
             ) VALUES (?1, ?2, 'APP_SCREEN', 'screen-1', ?3, '[1440]', 'PASS', 0, 0, 0.98, '[]', 1, ?4)",
            params![
                Uuid::new_v4().to_string(),
                &project_id,
                "c".repeat(40),
                now + 10000
            ],
        )
        .unwrap();
    }

    let (st_ok, ok_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/html-bundle",
        &editor_tok,
        Some(payload),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert_eq!(ok_res["data"]["qcGateStatus"], json!("PASS"));
}

#[tokio::test]
async fn it_07_2_04_pptx_body_guard_tenant_audit_and_plugin_verdict_symlink_hardening() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, _other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "split-export-proj-4").await;

    // 1. Oversized PPTX body (> 1 MiB) rejected with 413 ASSET_SIZE_LIMIT_EXCEEDED
    let huge_title = "T".repeat(1_200_000);
    let (st_big, big_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({ "projectId": project_id, "title": huge_title })),
    )
    .await;
    assert_eq!(st_big, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(big_res["error"], json!("ASSET_SIZE_LIMIT_EXCEEDED"));

    // 2. Non-PASS DB verdict (QUARANTINED) MUST NOT be overridden even if local plugin folder exists
    let local_plugin_dir = tmp.path().join("plugins").join("pptx-deck-exporter-plugin");
    std::fs::create_dir_all(&local_plugin_dir).unwrap();
    std::fs::write(local_plugin_dir.join("manifest.json"), "{}").unwrap();
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "INSERT INTO user_skill_registry (
                id, tenant_id, project_id, skill_slug, header_tokens,
                security_scan_verdict, license_classification, skill_md_path, created_at
             ) VALUES (?1, 'tenant-alpha', ?2, 'pptx-deck-exporter-plugin', 10, 'QUARANTINED', 'ADOPT_PRIOR_ART', 'plugins/pptx/SKILL.md', ?3)",
            params![Uuid::new_v4().to_string(), &project_id, Utc::now().timestamp_millis()],
        )
        .unwrap();
    }

    let (st_quar, quar_res) = send_json(
        &router,
        "POST",
        "/api/v1/export/pptx",
        &editor_tok,
        Some(json!({ "projectId": project_id, "title": "Quarantined Check" })),
    )
    .await;
    assert_eq!(st_quar, StatusCode::OK);
    assert_eq!(quar_res["pluginInstalled"], json!(false));

    // 3. Verify V30 audit persisted with tenant_id = 'tenant-alpha'
    {
        let conn = state.registry.pool().connection().unwrap();
        let tenant_stored: String = conn
            .query_row(
                "SELECT tenant_id FROM export_bundle_runs WHERE project_id = ?1 LIMIT 1",
                params![&project_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tenant_stored, "tenant-alpha");
    }

    // 4. Symlinked local plugin directory rejected when DB has no row
    #[cfg(unix)]
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "DELETE FROM user_skill_registry WHERE project_id = ?1",
            params![&project_id],
        )
        .unwrap();
        std::fs::remove_dir_all(&local_plugin_dir).unwrap();
        let outside_dir = TempDir::new().unwrap();
        std::fs::write(outside_dir.path().join("manifest.json"), "{}").unwrap();
        std::os::unix::fs::symlink(outside_dir.path(), &local_plugin_dir).unwrap();

        let (st_sym, sym_res) = send_json(
            &router,
            "POST",
            "/api/v1/export/pptx",
            &editor_tok,
            Some(json!({ "projectId": project_id, "title": "Symlink Check" })),
        )
        .await;
        assert_eq!(st_sym, StatusCode::OK);
        assert_eq!(sym_res["pluginInstalled"], json!(false));
    }
}
