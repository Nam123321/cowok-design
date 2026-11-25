// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon Integration Tests: V31 Persistence, Generation Epoch 409 Fencing, Cross-Variant Borrow + html_forge::reconcile_ff_ids, /crystallize & advisor Kernel Guards 403)
use std::fs;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use chrono::Utc;
use harness_daemon::daemon_auth::{DaemonAuthState, FlowPrincipal};
use harness_daemon::routes::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::{json, Value};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct TournamentHarness {
    _dir: tempfile::TempDir,
    registry: Arc<SpecRegistry>,
    app: Router,
    project_id: String,
    editor_a_tok: String,
    viewer_a_tok: String,
    editor_b_tok: String,
}

fn setup_harness() -> TournamentHarness {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("skills/design-craft")).expect("mkdir skills");
    fs::create_dir_all(dir.path().join("tests/eval/sealed")).expect("mkdir sealed");
    fs::write(dir.path().join("tests/eval/sealed/suite.json"), "{}").expect("seed sealed");

    let db_path = dir.path().join(".cowok-design").join("daemon-103.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let project_id = uuid::Uuid::new_v4().to_string();

    {
        let conn = registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES (?1, 'tenant-a', 'proj-103', 'Tournament Proj', ?2, '', 'ACTIVE', 1000, 1000)",
            params![project_id, dir.path().to_string_lossy().as_ref()],
        )
        .expect("seed project");
    }

    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_a_tok = format!("cwk_sess_103_ed_a_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        editor_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "user-a".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_a_tok = format!("cwk_sess_103_vw_a_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        viewer_a_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".to_string(),
            user_id: "viewer-a".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let editor_b_tok = format!("cwk_sess_103_ed_b_{}", uuid::Uuid::new_v4().simple());
    auth.insert_session(
        editor_b_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-b".to_string(),
            user_id: "user-b".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let state =
        DaemonAppState::with_auth(Arc::clone(&registry), "127.0.0.1".to_string(), 4172, auth);
    let _ = harness_daemon::routes::orchestrator::shared_write_actor_for_pool(registry.pool());
    let app = build_router(state);

    TournamentHarness {
        _dir: dir,
        registry,
        app,
        project_id,
        editor_a_tok,
        viewer_a_tok,
        editor_b_tok,
    }
}

async fn send_json(
    app: &Router,
    method: Method,
    uri: &str,
    session_token: &str,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4172")
        .header("x-cowok-session-token", session_token);
    let body = if let Some(val) = payload {
        builder = builder.header("content-type", "application/json");
        Body::from(val.to_string())
    } else {
        Body::empty()
    };
    let req = builder.body(body).expect("build request");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let json_val: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
    (status, json_val)
}

#[tokio::test]
async fn tournament_start_select_borrow_reconcile_and_epoch_fencing_end_to_end() {
    let h = setup_harness();

    // 1. Start 4-axis tournament
    let (status, start_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/start",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "prompt": "Executive operations dashboard with fast KPI scanning"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={start_body}");
    assert_eq!(start_body["ok"], true);
    assert_eq!(start_body["data"]["generationEpoch"], 1);
    let variants = start_body["data"]["variants"].as_array().expect("variants");
    assert_eq!(variants.len(), 4);
    assert_eq!(variants[0]["displayLabel"], "Option A");
    assert_eq!(variants[1]["displayLabel"], "Option B");
    assert_eq!(
        start_body["data"]["rubricSummary"]["allPairsSwapConsistent"],
        true
    );

    let run_id = start_body["data"]["runId"].as_str().expect("runId");
    let winner_id = variants[0]["id"].as_str().expect("winner_id");
    let donor_id = variants[1]["id"].as_str().expect("donor_id");

    // 2. Commit winner with Cross-Variant Borrow at generationEpoch = 3 (rapid A -> B -> C final choice)
    let (sel_status, sel_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/select",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "runId": run_id,
            "winningVariantId": winner_id,
            "generationEpoch": 3,
            "borrowFromVariantId": donor_id,
            "borrowSelectorOrInstruction": "ff-rail-navigation"
        })),
    )
    .await;
    assert_eq!(sel_status, StatusCode::OK, "body={sel_body}");
    assert_eq!(sel_body["data"]["generationEpoch"], 3);
    assert_eq!(sel_body["data"]["duplicateFfIdsAfterMerge"], 0);
    assert!(sel_body["data"]["borrowedFfId"].as_str().is_some());
    assert_eq!(
        sel_body["data"]["archivedVariantIds"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        sel_body["data"]["memoryEventIds"].as_array().unwrap().len(),
        2
    );

    // 3. Stale out-of-order commit with generationEpoch = 2 MUST return 409 STALE_GENERATION_EPOCH
    let (stale_status, stale_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/select",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "runId": run_id,
            "winningVariantId": donor_id,
            "generationEpoch": 2
        })),
    )
    .await;
    assert_eq!(stale_status, StatusCode::CONFLICT, "body={stale_body}");
    assert_eq!(stale_body["error"]["code"], "STALE_GENERATION_EPOCH");

    // 4. List runs via GET /api/v1/tournament/runs (viewer permitted, cross-tenant blocked)
    let (list_status, list_body) = send_json(
        &h.app,
        Method::GET,
        &format!("/api/v1/tournament/runs?projectId={}", h.project_id),
        &h.viewer_a_tok,
        None,
    )
    .await;
    assert_eq!(list_status, StatusCode::OK, "body={list_body}");
    assert_eq!(list_body["data"]["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list_body["data"]["runs"][0]["winningVariantId"], winner_id);

    let (idor_status, _) = send_json(
        &h.app,
        Method::GET,
        &format!("/api/v1/tournament/runs?projectId={}", h.project_id),
        &h.editor_b_tok,
        None,
    )
    .await;
    assert_eq!(idor_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn crystallize_and_advisor_evolve_enforce_security_guards_and_persist_memory() {
    let h = setup_harness();

    // Seed a recurring gate_failure_ledger cluster (hit_count = 4 >= 3)
    {
        let conn = h.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO gate_failure_ledger (id, tenant_id, project_id, rule_id, fingerprint_sha256, sample_message, hit_count, slpr_score, status, last_seen_at)
             VALUES ('led-103', 'tenant-a', ?1, 'palette_contrast_fix', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'Button label contrast raised to 4.8:1', 4, 0.96, 'PROPOSED', 2000)",
            params![h.project_id],
        )
        .expect("seed ledger");
    }

    // 1. /crystallize packages STARTER_TEMPLATE and logs CRYSTALLIZE event
    let (cryst_status, cryst_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/crystallize",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "packageKind": "STARTER_TEMPLATE",
            "name": "Executive Ops Kit",
            "description": "Reusable executive KPI starter",
            "htmlSnapshot": "<main><h1>Executive Ops</h1></main>"
        })),
    )
    .await;
    assert_eq!(cryst_status, StatusCode::OK, "body={cryst_body}");
    assert_eq!(cryst_body["data"]["packageKind"], "STARTER_TEMPLATE");
    assert!(cryst_body["data"]["memoryEventId"].as_str().is_some());

    // 2. /advisor/evolve happy path consumes memory + ledger cluster (>=3) and stages HUMAN_ONLY draft
    let (adv_status, adv_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/advisor/evolve",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "targetSkillPath": "skills/design-craft/SKILL.md",
            "referencePaths": ["skills/design-craft/references/contrast.md"]
        })),
    )
    .await;
    assert_eq!(adv_status, StatusCode::OK, "body={adv_body}");
    assert_eq!(adv_body["data"]["stagingStatus"], "STAGING_DRAFT");
    assert_eq!(adv_body["data"]["approvalPolicy"], "HUMAN_ONLY");
    assert_eq!(adv_body["data"]["promotedToProduction"], false);
    assert_eq!(adv_body["data"]["sealedEvalUntouched"], true);
    assert_eq!(adv_body["data"]["studioCorrectionsConsumed"], 1);
    assert_eq!(adv_body["data"]["signalsConsumed"], 1);
    assert_eq!(adv_body["data"]["steps"].as_array().unwrap().len(), 6);

    // 3. Kernel Guard 1: targeting tests/eval/sealed/ -> 403 SEALED_EVAL_TAMPER_ERROR
    let (tamper_status, tamper_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/advisor/evolve",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "targetSkillPath": "skills/../tests/eval/sealed/suite.json"
        })),
    )
    .await;
    assert_eq!(tamper_status, StatusCode::FORBIDDEN, "body={tamper_body}");
    assert_eq!(tamper_body["error"]["code"], "SEALED_EVAL_TAMPER_ERROR");

    // 4. Kernel Guard 2: promoteDirectly = true -> 403 HUMAN_APPROVAL_REQUIRED
    let (promote_status, promote_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/advisor/evolve",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "targetSkillPath": "skills/design-craft/SKILL.md",
            "promoteDirectly": true
        })),
    )
    .await;
    assert_eq!(promote_status, StatusCode::FORBIDDEN, "body={promote_body}");
    assert_eq!(promote_body["error"]["code"], "HUMAN_APPROVAL_REQUIRED");
}

#[tokio::test]
async fn stage3_p0_p1_regressions_xss_equal_epoch_immutable_base_idor_and_advisor_hardening() {
    let h = setup_harness();

    // Seed a second project in the same tenant to verify cross-project IDOR blocking on /crystallize
    let project_b_dir = tempdir().expect("tempdir B");
    let project_b_id = uuid::Uuid::new_v4().to_string();
    {
        let conn = h.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
             VALUES (?1, 'tenant-a', 'proj-103-b', 'Second Proj', ?2, '', 'ACTIVE', 1000, 1000)",
            params![project_b_id, project_b_dir.path().to_string_lossy().as_ref()],
        )
        .expect("seed project B");
    }

    // 1. P0 XSS / CSS sanitization on branchOverrides.initialHtml & rerollHtmls -> HTTP 422
    let (xss_init_status, xss_init_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/start",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "prompt": "Security test",
            "branchOverrides": [{
                "axis": "INFORMATION_DENSITY",
                "initialHtml": "<main><script>alert(1)</script></main>"
            }]
        })),
    )
    .await;
    assert_eq!(
        xss_init_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "body={xss_init_body}"
    );
    assert_eq!(xss_init_body["error"]["code"], "CSS_SECURITY_BLOCKED");

    let (css_reroll_status, css_reroll_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/start",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "prompt": "CSS security test",
            "branchOverrides": [{
                "axis": "NAVIGATION_TOPOLOGY",
                "rerollHtmls": ["<style>@import url('https://evil.example/a.css');</style><main><p>x</p></main>"]
            }]
        })),
    )
    .await;
    assert_eq!(
        css_reroll_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "body={css_reroll_body}"
    );
    assert_eq!(css_reroll_body["error"]["code"], "CSS_SECURITY_BLOCKED");

    // 2. Start clean run, commit at epoch 3, verify:
    //    (a) immutable base `<winner_id>.html` is NOT overwritten
    //    (b) epoch-scoped `<winner_id>.epoch-3.merged.html` IS created
    //    (c) equal-epoch replay (`generationEpoch: 3`) returns 409 STALE_GENERATION_EPOCH
    let (start_status, start_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/start",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "prompt": "Immutable base & strict epoch check"
        })),
    )
    .await;
    assert_eq!(start_status, StatusCode::OK, "body={start_body}");
    let run_id = start_body["data"]["runId"].as_str().unwrap();
    let winner_id = start_body["data"]["variants"][0]["id"].as_str().unwrap();
    let donor_id = start_body["data"]["variants"][1]["id"].as_str().unwrap();

    let base_path = h
        ._dir
        .path()
        .join(".cowok-design/tournaments")
        .join(run_id)
        .join(format!("{winner_id}.html"));
    let base_html_before = fs::read_to_string(&base_path).expect("read immutable base");

    let (sel_status, sel_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/select",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "runId": run_id,
            "winningVariantId": winner_id,
            "generationEpoch": 3,
            "borrowFromVariantId": donor_id,
            "borrowSelectorOrInstruction": "ff-rail-navigation"
        })),
    )
    .await;
    assert_eq!(sel_status, StatusCode::OK, "body={sel_body}");

    let base_html_after = fs::read_to_string(&base_path).expect("re-read immutable base");
    assert_eq!(
        base_html_before, base_html_after,
        "immutable base <variant_id>.html must never be overwritten"
    );
    let epoch_merged_path = h
        ._dir
        .path()
        .join(".cowok-design/tournaments")
        .join(run_id)
        .join(format!("{winner_id}.epoch-3.merged.html"));
    assert!(
        epoch_merged_path.is_file(),
        "epoch-scoped merged artifact must exist at {}",
        epoch_merged_path.display()
    );

    // Equal-epoch replay MUST return 409 STALE_GENERATION_EPOCH
    let (eq_status, eq_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/tournament/select",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "runId": run_id,
            "winningVariantId": donor_id,
            "generationEpoch": 3
        })),
    )
    .await;
    assert_eq!(eq_status, StatusCode::CONFLICT, "body={eq_body}");
    assert_eq!(eq_body["error"]["code"], "STALE_GENERATION_EPOCH");

    // 3. P1 Cross-project IDOR & `.` segment on POST /api/v1/crystallize
    let (idor_cryst_status, idor_cryst_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/crystallize",
        &h.editor_a_tok,
        Some(json!({
            "projectId": project_b_id,
            "runId": run_id,
            "variantId": winner_id,
            "packageKind": "STARTER_TEMPLATE",
            "name": "Cross Project Theft"
        })),
    )
    .await;
    assert_eq!(
        idor_cryst_status,
        StatusCode::NOT_FOUND,
        "body={idor_cryst_body}"
    );
    assert_eq!(idor_cryst_body["error"]["code"], "TOURNAMENT_RUN_NOT_FOUND");

    let (dot_status, dot_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/crystallize",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "runId": ".",
            "variantId": winner_id,
            "packageKind": "BRAND_KIT",
            "name": "Dot Segment Attack"
        })),
    )
    .await;
    assert_eq!(dot_status, StatusCode::BAD_REQUEST, "body={dot_body}");
    assert_eq!(dot_body["error"]["code"], "VALIDATION_ERROR");

    // Missing variant HTML artifact on disk must return 404 (never unwrap_or_default empty HTML)
    let missing_var_id = start_body["data"]["variants"][2]["id"].as_str().unwrap();
    let missing_var_path = h
        ._dir
        .path()
        .join(".cowok-design/tournaments")
        .join(run_id)
        .join(format!("{missing_var_id}.html"));
    fs::remove_file(&missing_var_path).expect("remove variant artifact");
    let (miss_status, miss_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/crystallize",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "runId": run_id,
            "variantId": missing_var_id,
            "packageKind": "STARTER_TEMPLATE",
            "name": "Missing File Check"
        })),
    )
    .await;
    assert_eq!(miss_status, StatusCode::NOT_FOUND, "body={miss_body}");
    assert_eq!(miss_body["error"]["code"], "VARIANT_ARTIFACT_NOT_FOUND");

    // XSS in crystallize htmlSnapshot -> 422 CSS_SECURITY_BLOCKED
    let (cryst_xss_status, cryst_xss_body) = send_json(
        &h.app,
        Method::POST,
        "/api/v1/crystallize",
        &h.editor_a_tok,
        Some(json!({
            "projectId": h.project_id,
            "packageKind": "STARTER_TEMPLATE",
            "name": "XSS Snapshot",
            "htmlSnapshot": "<main><img src='x' onerror='alert(1)'></main>"
        })),
    )
    .await;
    assert_eq!(
        cryst_xss_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "body={cryst_xss_body}"
    );
    assert_eq!(cryst_xss_body["error"]["code"], "CSS_SECURITY_BLOCKED");

    // 4. P0 Advisor Kernel Guard: %00, 4x-encoded tamper, non-skill path -> 403
    for bad_path in [
        "skills/design-craft/SKILL.md%00.txt",
        "%2525252e%2525252e/tests/eval/sealed/suite.json",
        "crates/harness-daemon/src/routes/design_tournament.rs",
    ] {
        let (st, bd) = send_json(
            &h.app,
            Method::POST,
            "/api/v1/advisor/evolve",
            &h.editor_a_tok,
            Some(json!({
                "projectId": h.project_id,
                "targetSkillPath": bad_path
            })),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "path={bad_path} body={bd}");
        assert_eq!(bd["error"]["code"], "SEALED_EVAL_TAMPER_ERROR");
    }

    // 5. P0 Advisor staging symlink error MUST propagate 403 and NOT insert ADVISOR_PATCH into SQLite!
    #[cfg(unix)]
    {
        let staging_parent = h._dir.path().join(".cowok-design/staging");
        fs::create_dir_all(&staging_parent).expect("mkdir staging");
        let outside = tempdir().expect("outside dir");
        std::os::unix::fs::symlink(outside.path(), staging_parent.join("design-craft"))
            .expect("symlink staging dir");

        let count_before: i64 = {
            let conn = h.registry.pool().connection().expect("conn");
            conn.query_row(
                "SELECT COUNT(*) FROM creative_memory_events WHERE project_id = ?1 AND event_kind = 'ADVISOR_PATCH'",
                params![h.project_id],
                |r| r.get(0),
            )
            .expect("count before")
        };

        let (sym_status, sym_body) = send_json(
            &h.app,
            Method::POST,
            "/api/v1/advisor/evolve",
            &h.editor_a_tok,
            Some(json!({
                "projectId": h.project_id,
                "targetSkillPath": "skills/design-craft/SKILL.md"
            })),
        )
        .await;
        assert_eq!(sym_status, StatusCode::FORBIDDEN, "body={sym_body}");

        let count_after: i64 = {
            let conn = h.registry.pool().connection().expect("conn");
            conn.query_row(
                "SELECT COUNT(*) FROM creative_memory_events WHERE project_id = ?1 AND event_kind = 'ADVISOR_PATCH'",
                params![h.project_id],
                |r| r.get(0),
            )
            .expect("count after")
        };
        assert_eq!(
            count_before, count_after,
            "ADVISOR_PATCH must NOT be persisted when staging write fails"
        );
    }
}
