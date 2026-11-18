// @story 04.3
// @cover AC-01, AC-02, AC-03, AC-04
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::Arc;
use std::time::Instant;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::legacy_workbench_v6::check_ssrf_socket_addr;
use harness_daemon::vault::VaultStore;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn setup_daemon(tmp: &TempDir) -> (Arc<DaemonAppState>, axum::Router, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open sqlite");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let tok_a = format!("sess-a-{}", uuid::Uuid::new_v4());
    let exp = Utc::now().timestamp() + 3600;
    state.auth.insert_session(
        tok_a.clone(),
        FlowPrincipal {
            tenant_id: "tenant-a".into(),
            user_id: "u1".into(),
            role: "editor".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok_a)
}

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    hdrs: &[(&str, &str)],
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170");
    if !raw.is_empty() {
        b = b.header("content-type", "application/json");
    }
    for (k, v) in hdrs {
        b = b.header(*k, *v);
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

fn spawn_allowed_loopback_http_sequence(responses: &[(&str, &str, &str)]) -> Option<u16> {
    let mut listener_opt = None;
    for candidate_port in [1234u16, 8000, 5000, 8080, 11434] {
        if let Ok(l) = TcpListener::bind(("127.0.0.1", candidate_port)) {
            listener_opt = Some(l);
            break;
        }
    }
    let listener = listener_opt?;
    let port = listener.local_addr().unwrap().port();
    let payloads: Vec<String> = responses
        .iter()
        .map(|(status_line, extra_headers, body)| {
            format!(
                "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n{body}",
                body.len()
            )
        })
        .collect();
    std::thread::spawn(move || {
        for resp_str in payloads {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(resp_str.as_bytes());
            }
        }
    });
    Some(port)
}

fn spawn_ephemeral_http_server(status_line: &str, extra_headers: &str, body: &str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let resp_str = format!(
        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n{body}",
        body.len()
    );
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(resp_str.as_bytes());
        }
    });
    port
}

#[test]
fn test_ssrf_socket_addr_blocklist_and_ollama_exception() {
    let blocked: &[&str] = &[
        "0.0.0.0:80",
        "127.0.0.1:8080",
        "10.0.0.1:443",
        "172.16.0.1:443",
        "192.168.1.1:443",
        "169.254.169.254:80",
        "100.64.0.1:443",
        "198.18.0.1:443",
        "240.0.0.1:443",
        "224.0.0.1:443",
        "[::]:80",
        "[::1]:80",
        "[::ffff:127.0.0.1]:80",
        "[::ffff:169.254.169.254]:80",
        "[64:ff9b::127.0.0.1]:80",
        "[fc00::1]:80",
        "[fe80::1]:80",
    ];
    for raw in blocked {
        let addr: SocketAddr = raw.parse().unwrap();
        assert!(
            check_ssrf_socket_addr(addr, false).is_err(),
            "expected SSRF block for {raw}"
        );
    }

    let public_v4: SocketAddr = "93.184.216.34:443".parse().unwrap();
    assert!(check_ssrf_socket_addr(public_v4, false).is_ok());

    let ollama_lo: SocketAddr = "127.0.0.1:11434".parse().unwrap();
    assert!(check_ssrf_socket_addr(ollama_lo, true).is_ok());
    let imds: SocketAddr = "169.254.169.254:80".parse().unwrap();
    assert!(check_ssrf_socket_addr(imds, true).is_err());
}

#[tokio::test]
async fn test_verify_provider_key_ssrf_redirect_dry_run_and_redaction() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (_s, router, tok_a) = setup_daemon(&tmp);
    let dummy_secret = format!("sk-secret-do-not-leak-{}", "9".repeat(8));

    for bad_url in [
        "http://[::ffff:127.0.0.1]/v1",
        "http://169.254.169.254/latest/meta-data",
        "http://metadata.google.internal/computeMetadata/v1/",
        "http://100.64.0.1/v1",
        "https://10.0.0.1/v1",
        "https://user@127.0.0.1/v1",
        "http://api.openai.com/v1/models",
    ] {
        let (st, val) = send_req(
            &router,
            "POST",
            "/api/v1/doctor/verify-key",
            &[("x-cowok-session-token", &tok_a)],
            Some(json!({
                "providerId": "byos",
                "apiKey": dummy_secret,
                "endpointUrl": bad_url
            })),
        )
        .await;
        assert_eq!(
            st,
            StatusCode::OK,
            "expected JSON validation rejection for {bad_url}: {val}"
        );
        assert_eq!(val["valid"], false);
        assert_eq!(val["status"], "invalid_endpoint");
        assert!(!val.to_string().contains(&dummy_secret));
    }

    // Non-allowlisted ephemeral loopback port for Ollama must be blocked by ALLOWED_LOOPBACK_PORTS
    let blocked_port = spawn_ephemeral_http_server("200 OK", "", "{\"models\":[]}");
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/verify-key",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "providerId": "ollama",
            "apiKey": "",
            "endpointUrl": format!("http://127.0.0.1:{blocked_port}/api/tags")
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["valid"], false);
    assert_eq!(val["status"], "invalid_endpoint");

    // Allowlisted loopback server handling: 302 IMDS redirect -> 200 OK -> 401 Unauthorized
    let loopback_port = spawn_allowed_loopback_http_sequence(&[
        (
            "302 Found",
            "Location: http://169.254.169.254/latest/meta-data\r\n",
            "redirect",
        ),
        ("200 OK", "", "{\"models\":[]}"),
        ("401 Unauthorized", "", "{\"error\":\"unauthorized\"}"),
    ])
    .expect("at least one allowlisted loopback port should be free for test");

    // Redirect (302 -> IMDS) must NOT be followed by reqwest Policy::none()
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/verify-key",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "providerId": "ollama",
            "apiKey": "",
            "endpointUrl": format!("http://127.0.0.1:{loopback_port}/api/tags")
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["valid"], false, "redirect must not be followed: {val}");

    // Live 200 OK against local Ollama loopback
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/verify-key",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "providerId": "ollama",
            "apiKey": "",
            "endpointUrl": format!("http://127.0.0.1:{loopback_port}/api/tags")
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["valid"], true);
    assert_eq!(val["status"], "live_verified");

    // Live 401 Unauthorized + credential redaction
    let unauth_key = format!("ollama-secret-keep-hidden-{}", "7".repeat(4));
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/verify-key",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "providerId": "ollama",
            "apiKey": unauth_key,
            "endpointUrl": format!("http://127.0.0.1:{loopback_port}/api/tags")
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["valid"], false);
    assert_eq!(val["status"], "unauthorized");
    assert_eq!(val["credentialRef"], "redacted:****7777");
    assert!(!val.to_string().contains(&unauth_key));

    // dryRunFormatOnly for Gemini without upstream HTTP call
    let dry_key = format!("AIzaSyFormatOnly{}", "8".repeat(20));
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/verify-key",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "providerId": "gemini",
            "apiKey": dry_key,
            "endpointUrl": "",
            "dryRunFormatOnly": true
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["valid"], true);
    assert_eq!(val["status"], "format_checked");
    assert_eq!(val["liveProbed"], false);
    assert_eq!(val["credentialRef"], "redacted:****8888");
    assert!(!val.to_string().contains(&dry_key));
}

#[tokio::test]
async fn test_vault_aead_0600_symlink_reject_and_anti_downgrade() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let key_dir = tmp.path().join("external-keys");
    std::env::set_var("COWOK_VAULT_KEY_DIR", &key_dir);

    let store = VaultStore::from_workspace_root(tmp.path());
    let s1 = format!("sk-first-{}", "1".repeat(16));
    let s2 = format!("sk-ant-second-{}", "2".repeat(16));

    let mut secrets = BTreeMap::new();
    secrets.insert("openai".to_string(), s1.clone());
    store.seal_secrets(&secrets, Some("master-pass-1")).unwrap();
    let raw1 = fs::read(store.vault_path()).unwrap();

    secrets.insert("anthropic".to_string(), s2.clone());
    store.seal_secrets(&secrets, Some("master-pass-1")).unwrap();
    let raw2 = fs::read(store.vault_path()).unwrap();
    assert_ne!(raw1, raw2, "CSPRNG salt+nonce must randomize each seal");
    let mode = fs::metadata(store.vault_path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    let unsealed = store.unseal_secrets(Some("master-pass-1")).unwrap();
    assert_eq!(
        unsealed.get("openai").map(String::as_str),
        Some(s1.as_str())
    );
    assert_eq!(
        unsealed.get("anthropic").map(String::as_str),
        Some(s2.as_str())
    );

    // Wrong passphrase must fail GCM authentication
    assert!(store.unseal_secrets(Some("wrong-pass")).is_err());

    // Tampered ciphertext must fail authentication
    let mut tampered = raw2.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xFF;
    fs::write(store.vault_path(), tampered).unwrap();
    assert!(store.unseal_secrets(Some("master-pass-1")).is_err());

    // Symlinked vault.enc must be rejected fail-closed
    fs::remove_file(store.vault_path()).unwrap();
    let decoy = tmp.path().join("decoy.txt");
    fs::write(&decoy, "decoy").unwrap();
    symlink(&decoy, store.vault_path()).unwrap();
    assert!(store.unseal_secrets(Some("master-pass-1")).is_err());
    fs::remove_file(store.vault_path()).unwrap();

    let (_s, router, tok_a) = setup_daemon(&tmp);

    // Ephemeral downgrade must return 409 VAULT_DOWNGRADE_FORBIDDEN when vault is writable
    let (st, val) = send_req(
        &router,
        "PUT",
        "/api/v1/settings/vault",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "vaultMode": "ephemeral" })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{val}");
    assert_eq!(val["code"], "VAULT_DOWNGRADE_FORBIDDEN");

    let (st, val) = send_req(
        &router,
        "PUT",
        "/api/v1/agent-settings",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({
            "providerId": "openai",
            "modelId": "gpt-4o",
            "vaultMode": "ephemeral",
            "permissionMode": "ask-every-time",
            "onboardingDone": true
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{val}");
    assert_eq!(val["code"], "VAULT_DOWNGRADE_FORBIDDEN");

    std::env::remove_var("COWOK_VAULT_KEY_DIR");
}

#[tokio::test]
async fn test_passive_doctor_ide_extension_and_bounded_pgid_probe() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let fake_home = tmp.path().join("home");
    let ext_dir = fake_home.join(".vscode/extensions/saoudrizwan.claude-dev-3.0.0");
    fs::create_dir_all(&ext_dir).unwrap();

    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let claude_stub = bin_dir.join("claude");
    // Spawn a child sleep in the same process group to prove -PGID kills the whole group within ~1500ms
    fs::write(&claude_stub, "#!/bin/sh\n/bin/sleep 15 &\nwait\n").unwrap();
    fs::set_permissions(&claude_stub, fs::Permissions::from_mode(0o755)).unwrap();

    let orig_home = std::env::var_os("HOME");
    let orig_path = std::env::var_os("PATH");
    std::env::set_var("HOME", &fake_home);
    std::env::set_var("PATH", bin_dir.to_string_lossy().as_ref());

    let (_s, router, tok_a) = setup_daemon(&tmp);

    // Passive scan completes quickly without running hanging `claude` subprocess
    let t_passive = Instant::now();
    let (st, val) = send_req(
        &router,
        "GET",
        "/api/v1/doctor/clis",
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    let passive_elapsed = t_passive.elapsed().as_millis();
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["scanMode"], "passive");
    assert!(
        passive_elapsed < 1200,
        "passive scan took {passive_elapsed}ms"
    );

    let clis = val["clis"].as_array().unwrap();
    assert!(clis.len() >= 20, "expected >=20 CLIs, got {}", clis.len());
    let cline = clis.iter().find(|c| c["id"] == "cline").unwrap();
    assert_eq!(cline["installed"], true);
    assert_eq!(cline["status"], "manual_ide_setup");

    // Unknown/disallowed CLI id is rejected with 404 CLI_NOT_FOUND
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/probe-cli",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "cliId": "sh -c id" })),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "{val}");
    assert_eq!(val["code"], "CLI_NOT_FOUND");

    // Bounded -PGID active probe kills hanging process group within ~1500ms budget
    let t_probe = Instant::now();
    let (st, val) = send_req(
        &router,
        "POST",
        "/api/v1/doctor/probe-cli",
        &[("x-cowok-session-token", &tok_a)],
        Some(json!({ "cliId": "claude" })),
    )
    .await;
    let probe_elapsed = t_probe.elapsed().as_millis();
    assert_eq!(st, StatusCode::OK, "{val}");
    assert_eq!(val["status"], "timeout", "{val}");
    assert!(
        probe_elapsed < 3500,
        "PGID kill exceeded bounded budget: {probe_elapsed}ms"
    );

    if let Some(h) = orig_home {
        std::env::set_var("HOME", h);
    } else {
        std::env::remove_var("HOME");
    }
    if let Some(p) = orig_path {
        std::env::set_var("PATH", p);
    } else {
        std::env::remove_var("PATH");
    }
}

#[tokio::test]
async fn test_plugins_list_tenant_isolation_and_enabled_filter() {
    let _g = ENV_LOCK.lock().await;
    let tmp = tempfile::tempdir().unwrap();
    let (state, router, tok_a) = setup_daemon(&tmp);
    {
        let conn = state.registry.pool().connection().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO projects (id, slug, name, product_brief, workspace_root, project_code, tenant_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, '', '.', 'P-043', 'tenant-a', ?4, ?4)",
            params!["11111111-1111-4111-8111-111111111111", "default-p", "Default", now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects (id, slug, name, product_brief, workspace_root, project_code, tenant_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, '', '.', 'P-043-B', 'tenant-b', ?4, ?4)",
            params!["22222222-2222-4222-8222-222222222222", "other-p", "Other", now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skill_plugins (id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "plug-043-enabled",
                "11111111-1111-4111-8111-111111111111",
                "cli-doctor-extension",
                "1.0.0",
                "ACTIVE",
                "local://src/plugins/cli-doctor-extension",
                "{\"id\":\"cli-doctor-extension\"}",
                now
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skill_plugins (id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "plug-043-disabled",
                "11111111-1111-4111-8111-111111111111",
                "disabled-extension",
                "1.0.0",
                "DEPRECATED",
                "local://src/plugins/disabled-extension",
                "{\"id\":\"disabled-extension\"}",
                now
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO skill_plugins (id, project_id, plugin_slug, version, lifecycle_state, source_repo_url, manifest_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "plug-043-tenant-b",
                "22222222-2222-4222-8222-222222222222",
                "tenant-b-secret-plugin",
                "1.0.0",
                "ACTIVE",
                "local://src/plugins/tenant-b",
                "{\"id\":\"tenant-b\"}",
                now
            ],
        )
        .unwrap();
    }

    let (st, val) = send_req(
        &router,
        "GET",
        "/api/v1/plugins?enabled=true",
        &[("x-cowok-session-token", &tok_a)],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{val}");
    let list = val.as_array().unwrap();
    assert_eq!(list.len(), 1, "expected 1 enabled tenant-a plugin: {val}");
    assert_eq!(list[0]["pluginSlug"], "cli-doctor-extension");
    assert_eq!(list[0]["enabled"], true);
}
