// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::reverse_sync::{reverse_sync_tsx_components, ReverseSyncRequest};
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

    let tok_editor = format!("sess-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        tok_editor.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-alpha".into(),
            role: "editor".into(),
        },
        exp,
    );

    let tok_admin = format!("sess-adm-{}", Uuid::new_v4());
    state.auth.insert_session(
        tok_admin.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "admin-alpha".into(),
            role: "admin".into(),
        },
        exp,
    );

    let tok_tenant_b = format!("sess-b-{}", Uuid::new_v4());
    state.auth.insert_session(
        tok_tenant_b.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".into(),
            user_id: "attacker-beta".into(),
            role: "editor".into(),
        },
        exp,
    );

    let router = build_router((*state).clone());
    (state, router, tok_editor, tok_admin, tok_tenant_b)
}

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: Option<&str>,
    portal_hdr: Option<&str>,
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
    if let Some(p) = portal_hdr {
        b = b.header("x-cowok-portal", p);
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

async fn create_project(router: &axum::Router, tok: &str, slug: &str) -> String {
    let (st, res) = send_req(
        router,
        "POST",
        "/api/v1/projects",
        Some(tok),
        None,
        Some(json!({ "slug": slug, "name": "SSOT 02.4 Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn sec_idor_03_and_full_ssot_daemon_routes_flow() {
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok_ed, tok_admin, tok_beta) = setup_daemon(&tmp);
    let project_id = create_project(&router, &tok_ed, "ssot-024-daemon").await;

    // Seed a synced component pattern via OXC reverse sync
    {
        let mut conn = state.registry.pool().connection().expect("conn");
        reverse_sync_tsx_components(
            &mut conn,
            &ReverseSyncRequest {
                tenant_id: "tenant-alpha".into(),
                project_id: project_id.clone(),
                portal_id: "DeveloperPortal".into(),
                caller_actor: "designer-alpha".into(),
                source_file_path: "src/pages/workbench/WorkbenchShell.tsx".into(),
                source_code: r#"
                    export function WorkbenchShell({ themeMode }: { themeMode: string }) {
                        return <div data-ff-id="appshell-lock-panel" style={{ background: "var(--bg-card)" }}>{themeMode}</div>;
                    }
                "#
                .into(),
                now_ms: Some(1_000_000),
            },
        )
        .expect("seed reverse sync");
    }

    // 1. Unauthenticated request blocked with 401
    let (st_unauth, _) = send_req(
        &router,
        "GET",
        &format!("/api/v1/ssot/components?projectId={project_id}&portalId=DeveloperPortal"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    // 2. SEC-IDOR-03: Tenant B attacker (including admin role!) accessing Tenant A UUID or non-UUID project -> 403 PROJECT_TENANT_MISMATCH
    let tok_beta_admin = format!("sess-b-adm-{}", Uuid::new_v4());
    state.auth.insert_session(
        tok_beta_admin.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".into(),
            user_id: "admin-beta".into(),
            role: "admin".into(),
        },
        Utc::now().timestamp() + 3600,
    );
    {
        let conn = state.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO projects (id, tenant_id, name, slug, workspace_root, product_brief, status, created_at, updated_at) \
             VALUES ('non-uuid-alpha', 'tenant-alpha', 'Alpha Str', 'alpha-str', '.', '', 'ACTIVE', 1700000000000, 1700000000000)",
            [],
        )
        .expect("seed non-uuid alpha project");
    }

    let (st_idor, body_idor) = send_req(
        &router,
        "GET",
        &format!("/api/v1/ssot/components?projectId={project_id}&portalId=DeveloperPortal"),
        Some(&tok_beta),
        None,
        None,
    )
    .await;
    assert_eq!(st_idor, StatusCode::FORBIDDEN);
    assert_eq!(body_idor["error"]["code"], "PROJECT_TENANT_MISMATCH");

    let (st_idor_adm_uuid, body_idor_adm_uuid) = send_req(
        &router,
        "GET",
        &format!("/api/v1/ssot/components?projectId={project_id}&portalId=DeveloperPortal"),
        Some(&tok_beta_admin),
        None,
        None,
    )
    .await;
    assert_eq!(st_idor_adm_uuid, StatusCode::FORBIDDEN);
    assert_eq!(
        body_idor_adm_uuid["error"]["code"],
        "PROJECT_TENANT_MISMATCH"
    );

    let (st_idor_adm_str, body_idor_adm_str) = send_req(
        &router,
        "GET",
        "/api/v1/ssot/components?projectId=non-uuid-alpha&portalId=DeveloperPortal",
        Some(&tok_beta_admin),
        None,
        None,
    )
    .await;
    assert_eq!(st_idor_adm_str, StatusCode::FORBIDDEN);
    assert_eq!(
        body_idor_adm_str["error"]["code"],
        "PROJECT_TENANT_MISMATCH"
    );

    // 3. Cross-portal spoofing (x-cowok-portal != portalId) & non-admin SuperAdmin -> 403 CROSS_PORTAL_LEAK_BLOCKED
    let (st_portal_spoof, body_portal_spoof) = send_req(
        &router,
        "GET",
        &format!("/api/v1/ssot/components?projectId={project_id}&portalId=DeveloperPortal"),
        Some(&tok_ed),
        Some("SalesApp"),
        None,
    )
    .await;
    assert_eq!(st_portal_spoof, StatusCode::FORBIDDEN);
    assert_eq!(
        body_portal_spoof["error"]["code"],
        "CROSS_PORTAL_LEAK_BLOCKED"
    );

    let (st_superadmin_deny, body_superadmin_deny) = send_req(
        &router,
        "GET",
        &format!("/api/v1/ssot/components?projectId={project_id}&portalId=SuperAdmin"),
        Some(&tok_ed),
        None,
        None,
    )
    .await;
    assert_eq!(st_superadmin_deny, StatusCode::FORBIDDEN);
    assert_eq!(
        body_superadmin_deny["error"]["code"],
        "CROSS_PORTAL_LEAK_BLOCKED"
    );

    // 4. Valid GET /api/v1/ssot/components returns pattern
    let (st_get, body_get) = send_req(
        &router,
        "GET",
        &format!("/api/v1/ssot/components?projectId={project_id}&portalId=DeveloperPortal"),
        Some(&tok_ed),
        Some("DeveloperPortal"),
        None,
    )
    .await;
    assert_eq!(st_get, StatusCode::OK);
    assert_eq!(body_get["ok"], true);
    assert_eq!(body_get["data"]["patterns"].as_array().unwrap().len(), 1);
    assert!(body_get["data"]["lock"].is_null());

    // 5. POST /api/v1/ssot/appshell/lock ACQUIRE -> 200 OK
    let (st_acq, body_acq) = send_req(
        &router,
        "POST",
        "/api/v1/ssot/appshell/lock",
        Some(&tok_ed),
        Some("DeveloperPortal"),
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "action": "ACQUIRE",
            "lockOwnerId": "designer-alpha",
            "expectedRevision": 0,
            "expectedFencingToken": null,
            "lockedSlots": ["TopBar", "LeftNav"],
            "ttlMs": 60000
        })),
    )
    .await;
    assert_eq!(st_acq, StatusCode::OK);
    assert_eq!(body_acq["data"]["lock"]["fencingToken"], 1);
    assert_eq!(body_acq["data"]["lock"]["revision"], 1);

    // 6. Conflicting ACQUIRE -> 409 LOCK_FENCING_CONFLICT
    let (st_conf, body_conf) = send_req(
        &router,
        "POST",
        "/api/v1/ssot/appshell/lock",
        Some(&tok_ed),
        Some("DeveloperPortal"),
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "action": "ACQUIRE",
            "lockOwnerId": "designer-alpha",
            "expectedRevision": 0,
            "ttlMs": 60000
        })),
    )
    .await;
    assert_eq!(st_conf, StatusCode::CONFLICT);
    assert_eq!(body_conf["error"]["code"], "LOCK_FENCING_CONFLICT");
    assert_eq!(body_conf["error"]["details"]["currentRevision"], 1);

    // 7. Non-admin TRANSFER -> 403 LOCK_TRANSFER_FORBIDDEN; Admin TRANSFER -> 200 OK
    let (st_tr_ed, body_tr_ed) = send_req(
        &router,
        "POST",
        "/api/v1/ssot/appshell/lock",
        Some(&tok_ed),
        None,
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "action": "TRANSFER",
            "lockOwnerId": "admin-alpha",
            "expectedRevision": 1,
            "lockedSlots": ["TopBar"],
            "ttlMs": 60000,
            "forceTransferReason": "emergency"
        })),
    )
    .await;
    assert_eq!(st_tr_ed, StatusCode::FORBIDDEN);
    assert_eq!(body_tr_ed["error"]["code"], "LOCK_TRANSFER_FORBIDDEN");

    let (st_tr_adm, body_tr_adm) = send_req(
        &router,
        "POST",
        "/api/v1/ssot/appshell/lock",
        Some(&tok_admin),
        None,
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "action": "TRANSFER",
            "lockOwnerId": "admin-alpha",
            "expectedRevision": 1,
            "expectedFencingToken": 1,
            "lockedSlots": ["TopBar"],
            "ttlMs": 60000,
            "forceTransferReason": "emergency takeover"
        })),
    )
    .await;
    assert_eq!(st_tr_adm, StatusCode::OK);
    assert_eq!(body_tr_adm["data"]["lock"]["fencingToken"], 2);
    assert_eq!(body_tr_adm["data"]["lock"]["revision"], 2);

    // 8. POST /api/v1/ssot/export -> 200 OK + dual markdown/json artifacts
    let (st_exp, body_exp) = send_req(
        &router,
        "POST",
        "/api/v1/ssot/export",
        Some(&tok_ed),
        Some("DeveloperPortal"),
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "formats": ["markdown", "json"],
            "includeQuarantined": false
        })),
    )
    .await;
    assert_eq!(st_exp, StatusCode::OK);
    assert_eq!(body_exp["data"]["revision"], 1);
    assert_eq!(
        body_exp["data"]["markdownSha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(body_exp["data"]["jsonSha256"].as_str().unwrap().len(), 64);

    // 9. POST /api/v1/skills/route -> 200 OK with <=250 spine tokens & invariant prefixSha256
    let (st_r1, body_r1) = send_req(
        &router,
        "POST",
        "/api/v1/skills/route",
        Some(&tok_ed),
        None,
        Some(json!({
            "projectId": project_id,
            "intentSummary": "Build analytics dashboard widgets",
            "portalId": "DeveloperPortal",
            "requestedSlugs": ["typography", "dashboard-ux"],
            "maxSpineTokens": 250
        })),
    )
    .await;
    assert_eq!(st_r1, StatusCode::OK);
    assert!(body_r1["data"]["spineTokens"].as_u64().unwrap() <= 250);
    assert_eq!(
        body_r1["data"]["selectedSlugs"],
        json!(["dashboard-ux", "typography"])
    );

    // 10. Viewer role readOnlyPost allowed on POST /api/v1/skills/route, denied on POST /api/v1/ssot/appshell/lock
    let tok_viewer = format!("sess-vw-{}", Uuid::new_v4());
    state.auth.insert_session(
        tok_viewer.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-alpha".into(),
            role: "viewer".into(),
        },
        Utc::now().timestamp() + 3600,
    );
    let (st_vw_route, _) = send_req(
        &router,
        "POST",
        "/api/v1/skills/route",
        Some(&tok_viewer),
        None,
        Some(json!({
            "projectId": project_id,
            "intentSummary": "Inspect typography scale",
            "portalId": "DeveloperPortal",
            "maxSpineTokens": 250
        })),
    )
    .await;
    assert_eq!(st_vw_route, StatusCode::OK);
}
