// @story 14.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Integration & Security Tests for Cloud Control Plane Ingestion, OIDC+MFA+4-Role RBAC, 2-Way Helpdesk, Feature Clusters, Funnel & Crash Center)
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use harness_daemon::routes::cloud_admin::cloud_admin_router_with_limiter;
use harness_daemon::routes::cloud_admin_auth::{
    issue_admin_session_token, scrub_server_pii, sign_oidc_assertion,
    validate_and_reencode_attachment, AdminSessionClaims, CloudAdminRole, IngestRateLimiter,
    OidcAssertionClaims, DEFAULT_DEV_MFA_CODE,
};
use harness_daemon::routes::DaemonAppState;
use harness_daemon::support_edge::{create_support_ticket, CreateSupportTicketInput};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct CloudTestHarness {
    _dir: tempfile::TempDir,
    pool: SqlitePool,
    daemon_nonce: String,
    app: Router,
}

fn setup_cloud_harness(rate_limit_max: usize) -> CloudTestHarness {
    setup_cloud_harness_with_limiter(Arc::new(IngestRateLimiter::new(rate_limit_max, 60)))
}

fn setup_cloud_harness_with_limiter(limiter: Arc<IngestRateLimiter>) -> CloudTestHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-142.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    let state = DaemonAppState::new(registry, "127.0.0.1".to_string(), 4172);
    let daemon_nonce = state.auth.daemon_nonce.clone();
    let app = cloud_admin_router_with_limiter(limiter).with_state(state);
    CloudTestHarness {
        _dir: dir,
        pool,
        daemon_nonce,
        app,
    }
}

async fn send_raw(
    app: &Router,
    method: Method,
    uri: &str,
    admin_token: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, axum::http::HeaderMap, axum::body::Bytes) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4172");
    if let Some(tok) = admin_token {
        builder = builder.header("x-admin-session", tok);
    }
    for (k, v) in extra_headers {
        builder = builder.header(*k, *v);
    }
    let req = if let Some(payload) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("build json req")
    } else {
        builder.body(Body::empty()).expect("build empty req")
    };
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    (status, headers, bytes)
}

async fn send_json(
    app: &Router,
    method: Method,
    uri: &str,
    admin_token: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, _headers, bytes) =
        send_raw(app, method, uri, admin_token, extra_headers, body).await;
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

async fn issue_role_session(
    h: &CloudTestHarness,
    role: &str,
    subject: &str,
    email: &str,
) -> String {
    let now = Utc::now().timestamp();
    let parsed_role = CloudAdminRole::parse_strict(role).expect("valid role");
    let assertion = sign_oidc_assertion(
        &h.daemon_nonce,
        &OidcAssertionClaims {
            oidc_provider: "google".to_string(),
            subject: subject.to_string(),
            email: email.to_string(),
            role: parsed_role,
            issued_at: now,
            expires_at: now + 300,
        },
    )
    .expect("sign oidc assertion");
    let (status, body) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/auth/session",
        None,
        &[],
        Some(json!({
            "oidcProvider": "google",
            "subject": subject,
            "email": email,
            "role": role,
            "mfaVerified": true,
            "mfaCode": DEFAULT_DEV_MFA_CODE,
            "oidcAssertion": assertion
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "session issue failed: {body}");
    body["data"]["sessionToken"]
        .as_str()
        .expect("sessionToken")
        .to_string()
}

fn minimal_valid_png_bytes() -> Vec<u8> {
    B64_STD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=")
        .expect("valid png base64")
}

fn png_with_text_chunk_and_trailing_bytes() -> Vec<u8> {
    let base = minimal_valid_png_bytes();
    // Insert a tEXt chunk right before the final 12-byte IEND chunk
    let iend_start = base.len() - 12;
    let mut out = Vec::new();
    out.extend_from_slice(&base[..iend_start]);
    let text_data = b"Comment\0SecretEXIFComment";
    out.extend_from_slice(&(text_data.len() as u32).to_be_bytes());
    out.extend_from_slice(b"tEXt");
    out.extend_from_slice(text_data);
    out.extend_from_slice(&[0u8; 4]); // placeholder CRC
    out.extend_from_slice(&base[iend_start..]);
    out.extend_from_slice(b"TRAILING_JUNK_BYTES");
    out
}

#[tokio::test]
async fn it_14_2_ac01_ingest_ticket_server_pii_scrub_and_idempotency() {
    let h = setup_cloud_harness(50);

    // Construct synthetic secret fixtures dynamically so static scanners (Gitleaks) never flag test literals.
    let synth_jwt = [
        "eyJ",
        "hbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.",
        "eyJzdWIiOiIxMjM0NTY3ODkwIn0.",
        "abc123456789",
    ]
    .concat();
    let synth_sk = ["sk-", "live-", "secret1234567890"].concat();
    let synth_ghp = ["ghp", "_abcdef1234567890"].concat();
    let synth_aiza = ["AIza", "SyDummyApiKey1234567890"].concat();
    let synth_proj_sk = ["sk-", "proj-", "supersecret99887766"].concat();
    let synth_ghp2 = ["ghp", "_token1234567890"].concat();
    let synth_aiza2 = ["AIza", "SySecretKey123456789012"].concat();

    let raw_sample = format!(
        "Bearer {synth_jwt} {synth_sk} {synth_ghp} {synth_aiza} user@corp.example /Users/alice/project/.env"
    );
    let (scrubbed, count) = scrub_server_pii(&raw_sample);
    assert!(count >= 5, "expected >=5 redactions, got {count}");
    assert!(!scrubbed.contains(&synth_sk));
    assert!(!scrubbed.contains(&synth_ghp));
    assert!(!scrubbed.contains(&synth_aiza));
    assert!(!scrubbed.contains("user@corp.example"));
    assert!(!scrubbed.contains("/Users/alice"));

    let ticket_payload = json!({
        "installId": "inst_clean_001",
        "category": "bug",
        "priority": "P1",
        "title": "Export crashed for alice@corp.io",
        "description": format!("Failed at /Users/alice/work with key {synth_proj_sk} and {synth_ghp2}"),
        "diagnosticBundle": {
            "os": "macos",
            "apiKey": synth_aiza2,
            "path": "/home/bob/secrets"
        },
        "channel": "desktop",
        "hostClient": "standalone",
        "appVersion": "1.2.0",
        "idempotencyKey": "idem-story-14-2-001"
    });

    let (status1, body1) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[("x-client-ip", "10.0.0.1")],
        Some(ticket_payload.clone()),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED, "body1={body1}");
    assert_eq!(body1["data"]["deduplicated"], false);
    assert!(body1["data"]["serverRedactionCount"].as_u64().unwrap_or(0) >= 4);
    let ticket_id = body1["data"]["ticket"]["id"].as_str().unwrap().to_string();
    let stored_desc = body1["data"]["ticket"]["descriptionSanitized"]
        .as_str()
        .unwrap();
    assert!(!stored_desc.contains("sk-proj-supersecret"));
    assert!(!stored_desc.contains("ghp_token1234567890"));
    assert!(!stored_desc.contains("/Users/alice"));
    assert_eq!(
        body1["data"]["ticket"]["diagnosticBundle"]["apiKey"],
        "[REDACTED_SECRET]"
    );

    // Replay same idempotencyKey -> deduplicated 200 OK with same ticket id
    let (status2, body2) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[("x-client-ip", "10.0.0.1")],
        Some(ticket_payload),
    )
    .await;
    assert_eq!(status2, StatusCode::OK, "body2={body2}");
    assert_eq!(body2["data"]["deduplicated"], true);
    assert_eq!(body2["data"]["ticket"]["id"], ticket_id);
}

#[tokio::test]
async fn it_14_2_ac01_ac05_attachment_reencode_and_polyglot_rejection() {
    let h = setup_cloud_harness(50);

    // 1. Valid PNG with tEXt metadata & trailing bytes -> stripped cleanly
    let dirty_png = png_with_text_chunk_and_trailing_bytes();
    let dirty_url = format!("data:image/png;base64,{}", B64_STD.encode(&dirty_png));
    let sanitized = validate_and_reencode_attachment(&dirty_url).expect("sanitized png");
    assert_eq!(sanitized.mime, "image/png");
    assert!(
        sanitized.byte_len < dirty_png.len(),
        "metadata + trailing bytes must be stripped"
    );
    assert!(!sanitized.bytes.windows(4).any(|w| w == b"tEXt"));

    // 2. Polyglot PNG containing <script> -> rejected 422 INVALID_ATTACHMENT_PAYLOAD
    let mut polyglot = minimal_valid_png_bytes();
    polyglot.extend_from_slice(b"<script>alert(1)</script>");
    let polyglot_url = format!("data:image/png;base64,{}", B64_STD.encode(&polyglot));
    let (status, body) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_polyglot_1",
            "category": "bug",
            "title": "Polyglot upload test",
            "description": "Attempting to upload malicious polyglot image",
            "attachmentDataUrl": polyglot_url
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body={body}");
    assert_eq!(body["error"]["code"], "INVALID_ATTACHMENT_PAYLOAD");

    // 3. Fake SVG disguised as PNG -> rejected 422 INVALID_ATTACHMENT_PAYLOAD
    let fake_svg_url = format!(
        "data:image/png;base64,{}",
        B64_STD.encode(b"<svg xmlns='http://www.w3.org/2000/svg' onload='alert(1)'/>")
    );
    let (status_svg, body_svg) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_polyglot_2",
            "category": "bug",
            "title": "Fake SVG upload test",
            "description": "Disguised SVG payload must fail magic check",
            "attachmentDataUrl": fake_svg_url
        })),
    )
    .await;
    assert_eq!(
        status_svg,
        StatusCode::UNPROCESSABLE_ENTITY,
        "body={body_svg}"
    );
    assert_eq!(body_svg["error"]["code"], "INVALID_ATTACHMENT_PAYLOAD");
}

#[tokio::test]
async fn it_14_2_ac05_rate_limit_flood_isolation() {
    let h = setup_cloud_harness(2);
    for i in 0..2 {
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/telemetry",
            None,
            &[("x-client-ip", "198.51.100.10")],
            Some(json!({
                "installId": "inst_flooder",
                "eventName": format!("ping_{i}"),
                "channel": "cli"
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }

    // 3rd request from same (install_id, ip) -> 429 RATE_LIMIT_EXCEEDED
    let (st_429, body_429) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[("x-client-ip", "198.51.100.10")],
        Some(json!({
            "installId": "inst_flooder",
            "eventName": "ping_overflow",
            "channel": "cli"
        })),
    )
    .await;
    assert_eq!(st_429, StatusCode::TOO_MANY_REQUESTS, "body={body_429}");
    assert_eq!(body_429["error"]["code"], "RATE_LIMIT_EXCEEDED");

    // Legitimate other install_id on same/different IP is completely unaffected -> 201
    let (st_ok, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[("x-client-ip", "198.51.100.10")],
        Some(json!({
            "installId": "inst_legit_user",
            "eventName": "day_0",
            "channel": "desktop"
        })),
    )
    .await;
    assert_eq!(st_ok, StatusCode::CREATED);
}

#[tokio::test]
async fn it_14_2_ac02_oidc_mfa_4role_rbac_and_audit_immutability() {
    let h = setup_cloud_harness(50);

    // 1. Missing token -> 401 ADMIN_AUTH_REQUIRED
    let (st_missing, body_missing) =
        send_json(&h.app, Method::GET, "/v1/admin/tickets", None, &[], None).await;
    assert_eq!(st_missing, StatusCode::UNAUTHORIZED);
    assert_eq!(body_missing["error"]["code"], "ADMIN_AUTH_REQUIRED");

    // 2. Session request with valid OIDC assertion but mfaVerified = false -> 401 MFA_REQUIRED
    let nomfa_assertion = sign_oidc_assertion(
        &h.daemon_nonce,
        &OidcAssertionClaims {
            oidc_provider: "okta".into(),
            subject: "sub_nomfa".into(),
            email: "ops-admin@cowok.design".into(),
            role: CloudAdminRole::SuperAdmin,
            issued_at: Utc::now().timestamp(),
            expires_at: Utc::now().timestamp() + 300,
        },
    )
    .expect("assertion");
    let (st_mfa, body_mfa) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/auth/session",
        None,
        &[],
        Some(json!({
            "oidcProvider": "okta",
            "subject": "sub_nomfa",
            "email": "ops-admin@cowok.design",
            "role": "SUPER_ADMIN",
            "mfaVerified": false,
            "oidcAssertion": nomfa_assertion
        })),
    )
    .await;
    assert_eq!(st_mfa, StatusCode::UNAUTHORIZED);
    assert_eq!(body_mfa["error"]["code"], "MFA_REQUIRED");

    // 3. Forged/unsigned MFA=false token directly -> 401 MFA_REQUIRED
    let no_mfa_tok = issue_admin_session_token(&AdminSessionClaims {
        subject: "sub_nomfa".into(),
        email: "nomfa@cowok.design".into(),
        role: CloudAdminRole::SuperAdmin,
        mfa_verified: false,
        oidc_provider: "okta".into(),
        issued_at: Utc::now().timestamp(),
        expires_at: Utc::now().timestamp() + 3600,
        jti: "jti_nomfa".into(),
    })
    .expect("token");
    let (st_tok_mfa, body_tok_mfa) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/tickets",
        Some(&no_mfa_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_tok_mfa, StatusCode::UNAUTHORIZED);
    assert_eq!(body_tok_mfa["error"]["code"], "MFA_REQUIRED");

    // Seed a ticket first
    let (_, created) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_rbac_1",
            "category": "feature_request",
            "title": "Need dark mode print export",
            "description": "Please add dark mode PDF export button"
        })),
    )
    .await;
    let ticket_id = created["data"]["ticket"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let analyst_tok =
        issue_role_session(&h, "ANALYST_READONLY", "sub_ana", "ana@cowok.design").await;
    let curator_tok = issue_role_session(&h, "SKILL_CURATOR", "sub_cur", "cur@cowok.design").await;
    let agent_tok = issue_role_session(&h, "SUPPORT_AGENT", "sub_agt", "agt@cowok.design").await;
    let super_tok = issue_role_session(&h, "SUPER_ADMIN", "sub_sup", "sup@cowok.design").await;

    // ANALYST_READONLY: can read tickets & funnel, FORBIDDEN from reply, cluster, audit-logs
    let (st, _) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/tickets",
        Some(&analyst_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st_reply_ana, body_reply_ana) = send_json(
        &h.app,
        Method::POST,
        &format!("/v1/admin/tickets/{ticket_id}/reply"),
        Some(&analyst_tok),
        &[],
        Some(json!({ "body": "Analyst trying to reply" })),
    )
    .await;
    assert_eq!(st_reply_ana, StatusCode::FORBIDDEN);
    assert_eq!(body_reply_ana["error"]["code"], "RBAC_FORBIDDEN");

    let (st_clu_ana, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/clusters",
        Some(&analyst_tok),
        &[],
        Some(json!({ "title": "Unauthorized cluster", "ticketIds": [ticket_id] })),
    )
    .await;
    assert_eq!(st_clu_ana, StatusCode::FORBIDDEN);

    // SKILL_CURATOR: FORBIDDEN from ticket reply, ALLOWED to create clusters
    let (st_reply_cur, _) = send_json(
        &h.app,
        Method::POST,
        &format!("/v1/admin/tickets/{ticket_id}/reply"),
        Some(&curator_tok),
        &[],
        Some(json!({ "body": "Curator trying to reply" })),
    )
    .await;
    assert_eq!(st_reply_cur, StatusCode::FORBIDDEN);

    let (st_clu_cur, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/clusters",
        Some(&curator_tok),
        &[],
        Some(json!({ "title": "Dark Mode Export", "ticketIds": [ticket_id] })),
    )
    .await;
    assert_eq!(st_clu_cur, StatusCode::CREATED);

    // SUPPORT_AGENT: ALLOWED to reply, FORBIDDEN from /v1/admin/audit-logs
    let (st_aud_agt, _) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/audit-logs",
        Some(&agent_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_aud_agt, StatusCode::FORBIDDEN);

    // SUPER_ADMIN: ALLOWED on /v1/admin/audit-logs and verifies immutable audit records exist
    let (st_aud_sup, body_aud) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/audit-logs",
        Some(&super_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_aud_sup, StatusCode::OK, "audit={body_aud}");
    let logs = body_aud["data"]["logs"].as_array().expect("logs array");
    assert!(logs.len() >= 5, "expected session + cluster audit logs");
    assert!(logs.iter().all(|l| l["mfaVerified"] == true));
}

#[tokio::test]
async fn it_14_2_ac03_two_way_helpdesk_reply_and_feature_clustering() {
    let h = setup_cloud_harness(50);

    // Create Story 14.1 local support ticket to verify true 2-way sync propagation
    let local_ticket = create_support_ticket(
        &h.pool,
        &CreateSupportTicketInput {
            category: "feature_request".to_string(),
            title: "One-click Figma export".to_string(),
            description: "Need direct export to Figma frames".to_string(),
            contact_email: None,
            screenshot_data_url: None,
            channel: "desktop".to_string(),
            locale: "en-US".to_string(),
            include_diagnostics: true,
            recent_error_codes: vec![],
            active_skills: vec![],
        },
    )
    .expect("create local ticket");

    let (st_t1, t1) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": local_ticket.install_id,
            "clientTicketId": local_ticket.id,
            "category": "feature_request",
            "title": "One-click Figma export",
            "description": "Need direct export to Figma frames",
            "channel": "desktop",
            "hostClient": "standalone"
        })),
    )
    .await;
    assert_eq!(st_t1, StatusCode::CREATED);
    let t1_id = t1["data"]["ticket"]["id"].as_str().unwrap().to_string();

    let (st_t2, t2) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_other_2",
            "category": "feature_request",
            "title": "Figma frame export button",
            "description": "Would love 1-click Figma export",
            "channel": "plugin",
            "hostClient": "cursor"
        })),
    )
    .await;
    assert_eq!(st_t2, StatusCode::CREATED);
    let t2_id = t2["data"]["ticket"]["id"].as_str().unwrap().to_string();

    let agent_tok = issue_role_session(&h, "SUPPORT_AGENT", "sub_agent", "desk@cowok.design").await;

    // Reply & upgrade priority to P0
    let (st_rep, rep_body) = send_json(
        &h.app,
        Method::POST,
        &format!("/v1/admin/tickets/{t1_id}/reply"),
        Some(&agent_tok),
        &[],
        Some(json!({
            "body": "Scheduled for Sprint 15! We will notify you inside Cowok App.",
            "priority": "P0",
            "status": "replied"
        })),
    )
    .await;
    assert_eq!(st_rep, StatusCode::CREATED, "reply={rep_body}");
    assert_eq!(rep_body["data"]["ticket"]["priority"], "P0");
    assert_eq!(rep_body["data"]["ticket"]["status"], "replied");
    assert_eq!(rep_body["data"]["reply"]["syncedToClient"], true);

    // Verify Story 14.1 local_support_tickets received the 2-way synced reply!
    let local_replies =
        harness_daemon::support_edge::list_support_tickets(&h.pool, 10).expect("list local");
    let matched_local = local_replies
        .iter()
        .find(|t| t.id == local_ticket.id)
        .expect("matched local ticket");
    assert_eq!(matched_local.admin_replies.len(), 1);
    assert!(matched_local.admin_replies[0]
        .message
        .contains("Scheduled for Sprint 15"));
    assert_eq!(matched_local.sync_status, "synced");

    // Cluster both feature requests & generate 1-click GitHub URL
    let (st_clu, clu_body) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/clusters",
        Some(&agent_tok),
        &[],
        Some(json!({
            "title": "One-Click Figma Frame Export",
            "summary": "Multiple non-tech users requested direct Figma frame export",
            "ticketIds": [t1_id, t2_id],
            "status": "planned"
        })),
    )
    .await;
    assert_eq!(st_clu, StatusCode::CREATED, "cluster={clu_body}");
    assert_eq!(clu_body["data"]["cluster"]["ticketCount"], 2);
    let gh_url = clu_body["data"]["cluster"]["githubIssueUrl"]
        .as_str()
        .expect("github url");
    assert!(gh_url.starts_with("https://github.com/cowok-design/cowok-design/issues/new?"));
}

#[tokio::test]
async fn it_14_2_ac04_activation_funnel_and_crash_grouping() {
    let h = setup_cloud_harness(100);
    let now_ms = Utc::now().timestamp_millis();

    let events = vec![
        ("u1", "app_installed", Some("day_0"), "cli", "standalone"),
        (
            "u1",
            "ai_connected",
            Some("ai_connected"),
            "cli",
            "standalone",
        ),
        (
            "u1",
            "project_created",
            Some("project"),
            "cli",
            "standalone",
        ),
        ("u1", "preview_opened", Some("preview"), "cli", "standalone"),
        (
            "u1",
            "export_completed",
            Some("export"),
            "cli",
            "standalone",
        ),
        (
            "u2",
            "app_installed",
            Some("day_0"),
            "desktop",
            "standalone",
        ),
        (
            "u2",
            "ai_connected",
            Some("ai_connected"),
            "desktop",
            "standalone",
        ),
        (
            "u2",
            "project_created",
            Some("project"),
            "desktop",
            "standalone",
        ),
        ("u3", "app_installed", Some("day_0"), "plugin", "cursor"),
        (
            "u3",
            "ai_connected",
            Some("ai_connected"),
            "plugin",
            "cursor",
        ),
        (
            "u4",
            "app_installed",
            Some("day_0"),
            "docker",
            "claude_code",
        ),
    ];

    for (install, event, stage, channel, host) in events {
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/telemetry",
            None,
            &[],
            Some(json!({
                "installId": install,
                "eventName": event,
                "funnelStage": stage,
                "channel": channel,
                "hostClient": host,
                "appVersion": "1.3.0",
                "occurredAt": now_ms
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }

    // Ingest 2 crash events sharing the same fingerprint from 2 installs (with PII inside stack)
    for install in ["u2", "u3"] {
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/telemetry",
            None,
            &[],
            Some(json!({
                "installId": install,
                "eventName": "app_crash",
                "channel": "desktop",
                "hostClient": "standalone",
                "appVersion": "1.3.0",
                "properties": {
                    "fingerprint": "fp_canvas_oom_01",
                    "errorSignature": "CanvasRenderer::OutOfMemory",
                    "sampleStack": "at /Users/dev/cowok/canvas.rs:42 with sk-live-secret9999999"
                },
                "occurredAt": now_ms
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }

    let analyst_tok =
        issue_role_session(&h, "ANALYST_READONLY", "sub_ana2", "ana2@cowok.design").await;

    let (st_fun, funnel) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/analytics/funnel",
        Some(&analyst_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_fun, StatusCode::OK, "funnel={funnel}");
    assert_eq!(funnel["data"]["dau"], 4);
    assert_eq!(funnel["data"]["wau"], 4);
    assert_eq!(funnel["data"]["mau"], 4);

    let steps = funnel["data"]["funnelSteps"]
        .as_array()
        .expect("funnelSteps");
    assert_eq!(steps.len(), 5);
    assert_eq!(steps[0]["stage"], "day_0");
    assert_eq!(steps[0]["users"], 4);
    assert_eq!(steps[1]["stage"], "ai_connected");
    assert_eq!(steps[1]["users"], 3);
    assert_eq!(steps[1]["dropOffCount"], 1);
    assert_eq!(steps[4]["stage"], "export");
    assert_eq!(steps[4]["users"], 1);

    let channels = funnel["data"]["channelBreakdown"]
        .as_array()
        .expect("channels");
    assert_eq!(channels.len(), 4);
    let hosts = funnel["data"]["hostBreakdown"].as_array().expect("hosts");
    assert_eq!(hosts.len(), 5);

    let (st_cr, crashes) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/crashes?appVersion=1.3.0",
        Some(&analyst_tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_cr, StatusCode::OK, "crashes={crashes}");
    let crash_list = crashes["data"]["crashes"]
        .as_array()
        .expect("crashes array");
    assert_eq!(crash_list.len(), 1);
    assert_eq!(crash_list[0]["fingerprint"], "fp_canvas_oom_01");
    assert_eq!(crash_list[0]["occurrenceCount"], 2);
    assert_eq!(crash_list[0]["affectedInstalls"], 2);
    let stack = crash_list[0]["sampleStackSanitized"].as_str().unwrap();
    assert!(!stack.contains("/Users/dev"));
    assert!(!stack.contains("sk-live-secret"));
}

#[tokio::test]
async fn it_14_2_stage3_d01_oidc_auth_privilege_escalation_and_mfa_enforced() {
    let h = setup_cloud_harness(50);

    // 1. Unauthenticated caller trying to self-mint SUPER_ADMIN without OIDC assertion/secret/BFF -> 401
    let (st_unauth, body_unauth) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/auth/session",
        None,
        &[],
        Some(json!({
            "oidcProvider": "google",
            "subject": "sub_sup",
            "email": "sup@cowok.design",
            "role": "SUPER_ADMIN",
            "mfaVerified": true,
            "mfaCode": DEFAULT_DEV_MFA_CODE
        })),
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED, "body={body_unauth}");
    assert_eq!(body_unauth["error"]["code"], "ADMIN_AUTH_REQUIRED");

    let shared_secret =
        harness_daemon::routes::cloud_admin_auth::resolve_oidc_shared_secret(&h.daemon_nonce);

    // 2. ANALYST_READONLY attempting to self-promote to SUPER_ADMIN even with valid OIDC secret -> 403
    let (st_esc, body_esc) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/auth/session",
        None,
        &[("x-cloud-admin-oidc-secret", &shared_secret)],
        Some(json!({
            "oidcProvider": "google",
            "subject": "sub_ana",
            "email": "ana@cowok.design",
            "role": "SUPER_ADMIN",
            "mfaVerified": true,
            "mfaCode": DEFAULT_DEV_MFA_CODE
        })),
    )
    .await;
    assert_eq!(st_esc, StatusCode::FORBIDDEN, "body={body_esc}");
    assert_eq!(body_esc["error"]["code"], "RBAC_FORBIDDEN");

    // 3. Unknown operator email attempting to mint SUPER_ADMIN -> 403
    let (st_unk, body_unk) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/auth/session",
        None,
        &[("x-cloud-admin-oidc-secret", &shared_secret)],
        Some(json!({
            "oidcProvider": "google",
            "subject": "sub_evil",
            "email": "attacker@evil.example",
            "role": "SUPER_ADMIN",
            "mfaVerified": true,
            "mfaCode": DEFAULT_DEV_MFA_CODE
        })),
    )
    .await;
    assert_eq!(st_unk, StatusCode::FORBIDDEN, "body={body_unk}");
    assert_eq!(body_unk["error"]["code"], "RBAC_FORBIDDEN");

    // 4. Missing or invalid mfaCode (even with mfaVerified: true) -> 401 MFA_REQUIRED
    for bad_code in [
        None,
        Some(""),
        Some("000000"),
        Some("654321"),
        Some("12ab56"),
    ] {
        let mut payload = json!({
            "oidcProvider": "google",
            "subject": "sub_sup",
            "email": "sup@cowok.design",
            "role": "SUPER_ADMIN",
            "mfaVerified": true
        });
        if let Some(code) = bad_code {
            payload["mfaCode"] = json!(code);
        }
        let (st_bad_mfa, body_bad_mfa) = send_json(
            &h.app,
            Method::POST,
            "/v1/admin/auth/session",
            None,
            &[("x-cloud-admin-oidc-secret", &shared_secret)],
            Some(payload),
        )
        .await;
        assert_eq!(
            st_bad_mfa,
            StatusCode::UNAUTHORIZED,
            "bad_code={bad_code:?}, body={body_bad_mfa}"
        );
        assert_eq!(body_bad_mfa["error"]["code"], "MFA_REQUIRED");
    }

    // 5. Active ANALYST_READONLY session attempting upward escalation via X-Admin-Session -> 403
    let analyst_tok =
        issue_role_session(&h, "ANALYST_READONLY", "sub_ana", "ana@cowok.design").await;
    let (st_up, body_up) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/auth/session",
        Some(&analyst_tok),
        &[("x-cloud-admin-oidc-secret", &shared_secret)],
        Some(json!({
            "oidcProvider": "google",
            "subject": "sub_sup",
            "email": "sup@cowok.design",
            "role": "SUPER_ADMIN",
            "mfaVerified": true,
            "mfaCode": DEFAULT_DEV_MFA_CODE
        })),
    )
    .await;
    assert_eq!(st_up, StatusCode::FORBIDDEN, "body={body_up}");
    assert_eq!(body_up["error"]["code"], "RBAC_FORBIDDEN");
}

#[tokio::test]
async fn it_14_2_stage3_d02_dual_rate_limiter_ip_spoof_and_bucket_cap() {
    // 1. Separate per-install and per-IP counters + capacity ceiling unit check
    let limiter = IngestRateLimiter::new_with_limits(2, 3, 60, 2);
    assert!(limiter.check_and_record("inst_a", "10.0.0.1").is_ok());
    assert!(limiter.check_and_record("inst_a", "10.0.0.2").is_ok());
    // Same install rotating IP hits per-install limit (2) -> 429
    assert!(limiter.check_and_record("inst_a", "10.0.0.3").is_err());
    // New install on existing IP succeeds (by_install has 2 entries == max_buckets)
    assert!(limiter.check_and_record("inst_b", "10.0.0.1").is_ok());
    // 3rd distinct install hits hard capacity ceiling (max_buckets = 2) -> fails closed 429
    assert!(limiter.check_and_record("inst_c", "10.0.0.1").is_err());

    // 2. Untrusted direct caller rotating X-Client-Ip and installId still maps to "loopback" and hits per-IP cap
    let h = setup_cloud_harness_with_limiter(Arc::new(IngestRateLimiter::new_with_limits(
        5, 2, 60, 100,
    )));
    for i in 0..2 {
        let ip = format!("203.0.113.{}", i + 1);
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/telemetry",
            None,
            &[("x-client-ip", &ip)],
            Some(json!({
                "installId": format!("inst_spoof_{i}"),
                "eventName": "ping"
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }
    let (st_429, body_429) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[("x-client-ip", "203.0.113.99")],
        Some(json!({
            "installId": "inst_spoof_99",
            "eventName": "ping"
        })),
    )
    .await;
    assert_eq!(st_429, StatusCode::TOO_MANY_REQUESTS, "body={body_429}");
    assert_eq!(body_429["error"]["code"], "RATE_LIMIT_EXCEEDED");
}

#[tokio::test]
async fn it_14_2_stage3_d04_hardened_image_validation_persistence_and_rbac_serving() {
    let h = setup_cloud_harness(50);

    // 1. Expanded polyglot patterns rejected
    for poly in [
        b"<img src=x onerror=alert(1)>".as_slice(),
        b"<object data='evil'></object>".as_slice(),
        b"<?xml version='1.0'?><svg/>".as_slice(),
        b"<body onload=alert(1)>".as_slice(),
    ] {
        let mut bytes = minimal_valid_png_bytes();
        bytes.extend_from_slice(poly);
        let url = format!("data:image/png;base64,{}", B64_STD.encode(&bytes));
        assert!(
            validate_and_reencode_attachment(&url).is_err(),
            "polyglot must be rejected"
        );
    }

    // 2. PNG with corrupted IHDR CRC-32 rejected
    let mut bad_crc_png = minimal_valid_png_bytes();
    bad_crc_png[29] ^= 0xFF; // flip bit in IHDR CRC
    let bad_crc_url = format!("data:image/png;base64,{}", B64_STD.encode(&bad_crc_png));
    assert!(validate_and_reencode_attachment(&bad_crc_url).is_err());

    // 3. JPEG without SOF/SOS rejected; valid JPEG with APP0 + COM stripped cleanly
    let fake_jpeg_url = format!(
        "data:image/jpeg;base64,{}",
        B64_STD.encode([0xFF, 0xD8, 0xFF, 0xD9])
    );
    assert!(validate_and_reencode_attachment(&fake_jpeg_url).is_err());

    let mut valid_jpeg = vec![0xFF, 0xD8];
    // APP0 segment (must be stripped)
    valid_jpeg.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x06, b'J', b'F', b'I', b'F']);
    // COM segment (must be stripped)
    valid_jpeg.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x05, b'E', b'X', b'I']);
    // SOF0 1x1 frame
    valid_jpeg.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00,
    ]);
    // SOS + scan byte + EOI
    valid_jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
    valid_jpeg.extend_from_slice(&[0x12, 0x34, 0xFF, 0xD9]);
    let jpeg_url = format!("data:image/jpeg;base64,{}", B64_STD.encode(&valid_jpeg));
    let clean_jpeg = validate_and_reencode_attachment(&jpeg_url).expect("valid jpeg sanitized");
    assert!(!clean_jpeg.bytes.windows(2).any(|w| w == [0xFF, 0xE0]));
    assert!(!clean_jpeg.bytes.windows(2).any(|w| w == [0xFF, 0xFE]));

    // 4. WebP with VP8X EXIF/XMP/ICCP flags + EXIF chunk -> strips EXIF and clears VP8X bits
    let mut webp_body = Vec::new();
    webp_body.extend_from_slice(b"WEBP");
    webp_body.extend_from_slice(b"VP8X");
    webp_body.extend_from_slice(&10u32.to_le_bytes());
    webp_body.extend_from_slice(&[0x2C, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // ICCP|EXIF|XMP set
    webp_body.extend_from_slice(b"VP8 ");
    webp_body.extend_from_slice(&4u32.to_le_bytes());
    webp_body.extend_from_slice(&[0x9D, 0x01, 0x2A, 0x00]);
    webp_body.extend_from_slice(b"EXIF");
    webp_body.extend_from_slice(&4u32.to_le_bytes());
    webp_body.extend_from_slice(b"GPS!");
    let mut webp = Vec::new();
    webp.extend_from_slice(b"RIFF");
    webp.extend_from_slice(&(webp_body.len() as u32).to_le_bytes());
    webp.extend_from_slice(&webp_body);
    let webp_url = format!("data:image/webp;base64,{}", B64_STD.encode(&webp));
    let clean_webp = validate_and_reencode_attachment(&webp_url).expect("valid webp sanitized");
    assert!(!clean_webp.bytes.windows(4).any(|w| w == b"EXIF"));
    assert_eq!(clean_webp.bytes[20] & (0x20 | 0x08 | 0x04), 0);

    // 5. Ingest ticket with valid PNG -> persisted to disk and served via RBAC-guarded endpoint
    let png_url = format!(
        "data:image/png;base64,{}",
        B64_STD.encode(minimal_valid_png_bytes())
    );
    let (st_ing, ing_body) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_att_1",
            "category": "bug",
            "title": "Screenshot attachment check",
            "description": "Attaching valid screenshot for preview check",
            "attachmentDataUrl": png_url
        })),
    )
    .await;
    assert_eq!(st_ing, StatusCode::CREATED, "ing={ing_body}");
    let ticket_id = ing_body["data"]["ticket"]["id"].as_str().unwrap();

    // Unauthenticated attachment fetch -> 401
    let (st_noauth, _, _) = send_raw(
        &h.app,
        Method::GET,
        &format!("/v1/admin/tickets/{ticket_id}/attachment"),
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(st_noauth, StatusCode::UNAUTHORIZED);

    // Authenticated fetch -> 200 with image/png + nosniff + no-store
    let tok = issue_role_session(&h, "ANALYST_READONLY", "sub_ana", "ana@cowok.design").await;
    let (st_ok, headers, bytes) = send_raw(
        &h.app,
        Method::GET,
        &format!("/v1/admin/tickets/{ticket_id}/attachment"),
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "image/png");
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
}

#[tokio::test]
async fn it_14_2_stage3_d05_sql_where_before_limit_and_accurate_total_count() {
    let h = setup_cloud_harness(100);
    // Insert 3 P0 tickets and 2 P2 tickets
    for i in 0..3 {
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/tickets",
            None,
            &[],
            Some(json!({
                "installId": format!("inst_p0_{i}"),
                "category": "bug",
                "priority": "P0",
                "title": format!("Urgent P0 bug #{i}"),
                "description": "High priority ticket that sorts first"
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }
    for i in 0..2 {
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/tickets",
            None,
            &[],
            Some(json!({
                "installId": format!("inst_p2_{i}"),
                "category": "feature_request",
                "priority": "P2",
                "title": format!("NeedleFeature P2 #{i}"),
                "description": "Lower priority ticket matching filter"
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }

    let tok = issue_role_session(&h, "ANALYST_READONLY", "sub_ana", "ana@cowok.design").await;
    // With limit=1 and priority=P2, MUST return 1 P2 ticket and totalCount=2 (NOT empty!)
    let (st, body) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/tickets?priority=P2&search=needlefeature&limit=1",
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "body={body}");
    assert_eq!(body["data"]["tickets"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["tickets"][0]["priority"], "P2");
    assert_eq!(body["data"]["totalCount"], 2);

    // Crashes: 2 high-occurrence v1.0.0 crashes and 1 low-occurrence v2.0.0 crash
    for _ in 0..3 {
        let _ = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/telemetry",
            None,
            &[],
            Some(json!({
                "installId": "u_v1",
                "eventName": "app_crash",
                "appVersion": "1.0.0",
                "properties": { "fingerprint": "fp_v1_high", "errorSignature": "V1Err" }
            })),
        )
        .await;
    }
    let _ = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[],
        Some(json!({
            "installId": "u_v2",
            "eventName": "app_crash",
            "appVersion": "2.0.0",
            "properties": { "fingerprint": "fp_v2_low", "errorSignature": "V2Err" }
        })),
    )
    .await;

    let (st_cr, cr_body) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/crashes?appVersion=2.0.0&limit=1",
        Some(&tok),
        &[],
        None,
    )
    .await;
    assert_eq!(st_cr, StatusCode::OK);
    let crashes = cr_body["data"]["crashes"].as_array().unwrap();
    assert_eq!(crashes.len(), 1);
    assert_eq!(crashes[0]["fingerprint"], "fp_v2_low");
}

#[tokio::test]
async fn it_14_2_stage3_d06_d07_d08_d09_d10_d11_comprehensive_regressions() {
    let h = setup_cloud_harness(100);

    // D06: expanded server PII scrubber (case-insensitive paths, tab bearer, AWS key, Stripe keys, Cookie/Set-Cookie/header key)
    let synth_aws = ["AKIA", "IOSFODNN7EXAMPLE"].concat();
    let synth_stripe_live = ["sk_", "live_", "1234567890abcdef"].concat();
    let synth_stripe_test = ["sk_", "test_", "abcdef1234567890"].concat();
    let synth_header_key = ["x-api-", "key: ", "topsecret98765"].concat();
    let synth_ver_leak = ["1.0.0-", "sk_", "live_", "1234567890"].concat();
    let d06_input = format!(
        "Bearer\tabcdefgh12345678 {synth_aws} {synth_stripe_live} {synth_stripe_test} \
         /users/alice/doc /HOME/bob/key c:/users/eve/app \
         Cookie: sid=secret123; theme=dark\nSet-Cookie: auth=xyz999\n{synth_header_key}"
    );
    let (clean, redactions) = scrub_server_pii(&d06_input);
    assert!(
        redactions >= 9,
        "expected >=9 redactions, got {redactions}: {clean}"
    );
    for leaked in [
        "abcdefgh12345678",
        synth_aws.as_str(),
        &synth_stripe_live[..18],
        &synth_stripe_test[..18],
        "/users/alice",
        "/HOME/bob",
        "c:/users/eve",
        "sid=secret123",
        "auth=xyz999",
        "topsecret98765",
    ] {
        assert!(!clean.contains(leaked), "leaked '{leaked}' in '{clean}'");
    }

    // D06: PII inside channel/hostClient/appVersion/clientTicketId scrubbed before DB write
    let (st_t1, t1_body) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_d06",
            "clientTicketId": "tkt_alice@corp.io",
            "category": "feature_request",
            "title": "Clean title for D06",
            "description": "Clean description text",
            "channel": "cli_/users/dev/bin",
            "hostClient": synth_aws,
            "appVersion": synth_ver_leak
        })),
    )
    .await;
    assert_eq!(st_t1, StatusCode::CREATED);
    assert!(t1_body["data"]["serverRedactionCount"].as_u64().unwrap() >= 4);
    let t1_id = t1_body["data"]["ticket"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!t1_body["data"]["ticket"]["channel"]
        .as_str()
        .unwrap()
        .contains("/users/dev"));
    assert!(!t1_body["data"]["ticket"]["hostClient"]
        .as_str()
        .unwrap()
        .contains("AKIA"));
    assert!(!t1_body["data"]["ticket"]["appVersion"]
        .as_str()
        .unwrap()
        .contains("sk_live_"));

    // D11: Reply to cloud ticket with NO matching local_support_tickets row -> syncedToClient == false
    let super_tok = issue_role_session(&h, "SUPER_ADMIN", "sub_sup", "sup@cowok.design").await;
    let (st_rep, rep_body) = send_json(
        &h.app,
        Method::POST,
        &format!("/v1/admin/tickets/{t1_id}/reply"),
        Some(&super_tok),
        &[],
        Some(json!({ "body": "Cloud-only reply test" })),
    )
    .await;
    assert_eq!(st_rep, StatusCode::CREATED, "rep={rep_body}");
    assert_eq!(rep_body["data"]["reply"]["syncedToClient"], false);

    // D09: DB-level immutability triggers block UPDATE and DELETE on admin_audit_logs
    {
        let conn = h.pool.connection().expect("db conn");
        let upd = conn.execute(
            "UPDATE admin_audit_logs SET action = 'tampered' WHERE 1=1",
            [],
        );
        assert!(
            upd.is_err(),
            "UPDATE on admin_audit_logs MUST be aborted by trigger"
        );
        let del = conn.execute("DELETE FROM admin_audit_logs WHERE 1=1", []);
        assert!(
            del.is_err(),
            "DELETE on admin_audit_logs MUST be aborted by trigger"
        );
    }

    // D10: Cluster reassignment & removal recomputes ticket_count on ALL affected clusters
    let (_, t2_body) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/tickets",
        None,
        &[],
        Some(json!({
            "installId": "inst_d10_2",
            "category": "feature_request",
            "title": "Second feature request",
            "description": "Another feature request for cluster count test"
        })),
    )
    .await;
    let t2_id = t2_body["data"]["ticket"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (_, clu_a) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/clusters",
        Some(&super_tok),
        &[],
        Some(json!({ "title": "Cluster Alpha", "ticketIds": [t1_id, t2_id] })),
    )
    .await;
    let clu_a_id = clu_a["data"]["cluster"]["id"].as_str().unwrap().to_string();
    assert_eq!(clu_a["data"]["cluster"]["ticketCount"], 2);

    // Move t2 into Cluster Beta -> Cluster Alpha count MUST drop to 1
    let (_, clu_b) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/clusters",
        Some(&super_tok),
        &[],
        Some(json!({ "title": "Cluster Beta", "ticketIds": [t2_id] })),
    )
    .await;
    assert_eq!(clu_b["data"]["cluster"]["ticketCount"], 1);

    let (_, clusters_list) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/clusters",
        Some(&super_tok),
        &[],
        None,
    )
    .await;
    let alpha_after_move = clusters_list["data"]["clusters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == clu_a_id)
        .expect("cluster alpha");
    assert_eq!(alpha_after_move["ticketCount"], 1);

    // Update Cluster Alpha removing t1 -> Cluster Alpha count MUST drop to 0
    let (_, clu_a_upd) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/clusters",
        Some(&super_tok),
        &[],
        Some(json!({ "id": clu_a_id, "title": "Cluster Alpha", "ticketIds": [] })),
    )
    .await;
    assert_eq!(clu_a_upd["data"]["cluster"]["ticketCount"], 0);

    // D07: Mixed-case APP_CRASH, exact fingerprint match (no substring collision), and regression reopening
    for (ev, fp, inst) in [
        ("APP_CRASH", "fp_base", "u_c1"),
        ("app_crash", "fp_base_extended", "u_c2"),
        ("App_Crash", "fp_base", "u_c3"),
    ] {
        let (st, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/ingest/telemetry",
            None,
            &[],
            Some(json!({
                "installId": inst,
                "eventName": ev,
                "appVersion": "3.0.0",
                "properties": { "fingerprint": fp, "errorSignature": "Sig" }
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
    }
    // Mark fp_base resolved, then send a regression crash -> MUST reopen to 'open'
    {
        let conn = h.pool.connection().expect("db conn");
        conn.execute(
            "UPDATE cloud_crash_groups SET status = 'resolved' WHERE fingerprint = 'fp_base'",
            [],
        )
        .expect("mark resolved");
    }
    let (st_reopen, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[],
        Some(json!({
            "installId": "u_c3",
            "eventName": "APP_CRASH",
            "appVersion": "3.0.0",
            "properties": { "fingerprint": "fp_base", "errorSignature": "Sig" }
        })),
    )
    .await;
    assert_eq!(st_reopen, StatusCode::CREATED);

    let (_, crashes_res) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/crashes?appVersion=3.0.0",
        Some(&super_tok),
        &[],
        None,
    )
    .await;
    let fp_base_row = crashes_res["data"]["crashes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["fingerprint"] == "fp_base")
        .expect("fp_base");
    // Exact distinct installs = 2 (u_c1, u_c3) — NOT 3 (u_c2 had fp_base_extended!)
    assert_eq!(fp_base_row["affectedInstalls"], 2);
    assert_eq!(fp_base_row["status"], "open");

    // D08: Future-timestamp poisoning clamped + non-cohort events cannot push funnel conversion > 100%
    let far_future_ms = Utc::now().timestamp_millis() + 10 * 86_400_000;
    let _ = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[],
        Some(json!({
            "installId": "u_future",
            "eventName": "export_completed",
            "funnelStage": "export",
            "occurredAt": far_future_ms
        })),
    )
    .await;
    let _ = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[],
        Some(json!({
            "installId": "u_cohort_1",
            "eventName": "app_installed",
            "funnelStage": "day_0"
        })),
    )
    .await;
    let _ = send_json(
        &h.app,
        Method::POST,
        "/v1/ingest/telemetry",
        None,
        &[],
        Some(json!({
            "installId": "u_noncohort_jump",
            "eventName": "ai_connected",
            "funnelStage": "ai_connected"
        })),
    )
    .await;

    let (_, fun_body) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/analytics/funnel",
        Some(&super_tok),
        &[],
        None,
    )
    .await;
    let steps = fun_body["data"]["funnelSteps"].as_array().unwrap();
    let mut prev_u = usize::MAX;
    for step in steps {
        let u = step["users"].as_u64().unwrap() as usize;
        let pct = step["conversionRatePct"].as_f64().unwrap();
        assert!(
            u <= prev_u,
            "funnel must be monotonically non-increasing: {steps:?}"
        );
        assert!(
            (0.0..=100.0).contains(&pct),
            "conversion_rate_pct out of 0..=100: {pct}"
        );
        prev_u = u;
    }
}
