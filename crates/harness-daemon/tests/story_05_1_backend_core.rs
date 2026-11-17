// @story 05.1
// @implements AC-01, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use constraint_engine::{ActiveConstraint, AstLinter};
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState, ProjectDto};
use http_body_util::BodyExt;
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
    let tok = format!("sess-051-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok)
}

fn add_tenant_session(state: &Arc<DaemonAppState>, tenant_id: &str, user_id: &str) -> String {
    let tok = format!("sess-051-{}", uuid::Uuid::new_v4());
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
    headers: &[(&str, &str)],
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
    for (k, v) in headers {
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

async fn create_project(router: &axum::Router, tok: &str, slug: &str) -> ProjectDto {
    let (st, val) = send_req(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        &[],
        Some(json!({
            "slug": slug,
            "name": format!("Project {slug}"),
            "product_brief": "Story 05.1 integration suite"
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "project create: {val}");
    serde_json::from_value(val).unwrap()
}

#[tokio::test]
async fn story_05_1_loopback_vs_dag_cycle_and_cas() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-dag-01").await;
    let proj_id = proj.id.to_string();

    // 1. Seed two nodes
    let node1_id = uuid::Uuid::new_v4().to_string();
    let node2_id = uuid::Uuid::new_v4().to_string();
    {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, revision, updated_at)
             VALUES (?1, 'tenant-a', ?2, 'default', 'Main Flow', 1, 1, 1, ?3)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), proj_id, now_ms],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, revision, created_at, updated_at)
             VALUES (?1, (SELECT id FROM flow_graphs WHERE project_id = ?2), 'tenant-a', ?2, 'default', 'home', 'Home', 'screen', '/', 1, ?3, ?3)",
            rusqlite::params![node1_id, proj_id, now_ms],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, revision, created_at, updated_at)
             VALUES (?1, (SELECT id FROM flow_graphs WHERE project_id = ?2), 'tenant-a', ?2, 'default', 'settings', 'Settings', 'screen', '/settings', 1, ?3, ?3)",
            rusqlite::params![node2_id, proj_id, now_ms],
        )
        .unwrap();
    }

    // 2. Add transition node1 -> node2 (main)
    let tr_uri = format!("/api/v1/projects/{proj_id}/flow/transitions");
    let (st1, res1) = send_req(
        &router,
        "POST",
        &tr_uri,
        &tok,
        &[],
        Some(json!({
            "sourceNodeId": node1_id,
            "targetNodeId": node2_id,
            "flowType": "main",
            "triggerEvent": "onClick"
        })),
    )
    .await;
    assert_eq!(st1, StatusCode::CREATED, "create edge node1->node2: {res1}");
    let edge1_id = res1["id"].as_str().unwrap().to_string();

    // 3. Self-loop: node1 -> node1 must be rejected with 400 VALIDATION_ERROR
    let (st_self, res_self) = send_req(
        &router,
        "POST",
        &tr_uri,
        &tok,
        &[],
        Some(json!({
            "sourceNodeId": node1_id,
            "targetNodeId": node1_id,
            "flowType": "main",
            "triggerEvent": "onClick"
        })),
    )
    .await;
    assert_eq!(st_self, StatusCode::BAD_REQUEST);
    assert_eq!(res_self["code"], "VALIDATION_ERROR");

    // 4. Normal back-edge forming cycle (node2 -> node1, flowType: "main") must be rejected with 422 DAG_CYCLE_DETECTED
    let (st_cycle, res_cycle) = send_req(
        &router,
        "POST",
        &tr_uri,
        &tok,
        &[],
        Some(json!({
            "sourceNodeId": node2_id,
            "targetNodeId": node1_id,
            "flowType": "main",
            "triggerEvent": "onClick"
        })),
    )
    .await;
    assert_eq!(st_cycle, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res_cycle["code"], "DAG_CYCLE_DETECTED");

    // 5. Loopback edge without explicit allowConfirmedLoopback must fail with 422 UNCONFIRMED_LOOPBACK
    let (st_unconf, res_unconf) = send_req(
        &router,
        "POST",
        &tr_uri,
        &tok,
        &[],
        Some(json!({
            "sourceNodeId": node2_id,
            "targetNodeId": node1_id,
            "flowType": "loopback",
            "maxLoopIterations": 3,
            "triggerEvent": "onRetry"
        })),
    )
    .await;
    assert_eq!(st_unconf, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res_unconf["code"], "UNCONFIRMED_LOOPBACK");

    // 6. Loopback edge with invalid maxLoopIterations (15 > 10) must fail with 400 VALIDATION_ERROR
    let (st_max_iter, res_max_iter) = send_req(
        &router,
        "POST",
        &tr_uri,
        &tok,
        &[],
        Some(json!({
            "sourceNodeId": node2_id,
            "targetNodeId": node1_id,
            "flowType": "loopback",
            "maxLoopIterations": 15,
            "allowConfirmedLoopback": true,
            "triggerEvent": "onRetry"
        })),
    )
    .await;
    assert_eq!(st_max_iter, StatusCode::BAD_REQUEST);
    assert_eq!(res_max_iter["code"], "VALIDATION_ERROR");

    // 7. Loopback edge with allowConfirmedLoopback: true & maxLoopIterations: 3 succeeds without DAG cycle error!
    let (st_loopback, res_loopback) = send_req(
        &router,
        "POST",
        &tr_uri,
        &tok,
        &[],
        Some(json!({
            "sourceNodeId": node2_id,
            "targetNodeId": node1_id,
            "flowType": "loopback",
            "maxLoopIterations": 3,
            "allowConfirmedLoopback": true,
            "triggerEvent": "onRetry"
        })),
    )
    .await;
    assert_eq!(
        st_loopback,
        StatusCode::CREATED,
        "loopback edge should succeed: {res_loopback}"
    );
    assert_eq!(res_loopback["flowType"], "loopback");
    assert_eq!(res_loopback["maxLoopIterations"], 3);
    assert_eq!(res_loopback["revision"], 1);

    // 8. CAS update on edge: mismatched expectedRevision returns 409 STALE_REVISION
    let patch_uri = format!("/api/v1/projects/{proj_id}/flow/transitions/{edge1_id}");
    let (st_stale, res_stale) = send_req(
        &router,
        "PATCH",
        &patch_uri,
        &tok,
        &[],
        Some(json!({
            "triggerEvent": "onHover",
            "expectedRevision": 99
        })),
    )
    .await;
    assert_eq!(st_stale, StatusCode::CONFLICT);
    assert_eq!(res_stale["code"], "STALE_REVISION");

    // 9. CAS update on edge with correct expectedRevision (1) succeeds and increments revision to 2
    let (st_cas_ok, res_cas_ok) = send_req(
        &router,
        "PATCH",
        &patch_uri,
        &tok,
        &[],
        Some(json!({
            "triggerEvent": "onHover",
            "expectedRevision": 1
        })),
    )
    .await;
    assert_eq!(st_cas_ok, StatusCode::OK);
    assert_eq!(res_cas_ok["triggerEvent"], "onHover");
    assert_eq!(res_cas_ok["revision"], 2);
}

#[tokio::test]
async fn story_05_1_two_phase_idempotency_and_sdi_grounding() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let proj = create_project(&router, &tok, "proj-sdi-01").await;
    let proj_id = proj.id.to_string();

    // 1. Seed home screen node
    let node_id = uuid::Uuid::new_v4().to_string();
    {
        let conn = state.registry.pool().connection().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO flow_graphs (id, tenant_id, project_id, portal_key, title, version, layout_version, revision, updated_at)
             VALUES (?1, 'tenant-a', ?2, 'default', 'Default Flow', 1, 1, 1, ?3)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), proj_id, now_ms],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO flow_nodes (id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type, route_path, revision, created_at, updated_at)
             VALUES (?1, (SELECT id FROM flow_graphs WHERE project_id = ?2), 'tenant-a', ?2, 'default', 'home', 'Trang Chủ', 'screen', '/', 1, ?3, ?3)",
            rusqlite::params![node_id, proj_id, now_ms],
        )
        .unwrap();
    }

    let sdi_uri = format!("/api/v1/projects/{proj_id}/sdi/intent");

    // 2. Vague prompt triggers ambiguity_score >= 0.45, routing_mode = "clarification_required", 1-3 gap questions
    let (st_vague, res_vague) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok,
        &[("idempotency-key", "idemp-vague-01")],
        Some(json!({
            "rawPrompt": "làm đẹp giao diện"
        })),
    )
    .await;
    assert_eq!(
        st_vague,
        StatusCode::CREATED,
        "vague sdi compile: {res_vague}"
    );
    assert_eq!(res_vague["status"], "clarifying");
    assert_eq!(res_vague["routingMode"], "clarification_required");
    assert!(res_vague["ambiguityScore"].as_f64().unwrap() >= 0.45);
    let gap_q = res_vague["gapQuestions"].as_array().expect("gapQuestions");
    assert!(!gap_q.is_empty() && gap_q.len() <= 3);
    let variants = res_vague["clarificationVariants"]
        .as_array()
        .expect("variants");
    assert_eq!(variants.len(), 3);
    let intent_id = res_vague["id"].as_str().unwrap().to_string();

    // 3. Replay with identical payload & Idempotency-Key returns cached response
    let (st_replay, res_replay) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok,
        &[("idempotency-key", "idemp-vague-01")],
        Some(json!({
            "rawPrompt": "làm đẹp giao diện"
        })),
    )
    .await;
    assert_eq!(st_replay, StatusCode::CREATED);
    assert_eq!(res_replay["id"], intent_id);

    // 4. Reuse same Idempotency-Key with different payload triggers 422 IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_PAYLOAD
    let (st_mismatch, res_mismatch) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok,
        &[("idempotency-key", "idemp-vague-01")],
        Some(json!({
            "rawPrompt": "thay đổi khác hẳn payload cũ"
        })),
    )
    .await;
    assert_eq!(st_mismatch, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res_mismatch["code"],
        "IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_PAYLOAD"
    );

    // 5. Vague prompt with executeImmediately: true triggers 409 CLARIFICATION_REQUIRED and cleans up TX1 idempotency lease
    let (st_clar_req, res_clar_req) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok,
        &[("idempotency-key", "idemp-clar-abort-01")],
        Some(json!({
            "rawPrompt": "fix giao diện",
            "executeImmediately": true
        })),
    )
    .await;
    assert_eq!(st_clar_req, StatusCode::CONFLICT);
    assert_eq!(res_clar_req["code"], "CLARIFICATION_REQUIRED");

    let (st_after_abort, res_after_abort) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok,
        &[("idempotency-key", "idemp-clar-abort-01")],
        Some(json!({
            "rawPrompt": "fix giao diện"
        })),
    )
    .await;
    assert_eq!(
        st_after_abort,
        StatusCode::CREATED,
        "lease should be cleaned up on early 409 abort: {res_after_abort}"
    );

    // 6. Targeted prompt on existing node produces direct_edit & ambiguityScore < 0.45
    let (st_target, res_target) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok,
        &[],
        Some(json!({
            "rawPrompt": "đổi màu nút submit trong header",
            "targetNodeId": node_id
        })),
    )
    .await;
    assert_eq!(st_target, StatusCode::CREATED);
    assert_eq!(res_target["status"], "ready");
    assert_eq!(res_target["routingMode"], "direct_edit");
    assert!(res_target["ambiguityScore"].as_f64().unwrap() < 0.45);

    // 7. Select clarification for intent transitions status to ready and increments revision
    let clar_uri = format!("/api/v1/projects/{proj_id}/sdi/clarification");
    let (st_select, res_select) = send_req(
        &router,
        "POST",
        &clar_uri,
        &tok,
        &[],
        Some(json!({
            "intentId": intent_id,
            "selectedVariantId": "var-1-minimal"
        })),
    )
    .await;
    assert_eq!(st_select, StatusCode::OK);
    assert_eq!(res_select["status"], "ready");
    assert_eq!(res_select["routingMode"], "direct_edit");
    assert_eq!(res_select["revision"], 2);
}

#[tokio::test]
async fn story_05_1_cross_tenant_anti_idor_and_fixture_cas() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok_tenant_a) = setup_daemon(&tmp);
    let tok_tenant_b = add_tenant_session(&state, "tenant-b", "user-b");

    let proj = create_project(&router, &tok_tenant_a, "proj-tenant-a").await;
    let proj_id = proj.id.to_string();
    let node_id = "node-scr-fix-01";

    // 1. Create screen fixture under tenant-a
    let fix_uri = format!("/api/v1/projects/{proj_id}/flow/nodes/{node_id}/fixtures");
    let (st_create, res_create) = send_req(
        &router,
        "POST",
        &fix_uri,
        &tok_tenant_a,
        &[],
        Some(json!({
            "variant": "DEFAULT",
            "presetKey": "happy",
            "payloadJson": "{\"users\":[{\"name\":\"Alice\"}]}",
            "sampleRowsJson": "[{\"name\":\"Alice\"}]"
        })),
    )
    .await;
    assert_eq!(
        st_create,
        StatusCode::CREATED,
        "create fixture: {res_create}"
    );
    let fixture_id = res_create["id"].as_str().unwrap().to_string();
    assert_eq!(res_create["presetKey"], "happy");
    assert_eq!(res_create["revision"], 1);

    // 2. Cross-tenant anti-IDOR: tenant-b accessing tenant-a's project/fixtures returns 404 NOT_FOUND (never 403)
    let single_fix_uri =
        format!("/api/v1/projects/{proj_id}/flow/nodes/{node_id}/fixtures/{fixture_id}");
    let (st_cross, _) = send_req(&router, "GET", &single_fix_uri, &tok_tenant_b, &[], None).await;
    assert_eq!(
        st_cross,
        StatusCode::NOT_FOUND,
        "cross-tenant fixture access must be 404"
    );

    let sdi_uri = format!("/api/v1/projects/{proj_id}/sdi/intent");
    let (st_cross_sdi, _) = send_req(
        &router,
        "POST",
        &sdi_uri,
        &tok_tenant_b,
        &[],
        Some(json!({ "rawPrompt": "test" })),
    )
    .await;
    assert_eq!(
        st_cross_sdi,
        StatusCode::NOT_FOUND,
        "cross-tenant sdi access must be 404"
    );

    // 3. Stale revision CAS on ScreenFixture: expectedRevision: 99 returns 409 STALE_REVISION with currentRevision
    let (st_fix_stale, res_fix_stale) = send_req(
        &router,
        "PATCH",
        &single_fix_uri,
        &tok_tenant_a,
        &[],
        Some(json!({
            "presetKey": "empty_state",
            "expectedRevision": 99
        })),
    )
    .await;
    assert_eq!(st_fix_stale, StatusCode::CONFLICT);
    assert_eq!(res_fix_stale["code"], "STALE_REVISION");
    assert_eq!(res_fix_stale["currentRevision"], 1);

    // 4. Valid CAS on ScreenFixture increments revision to 2
    let (st_fix_ok, res_fix_ok) = send_req(
        &router,
        "PATCH",
        &single_fix_uri,
        &tok_tenant_a,
        &[],
        Some(json!({
            "presetKey": "empty_state",
            "expectedRevision": 1
        })),
    )
    .await;
    assert_eq!(st_fix_ok, StatusCode::OK);
    assert_eq!(res_fix_ok["presetKey"], "empty_state");
    assert_eq!(res_fix_ok["revision"], 2);
}

#[test]
fn story_05_1_ast_linter_extended_classname_and_cn() {
    let rules = vec![ActiveConstraint {
        rule_code: "BAN_TAILWIND".to_string(),
        severity: "BLOCKER".to_string(),
        config_json: "{}".to_string(),
    }];

    // 1. JSX with *ClassName and *Class attributes
    let tsx_attrs = r#"
        export const StoryboardCard = () => (
            <div containerClassName="flex flex-col items-center" headerClass="p-4 bg-white" buttonClassName="rounded-lg">
                <span badgeClass="inline-flex py-1">Active</span>
            </div>
        );
    "#;
    let v_attrs = AstLinter::lint_source(tsx_attrs, &rules).expect("lint attrs");
    assert!(v_attrs.iter().any(|v| v.message.contains("flex")));
    assert!(v_attrs.iter().any(|v| v.message.contains("p-4")));
    assert!(v_attrs.iter().any(|v| v.message.contains("rounded-lg")));
    assert!(v_attrs.iter().any(|v| v.message.contains("inline-flex")));

    // 2. CallExpressions for cn(...) and clsx(...)
    let tsx_cn = r#"
        import { cn } from "@/lib/utils";
        import clsx from "clsx";
        const btnCls = cn("flex p-2", isHovered && "bg-blue-500");
        const cardCls = clsx("grid gap-4", "shadow-sm");
    "#;
    let v_cn = AstLinter::lint_source(tsx_cn, &rules).expect("lint cn");
    assert!(v_cn.iter().any(|v| v.message.contains("flex")));
    assert!(v_cn.iter().any(|v| v.message.contains("p-2")));
    assert!(v_cn.iter().any(|v| v.message.contains("bg-blue-500")));
    assert!(v_cn.iter().any(|v| v.message.contains("grid")));
    assert!(v_cn.iter().any(|v| v.message.contains("gap-4")));
}
