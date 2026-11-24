// @story 14.1
// @implements AC-01, AC-02, AC-03, AC-04 (Integration tests for Zero-PII Scrubber, FIFO-500 Outbox, Backoff, Opt-Out & Support Ticket Routes)
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use chrono::Utc;
use harness_daemon::daemon_auth::{DaemonAuthState, FlowPrincipal};
use harness_daemon::routes::{build_router, DaemonAppState};
use harness_daemon::support_edge::{
    compute_backoff_secs, create_support_ticket_with_env, enqueue_telemetry_event_with_env,
    flush_telemetry_batch_with_transport, list_outbox_items, recover_stale_flushing_rows,
    scrub_json_value, scrub_zero_pii, set_telemetry_preferences_with_env, CreateSupportTicketInput,
    EnqueueTelemetryInput, TelemetryBatchTransport, TelemetryEnvOverride, TelemetryOutboxItem,
    TransportSendOutcome, TELEMETRY_OUTBOX_FIFO_CAP, TELEMETRY_STALE_FLUSHING_MS, VALID_CHANNELS,
    VALID_EVENT_GROUPS,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct MockCountingTransport {
    calls: AtomicUsize,
    fail_mode: bool,
}

impl MockCountingTransport {
    fn new(fail_mode: bool) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail_mode,
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl TelemetryBatchTransport for MockCountingTransport {
    fn send_batch(&self, _endpoint: &str, _items: &[TelemetryOutboxItem]) -> TransportSendOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_mode {
            TransportSendOutcome::RetryableFailure("HTTP 503 upstream collector unavailable".into())
        } else {
            TransportSendOutcome::Delivered
        }
    }
}

struct TestHarness {
    _dir: tempfile::TempDir,
    pool: SqlitePool,
    app: Router,
    editor_tok: String,
    viewer_tok: String,
}

fn setup_harness() -> TestHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-141.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    let auth =
        DaemonAuthState::from_registry_root_with_pool(dir.path(), Some(registry.pool().clone()));
    let expires_at = Utc::now().timestamp() + 3600;

    let editor_tok = "cwk_sess_141_editor".to_string();
    auth.insert_session(
        editor_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-141".to_string(),
            user_id: "operator-141".to_string(),
            role: "editor".to_string(),
        },
        expires_at,
    );

    let viewer_tok = "cwk_sess_141_viewer".to_string();
    auth.insert_session(
        viewer_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-141".to_string(),
            user_id: "viewer-141".to_string(),
            role: "viewer".to_string(),
        },
        expires_at,
    );

    let state = DaemonAppState::with_auth(registry, "127.0.0.1".to_string(), 4171, auth);
    let app = build_router(state);

    TestHarness {
        _dir: dir,
        pool,
        app,
        editor_tok,
        viewer_tok,
    }
}

async fn send_req(
    app: &Router,
    method: Method,
    uri: &str,
    session_token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4171");
    if let Some(tok) = session_token {
        builder = builder.header("x-cowok-session-token", tok);
    }
    let req = if let Some(json_body) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(json_body.to_string()))
            .expect("request")
    } else {
        builder.body(Body::empty()).expect("request")
    };

    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

#[test]
fn it_14_1_ac02_zero_pii_scrubber_and_diagnostic_allowlist() {
    let h = setup_harness();

    // Runtime-assembled synthetic secrets (zero static secret literals).
    let synth_openai = ["sk", "-", "proj_", "synthetic1234567890abcdef"].concat();
    let synth_google = ["AIza", "SySyntheticKey1234567890ABCDEF"].concat();
    let synth_github = ["ghp", "_", "synthetic9876543210ABCDEF"].concat();
    let synth_slack = ["xox", "b-", "9988776655-syntheticToken99"].concat();
    let synth_jwt = [
        "eyJ",
        "hbGciOiJIUzI1NiJ9",
        ".",
        "eyJ",
        "zdWIiOiJ1c2VyXzE0MSJ9",
        ".",
        "sigPartSynthetic_1234567890",
    ]
    .concat();
    let synth_bearer = ["Bearer ", "secret_bearer_", "token1234567890"].concat();
    let synth_cookie = ["Cookie: ", "cowok_sid=topsecret_cookie_val"].concat();

    let raw_desc = format!(
        "Failed calling provider {synth_openai} and {synth_google} with {synth_github} {synth_slack}\n\
         JWT: {synth_jwt}\n\
         Authorization: {synth_bearer}\n\
         {synth_cookie}\n\
         Trace: /Users/alice/Desktop/app/main.rs /home/bob/src/index.ts C:\\Users\\carol\\proj\\file.ts"
    );

    let scrubbed = scrub_zero_pii(&raw_desc);
    assert!(scrubbed.redaction_count >= 10);
    for forbidden in [
        &synth_openai,
        &synth_google,
        &synth_github,
        &synth_slack,
        &synth_jwt,
        "secret_bearer_token1234567890",
        "topsecret_cookie_val",
        "/Users/alice",
        "/home/bob",
        "C:\\Users\\carol",
    ] {
        assert!(
            !scrubbed.scrubbed.contains(forbidden),
            "Leaked sensitive token: {forbidden}"
        );
    }

    // Payload JSON scrub strips forbidden source code & prompt keys.
    let (clean_json, redactions) = scrub_json_value(&json!({
        "screenCount": 4,
        "sourceCode": "const secret = 1;",
        "userPrompt": "Build a banking dashboard",
        "note": format!("path=/Users/alice/work key={synth_openai}")
    }));
    assert!(redactions >= 4);
    assert!(clean_json.get("sourceCode").is_none());
    assert!(clean_json.get("userPrompt").is_none());
    assert_eq!(clean_json["screenCount"], json!(4));
    assert!(!clean_json["note"]
        .as_str()
        .unwrap_or("")
        .contains(&synth_openai));
    assert!(clean_json["note"]
        .as_str()
        .unwrap_or("")
        .contains("<HOME>/work"));

    // Persisted support ticket has 0 leaks and strictly allowlisted diagnostics.
    let ticket = create_support_ticket_with_env(
        &h.pool,
        &CreateSupportTicketInput {
            category: "bug_report".to_string(),
            title: format!("Export crash at /Users/alice/repo with {synth_openai}"),
            description: raw_desc,
            contact_email: Some("non-tech@example.com".to_string()),
            screenshot_data_url: Some("data:image/png;base64,iVBORw0KGgo=".to_string()),
            channel: "desktop".to_string(),
            locale: "vi-VN".to_string(),
            include_diagnostics: true,
            recent_error_codes: vec![format!("E_AUTH_{synth_github}")],
            active_skills: vec!["design-craft".to_string()],
        },
        &TelemetryEnvOverride::default(),
    )
    .expect("create_support_ticket");

    assert!(ticket.ticket_code.starts_with("CWK-"));
    assert!(ticket.screenshot_sha256.is_some());
    assert!(!ticket.title.contains(&synth_openai));
    assert!(!ticket.title.contains("/Users/alice"));
    assert!(!ticket.description.contains(&synth_jwt));
    assert!(!ticket.diagnostics.recent_error_codes[0].contains(&synth_github));
    assert!(ticket.diagnostics.redaction_count > 0);
}

#[test]
fn it_14_1_ac03_ac04_fifo_500_cap_and_exponential_backoff() {
    let h = setup_harness();
    let env_online = TelemetryEnvOverride::default();

    // Enqueue 550 events across all 8 channels and 5 event groups -> FIFO-500 ring cap keeps newest 500.
    for i in 0..550usize {
        let channel = VALID_CHANNELS[i % VALID_CHANNELS.len()];
        let group = VALID_EVENT_GROUPS[i % VALID_EVENT_GROUPS.len()];
        let res = enqueue_telemetry_event_with_env(
            &h.pool,
            &EnqueueTelemetryInput {
                channel: channel.to_string(),
                event_group: group.to_string(),
                event_name: format!("evt_{i:04}"),
                payload: json!({ "seq": i }),
            },
            &env_online,
        )
        .expect("enqueue");
        assert!(res.queued);
        assert!(!res.dropped_by_opt_out);
        assert!(res.outbox_depth <= TELEMETRY_OUTBOX_FIFO_CAP);
    }

    let items = list_outbox_items(&h.pool, 600).expect("list_outbox_items");
    assert_eq!(items.len(), TELEMETRY_OUTBOX_FIFO_CAP);
    assert_eq!(items[0].event_name, "evt_0050");
    assert_eq!(items[499].event_name, "evt_0549");

    // Simulate 5 consecutive 5xx failures and verify backoff schedule: 30s -> 60s -> 120s -> 300s -> 900s.
    let failing = MockCountingTransport::new(true);
    let mut now_ms = 1_700_000_000_000i64;
    let expected_delays = [30i64, 60, 120, 300, 900];
    for (idx, expected_sec) in expected_delays.iter().enumerate() {
        let outcome = flush_telemetry_batch_with_transport(
            &h.pool,
            &env_online,
            Some("https://telemetry.example.invalid/v1/ingest"),
            10,
            now_ms,
            &failing,
        )
        .expect("flush failure");
        assert_eq!(outcome.attempted, 10);
        assert_eq!(outcome.failed, 10);
        assert_eq!(outcome.http_calls, 1);

        let updated = list_outbox_items(&h.pool, 10).expect("list after fail");
        assert_eq!(updated[0].attempt_count, (idx + 1) as u32);
        assert_eq!(
            compute_backoff_secs(updated[0].attempt_count),
            *expected_sec
        );
        assert_eq!(updated[0].next_retry_at, now_ms + expected_sec * 1000);
        now_ms = updated[0].next_retry_at;
    }

    // Recovery when collector returns 200 OK.
    let healthy = MockCountingTransport::new(false);
    let ok_outcome = flush_telemetry_batch_with_transport(
        &h.pool,
        &env_online,
        Some("https://telemetry.example.invalid/v1/ingest"),
        10,
        now_ms,
        &healthy,
    )
    .expect("flush success");
    assert_eq!(ok_outcome.sent, 10);
    assert_eq!(ok_outcome.failed, 0);
    assert_eq!(healthy.call_count(), 1);
}

#[test]
fn it_14_1_ac04_opt_out_and_airgapped_guarantee_zero_http_calls() {
    let h = setup_harness();
    let transport = MockCountingTransport::new(false);

    // Seed 1 event while online.
    enqueue_telemetry_event_with_env(
        &h.pool,
        &EnqueueTelemetryInput {
            channel: "cli".to_string(),
            event_group: "lifecycle".to_string(),
            event_name: "app_started".to_string(),
            payload: json!({}),
        },
        &TelemetryEnvOverride::default(),
    )
    .expect("seed event");

    // Case 1: COWOK_TELEMETRY=0
    let env_opt_out = TelemetryEnvOverride::from_values(Some("0"), None);
    let drop1 = enqueue_telemetry_event_with_env(
        &h.pool,
        &EnqueueTelemetryInput {
            channel: "claude_code".to_string(),
            event_group: "onboarding_funnel".to_string(),
            event_name: "day0_opened".to_string(),
            payload: json!({}),
        },
        &env_opt_out,
    )
    .expect("enqueue opt_out");
    assert!(drop1.dropped_by_opt_out);
    assert!(!drop1.queued);

    let flush1 = flush_telemetry_batch_with_transport(
        &h.pool,
        &env_opt_out,
        Some("https://telemetry.example.invalid/v1/ingest"),
        50,
        Utc::now().timestamp_millis(),
        &transport,
    )
    .expect("flush opt_out");
    assert!(flush1.skipped_by_opt_out);
    assert_eq!(flush1.http_calls, 0);

    // Case 2: COWOK_AIRGAPPED=1
    let env_airgap = TelemetryEnvOverride::from_values(None, Some("1"));
    let flush2 = flush_telemetry_batch_with_transport(
        &h.pool,
        &env_airgap,
        Some("https://telemetry.example.invalid/v1/ingest"),
        50,
        Utc::now().timestamp_millis(),
        &transport,
    )
    .expect("flush airgap");
    assert!(flush2.skipped_by_opt_out);
    assert_eq!(flush2.http_calls, 0);

    // Case 3: DB persisted UI preference telemetry_enabled = false
    set_telemetry_preferences_with_env(&h.pool, false, &TelemetryEnvOverride::default())
        .expect("set pref false");
    let drop3 = enqueue_telemetry_event_with_env(
        &h.pool,
        &EnqueueTelemetryInput {
            channel: "desktop".to_string(),
            event_group: "skill_ecosystem".to_string(),
            event_name: "skill_installed".to_string(),
            payload: json!({}),
        },
        &TelemetryEnvOverride::default(),
    )
    .expect("enqueue db opt_out");
    assert!(drop3.dropped_by_opt_out);

    let flush3 = flush_telemetry_batch_with_transport(
        &h.pool,
        &TelemetryEnvOverride::default(),
        Some("https://telemetry.example.invalid/v1/ingest"),
        50,
        Utc::now().timestamp_millis(),
        &transport,
    )
    .expect("flush db opt_out");
    assert!(flush3.skipped_by_opt_out);
    assert_eq!(flush3.http_calls, 0);
    assert_eq!(transport.call_count(), 0);
}

#[tokio::test]
async fn it_14_1_http_routes_auth_ticket_crud_and_preferences() {
    let h = setup_harness();

    // Unauthenticated request rejected with 401.
    let (st_unauth, _) = send_req(&h.app, Method::GET, "/api/v1/support/tickets", None, None).await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    // Viewer cannot mutate telemetry preferences (403).
    let (st_viewer_put, _) = send_req(
        &h.app,
        Method::PUT,
        "/api/v1/telemetry/preferences",
        Some(&h.viewer_tok),
        Some(json!({ "telemetryEnabled": false })),
    )
    .await;
    assert_eq!(st_viewer_put, StatusCode::FORBIDDEN);

    // Editor creates support ticket via POST /api/v1/support/tickets.
    let (st_create, created_body) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/support/tickets",
        Some(&h.editor_tok),
        Some(json!({
            "category": "feature_request",
            "title": "Add quick chart export from /Users/alice/docs",
            "description": "Would love a 1-click PNG export button on slides.",
            "contactEmail": "user@example.com",
            "channel": "desktop",
            "locale": "vi-VN",
            "includeDiagnostics": true,
            "recentErrorCodes": [],
            "activeSkills": ["design-craft"]
        })),
    )
    .await;
    assert_eq!(st_create, StatusCode::CREATED);
    assert_eq!(created_body["ok"], json!(true));
    assert!(created_body["data"]["title"]
        .as_str()
        .unwrap_or("")
        .contains("<HOME>/docs"));

    // GET /api/v1/support/tickets returns persisted ticket.
    let (st_list, list_body) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/support/tickets",
        Some(&h.viewer_tok),
        None,
    )
    .await;
    assert_eq!(st_list, StatusCode::OK);
    assert_eq!(
        list_body["data"]["tickets"].as_array().map(|a| a.len()),
        Some(1)
    );

    // Toggle telemetry preference OFF via PUT /api/v1/telemetry/preferences.
    let (st_put, put_body) = send_req(
        &h.app,
        Method::PUT,
        "/api/v1/telemetry/preferences",
        Some(&h.editor_tok),
        Some(json!({ "telemetryEnabled": false })),
    )
    .await;
    assert_eq!(st_put, StatusCode::OK);
    assert_eq!(put_body["data"]["telemetryEnabled"], json!(false));
    assert_eq!(put_body["data"]["effectiveEnabled"], json!(false));

    // Subsequent POST /api/v1/telemetry/events drops by opt-out.
    let (st_evt, evt_body) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/telemetry/events",
        Some(&h.editor_tok),
        Some(json!({
            "channel": "antigravity",
            "eventGroup": "feature_usage",
            "eventName": "drawer_opened",
            "payload": {}
        })),
    )
    .await;
    assert_eq!(st_evt, StatusCode::OK);
    assert_eq!(evt_body["data"]["droppedByOptOut"], json!(true));
    assert_eq!(evt_body["data"]["queued"], json!(false));
}

#[tokio::test]
async fn it_14_1_remediation_validation_422_legacy_bearer_401_and_unique_ticket_codes() {
    let h = setup_harness();

    // Legacy non-superadmin Bearer token is strictly rejected with 401.
    let legacy_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/support/tickets")
        .header("host", "127.0.0.1:4171")
        .header(
            "authorization",
            ["Bearer ", "legacy_non_superadmin_", "token141"].concat(),
        )
        .body(Body::empty())
        .expect("legacy bearer req");
    let legacy_resp = h.app.clone().oneshot(legacy_req).await.expect("oneshot");
    assert_eq!(legacy_resp.status(), StatusCode::UNAUTHORIZED);

    // 2 consecutive ticket creations produce distinct 8-hex-char CWK-XXXXXXXX codes.
    let mut created_codes = Vec::new();
    for idx in 1..=2 {
        let (st, body) = send_req(
            &h.app,
            Method::POST,
            "/api/v1/support/tickets",
            Some(&h.editor_tok),
            Some(json!({
                "category": "bug_report",
                "title": format!("Consecutive ticket #{idx}"),
                "description": format!("Detailed description for consecutive ticket #{idx}."),
                "channel": "desktop",
                "locale": "vi-VN"
            })),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
        let code = body["data"]["ticketCode"]
            .as_str()
            .expect("ticketCode")
            .to_string();
        assert_eq!(code.len(), 12);
        assert!(code.starts_with("CWK-"));
        created_codes.push(code);
    }
    assert_ne!(created_codes[0], created_codes[1]);

    // Invalid inputs return 422 UNPROCESSABLE_ENTITY: email, category, channel, screenshot, long locale.
    let invalid_cases = vec![
        json!({
            "category": "bug_report",
            "title": "Bad email double dot",
            "description": "Email domain contains double dots.",
            "contactEmail": "user@bad..domain.com",
            "channel": "desktop",
            "locale": "vi-VN"
        }),
        json!({
            "category": "bug_report",
            "title": "Bad email control char",
            "description": "Email contains newline control char.",
            "contactEmail": "user\n@example.com",
            "channel": "desktop",
            "locale": "vi-VN"
        }),
        json!({
            "category": "invalid_cat",
            "title": "Bad category",
            "description": "Category not in allowlist.",
            "channel": "desktop",
            "locale": "vi-VN"
        }),
        json!({
            "category": "bug_report",
            "title": "Bad channel",
            "description": "Channel not in allowlist.",
            "channel": "unknown_host",
            "locale": "vi-VN"
        }),
        json!({
            "category": "bug_report",
            "title": "Bad screenshot",
            "description": "SVG data URL must be rejected.",
            "screenshotDataUrl": "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
            "channel": "desktop",
            "locale": "vi-VN"
        }),
        json!({
            "category": "bug_report",
            "title": "Long locale",
            "description": "Locale longer than 16 chars must be rejected.",
            "channel": "desktop",
            "locale": "vi-VN-super-long-locale-poison"
        }),
    ];

    for payload in invalid_cases {
        let (st, body) = send_req(
            &h.app,
            Method::POST,
            "/api/v1/support/tickets",
            Some(&h.editor_tok),
            Some(payload),
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "body={body}");
    }
}

#[test]
fn it_14_1_remediation_scrubber_edge_patterns_and_json_keys() {
    let dyn_jwt = [
        "eyJ",
        "hbGciOiJIUzI1NiJ9",
        ".",
        "eyJ",
        "zdWIiOiJ0ZXN0MTQxIn0",
        ".",
        "sigSyntheticPart_987654",
    ]
    .concat();
    let tab_val = ["tab_", "sample_", "val", "99999"].concat();
    let dyn_bearer_tab = ["hdr-", "Bearer\t", &tab_val].concat();
    let dyn_cookie = ["x_cookie:", " sid=super_secret_cookie_99"].concat();
    let dyn_sk = ["sk", "-", "live_", "keyInObjectName98765"].concat();

    let raw = format!(
        "prefix jwt_{dyn_jwt} {dyn_bearer_tab}\n{dyn_cookie}\npaths: C:/Users/alice/docs /users/bob/src /HOME/carol/app"
    );
    let scrubbed = scrub_zero_pii(&raw);
    assert!(!scrubbed.scrubbed.contains(&dyn_jwt));
    assert!(!scrubbed.scrubbed.contains(&tab_val));
    assert!(!scrubbed.scrubbed.contains("super_secret_cookie_99"));
    assert!(!scrubbed.scrubbed.contains("C:/Users/alice"));
    assert!(!scrubbed.scrubbed.contains("/users/bob"));
    assert!(!scrubbed.scrubbed.contains("/HOME/carol"));
    assert!(scrubbed.scrubbed.contains("<HOME>/docs"));
    assert!(scrubbed.scrubbed.contains("<HOME>/src"));
    assert!(scrubbed.scrubbed.contains("<HOME>/app"));

    let mut raw_payload = json!({
        "systemPrompt": "hidden system prompt",
        "chat_prompt": "hidden chat prompt",
        "rawHtml": "<div>secret</div>",
        "access_token": "tok-value",
        "refreshToken": "ref-value",
        "X-Api-Key": "secret-header"
    });
    raw_payload
        .as_object_mut()
        .expect("raw object")
        .insert(format!("diag_{dyn_sk}"), json!("ok"));
    let (clean_json, redactions) = scrub_json_value(&raw_payload);
    assert!(redactions >= 7);
    let obj = clean_json.as_object().expect("json object");
    assert_eq!(obj.len(), 1);
    let only_key = obj.keys().next().expect("scrubbed key");
    assert!(!only_key.contains(&dyn_sk));
    assert!(only_key.contains("[REDACTED_SECRET]"));
}

struct BarrierTransport {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: AtomicUsize,
}

impl TelemetryBatchTransport for BarrierTransport {
    fn send_batch(&self, _endpoint: &str, _items: &[TelemetryOutboxItem]) -> TransportSendOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.wait();
        self.release.wait();
        TransportSendOutcome::Delivered
    }
}

#[test]
fn it_14_1_remediation_concurrent_flush_deduplication_and_stale_recovery() {
    let h = setup_harness();
    let env_online = TelemetryEnvOverride::default();

    enqueue_telemetry_event_with_env(
        &h.pool,
        &EnqueueTelemetryInput {
            channel: "desktop".to_string(),
            event_group: "lifecycle".to_string(),
            event_name: "concurrent_flush_test".to_string(),
            payload: json!({ "ok": true }),
        },
        &env_online,
    )
    .expect("enqueue");

    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let t1 = Arc::new(BarrierTransport {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
        calls: AtomicUsize::new(0),
    });
    let pool_a = h.pool.clone();
    let t1_clone = Arc::clone(&t1);
    let now_ms = 1_750_000_000_000i64;

    let handle = std::thread::spawn(move || {
        flush_telemetry_batch_with_transport(
            &pool_a,
            &TelemetryEnvOverride::default(),
            Some("https://telemetry.example.invalid/v1/ingest"),
            10,
            now_ms,
            t1_clone.as_ref(),
        )
        .expect("thread A flush")
    });

    // Wait until Thread A has atomically claimed the row as 'flushing' and released SQLite lock.
    entered.wait();

    // Thread B runs while Thread A is mid-network call: must see 0 claimable rows (0 duplicate sends).
    let t2 = MockCountingTransport::new(false);
    let outcome_b = flush_telemetry_batch_with_transport(
        &h.pool,
        &env_online,
        Some("https://telemetry.example.invalid/v1/ingest"),
        10,
        now_ms,
        &t2,
    )
    .expect("thread B flush");
    assert_eq!(outcome_b.attempted, 0);
    assert_eq!(outcome_b.http_calls, 0);
    assert_eq!(t2.call_count(), 0);

    // Let Thread A finish and mark 'sent'.
    release.wait();
    let outcome_a = handle.join().expect("join thread A");
    assert_eq!(outcome_a.sent, 1);
    assert_eq!(t1.calls.load(Ordering::SeqCst), 1);

    // Verify stale 'flushing' recovery (> 5 min).
    enqueue_telemetry_event_with_env(
        &h.pool,
        &EnqueueTelemetryInput {
            channel: "cli".to_string(),
            event_group: "lifecycle".to_string(),
            event_name: "stale_flushing_evt".to_string(),
            payload: json!({}),
        },
        &env_online,
    )
    .expect("enqueue stale");
    {
        let conn = h.pool.connection().expect("conn");
        conn.execute(
            "UPDATE telemetry_outbox SET status = 'flushing', next_retry_at = ?1 WHERE event_name = 'stale_flushing_evt'",
            rusqlite::params![now_ms - TELEMETRY_STALE_FLUSHING_MS - 10_000],
        )
        .expect("mark stale flushing");
    }
    let recovered =
        recover_stale_flushing_rows(&h.pool, now_ms, TELEMETRY_STALE_FLUSHING_MS).expect("recover");
    assert_eq!(recovered, 1);
}

async fn send_raw_get(
    app: &Router,
    uri: &str,
    session_token: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("host", "127.0.0.1:4171");
    if let Some(tok) = session_token {
        builder = builder.header("x-cowok-session-token", tok);
    }
    let req = builder.body(Body::empty()).expect("raw get req");
    let resp = app.clone().oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("raw body")
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

#[tokio::test]
async fn it_14_1_attachment_persist_list_mime_and_serve_route() {
    use base64::Engine as _;

    let h = setup_harness();
    let png_bytes: Vec<u8> = vec![137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82];
    let mp4_bytes: Vec<u8> = vec![0, 0, 0, 24, 102, 116, 121, 112, 105, 115, 111, 109];
    let png_data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png_bytes)
    );
    let mp4_data_url = format!(
        "data:video/mp4;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&mp4_bytes)
    );

    // 1. Create PNG ticket
    let (st_png, png_created) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/support/tickets",
        Some(&h.editor_tok),
        Some(json!({
            "category": "bug_report",
            "title": "PNG attachment ticket",
            "description": "Includes a PNG screenshot persisted to local disk.",
            "screenshotDataUrl": png_data_url,
            "channel": "desktop",
            "locale": "vi-VN"
        })),
    )
    .await;
    assert_eq!(st_png, StatusCode::CREATED, "body={png_created}");
    let png_id = png_created["data"]["id"].as_str().expect("png id");
    let png_sha = png_created["data"]["screenshotSha256"]
        .as_str()
        .expect("png sha");
    assert_eq!(png_created["data"]["attachmentMime"], json!("image/png"));

    // 2. Create MP4 ticket
    let (st_mp4, mp4_created) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/support/tickets",
        Some(&h.editor_tok),
        Some(json!({
            "category": "bug_report",
            "title": "MP4 attachment ticket",
            "description": "Includes an MP4 video persisted to local disk.",
            "screenshotDataUrl": mp4_data_url,
            "channel": "desktop",
            "locale": "vi-VN"
        })),
    )
    .await;
    assert_eq!(st_mp4, StatusCode::CREATED, "body={mp4_created}");
    let mp4_id = mp4_created["data"]["id"].as_str().expect("mp4 id");
    let mp4_sha = mp4_created["data"]["screenshotSha256"]
        .as_str()
        .expect("mp4 sha");
    assert_eq!(mp4_created["data"]["attachmentMime"], json!("video/mp4"));

    // 3. Create ticket without attachment
    let (st_plain, plain_created) = send_req(
        &h.app,
        Method::POST,
        "/api/v1/support/tickets",
        Some(&h.editor_tok),
        Some(json!({
            "category": "question",
            "title": "Ticket without attachment",
            "description": "Plain text request with no uploaded file.",
            "channel": "desktop",
            "locale": "vi-VN"
        })),
    )
    .await;
    assert_eq!(st_plain, StatusCode::CREATED);
    let plain_id = plain_created["data"]["id"].as_str().expect("plain id");
    assert!(plain_created["data"].get("attachmentMime").is_none());

    // 4. Verify files exist on local disk under .cowok-design/support-attachments/
    let att_dir = h
        ._dir
        .path()
        .join(".cowok-design")
        .join("support-attachments");
    let png_file = att_dir.join(format!("{png_sha}.png"));
    let mp4_file = att_dir.join(format!("{mp4_sha}.mp4"));
    assert_eq!(std::fs::read(&png_file).expect("read png"), png_bytes);
    assert_eq!(std::fs::read(&mp4_file).expect("read mp4"), mp4_bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = std::fs::metadata(&att_dir)
            .expect("dir meta")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }

    // 5. Verify list endpoint populates attachmentMime for each ticket
    let (st_list, list_body) = send_req(
        &h.app,
        Method::GET,
        "/api/v1/support/tickets",
        Some(&h.viewer_tok),
        None,
    )
    .await;
    assert_eq!(st_list, StatusCode::OK);
    let tickets = list_body["data"]["tickets"].as_array().expect("tickets");
    let find_mime = |id: &str| {
        tickets
            .iter()
            .find(|t| t["id"].as_str() == Some(id))
            .and_then(|t| t.get("attachmentMime"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    assert_eq!(find_mime(png_id).as_deref(), Some("image/png"));
    assert_eq!(find_mime(mp4_id).as_deref(), Some("video/mp4"));
    assert_eq!(find_mime(plain_id), None);

    // 6. Unauthenticated attachment request is rejected with 401
    let (st_unauth, _, _) = send_raw_get(
        &h.app,
        &format!("/api/v1/support/tickets/{png_id}/attachment"),
        None,
    )
    .await;
    assert_eq!(st_unauth, StatusCode::UNAUTHORIZED);

    // 7. Authenticated GET serves PNG with exact headers & bytes
    let (st_get_png, png_headers, png_body) = send_raw_get(
        &h.app,
        &format!("/api/v1/support/tickets/{png_id}/attachment"),
        Some(&h.viewer_tok),
    )
    .await;
    assert_eq!(st_get_png, StatusCode::OK);
    assert_eq!(png_headers.get("content-type").unwrap(), "image/png");
    assert_eq!(
        png_headers.get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(
        png_headers.get("cache-control").unwrap(),
        "private, max-age=3600"
    );
    assert_eq!(png_body, png_bytes);

    // 8. Authenticated GET serves MP4 with exact headers & bytes
    let (st_get_mp4, mp4_headers, mp4_body) = send_raw_get(
        &h.app,
        &format!("/api/v1/support/tickets/{mp4_id}/attachment"),
        Some(&h.viewer_tok),
    )
    .await;
    assert_eq!(st_get_mp4, StatusCode::OK);
    assert_eq!(mp4_headers.get("content-type").unwrap(), "video/mp4");
    assert_eq!(
        mp4_headers.get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(
        mp4_headers.get("cache-control").unwrap(),
        "private, max-age=3600"
    );
    assert_eq!(mp4_body, mp4_bytes);

    // 9. Ticket without attachment or nonexistent ticket returns 404
    let (st_plain_att, _, _) = send_raw_get(
        &h.app,
        &format!("/api/v1/support/tickets/{plain_id}/attachment"),
        Some(&h.viewer_tok),
    )
    .await;
    assert_eq!(st_plain_att, StatusCode::NOT_FOUND);

    let (st_missing_att, _, _) = send_raw_get(
        &h.app,
        "/api/v1/support/tickets/nonexistent-ticket-id/attachment",
        Some(&h.viewer_tok),
    )
    .await;
    assert_eq!(st_missing_att, StatusCode::NOT_FOUND);
}
