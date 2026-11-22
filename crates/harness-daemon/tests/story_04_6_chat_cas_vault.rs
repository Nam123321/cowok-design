// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05, UT-IDOR-001, UT-IDEMPOTENCY-001..003, UT-FK-RECEIPT-001, UT-GC-RACE-001, UT-ARCHIVE-CAPS-001, UT-RETRY-001, UT-ERROR-001
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::{body::Body, extract::Request, http::StatusCode};
use chrono::Utc;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::chat_cas_archive::run_cas_gc_sweep;
use harness_daemon::routes::chat_cas_storage::{
    execute_immediate_with_retry, purge_expired_idempotency_receipts, safe_api_error_response,
    ChatCasTxError,
};
use harness_daemon::routes::chat_cas_upload_handler::delete_cas_attachment_scoped;
use harness_daemon::{build_router, DaemonAppState};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;

fn setup_daemon_with_tenants(
    tmp: &TempDir,
) -> (Arc<DaemonAppState>, axum::Router, String, String, String) {
    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, tmp.path())).expect("open wal");
    let reg = Arc::new(SpecRegistry::new(pool));
    let state = Arc::new(DaemonAppState::new(reg, "127.0.0.1".into(), 4170));
    let exp = Utc::now().timestamp() + 3600;

    let tok_a = format!("sess-a-{}", uuid::Uuid::new_v4());
    state.auth.insert_session(
        tok_a.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "user-a".into(),
            role: "editor".into(),
        },
        exp,
    );
    let tok_b = format!("sess-b-{}", uuid::Uuid::new_v4());
    state.auth.insert_session(
        tok_b.clone(),
        FlowPrincipal {
            tenant_id: "tenant-beta".into(),
            user_id: "user-b".into(),
            role: "editor".into(),
        },
        exp,
    );
    let tok_viewer = format!("sess-v-{}:viewer", uuid::Uuid::new_v4());
    state.auth.insert_session(
        tok_viewer.clone(),
        FlowPrincipal {
            tenant_id: "tenant-alpha".into(),
            user_id: "viewer-a".into(),
            role: "viewer".into(),
        },
        exp,
    );
    let router = build_router((*state).clone());
    (state, router, tok_a, tok_b, tok_viewer)
}

async fn json_req(
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
        b = b.header("x-idempotency-key", k);
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
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        st,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn upload_cas_req(
    router: &axum::Router,
    tok: &str,
    sid: &str,
    filename: &str,
    idem_key: &str,
    bytes: Vec<u8>,
) -> (StatusCode, serde_json::Value) {
    let boundary = "----CowokCasTestBoundary";
    let mut payload = Vec::new();
    payload.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"sessionId\"\r\n\r\n{sid}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"idempotencyKey\"\r\n\r\n{idem_key}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    payload.extend_from_slice(&bytes);
    payload.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/cas/upload")
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", tok)
        .header("x-idempotency-key", idem_key)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(payload))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let st = resp.status();
    let out = resp.into_body().collect().await.unwrap().to_bytes();
    (
        st,
        serde_json::from_slice(&out).unwrap_or(serde_json::Value::Null),
    )
}

fn build_synthetic_zip(entries: &[(&str, u32, u32)]) -> Vec<u8> {
    let mut buf = b"PK\x03\x04".to_vec();
    let cd_offset = buf.len() as u32;
    for &(name, comp, uncomp) in entries {
        buf.extend_from_slice(b"PK\x01\x02");
        buf.extend_from_slice(&[20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        buf.extend_from_slice(&comp.to_le_bytes());
        buf.extend_from_slice(&uncomp.to_le_bytes());
        buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&[0u8; 12]);
        buf.extend_from_slice(name.as_bytes());
    }
    let cd_size = (buf.len() as u32) - cd_offset;
    let n = entries.len() as u16;
    buf.extend_from_slice(b"PK\x05\x06");
    buf.extend_from_slice(&[0, 0, 0, 0]);
    buf.extend_from_slice(&n.to_le_bytes());
    buf.extend_from_slice(&n.to_le_bytes());
    buf.extend_from_slice(&cd_size.to_le_bytes());
    buf.extend_from_slice(&cd_offset.to_le_bytes());
    buf.extend_from_slice(&[0, 0]);
    buf
}

#[tokio::test]
async fn ut_idor_001_and_error_no_leak() {
    let tmp = TempDir::new().unwrap();
    let (_state, router, tok_a, tok_b, tok_viewer) = setup_daemon_with_tenants(&tmp);

    // 1. Auto-provision session for tenant-alpha + ingest vault receipt
    let (st, m) = json_req(
        &router,
        "POST",
        "/api/v1/chat-sessions/sid-a1/messages",
        &tok_a,
        None,
        Some(json!({ "role": "user", "proseMarkdown": "hello AIzaSyD4u5v6w7x8y9z01234567890abcdef" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    assert!(m["proseMarkdown"]
        .as_str()
        .unwrap()
        .contains("[REDACTED:GOOGLE_AI_STUDIO]"));

    let (st_v, v_rcpt) = json_req(
        &router,
        "POST",
        "/api/v1/vault/chat-ingest",
        &tok_a,
        Some("vk-1"),
        Some(json!({
            "sessionId": "sid-a1",
            "provider": "typesafe_jev",
            "rawSecretEnvelope": "jev_live_secret_token_987654321",
            "enterpriseLocalOnlyLock": true
        })),
    )
    .await;
    assert_eq!(st_v, StatusCode::CREATED);
    let rid = v_rcpt["receiptId"].as_str().unwrap();

    // Viewer forbidden (403)
    let (st_vw1, _) = json_req(
        &router,
        "POST",
        "/api/v1/chat-sessions/sid-a1/messages",
        &tok_viewer,
        None,
        Some(json!({ "role": "user", "proseMarkdown": "nope" })),
    )
    .await;
    let (st_vw2, _) = upload_cas_req(
        &router,
        &tok_viewer,
        "sid-a1",
        "a.csv",
        "cv1",
        b"a,b\n1,2\n".to_vec(),
    )
    .await;
    let (st_vw3, _) = json_req(&router, "POST", "/api/v1/vault/chat-ingest", &tok_viewer, Some("vk-vw"), Some(json!({ "sessionId": "sid-a1", "provider": "custom", "rawSecretEnvelope": "jev_secret_viewer_1234" }))).await;
    let (st_vw4, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/vault/receipts/{rid}"),
        &tok_viewer,
        None,
        None,
    )
    .await;
    assert_eq!(st_vw1, StatusCode::FORBIDDEN);
    assert_eq!(st_vw2, StatusCode::FORBIDDEN);
    assert_eq!(st_vw3, StatusCode::FORBIDDEN);
    assert_eq!(st_vw4, StatusCode::FORBIDDEN);

    // Cross-tenant & missing IDs across all 6 endpoints return identical 404 RESOURCE_NOT_FOUND (UT-ERROR-001 no-leak)
    for (method, uri, body) in [
        ("GET", "/api/v1/chat-sessions/sid-a1/messages", None),
        ("GET", "/api/v1/chat-sessions/missing-sid/messages", None),
        (
            "POST",
            "/api/v1/chat-sessions/sid-a1/messages",
            Some(json!({"role":"user","proseMarkdown":"hijack"})),
        ),
        (
            "POST",
            "/api/v1/vault/chat-ingest",
            Some(
                json!({"sessionId":"sid-a1","provider":"custom","rawSecretEnvelope":"jev_secret_hijack_9999"}),
            ),
        ),
        (
            "POST",
            &format!("/api/v1/vault/receipts/{rid}"),
            Some(json!({"provider":"custom","rawSecretEnvelope":"jev_secret_replace_999"})),
        ),
        ("DELETE", &format!("/api/v1/vault/receipts/{rid}"), None),
        ("DELETE", "/api/v1/vault/receipts/rcpt-nonexistent", None),
    ] {
        let (s, e) = json_req(&router, method, uri, &tok_b, Some("xb"), body).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{method} {uri}");
        assert_eq!(e["code"], "RESOURCE_NOT_FOUND");
        assert!(!e.to_string().contains("tenant-alpha"));
    }
    let (s_cas_x, e_cas_x) = upload_cas_req(
        &router,
        &tok_b,
        "sid-a1",
        "x.csv",
        "x1",
        b"a,b\n1,2\n".to_vec(),
    )
    .await;
    assert_eq!(s_cas_x, StatusCode::NOT_FOUND);
    assert_eq!(e_cas_x["code"], "RESOURCE_NOT_FOUND");
}

#[tokio::test]
async fn ut_idempotency_001_002_003_and_fk_and_gc_race() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok_a, _, _) = setup_daemon_with_tenants(&tmp);

    let (st_m, _) = json_req(
        &router,
        "POST",
        "/api/v1/chat-sessions/sid-idem/messages",
        &tok_a,
        None,
        Some(json!({"role": "user", "proseMarkdown": "init session"})),
    )
    .await;
    assert_eq!(st_m, StatusCode::CREATED);

    let csv_bytes = b"col1,col2\n10,20\n".to_vec();
    let (st1, d1) = upload_cas_req(
        &router,
        &tok_a,
        "sid-idem",
        "data.csv",
        "k-same",
        csv_bytes.clone(),
    )
    .await;
    assert_eq!(st1, StatusCode::CREATED);
    assert_eq!(d1["replayed"], false);
    let att1 = d1["id"].as_str().unwrap().to_string();
    let sha = d1["blobSha256"].as_str().unwrap().to_string();

    // UT-IDEMPOTENCY-001: Replay returns 200 without incrementing refcount, usage, or ingress
    let (st2, d2) = upload_cas_req(
        &router,
        &tok_a,
        "sid-idem",
        "data.csv",
        "k-same",
        csv_bytes.clone(),
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    assert_eq!(d2["replayed"], true);
    assert_eq!(d2["id"], att1);

    let (st_mismatch, d_mis) = upload_cas_req(
        &router,
        &tok_a,
        "sid-idem",
        "data.csv",
        "k-same",
        b"different,csv\n99,100\n".to_vec(),
    )
    .await;
    assert_eq!(st_mismatch, StatusCode::CONFLICT);
    assert_eq!(d_mis["code"], "IDEMPOTENCY_KEY_PAYLOAD_MISMATCH");

    let conn = state.registry.pool().connection().unwrap();
    let ref_cnt: i64 = conn
        .query_row(
            "SELECT ref_count FROM cas_blobs WHERE sha256 = ?1",
            params![sha],
            |r| r.get(0),
        )
        .unwrap();
    let ingress_cnt: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cas_upload_ingress WHERE tenant_id = 'tenant-alpha'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ref_cnt, 1);
    assert_eq!(ingress_cnt, 1);

    // UT-IDEMPOTENCY-002: Expire CAS receipt (>24h), reuse same key -> previous attachment/blob/ingress stay intact
    conn.execute(
        "UPDATE cas_command_idempotency SET expires_at = accepted_at - 10 WHERE attachment_id = ?1",
        params![att1],
    )
    .unwrap();
    let (st3, d3) = upload_cas_req(
        &router,
        &tok_a,
        "sid-idem",
        "data2.csv",
        "k-same",
        b"x,y\n1,2\n".to_vec(),
    )
    .await;
    assert_eq!(st3, StatusCode::CREATED);
    let att2 = d3["id"].as_str().unwrap().to_string();
    assert_ne!(att1, att2);
    let old_att_still: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cas_attachments WHERE id = ?1",
            params![att1],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(old_att_still, 1);

    // UT-IDEMPOTENCY-003: Vault receipt TTL boundary (expires_at == now_ms kept, < now_ms purged), zero raw secrets in SQLite
    let (_, vr) = json_req(
        &router,
        "POST",
        "/api/v1/vault/chat-ingest",
        &tok_a,
        Some("vk-ttl"),
        Some(json!({
            "sessionId": "sid-idem",
            "provider": "google_ai_studio",
            "rawSecretEnvelope": "AIzaSyTopSecretVaultKey9988776655",
            "enterpriseLocalOnlyLock": true
        })),
    )
    .await;
    let raw_row_json: String = conn
        .query_row(
            "SELECT receipt_json FROM vault_command_idempotency WHERE receipt_id = ?1",
            params![vr["receiptId"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!raw_row_json.contains("AIzaSyTopSecret"));

    {
        let mut c2 = state.registry.pool().connection().unwrap();
        let now_exact = Utc::now().timestamp_millis();
        c2.execute(
            "UPDATE vault_command_idempotency SET expires_at = ?1 WHERE idempotency_key = 'vk-ttl'",
            params![now_exact],
        )
        .unwrap();
        let tx = c2.transaction().unwrap();
        let (_, v_purged_eq) =
            purge_expired_idempotency_receipts(&tx, "tenant-alpha", now_exact).unwrap();
        tx.commit().unwrap();
        assert_eq!(v_purged_eq, 0, "expires_at == now_ms must NOT be purged");
        let tx2 = c2.transaction().unwrap();
        let (_, v_purged_lt) =
            purge_expired_idempotency_receipts(&tx2, "tenant-alpha", now_exact + 1).unwrap();
        tx2.commit().unwrap();
        assert_eq!(v_purged_lt, 1);
    }

    // UT-FK-RECEIPT-001 & UT-GC-RACE-001: Deleting attachment cascades its CAS receipt & respects 60s GC lease
    assert!(delete_cas_attachment_scoped(
        &state,
        "tenant-alpha",
        &att2,
        Utc::now().timestamp_millis()
    )
    .await
    .unwrap());
    let receipt_for_att2: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cas_command_idempotency WHERE attachment_id = ?1",
            params![att2],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(receipt_for_att2, 0);

    let sha2 = d3["blobSha256"].as_str().unwrap().to_string();
    let blob_path2 = tmp.path().join(".cowok-design/cas").join(&sha2);
    assert!(blob_path2.exists());
    let swept_before_lease = run_cas_gc_sweep(&state, Utc::now().timestamp_millis() + 1_000).await;
    assert_eq!(swept_before_lease, 0);
    assert!(blob_path2.exists());

    let swept_after_lease = run_cas_gc_sweep(&state, Utc::now().timestamp_millis() + 120_000).await;
    assert!(swept_after_lease >= 1);
    assert!(!blob_path2.exists());
}

#[tokio::test]
#[allow(clippy::result_large_err)]
async fn ut_archive_caps_001_and_retry_001() {
    let tmp = TempDir::new().unwrap();
    let (state, router, tok_a, _, _) = setup_daemon_with_tenants(&tmp);
    let _ = json_req(
        &router,
        "POST",
        "/api/v1/chat-sessions/sid-zip/messages",
        &tok_a,
        None,
        Some(json!({"role":"user","proseMarkdown":"zip test"})),
    )
    .await;

    // Valid ZIP .xlsx succeeds
    let valid_zip = build_synthetic_zip(&[("xl/workbook.xml", 50, 100)]);
    let (st_ok, _) = upload_cas_req(&router, &tok_a, "sid-zip", "ok.xlsx", "zk-1", valid_zip).await;
    assert_eq!(st_ok, StatusCode::CREATED);

    // Ratio > 20:1 fails 422
    let ratio_bomb = build_synthetic_zip(&[("xl/huge.xml", 10, 500)]);
    let (st_ratio, b_ratio) =
        upload_cas_req(&router, &tok_a, "sid-zip", "bomb.xlsx", "zk-2", ratio_bomb).await;
    assert_eq!(st_ratio, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b_ratio["code"], "CAS_ARCHIVE_REJECTED");

    // >1024 entries fails 422
    let many_names: Vec<(String, u32, u32)> =
        (0..1025).map(|i| (format!("e{i}.txt"), 2, 10)).collect();
    let many_refs: Vec<(&str, u32, u32)> = many_names
        .iter()
        .map(|(s, c, u)| (s.as_str(), *c, *u))
        .collect();
    let (st_cnt, _) = upload_cas_req(
        &router,
        &tok_a,
        "sid-zip",
        "many.fig",
        "zk-3",
        build_synthetic_zip(&many_refs),
    )
    .await;
    assert_eq!(st_cnt, StatusCode::UNPROCESSABLE_ENTITY);

    // Oversize >26,214,400 + 262,144 fails 413 CAS_QUOTA_EXCEEDED without leaking internals
    let oversized = vec![b'a'; 26_500_000];
    let (st_big, b_big) = upload_cas_req(
        &router,
        &tok_a,
        "sid-zip",
        "huge-secret-name.csv",
        "zk-4",
        oversized,
    )
    .await;
    assert_eq!(st_big, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(b_big["code"], "CAS_QUOTA_EXCEEDED");
    assert_eq!(b_big["limit"], "REQUEST_BYTES");
    assert!(!b_big.to_string().contains("huge-secret-name"));

    // UT-RETRY-001 (A): Real SQLite IMMEDIATE lock contention -> 503 CHAT_WRITE_CONTENTION + 0 partial rows
    let mut lock_conn = state.registry.pool().connection().unwrap();
    let lock_tx = lock_conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    lock_tx.execute("INSERT OR IGNORE INTO cas_upload_ingress (id, tenant_id, session_id, byte_size, accepted_at) VALUES ('lock-seed', 'tenant-alpha', 'sid-zip', 1, 1000)", []).unwrap();

    let db_path = tmp.path().join(".cowok-design/cowok-design.sqlite3");
    let err_real_busy = execute_immediate_with_retry(
        || {
            let c = rusqlite::Connection::open(&db_path).map_err(|_| {
                safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "ERR",
                    "err",
                    false,
                    None,
                )
            })?;
            c.busy_timeout(std::time::Duration::from_millis(0)).unwrap();
            Ok(c)
        },
        |_tx| Ok::<(), ChatCasTxError>(()),
    )
    .await
    .unwrap_err();
    drop(lock_tx);
    assert_eq!(err_real_busy.status(), StatusCode::SERVICE_UNAVAILABLE);

    // UT-RETRY-001 (B): Closure contention verifies exact 3 retry attempts & 0 partial writes
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_cl = Arc::clone(&attempts);
    let err_resp = execute_immediate_with_retry(
        || {
            rusqlite::Connection::open_in_memory().map_err(|_| {
                safe_api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "ERR",
                    "err",
                    false,
                    None,
                )
            })
        },
        |_tx| {
            attempts_cl.fetch_add(1, Ordering::SeqCst);
            Err::<(), ChatCasTxError>(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    Some("locked".to_string()),
                )
                .into(),
            )
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body_bytes = err_resp.into_body().collect().await.unwrap().to_bytes();
    let err_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(err_json["code"], "CHAT_WRITE_CONTENTION");
    assert_eq!(err_json["retryable"], true);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);

    // CSV/TSV formula/DDE neutralization + malformed provider key -> INVALID status
    let csv_payload = b"a,b\n=HYPERLINK(\"https://x\",\"y\"),\"=SUM(1,2)\"\n+1,-2\n";
    let (st_csv, body_csv) = upload_cas_req(
        &router,
        &tok_a,
        "sid-zip",
        "formulas.csv",
        "csv-dde-1",
        csv_payload.to_vec(),
    )
    .await;
    assert_eq!(st_csv, StatusCode::CREATED);
    let stored_csv = std::fs::read_to_string(
        tmp.path()
            .join(body_csv["storageRelpath"].as_str().unwrap()),
    )
    .unwrap();
    assert!(stored_csv.contains("'=HYPERLINK("));
    assert!(stored_csv.contains("\"'=SUM(1,2)\""));

    let (st_inv, body_inv) = json_req(
        &router,
        "POST",
        "/api/v1/vault/chat-ingest",
        &tok_a,
        Some("vk-invalid-fmt"),
        Some(json!({
            "sessionId": "sid-zip",
            "provider": "google_ai_studio",
            "rawSecretEnvelope": "not_a_valid_google_key_123",
            "enterpriseLocalOnlyLock": true
        })),
    )
    .await;
    assert_eq!(st_inv, StatusCode::CREATED);
    assert_eq!(body_inv["status"], "INVALID");
    assert_eq!(body_inv["localOnly"], true);

    // OCR-046-R2-004 & R2-005 & R2-002: Standalone auto-provision on /vault/chat-ingest + short 8-char mask + replace/revoke DB parity
    let (st_short, b_short) = json_req(
        &router,
        "POST",
        "/api/v1/vault/chat-ingest",
        &tok_a,
        Some("vk-short-8"),
        Some(json!({
            "sessionId": "sid-standalone-fresh",
            "provider": "custom",
            "rawSecretEnvelope": "12345678",
            "enterpriseLocalOnlyLock": true
        })),
    )
    .await;
    assert_eq!(st_short, StatusCode::CREATED);
    assert_eq!(b_short["presenterMask"], "12...78");
    let rcpt_id = b_short["receiptId"].as_str().unwrap().to_string();

    let (st_msg, _) = json_req(
        &router,
        "POST",
        "/api/v1/chat-sessions/sid-standalone-fresh/messages",
        &tok_a,
        None,
        Some(json!({
            "role": "user",
            "proseMarkdown": "[VAULTED:custom]",
            "vaultReceipt": b_short
        })),
    )
    .await;
    assert_eq!(st_msg, StatusCode::CREATED);

    let (st_rep, b_rep) = json_req(
        &router,
        "POST",
        &format!("/api/v1/vault/receipts/{rcpt_id}"),
        &tok_a,
        None,
        Some(json!({
            "provider": "google_ai_studio",
            "rawSecretEnvelope": "AIzaSyReplacedVaultKey9876543210",
            "enterpriseLocalOnlyLock": true
        })),
    )
    .await;
    assert_eq!(st_rep, StatusCode::OK);
    assert_eq!(b_rep["presenterMask"], "AIza...3210");

    let (st_g1, b_g1) = json_req(
        &router,
        "GET",
        "/api/v1/chat-sessions/sid-standalone-fresh/messages",
        &tok_a,
        None,
        None,
    )
    .await;
    assert_eq!(st_g1, StatusCode::OK);
    assert_eq!(
        b_g1["messages"][0]["vaultReceipt"]["presenterMask"],
        "AIza...3210"
    );

    let (st_rev, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/vault/receipts/{rcpt_id}"),
        &tok_a,
        None,
        None,
    )
    .await;
    assert_eq!(st_rev, StatusCode::OK);
    let (st_g2, b_g2) = json_req(
        &router,
        "GET",
        "/api/v1/chat-sessions/sid-standalone-fresh/messages",
        &tok_a,
        None,
        None,
    )
    .await;
    assert_eq!(st_g2, StatusCode::OK);
    assert!(b_g2["messages"][0]["vaultReceipt"].is_null());

    // F-05: Binary PNG containing `--boundary` mid-payload is stored intact without truncation
    let boundary = "cowoktestbound99";
    let mut png_with_bound = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    png_with_bound.extend_from_slice(b"BEFORE--cowoktestbound99AFTER");
    let mut mp = Vec::new();
    mp.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"sessionId\"\r\n\r\nsid-cas-standalone\r\n").as_bytes());
    mp.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"pic.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
    mp.extend_from_slice(&png_with_bound);
    mp.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req_png = Request::builder()
        .method("POST")
        .uri("/api/v1/cas/upload")
        .header("host", "127.0.0.1:4170")
        .header("x-cowok-session-token", &tok_a)
        .header("x-idempotency-key", "png-bound-1")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(mp))
        .unwrap();
    let resp_png = router.clone().oneshot(req_png).await.unwrap();
    assert_eq!(resp_png.status(), StatusCode::CREATED);
    let body_png: serde_json::Value =
        serde_json::from_slice(&resp_png.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body_png["byteSize"], png_with_bound.len() as i64);
}
