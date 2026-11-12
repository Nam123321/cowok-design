// @story 01.3
// @cover AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10
use std::fs;
use std::net::TcpListener;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
};
use domain::{CanonicalSpecKey, ProvenanceStatus};
use harness_daemon::{
    build_router, map_error_to_response, read_daemon_lock, resolve_project_scoped_registry,
    validate_loopback_host, validate_seed_repo_path, ChatSessionDto, CreateProjectRequest,
    DaemonAppState, DaemonConfig, DaemonHttpClient, DaemonServer, GitHubPRResponseDto,
    HealthResponse, HttpErrorBody, ModelAdapterRequestDto, PathTokenResponse, ProjectBoundaryDto,
    ProjectDto, ProjectSpecsListResponse, PromptCompileRequest, SpecItemWithDriftDto,
    SynthesizeSpecsRequest, SynthesizeSpecsResponse,
};
use http_body_util::BodyExt;
use rusqlite::params;
use sha2::{Digest, Sha256};
use spec_registry::{atomic_write_spec_file, SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn setup_daemon_router(tmp: &TempDir) -> (Arc<SpecRegistry>, axum::Router) {
    let ws_path = tmp.path().display().to_string();
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let state = DaemonAppState {
        registry: Arc::clone(&registry),
        host: "127.0.0.1".to_string(),
        port: 4170,
    };
    let reg_for_layer = Arc::clone(&registry);
    let router =
        build_router(state).layer(middleware::from_fn(move |mut req: Request, next: Next| {
            let ws_default = ws_path.clone();
            let reg = Arc::clone(&reg_for_layer);
            async move {
                let path = req.uri().path().to_string();
                let is_project_route = path.starts_with("/api/v1/projects/");
                let no_auth_requested = req.headers().contains_key("x-cowok-test-no-auth");

                if !req.headers().contains_key("x-cowok-workspace-root") {
                    let mut resolved_ws = ws_default;
                    if let Some(rest) = path.strip_prefix("/api/v1/projects/") {
                        let seg = rest.split('/').next().unwrap_or("");
                        if let Ok(pid) = Uuid::parse_str(seg) {
                            if let Ok(conn) = reg.pool().connection() {
                                if let Ok(ws) = conn.query_row(
                                    "SELECT workspace_root FROM projects WHERE id = ?1",
                                    params![pid.to_string()],
                                    |r| r.get::<_, String>(0),
                                ) {
                                    if !ws.is_empty() && ws != "." {
                                        resolved_ws = ws;
                                    }
                                }
                            }
                        }
                    }
                    if let Ok(val) = HeaderValue::from_str(&resolved_ws) {
                        req.headers_mut().insert("x-cowok-workspace-root", val);
                    }
                }

                // If not explicitly testing unauthorized, auto-inject valid Bearer auth for test convenience
                if is_project_route
                    && !no_auth_requested
                    && !req.headers().contains_key("authorization")
                {
                    if let Some(rest) = path.strip_prefix("/api/v1/projects/") {
                        let seg = rest.split('/').next().unwrap_or("");
                        if let Ok(pid) = Uuid::parse_str(seg) {
                            if let Ok((_r, root)) = resolve_project_scoped_registry(&reg, pid) {
                                let tp = root.join(".cowok-design").join("project.token");
                                if let Ok(tok) = fs::read_to_string(&tp) {
                                    if let Ok(val) =
                                        HeaderValue::from_str(&format!("Bearer {}", tok.trim()))
                                    {
                                        req.headers_mut().insert("authorization", val);
                                    }
                                }
                            }
                        }
                    }
                }

                next.run(req).await
            }
        }));
    (registry, router)
}

async fn send_json_request(
    router: &axum::Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    json_body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }
    let body = if let Some(val) = json_body {
        builder = builder.header("content-type", "application/json");
        Body::from(serde_json::to_vec(&val).expect("serialize request json"))
    } else {
        Body::empty()
    };
    let req = builder.body(body).expect("build http request");
    let resp = router
        .clone()
        .oneshot(req)
        .await
        .expect("execute oneshot request");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    let parsed: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                panic!(
                    "parse json response: {e}; status: {status}; body: '{}'",
                    String::from_utf8_lossy(&bytes)
                );
            }
        }
    };
    (status, parsed)
}

#[tokio::test]
async fn api_01_to_05_and_syn_01_to_04_rest_endpoints_provenance_and_idempotency() {
    let tmp = TempDir::new().expect("create temp workspace");
    let (registry, router) = setup_daemon_router(&tmp);

    // API-01: GET /api/v1/health
    let (status, health_json) =
        send_json_request(&router, "GET", "/api/v1/health", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let health: HealthResponse = serde_json::from_value(health_json).expect("parse health");
    assert_eq!(health.status, "ok");
    assert_eq!(health.service, "cowok-design-harness-daemon");
    assert_eq!(health.host, "127.0.0.1");

    // API-02: POST /api/v1/projects (Upstream Brand Project)
    let create_upstream = serde_json::json!({
        "slug": "acme-brand-core",
        "name": "Acme Brand Core System",
        "product_brief": "Enterprise B2B Design System with strict WCAG 2.2 AA tokens, multi-portal executive deck & workbench.",
        "workspace_root": tmp.path().display().to_string(),
        "inherit_brand_from_project_id": null
    });
    let (status, upstream_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(create_upstream),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let upstream_proj: ProjectDto =
        serde_json::from_value(upstream_val).expect("parse upstream project");

    // SYN-01: POST /api/v1/projects/:id/specs/synthesize (Rich brief >= 40 chars)
    let syn_req = serde_json::json!({
        "product_brief": null,
        "seed_repo_path": null,
        "inherit_brand_from_project_id": null,
        "actor_id": "test-synthesizer-01"
    });
    let (status, syn_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/specs/synthesize", upstream_proj.id),
        &[],
        Some(syn_req.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let syn_resp: SynthesizeSpecsResponse =
        serde_json::from_value(syn_val).expect("parse synthesize response");
    assert!(!syn_resp.sparse_brief_detected);
    assert!(syn_resp.total_confirmed_count >= 9);
    assert!(syn_resp.open_questions.is_empty());
    assert_eq!(syn_resp.specs.len(), 9);
    for s in &syn_resp.specs {
        assert_eq!(s.version, 1);
        assert_eq!(s.status, ProvenanceStatus::Confirmed);
    }

    // SYN-04: Idempotent re-synthesis with identical brief does NOT bump version or create extra revisions
    let (status, syn_idem_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/specs/synthesize", upstream_proj.id),
        &[],
        Some(syn_req),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let syn_idem: SynthesizeSpecsResponse =
        serde_json::from_value(syn_idem_val).expect("parse idempotent synthesize response");
    for s in &syn_idem.specs {
        assert_eq!(
            s.version, 1,
            "idempotent re-synthesis must keep version == 1"
        );
    }

    // SYN-03: Cross-project brand inheritance + upstream drift detection
    let create_child = serde_json::json!({
        "slug": "acme-sub-portal",
        "name": "Acme Sub Portal",
        "product_brief": "Customer analytics portal inheriting brand tokens and architecture from Acme Brand Core System.",
        "workspace_root": null,
        "inherit_brand_from_project_id": upstream_proj.id
    });
    let (status, child_val) =
        send_json_request(&router, "POST", "/api/v1/projects", &[], Some(create_child)).await;
    assert_eq!(status, StatusCode::CREATED);
    let child_proj: ProjectDto = serde_json::from_value(child_val).expect("parse child project");

    let (status, child_syn_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/specs/synthesize", child_proj.id),
        &[],
        Some(serde_json::json!({
            "product_brief": null,
            "seed_repo_path": null,
            "inherit_brand_from_project_id": upstream_proj.id,
            "actor_id": "brand-inheritor"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let child_syn: SynthesizeSpecsResponse =
        serde_json::from_value(child_syn_val).expect("parse child syn");
    let inherited_sec = child_syn
        .specs
        .iter()
        .find(|s| s.spec_key == CanonicalSpecKey::SecurityTenancy)
        .expect("08-security-tenancy.md");
    assert_eq!(inherited_sec.source_project_id, Some(upstream_proj.id));
    assert!(inherited_sec.source_revision_sha256.is_some());

    // API-03: GET /api/v1/projects/:id/specs before upstream change -> upstream_brand_drift_detected == false
    let (status, list_before_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/specs", child_proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list_before: ProjectSpecsListResponse =
        serde_json::from_value(list_before_val).expect("parse list specs before");
    assert!(!list_before.upstream_brand_drift_detected);
    assert_eq!(list_before.specs.len(), 9);

    // API-04: PUT /api/v1/projects/:id/specs/:spec_key on upstream project to trigger brand drift
    let (upstream_reg, _) =
        resolve_project_scoped_registry(&registry, upstream_proj.id).expect("upstream reg");
    let upstream_sec = upstream_reg
        .get_spec(upstream_proj.id, CanonicalSpecKey::SecurityTenancy)
        .expect("get upstream sec")
        .expect("upstream sec exists");
    let put_upstream_req = serde_json::json!({
        "markdown_content": "# 08-security-tenancy.md\n\n- [CONFIRMED] Updated v2 Brand & Security Tenancy Guidelines.\n",
        "expected_sha256": upstream_sec.content_sha256,
        "status": "CONFIRMED",
        "change_summary": "Upgrade brand security tokens to v2",
        "source_project_id": null,
        "source_revision_sha256": null
    });
    let (status, put_up_val) = send_json_request(
        &router,
        "PUT",
        &format!(
            "/api/v1/projects/{}/specs/08-security-tenancy.md",
            upstream_proj.id
        ),
        &[("x-cowok-actor-id", "brand-admin")],
        Some(put_upstream_req),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let updated_up_spec: SpecItemWithDriftDto =
        serde_json::from_value(put_up_val).expect("parse updated upstream spec");
    assert_eq!(updated_up_spec.version, 2);

    // Verify child project now detects upstream brand drift (`upstream_brand_drift_detected == true`)
    let (status, list_after_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/specs", child_proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list_after: ProjectSpecsListResponse =
        serde_json::from_value(list_after_val).expect("parse list specs after");
    assert!(
        list_after.upstream_brand_drift_detected,
        "child project must detect upstream brand revision drift"
    );

    // SYN-02: Sparse brief (< 40 chars) in the SAME daemon workspace sets confirmed_count == 0 and populates open_questions (EC-E01-03)
    // and proves multi-project isolation does NOT clobber upstream_proj or child_proj!
    let create_sparse = serde_json::json!({
        "slug": "sparse-demo",
        "name": "Sparse Demo",
        "product_brief": "Tiny app",
        "workspace_root": null,
        "inherit_brand_from_project_id": null
    });
    let (status, sparse_proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(create_sparse),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let sparse_proj: ProjectDto =
        serde_json::from_value(sparse_proj_val).expect("parse sparse project");

    let (status, sparse_syn_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/specs/synthesize", sparse_proj.id),
        &[],
        Some(serde_json::json!({
            "product_brief": "Short brief",
            "seed_repo_path": null,
            "inherit_brand_from_project_id": null,
            "actor_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let sparse_syn: SynthesizeSpecsResponse =
        serde_json::from_value(sparse_syn_val).expect("parse sparse syn");
    assert!(sparse_syn.sparse_brief_detected);
    assert_eq!(
        sparse_syn.total_confirmed_count, 0,
        "sparse brief must have 0 CONFIRMED items"
    );
    assert!(sparse_syn.total_inferred_count >= 9);
    assert!(sparse_syn.total_missing_count >= 9);
    assert!(!sparse_syn.open_questions.is_empty());

    // API-05: Verify HTTP error codes: 409 STALE_FILE, 422 INVALID_SPEC_KEY, 404 PROJECT_NOT_FOUND
    let (status, stale_err_val) = send_json_request(
        &router,
        "PUT",
        &format!(
            "/api/v1/projects/{}/specs/08-security-tenancy.md",
            upstream_proj.id
        ),
        &[],
        Some(serde_json::json!({
            "markdown_content": "# Stale overwrite attempt\n",
            "expected_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "status": "CONFIRMED",
            "change_summary": null,
            "source_project_id": null,
            "source_revision_sha256": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let stale_err: HttpErrorBody = serde_json::from_value(stale_err_val).expect("parse stale err");
    assert_eq!(stale_err.code, "STALE_FILE");

    let (status, inv_key_val) = send_json_request(
        &router,
        "PUT",
        &format!(
            "/api/v1/projects/{}/specs/99-invalid-spec.md",
            upstream_proj.id
        ),
        &[],
        Some(serde_json::json!({
            "markdown_content": "# Invalid spec\n",
            "expected_sha256": null,
            "status": null,
            "change_summary": null,
            "source_project_id": null,
            "source_revision_sha256": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let inv_key_err: HttpErrorBody =
        serde_json::from_value(inv_key_val).expect("parse invalid spec key err");
    assert_eq!(inv_key_err.code, "INVALID_SPEC_KEY");

    let missing_id = Uuid::new_v4();
    let (status, not_found_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{missing_id}/specs"),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let not_found_err: HttpErrorBody =
        serde_json::from_value(not_found_val).expect("parse project not found err");
    assert_eq!(not_found_err.code, "PROJECT_NOT_FOUND");

    // Verify Typed Client SDK (`DaemonHttpClient`) + `seed_repo_path` extraction (`F-01`)
    let seed_dir = tmp.path().join("seeded-design-repo");
    fs::create_dir_all(&seed_dir).expect("create seed_dir");
    fs::write(
        seed_dir.join("README.md"),
        "# Seeded Design Repo\nEnterprise multi-portal design workbench with WCAG 2.2 AA tokens and presentation deck.",
    )
    .expect("write seed README.md");

    let sdk = DaemonHttpClient::new(Arc::clone(&registry), "127.0.0.1".to_string(), 4170);
    assert_eq!(sdk.health().status, "ok");
    let sdk_proj = sdk
        .create_project(&CreateProjectRequest {
            id: None,
            slug: "sdk-client-project".to_string(),
            name: "SDK Client Project".to_string(),
            product_brief: String::new(),
            workspace_root: None,
            inherit_brand_from_project_id: None,
        })
        .expect("sdk create_project");
    let sdk_syn = sdk
        .synthesize_specs(
            sdk_proj.id,
            &SynthesizeSpecsRequest {
                product_brief: None,
                seed_repo_path: Some("seeded-design-repo".to_string()),
                inherit_brand_from_project_id: None,
                actor_id: Some("sdk-actor".to_string()),
            },
        )
        .expect("sdk synthesize_specs from seed_repo_path");
    assert_eq!(sdk_syn.specs.len(), 9);
    assert!(
        !sdk_syn.sparse_brief_detected,
        "seed_repo_path README content >= 40 chars must synthesize rich CONFIRMED specs"
    );
    assert!(sdk_syn.total_confirmed_count >= 9);
    assert!(sdk
        .get_spec(sdk_proj.id, CanonicalSpecKey::ProductOverview)
        .expect("sdk get_spec")
        .is_some());

    // Verify duplicate custom `workspace_root` across two projects is rejected with 409 UNIQUE_CONSTRAINT (`F-02`)
    let custom_sub_ws = tmp.path().join("custom-project-ws");
    fs::create_dir_all(&custom_sub_ws).expect("create custom_sub_ws");
    let (status, _) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "ws-owner-1",
            "name": "WS Owner 1",
            "product_brief": "First project owning custom-project-ws directory.",
            "workspace_root": custom_sub_ws.display().to_string(),
            "inherit_brand_from_project_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, dup_ws_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "ws-owner-2",
            "name": "WS Owner 2",
            "product_brief": "Second project attempting to share custom-project-ws directory.",
            "workspace_root": custom_sub_ws.display().to_string(),
            "inherit_brand_from_project_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let dup_ws_err: HttpErrorBody = serde_json::from_value(dup_ws_val).expect("parse dup ws err");
    assert_eq!(dup_ws_err.code, "UNIQUE_CONSTRAINT");
}

#[test]
fn port_01_and_lock_recovery_01_dynamic_port_fallback_and_daemon_lock() {
    let tmp = TempDir::new().expect("create temp workspace");
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open pool");
    let registry = Arc::new(SpecRegistry::new(pool));

    // Reject non-loopback host (`0.0.0.0`)
    assert!(validate_loopback_host("0.0.0.0").is_err());

    // Pre-seed a stale `.cowok-design/daemon.lock` simulating a prior crashed daemon (`LOCK-RECOVERY-01`)
    let cowok_dir = tmp.path().join(".cowok-design");
    fs::create_dir_all(&cowok_dir).expect("create .cowok-design");
    fs::write(
        cowok_dir.join("daemon.lock"),
        r#"{"pid":999999,"host":"127.0.0.1","port":4170,"started_at":1700000000000}"#,
    )
    .expect("write stale lock");

    // Occupy a loopback port so `DaemonServer::bind_loopback` encounters `AddrInUse` (`PORT-01`)
    let blocker = TcpListener::bind("127.0.0.1:0").expect("bind blocker port");
    let occupied_port = blocker.local_addr().expect("local addr").port();

    let cfg = DaemonConfig {
        host: "127.0.0.1".to_string(),
        preferred_port: occupied_port,
        workspace_root: tmp.path().to_path_buf(),
    };

    let server = DaemonServer::bind_loopback(&cfg, registry).expect("bind loopback with fallback");
    let actual_port = server.lock_info().port;
    assert_ne!(
        actual_port, occupied_port,
        "daemon must dynamically fall back to a free port when preferred_port is in use"
    );
    assert_eq!(server.lock_info().host, "127.0.0.1");

    // Verify `.cowok-design/daemon.lock` was atomically overwritten with the new port and current PID
    let disk_lock = read_daemon_lock(tmp.path()).expect("read daemon.lock");
    assert_eq!(disk_lock.port, actual_port);
    assert_eq!(disk_lock.pid, std::process::id());
}

#[tokio::test]
async fn txn_recovery_01_txn_retry_01_trace_01_idor_01_sre_p2002_p2003_conc_01() {
    let tmp = TempDir::new().expect("create temp workspace");
    let (registry, router) = setup_daemon_router(&tmp);

    // Create base project
    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "resilience-proj",
            "name": "Resilience Project",
            "product_brief": "Comprehensive resilience, anti-IDOR, P2002/P2003, and crash recovery verification project.",
            "workspace_root": null,
            "inherit_brand_from_project_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("parse project");

    // SRE-P2002-01: Duplicate project slug returns 409 Conflict (`UNIQUE_CONSTRAINT`)
    let (status, dup_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "resilience-proj",
            "name": "Duplicate Slug Project",
            "product_brief": "Duplicate slug test",
            "workspace_root": null,
            "inherit_brand_from_project_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let dup_err: HttpErrorBody = serde_json::from_value(dup_val).expect("parse dup err");
    assert_eq!(dup_err.code, "UNIQUE_CONSTRAINT");

    // SRE-P2003-01: Non-existent `inherit_brand_from_project_id` returns 422 (`REFERENTIAL_INTEGRITY`)
    let (status, fk_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/specs/synthesize", proj.id),
        &[],
        Some(serde_json::json!({
            "product_brief": null,
            "seed_repo_path": null,
            "inherit_brand_from_project_id": Uuid::new_v4(),
            "actor_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let fk_err: HttpErrorBody = serde_json::from_value(fk_val).expect("parse fk err");
    assert_eq!(fk_err.code, "REFERENTIAL_INTEGRITY");

    // IDOR-01: Mismatched `X-Cowok-Workspace-Root` returns 403 Forbidden (`PROJECT_SCOPE_VIOLATION`)
    let other_tmp = TempDir::new().expect("other temp dir");
    let (status, idor_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/specs", proj.id),
        &[(
            "x-cowok-workspace-root",
            other_tmp.path().to_str().expect("str"),
        )],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let idor_err: HttpErrorBody = serde_json::from_value(idor_val).expect("parse idor err");
    assert_eq!(idor_err.code, "PROJECT_SCOPE_VIOLATION");

    // Seed repo path traversal (`..`) rejected
    assert!(validate_seed_repo_path(tmp.path(), "../outside").is_err());

    // TRACE-01: Verify `SPEC_REVISIONS` provenance columns after `synthesize` and `PUT`
    let (status, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/specs/synthesize", proj.id),
        &[("x-cowok-actor-id", "trace-synth-agent")],
        Some(serde_json::json!({
            "product_brief": null,
            "seed_repo_path": null,
            "inherit_brand_from_project_id": null,
            "actor_id": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (proj_reg, proj_ws_root) =
        resolve_project_scoped_registry(&registry, proj.id).expect("resolve proj_reg");
    let spec01 = proj_reg
        .get_spec(proj.id, CanonicalSpecKey::ProductOverview)
        .expect("get spec01")
        .expect("spec01 exists");

    let (status, _) = send_json_request(
        &router,
        "PUT",
        &format!(
            "/api/v1/projects/{}/specs/01-product-overview.md",
            proj.id
        ),
        &[("x-cowok-actor-id", "trace-human-editor")],
        Some(serde_json::json!({
            "markdown_content": "# 01-product-overview.md\n\n- [CONFIRMED] Edited by trace-human-editor.\n",
            "expected_sha256": spec01.content_sha256,
            "status": "CONFIRMED",
            "change_summary": "Manual refinement of product overview",
            "source_project_id": null,
            "source_revision_sha256": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    {
        let conn = registry.pool().connection().expect("db conn");
        let (actor_kind_v1, actor_id_v1): (String, String) = conn
            .query_row(
                "SELECT actor_kind, actor_id FROM spec_revisions WHERE spec_id = ?1 AND revision_number = 1",
                params![spec01.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("query v1 revision");
        assert_eq!(actor_kind_v1, "synthesis_pipeline");
        assert_eq!(actor_id_v1, "trace-synth-agent");

        let (actor_kind_v2, actor_id_v2): (String, String) = conn
            .query_row(
                "SELECT actor_kind, actor_id FROM spec_revisions WHERE spec_id = ?1 AND revision_number = 2",
                params![spec01.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("query v2 revision");
        assert_eq!(actor_kind_v2, "http_client");
        assert_eq!(actor_id_v2, "trace-human-editor");
    }

    // TXN-RECOVERY-01: Simulate crash between disk rename and SQL commit (`spec_write_intents(PENDING)`)
    // then verify `GET /api/v1/projects/:id/specs` automatically replays recovery to v3
    let spec01_v2 = proj_reg
        .get_spec(proj.id, CanonicalSpecKey::ProductOverview)
        .expect("get spec01 v2")
        .expect("exists");
    let crashed_md =
        "# 01-product-overview.md\n\n- [CONFIRMED] Recovered via daemon GET endpoint.\n";
    let mut hasher = Sha256::new();
    hasher.update(crashed_md.as_bytes());
    let target_sha = format!("{:x}", hasher.finalize());

    let op_id = Uuid::new_v4();
    atomic_write_spec_file(
        &proj_ws_root,
        CanonicalSpecKey::ProductOverview,
        op_id,
        crashed_md.as_bytes(),
    )
    .expect("write target disk bytes before simulated crash");

    {
        let conn = registry.pool().connection().expect("db conn");
        conn.execute(
            "INSERT INTO spec_write_intents (
                operation_id, spec_id, project_id, spec_key, expected_version,
                old_sha256, new_sha256, actor_kind, actor_id, change_summary,
                provenance_summary_json, source_project_id, source_revision_sha256,
                state, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 2, ?5, ?6, 'daemon_test', 'crash-sim', 'Replay on GET', '{}', NULL, NULL, 'PENDING', 1700000000000, 1700000000000)",
            params![
                op_id.to_string(),
                spec01_v2.id.to_string(),
                proj.id.to_string(),
                CanonicalSpecKey::ProductOverview.as_str(),
                spec01_v2.content_sha256,
                target_sha,
            ],
        )
        .expect("insert pending write intent");
    }

    let (status, list_recovered_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/specs", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list_recovered: ProjectSpecsListResponse =
        serde_json::from_value(list_recovered_val).expect("parse recovered list");
    let recovered_01 = list_recovered
        .specs
        .iter()
        .find(|s| s.spec_key == CanonicalSpecKey::ProductOverview)
        .expect("find 01-product-overview.md");
    assert_eq!(
        recovered_01.version, 3,
        "GET endpoint must replay PENDING intent to version 3"
    );
    assert_eq!(recovered_01.content_sha256, target_sha);

    // CONC-01: Concurrent PUT with stale hash vs updated state returns 409 STALE_FILE
    let (status, conc_err_val) = send_json_request(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}/specs/01-product-overview.md", proj.id),
        &[],
        Some(serde_json::json!({
            "markdown_content": "# Concurrent writer with outdated v2 sha\n",
            "expected_sha256": spec01_v2.content_sha256,
            "status": "CONFIRMED",
            "change_summary": "Stale concurrent write",
            "source_project_id": null,
            "source_revision_sha256": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let conc_err: HttpErrorBody = serde_json::from_value(conc_err_val).expect("parse conc err");
    assert_eq!(conc_err.code, "STALE_FILE");

    // TXN-RETRY-01: Verify `SqliteBusy` maps deterministically to HTTP 503 `DATABASE_BUSY`
    let busy_resp = map_error_to_response(domain::HarnessError::SqliteBusy {
        details: "upsert_spec exhausted 3 retries".to_string(),
    });
    assert_eq!(busy_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_prompt_compile_route_and_token_budget_governor() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (registry, router) = setup_daemon_router(&tmp);

    // 1. Create a project
    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "prompt-compile-project",
            "name": "Prompt Compile Project",
            "product_brief": "A project for testing prompt compiler and token governor."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("parse project");

    // 1b. Verify project.token permission hardening (POSIX 0600)
    let (_proj_reg, proj_root) =
        resolve_project_scoped_registry(&registry, proj.id).expect("resolve project");
    let token_path = proj_root.join(".cowok-design").join("project.token");
    assert!(token_path.exists(), "project.token must exist");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&token_path).expect("metadata");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "project.token must strictly have 0600 permission mode"
        );
    }

    // 2. Unauthenticated request to prompt-compile returns 401 Unauthorized
    let (status, unauth_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/prompt-compile", proj.id),
        &[("x-cowok-test-no-auth", "1")],
        Some(serde_json::json!({
            "task_instructions": "Build user auth flow"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(unauth_val["kind"], "UNAUTHORIZED");

    // 2b. Request with arbitrary unmapped Bearer token returns 403 Forbidden (TENANT_ISOLATION_BREACH)
    let (status, idor_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/prompt-compile", proj.id),
        &[("authorization", "Bearer arbitrary-token")],
        Some(serde_json::json!({
            "task_instructions": "Build user auth flow"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(idor_val["kind"], "TENANT_ISOLATION_BREACH");

    // 3. Authenticated request with superadmin Bearer token succeeds
    std::env::set_var("COWOK_SUPERADMIN_SECRET", "superadmin-secret-token");
    let (status, payload_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/prompt-compile", proj.id),
        &[("authorization", "Bearer superadmin-secret-token")],
        Some(serde_json::json!({
            "system_instruction": "You are the Cowok-Design AI assistant.",
            "project_constraints": ["No unsafe code", "100% Rust"],
            "task_instructions": "Implement Token Budget Governor",
            "optional_components": ["Button", "Modal"],
            "token_ceiling": 1500
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let payload: domain::CompiledPromptPayload =
        serde_json::from_value(payload_val).expect("parse payload");
    assert!(payload.token_count <= 1500);
    assert!(!payload.sha256_digest.is_empty());
    assert!(payload.compiled_prompt.contains("SYSTEM INSTRUCTIONS"));
    assert!(payload.compiled_prompt.contains("PROJECT CONSTRAINTS"));
    assert!(payload.compiled_prompt.contains("TASK INSTRUCTIONS"));

    // 4. Test Client SDK method `compile_prompt`
    let client = DaemonHttpClient::new(registry.clone(), "127.0.0.1".into(), 9999);
    let req = PromptCompileRequest {
        system_instruction: Some("You are the AI assistant.".into()),
        project_constraints: Some(vec!["Strict memory limits".into()]),
        slice_request: None,
        task_instructions: "Compile prompt via SDK".into(),
        optional_components: None,
        token_ceiling: Some(1500),
    };
    let sdk_res = client.compile_prompt(proj.id, &req).expect("sdk compile");
    assert!(sdk_res.token_count <= 1500);
    assert_eq!(sdk_res.token_ceiling, 1500);
    assert!(!sdk_res.sha256_digest.is_empty());
}

#[tokio::test]
async fn test_model_adapter_route_and_capability_tiers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (registry, router) = setup_daemon_router(&tmp);

    // 1. Create a project
    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "model-adapter-project",
            "name": "Model Adapter Project",
            "product_brief": "A project for testing 3-profile model capability adapter."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("parse project");

    // Sample tool definitions
    let sample_tools = vec![domain::ModelToolDefinition {
        name: "mutate_ast".to_string(),
        description: "Applies surgical AST patch".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "ast_node_id": { "type": "string" },
                "details": {
                    "type": "object",
                    "properties": {
                        "backup": { "type": "boolean" },
                        "force": { "type": "boolean" }
                    }
                }
            },
            "required": ["ast_node_id"]
        }),
        is_micro_tool: false,
    }];

    // 2. Unauthenticated request to model-adapter returns 401 Unauthorized
    let (status, unauth_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj.id),
        &[("x-cowok-test-no-auth", "1")],
        Some(serde_json::json!({
            "requested_profile": "frontier_reasoning",
            "raw_system_prompt": "You are the specialist agent.",
            "tools": sample_tools
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(unauth_val["kind"], "UNAUTHORIZED");

    // 2b. Request with arbitrary unmapped Bearer token returns 403 Forbidden (TENANT_ISOLATION_BREACH)
    let (status, idor_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj.id),
        &[("authorization", "Bearer arbitrary-invalid-token")],
        Some(serde_json::json!({
            "requested_profile": "frontier_reasoning",
            "raw_system_prompt": "You are the specialist agent.",
            "tools": sample_tools
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(idor_val["kind"], "TENANT_ISOLATION_BREACH");

    // 3. Authenticated request for Profile A (Frontier Reasoning)
    std::env::set_var("COWOK_SUPERADMIN_SECRET", "superadmin-secret-token");
    let (status, payload_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj.id),
        &[("authorization", "Bearer superadmin-secret-token")],
        Some(serde_json::json!({
            "requested_profile": "frontier_reasoning",
            "raw_system_prompt": "Perform architecture reasoning.",
            "tools": sample_tools
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: domain::AdaptationResponse =
        serde_json::from_value(payload_val).expect("parse adaptation response");
    assert_eq!(
        resp.active_profile,
        domain::ModelProfileTier::ProfileA_Frontier
    );
    assert_eq!(resp.reasoning_effort, domain::ReasoningEffort::High);
    assert!(!resp.degraded);
    assert!(resp.system_prompt.contains("PROFILE A: FRONTIER REASONING"));
    assert_eq!(resp.token_budget, 1500);
    assert!(!resp.sha256_digest.is_empty());

    // 4. Authenticated request for Profile C (Compact / Micro-tools)
    let (status, payload_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj.id),
        &[("authorization", "Bearer superadmin-secret-token")],
        Some(serde_json::json!({
            "requested_profile": "compact_motion",
            "raw_system_prompt": "Generate 3D motion spec.",
            "tools": sample_tools
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: domain::AdaptationResponse =
        serde_json::from_value(payload_val).expect("parse compact response");
    assert_eq!(
        resp.active_profile,
        domain::ModelProfileTier::ProfileC_Compact
    );
    assert_eq!(resp.reasoning_effort, domain::ReasoningEffort::Low);
    assert_eq!(resp.token_budget, 1000);
    assert!(resp.system_prompt.contains("PROFILE C: COMPACT"));
    assert!(resp.tools[0].is_micro_tool);

    // 5. AC-03 Edge Case: Syntax error reported -> Graceful degradation to Profile B with micro-tools
    let (status, payload_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj.id),
        &[("authorization", "Bearer superadmin-secret-token")],
        Some(serde_json::json!({
            "requested_profile": "compact_motion",
            "raw_system_prompt": "Retry after malformed tool call.",
            "tools": sample_tools,
            "syntax_error_reported": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: domain::AdaptationResponse =
        serde_json::from_value(payload_val).expect("parse degraded response");
    assert_eq!(
        resp.active_profile,
        domain::ModelProfileTier::ProfileB_FastCoding
    );
    assert_eq!(resp.reasoning_effort, domain::ReasoningEffort::Medium);
    assert!(resp.degraded);
    assert_eq!(
        resp.degradation_reason,
        Some(domain::DegradationReason::SyntaxErrorRetry)
    );
    assert!(resp.tools[0].is_micro_tool);

    // 6. Test Client SDK method `adapt_model_capabilities`
    let client = DaemonHttpClient::new(registry.clone(), "127.0.0.1".into(), 9999);
    let sdk_req = ModelAdapterRequestDto {
        requested_profile: domain::ModelProfileTier::ProfileB_FastCoding,
        raw_system_prompt: "Direct code fix via SDK.".into(),
        tools: sample_tools.clone(),
        disallowed_profiles: vec![],
        syntax_error_reported: false,
        max_budget_override: None,
    };
    let sdk_res = client
        .adapt_model_capabilities(proj.id, &sdk_req)
        .expect("sdk adapt");
    assert_eq!(
        sdk_res.active_profile,
        domain::ModelProfileTier::ProfileB_FastCoding
    );
    assert_eq!(sdk_res.reasoning_effort, domain::ReasoningEffort::Medium);
    assert!(!sdk_res.degraded);

    // 7. F-01: Insert PROJECT_CONSTRAINTS in SQLite WAL and verify automatic disallowed profile enforcement
    let (proj_reg, _) = resolve_project_scoped_registry(&registry, proj.id).expect("resolve proj");
    let now_ms = chrono::Utc::now().timestamp_millis();
    let conn = proj_reg.pool().connection().expect("acquire conn");
    let profile_id = uuid::Uuid::new_v4().to_string();
    let rule_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at)
         VALUES (?1, ?2, 'strict_b2b', 'tailwind', 'tokens.css', 'icons.json', '[]', '{}', ?3)",
        rusqlite::params![profile_id, proj.id.to_string(), now_ms],
    )
    .expect("insert profile");

    conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES (?1, ?2, 'FORBID_FRONTIER', 'BLOCKER', '{\"forbid_frontier\": true}', 1, ?3)",
        rusqlite::params![rule_id, profile_id, now_ms],
    )
    .expect("insert rule");

    let (status, payload_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj.id),
        &[("authorization", "Bearer superadmin-secret-token")],
        Some(serde_json::json!({
            "requested_profile": "frontier_reasoning",
            "raw_system_prompt": "Attempt frontier profile on air-gapped project.",
            "tools": sample_tools,
            "disallowed_profiles": [] // Empty on client, enforced by SQLite WAL
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp: domain::AdaptationResponse =
        serde_json::from_value(payload_val).expect("parse response");
    assert_eq!(
        resp.active_profile,
        domain::ModelProfileTier::ProfileB_FastCoding
    );
    assert!(resp.degraded);
    assert_eq!(
        resp.degradation_reason,
        Some(domain::DegradationReason::DisallowedProfileConstraint)
    );

    // 8. F-07: A non-forbid constraint mentioning profile name (e.g. preferred_profile) must NOT disallow it
    let (status2, proj2_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "f07-test-proj",
            "name": "F07 Test Project",
            "product_brief": "Testing non-forbidding constraint rule."
        })),
    )
    .await;
    assert_eq!(status2, StatusCode::CREATED);
    let proj2: ProjectDto = serde_json::from_value(proj2_val).expect("parse project 2");

    let (proj2_reg, _) =
        resolve_project_scoped_registry(&registry, proj2.id).expect("resolve proj2");
    let conn2 = proj2_reg.pool().connection().expect("conn2");
    let profile_id2 = uuid::Uuid::new_v4().to_string();
    let rule_id2 = uuid::Uuid::new_v4().to_string();
    conn2.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at)
         VALUES (?1, ?2, 'strict_b2b', 'tailwind', 'tokens.css', 'icons.json', '[]', '{}', ?3)",
        rusqlite::params![profile_id2, proj2.id.to_string(), now_ms],
    )
    .expect("insert profile2");

    conn2.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES (?1, ?2, 'PREFERRED_PROFILE', 'INFO', '{\"preferred_profile\": \"frontier_reasoning\"}', 1, ?3)",
        rusqlite::params![rule_id2, profile_id2, now_ms],
    )
    .expect("insert rule2");

    let (status, payload_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/model-adapter", proj2.id),
        &[("authorization", "Bearer superadmin-secret-token")],
        Some(serde_json::json!({
            "requested_profile": "frontier_reasoning",
            "raw_system_prompt": "Preferred profile should not be disallowed.",
            "tools": sample_tools
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resp2: domain::AdaptationResponse =
        serde_json::from_value(payload_val).expect("parse response 2");
    assert_eq!(
        resp2.active_profile,
        domain::ModelProfileTier::ProfileA_Frontier
    );
    assert!(!resp2.degraded);
}

#[tokio::test]
async fn test_lint_source_and_constraint_profile_routes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (registry, router) = setup_daemon_router(&tmp);
    let client =
        harness_daemon::DaemonHttpClient::new(registry.clone(), "127.0.0.1".to_string(), 0);

    // 1. Create project
    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "name": "lint-test-proj",
            "slug": "lint-test-proj",
            "product_brief": "Lint test project"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("parse project");

    // 2. GET constraint profile initially
    let (status, profile_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/constraint-profile", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let profile: domain::ConstraintProfile =
        serde_json::from_value(profile_val).expect("profile json");
    assert_eq!(profile.project_id, proj.id);

    // 3. Update constraint profile via PUT with forbidden Tailwind & Hex patterns
    let mut updated_profile = profile.clone();
    updated_profile.forbidden_patterns = vec!["TAILWIND".to_string(), "RAW_HEX".to_string()];
    let (status, active_val) = send_json_request(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}/constraint-profile", proj.id),
        &[],
        Some(serde_json::to_value(&updated_profile).expect("serialize profile")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let active_rules: Vec<constraint_engine::ActiveConstraint> =
        serde_json::from_value(active_val).expect("active json");
    assert!(!active_rules.is_empty());

    // 4. POST /lint with tailwind class and inline hex
    let (status, lint_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/lint", proj.id),
        &[],
        Some(serde_json::json!({
            "source": "<div className={`flex p-4`} style={{ color: '#ff0000' }} />"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let lint_resp: harness_daemon::LintSourceResponse =
        serde_json::from_value(lint_val).expect("lint response");
    assert!(!lint_resp.passed);
    assert!(lint_resp.violations.len() >= 2);

    // 5. Test DaemonHttpClient direct methods
    let fetched = client.get_constraint_profile(proj.id).expect("client get");
    assert_eq!(fetched.project_id, proj.id);
    let lint_client_resp = client
        .lint_source(proj.id, "<div className='clean-token' />")
        .expect("client lint");
    assert!(lint_client_resp.passed);
}

#[tokio::test]
async fn test_icon_manifest_and_validation_routes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (registry, router) = setup_daemon_router(&tmp);
    let client = DaemonHttpClient::new(registry.clone(), "127.0.0.1".to_string(), 0);

    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "name": "Icon Route Project",
            "slug": "icon-route-proj",
            "product_brief": "Testing icon manifest and validation endpoints"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("parse project");

    // 1. GET /api/v1/projects/:id/icon-manifest
    let (status, manifest_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/icon-manifest", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let manifest: constraint_engine::IconManifest =
        serde_json::from_value(manifest_val).expect("manifest json");
    assert_eq!(manifest.count, 143);

    // 2. POST /api/v1/projects/:id/validate-icon with valid icon
    let (status, valid_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/validate-icon", proj.id),
        &[],
        Some(serde_json::json!({
            "icon": "activity"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let valid_report: constraint_engine::IconValidationReport =
        serde_json::from_value(valid_val).expect("valid report");
    assert!(valid_report.is_valid);
    assert!(valid_report.suggestions.is_empty());

    // 3. POST /api/v1/projects/:id/validate-icon with typo icon -> Top-3 suggestions
    let (status, typo_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/validate-icon", proj.id),
        &[],
        Some(serde_json::json!({
            "icon": "activty"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let typo_report: constraint_engine::IconValidationReport =
        serde_json::from_value(typo_val).expect("typo report");
    assert!(!typo_report.is_valid);
    assert_eq!(typo_report.suggestions.len(), 3);
    assert_eq!(typo_report.suggestions[0], "activity");

    // 4. Test DaemonHttpClient direct methods
    let client_manifest = client
        .get_icon_manifest(proj.id)
        .expect("client get manifest");
    assert_eq!(client_manifest.count, 143);
    let client_report = client
        .validate_icon(proj.id, "ai-agent")
        .expect("client validate");
    assert!(client_report.is_valid);
}

#[tokio::test]
async fn test_inject_ff_id_and_surgical_patch_routes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (registry, router) = setup_daemon_router(&tmp);
    let client = DaemonHttpClient::new(registry.clone(), "127.0.0.1".to_string(), 0);

    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "name": "Patcher Route Project",
            "slug": "patcher-route-proj",
            "product_brief": "Testing inject-ff-id and surgical-patch endpoints"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("parse project");

    // 1. POST /api/v1/projects/:id/inject-ff-id
    let raw_source = "<div><span>Unpatched Text</span></div>";
    let (status, inject_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/inject-ff-id", proj.id),
        &[],
        Some(serde_json::json!({
            "source": raw_source,
            "component_name": "TestComponent"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let inject_resp: constraint_engine::AstInjectionResponse =
        serde_json::from_value(inject_val).expect("injection response");
    assert!(inject_resp.total_elements_tagged >= 2);
    assert!(inject_resp.tagged_source.contains("data-ff-id"));

    // 2. POST /api/v1/projects/:id/surgical-patch
    let target_ff_id = &inject_resp.generated_ids[1]; // Target the inner span
    let (status, patch_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/surgical-patch", proj.id),
        &[],
        Some(serde_json::json!({
            "ff_id": target_ff_id,
            "patch_kind": "text_content",
            "new_value": "Surgically Patched Content",
            "source": &inject_resp.tagged_source,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let patch_resp: constraint_engine::SurgicalPatchResponse =
        serde_json::from_value(patch_val).expect("patch response");
    assert!(patch_resp.success);
    assert!(patch_resp
        .patched_source
        .contains("Surgically Patched Content"));

    // 3. Test DaemonHttpClient direct methods
    let client_inject = client
        .inject_ff_id(proj.id, "<div><h3>Title</h3></div>", Some("ClientCmp"))
        .expect("client inject");
    assert!(client_inject.tagged_source.contains("data-ff-id"));

    let client_patch_req = constraint_engine::SurgicalPatchRequest {
        ff_id: client_inject.generated_ids[1].clone(),
        patch_kind: constraint_engine::PatchKind::TextContent,
        new_value: "Client Patched Title".to_string(),
        source: client_inject.tagged_source,
        target_attribute: None,
    };
    let client_patch = client
        .surgical_patch(proj.id, &client_patch_req)
        .expect("client patch");
    assert!(client_patch.success);
    assert!(client_patch.patched_source.contains("Client Patched Title"));
}

#[tokio::test]
async fn test_hybrid_project_boundary_chat_sessions_and_hmac_path_token() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_registry, router) = setup_daemon_router(&tmp);

    // 1. Create a Managed Scratch project (no workspace_root specified)
    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "scratch-app",
            "name": "Scratch App Project",
            "product_brief": "A scratch project for rapid prototyping",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("project dto");

    // 2. GET /api/v1/projects/:id/boundary -> check Managed Scratch boundary
    let (status, boundary_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/boundary", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let boundary: ProjectBoundaryDto = serde_json::from_value(boundary_val).expect("boundary dto");
    assert_eq!(boundary.boundary_type, "managed_scratch");
    assert_eq!(boundary.config_dir, ".opendesign");
    assert!(!boundary.is_git_repository);
    assert!(boundary.promoted_at.is_none());

    // Verify .opendesign/project.json was created on disk in the scratch workspace
    let scratch_ws = std::path::PathBuf::from(&boundary.workspace_path);
    let opendesign_proj = scratch_ws.join(".opendesign").join("project.json");
    assert!(
        opendesign_proj.exists(),
        ".opendesign/project.json must exist in scratch workspace"
    );

    // Create a dummy spec file in scratch workspace to verify it gets copied on promotion
    let dummy_spec_file = scratch_ws.join("demo.txt");
    fs::write(&dummy_spec_file, "Hello from scratch workspace").expect("write demo.txt");

    // 3. Test 1 Project : N Chat Sessions
    // Initial default session
    let (status, sessions_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sessions", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let sessions: Vec<ChatSessionDto> =
        serde_json::from_value(sessions_val).expect("sessions list");
    assert_eq!(sessions.len(), 1, "Must contain default Main Session");
    assert_eq!(sessions[0].title, "Main Session");

    // Create second session
    let (status, new_sess_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/sessions", proj.id),
        &[],
        Some(serde_json::json!({
            "title": "Design System Review",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let new_sess: ChatSessionDto = serde_json::from_value(new_sess_val).expect("new session dto");
    assert_eq!(new_sess.title, "Design System Review");

    // Re-list sessions (should be 2)
    let (status, sessions_val2) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sessions", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let sessions2: Vec<ChatSessionDto> =
        serde_json::from_value(sessions_val2).expect("sessions list 2");
    assert_eq!(sessions2.len(), 2);

    // Get session by ID
    let (status, get_sess_val) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sessions/{}", proj.id, new_sess.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let fetched_sess: ChatSessionDto =
        serde_json::from_value(get_sess_val).expect("fetched session");
    assert_eq!(fetched_sess.id, new_sess.id);

    // Update session title
    let (status, upd_sess_val) = send_json_request(
        &router,
        "PUT",
        &format!("/api/v1/projects/{}/sessions/{}", proj.id, new_sess.id),
        &[],
        Some(serde_json::json!({
            "title": "Updated Session Title",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let updated_sess: ChatSessionDto =
        serde_json::from_value(upd_sess_val).expect("updated session");
    assert_eq!(updated_sess.title, "Updated Session Title");

    // Delete session
    let (status, _) = send_json_request(
        &router,
        "DELETE",
        &format!("/api/v1/projects/{}/sessions/{}", proj.id, new_sess.id),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Verify deleted session returns 422/404
    let (status, _) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/sessions/{}", proj.id, new_sess.id),
        &[],
        None,
    )
    .await;
    assert_ne!(status, StatusCode::OK);

    // 4. Test Promote Scratch to Folder-Backed Project
    let target_dir = tmp.path().join("promoted-repo");
    fs::create_dir_all(&target_dir).expect("create target dir");
    // Simulate user's preexisting git repo in target directory
    let git_dir = target_dir.join(".git");
    fs::create_dir_all(&git_dir).expect("create .git");
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").expect("write .git/HEAD");

    let (status, prom_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/promote", proj.id),
        &[],
        Some(serde_json::json!({
            "target_directory": target_dir.display().to_string(),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let prom_boundary: ProjectBoundaryDto =
        serde_json::from_value(prom_val).expect("promoted boundary");
    assert_eq!(prom_boundary.boundary_type, "folder_backed");
    assert!(
        prom_boundary.is_git_repository,
        "Should detect user's preexisting git repository"
    );
    assert!(prom_boundary.promoted_at.is_some());

    // Verify user's .git/ was preserved and not mutated
    assert_eq!(
        fs::read_to_string(git_dir.join("HEAD")).unwrap(),
        "ref: refs/heads/main\n",
        "Original .git/ must be preserved untouched"
    );

    // Verify files copied to target
    assert!(
        target_dir.join("demo.txt").exists(),
        "demo.txt must be copied to target"
    );
    assert!(
        target_dir.join(".opendesign").join("project.json").exists(),
        ".opendesign/project.json must be created in target"
    );

    // Subsequent promote should fail with 409 Conflict
    let (status_conflict, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/promote", proj.id),
        &[],
        Some(serde_json::json!({
            "target_directory": tmp.path().join("another-repo").display().to_string(),
        })),
    )
    .await;
    assert_eq!(status_conflict, StatusCode::CONFLICT);

    // 5. Test HMAC-SHA256 Path Token
    // Issue token for safe path
    let (status, tok_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/path-token", proj.id),
        &[],
        Some(serde_json::json!({
            "relative_path": "product/specs/01-product-overview.md",
            "ttl_seconds": 3600,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tok_resp: PathTokenResponse = serde_json::from_value(tok_val).expect("path token response");
    assert_eq!(
        tok_resp.normalized_relative_path,
        "product/specs/01-product-overview.md"
    );
    assert!(!tok_resp.token.is_empty());

    // Verify valid token
    let (status, verify_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/verify-path-token", proj.id),
        &[],
        Some(serde_json::json!({
            "token": tok_resp.token,
            "expected_relative_path": "product/specs/01-product-overview.md",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(verify_val.get("valid").unwrap(), true);

    // Traversal attack: "../secret.txt" should be denied with 422
    let (status, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/path-token", proj.id),
        &[],
        Some(serde_json::json!({
            "relative_path": "../secret.txt",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Encoded traversal attack: "%2e%2e/secret.txt" should be denied with 422
    let (status, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/path-token", proj.id),
        &[],
        Some(serde_json::json!({
            "relative_path": "%2e%2e/secret.txt",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Absolute path attack: "/etc/passwd" should be denied with 422
    let (status, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/path-token", proj.id),
        &[],
        Some(serde_json::json!({
            "relative_path": "/etc/passwd",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Tampered token signature should fail with 422
    let mut tampered = tok_resp.token.clone();
    tampered.push('a');
    let (status, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/verify-path-token", proj.id),
        &[],
        Some(serde_json::json!({
            "token": tampered,
            "expected_relative_path": "product/specs/01-product-overview.md",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_ac07_ac08_ac09_ac10_hardening() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_registry, router) = setup_daemon_router(&tmp);

    // AC-08: Non-RFC4122 UUID strictly returns 422 Unprocessable Entity
    let (status, err_val) = send_json_request(
        &router,
        "GET",
        "/api/v1/projects/not-a-valid-uuid-string/boundary",
        &[],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "Non-UUID project ID must be rejected with 422"
    );
    assert!(err_val.to_string().contains("INVALID_PROJECT_UUID"));

    // Create a scratch project
    let (status, proj_val) = send_json_request(
        &router,
        "POST",
        "/api/v1/projects",
        &[],
        Some(serde_json::json!({
            "slug": "hardening-proj",
            "name": "Hardening Project",
            "product_brief": "Testing AC07-AC10",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let proj: ProjectDto = serde_json::from_value(proj_val).expect("project dto");

    // AC-09: Promote to forbidden system path (e.g. /etc or /bin) must be denied with 403 SYSTEM_PATH_DENIED
    let (status, _) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/promote", proj.id),
        &[],
        Some(serde_json::json!({
            "target_directory": "/etc/malicious-folder",
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "Promote to /etc must return 403 SYSTEM_PATH_DENIED"
    );

    // AC-07: Promote to an external directory (outside daemon root) and verify scope resolution works
    let ext_tmp = tempfile::tempdir().expect("ext_tmp");
    let ext_target = ext_tmp.path().join("external-repo");
    fs::create_dir_all(&ext_target).expect("create ext target");

    let (status, prom_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/promote", proj.id),
        &[],
        Some(serde_json::json!({
            "target_directory": ext_target.display().to_string(),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let prom_boundary: ProjectBoundaryDto = serde_json::from_value(prom_val).expect("boundary");
    assert_eq!(prom_boundary.boundary_type, "folder_backed");

    // Now test GET /api/v1/projects/:id/specs on the promoted folder-backed project:
    // It must resolve the external root without PROJECT_SCOPE_VIOLATION error (AC-07)
    let (status, _specs) = send_json_request(
        &router,
        "GET",
        &format!("/api/v1/projects/{}/specs", proj.id),
        &[],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Folder-backed external project must resolve specs successfully (AC-07)"
    );

    // AC-10: Test offline mock flag on GitHub PR response
    let (status, pr_val) = send_json_request(
        &router,
        "POST",
        &format!("/api/v1/projects/{}/snapshots/pr", proj.id),
        &[],
        Some(serde_json::json!({
            "branch_name": "feature/mock-pr",
            "pr_title": "Test PR",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let pr_dto: GitHubPRResponseDto = serde_json::from_value(pr_val).expect("pr dto");
    assert!(
        pr_dto.is_offline_mock,
        "GitHub PR response must include isOfflineMock: true (AC-10)"
    );
}
