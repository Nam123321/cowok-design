// @story 05.2
// @implements V10 proposals, target nodes, HMAC gate receipts, CAS base hashes, pre_apply_safety snapshot, cowok-design-diff, tradeoff decisions
use std::fs;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::lockfile::CURRENT_SCHEMA_VERSION;
use harness_daemon::{build_router, DaemonAppState, ProjectDto};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok = format!("sess-052-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok)
}

fn add_tenant_session(state: &Arc<DaemonAppState>, tenant_id: &str, user_id: &str) -> String {
    let tok = format!("sess-052-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            role: "editor".into(),
        },
        exp,
    );
    tok
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

async fn create_project(
    router: &axum::Router,
    tok: &str,
    slug: &str,
    ws_root: Option<&std::path::Path>,
) -> ProjectDto {
    let (st, val) = send_req(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        Some(json!({
            "slug": slug,
            "name": format!("Project {slug}"),
            "workspace_root": ws_root.map(|p| p.to_string_lossy().to_string()),
            "product_brief": "Story 05.2 proposals test"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "project create: {val}");
    serde_json::from_value(val).unwrap()
}

fn seed_two_flow_nodes(state: &Arc<DaemonAppState>, proj_id: &str) -> (String, String) {
    let conn = state.registry.pool().connection().unwrap();
    let now_ms = Utc::now().timestamp_millis();
    let graph_id = uuid::Uuid::new_v4().to_string();
    let node_a = uuid::Uuid::new_v4().to_string();
    let node_b = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, revision, updated_at)
         VALUES (?1, 'tenant-alpha', ?2, 'default', 'Main Flow', 1, 1, 1, ?3)",
        params![graph_id, proj_id, now_ms],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, revision, created_at, updated_at)
         VALUES (?1, ?2, 'tenant-alpha', ?3, 'default', 'home', 'Home', 'screen', '/', 1, ?4, ?4)",
        params![node_a, graph_id, proj_id, now_ms],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, revision, created_at, updated_at)
         VALUES (?1, ?2, 'tenant-alpha', ?3, 'default', 'settings', 'Settings', 'screen', '/settings', 1, ?4, ?4)",
        params![node_b, graph_id, proj_id, now_ms],
    )
    .unwrap();
    (node_a, node_b)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[tokio::test]
async fn story_05_2_v10_schema_node_filtering_and_target_cascade() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);

    // Health endpoint reports schema_version == CURRENT_SCHEMA_VERSION (V21)
    assert_eq!(CURRENT_SCHEMA_VERSION, 29);
    let (st_h, health) = send_req(&router, "GET", "/api/v1/health", &tok, None).await;
    assert_eq!(st_h, StatusCode::OK);
    assert_eq!(health["schema_version"], CURRENT_SCHEMA_VERSION);

    let proj = create_project(&router, &tok, "proj-node-filter", None).await;
    let proj_id = proj.id.to_string();
    let (node_a, node_b) = seed_two_flow_nodes(&state, &proj_id);

    // Proposal 1 targets node_a directly and [node_a, node_b] via proposal_target_nodes
    let (st1, p1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Multi-node header update",
            "nodeId": node_a,
            "batchId": "batch-01",
            "nodeIds": [node_a, node_b],
            "contentMode": "full_file",
            "files": [{
                "path": "src/Header.tsx",
                "status": "added",
                "additions": 1,
                "deletions": 0,
                "patch": "export const Header = () => <header>OK</header>;"
            }]
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::CREATED, "p1: {p1}");
    assert_eq!(p1["nodeId"], node_a);
    assert_eq!(p1["batchId"], "batch-01");
    assert_eq!(p1["contentMode"], "full_file");
    let p1_nodes: Vec<String> = serde_json::from_value(p1["nodeIds"].clone()).unwrap();
    assert_eq!(p1_nodes.len(), 2);
    assert!(p1["gateReceiptId"].as_str().is_some());
    assert_eq!(p1["gateReceipt"]["exitCode"], 0);

    // Proposal 2 targets node_b directly
    let (st2, p2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Settings panel copy",
            "nodeId": node_b,
            "files": [{
                "path": "src/Settings.tsx",
                "status": "added",
                "additions": 1,
                "deletions": 0,
                "patch": "export const Settings = () => <section>Settings</section>;"
            }]
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::CREATED, "p2: {p2}");

    // Proposal 3 has no nodeId
    let (st3, _p3) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Global tokens update",
            "files": [{
                "path": "src/Tokens.ts",
                "status": "added",
                "additions": 1,
                "deletions": 0,
                "patch": "export const spacing = 8;"
            }]
        })),
    )
    .await;
    assert_eq!(st3, StatusCode::CREATED);

    // Filter by node_a -> only Proposal 1
    let (st_la, list_a) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/proposals?nodeId={node_a}"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_la, StatusCode::OK);
    let arr_a = list_a.as_array().unwrap();
    assert_eq!(arr_a.len(), 1);
    assert_eq!(arr_a[0]["id"], p1["id"]);

    // Filter by node_b -> Proposal 1 (via proposal_target_nodes) + Proposal 2 (via node_id)
    let (st_lb, list_b) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/proposals?nodeId={node_b}"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_lb, StatusCode::OK);
    let arr_b = list_b.as_array().unwrap();
    assert_eq!(arr_b.len(), 2);

    // Unfiltered -> all 3
    let (st_all, list_all) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_all, StatusCode::OK);
    assert_eq!(list_all.as_array().unwrap().len(), 3);

    // Referential integrity: non-existent target node in nodeIds fails with 422
    let (st_bad_fk, bad_fk) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Bogus target node",
            "nodeIds": ["nonexistent-node-id"],
            "files": [{
                "path": "src/X.ts",
                "status": "added",
                "additions": 1,
                "deletions": 0,
                "patch": "export const x = 1;"
            }]
        })),
    )
    .await;
    assert_eq!(st_bad_fk, StatusCode::UNPROCESSABLE_ENTITY, "fk: {bad_fk}");

    // Composite FK ON DELETE CASCADE: deleting node_a deletes its proposal_target_nodes row
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute("DELETE FROM flow_nodes WHERE id = ?1", params![node_a])
            .unwrap();
        let rem: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM proposal_target_nodes WHERE proposal_id = ?1",
                params![p1["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rem, 1, "only node_b target row should remain");
    }
}

#[tokio::test]
async fn story_05_2_gate_receipt_tamper_returns_412_unverified_gate_claim() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-receipt-412", None).await;
    let proj_id = proj.id.to_string();

    let (st, created) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Verified button",
            "files": [{
                "path": "src/Btn.tsx",
                "status": "added",
                "additions": 1,
                "deletions": 0,
                "patch": "export const Btn = () => <button>Go</button>;"
            }]
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let prop_id = created["id"].as_str().unwrap().to_string();
    let receipt_id = created["gateReceiptId"].as_str().unwrap().to_string();

    // 1. Missing gate_receipt_id on a "ready"/"passed" proposal -> 412 UNVERIFIED_GATE_CLAIM
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "UPDATE change_proposals SET gate_receipt_id = NULL WHERE id = ?1",
            params![prop_id],
        )
        .unwrap();
    }
    let (st_a1, res_a1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/apply"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_a1, StatusCode::PRECONDITION_FAILED, "a1: {res_a1}");
    assert_eq!(res_a1["error"], "UNVERIFIED_GATE_CLAIM");

    // Restore receipt_id, then tamper receipt_hmac -> 412 UNVERIFIED_GATE_CLAIM
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "UPDATE change_proposals SET gate_receipt_id = ?1 WHERE id = ?2",
            params![receipt_id, prop_id],
        )
        .unwrap();
        conn.execute(
            "UPDATE proposal_gate_receipts SET receipt_hmac = 'forged-hmac' WHERE id = ?1",
            params![receipt_id],
        )
        .unwrap();
    }
    let (st_a2, res_a2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/apply"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_a2, StatusCode::PRECONDITION_FAILED, "a2: {res_a2}");
    assert_eq!(res_a2["error"], "UNVERIFIED_GATE_CLAIM");

    // Re-validate mints a fresh genuine signed receipt -> apply succeeds (200 OK)
    let (st_val, reval) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/validate"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_val, StatusCode::OK, "reval: {reval}");
    let new_receipt_id = reval["gateReceiptId"].as_str().unwrap();
    assert_ne!(new_receipt_id, receipt_id);

    // Tamper workspace_head_sha256 -> 412 UNVERIFIED_GATE_CLAIM
    {
        let conn = state.registry.pool().connection().unwrap();
        conn.execute(
            "UPDATE proposal_gate_receipts SET workspace_head_sha256 = '0000' WHERE id = ?1",
            params![new_receipt_id],
        )
        .unwrap();
    }
    let (st_a3, res_a3) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/apply"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_a3, StatusCode::PRECONDITION_FAILED, "a3: {res_a3}");
    assert_eq!(res_a3["error"], "UNVERIFIED_GATE_CLAIM");

    // Re-validate once more and apply cleanly
    let (st_val2, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/validate"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_val2, StatusCode::OK);
    let (st_ok, applied) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/apply"),
        &tok,
        Some(json!({ "force": false })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "apply: {applied}");
    assert_eq!(applied["status"], "applied");
}

#[tokio::test]
async fn story_05_2_cas_stale_base_hash_409_rebase_and_pre_apply_snapshot() {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path().join("ws");
    fs::create_dir_all(&ws).unwrap();
    let ws = fs::canonicalize(&ws).unwrap();

    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-cas-052", Some(&ws)).await;
    let proj_id = proj.id.to_string();

    // Seed initial file via Proposal 1
    let v1_code = "export const Card = () => <div>v1</div>;\n";
    let (_, p1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Seed Card v1",
            "files": [{
                "path": "src/Card.tsx",
                "status": "added",
                "additions": 1,
                "deletions": 0,
                "patch": v1_code
            }]
        })),
    )
    .await;
    let (st_ap1, _) = send_req(
        &router,
        "POST",
        &format!(
            "/api/v1/projects/{proj_id}/proposals/{}/apply",
            p1["id"].as_str().unwrap()
        ),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_ap1, StatusCode::OK);

    // Create Proposal 2 modifying src/Card.tsx (snapshots baseFileHashes["src/Card.tsx"] == sha256(v1))
    let v2_code = "export const Card = () => <div>v2</div>;\n";
    let (st_c2, p2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Update Card to v2",
            "files": [{
                "path": "src/Card.tsx",
                "status": "modified",
                "additions": 1,
                "deletions": 1,
                "patch": v2_code
            }]
        })),
    )
    .await;
    assert_eq!(st_c2, StatusCode::CREATED);
    assert_eq!(
        p2["baseFileHashes"]["src/Card.tsx"],
        sha256_hex(v1_code.as_bytes())
    );
    let p2_id = p2["id"].as_str().unwrap().to_string();

    // External edit on disk changes src/Card.tsx to v1_hotfix
    let hotfix_code = "export const Card = () => <div>v1-hotfix</div>;\n";
    fs::write(ws.join("src/Card.tsx"), hotfix_code).unwrap();

    // Apply Proposal 2 -> 409 PROPOSAL_STALE_BASE_HASH
    let (st_conflict, conflict) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{p2_id}/apply"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_conflict, StatusCode::CONFLICT, "conflict: {conflict}");
    assert_eq!(conflict["error"], "PROPOSAL_STALE_BASE_HASH");
    assert_eq!(conflict["staleFiles"], json!(["src/Card.tsx"]));

    // Re-validate WITHOUT rebaseBaseHashes keeps base hash pinned and still 409s
    let (st_no_rebase, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{p2_id}/validate"),
        &tok,
        Some(json!({ "rebaseBaseHashes": false })),
    )
    .await;
    assert_eq!(st_no_rebase, StatusCode::OK);
    let (st_still_409, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{p2_id}/apply"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_still_409, StatusCode::CONFLICT);

    // Re-validate WITH rebaseBaseHashes: true updates baseFileHashes to hotfix hash
    let (st_rebase, rebased) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{p2_id}/validate"),
        &tok,
        Some(json!({ "rebaseBaseHashes": true })),
    )
    .await;
    assert_eq!(st_rebase, StatusCode::OK);
    assert_eq!(
        rebased["baseFileHashes"]["src/Card.tsx"],
        sha256_hex(hotfix_code.as_bytes())
    );

    // Apply Proposal 2 -> succeeds and captures pre_apply_safety snapshot!
    let (st_ok, applied) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{p2_id}/apply"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "applied: {applied}");
    assert_eq!(applied["status"], "applied");
    assert_eq!(
        fs::read_to_string(ws.join("src/Card.tsx")).unwrap(),
        v2_code
    );

    let conn = state.registry.pool().connection().unwrap();
    let safety_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM version_snapshots WHERE project_id = ?1 AND trigger = 'pre_apply_safety'",
            params![proj_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        safety_count, 2,
        "each apply captures a pre_apply_safety snapshot"
    );
}

#[tokio::test]
async fn story_05_2_visual_diff_pixelmatch_ssim_and_rasterizer_fallback() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-diff-052", None).await;
    let proj_id = proj.id.to_string();

    let (st_c, prop) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Visual diff target",
            "files": [{
                "path": "src/Hero.tsx",
                "status": "modified",
                "additions": 2,
                "deletions": 1,
                "patch": "export const Hero = () => <main>Updated</main>;"
            }]
        })),
    )
    .await;
    assert_eq!(st_c, StatusCode::CREATED);
    let prop_id = prop["id"].as_str().unwrap();

    // 1. Missing RGBA buffers -> RASTERIZER_UNAVAILABLE fallback
    let (st_fallback, fallback_res) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/diff"),
        &tok,
        Some(json!({})),
    )
    .await;
    assert_eq!(st_fallback, StatusCode::OK);
    assert_eq!(fallback_res["status"], "RASTERIZER_UNAVAILABLE");
    assert_eq!(fallback_res["ssim"], 1.0);
    assert_eq!(
        fallback_res["structuralDiffSummary"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // 2. Out-of-bounds width > 1280 -> 422
    let (st_oob, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/diff"),
        &tok,
        Some(json!({
            "beforeRgbaBase64": B64_STD.encode([255u8; 4]),
            "afterRgbaBase64": B64_STD.encode([255u8; 4]),
            "width": 1281,
            "height": 1
        })),
    )
    .await;
    assert_eq!(st_oob, StatusCode::UNPROCESSABLE_ENTITY);

    // 3. Localized 4x4 red box at (4, 6) inside 16x16 white canvas
    let w = 16usize;
    let h = 16usize;
    let before = vec![255u8; w * h * 4];
    let mut after = vec![255u8; w * h * 4];
    for y in 6..10 {
        for x in 4..8 {
            let idx = (y * w + x) * 4;
            after[idx] = 220;
            after[idx + 1] = 20;
            after[idx + 2] = 20;
            after[idx + 3] = 255;
        }
    }
    let (st_diff, diff_res) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/diff"),
        &tok,
        Some(json!({
            "beforeRgbaBase64": B64_STD.encode(&before),
            "afterRgbaBase64": B64_STD.encode(&after),
            "width": w,
            "height": h,
            "threshold": 0.1
        })),
    )
    .await;
    assert_eq!(st_diff, StatusCode::OK, "diff: {diff_res}");
    assert_eq!(diff_res["status"], "COMPUTED");
    assert_eq!(diff_res["mismatchedPixels"], 16);
    assert_eq!(diff_res["totalPixels"], 256);
    assert_eq!(
        diff_res["changedBbox"],
        json!({ "x": 4, "y": 6, "width": 4, "height": 4 })
    );
    let ssim = diff_res["ssim"].as_f64().unwrap();
    assert!(
        ssim > 0.0 && ssim < 1.0,
        "expected partial SSIM, got {ssim}"
    );

    // Persisted in GET /proposals/:id
    let (st_get, fetched) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_get, StatusCode::OK);
    assert_eq!(fetched["visualDiff"]["status"], "COMPUTED");
    assert_eq!(fetched["visualDiff"]["mismatchedPixels"], 16);
}

#[tokio::test]
async fn story_05_2_tradeoff_badges_limits_and_decisions() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-tradeoffs-052", None).await;
    let proj_id = proj.id.to_string();

    // > 12 tradeoffs -> 422
    let too_many: Vec<_> = (1..=13)
        .map(|i| {
            json!({
                "elementFfId": format!("ff-el-{i}"),
                "badgeIndex": ((i - 1) % 12) + 1,
                "designNote": "Note"
            })
        })
        .collect();
    let (st_too_many, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Too many tradeoffs",
            "files": [{ "path": "src/A.tsx", "status": "added", "additions": 1, "deletions": 0, "patch": "export const A = 1;" }],
            "tradeoffs": too_many
        })),
    )
    .await;
    assert_eq!(st_too_many, StatusCode::UNPROCESSABLE_ENTITY);

    // Valid proposal with 2 tradeoffs
    let (st_c, prop) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Tradeoff badges proposal",
            "files": [{ "path": "src/Card.tsx", "status": "added", "additions": 1, "deletions": 0, "patch": "export const Card = () => <div/>;" }],
            "tradeoffs": [
                {
                    "elementFfId": "ff-card-padding",
                    "badgeIndex": 1,
                    "designNote": "Increased card padding to 24px for rhythm",
                    "insteadLabel": "Keep 16px compact padding",
                    "insteadPatch": { "padding": "16px" }
                },
                {
                    "elementFfId": "ff-card-radius",
                    "badgeIndex": 2,
                    "designNote": "Rounded corners 12px",
                    "insteadLabel": "Use 8px radius"
                }
            ]
        })),
    )
    .await;
    assert_eq!(st_c, StatusCode::CREATED, "prop: {prop}");
    let prop_id = prop["id"].as_str().unwrap();
    let tradeoffs = prop["tradeoffs"].as_array().unwrap();
    assert_eq!(tradeoffs.len(), 2);
    assert_eq!(tradeoffs[0]["decision"], "pending");
    let t1_id = tradeoffs[0]["id"].as_str().unwrap();
    let t2_id = tradeoffs[1]["id"].as_str().unwrap();

    // Decide agreed on t1
    let (st_d1, d1) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/tradeoffs/{t1_id}/decide"),
        &tok,
        Some(json!({ "decision": "agreed" })),
    )
    .await;
    assert_eq!(st_d1, StatusCode::OK, "d1: {d1}");
    assert_eq!(d1["decision"], "agreed");
    assert!(d1["decidedAt"].as_str().is_some());

    // Decide asked on t2 with askPrompt
    let (st_d2, d2) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/tradeoffs/{t2_id}/decide"),
        &tok,
        Some(json!({ "decision": "asked", "askPrompt": "Why 12px instead of 8px?" })),
    )
    .await;
    assert_eq!(st_d2, StatusCode::OK, "d2: {d2}");
    assert_eq!(d2["decision"], "asked");
    assert_eq!(d2["askPrompt"], "Why 12px instead of 8px?");

    // Cross-tenant decide is rejected with 403
    let other_tok = add_tenant_session(&state, "tenant-stranger", "mallory");
    let (st_cross, _) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/tradeoffs/{t1_id}/decide"),
        &other_tok,
        Some(json!({ "decision": "skipped" })),
    )
    .await;
    assert_eq!(st_cross, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn story_05_2_source_comment_id_auto_resolves_element_comment_with_i64_updated_at() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let target_file = tmp.path().join("src/comment-fix.html");
    fs::create_dir_all(target_file.parent().unwrap()).unwrap();
    fs::write(&target_file, "<div id='cta'>Before</div>").unwrap();

    let proj = create_project(&router, &tok, "comment-resolve-proj", Some(tmp.path())).await;
    let proj_id = proj.id.to_string();
    let (node_a, _) = seed_two_flow_nodes(&state, &proj_id);

    // Seed an element comment for node_a
    let (st_cm, cm_json) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/flow/nodes/{node_a}/comments"),
        &tok,
        Some(json!({
            "elementFfId": "cta",
            "content": "Polish CTA contrast",
            "author": "designer-1",
            "domHash": "domhash1234",
            "anchorKind": "html_element"
        })),
    )
    .await;
    assert_eq!(st_cm, StatusCode::CREATED, "comment: {cm_json}");
    let cmt_id = cm_json["id"].as_str().unwrap().to_string();

    let mut base_hashes = serde_json::Map::new();
    base_hashes.insert(
        "src/comment-fix.html".into(),
        json!(format!(
            "{:x}",
            Sha256::digest(b"<div id='cta'>Before</div>")
        )),
    );
    let (st_c, prop) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals"),
        &tok,
        Some(json!({
            "title": "Fix CTA per comment",
            "nodeId": node_a,
            "sourceCommentId": cmt_id,
            "contentMode": "full_file",
            "baseFileHashes": base_hashes,
            "files": [{
                "path": "src/comment-fix.html",
                "status": "modified",
                "patch": "<div id='cta'>After</div>",
                "additions": 1,
                "deletions": 1
            }]
        })),
    )
    .await;
    assert_eq!(st_c, StatusCode::CREATED, "prop: {prop}");
    assert_eq!(prop["sourceCommentId"], cmt_id);
    let prop_id = prop["id"].as_str().unwrap();

    let (st_v, v_json) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/validate"),
        &tok,
        Some(json!({ "rebaseBaseHashes": true })),
    )
    .await;
    assert_eq!(st_v, StatusCode::OK, "validate: {v_json}");

    let (st_a, applied) = send_req(
        &router,
        "POST",
        &format!("/api/v1/projects/{proj_id}/proposals/{prop_id}/apply"),
        &tok,
        Some(json!({ "force": false })),
    )
    .await;
    assert_eq!(st_a, StatusCode::OK, "apply: {applied}");
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["sourceCommentId"], cmt_id);

    // Listing comments via HTTP must succeed with integer updated_at and anchorStatus == resolved
    let (st_list, list_json) = send_req(
        &router,
        "GET",
        &format!("/api/v1/projects/{proj_id}/flow/nodes/{node_a}/comments"),
        &tok,
        None,
    )
    .await;
    assert_eq!(st_list, StatusCode::OK, "list comments: {list_json}");
    let items = list_json.as_array().expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], cmt_id);
    assert_eq!(items[0]["anchorStatus"], "resolved");
}
