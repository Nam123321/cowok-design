// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::process::Command;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::lockfile::CURRENT_SCHEMA_VERSION;
use harness_daemon::routes::execution_reaper::{spawn_pgid_child, ActiveWorkerEntry};
use harness_daemon::{build_router, DaemonAppState};
use html_forge::compute_sha256_hex;
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-054-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let viewer_tok = format!("sess-054-vw-{}", Uuid::new_v4());
    state.auth.insert_session(
        viewer_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-1".into(),
            role: "viewer".into(),
        },
        exp,
    );

    let other_tenant_tok = format!("sess-054-other-{}", Uuid::new_v4());
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
    (state, router, editor_tok, viewer_tok, other_tenant_tok)
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
        Some(json!({ "slug": slug, "name": "Sketch Polish Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn it_05_4_01_clarify_direction_preempts_pgid_increments_epoch_and_rejects_stale_epoch() {
    assert_eq!(CURRENT_SCHEMA_VERSION, 20);
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, editor_tok, viewer_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "clarify-proj").await;

    // 1. Initial Day-0 clarify request locks into clarifying with 2 localized directions + routed craft skills
    let (st1, r1) = send_json(
        &router,
        "POST",
        "/api/v1/sdi/clarify",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "goalPrompt": "Xây bảng điều khiển doanh thu SaaS tối giản",
            "locale": "vi"
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "r1={r1}");
    assert_eq!(r1["ok"], true);
    assert_eq!(r1["data"]["status"], "clarifying");
    assert_eq!(r1["data"]["generationEpoch"], 1);
    assert_eq!(r1["data"]["directions"].as_array().unwrap().len(), 2);
    let routed = r1["data"]["routedSkills"].as_array().unwrap();
    assert!(routed.iter().any(|s| s == "sketch-monochrome"));
    assert!(routed.iter().any(|s| s == "impeccable-polish"));

    // 2. Spawn a real managed PGID child worker & register it under project_id
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    let (mut child, pid, pgid, start_time) = spawn_pgid_child(cmd).expect("spawn sleep");
    let shadow_dir = tmp.path().join(".cowok-design/shadow/job-clarify-1");
    std::fs::create_dir_all(&shadow_dir).expect("mkdir shadow");
    state.active_workers.register(ActiveWorkerEntry {
        job_id: "job-clarify-1".into(),
        project_id: project_id.clone(),
        fencing_token: 1,
        pid,
        pgid,
        start_time,
        shadow_dir: shadow_dir.clone(),
    });

    // 3. Switch to Direction Pill 2 with preemptPgid -> kills managed -PGID worker & bumps generationEpoch to 2
    let (st2, r2) = send_json(
        &router,
        "POST",
        "/api/v1/sdi/clarify",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "goalPrompt": "Xây bảng điều khiển doanh thu SaaS tối giản",
            "locale": "vi",
            "activeDirectionId": "dir-polish-workbench",
            "preemptPgid": pgid,
            "expectedGenerationEpoch": 1
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::OK, "r2={r2}");
    assert_eq!(r2["data"]["status"], "preempted");
    assert_eq!(r2["data"]["generationEpoch"], 2);
    assert_eq!(r2["data"]["preemptedWorkers"], 1);
    assert!(
        !shadow_dir.exists(),
        "shadow dir must be discarded on preempt"
    );
    let _ = child.wait();

    // 4. Stale epoch=1 request MUST be dropped with 409 STALE_GENERATION_EPOCH
    let (st3, r3) = send_json(
        &router,
        "POST",
        "/api/v1/sdi/clarify",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "goalPrompt": "Old generation",
            "locale": "vi",
            "expectedGenerationEpoch": 1
        })),
    )
    .await;
    assert_eq!(st3, StatusCode::CONFLICT, "r3={r3}");
    assert_eq!(r3["error"]["code"], "STALE_GENERATION_EPOCH");

    // 5. Viewer write & cross-tenant access MUST be rejected (403)
    let (st_vw, _) = send_json(
        &router,
        "POST",
        "/api/v1/sdi/clarify",
        &viewer_tok,
        Some(json!({ "projectId": project_id, "goalPrompt": "x", "locale": "en" })),
    )
    .await;
    assert_eq!(st_vw, StatusCode::FORBIDDEN);

    let (st_other, _) = send_json(
        &router,
        "GET",
        &format!("/api/v1/sdi/clarify?projectId={project_id}"),
        &other_tenant_tok,
        None,
    )
    .await;
    assert_eq!(st_other, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn it_05_4_02_patch_persists_to_sqlite_enforces_occ_and_2tier_locked_rule_receipt() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "patch-proj").await;

    let starter_html = r#"<!DOCTYPE html><html><body><main data-ff-id="stage-root"><header data-ff-id="shell-nav">Nav</header><section data-ff-id="hero-card"><h1 data-ff-id="hero-title">Original Headline</h1></section></main></body></html>"#;
    let base_sha = compute_sha256_hex(starter_html.as_bytes());

    // 1. Tier-1 TEXT_EDIT auto-applies immediately with INSTANT_UNDO + SQLite persistence
    let (st1, r1) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "home-dashboard",
            "baseArtifactSha256": base_sha,
            "ffId": "hero-title",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Revenue Overview" },
            "baseHtml": starter_html
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "r1={r1}");
    assert_eq!(r1["data"]["astRevision"], 1);
    assert_eq!(r1["data"]["applyTier"], "INSTANT_UNDO");
    assert_eq!(r1["data"]["undoAvailable"], true);
    assert!(r1["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Revenue Overview"));
    let rev1_sha = r1["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Concurrent stale edit using old base_sha MUST return 409 STALE_MAPPING
    let (st_stale, r_stale) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "home-dashboard",
            "baseArtifactSha256": base_sha,
            "ffId": "hero-title",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Conflicting Edit" },
            "expectedAstRevision": 0
        })),
    )
    .await;
    assert_eq!(st_stale, StatusCode::CONFLICT, "r_stale={r_stale}");
    assert_eq!(r_stale["error"]["code"], "STALE_MAPPING");

    // 3. TOKEN_SWAP with low-contrast tint triggers OKLCH WCAG AA clamp warning
    let (st2, r2) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "home-dashboard",
            "baseArtifactSha256": rev1_sha,
            "ffId": "hero-card",
            "opKind": "TOKEN_SWAP",
            "payloadJson": { "fgHex": "#D9E2EC", "bgHex": "#FAF9F6" },
            "expectedAstRevision": 1
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::OK, "r2={r2}");
    assert_eq!(r2["data"]["astRevision"], 2);
    assert_eq!(r2["data"]["oklchGuard"]["clamped"], true);
    let rev2_sha = r2["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // 4. Acquire an active AppShell lock on slot `shell-nav`, then patch `shell-nav` -> Tier 2 LOCKED_RULE_RECEIPT!
    let (st_lock, r_lock) = send_json(
        &router,
        "POST",
        "/api/v1/ssot/appshell/lock",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "portalId": "SalesApp",
            "action": "ACQUIRE",
            "lockOwnerId": "designer-1",
            "expectedRevision": 0,
            "lockedSlots": ["shell-nav"],
            "ttlMs": 60000
        })),
    )
    .await;
    assert_eq!(st_lock, StatusCode::OK, "r_lock={r_lock}");

    let (st3, r3) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "home-dashboard",
            "baseArtifactSha256": rev2_sha,
            "ffId": "shell-nav",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Hacked Nav" },
            "expectedAstRevision": 2,
            "portalId": "SalesApp"
        })),
    )
    .await;
    assert_eq!(st3, StatusCode::OK, "r3={r3}");
    assert_eq!(r3["data"]["applyTier"], "LOCKED_RULE_RECEIPT");
    assert_eq!(
        r3["data"]["lockedRuleReceipt"]["ruleCode"],
        "APPSHELL_LOCK_BOUNDARY"
    );
    assert_eq!(r3["data"]["lockedRuleReceipt"]["requiresReview"], true);
    assert_eq!(r3["data"]["lockedRuleReceipt"]["applied"], false);
    assert!(!r3["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Hacked Nav"));

    // 5. GET /api/v1/patch hydrates persisted SQLite WAL state cleanly; cross-tenant GET is 403
    let (st_get, r_get) = send_json(
        &router,
        "GET",
        &format!("/api/v1/patch?projectId={project_id}&screenId=home-dashboard"),
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_get, StatusCode::OK, "r_get={r_get}");
    assert_eq!(r_get["data"]["astRevision"], 3);
    assert_eq!(r_get["data"]["patches"].as_array().unwrap().len(), 3);

    let (st_cross, _) = send_json(
        &router,
        "GET",
        &format!("/api/v1/patch?projectId={project_id}&screenId=home-dashboard"),
        &other_tenant_tok,
        None,
    )
    .await;
    assert_eq!(st_cross, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn it_05_4_03_regressions_hero_card_starter_next_html_mxss_missing_ffid_and_multi_undo() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok, _other) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "reg-proj").await;
    let empty_sha = compute_sha256_hex(b"");

    // CRITICAL-3: First STEERING_CHIP click on ffId="hero-card" without baseHtml MUST succeed (200 OK, no duplicate data-ff-id)
    let (st1, r1) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": empty_sha,
            "ffId": "hero-card",
            "opKind": "STEERING_CHIP",
            "payloadJson": { "chipId": "more-whitespace" },
            "expectedAstRevision": 0
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "r1={r1}");
    let sha1 = r1["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // CRITICAL-2: Malicious nextHtml on UNDO or LOCKED_RULE_RECEIPT must NEVER bypass sanitizer
    let (st_xss, r_xss) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": sha1,
            "ffId": "hero-card",
            "opKind": "UNDO",
            "payloadJson": {},
            "expectedAstRevision": 1,
            "nextHtml": "<div data-ff-id=\"hero-card\"><script>alert(1)</script></div>"
        })),
    )
    .await;
    assert_eq!(st_xss, StatusCode::UNPROCESSABLE_ENTITY, "r_xss={r_xss}");
    assert_eq!(r_xss["error"]["code"], "E_MXSS_BLOCKED");

    // HIGH-3a: Missing ffId must return 409 STALE_MAPPING (never silent 200 no-op)
    let (st_miss, r_miss) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": sha1,
            "ffId": "nonexistent-node-99",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Ghost" },
            "expectedAstRevision": 1
        })),
    )
    .await;
    assert_eq!(st_miss, StatusCode::CONFLICT, "r_miss={r_miss}");
    assert_eq!(r_miss["error"]["code"], "STALE_MAPPING");

    // HIGH-3b: Multi-step monotonic UNDO (Rev 1 -> Rev 2 -> Rev 3 -> UNDO -> Rev 2 -> UNDO -> Rev 1)
    let (st2, r2) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": sha1,
            "ffId": "hero-title",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Step Two" },
            "expectedAstRevision": 1
        })),
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    let sha2 = r2["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    let (st3, r3) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": sha2,
            "ffId": "hero-title",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Step Three" },
            "expectedAstRevision": 2
        })),
    )
    .await;
    assert_eq!(st3, StatusCode::OK);
    let sha3 = r3["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    let (st_u1, r_u1) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": sha3,
            "ffId": "hero-title",
            "opKind": "UNDO",
            "payloadJson": {},
            "expectedAstRevision": 3
        })),
    )
    .await;
    assert_eq!(st_u1, StatusCode::OK);
    assert!(r_u1["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Step Two"));
    let sha4 = r_u1["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    let (st_u2, r_u2) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "fresh-screen",
            "baseArtifactSha256": sha4,
            "ffId": "hero-title",
            "opKind": "UNDO",
            "payloadJson": {},
            "expectedAstRevision": 4
        })),
    )
    .await;
    assert_eq!(st_u2, StatusCode::OK);
    assert!(!r_u2["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Step Two"));
    assert!(r_u2["data"]["htmlSnapshot"]
        .as_str()
        .unwrap()
        .contains("Workspace Screen"));
}

#[tokio::test]
async fn it_05_4_04_round2_regressions_all_cms_slots_and_undo_edit_undo_no_resurrection() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, _viewer_tok, _other) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "r2-reg-proj").await;

    // 1. CRITICAL-NEW-1: Editing multiple real App CMS slots + sec-header steering chip in sequence without baseHtml
    let zero_sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let (s1, b1) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": zero_sha,
            "ffId": "ff-slide-headline",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Title A" },
            "expectedAstRevision": 0
        })),
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "headline edit failed: {b1}");
    let sha1 = b1["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    let (s2, b2) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": sha1,
            "ffId": "ff-slide-subtitle",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Subtitle B" },
            "expectedAstRevision": 1
        })),
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "subtitle edit must not 409: {b2}");
    let sha2 = b2["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    let (s3, b3) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": sha2,
            "ffId": "ff-action-cta",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Launch C" },
            "expectedAstRevision": 2
        })),
    )
    .await;
    assert_eq!(s3, StatusCode::OK, "cta edit must not 409: {b3}");
    let sha3 = b3["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. MEDIUM-2: UNDO (rev 4 -> restores rev 2) -> Edit D (rev 5) -> UNDO (rev 6 -> must restore rev 2, NOT rev 3!)
    let (su1, bu1) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": sha3,
            "ffId": "ff-action-cta",
            "opKind": "UNDO",
            "payloadJson": {},
            "expectedAstRevision": 3
        })),
    )
    .await;
    assert_eq!(su1, StatusCode::OK);
    let html_u1 = bu1["data"]["htmlSnapshot"].as_str().unwrap();
    assert!(html_u1.contains("Subtitle B"));
    assert!(!html_u1.contains("Launch C"));
    let sha4 = bu1["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    let (s5, b5) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": sha4,
            "ffId": "ff-kpi-1-val",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "$99,000" },
            "expectedAstRevision": 4
        })),
    )
    .await;
    assert_eq!(s5, StatusCode::OK);
    let sha5 = b5["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // Also acquire an active AppShell lock to create an unapplied Tier-2 receipt (rev 6) and ensure UNDO (rev 7) skips it!
    let (st_lock, r_lock) = send_json(
        &router,
        "POST",
        "/api/v1/ssot/appshell/lock",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "portalId": "SalesApp",
            "action": "ACQUIRE",
            "lockOwnerId": "designer-1",
            "expectedRevision": 0,
            "lockedSlots": ["ff-action-cta"],
            "ttlMs": 60000
        })),
    )
    .await;
    assert_eq!(st_lock, StatusCode::OK, "lock acquire failed: {r_lock}");
    let (s6, b6) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": sha5,
            "ffId": "ff-action-cta",
            "opKind": "TEXT_EDIT",
            "payloadJson": { "text": "Blocked Locked Edit" },
            "expectedAstRevision": 5
        })),
    )
    .await;
    assert_eq!(s6, StatusCode::OK);
    assert_eq!(b6["data"]["lockedRuleReceipt"]["applied"], json!(false));
    let sha6 = b6["data"]["resultArtifactSha256"]
        .as_str()
        .unwrap()
        .to_string();

    // UNDO after rev 6 (unapplied) must pop rev 5 ($99,000) and restore rev 2 (Subtitle B, NOT Launch C!)
    let (su2, bu2) = send_json(
        &router,
        "POST",
        "/api/v1/patch",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "screenId": "cms-slots-screen",
            "baseArtifactSha256": sha6,
            "ffId": "ff-kpi-1-val",
            "opKind": "UNDO",
            "payloadJson": {},
            "expectedAstRevision": 6
        })),
    )
    .await;
    assert_eq!(su2, StatusCode::OK);
    let html_u2 = bu2["data"]["htmlSnapshot"].as_str().unwrap();
    assert!(html_u2.contains("Subtitle B"), "must restore rev 2");
    assert!(!html_u2.contains("$99,000"), "must pop rev 5");
    assert!(
        !html_u2.contains("Launch C"),
        "must NEVER resurrect undone rev 3"
    );
}
