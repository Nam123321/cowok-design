// @story 04.1
// @cover AC-01, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use harness_daemon::{
    daemon_auth::FlowPrincipal, read_daemon_lock, routes::hmac_sha256, DaemonConfig, DaemonServer,
};
use http_body_util::BodyExt;
use rusqlite::params;
use sha2::{Digest, Sha256};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

async fn send_req(
    router: &axum::Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body_bytes: Vec<u8>,
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    if !body_bytes.is_empty() {
        b = b.header("content-type", "application/json");
    }
    let req = b.body(Body::from(body_bytes)).expect("req");
    let resp = router.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let val = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned())
        })
    };
    (status, val)
}

#[tokio::test]
async fn sec_daemon_01_to_05_dual_ingress_hmac_v2_replay_roles_and_redacted_health() {
    let tmp = TempDir::new().expect("tmp");
    let canon_root = tmp.path().canonicalize().expect("canon");
    let db_path = canon_root.join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &canon_root)).expect("wal");
    let project_id = Uuid::new_v4().to_string();
    pool.connection()
        .expect("conn")
        .execute(
            "INSERT INTO projects (id, tenant_id, slug, name, product_brief, workspace_root, created_at, updated_at) \
             VALUES (?1, 'local-default', 'sec-proj', 'Sec Proj', 'Brief', '.', 1700000000000, 1700000000000)",
            params![project_id],
        )
        .expect("seed");

    let registry = Arc::new(SpecRegistry::new(pool));
    let cfg = DaemonConfig {
        host: "127.0.0.1".into(),
        preferred_port: 0,
        workspace_root: canon_root.clone(),
    };
    let server = DaemonServer::bind_loopback(&cfg, registry).expect("bind");
    let router = server.router();

    // SEC-DAEMON-01: .cowok-design is 0700 and daemon.lock is 0600
    let dir_mode = std::fs::metadata(canon_root.join(".cowok-design"))
        .expect("meta")
        .permissions()
        .mode()
        & 0o777;
    let lock_mode = std::fs::metadata(canon_root.join(".cowok-design/daemon.lock"))
        .expect("meta")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700);
    assert_eq!(lock_mode, 0o600);

    // SEC-DAEMON-02: /api/v1/health exposes schema_version=6, NEVER daemon_nonce or hmac_key_hex
    let lock = read_daemon_lock(&canon_root).expect("lock");
    let bound_host = format!("127.0.0.1:{}", lock.port);

    // Missing Host or cross-port Host MUST fail closed with 403 ORIGIN_REJECTED
    let (missing_host_st, missing_host_json) =
        send_req(&router, "GET", "/api/v1/health", &[], vec![]).await;
    assert_eq!(missing_host_st, StatusCode::FORBIDDEN);
    assert_eq!(missing_host_json["code"], "ORIGIN_REJECTED");

    let cross_port = if lock.port == 9999 { 9998 } else { 9999 };
    let cross_host = format!("127.0.0.1:{cross_port}");
    let (cross_port_st, cross_port_json) = send_req(
        &router,
        "GET",
        "/api/v1/health",
        &[("host", cross_host.as_str())],
        vec![],
    )
    .await;
    assert_eq!(cross_port_st, StatusCode::FORBIDDEN);
    assert_eq!(cross_port_json["code"], "ORIGIN_REJECTED");

    let (h_st, h_json) = send_req(
        &router,
        "GET",
        "/api/v1/health",
        &[("host", bound_host.as_str())],
        vec![],
    )
    .await;
    assert_eq!(h_st, StatusCode::OK);
    assert_eq!(
        h_json["schema_version"],
        harness_daemon::lockfile::CURRENT_SCHEMA_VERSION
    );
    assert!(h_json.get("hmac_key_hex").is_none());
    assert!(h_json.get("daemon_nonce").is_none());

    let snap_uri = format!("/api/v1/projects/{project_id}/flow/snapshot");
    let nodes_uri = format!("/api/v1/projects/{project_id}/flow/nodes");

    // SEC-DAEMON-03: Unauthenticated flow call rejected with 401 UNAUTHENTICATED; DNS rebinding Host rejected 403
    let (u_st, u_json) = send_req(
        &router,
        "GET",
        &snap_uri,
        &[("host", bound_host.as_str())],
        vec![],
    )
    .await;
    assert_eq!(u_st, StatusCode::UNAUTHORIZED);
    assert_eq!(u_json["code"], "UNAUTHENTICATED");

    let (evil_st, _) = send_req(
        &router,
        "GET",
        &snap_uri,
        &[("host", "evil.example.com")],
        vec![],
    )
    .await;
    assert_eq!(evil_st, StatusCode::FORBIDDEN);

    // SEC-DAEMON-04: Unauthenticated caller cannot mint editor token; cross-port caller cannot mint even with daemon_nonce
    let editor_body = serde_json::to_vec(&serde_json::json!({ "role": "editor" })).unwrap();
    let (unauth_ed_st, unauth_ed_json) = send_req(
        &router,
        "POST",
        "/api/v1/auth/local-session",
        &[
            ("host", bound_host.as_str()),
            ("sec-fetch-site", "same-origin"),
        ],
        editor_body.clone(),
    )
    .await;
    assert_eq!(unauth_ed_st, StatusCode::FORBIDDEN);
    assert_eq!(unauth_ed_json["code"], "INSUFFICIENT_ROLE");

    let (cross_ed_st, cross_ed_json) = send_req(
        &router,
        "POST",
        "/api/v1/auth/local-session",
        &[
            ("host", cross_host.as_str()),
            ("sec-fetch-site", "same-origin"),
            ("x-cowok-daemon-nonce", lock.daemon_nonce.as_str()),
        ],
        editor_body.clone(),
    )
    .await;
    assert_eq!(cross_ed_st, StatusCode::FORBIDDEN);
    assert_eq!(cross_ed_json["code"], "ORIGIN_REJECTED");

    let (nonce_ed_st, nonce_ed_json) = send_req(
        &router,
        "POST",
        "/api/v1/auth/local-session",
        &[
            ("host", bound_host.as_str()),
            ("sec-fetch-site", "same-origin"),
            ("x-cowok-daemon-nonce", lock.daemon_nonce.as_str()),
        ],
        editor_body,
    )
    .await;
    assert_eq!(nonce_ed_st, StatusCode::OK);
    assert_eq!(nonce_ed_json["role"], "editor");

    // Local-session viewer token works for GET; viewer role blocked from POST (403 INSUFFICIENT_ROLE)
    let viewer_body = serde_json::to_vec(&serde_json::json!({ "role": "viewer" })).unwrap();
    let (s_st, s_json) = send_req(
        &router,
        "POST",
        "/api/v1/auth/local-session",
        &[
            ("host", bound_host.as_str()),
            ("sec-fetch-site", "same-origin"),
        ],
        viewer_body,
    )
    .await;
    assert_eq!(s_st, StatusCode::OK);
    let sess_tok = s_json["sessionToken"].as_str().expect("token").to_string();

    let (vg_st, _) = send_req(
        &router,
        "GET",
        &snap_uri,
        &[
            ("host", bound_host.as_str()),
            ("x-cowok-session-token", &sess_tok),
        ],
        vec![],
    )
    .await;
    assert_eq!(vg_st, StatusCode::OK);

    let create_body = serde_json::to_vec(&serde_json::json!({
        "screenKey": "home",
        "title": "Home"
    }))
    .unwrap();
    let (vp_st, vp_json) = send_req(
        &router,
        "POST",
        &nodes_uri,
        &[
            ("host", bound_host.as_str()),
            ("x-cowok-session-token", &sess_tok),
        ],
        create_body.clone(),
    )
    .await;
    assert_eq!(vp_st, StatusCode::FORBIDDEN);
    assert_eq!(vp_json["code"], "INSUFFICIENT_ROLE");

    // SEC-DAEMON-05: Canonical 8-field HMAC-SHA256 v2 succeeds once and rejects replay nonce!
    let key_bytes: Vec<u8> = (0..lock.hmac_key_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&lock.hmac_key_hex[i..i + 2], 16).unwrap())
        .collect();
    let ts = Utc::now().timestamp().to_string();
    let req_nonce = Uuid::new_v4().to_string();
    let principal_b64 = B64_STD.encode(
        serde_json::to_vec(&FlowPrincipal {
            tenant_id: "local-default".into(),
            user_id: "bff-proxy".into(),
            role: "editor".into(),
        })
        .unwrap(),
    );
    let body_sha = format!("{:x}", Sha256::digest(&create_body));
    let canonical = format!(
        "{}\n{}\n{}\nPOST\n{}\n\n{}\n{}",
        lock.daemon_nonce, ts, req_nonce, nodes_uri, principal_b64, body_sha
    );
    let sig_hex: String = hmac_sha256(&key_bytes, canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let hmac_headers = [
        ("host", bound_host.as_str()),
        ("x-cowok-hmac-sig", sig_hex.as_str()),
        ("x-cowok-timestamp", ts.as_str()),
        ("x-cowok-nonce", req_nonce.as_str()),
        ("x-cowok-principal", principal_b64.as_str()),
    ];
    let (ok_st, ok_json) = send_req(
        &router,
        "POST",
        &nodes_uri,
        &hmac_headers,
        create_body.clone(),
    )
    .await;
    assert_eq!(ok_st, StatusCode::CREATED);
    assert_eq!(ok_json["screenKey"], "home");

    let (rep_st, rep_json) =
        send_req(&router, "POST", &nodes_uri, &hmac_headers, create_body).await;
    assert_eq!(rep_st, StatusCode::UNAUTHORIZED);
    assert_eq!(rep_json["code"], "UNAUTHENTICATED");
}

#[tokio::test]
async fn sec_legacy_ingress_fail_closed_and_verified_hmac_v2() {
    let tmp = TempDir::new().expect("tmp");
    let canon_root = tmp.path().canonicalize().expect("canon");
    let db_path = canon_root.join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &canon_root)).expect("wal");
    let project_id = Uuid::new_v4().to_string();
    pool.connection()
        .expect("conn")
        .execute(
            "INSERT INTO projects (id, tenant_id, slug, name, product_brief, workspace_root, created_at, updated_at) \
             VALUES (?1, 'local-default', 'legacy-sec', 'Legacy Sec', 'Brief', '.', 1700000000000, 1700000000000)",
            params![project_id],
        )
        .expect("seed");

    let registry = Arc::new(SpecRegistry::new(pool));
    let cfg = DaemonConfig {
        host: "127.0.0.1".into(),
        preferred_port: 0,
        workspace_root: canon_root.clone(),
    };
    let server = DaemonServer::bind_loopback(&cfg, registry).expect("bind");
    let router = server.router();
    let lock = read_daemon_lock(&canon_root).expect("lock");
    let bound_host = format!("127.0.0.1:{}", lock.port);

    let specs_uri = format!("/api/v1/projects/{project_id}/specs");
    let spec_put_uri = format!("/api/v1/projects/{project_id}/specs/product-brief");
    let promote_uri = format!("/api/v1/projects/{project_id}/promote");
    let path_token_uri = format!("/api/v1/projects/{project_id}/path-token");
    let patch_uri = format!("/api/v1/projects/{project_id}/surgical-patch");

    // 1. All protected legacy ingress routes MUST fail closed (401/403) without auth
    for (method, uri, body) in [
        ("GET", specs_uri.as_str(), vec![]),
        ("PUT", spec_put_uri.as_str(), b"{}".to_vec()),
        ("POST", promote_uri.as_str(), b"{}".to_vec()),
        ("POST", path_token_uri.as_str(), b"{}".to_vec()),
        ("POST", patch_uri.as_str(), b"{}".to_vec()),
    ] {
        let (st, _) = send_req(
            &router,
            method,
            uri,
            &[("host", bound_host.as_str())],
            body.clone(),
        )
        .await;
        assert!(
            st == StatusCode::UNAUTHORIZED || st == StatusCode::FORBIDDEN,
            "Expected fail-closed 401/403 on unauthenticated {method} {uri}, got {st}"
        );

        // Spoofed header / bogus HMAC must also fail closed
        let (spoof_st, _) = send_req(
            &router,
            method,
            uri,
            &[
                ("host", bound_host.as_str()),
                ("x-cowok-actor-id", "attacker"),
                ("x-cowok-hmac-sig", "0000"),
                ("x-cowok-timestamp", "1700000000"),
                ("x-cowok-nonce", "bad-nonce"),
            ],
            body,
        )
        .await;
        assert!(
            spoof_st == StatusCode::UNAUTHORIZED || spoof_st == StatusCode::FORBIDDEN,
            "Expected fail-closed 401/403 on spoofed {method} {uri}, got {spoof_st}"
        );
    }

    // 2. Valid Canonical 8-field HMAC-SHA256 v2 succeeds on legacy ingress
    let key_bytes: Vec<u8> = (0..lock.hmac_key_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&lock.hmac_key_hex[i..i + 2], 16).unwrap())
        .collect();
    let ts = Utc::now().timestamp().to_string();
    let req_nonce = Uuid::new_v4().to_string();
    let principal_b64 = B64_STD.encode(
        serde_json::to_vec(&FlowPrincipal {
            tenant_id: "local-default".into(),
            user_id: "bff-proxy".into(),
            role: "editor".into(),
        })
        .unwrap(),
    );
    let body_sha = format!("{:x}", Sha256::digest(b""));
    let canonical = format!(
        "{}\n{}\n{}\nGET\n{}\n\n{}\n{}",
        lock.daemon_nonce, ts, req_nonce, specs_uri, principal_b64, body_sha
    );
    let sig_hex: String = hmac_sha256(&key_bytes, canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let (ok_st, ok_json) = send_req(
        &router,
        "GET",
        &specs_uri,
        &[
            ("host", bound_host.as_str()),
            ("x-cowok-hmac-sig", sig_hex.as_str()),
            ("x-cowok-timestamp", ts.as_str()),
            ("x-cowok-nonce", req_nonce.as_str()),
            ("x-cowok-principal", principal_b64.as_str()),
        ],
        vec![],
    )
    .await;
    assert_eq!(ok_st, StatusCode::OK);
    assert_eq!(ok_json["project_id"], project_id);
}
