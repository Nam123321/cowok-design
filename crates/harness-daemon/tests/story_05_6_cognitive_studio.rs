// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, OnceLock};

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let exp = Utc::now().timestamp() + 3600;

    let editor_tok = format!("sess-056-ed-{}", Uuid::new_v4());
    state.auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "designer-1".into(),
            role: "editor".into(),
        },
        exp,
    );

    let viewer_tok = format!("sess-056-vw-{}", Uuid::new_v4());
    state.auth.insert_session(
        viewer_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-1".into(),
            role: "viewer".into(),
        },
        exp,
    );

    let other_tenant_tok = format!("sess-056-other-{}", Uuid::new_v4());
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
        Some(json!({ "slug": slug, "name": "Brand Studio Test", "product_brief": "brief" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    res["id"].as_str().expect("project id").to_string()
}

#[tokio::test]
async fn it_05_6_01_brand_proposals_strict_theme_modes_and_tenant_isolation() {
    let tmp = TempDir::new().expect("tempdir");
    let (_state, router, editor_tok, viewer_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "studio-brand-proj").await;

    // Viewer cannot mutate brand proposals -> 403
    let (st_v, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/brand-proposals",
        &viewer_tok,
        Some(json!({ "projectId": project_id, "themeModes": ["warm"], "locale": "vi" })),
    )
    .await;
    assert_eq!(st_v, StatusCode::FORBIDDEN);

    // Cross-tenant editor cannot access tenant-alpha project -> 403
    let (st_x, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/brand-proposals",
        &other_tenant_tok,
        Some(json!({ "projectId": project_id, "themeModes": ["warm"], "locale": "vi" })),
    )
    .await;
    assert_eq!(st_x, StatusCode::FORBIDDEN);

    // Valid single-mode ["warm"] returns 3 proposals with strictly 1 mode and <=3 rendered Sketch pages
    let (st_ok, res) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/brand-proposals",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "referenceApps": ["Linear", "Stripe"],
            "themeModes": ["warm"],
            "locale": "vi"
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK, "res={res}");
    assert_eq!(res["ok"], true);
    assert_eq!(res["data"]["requestedModes"], json!(["warm"]));
    assert_eq!(res["data"]["proposals"].as_array().unwrap().len(), 3);
    assert!(
        res["data"]["previewBundle"]["pages"]
            .as_array()
            .unwrap()
            .len()
            <= 3
    );
}

#[tokio::test]
async fn it_05_6_02_scan_components_fe_contract_hidden_specs_occ_and_heritage_pack() {
    let _env_guard = env_lock().lock().await;
    let tmp = TempDir::new().expect("tempdir");
    std::env::set_var("COWOK_HERITAGE_PACKS_ROOT", tmp.path());
    let (_state, router, editor_tok, _viewer_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "studio-full-proj").await;

    // 1. Scan components & persist to 07_COMPONENT_REGISTRY (component_patterns)
    let (st_scan, r_scan) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/scan-components",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "portalId": "SalesApp",
            "surface": "fe",
            "sketchHtmls": ["<html><body><table><tr><td>A</td></tr></table><button>Save</button></body></html>"],
            "selectedKinds": ["data-table", "button", "toast"]
        })),
    )
    .await;
    assert_eq!(st_scan, StatusCode::OK, "r_scan={r_scan}");
    assert!(r_scan["data"]["syncedToRegistryCount"].as_u64().unwrap() >= 3);

    // 2. Extract FE Data Contract
    let (st_fe, r_fe) = send_json(
        &router,
        "GET",
        &format!("/api/v1/cognitive/fe-data-contract?projectId={project_id}&screenId=default"),
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_fe, StatusCode::OK, "r_fe={r_fe}");
    assert_eq!(
        r_fe["data"]["draft"]["boundaryNotice"],
        "FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY"
    );
    assert!(r_fe["data"]["draftTsSource"]
        .as_str()
        .unwrap()
        .contains("export interface ScreenViewModel"));

    // 3. Hidden Specs GET + PUT with strict OCC 409 STALE_MAPPING
    let (st_hs_get, r_hs_get) = send_json(
        &router,
        "GET",
        &format!("/api/v1/cognitive/hidden-specs?projectId={project_id}"),
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_hs_get, StatusCode::OK);
    assert_eq!(r_hs_get["data"]["specs"].as_array().unwrap().len(), 9);

    let (st_put1, r_put1) = send_json(
        &router,
        "PUT",
        "/api/v1/cognitive/hidden-specs",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "specKey": "02_BRAND_IDENTITY",
            "contentMarkdown": "# 02 Brand Identity\nMode: warm only",
            "expectedRevision": 0
        })),
    )
    .await;
    assert_eq!(st_put1, StatusCode::OK, "r_put1={r_put1}");
    assert_eq!(r_put1["data"]["revision"], 1);

    let (st_conflict, r_conflict) = send_json(
        &router,
        "PUT",
        "/api/v1/cognitive/hidden-specs",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "specKey": "02_BRAND_IDENTITY",
            "contentMarkdown": "# Stale overwrite attempt",
            "expectedRevision": 0
        })),
    )
    .await;
    assert_eq!(st_conflict, StatusCode::CONFLICT, "r_conflict={r_conflict}");
    assert_eq!(r_conflict["error"]["code"], "STALE_MAPPING");

    // 4. Heritage Pack save + tenant-isolated list + jailed 0o600 file on disk
    let (st_pack, r_pack) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        Some(json!({
            "sourceProjectId": project_id,
            "name": "Atelier Warm Pack v1",
            "entryMode": "DECK_FROM_APP",
            "brandIdentity": { "proposalId": "brand-atelier-precision" },
            "componentRegistry": { "components": ["button", "toast"] },
            "presentationTheme": { "aspectRatio": "16:9" },
            "themeModes": ["warm", "night"]
        })),
    )
    .await;
    assert_eq!(st_pack, StatusCode::OK, "r_pack={r_pack}");
    let pack_file = r_pack["data"]["filePath"].as_str().unwrap();
    assert!(std::path::Path::new(pack_file).exists());

    let (st_list_alpha, r_list_alpha) = send_json(
        &router,
        "GET",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_list_alpha, StatusCode::OK);
    assert_eq!(r_list_alpha["data"]["packs"].as_array().unwrap().len(), 1);

    // Other tenant MUST see 0 packs (zero cross-tenant leakage)
    let (st_list_beta, r_list_beta) = send_json(
        &router,
        "GET",
        "/api/v1/cognitive/heritage-packs",
        &other_tenant_tok,
        None,
    )
    .await;
    assert_eq!(st_list_beta, StatusCode::OK);
    assert_eq!(r_list_beta["data"]["packs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn it_05_6_03_regression_auth_body_cap_null_pack_bidirectional_occ_and_uncheck_sync() {
    let _env_guard = env_lock().lock().await;
    let tmp = TempDir::new().expect("tempdir");
    // Pre-create .cowok-design with overly permissive 0o755 to verify fs_jail tightens it to 0o700
    let cowok_dir = tmp.path().join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir");
    std::fs::set_permissions(&cowok_dir, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    std::env::set_var("COWOK_HERITAGE_PACKS_ROOT", tmp.path());

    let (state, router, editor_tok, _viewer_tok, other_tenant_tok) = setup_daemon(&tmp);
    let project_id = create_project(&router, &editor_tok, "studio-remediation-proj").await;

    // 1. Fake Bearer on GET /api/v1/cognitive/heritage-packs MUST be rejected with 401
    let fake_bearer_req = Request::builder()
        .method("GET")
        .uri("/api/v1/cognitive/heritage-packs")
        .header("host", "127.0.0.1:4170")
        .header("authorization", "Bearer forged-non-superadmin-token")
        .body(Body::empty())
        .unwrap();
    let fake_resp = router.clone().oneshot(fake_bearer_req).await.unwrap();
    assert_eq!(fake_resp.status(), StatusCode::UNAUTHORIZED);

    // 2. MAX_BODY_BYTES (512 KiB) -> 413 PAYLOAD_TOO_LARGE on scan-components, hidden-specs, heritage-packs
    let oversized = vec![b'a'; 512 * 1024 + 64];
    for (method, uri) in [
        ("POST", "/api/v1/cognitive/scan-components"),
        ("PUT", "/api/v1/cognitive/hidden-specs"),
        ("POST", "/api/v1/cognitive/heritage-packs"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("host", "127.0.0.1:4170")
            .header("x-cowok-session-token", &editor_tok)
            .header("content-type", "application/json")
            .body(Body::from(oversized.clone()))
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "expected 413 on {method} {uri}"
        );
    }

    // 3. Bidirectional Hidden Spec OCC: POST /brand-proposals bumps 02_BRAND_IDENTITY & 04_DESIGN_TOKENS
    let (st_bp, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/brand-proposals",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "themeModes": ["warm"],
            "locale": "en"
        })),
    )
    .await;
    assert_eq!(st_bp, StatusCode::OK);

    // Stale PUT with expectedRevision=0 MUST return 409 STALE_MAPPING because brand apply bumped rev to 1
    let (st_occ, r_occ) = send_json(
        &router,
        "PUT",
        "/api/v1/cognitive/hidden-specs",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "specKey": "02_BRAND_IDENTITY",
            "contentMarkdown": "# stale drawer write",
            "expectedRevision": 0
        })),
    )
    .await;
    assert_eq!(st_occ, StatusCode::CONFLICT, "r_occ={r_occ}");
    assert_eq!(r_occ["error"]["code"], "STALE_MAPPING");
    assert_eq!(r_occ["error"]["details"]["currentRevision"], 1);

    // 4. scan-components uncheck deletes previously synced studio-* rows and bumps 07_COMPONENT_REGISTRY
    let (st_s1, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/scan-components",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "portalId": "SalesApp",
            "selectedKinds": ["button", "toast"]
        })),
    )
    .await;
    assert_eq!(st_s1, StatusCode::OK);

    let (st_s2, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/scan-components",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "portalId": "SalesApp",
            "selectedKinds": ["button"]
        })),
    )
    .await;
    assert_eq!(st_s2, StatusCode::OK);
    {
        let conn = state.registry.pool().connection().expect("conn");
        let toast_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM component_patterns WHERE project_id = ?1 AND pattern_slug = 'studio-toast'",
                rusqlite::params![project_id],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(
            toast_count, 0,
            "unchecked component must be deleted from registry"
        );
        let reg_rev: i64 = conn
            .query_row(
                "SELECT revision FROM hidden_spec_documents WHERE project_id = ?1 AND spec_key = '07_COMPONENT_REGISTRY'",
                rusqlite::params![project_id],
                |r| r.get(0),
            )
            .expect("rev");
        assert_eq!(reg_rev, 2);
    }

    // 5. Heritage pack validation: >6 themeModes rejected, null brandIdentity rejected, omitted defaults to {}
    let (st_too_many, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        Some(json!({
            "sourceProjectId": project_id,
            "name": "Too Many Modes",
            "themeModes": ["m1", "m2", "m3", "m4", "m5", "m6", "m7"]
        })),
    )
    .await;
    assert_eq!(st_too_many, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_null_bi, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        Some(json!({
            "sourceProjectId": project_id,
            "name": "Null Brand Identity",
            "brandIdentity": null,
            "themeModes": ["warm"]
        })),
    )
    .await;
    assert_eq!(st_null_bi, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_defaults, r_defaults) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        Some(json!({
            "sourceProjectId": project_id,
            "name": "Defaults Pack",
            "themeModes": ["warm"]
        })),
    )
    .await;
    assert_eq!(st_defaults, StatusCode::OK, "r_defaults={r_defaults}");
    assert_eq!(r_defaults["data"]["pack"]["brandIdentity"], json!({}));
    assert_eq!(r_defaults["data"]["pack"]["componentRegistry"], json!({}));
    assert_eq!(r_defaults["data"]["pack"]["presentationTheme"], json!({}));

    // Verify .cowok-design was tightened from 0o755 to 0o700
    let perm = std::fs::metadata(&cowok_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        perm, 0o700,
        "existing .cowok-design dir must be tightened to 0o700"
    );

    // 6. Legacy DB row with "null" JSON normalized to {} on GET, AND disk cross-project pack merged
    {
        let conn = state.registry.pool().connection().expect("conn");
        conn.execute(
            "INSERT INTO heritage_design_packs (
                id, tenant_id, name, source_project_id, entry_mode, brand_identity_json,
                component_registry_json, presentation_theme_json, theme_modes_json, created_at
            ) VALUES ('null-legacy-pack', 'tenant-alpha', 'Legacy Null Pack', ?1, 'GREENFIELD', 'null', 'null', 'null', '[\"warm\"]', 100)",
            rusqlite::params![project_id],
        )
        .expect("insert legacy null pack");
    }
    let disk_only_id = Uuid::new_v4().to_string();
    let disk_pack = json!({
        "id": disk_only_id,
        "tenantId": "tenant-alpha",
        "name": "Disk Cross-Project Pack",
        "sourceProjectId": project_id,
        "entryMode": "REVAMP",
        "brandIdentity": { "brandName": "Cross" },
        "componentRegistry": {},
        "presentationTheme": {},
        "themeModes": ["warm"],
        "createdAt": 9_999_999_999_999i64
    });
    std::fs::write(
        cowok_dir
            .join("heritage-packs")
            .join(format!("{disk_only_id}.json")),
        serde_json::to_vec(&disk_pack).unwrap(),
    )
    .expect("write disk pack");

    let (st_get, r_get) = send_json(
        &router,
        "GET",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_get, StatusCode::OK);
    let packs_arr = r_get["data"]["packs"].as_array().unwrap();
    let legacy = packs_arr
        .iter()
        .find(|p| p["id"] == "null-legacy-pack")
        .expect("legacy pack present");
    assert_eq!(legacy["brandIdentity"], json!({}));
    assert!(
        packs_arr.iter().any(|p| p["id"] == disk_only_id),
        "verified disk pack must be merged on GET"
    );

    // Other tenant MUST NOT see tenant-alpha's disk pack
    let (st_get_beta, r_get_beta) = send_json(
        &router,
        "GET",
        "/api/v1/cognitive/heritage-packs",
        &other_tenant_tok,
        None,
    )
    .await;
    assert_eq!(st_get_beta, StatusCode::OK);
    assert_eq!(r_get_beta["data"]["packs"].as_array().unwrap().len(), 0);

    // 7. Failed jail write (parent traversal in root) MUST NOT leave phantom SQLite row
    std::env::set_var("COWOK_HERITAGE_PACKS_ROOT", "/tmp/../etc/evil");
    let (st_jail_fail, _) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/heritage-packs",
        &editor_tok,
        Some(json!({
            "sourceProjectId": project_id,
            "name": "Phantom Pack Must Rollback",
            "themeModes": ["warm"]
        })),
    )
    .await;
    assert_eq!(st_jail_fail, StatusCode::INTERNAL_SERVER_ERROR);
    {
        let conn = state.registry.pool().connection().expect("conn");
        let phantom: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM heritage_design_packs WHERE name = 'Phantom Pack Must Rollback'",
                [],
                |r| r.get(0),
            )
            .expect("count phantom");
        assert_eq!(phantom, 0, "failed jail write must never commit SQLite row");
    }
    std::env::set_var("COWOK_HERITAGE_PACKS_ROOT", tmp.path());

    // 8. CSS_SECURITY_BLOCKED mapping on MxssBlocked sketchPages & percent-decoded screenId
    let (st_mxss, r_mxss) = send_json(
        &router,
        "POST",
        "/api/v1/cognitive/brand-proposals",
        &editor_tok,
        Some(json!({
            "projectId": project_id,
            "themeModes": ["warm"],
            "sketchPages": [{
                "screenId": "scr-evil",
                "title": "Evil",
                "html": "<html><body><script>alert(1)</script></body></html>"
            }]
        })),
    )
    .await;
    assert_eq!(st_mxss, StatusCode::UNPROCESSABLE_ENTITY, "r_mxss={r_mxss}");
    assert_eq!(r_mxss["error"]["code"], "CSS_SECURITY_BLOCKED");
    assert_eq!(r_mxss["error"]["field"], "sketchPages");

    let (st_q, r_q) = send_json(
        &router,
        "GET",
        &format!(
            "/api/v1/cognitive/fe-data-contract?projectId={project_id}&screenId=screen%20alpha%2F1"
        ),
        &editor_tok,
        None,
    )
    .await;
    assert_eq!(st_q, StatusCode::OK, "r_q={r_q}");
    assert_eq!(r_q["data"]["draft"]["screenId"], "screen alpha/1");
}
