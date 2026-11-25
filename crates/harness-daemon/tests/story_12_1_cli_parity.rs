// @story 12.1
// @implements AC-01, AC-02, AC-03, AC-04 (Contract Tests ut_12_1_01..ut_12_1_08)
use std::collections::HashSet;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use harness_daemon::cli::commands::execute_cli_command;
use harness_daemon::cli::parse_cli_args;
use harness_daemon::lockfile::{generate_csprng_hex_32, DaemonLockInfo, CURRENT_SCHEMA_VERSION};
use harness_daemon::port_binder::{
    bind_zero_collision_loopback, resolve_runtime_storage_paths, ChildProcessReaper,
};
use harness_daemon::routes::legacy_router::{
    normalize_standalone_api_alias_path, wrap_with_standalone_parity, STANDALONE_API_ALIAS_HEADER,
};
use harness_daemon::routes::{build_router, DaemonAppState};
use harness_daemon::static_spa::{EmbeddedBuiltinSkillsRegistry, StaticSpaHandler};
use harness_daemon::visual_diff::{
    build_focus_free_chromium_args, count_external_cdn_refs, validate_diff_target,
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

fn setup_test_registry() -> (TempDir, Arc<SpecRegistry>) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    (dir, registry)
}

async fn read_body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("valid json body")
}

// -----------------------------------------------------------------------------
// ut_12_1_01: Standalone `/api/* -> /api/v1/*` Axum Parity Alias & Auth Guard
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_1_01_standalone_api_alias_parity() {
    assert_eq!(
        normalize_standalone_api_alias_path("/api/health").as_deref(),
        Some("/api/v1/health")
    );
    assert_eq!(
        normalize_standalone_api_alias_path("/api/doctor/cli-matrix").as_deref(),
        Some("/api/v1/doctor/cli-matrix")
    );
    assert_eq!(normalize_standalone_api_alias_path("/api/v1/health"), None);
    assert_eq!(normalize_standalone_api_alias_path("/api/v1"), None);
    assert_eq!(normalize_standalone_api_alias_path("/workbench"), None);

    let (_dir, registry) = setup_test_registry();
    let state = DaemonAppState::new(registry, "127.0.0.1".to_string(), 4170);
    let daemon_nonce = state.auth.daemon_nonce.clone();
    let app = wrap_with_standalone_parity(
        build_router(state.clone()),
        state,
        StaticSpaHandler::embedded_fallback(),
    );

    // 1. `/api/health` vs `/api/v1/health` identical body, alias header only on `/api/health`
    let resp_alias = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/health")
                .header("host", "127.0.0.1:4170")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_alias.status(), StatusCode::OK);
    assert_eq!(
        resp_alias
            .headers()
            .get(STANDALONE_API_ALIAS_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some("v1")
    );
    let alias_json = read_body_json(resp_alias).await;

    let resp_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/health")
                .header("host", "127.0.0.1:4170")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_v1.status(), StatusCode::OK);
    assert!(resp_v1.headers().get(STANDALONE_API_ALIAS_HEADER).is_none());
    let v1_json = read_body_json(resp_v1).await;
    assert_eq!(alias_json, v1_json);

    // 2. Unauthenticated `/api/projects` MUST be rejected (ZERO auth bypass!)
    let resp_unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/projects")
                .header("host", "127.0.0.1:4170")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_unauth.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp_unauth
            .headers()
            .get(STANDALONE_API_ALIAS_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some("v1")
    );

    // 3. POST `/api/auth/local-session` preserves method, headers & body and mints a real session
    let resp_sess = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/auth/local-session")
                .header("host", "127.0.0.1:4170")
                .header("content-type", "application/json")
                .header("x-cowok-daemon-nonce", &daemon_nonce)
                .body(Body::from(r#"{"role":"editor"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_sess.status(), StatusCode::OK);
    let sess_json = read_body_json(resp_sess).await;
    let session_token = sess_json["sessionToken"].as_str().expect("session token");

    // 4. Authenticated `/api/skills/catalog` vs `/api/v1/skills/catalog` parity
    let resp_skills_alias = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/skills/catalog")
                .header("host", "127.0.0.1:4170")
                .header("x-cowok-session-token", session_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_skills_alias.status(), StatusCode::OK);
    assert_eq!(
        resp_skills_alias
            .headers()
            .get(STANDALONE_API_ALIAS_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some("v1")
    );

    // 5. Unknown `/api/nonexistent` and `/api/v1/nonexistent` return JSON 404 (never SPA HTML)
    for unknown in ["/api/nonexistent", "/api/v1/nonexistent"] {
        let resp_404 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(unknown)
                    .header("host", "127.0.0.1:4170")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_404.status(), StatusCode::NOT_FOUND);
        let ct = resp_404
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains("application/json"),
            "expected JSON 404 for {unknown}, got {ct}"
        );
        let body_404 = read_body_json(resp_404).await;
        assert_eq!(body_404["error"]["code"].as_str(), Some("NOT_FOUND"));
    }
}

// -----------------------------------------------------------------------------
// ut_12_1_02: Embedded React 19 Workbench SPA & 7 Built-in Craft Skills + 7 DE Tools
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_1_02_embedded_spa_and_seven_builtin_skills() {
    // Verify 7 built-in craft skills & 7 DE tools load purely from compiled-in binary data
    let skills = EmbeddedBuiltinSkillsRegistry::load();
    assert_eq!(skills.skills.len(), 7);
    assert_eq!(skills.de_tools.len(), 7);
    assert_eq!(skills.manifest_sha256.len(), 64);
    for skill in &skills.skills {
        assert!(!skill.content.is_empty());
        assert_eq!(skill.sha256.len(), 64);
    }

    // Verify embedded HTML fallback shell works without any `dist/` directory on disk
    let fallback = StaticSpaHandler::embedded_fallback();
    let root_resp = fallback.handle_request(&Method::GET, "/");
    assert_eq!(root_resp.status(), StatusCode::OK);
    let root_bytes = root_resp.into_body().collect().await.unwrap().to_bytes();
    let root_html = String::from_utf8_lossy(&root_bytes);
    assert!(root_html.contains("data-cowok-standalone=\"true\""));
    assert_eq!(count_external_cdn_refs(&root_html), 0);

    // Verify resolved `dist/` directory with static assets, MIME, Cache-Control & symlink/traversal blocks
    let tmp = TempDir::new().unwrap();
    let dist = tmp.path().join("dist");
    let assets = dist.join("assets");
    fs::create_dir_all(&assets).unwrap();
    fs::write(
        dist.join("index.html"),
        "<!doctype html><html><body><div id=\"root\">REACT19_SPA</div></body></html>",
    )
    .unwrap();
    fs::write(assets.join("bundle.js"), "console.log('cowok');").unwrap();

    // Create symlink trap inside dist/
    let secret = tmp.path().join("secret.txt");
    fs::write(&secret, "TOP_SECRET").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&secret, dist.join("leak.txt")).unwrap();

    let spa = StaticSpaHandler::discover(Some(&dist), None);
    assert!(spa.dist_dir().is_some());

    let js_resp = spa.handle_request(&Method::GET, "/assets/bundle.js");
    assert_eq!(js_resp.status(), StatusCode::OK);
    assert_eq!(
        js_resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/javascript; charset=utf-8")
    );
    assert_eq!(
        js_resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=31536000, immutable")
    );

    // Client-side SPA route falls back to index.html
    let deep_resp = spa.handle_request(&Method::GET, "/workbench/projects/123");
    assert_eq!(deep_resp.status(), StatusCode::OK);
    let deep_body = deep_resp.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&deep_body).contains("REACT19_SPA"));

    // Path traversal blocked
    let trav_resp = spa.handle_request(&Method::GET, "/../secret.txt");
    assert_eq!(trav_resp.status(), StatusCode::BAD_REQUEST);

    // Symlink blocked
    #[cfg(unix)]
    {
        let sym_resp = spa.handle_request(&Method::GET, "/leak.txt");
        assert_eq!(sym_resp.status(), StatusCode::FORBIDDEN);
    }

    // Missing asset with extension returns 404 (not index.html)
    let miss_resp = spa.handle_request(&Method::GET, "/assets/does-not-exist.js");
    assert_eq!(miss_resp.status(), StatusCode::NOT_FOUND);
}

// -----------------------------------------------------------------------------
// ut_12_1_03: CLI Subcommands 1:1 `--json` Parity (`doctor`, `export`, `mcp`, `feedback`, `qc`, `de`)
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_03_cli_subcommands_json_parity() {
    let ws = TempDir::new().unwrap();
    let ws_str = ws.path().to_string_lossy().into_owned();
    let db_path = ws.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, ws.path())).expect("open wal");
    let registry = Arc::new(SpecRegistry::new(pool));
    let created_project = harness_daemon::create_project_isolated(
        &registry,
        &harness_daemon::CreateProjectRequest {
            id: Some(Uuid::new_v4()),
            slug: "cli-parity-project".to_string(),
            name: "CLI Parity Project".to_string(),
            product_brief: "Story 12.1 shared SQLite WAL parity".to_string(),
            workspace_root: Some(ws_str.clone()),
            inherit_brand_from_project_id: None,
        },
    )
    .expect("create project in shared SQLite WAL");
    let project_id = created_project.id.to_string();

    // 1. `--version --json`
    let ver_cmd = parse_cli_args(&["--version".into(), "--json".into()]).unwrap();
    let ver_out = execute_cli_command(ver_cmd);
    assert_eq!(ver_out.exit_code, 0);
    let ver_json: serde_json::Value = serde_json::from_str(&ver_out.stdout).unwrap();
    assert_eq!(ver_json["ok"], true);
    assert_eq!(ver_json["data"]["embeddedSkillsCount"], 7);
    assert_eq!(ver_json["data"]["deToolsCount"], 7);

    // 2. `doctor --mode passive --json` (DoctorCliMatrixResponse parity)
    let doc_cmd = parse_cli_args(&[
        "doctor".into(),
        "--mode".into(),
        "passive".into(),
        "--json".into(),
    ])
    .unwrap();
    let doc_out = execute_cli_command(doc_cmd);
    assert_eq!(doc_out.exit_code, 0);
    let doc_json: serde_json::Value = serde_json::from_str(&doc_out.stdout).unwrap();
    assert_eq!(doc_json["scanMode"], "passive");
    assert!(doc_json["tools"].as_array().unwrap().len() >= 20);
    assert_eq!(
        doc_json["tools"].as_array().unwrap().len(),
        doc_json["clis"].as_array().unwrap().len()
    );

    // 3. `export html|pdf|pptx|dev --json`
    let html_out_path = ws.path().join("out/deck.html");
    let exp_html = execute_cli_command(
        parse_cli_args(&[
            "export".into(),
            "html".into(),
            "--project".into(),
            project_id.clone(),
            "--out".into(),
            html_out_path.to_string_lossy().into_owned(),
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(exp_html.exit_code, 0, "stderr: {}", exp_html.stderr);
    let exp_html_json: serde_json::Value = serde_json::from_str(&exp_html.stdout).unwrap();
    assert_eq!(exp_html_json["ok"], true);
    assert_eq!(exp_html_json["data"]["externalCdnRefsCount"], 0);
    assert!(html_out_path.is_file());

    let pdf_out_path = ws.path().join("out/deck.pdf");
    let exp_pdf = execute_cli_command(
        parse_cli_args(&[
            "export".into(),
            "pdf".into(),
            "--project".into(),
            project_id.clone(),
            "--out".into(),
            pdf_out_path.to_string_lossy().into_owned(),
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(exp_pdf.exit_code, 0, "stderr: {}", exp_pdf.stderr);
    let exp_pdf_json: serde_json::Value = serde_json::from_str(&exp_pdf.stdout).unwrap();
    assert_eq!(exp_pdf_json["data"]["widthPt"], 1440.0);
    assert_eq!(exp_pdf_json["data"]["heightPt"], 810.0);
    let pdf_bytes = fs::read(&pdf_out_path).unwrap();
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    let pptx_out_path = ws.path().join("out/deck.pptx");
    let exp_pptx = execute_cli_command(
        parse_cli_args(&[
            "export".into(),
            "pptx".into(),
            "--project".into(),
            project_id.clone(),
            "--out".into(),
            pptx_out_path.to_string_lossy().into_owned(),
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(exp_pptx.exit_code, 0, "stderr: {}", exp_pptx.stderr);
    let exp_pptx_json: serde_json::Value = serde_json::from_str(&exp_pptx.stdout).unwrap();
    assert_eq!(exp_pptx_json["ok"], true);
    assert_eq!(exp_pptx_json["pluginInstalled"], true);
    assert_eq!(
        exp_pptx_json["pluginId"],
        "builtin-native-rust-pptx-exporter"
    );
    assert_eq!(exp_pptx_json["data"]["pluginStatus"], "BUILTIN_ACTIVE");
    assert!(pptx_out_path.is_file());
    let pptx_bytes = fs::read(&pptx_out_path).unwrap();
    assert!(pptx_bytes.starts_with(b"PK\x03\x04"));

    let dev_out_dir = ws.path().join("out/dev-handoff");
    let exp_dev = execute_cli_command(
        parse_cli_args(&[
            "export".into(),
            "dev".into(),
            "--project".into(),
            project_id.clone(),
            "--out".into(),
            dev_out_dir.to_string_lossy().into_owned(),
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(exp_dev.exit_code, 0);
    let exp_dev_json: serde_json::Value = serde_json::from_str(&exp_dev.stdout).unwrap();
    assert!(exp_dev_json["data"]["fileCount"].as_u64().unwrap() >= 1);

    // 4. `mcp install cursor --config-path <tmp> --json` + rollback
    let cursor_cfg = ws.path().join("cursor-mcp.json");
    fs::write(&cursor_cfg, r#"{"mcpServers":{}}"#).unwrap();
    let mcp_inst = execute_cli_command(
        parse_cli_args(&[
            "mcp".into(),
            "install".into(),
            "cursor".into(),
            "--config-path".into(),
            cursor_cfg.to_string_lossy().into_owned(),
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(mcp_inst.exit_code, 0, "stderr: {}", mcp_inst.stderr);
    let mcp_json: serde_json::Value = serde_json::from_str(&mcp_inst.stdout).unwrap();
    assert_eq!(mcp_json["ok"], true);
    assert_eq!(mcp_json["data"]["hostId"], "cursor");
    assert_eq!(mcp_json["data"]["action"], "install");
    let backup_id = mcp_json["data"]["backupId"].as_str().unwrap().to_string();

    let mcp_rb = execute_cli_command(
        parse_cli_args(&[
            "mcp".into(),
            "install".into(),
            "cursor".into(),
            "--rollback".into(),
            backup_id,
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(mcp_rb.exit_code, 0, "stderr: {}", mcp_rb.stderr);
    let mcp_rb_json: serde_json::Value = serde_json::from_str(&mcp_rb.stdout).unwrap();
    assert_eq!(mcp_rb_json["data"]["rolledBack"], true);

    // 5. `feedback --title ... --body ... --json` with Zero-PII scrubbing
    let fake_key = ["sk", "-", "live_", "cliSecretToken1234567890"].concat();
    let fb_body = format!("Crash with {fake_key} at /Users/alice/projects/app");
    let fb_out = execute_cli_command(
        parse_cli_args(&[
            "feedback".into(),
            "--title".into(),
            "CLI Export Regression".into(),
            "--body".into(),
            fb_body,
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(fb_out.exit_code, 0, "stderr: {}", fb_out.stderr);
    let fb_json: serde_json::Value = serde_json::from_str(&fb_out.stdout).unwrap();
    assert_eq!(fb_json["ok"], true);
    assert_eq!(fb_json["data"]["channel"], "cli");
    let desc = fb_json["data"]["description"].as_str().unwrap();
    assert!(!desc.contains(&fake_key));
    assert!(!desc.contains("/Users/alice"));
    assert!(desc.contains("[REDACTED_SECRET]"));
    assert!(desc.contains("<HOME>"));

    // 6. `qc --project <id> --force-no-browser --json`
    let qc_out = execute_cli_command(
        parse_cli_args(&[
            "qc".into(),
            "--project".into(),
            project_id,
            "--force-no-browser".into(),
            "--workspace".into(),
            ws_str.clone(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(
        qc_out.exit_code, 0,
        "stdout: {} stderr: {}",
        qc_out.stdout, qc_out.stderr
    );
    let qc_json: serde_json::Value = serde_json::from_str(&qc_out.stdout).unwrap();
    assert_eq!(qc_json["ok"], true);
    assert_eq!(qc_json["data"]["verdict"], "UNVERIFIED_NO_BROWSER");

    // 7. `de --json` runs all 7 built-in DE tools
    let de_out = execute_cli_command(
        parse_cli_args(&["de".into(), "--workspace".into(), ws_str, "--json".into()]).unwrap(),
    );
    assert_eq!(de_out.exit_code, 0, "stderr: {}", de_out.stderr);
    let de_json: serde_json::Value = serde_json::from_str(&de_out.stdout).unwrap();
    assert_eq!(de_json["ok"], true);
    assert_eq!(de_json["data"]["results"].as_array().unwrap().len(), 7);
}

// -----------------------------------------------------------------------------
// ut_12_1_04: 8-Instance Parallel Bound-FD Zero Port Collision (Zero TOCTOU)
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_04_multi_instance_zero_port_collision() {
    // Occupy one preferred loopback port to force collision fallback across all 8 parallel binders
    let blocker = TcpListener::bind("127.0.0.1:0").expect("bind blocker");
    let busy_port = blocker.local_addr().unwrap().port();

    let handles: Vec<_> = (0..8)
        .map(|_| std::thread::spawn(move || bind_zero_collision_loopback(busy_port)))
        .collect();

    let mut bound_listeners = Vec::with_capacity(8);
    let mut ports = HashSet::new();
    for h in handles {
        let bound = h.join().expect("thread join").expect("zero-collision bind");
        let port = bound.port();
        assert_ne!(port, 0);
        assert_ne!(port, busy_port);
        assert!(ports.insert(port), "duplicate port allocated: {port}");
        // Verify live OS FD is immediately accepting connections (no close-and-rebind race)
        assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
        bound_listeners.push(bound);
    }
    assert_eq!(ports.len(), 8);
    drop(bound_listeners);
    drop(blocker);
}

// -----------------------------------------------------------------------------
// ut_12_1_05: SIGINT Graceful Child Process Reaper & Lockfile Cleanup
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_05_sigint_graceful_child_reaper() {
    // Safety guards: never kill PID 0, PGID <= 1, self PID, or self PGID
    assert!(ChildProcessReaper::terminate_pgid_safe(0, 100, "t").is_err());
    assert!(ChildProcessReaper::terminate_pgid_safe(100, 0, "t").is_err());
    assert!(ChildProcessReaper::terminate_pgid_safe(100, 1, "t").is_err());
    assert!(ChildProcessReaper::terminate_pgid_safe(std::process::id(), 100, "t").is_err());
    #[cfg(unix)]
    {
        let self_pgid = unsafe { libc::getpgrp() };
        assert!(ChildProcessReaper::terminate_pgid_safe(999999, self_pgid, "t").is_err());
    }

    let tmp = TempDir::new().unwrap();
    let lock_path = tmp.path().join("daemon.lock");
    let nonce = generate_csprng_hex_32();
    let lock_info = DaemonLockInfo {
        pid: std::process::id(),
        host: "127.0.0.1".to_string(),
        port: 4170,
        started_at: domain::SafeJsInteger::new(chrono::Utc::now().timestamp_millis()).unwrap(),
        daemon_nonce: nonce.clone(),
        hmac_key_hex: generate_csprng_hex_32(),
        schema_version: CURRENT_SCHEMA_VERSION,
    };
    fs::write(&lock_path, serde_json::to_vec_pretty(&lock_info).unwrap()).unwrap();

    let reaper = ChildProcessReaper::new();
    reaper.set_owned_lockfile(lock_path.clone(), lock_info.pid, nonce);

    let mut sleep_cmd = Command::new("sleep");
    sleep_cmd.arg("30");
    let (mut child, handle) = reaper
        .spawn_managed_child(sleep_cmd, "dummy-chromium-worker")
        .expect("spawn sleep");
    assert_eq!(reaper.tracked_count(), 1);

    let summary = reaper.reap_all();
    let _ = child.wait();
    assert_eq!(summary.children_reaped, 1);
    assert!(summary.lockfile_removed);
    assert!(!lock_path.exists());

    #[cfg(unix)]
    {
        // Confirm target PGID is completely gone (ESRCH)
        let probe = unsafe { libc::kill(-handle.pgid, 0) };
        assert_ne!(probe, 0);
    }
}

// -----------------------------------------------------------------------------
// ut_12_1_06: Read-Only Binary/Workspace Directory (`chmod 0555`) XDG Fallback
// -----------------------------------------------------------------------------
#[cfg(unix)]
#[test]
fn ut_12_1_06_readonly_dir_xdg_fallback() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let ro_dir = tmp.path().join("readonly-homebrew-cellar");
    let fake_home = tmp.path().join("fake-home");
    fs::create_dir_all(&ro_dir).unwrap();
    fs::create_dir_all(&fake_home).unwrap();

    // Lock directory read-only (`0555`)
    fs::set_permissions(&ro_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let resolved_res = resolve_runtime_storage_paths(Some(&ro_dir), Some(&fake_home));

    // Always restore permissions before asserting so TempDir cleans up cleanly
    let _ = fs::set_permissions(&ro_dir, fs::Permissions::from_mode(0o755));

    let paths = resolved_res.expect("XDG fallback should succeed on 0555 dir");
    assert!(paths.read_only_fallback);
    assert!(!paths.db_path.starts_with(&ro_dir));

    // Confirm SQLite WAL opens and writes cleanly inside the resolved fallback directory
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&paths.db_path, &paths.state_root))
        .expect("open WAL in fallback dir");
    let conn = pool.connection().expect("sqlite connection");
    let one: i64 = conn.query_row("SELECT 1", [], |r| r.get(0)).unwrap();
    assert_eq!(one, 1);
}

// -----------------------------------------------------------------------------
// ut_12_1_07: Focus-Free Headless Screenshot & Self-Contained Offline HTML Visual Diff Suite
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_07_focus_free_diff_suite_and_html_report() {
    // 1. Verify focus-free macOS Chromium CLI arguments
    let args = build_focus_free_chromium_args(
        "file:///tmp/screen.html",
        &PathBuf::from("/tmp/out.png"),
        &PathBuf::from("/tmp/profile"),
        1440,
        900,
    );
    assert!(args.iter().any(|a| a == "--headless=new"));
    assert!(args.iter().any(|a| a == "--disable-gpu"));
    assert!(args.iter().any(|a| a == "--no-first-run"));
    assert!(args
        .iter()
        .any(|a| a == "--disable-background-timer-throttling"));
    assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
    assert!(args.iter().any(|a| a.starts_with("--screenshot=")));

    // 2. Verify SSRF protection rejects external non-loopback URLs
    let ssrf_err = validate_diff_target("https://example.com/evil").unwrap_err();
    assert!(ssrf_err.contains("SSRF_BLOCKED"));

    // 3. Create suite directory with 2 screens & run `cowok-design diff --suite ... --force-no-browser --json`
    let tmp = TempDir::new().unwrap();
    let suite_dir = tmp.path().join("suite");
    fs::create_dir_all(&suite_dir).unwrap();
    fs::write(
        suite_dir.join("01-home.html"),
        "<!doctype html><html><body><main><h1>Home Screen</h1></main></body></html>",
    )
    .unwrap();
    fs::write(
        suite_dir.join("02-settings.html"),
        "<!doctype html><html><body><main><h1>Settings <script>alert(1)</script></h1></main></body></html>",
    )
    .unwrap();

    let suite_report_path = tmp.path().join("reports/suite-diff.html");
    let suite_out = execute_cli_command(
        parse_cli_args(&[
            "diff".into(),
            "--suite".into(),
            suite_dir.to_string_lossy().into_owned(),
            "--out".into(),
            suite_report_path.to_string_lossy().into_owned(),
            "--force-no-browser".into(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(suite_out.exit_code, 0, "stderr: {}", suite_out.stderr);
    let suite_json: serde_json::Value = serde_json::from_str(&suite_out.stdout).unwrap();
    assert_eq!(suite_json["ok"], true);
    assert_eq!(suite_json["data"]["mode"], "suite");
    assert_eq!(suite_json["data"]["verdict"], "UNVERIFIED_NO_BROWSER");
    assert_eq!(suite_json["data"]["focusFree"], true);
    assert_eq!(suite_json["data"]["usedFallback"], true);
    assert_eq!(suite_json["data"]["totalCases"], 2);
    assert_eq!(suite_json["data"]["externalCdnRefsCount"], 0);

    let report_html = fs::read_to_string(&suite_report_path).unwrap();
    assert_eq!(count_external_cdn_refs(&report_html), 0);
    assert!(report_html.contains("Content-Security-Policy"));
    assert!(report_html.contains("01-home"));
    assert!(report_html.contains("02-settings"));
    let actual_sha = format!("{:x}", Sha256::digest(report_html.as_bytes()));
    assert_eq!(
        suite_json["data"]["reportSha256"].as_str(),
        Some(actual_sha.as_str())
    );

    // 4. Exercise `--screenshot` mode cleanly as well
    let shot_report_path = tmp.path().join("reports/single-shot.html");
    let shot_out = execute_cli_command(
        parse_cli_args(&[
            "diff".into(),
            "--screenshot".into(),
            suite_dir
                .join("01-home.html")
                .to_string_lossy()
                .into_owned(),
            "--out".into(),
            shot_report_path.to_string_lossy().into_owned(),
            "--force-no-browser".into(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(shot_out.exit_code, 0, "stderr: {}", shot_out.stderr);
    let shot_json: serde_json::Value = serde_json::from_str(&shot_out.stdout).unwrap();
    assert_eq!(shot_json["data"]["mode"], "screenshot");
    assert_eq!(shot_json["data"]["totalCases"], 1);
    assert!(shot_report_path.is_file());
}

// -----------------------------------------------------------------------------
// ut_12_1_08: Distribution Packaging — Node `npx` Wrapper SHA-256 Verification & Homebrew Formula
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_08_npx_wrapper_sha256_verification() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    let wrapper_js = repo_root.join("packaging/npm/bin/cowok-design.mjs");
    let homebrew_rb = repo_root.join("packaging/homebrew/cowok-design.rb");
    assert!(wrapper_js.is_file(), "missing {}", wrapper_js.display());
    assert!(homebrew_rb.is_file(), "missing {}", homebrew_rb.display());

    // 1. Validate Homebrew formula structure & SHA-256 blocks
    let rb_text = fs::read_to_string(&homebrew_rb).unwrap();
    assert!(rb_text.contains("class CowokDesign < Formula"));
    assert!(rb_text.contains("on_macos"));
    assert!(rb_text.contains("on_linux"));
    assert!(rb_text.contains("on_arm"));
    assert!(rb_text.contains("on_intel"));
    assert!(rb_text.contains("bin.install \"cowok-design\""));
    assert!(rb_text.contains("sha256"));
    assert!(rb_text.contains("test do"));

    // 2. `--print-manifest` returns valid JSON with all 4 target triples
    let print_out = Command::new("node")
        .arg(&wrapper_js)
        .arg("--print-manifest")
        .output()
        .expect("run node --print-manifest");
    assert!(
        print_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&print_out.stderr)
    );
    let manifest_json: serde_json::Value = serde_json::from_slice(&print_out.stdout).unwrap();
    for key in ["darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64"] {
        assert!(manifest_json["checksums"][key]["sha256"].is_string());
    }

    // 3. Create executable fake binary, verify matching SHA-256 succeeds & forwards args
    let tmp = TempDir::new().unwrap();
    let fake_bin = tmp.path().join("fake-cowok-design.sh");
    fs::write(&fake_bin, "#!/bin/sh\necho \"verified-run:$*\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let bin_bytes = fs::read(&fake_bin).unwrap();
    let valid_sha = format!("{:x}", Sha256::digest(&bin_bytes));

    let ok_run = Command::new("node")
        .arg(&wrapper_js)
        .arg("doctor")
        .arg("--json")
        .env("COWOK_DESIGN_BIN", &fake_bin)
        .env("COWOK_DESIGN_EXPECTED_SHA256", &valid_sha)
        .output()
        .expect("run wrapper with valid sha");
    assert!(
        ok_run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&ok_run.stderr)
    );
    let stdout_str = String::from_utf8_lossy(&ok_run.stdout);
    assert!(stdout_str.contains("verified-run:doctor --json"));

    // 4. Tampered SHA-256 MUST fail closed with E_CHECKSUM_MISMATCH (exit code 3)
    let tampered_sha = "0".repeat(64);
    let fail_run = Command::new("node")
        .arg(&wrapper_js)
        .arg("--version")
        .env("COWOK_DESIGN_BIN", &fake_bin)
        .env("COWOK_DESIGN_EXPECTED_SHA256", &tampered_sha)
        .output()
        .expect("run wrapper with tampered sha");
    assert_eq!(fail_run.status.code(), Some(3));
    let fail_stderr = String::from_utf8_lossy(&fail_run.stderr);
    assert!(fail_stderr.contains("E_CHECKSUM_MISMATCH"));

    // 5. `--verify-binary --delete-on-mismatch` deletes unverified temp file cleanly
    let temp_unverified = tmp.path().join("unverified.bin");
    fs::write(&temp_unverified, b"tampered-payload").unwrap();
    let verify_fail = Command::new("node")
        .arg(&wrapper_js)
        .arg("--verify-binary")
        .arg(&temp_unverified)
        .arg("--expected-sha256")
        .arg(&tampered_sha)
        .arg("--delete-on-mismatch")
        .output()
        .expect("run --verify-binary");
    assert_eq!(verify_fail.status.code(), Some(3));
    assert!(!temp_unverified.exists(), "tampered binary must be deleted");
}

// -----------------------------------------------------------------------------
// ut_12_1_09 (F-01): Strict URL-Parsed SSRF Defense (Userinfo, Backslash, Non-Loopback)
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_09_f01_ssrf_userinfo_and_backslash_blocked() {
    for evil in [
        "http://127.0.0.1:x@169.254.169.254/",
        "https://localhost:secret@evil.example/metadata",
        "http://user@127.0.0.1:4170/",
        "http://127.0.0.1\\@169.254.169.254/",
        "http://127.0.0.1.evil.example/",
        "ftp://127.0.0.1/etc/passwd",
        "file:///etc/passwd",
    ] {
        let err = validate_diff_target(evil).expect_err(&format!("must reject {evil}"));
        assert!(
            err.contains("SSRF_BLOCKED"),
            "expected SSRF_BLOCKED for {evil}, got: {err}"
        );
    }

    assert!(validate_diff_target("http://127.0.0.1:4170/workbench").is_ok());
    assert!(validate_diff_target("http://localhost:4170/").is_ok());
}

// -----------------------------------------------------------------------------
// ut_12_1_10 (F-02 & F-05): Symlinked npm Wrapper Entrypoint & Supply-Chain Hardening
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_10_f02_f05_npm_wrapper_symlink_and_supply_chain() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    let wrapper_js = repo_root.join("packaging/npm/bin/cowok-design.mjs");
    let pkg_json = repo_root.join("packaging/npm/package.json");
    let homebrew_rb = repo_root.join("packaging/homebrew/cowok-design.rb");

    let empty_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    // F-05: Manifest & Homebrew formula must NOT contain empty-file SHA-256 placeholders
    let pkg_text = fs::read_to_string(&pkg_json).unwrap();
    let rb_text = fs::read_to_string(&homebrew_rb).unwrap();
    assert!(!pkg_text.contains(empty_sha256));
    assert!(!rb_text.contains(empty_sha256));
    assert!(pkg_text.contains("REPLACE_WITH_RELEASE_SHA256_"));
    assert!(rb_text.contains("REPLACE_WITH_RELEASE_SHA256_"));

    let tmp = TempDir::new().unwrap();

    // F-02: Invoking cowok-design.mjs through a symlink (like node_modules/.bin/cowok-design) MUST run main()
    #[cfg(unix)]
    {
        let bin_dir = tmp.path().join("node_modules/.bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let symlink_entry = bin_dir.join("cowok-design");
        std::os::unix::fs::symlink(&wrapper_js, &symlink_entry).unwrap();

        let sym_out = Command::new("node")
            .arg(&symlink_entry)
            .arg("--print-manifest")
            .output()
            .expect("run symlinked wrapper");
        assert!(
            sym_out.status.success(),
            "symlinked wrapper failed: {}",
            String::from_utf8_lossy(&sym_out.stderr)
        );
        let parsed: serde_json::Value =
            serde_json::from_slice(&sym_out.stdout).expect("valid manifest json from symlink");
        assert_eq!(parsed["name"], "@cowok-design/cli");
    }

    // F-05: Empty 0-byte file MUST be rejected even when expected SHA-256 is EMPTY_FILE_SHA256
    let empty_bin = tmp.path().join("empty.bin");
    fs::write(&empty_bin, b"").unwrap();
    let empty_verify = Command::new("node")
        .arg(&wrapper_js)
        .arg("--verify-binary")
        .arg(&empty_bin)
        .arg("--expected-sha256")
        .arg(empty_sha256)
        .arg("--delete-on-mismatch")
        .output()
        .expect("verify empty file");
    assert_eq!(empty_verify.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&empty_verify.stderr).contains("E_CHECKSUM_MISMATCH"));
    assert!(
        !empty_bin.exists(),
        "empty binary must be deleted on mismatch"
    );

    // F-05: Placeholder manifest checksums MUST fail closed BEFORE downloading
    let unconfigured_run = Command::new("node")
        .arg(&wrapper_js)
        .arg("--version")
        .env_remove("COWOK_DESIGN_BIN")
        .env_remove("COWOK_DESIGN_EXPECTED_SHA256")
        .env("XDG_CACHE_HOME", tmp.path().join("cache"))
        .output()
        .expect("run with unconfigured release sha");
    assert_eq!(unconfigured_run.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&unconfigured_run.stderr).contains("E_CHECKSUM_MISMATCH"));

    // F-05: COWOK_DESIGN_BIN without COWOK_DESIGN_EXPECTED_SHA256 MUST fail closed unless COWOK_ALLOW_UNVERIFIED_DEV_BIN=1
    let dev_bin = tmp.path().join("dev-cowok.sh");
    fs::write(&dev_bin, "#!/bin/sh\necho \"dev-bin-ok\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dev_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let missing_sha_run = Command::new("node")
        .arg(&wrapper_js)
        .arg("--version")
        .env("COWOK_DESIGN_BIN", &dev_bin)
        .env_remove("COWOK_DESIGN_EXPECTED_SHA256")
        .env_remove("COWOK_ALLOW_UNVERIFIED_DEV_BIN")
        .output()
        .expect("run COWOK_DESIGN_BIN without sha");
    assert_eq!(missing_sha_run.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&missing_sha_run.stderr).contains("E_CHECKSUM_REQUIRED"));

    let dev_opt_in_run = Command::new("node")
        .arg(&wrapper_js)
        .arg("--version")
        .env("COWOK_DESIGN_BIN", &dev_bin)
        .env_remove("COWOK_DESIGN_EXPECTED_SHA256")
        .env("COWOK_ALLOW_UNVERIFIED_DEV_BIN", "1")
        .output()
        .expect("run COWOK_DESIGN_BIN with dev opt-in");
    assert!(dev_opt_in_run.status.success());
    assert!(String::from_utf8_lossy(&dev_opt_in_run.stdout).contains("dev-bin-ok"));
}

// -----------------------------------------------------------------------------
// ut_12_1_11 (F-03): Outermost Loopback/CORS Guard & No `//api/` / `/API/` / `..` SPA Leak
// -----------------------------------------------------------------------------
#[tokio::test]
async fn ut_12_1_11_f03_standalone_loopback_cors_and_api_normalization() {
    let (_dir, registry) = setup_test_registry();
    let state = DaemonAppState::new(registry, "127.0.0.1".to_string(), 4170);
    let app = wrap_with_standalone_parity(
        build_router(state.clone()),
        state,
        StaticSpaHandler::embedded_fallback(),
    );

    // 1. Evil Host header MUST be rejected with 403 on SPA `/`, aliased `/api/projects`, AND `/api/nonexistent`
    for path in ["/", "/api/projects", "/api/nonexistent"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(path)
                    .header("host", "evil.example:4170")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "expected 403 for evil Host on {path}"
        );
    }

    // 2. `//api/projects`, `/API/projects`, `/API/health`, `//api` MUST NOT fall through to SPA HTML
    for api_variant in ["//api/projects", "/API/projects", "/API/health", "//api"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(api_variant)
                    .header("host", "127.0.0.1:4170")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "expected JSON 404 (not SPA 200) for {api_variant}"
        );
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains("application/json"),
            "expected application/json for {api_variant}, got {ct}"
        );
        let body = read_body_json(resp).await;
        assert_eq!(body["error"]["code"].as_str(), Some("NOT_FOUND"));
    }

    // 3. Unsafe path tokens (`..`, `\`, percent-encoded `..`) MUST return JSON 400 BAD_REQUEST
    for unsafe_path in ["/api/../secret", "/assets/%2e%2e/secret", "/api/%5cfoo"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(unsafe_path)
                    .header("host", "127.0.0.1:4170")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "expected 400 BAD_REQUEST for {unsafe_path}"
        );
        let body = read_body_json(resp).await;
        assert_eq!(body["error"]["code"].as_str(), Some("BAD_REQUEST"));
    }
}

// -----------------------------------------------------------------------------
// ut_12_1_12 (F-04): ActiveWorkerRegistry Drained & Reaped on Shutdown
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_12_f04_reap_active_workers_on_shutdown() {
    use harness_daemon::routes::execution_reaper::{
        spawn_pgid_child, ActiveWorkerEntry, ActiveWorkerRegistry,
    };

    let workers = ActiveWorkerRegistry::new();
    let (hb_tx, hb_rx) = tokio::sync::watch::channel(false);

    let mut sleep_cmd = Command::new("sleep");
    sleep_cmd.arg("30");
    let (mut child, pid, pgid, start_time) =
        spawn_pgid_child(sleep_cmd).expect("spawn pgid worker");

    workers.register(ActiveWorkerEntry {
        job_id: "job-f04".to_string(),
        project_id: "proj-f04".to_string(),
        fencing_token: 1,
        pid,
        pgid,
        start_time,
        shadow_dir: PathBuf::from("/tmp/nonexistent-shadow-f04"),
    });
    workers.register_heartbeat("job-f04", 1, hb_tx);

    let reaper = ChildProcessReaper::new();
    reaper.set_active_workers(workers.clone());

    let summary = reaper.reap_all();
    let _ = child.wait();

    assert_eq!(summary.active_workers_reaped, 1);
    assert!(
        *hb_rx.borrow(),
        "worker heartbeat must be cancelled on reap_all"
    );
    assert!(
        workers.take_all().is_empty(),
        "ActiveWorkerRegistry must be drained after reap_all"
    );
    #[cfg(unix)]
    {
        let probe = unsafe { libc::kill(-pgid, 0) };
        assert_ne!(probe, 0, "worker process group must be terminated");
    }
}

// -----------------------------------------------------------------------------
// ut_12_1_13 (F-06, F-07, F-08): Intermediate Symlinks, Baseline Validation, HTML Escape & PNG Fallback
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_13_f06_f07_f08_symlink_ancestors_baseline_xss_and_png_fallback() {
    let ws = TempDir::new().unwrap();
    let ws_str = ws.path().to_string_lossy().into_owned();

    // F-06a: Intermediate symlink `out -> outside` with non-existent `sub/` must be rejected BEFORE create_dir_all
    #[cfg(unix)]
    {
        let outside = TempDir::new().unwrap();
        let sym_out = ws.path().join("out");
        std::os::unix::fs::symlink(outside.path(), &sym_out).unwrap();

        // 1) CLI export rejects intermediate symlink
        let escape_export = sym_out.join("sub/deck.html");
        let exp_res = execute_cli_command(
            parse_cli_args(&[
                "export".into(),
                "html".into(),
                "--project".into(),
                "proj-sym".into(),
                "--out".into(),
                escape_export.to_string_lossy().into_owned(),
                "--workspace".into(),
                ws_str.clone(),
                "--json".into(),
            ])
            .unwrap(),
        );
        assert_ne!(exp_res.exit_code, 0);
        assert!(exp_res.stdout.contains("INVALID_OUT_PATH"));
        assert!(
            !outside.path().join("sub").exists(),
            "create_dir_all must not follow intermediate symlink"
        );

        // 2) Visual diff rejects intermediate symlink output path
        let screen = ws.path().join("screen.html");
        fs::write(
            &screen,
            "<!doctype html><html><body><h1>Hi</h1></body></html>",
        )
        .unwrap();
        let escape_diff = sym_out.join("sub/report.html");
        let diff_res = execute_cli_command(
            parse_cli_args(&[
                "diff".into(),
                "--screenshot".into(),
                screen.to_string_lossy().into_owned(),
                "--out".into(),
                escape_diff.to_string_lossy().into_owned(),
                "--force-no-browser".into(),
                "--json".into(),
            ])
            .unwrap(),
        );
        assert_ne!(diff_res.exit_code, 0);
        assert!(diff_res.stdout.contains("SYMLINK_REJECTED"));
        assert!(!outside.path().join("sub").exists());

        // 3) Visual diff rejects symlinked --baseline file
        let real_base = outside.path().join("base.html");
        fs::write(&real_base, "<html></html>").unwrap();
        let sym_base = ws.path().join("base-link.html");
        std::os::unix::fs::symlink(&real_base, &sym_base).unwrap();
        let safe_out = ws.path().join("safe/diff.html");
        let base_sym_res = execute_cli_command(
            parse_cli_args(&[
                "diff".into(),
                "--screenshot".into(),
                screen.to_string_lossy().into_owned(),
                "--baseline".into(),
                sym_base.to_string_lossy().into_owned(),
                "--out".into(),
                safe_out.to_string_lossy().into_owned(),
                "--force-no-browser".into(),
                "--json".into(),
            ])
            .unwrap(),
        );
        assert_ne!(base_sym_res.exit_code, 0);
        assert!(base_sym_res.stdout.contains("SYMLINK_REJECTED"));
    }

    // F-06b: Visual diff rejects `..` in --baseline
    let screen = ws.path().join("screen2.html");
    fs::write(
        &screen,
        "<!doctype html><html><body><h1>Screen</h1></body></html>",
    )
    .unwrap();
    let trav_out = ws.path().join("safe/diff-trav.html");
    let trav_base_res = execute_cli_command(
        parse_cli_args(&[
            "diff".into(),
            "--screenshot".into(),
            screen.to_string_lossy().into_owned(),
            "--baseline".into(),
            "../escape.png".into(),
            "--out".into(),
            trav_out.to_string_lossy().into_owned(),
            "--force-no-browser".into(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_ne!(trav_base_res.exit_code, 0);
    assert!(trav_base_res.stdout.contains("PATH_TRAVERSAL_DENIED"));

    // F-07: HTML fallback export MUST escape project_name and project_id
    let xss_out = ws.path().join("safe/xss-check.html");
    let xss_res = execute_cli_command(
        parse_cli_args(&[
            "export".into(),
            "html".into(),
            "--project".into(),
            "<img src=x onerror=alert(1)>".into(),
            "--title".into(),
            "A & <script>alert('xss')</script> \"quote\"".into(),
            "--out".into(),
            xss_out.to_string_lossy().into_owned(),
            "--workspace".into(),
            ws_str,
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(xss_res.exit_code, 0, "stderr: {}", xss_res.stderr);
    let exported_html = fs::read_to_string(&xss_out).unwrap();
    assert!(!exported_html.contains("<script>alert('xss')</script>"));
    assert!(!exported_html.contains("<img src=x onerror=alert(1)>"));
    assert!(exported_html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(exported_html.contains("&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"));

    // F-08: When `--out shot.png` degrades to HTML fallback, write `<stem>.html`, NEVER HTML into `.png`
    let png_out = ws.path().join("safe/shot.png");
    let html_fallback = ws.path().join("safe/shot.html");
    let png_res = execute_cli_command(
        parse_cli_args(&[
            "diff".into(),
            "--screenshot".into(),
            screen.to_string_lossy().into_owned(),
            "--out".into(),
            png_out.to_string_lossy().into_owned(),
            "--force-no-browser".into(),
            "--json".into(),
        ])
        .unwrap(),
    );
    assert_eq!(png_res.exit_code, 0, "stderr: {}", png_res.stderr);
    let png_json: serde_json::Value = serde_json::from_str(&png_res.stdout).unwrap();
    assert!(
        !png_out.exists(),
        "HTML fallback must NEVER be written into .png path"
    );
    assert!(
        html_fallback.is_file(),
        "HTML fallback must be written to shot.html"
    );
    assert_eq!(
        png_json["data"]["reportPath"].as_str(),
        Some(html_fallback.to_string_lossy().as_ref())
    );
    let fallback_content = fs::read_to_string(&html_fallback).unwrap();
    assert!(fallback_content.contains("<!DOCTYPE html>"));
}

// -----------------------------------------------------------------------------
// ut_12_1_14 (F-09): Global `--json` Flag Accepted Before Subcommand
// -----------------------------------------------------------------------------
#[test]
fn ut_12_1_14_f09_global_json_flag_before_subcommand() {
    let cmd = parse_cli_args(&["--json".into(), "doctor".into()]).expect("--json doctor parses");
    let out = execute_cli_command(cmd);
    assert_eq!(out.exit_code, 0, "stderr: {}", out.stderr);
    let parsed: serde_json::Value =
        serde_json::from_str(&out.stdout).expect("--json doctor emits valid json");
    assert_eq!(parsed["scanMode"], "passive");
    assert!(parsed["tools"].as_array().unwrap().len() >= 20);

    let export_cmd = parse_cli_args(&[
        "--json".into(),
        "export".into(),
        "html".into(),
        "--project".into(),
        "p1".into(),
        "--out".into(),
        "/tmp/out.html".into(),
    ])
    .expect("--json export parses");
    match export_cmd {
        harness_daemon::cli::CliCommand::Export { json, .. } => assert!(json),
        other => panic!("expected Export command, got {other:?}"),
    }

    let err = parse_cli_args(&["--json".into(), "not-a-command".into()]).unwrap_err();
    assert_eq!(err.code, "UNKNOWN_SUBCOMMAND");
    assert!(err.json_requested);
}
