// @story 08.4
// @cover AC-01, AC-02, AC-03, AC-04, AC-05
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use base64::{engine::general_purpose::STANDARD as B64_STD, Engine as _};
use chrono::Utc;
use harness_daemon::{
    daemon_auth::FlowPrincipal,
    read_daemon_lock,
    routes::hmac_sha256,
    routes::plugins::CompilePromptBundleRequest,
    routes::plugins_cron::{
        clone_repo_hardened, compute_backoff_ms, load_bundle_from_dir_verified,
        probe_upstream_with_ssrf_redirects, validate_redirect_target, UpstreamCheckResult,
    },
    vault::VaultStore,
    DaemonConfig, DaemonHttpClient, DaemonLockInfo, DaemonServer,
};
use http_body_util::BodyExt;
use plugin_host::{HumanSessionAuthority, LearnPipeline};
use rusqlite::params;
use sha2::{Digest, Sha256};
use spec_registry::plugin_store::{
    claim_cron_lease, commit_cron_fenced, get_cron_schedule_scoped, reap_stale_cron_leases,
    upsert_cron_schedule, FencedCronCommit, SkillPluginRow,
};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

static ENV_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn setup_test_daemon() -> (
    TempDir,
    std::path::PathBuf,
    SqlitePool,
    Arc<SpecRegistry>,
    DaemonServer,
    String,
    String,
) {
    let tmp = TempDir::new().expect("tmp");
    let canon_root = tmp.path().canonicalize().expect("canon");
    let db_path = canon_root.join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &canon_root)).expect("wal");
    let project_a = Uuid::new_v4().to_string();
    let project_b = Uuid::new_v4().to_string();
    let conn = pool.connection().expect("conn");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, slug, name, product_brief, workspace_root, created_at, updated_at)
         VALUES (?1, 'tenant-alpha', 'alpha-proj', 'Alpha Proj', 'Alpha Brief', '.', 1700000000000, 1700000000000)",
        params![project_a],
    )
    .expect("seed alpha");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, slug, name, product_brief, workspace_root, created_at, updated_at)
         VALUES (?1, 'tenant-beta', 'beta-proj', 'Beta Proj', 'Beta Brief', '.', 1700000000000, 1700000000000)",
        params![project_b],
    )
    .expect("seed beta");
    drop(conn);

    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    let cfg = DaemonConfig {
        host: "127.0.0.1".into(),
        preferred_port: 0,
        workspace_root: canon_root.clone(),
    };
    let server = DaemonServer::bind_loopback(&cfg, registry.clone()).expect("bind");
    (
        tmp, canon_root, pool, registry, server, project_a, project_b,
    )
}

fn sign_hmac_v2_headers(
    lock: &DaemonLockInfo,
    method: &str,
    path_and_query: &str,
    tenant_id: &str,
    user_id: &str,
    role: &str,
    raw_body: &[u8],
) -> Vec<(String, String)> {
    let (canon_path, raw_query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    let key_bytes: Vec<u8> = (0..lock.hmac_key_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&lock.hmac_key_hex[i..i + 2], 16).unwrap())
        .collect();
    let ts = Utc::now().timestamp().to_string();
    let req_nonce = Uuid::new_v4().to_string();
    let principal_b64 = B64_STD.encode(
        serde_json::to_vec(&FlowPrincipal {
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            role: role.into(),
        })
        .unwrap(),
    );
    let body_sha = format!("{:x}", Sha256::digest(raw_body));
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        lock.daemon_nonce, ts, req_nonce, method, canon_path, raw_query, principal_b64, body_sha
    );
    let sig_hex: String = hmac_sha256(&key_bytes, canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    vec![
        ("host".into(), format!("127.0.0.1:{}", lock.port)),
        ("x-cowok-hmac-sig".into(), sig_hex),
        ("x-cowok-timestamp".into(), ts),
        ("x-cowok-nonce".into(), req_nonce),
        ("x-cowok-principal".into(), principal_b64),
    ]
}

async fn send_raw(
    router: &Router,
    method: &str,
    uri: &str,
    headers: &[(String, String)],
    body: Vec<u8>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    if !body.is_empty() {
        builder = builder.header("content-type", "application/json");
    }
    let req = builder.body(Body::from(body)).expect("build req");
    let resp = router.clone().oneshot(req).await.expect("oneshot");
    let st = resp.status();
    let bytes = resp.into_body().collect().await.expect("bytes").to_bytes();
    let parsed = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned())
        })
    };
    (st, parsed)
}

struct CallScope<'a> {
    router: &'a Router,
    lock: &'a DaemonLockInfo,
    tenant_id: &'a str,
    user_id: &'a str,
    role: &'a str,
}

async fn signed_call(
    scope: &CallScope<'_>,
    method: &str,
    uri: &str,
    extra_headers: &[(&str, &str)],
    json_body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let body_bytes = json_body
        .map(|v| serde_json::to_vec(&v).expect("json"))
        .unwrap_or_default();
    let mut headers = sign_hmac_v2_headers(
        scope.lock,
        method,
        uri,
        scope.tenant_id,
        scope.user_id,
        scope.role,
        &body_bytes,
    );
    for (k, v) in extra_headers {
        headers.push(((*k).to_string(), (*v).to_string()));
    }
    send_raw(scope.router, method, uri, &headers, body_bytes).await
}

async fn issue_human_token_for_test(
    router_port: u16,
    router: &Router,
    tenant_id: &str,
    user_id: &str,
) -> String {
    let payload = serde_json::to_vec(&serde_json::json!({
        "tenantId": tenant_id,
        "userId": user_id,
        "role": "editor",
    }))
    .unwrap();
    let headers = vec![
        ("host".to_string(), format!("127.0.0.1:{router_port}")),
        (
            "x-cowok-workbench-secret".to_string(),
            "human-secret-084".to_string(),
        ),
    ];
    let (st, val) = send_raw(
        router,
        "POST",
        "/api/v1/auth/workbench-session",
        &headers,
        payload,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "issue human session failed: {val}");
    val["sessionToken"]
        .as_str()
        .expect("sessionToken")
        .to_string()
}

#[tokio::test]
async fn test_01_actor_forgery_human_only_and_idor_boundaries() {
    let (_tmp, canon_root, _pool, _reg, server, proj_a, proj_b) = setup_test_daemon();
    let lock = read_daemon_lock(&canon_root).expect("lock");
    let router = server.router();

    // Verify mcp-gateway exposes ZERO HUMAN_ONLY plugin/learn control routes or tools
    let mcp_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../mcp-gateway/src/lib.rs"
    ))
    .expect("mcp-gateway src");
    assert!(!mcp_src.contains("learn/cards") && !mcp_src.contains("staging/promotions"));

    // Hold ENV_MUTEX while setting COWOK_WORKBENCH_HUMAN_SECRET to issue an interactive HumanWorkbenchSession
    let human_token = {
        let _guard = ENV_MUTEX.lock().await;
        std::env::set_var("COWOK_WORKBENCH_HUMAN_SECRET", "human-secret-084");
        // Verify daemon.lock / missing secret CANNOT mint HUMAN_ONLY workbench session
        let deny_headers = vec![
            ("host".to_string(), format!("127.0.0.1:{}", lock.port)),
            (
                "x-cowok-daemon-nonce".to_string(),
                lock.daemon_nonce.clone(),
            ),
        ];
        let (deny_st, deny_json) = send_raw(
            &router,
            "POST",
            "/api/v1/auth/workbench-session",
            &deny_headers,
            b"{}".to_vec(),
        )
        .await;
        assert_eq!(deny_st, StatusCode::FORBIDDEN);
        assert_eq!(deny_json["code"], "HUMAN_APPROVAL_REQUIRED");

        let tok = issue_human_token_for_test(lock.port, &router, "tenant-alpha", "alice").await;
        std::env::remove_var("COWOK_WORKBENCH_HUMAN_SECRET");
        tok
    };

    let alpha_agent = CallScope {
        router: &router,
        lock: &lock,
        tenant_id: "tenant-alpha",
        user_id: "agent-1",
        role: "editor",
    };
    let alpha_human = CallScope {
        router: &router,
        lock: &lock,
        tenant_id: "tenant-alpha",
        user_id: "alice",
        role: "editor",
    };
    let alpha_mcp = CallScope {
        router: &router,
        lock: &lock,
        tenant_id: "tenant-alpha",
        user_id: "mcp-runner",
        role: "editor",
    };
    let beta_bob = CallScope {
        router: &router,
        lock: &lock,
        tenant_id: "tenant-beta",
        user_id: "bob",
        role: "editor",
    };

    // AgentHmac can discover plugins (low-risk DISCOVERED)
    let discover_payload = serde_json::json!({
        "projectId": proj_a,
        "pluginSlug": "alpha-skill",
        "sourceType": "local",
        "sourceRepoUrl": "https://github.com/cowok-ai/skills",
        "manifest": {
            "name": "alpha-skill",
            "version": "1.0.0",
            "description": "Alpha test skill",
            "capabilities_required": ["specs.read"],
            "risk_tier": 1
        }
    });
    let (disc_st, disc_json) = signed_call(
        &alpha_agent,
        "POST",
        "/api/v1/plugins/discovery",
        &[],
        Some(discover_payload.clone()),
    )
    .await;
    assert_eq!(disc_st, StatusCode::OK, "discover failed: {disc_json}");
    let plugin_id = disc_json["id"].as_str().unwrap().to_string();
    assert_eq!(disc_json["lifecycleState"], "DISCOVERED");

    // 1. AgentHmac + spoofed principal JSON actorKind MUST be rejected with 403 on installs & ACTIVE transitions
    let spoof_principal_b64 = B64_STD.encode(
        serde_json::json!({
            "tenant_id": "tenant-alpha",
            "user_id": "agent-1",
            "role": "editor",
            "actorKind": "HUMAN_WORKBENCH_SESSION",
            "actor_kind": "HumanWorkbenchSession"
        })
        .to_string()
        .as_bytes(),
    );
    let key_bytes: Vec<u8> = (0..lock.hmac_key_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&lock.hmac_key_hex[i..i + 2], 16).unwrap())
        .collect();
    let install_body = serde_json::to_vec(&discover_payload).unwrap();
    let ts = Utc::now().timestamp().to_string();
    let nonce = Uuid::new_v4().to_string();
    let canonical = format!(
        "{}\n{}\n{}\nPOST\n/api/v1/plugins/installs\n\n{}\n{:x}",
        lock.daemon_nonce,
        ts,
        nonce,
        spoof_principal_b64,
        Sha256::digest(&install_body)
    );
    let sig: String = hmac_sha256(&key_bytes, canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let (spoof_st, spoof_json) = send_raw(
        &router,
        "POST",
        "/api/v1/plugins/installs",
        &[
            ("host".into(), format!("127.0.0.1:{}", lock.port)),
            ("x-cowok-hmac-sig".into(), sig),
            ("x-cowok-timestamp".into(), ts),
            ("x-cowok-nonce".into(), nonce),
            ("x-cowok-principal".into(), spoof_principal_b64),
        ],
        install_body,
    )
    .await;
    assert_eq!(spoof_st, StatusCode::FORBIDDEN);
    assert_eq!(spoof_json["code"], "HUMAN_APPROVAL_REQUIRED");

    // Agent transition to ACTIVE is also rejected 403 (even if using daemon.lock local-session style AgentHmac)
    let transition_uri = format!("/api/v1/plugins/{plugin_id}/transitions");
    let (ag_act_st, ag_act_json) = signed_call(
        &alpha_mcp,
        "POST",
        &transition_uri,
        &[("x-cowok-client-kind", "mcp")],
        Some(serde_json::json!({
            "projectId": proj_a,
            "targetState": "ACTIVE",
            "expectedRevision": 1
        })),
    )
    .await;
    assert_eq!(ag_act_st, StatusCode::FORBIDDEN);
    assert_eq!(ag_act_json["code"], "HUMAN_APPROVAL_REQUIRED");

    // Human Workbench Session promotes DISCOVERED -> VALIDATED -> INSTALLED -> ACTIVE
    let (val_st, val_json) = signed_call(
        &alpha_human,
        "POST",
        &transition_uri,
        &[("x-cowok-human-session", human_token.as_str())],
        Some(serde_json::json!({
            "projectId": proj_a,
            "targetState": "VALIDATED",
            "expectedRevision": 1
        })),
    )
    .await;
    assert_eq!(
        val_st,
        StatusCode::OK,
        "validate transition failed: {val_json}"
    );
    let val_rev = val_json["revision"].as_u64().unwrap();

    let (inst_st, inst_json) = signed_call(
        &alpha_human,
        "POST",
        &transition_uri,
        &[("x-cowok-human-session", human_token.as_str())],
        Some(serde_json::json!({
            "projectId": proj_a,
            "targetState": "INSTALLED",
            "expectedRevision": val_rev
        })),
    )
    .await;
    assert_eq!(
        inst_st,
        StatusCode::OK,
        "install transition failed: {inst_json}"
    );
    let rev = inst_json["revision"].as_u64().unwrap();

    // CAS stale revision check (409)
    let (cas_st, cas_json) = signed_call(
        &alpha_human,
        "POST",
        &transition_uri,
        &[("x-cowok-human-session", human_token.as_str())],
        Some(serde_json::json!({
            "projectId": proj_a,
            "targetState": "ACTIVE",
            "expectedRevision": 999
        })),
    )
    .await;
    assert_eq!(
        cas_st,
        StatusCode::CONFLICT,
        "expected CAS conflict: {cas_json}"
    );

    let (act_st, act_json) = signed_call(
        &alpha_human,
        "POST",
        &transition_uri,
        &[("x-cowok-human-session", human_token.as_str())],
        Some(serde_json::json!({
            "projectId": proj_a,
            "targetState": "ACTIVE",
            "expectedRevision": rev
        })),
    )
    .await;
    assert_eq!(act_st, StatusCode::OK, "activate failed: {act_json}");
    assert_eq!(act_json["lifecycleState"], "ACTIVE");

    // Cross-tenant IDOR across plugins, LEARN cards, and schedules MUST return 404 NOT_FOUND
    let (idor_plug_st, _) = signed_call(
        &beta_bob,
        "GET",
        &format!("/api/v1/plugins/{plugin_id}/schedule"),
        &[],
        None,
    )
    .await;
    assert_eq!(idor_plug_st, StatusCode::NOT_FOUND);

    let (idor_learn_st, _) = signed_call(
        &beta_bob,
        "GET",
        &format!("/api/v1/projects/{proj_a}/learn/cards"),
        &[],
        None,
    )
    .await;
    assert_eq!(idor_learn_st, StatusCode::NOT_FOUND);

    // Propose LEARN card via Agent + verify idempotent replay + reject HUMAN_ONLY approval from Agent/MCP -> allow Human
    let learn_uri = format!("/api/v1/projects/{proj_a}/learn/cards");
    let learn_body = serde_json::json!({
        "idempotencyKey": "idem-key-01",
        "category": "RULE",
        "title": "Enforce tokens.css",
        "content": "Always use var(--space-3) and never import Tailwind utility classes.",
        "sourceEvidence": "tokens.css:L1-L30"
    });
    let (p1_st, p1_json) = signed_call(
        &alpha_agent,
        "POST",
        &learn_uri,
        &[],
        Some(learn_body.clone()),
    )
    .await;
    assert_eq!(p1_st, StatusCode::OK, "propose card failed: {p1_json}");
    let card_id = p1_json["id"].as_str().unwrap().to_string();

    // Idempotent replay returns same card id without duplicate
    let (p2_st, p2_json) =
        signed_call(&alpha_agent, "POST", &learn_uri, &[], Some(learn_body)).await;
    assert_eq!(p2_st, StatusCode::OK);
    assert_eq!(p2_json["id"], card_id);

    // Agent/MCP MUST be rejected (403) on LEARN approve/reject, staging promote/discard, PUT schedule, POST sync-jobs
    for (m, u, body) in [
        (
            "POST",
            format!("/api/v1/projects/{proj_a}/learn/cards/{card_id}/approvals"),
            serde_json::json!({ "reason": "try spoof", "expectedRevision": 1 }),
        ),
        (
            "POST",
            format!("/api/v1/projects/{proj_a}/learn/cards/{card_id}/rejections"),
            serde_json::json!({ "reason": "try spoof", "expectedRevision": 1 }),
        ),
        (
            "POST",
            format!("/api/v1/plugins/{plugin_id}/staging/promotions"),
            serde_json::json!({ "projectId": proj_a, "reason": "try spoof", "expectedRevision": 1 }),
        ),
        (
            "POST",
            format!("/api/v1/plugins/{plugin_id}/staging/discards"),
            serde_json::json!({ "projectId": proj_a, "reason": "try spoof", "expectedRevision": 1 }),
        ),
        (
            "PUT",
            format!("/api/v1/plugins/{plugin_id}/schedule"),
            serde_json::json!({ "projectId": proj_a, "cronExpression": "*/5 * * * *", "upstreamRepoUrl": "https://github.com/cowok-ai/skills" }),
        ),
        (
            "POST",
            format!("/api/v1/plugins/{plugin_id}/sync-jobs"),
            serde_json::json!({ "projectId": proj_a }),
        ),
    ] {
        let (st, j) = signed_call(
            &alpha_agent,
            m,
            &u,
            &[("x-cowok-client-kind", "mcp")],
            Some(body),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "expected 403 on {m} {u}: {j}");
        assert_eq!(j["code"], "HUMAN_APPROVAL_REQUIRED");
    }

    // Human Workbench Session approves LEARN card and cross-tenant beta cannot touch proj_b plugins
    let approve_uri = format!("/api/v1/projects/{proj_a}/learn/cards/{card_id}/approvals");
    let (ap_st, ap_json) = signed_call(
        &alpha_human,
        "POST",
        &approve_uri,
        &[("x-cowok-human-session", human_token.as_str())],
        Some(serde_json::json!({ "reason": "Verified rule against DESIGN.md", "expectedRevision": 1 })),
    )
    .await;
    assert_eq!(
        ap_st,
        StatusCode::OK,
        "approve learn card failed: {ap_json}"
    );
    assert_eq!(ap_json["card"]["status"], "ACCEPTED");
    assert_eq!(ap_json["actorKind"], "HUMAN_WORKBENCH_SESSION");

    // Prompt compilation writes immutable `.cowok-design/.cowok-skills/<run_id>/` + LEARN cap <= 1,500 tokens
    let run_id = format!("run-{}", Uuid::new_v4().simple());
    let (cmp_st, cmp_json) = signed_call(
        &alpha_agent,
        "POST",
        &format!("/api/v1/projects/{proj_a}/skills/prompt-bundles"),
        &[],
        Some(serde_json::json!({ "runId": run_id, "selectedSkillNames": ["alpha-skill"] })),
    )
    .await;
    assert_eq!(cmp_st, StatusCode::OK, "compile failed: {cmp_json}");
    assert!(cmp_json["learnTotalTokens"].as_u64().unwrap() <= 1500);
    assert!(cmp_json["fencedPromptXml"]
        .as_str()
        .unwrap()
        .contains("Enforce tokens.css"));
    let staged_path = Path::new(cmp_json["stagingDir"].as_str().unwrap());
    assert!(staged_path.join("alpha-skill/plugin.json").exists());

    // DaemonHttpClient synchronous compile_skill_prompt_bundle rejects duplicate run_id rewrite
    let client = DaemonHttpClient::new(_reg.clone(), lock.host.clone(), lock.port);
    let dup_err = client.compile_skill_prompt_bundle(
        "tenant-alpha",
        Uuid::parse_str(&proj_a).unwrap(),
        &CompilePromptBundleRequest {
            run_id: run_id.clone(),
            selected_skill_names: vec!["alpha-skill".into()],
            max_learn_tokens: None,
        },
    );
    assert!(dup_err.is_err(), "duplicate run_id must fail immutably");

    // Suppress unused warning on proj_b after verifying tenant isolation
    let beta_viewer = CallScope {
        router: &router,
        lock: &lock,
        tenant_id: "tenant-beta",
        user_id: "bob",
        role: "viewer",
    };
    let (beta_list_st, beta_list) = signed_call(
        &beta_viewer,
        "GET",
        &format!("/api/v1/projects/{proj_b}/learn/cards"),
        &[],
        None,
    )
    .await;
    assert_eq!(beta_list_st, StatusCode::OK);
    assert!(beta_list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_02_vault_kid_rotation_ssrf_redirect_git_hardening_and_cron_fencing() {
    let (_tmp, canon_root, pool, _reg, _server, proj_a, _proj_b) = setup_test_daemon();

    // 1. Vault 0o600 key ring rotation + historical LEARN signature verification + nonce replay block
    let vault = VaultStore::from_workspace_root(&canon_root);
    let ring1 = vault.ensure_or_load_learn_signing_ring().expect("ring1");
    let vault_mode = std::fs::metadata(vault.vault_path())
        .expect("vault meta")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(vault_mode, 0o600);

    let auth1 = HumanSessionAuthority::from_key_with_kid(&ring1.active_kid, ring1.active_key)
        .expect("auth1");
    let mut pipeline = LearnPipeline::new();
    let card_sha = pipeline
        .propose_card_scoped(&plugin_host::ProposeCardInput {
            tenant_id: "tenant-alpha",
            project_id: &proj_a,
            id: "card-hist-1",
            idempotency_key: None,
            category: plugin_host::LearnCardCategory::Rule,
            title: "Title",
            content: "Body",
            source_evidence: "specs/DESIGN.md",
            created_at: 1_700_000_000,
        })
        .expect("propose")
        .content_sha256
        .clone();

    let old_sig = auth1
        .mint_token(&plugin_host::MintTokenInput {
            actor_kind: plugin_host::ActorKind::HumanWorkbenchSession,
            actor_id: "alice",
            tenant_id: "tenant-alpha",
            project_id: &proj_a,
            card_id: "card-hist-1",
            content_sha256: &card_sha,
            action: plugin_host::CardDecisionAction::Accept,
            expected_revision: 1,
            reason: "Initial review",
            now_s: 1_700_000_000,
            ttl_s: 300,
            nonce: "nonce-hist-1",
        })
        .expect("mint old token");

    let ring2 = vault.rotate_learn_signing_key().expect("rotate ring2");
    assert_ne!(ring2.active_kid, ring1.active_kid);
    assert_eq!(ring2.retired.len(), 1);
    assert_eq!(ring2.retired[0].0, ring1.active_kid);

    pipeline.register_authority(
        &HumanSessionAuthority::from_key_with_kid(&ring2.active_kid, ring2.active_key).unwrap(),
    );
    for (rkid, rkey) in &ring2.retired {
        pipeline
            .register_authority(&HumanSessionAuthority::from_key_with_kid(rkid, *rkey).unwrap());
    }
    // Historical signature minted BEFORE rotation verifies cleanly using retired kid ring
    pipeline
        .accept_card_verified(
            "tenant-alpha",
            &proj_a,
            "card-hist-1",
            &old_sig,
            1,
            1_700_000_010,
        )
        .expect("historical kid signature must verify after rotation");
    // Replaying the consumed nonce must fail closed
    assert!(pipeline
        .accept_card_verified(
            "tenant-alpha",
            &proj_a,
            "card-hist-1",
            &old_sig,
            2,
            1_700_000_011,
        )
        .is_err());

    // 2. Exponential backoff + AUTH_REQUIRED fail-closed + recovery via Vault github PAT clear
    assert_eq!(compute_backoff_ms(1, None, 0), 30_000);
    assert_eq!(compute_backoff_ms(2, None, 0), 60_000);
    assert!(compute_backoff_ms(20, None, 0) <= 3_600_000);

    // Seed plugin + schedule in DB
    let plugin_id = "plug-cron-1";
    let plugin_row = SkillPluginRow {
        id: plugin_id.into(),
        tenant_id: "tenant-alpha".into(),
        project_id: proj_a.clone(),
        plugin_slug: "cron-skill".into(),
        version: "1.0.0".into(),
        lifecycle_state: "ACTIVE".into(),
        source_type: "github".into(),
        source_repo_url: "https://github.com/cowok-ai/skills".into(),
        upstream_repo_url: Some("https://github.com/cowok-ai/skills".into()),
        upstream_commit_sha: None,
        cron_schedule: Some("0 */5 * * * *".into()),
        skill_md_path: "SKILL.md".into(),
        plugin_json_path: "plugin.json".into(),
        manifest_json: serde_json::json!({
            "name": "cron-skill",
            "version": "1.0.0",
            "description": "Cron skill"
        })
        .to_string(),
        last_synced_at: None,
        revision: 1,
        staging_version: None,
        staging_manifest_json: None,
        staging_diff_reasons_json: None,
        staging_bundle_dir: None,
        staging_package_sha256: None,
        breaking_change_isolated: false,
        quarantined: false,
        quarantine_reason: None,
        updated_at: 1_700_000_000_000,
    };
    spec_registry::plugin_store::upsert_plugin_record(&pool, &plugin_row, None)
        .expect("upsert plugin");
    let sched = upsert_cron_schedule(
        &pool,
        "tenant-alpha",
        &proj_a,
        plugin_id,
        "0 */5 * * * *",
        "https://github.com/cowok-ai/skills",
        None,
    )
    .expect("upsert sched");

    // Worker 1 claims lease with min TTL 1_000ms -> stale_after_timestamp = 2_000ms
    let claim1 = claim_cron_lease(
        &pool,
        "tenant-alpha",
        &proj_a,
        plugin_id,
        "worker-1",
        false,
        1_000,
        1_000,
    )
    .expect("claim1")
    .expect("lease1");
    assert_eq!(claim1.fencing_token, sched.fencing_token + 1);
    // LeaseReaper expires worker-1 stale lease at 2_500ms and bumps fencing_token
    let reaped = reap_stale_cron_leases(&pool, 2_500).expect("reap");
    assert_eq!(reaped, 1);
    // Worker 2 acquires newer lease with higher fencing_token
    let claim2 = claim_cron_lease(
        &pool,
        "tenant-alpha",
        &proj_a,
        plugin_id,
        "worker-2",
        true,
        2_600,
        30_000,
    )
    .expect("claim2")
    .expect("lease2");
    assert!(claim2.fencing_token > claim1.fencing_token);
    // Zombie worker-1 attempting to commit with stale fencing_token MUST abort (returns Ok(false))
    let zombie_committed = commit_cron_fenced(
        &pool,
        &FencedCronCommit {
            tenant_id: "tenant-alpha",
            project_id: &proj_a,
            plugin_id,
            fencing_token: claim1.fencing_token,
            sync_status: "AUTH_REQUIRED",
            last_etag: None,
            last_commit_sha: None,
            consecutive_failures: 1,
            next_retry_at: None,
            last_error_code: Some("AUTH_REQUIRED"),
            last_error_message: Some("401"),
            updated_at: 2_650,
        },
    )
    .expect("zombie call");
    assert!(!zombie_committed, "stale fencing_token must abort commit");
    // Worker-2 marks AUTH_REQUIRED cleanly; saving GitHub PAT resets AUTH_REQUIRED back to IDLE
    let w2_committed = commit_cron_fenced(
        &pool,
        &FencedCronCommit {
            tenant_id: "tenant-alpha",
            project_id: &proj_a,
            plugin_id,
            fencing_token: claim2.fencing_token,
            sync_status: "AUTH_REQUIRED",
            last_etag: None,
            last_commit_sha: None,
            consecutive_failures: 1,
            next_retry_at: None,
            last_error_code: Some("AUTH_REQUIRED"),
            last_error_message: Some("HTTP 401"),
            updated_at: 2_700,
        },
    )
    .expect("w2");
    assert!(w2_committed, "worker-2 valid fencing_token must commit");
    let conn_pre = pool.connection().unwrap();
    let mid = get_cron_schedule_scoped(&conn_pre, "tenant-alpha", &proj_a, plugin_id)
        .unwrap()
        .unwrap();
    assert_eq!(mid.sync_status, "AUTH_REQUIRED");
    drop(conn_pre);

    vault
        .upsert_secret("github_plugins", "ghp_mock_token_084", None)
        .expect("upsert github pat");
    assert_eq!(
        vault.get_github_sync_token().unwrap().unwrap().as_str(),
        "ghp_mock_token_084"
    );
    let cleared = spec_registry::plugin_store::clear_auth_required_schedules_for_tenant(
        &pool,
        "tenant-alpha",
        3_000,
    )
    .expect("clear auth");
    assert_eq!(cleared, 1);
    let conn = pool.connection().unwrap();
    let after = get_cron_schedule_scoped(&conn, "tenant-alpha", &proj_a, plugin_id)
        .unwrap()
        .unwrap();
    assert_eq!(after.sync_status, "IDLE");

    // 3. Hardened git clone MUST reject `file://` URLs (protocol.file.allow=never + SSRF guard)
    let clone_dest = TempDir::new().unwrap();
    let file_git_res = clone_repo_hardened("file:///tmp/evil-repo", clone_dest.path(), None).await;
    assert!(file_git_res.is_err());

    // Also verify symlink inside git repo fails closed during verified bundle load
    let bare_src = TempDir::new().unwrap();
    let status_init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(bare_src.path())
        .status()
        .expect("git init");
    assert!(status_init.success());
    let link_target = canon_root.join("outside-secret.txt");
    std::fs::write(&link_target, "secret").unwrap();
    std::os::unix::fs::symlink(&link_target, bare_src.path().join("SKILL.md")).unwrap();
    let scan_res = load_bundle_from_dir_verified(bare_src.path(), None);
    assert!(
        scan_res.is_err(),
        "symlink SKILL.md inside cloned repo must fail closed"
    );

    // 4. SSRF redirect hop block (`302 -> http://169.254.169.254/latest/meta-data/` & RFC1918)
    let redirect_app = Router::new().route(
        "/redirect-ssrf",
        get(|| async {
            (
                StatusCode::FOUND,
                [(header::LOCATION, "http://169.254.169.254/latest/meta-data/")],
            )
                .into_response()
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let srv_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, redirect_app).await;
    });
    let ssrf_probe = probe_upstream_with_ssrf_redirects(
        &format!("http://127.0.0.1:{}/redirect-ssrf", local_addr.port()),
        None,
        None,
    )
    .await;
    assert!(matches!(ssrf_probe, UpstreamCheckResult::SsrfBlocked(_)));
    assert!(validate_redirect_target(
        "https://github.com/cowok-ai/skills",
        "http://169.254.169.254/latest/meta-data/"
    )
    .is_err());
    assert!(
        validate_redirect_target("https://github.com/cowok-ai/skills", "http://10.0.0.5/x")
            .is_err()
    );
    srv_handle.abort();

    // 5. End-to-end staged breaking update repackaging -> isolated staging -> HUMAN_ONLY promote -> ACTIVE v2.0.0
    let active_plugin_id = Uuid::new_v4().to_string();
    let base_manifest = plugin_host::SkillManifest {
        name: "staged-promote-skill".into(),
        version: "1.0.0".into(),
        description: "Baseline v1 plugin".into(),
        entry_point: "SKILL.md".into(),
        capabilities_required: vec![],
        risk_tier: 1,
        parameters: None,
        tags: vec![],
        prompt_overview: None,
    };
    let active_row = SkillPluginRow {
        id: active_plugin_id.clone(),
        tenant_id: "tenant-alpha".into(),
        project_id: proj_a.clone(),
        plugin_slug: "staged-promote-skill".into(),
        version: "1.0.0".into(),
        lifecycle_state: "ACTIVE".into(),
        source_type: "github".into(),
        source_repo_url: "https://github.com/cowok-ai/staged-promote-skill".into(),
        upstream_repo_url: Some("https://github.com/cowok-ai/staged-promote-skill".into()),
        upstream_commit_sha: None,
        cron_schedule: None,
        skill_md_path: "SKILL.md".into(),
        plugin_json_path: "plugin.json".into(),
        manifest_json: serde_json::to_string(&base_manifest).unwrap(),
        last_synced_at: None,
        revision: 1,
        staging_version: None,
        staging_manifest_json: None,
        staging_diff_reasons_json: None,
        staging_bundle_dir: None,
        staging_package_sha256: None,
        breaking_change_isolated: false,
        quarantined: false,
        quarantine_reason: None,
        updated_at: 1_700_000_000_000,
    };
    let saved_active =
        spec_registry::plugin_store::upsert_plugin_record(&pool, &active_row, None).unwrap();
    let breaking_bundle = plugin_host::StagedPluginBundle {
        manifest: plugin_host::SkillManifest {
            name: "staged-promote-skill".into(),
            version: "2.0.0".into(),
            description: "Breaking v2 plugin".into(),
            entry_point: "SKILL.md".into(),
            capabilities_required: vec!["net:outbound".into()],
            risk_tier: 3,
            parameters: None,
            tags: vec![],
            prompt_overview: None,
        },
        skill_md: "---\nname: staged-promote-skill\nversion: \"2.0.0\"\nrisk_tier: 3\ncapabilities_required:\n  - \"net:outbound\"\ndescription: \"Breaking v2 plugin\"\n---\n# Body\n"
            .into(),
        scripts: vec![],
    };
    let staged_row = harness_daemon::routes::plugins_cron::apply_synced_bundle_to_plugin(
        &pool,
        &canon_root,
        &saved_active,
        breaking_bundle,
        Some("abc123def"),
    )
    .expect("apply_synced_bundle_to_plugin");
    assert!(staged_row.breaking_change_isolated);
    assert_eq!(staged_row.staging_version.as_deref(), Some("2.0.0"));
    assert!(staged_row.staging_bundle_dir.is_some());

    // Verify persisted staged breaking bundle promotes cleanly via O_NOFOLLOW + skills-lock.json
    let mut managed_promote =
        harness_daemon::routes::plugins_cron::snapshot_from_plugin_row(&staged_row).unwrap();
    let staged_dir_path =
        std::path::PathBuf::from(staged_row.staging_bundle_dir.as_deref().unwrap());
    managed_promote
        .promote_verified_staging_dir(
            &staged_dir_path,
            staged_row.staging_package_sha256.as_deref(),
            staged_row.revision,
        )
        .expect("promote_verified_staged_bundle_from_dir must verify & promote v2.0.0 cleanly");
    assert_eq!(managed_promote.version(), "2.0.0");
    assert!(!managed_promote.breaking_change_isolated());
}
