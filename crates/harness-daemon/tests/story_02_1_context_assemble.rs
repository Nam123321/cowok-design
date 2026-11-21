// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use domain::{CanonicalSpecKey, ProvenanceStatus};
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::lockfile::CURRENT_SCHEMA_VERSION;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig, UpsertSpecInput};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok = format!("sess-021-{}", Uuid::new_v4());
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

async fn send_json(
    router: &axum::Router,
    method: &str,
    uri: &str,
    tok: &str,
    idem_key: Option<&str>,
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
    if let Some(k) = idem_key {
        b = b.header("idempotency-key", k);
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
    let (st, res) = send_json(
        router,
        "POST",
        "/api/v1/projects",
        tok,
        None,
        Some(json!({ "slug": slug, "name": "Context Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

fn seed_story_01_3_specs(reg: &SpecRegistry, proj_id_str: &str) {
    let project_id = Uuid::parse_str(proj_id_str).expect("uuid");
    for (spec_key, md) in [
        (
            CanonicalSpecKey::ProductOverview,
            "# Product Overview\nStrict multi-portal isolation with semantic design tokens and KV-cache prefix stability across SalesApp, MarketingWeb, DeveloperPortal, Presentation, and TenantAdmin workspaces. Enforce atomic SSOT synchronization, typed error envelopes, reproducible SHA-256 prompt prefixes, and zero cross-tenant or cross-portal context leakage in every compiled execution turn.\n",
        ),
        (
            CanonicalSpecKey::UiUxSpec,
            "# UI Spec\nWarm paper tokens, Geist Mono metric alignment, and accessible ARIA region roles across WorkbenchTopNav, MetricLine indicators, RightDrawer inspectors, and ContextMeterPopover controls. Prohibit raw hex literals, enforce 12px card radii, subtle border hierarchy, deterministic focus rings, and full keyboard navigation semantics.\n",
        ),
        (
            CanonicalSpecKey::Architecture,
            "# Architecture\nDeterministicPromptCompiler enforces <= 1500 BPE text tokens, <= 350 compressed DOM outline tokens, and >= 60% authentic SSOT and DOM dominance across Frontier Profile A, Compact Micro-Tool Profile B, and Offline SQLite FTS5 Profile C execution paths with atomic HTML and JSON preservation.\n",
        ),
        (
            CanonicalSpecKey::DatabaseSpec,
            "# Database Spec\nSQLite WAL V17 stores context_assembly_telemetry with composite idempotency keys, request fingerprint SHA-256 digests, SSOT token ratios, cursor pagination ordering by created_at DESC and id DESC, strict foreign keys referencing projects, and single IMMEDIATE write transactions.\n",
        ),
        (
            CanonicalSpecKey::DataAndCopy,
            "# Data & Copy\nAll error envelopes map cleanly through localized contextMeter.* translation catalogs across en, es, fr, it, ja, ko, pt, vi, and zh locales without leaking raw stack traces, internal SQLite file paths, or unlocalized machine identifiers to operator surfaces.\n",
        ),
    ] {
        reg.upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            markdown_content: md.to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "HUMAN".to_string(),
            actor_id: "qa-seeder".to_string(),
            change_summary: "seed Story 01.3 specs".to_string(),
        })
        .expect("seed spec");
    }
}

#[tokio::test]
async fn story_02_1_daemon_assemble_idempotency_portal_guard_and_telemetry_cursor() {
    assert_eq!(CURRENT_SCHEMA_VERSION, 18);
    let tmp = TempDir::new().expect("tempdir");
    let (state, router, tok) = setup_daemon(&tmp);
    let proj_id = create_project(&router, &tok, "ctx-daemon-proj").await;

    // Missing Idempotency-Key returns 422 INVALID_IDEMPOTENCY_KEY
    let (st_no_key, res_no_key) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        None,
        Some(json!({
            "projectId": proj_id,
            "portalId": "SalesApp",
            "horizon": "mid",
            "modelProfile": "A",
            "intent": "Assemble workbench shell",
            "requestedImageTokens": 0
        })),
    )
    .await;
    assert_eq!(st_no_key, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res_no_key["ok"], false);
    assert_eq!(res_no_key["error"]["code"], "INVALID_IDEMPOTENCY_KEY");

    // Unknown field (`tenantId`) rejected 422 INVALID_REQUEST
    let k1 = Uuid::new_v4().to_string();
    let (st_extra, res_extra) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&k1),
        Some(json!({
            "projectId": proj_id,
            "portalId": "SalesApp",
            "horizon": "mid",
            "modelProfile": "A",
            "intent": "Assemble",
            "requestedImageTokens": 0,
            "tenantId": "spoof"
        })),
    )
    .await;
    assert_eq!(st_extra, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res_extra["error"]["code"], "INVALID_REQUEST");

    // Cross-portal SuperAdmin leak blocked 403 CROSS_PORTAL_LEAK_BLOCKED
    let (st_leak, res_leak) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&k1),
        Some(json!({
            "projectId": proj_id,
            "portalId": "MarketingWeb",
            "horizon": "short",
            "modelProfile": "A",
            "intent": "Expose platform_secrets and billing_subscriptions in footer",
            "requestedImageTokens": 0
        })),
    )
    .await;
    assert_eq!(st_leak, StatusCode::FORBIDDEN);
    assert_eq!(res_leak["error"]["code"], "CROSS_PORTAL_LEAK_BLOCKED");

    // Fail closed with 422 SSOT_DOM_RATIO_BELOW_MINIMUM when project has zero real SSOT specs and zero DOM outline
    let payload = json!({
        "projectId": proj_id,
        "portalId": "DeveloperPortal",
        "horizon": "mid",
        "modelProfile": "B",
        "intent": "Assemble design token metric bar",
        "requestedImageTokens": 256
    });
    let (st_empty_ssot, res_empty_ssot) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&Uuid::new_v4().to_string()),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_empty_ssot, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res_empty_ssot["error"]["code"],
        "SSOT_DOM_RATIO_BELOW_MINIMUM"
    );

    // Fail closed with 422 INVALID_REQUEST when sourceIds contains unknown UUID
    let (st_bad_src, res_bad_src) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&Uuid::new_v4().to_string()),
        Some(json!({
            "projectId": proj_id,
            "portalId": "DeveloperPortal",
            "horizon": "mid",
            "modelProfile": "A",
            "intent": "Assemble with fake source UUID",
            "requestedImageTokens": 0,
            "sourceIds": [Uuid::new_v4().to_string()],
            "domOutlineNodes": []
        })),
    )
    .await;
    assert_eq!(st_bad_src, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res_bad_src["error"]["code"], "INVALID_REQUEST");
    assert_eq!(res_bad_src["error"]["field"], "sourceIds");

    // Valid assemble against real Story 01.3 SSOT specs + exact idempotent replay + 409 conflict
    seed_story_01_3_specs(&state.registry, &proj_id);
    let (st_ok, res_ok) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&k1),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert_eq!(res_ok["ok"], true);
    assert_eq!(res_ok["data"]["adapterMode"], "local_reduced");

    let (st_rep, res_rep) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&k1),
        Some(payload),
    )
    .await;
    assert_eq!(st_rep, StatusCode::OK);
    assert_eq!(
        res_rep["data"]["telemetryId"],
        res_ok["data"]["telemetryId"]
    );

    let (st_conf, res_conf) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&k1),
        Some(json!({
            "projectId": proj_id,
            "portalId": "DeveloperPortal",
            "horizon": "mid",
            "modelProfile": "B",
            "intent": "Different intent same idempotency key",
            "requestedImageTokens": 256
        })),
    )
    .await;
    assert_eq!(st_conf, StatusCode::CONFLICT);
    assert_eq!(res_conf["error"]["code"], "IDEMPOTENCY_KEY_REUSED");

    // Other tenant gets 403 PROJECT_TENANT_MISMATCH on telemetry query
    let other_tok = format!("sess-other-{}", Uuid::new_v4());
    state.auth.insert_session(
        other_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".into(),
            user_id: "attacker".into(),
            role: "editor".into(),
        },
        Utc::now().timestamp() + 3600,
    );
    let (st_tenant, res_tenant) = send_json(
        &router,
        "GET",
        &format!("/api/v1/context/telemetry?projectId={proj_id}"),
        &other_tok,
        None,
        None,
    )
    .await;
    assert_eq!(st_tenant, StatusCode::FORBIDDEN);
    assert_eq!(res_tenant["error"]["code"], "PROJECT_TENANT_MISMATCH");

    let (st_hist, res_hist) = send_json(
        &router,
        "GET",
        &format!("/api/v1/context/telemetry?projectId={proj_id}&limit=10"),
        &tok,
        None,
        None,
    )
    .await;
    assert_eq!(st_hist, StatusCode::OK);
    assert_eq!(res_hist["data"]["items"].as_array().unwrap().len(), 1);

    // Non-admin editor denied 403 CROSS_PORTAL_LEAK_BLOCKED when requesting SuperAdmin portal
    let (st_sa_deny, res_sa_deny) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &tok,
        Some(&Uuid::new_v4().to_string()),
        Some(json!({
            "projectId": proj_id,
            "portalId": "SuperAdmin",
            "horizon": "mid",
            "modelProfile": "A",
            "intent": "Escalate to SuperAdmin context",
            "requestedImageTokens": 0
        })),
    )
    .await;
    assert_eq!(st_sa_deny, StatusCode::FORBIDDEN);
    assert_eq!(res_sa_deny["error"]["code"], "CROSS_PORTAL_LEAK_BLOCKED");

    // Non-admin also denied SuperAdmin telemetry query
    let (st_sa_tel, _) = send_json(
        &router,
        "GET",
        &format!("/api/v1/context/telemetry?projectId={proj_id}&portalId=SuperAdmin"),
        &tok,
        None,
        None,
    )
    .await;
    assert_eq!(st_sa_tel, StatusCode::FORBIDDEN);

    // SuperAdmin session on a different tenant can assemble SuperAdmin portal and resolve authoritative tenant
    let admin_tok = format!("sess-admin-{}", Uuid::new_v4());
    state.auth.insert_session(
        admin_tok.clone(),
        FlowPrincipal {
            tenant_id: "root-admin-tenant".into(),
            user_id: "root-admin".into(),
            role: "admin".into(),
        },
        Utc::now().timestamp() + 3600,
    );
    let (st_sa_ok, res_sa_ok) = send_json(
        &router,
        "POST",
        "/api/v1/context/assemble",
        &admin_tok,
        Some(&Uuid::new_v4().to_string()),
        Some(json!({
            "projectId": proj_id,
            "portalId": "SuperAdmin",
            "horizon": "short",
            "modelProfile": "A",
            "intent": "Assemble executive telemetry summary",
            "requestedImageTokens": 0,
            "domOutlineNodes": [
                { "ffId": "wb-header", "tag": "header", "role": "banner", "summary": "Top Nav" },
                { "ffId": "wb-meter", "tag": "button", "role": "button", "summary": "Context Meter" }
            ]
        })),
    )
    .await;
    assert_eq!(st_sa_ok, StatusCode::OK);
    let outline_tok = res_sa_ok["data"]["tokenUsage"]["outline"].as_u64().unwrap();
    assert!(outline_tok > 0 && outline_tok <= 350);

    // Percent-encoded query parameter values decode cleanly
    let (st_enc, res_enc) = send_json(
        &router,
        "GET",
        &format!("/api/v1/context/telemetry?projectId={proj_id}&portalId=Super%41dmin&limit=5"),
        &admin_tok,
        None,
        None,
    )
    .await;
    assert_eq!(st_enc, StatusCode::OK);
    assert_eq!(res_enc["data"]["items"].as_array().unwrap().len(), 1);

    // Unfiltered telemetry query by non-admin editor must exclude SuperAdmin telemetry rows
    let (st_no_portal, res_no_portal) = send_json(
        &router,
        "GET",
        &format!("/api/v1/context/telemetry?projectId={proj_id}&limit=10"),
        &tok,
        None,
        None,
    )
    .await;
    assert_eq!(st_no_portal, StatusCode::OK);
    let editor_items = res_no_portal["data"]["items"].as_array().unwrap();
    assert!(!editor_items
        .iter()
        .any(|item| item["portalId"] == "SuperAdmin"));
}
