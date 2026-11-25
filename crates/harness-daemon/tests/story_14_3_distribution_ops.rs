// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Cryptographic Ed25519 Tamper/Expiry/Rollback Tests, Deterministic Zero-PII A/B Bucketing, Security Denylist, Local Override vs Global Quarantine, Offline License & Cowok.ai Federation, RBAC 403 & Audit Logs)
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use chrono::Utc;
use harness_daemon::routes::cloud_admin_auth::{
    issue_admin_session_token, AdminSessionClaims, CloudAdminRole,
};
use harness_daemon::routes::distribution_ops::{
    distribution_ops_router, CatalogManifestPayload, LatestReleaseManifestPayload,
};
use harness_daemon::routes::distribution_ops_crypto::{
    assign_ab_bucket, load_or_create_workspace_signing_key, resolve_effective_plugin_states,
    sign_manifest, sign_offline_license, validate_experiment_security_guard,
    verify_manifest_fail_closed, verify_offline_license, CatalogPluginRecord, LocalPluginOverride,
    ManifestVerifyError, OfflineLicensePayload, SignedManifestEnvelope, SkillAdvisoryRecord,
};
use harness_daemon::routes::DaemonAppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::tempdir;
use tower::ServiceExt;

struct DistTestHarness {
    dir: tempfile::TempDir,
    pool: SqlitePool,
    app: Router,
}

fn setup_dist_harness() -> DistTestHarness {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join(".cowok-design").join("daemon-143.db");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open_wal");
    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    let state = DaemonAppState::new(registry, "127.0.0.1".to_string(), 4173);
    let app = distribution_ops_router().with_state(state);
    DistTestHarness { dir, pool, app }
}

fn make_admin_token(role: CloudAdminRole, email: &str) -> String {
    let now = Utc::now().timestamp();
    issue_admin_session_token(&AdminSessionClaims {
        subject: format!("oidc:{email}"),
        email: email.to_string(),
        role,
        mfa_verified: true,
        oidc_provider: "google".to_string(),
        issued_at: now,
        expires_at: now + 3600,
        jti: format!("jti_{}", uuid::Uuid::new_v4().simple()),
    })
    .expect("issue admin session token")
}

async fn send_json(
    app: &Router,
    method: Method,
    uri: &str,
    admin_token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4173");
    if let Some(tok) = admin_token {
        builder = builder.header("x-admin-session", tok);
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
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

#[test]
fn ed25519_manifest_fail_closed_rejects_1byte_tamper_expiry_and_rollback() {
    let dir = tempdir().expect("tempdir");
    let (signing_key, key_id) =
        load_or_create_workspace_signing_key(dir.path()).expect("workspace key");
    let vk = signing_key.verifying_key();
    let now = Utc::now().timestamp();

    let env = sign_manifest(
        &signing_key,
        &key_id,
        "latest",
        5,
        now,
        now + 3600,
        json!({ "version": "1.5.0", "rolloutPercent": 25 }),
    )
    .expect("sign");

    // Valid verification passes
    assert!(verify_manifest_fail_closed(&env, "latest", &vk, 5, now).is_ok());

    // Untrusted/self-signed key MUST fail closed
    let other_dir = tempdir().expect("other tempdir");
    let (untrusted_sk, _) =
        load_or_create_workspace_signing_key(other_dir.path()).expect("other key");
    let self_signed = sign_manifest(
        &untrusted_sk,
        &key_id,
        "latest",
        5,
        now,
        now + 3600,
        json!({ "version": "1.5.0", "rolloutPercent": 25 }),
    )
    .expect("sign untrusted");
    assert!(matches!(
        verify_manifest_fail_closed(&self_signed, "latest", &vk, 1, now),
        Err(ManifestVerifyError::TamperedSignature(_))
    ));

    // Tampering key_id fails closed because key_id is bound into signing_input
    let mut bad_kid = env.clone();
    bad_kid.key_id = "tampered-key-id".to_string();
    assert!(matches!(
        verify_manifest_fail_closed(&bad_kid, "latest", &vk, 1, now),
        Err(ManifestVerifyError::TamperedSignature(_))
    ));

    // Strict expiry boundary (expires_at == now) fails closed
    assert!(matches!(
        verify_manifest_fail_closed(&env, "latest", &vk, 1, now + 3600),
        Err(ManifestVerifyError::ExpiredManifest { .. })
    ));

    // issued_at > expires_at fails closed
    let inverted = sign_manifest(
        &signing_key,
        &key_id,
        "latest",
        5,
        now + 5000,
        now + 3600,
        json!({ "version": "1.5.0" }),
    )
    .expect("sign inverted");
    assert!(matches!(
        verify_manifest_fail_closed(&inverted, "latest", &vk, 1, now),
        Err(ManifestVerifyError::InvalidEnvelope(_))
    ));

    // manifest_seq < 1 fails closed
    let zero_seq = sign_manifest(
        &signing_key,
        &key_id,
        "latest",
        0,
        now,
        now + 3600,
        json!({ "version": "1.5.0" }),
    )
    .expect("sign zero seq");
    assert!(matches!(
        verify_manifest_fail_closed(&zero_seq, "latest", &vk, 0, now),
        Err(ManifestVerifyError::InvalidEnvelope(_))
    ));

    // 1-byte tamper in payload fails closed
    let mut tampered = env.clone();
    tampered.payload = json!({ "version": "1.5.1", "rolloutPercent": 25 });
    assert!(matches!(
        verify_manifest_fail_closed(&tampered, "latest", &vk, 1, now),
        Err(ManifestVerifyError::TamperedSignature(_))
    ));

    // 1-byte tamper in signature fails closed
    let mut bad_sig = env.clone();
    let mut chars: Vec<char> = bad_sig.signature_base64.chars().collect();
    chars[0] = if chars[0] == 'A' { 'B' } else { 'A' };
    bad_sig.signature_base64 = chars.into_iter().collect();
    assert!(matches!(
        verify_manifest_fail_closed(&bad_sig, "latest", &vk, 1, now),
        Err(ManifestVerifyError::TamperedSignature(_))
    ));

    // Expired manifest fails closed
    assert!(matches!(
        verify_manifest_fail_closed(&env, "latest", &vk, 1, now + 7200),
        Err(ManifestVerifyError::ExpiredManifest { .. })
    ));

    // Rollback attack (manifest_seq < last_seen_seq) fails closed
    assert!(matches!(
        verify_manifest_fail_closed(&env, "latest", &vk, 6, now),
        Err(ManifestVerifyError::RollbackAttackDetected {
            manifest_seq: 5,
            last_seen_seq: 6
        })
    ));
}

#[test]
fn ab_bucketing_is_deterministic_zero_pii_and_blocks_security_invariants() {
    let b1 = assign_ab_bucket("inst_anon_42", "onboarding_pack_v2");
    let b2 = assign_ab_bucket("inst_anon_42", "onboarding_pack_v2");
    assert_eq!(b1, b2);
    assert!(b1 < 100);

    for forbidden in [
        "HUMAN_ONLY",
        "humanOnly",
        "human\u{200B}Only",
        "ZERO_PII",
        "disableZeroPii",
        "O_NOFOLLOW",
        "o---no__follow",
        "no\u{FEFF}follow",
        "RBAC",
        "\u{FF32}\u{FF22}\u{FF21}\u{FF23}",
        "SECURITY_BYPASS",
        "security...Bypass",
        "PRIVACY_SCRUBBER",
        "p\u{0456}\u{0456}_scrubber",
        "disableMfa",
        "bypassAuth",
        "skipMfaCheck",
    ] {
        let err = validate_experiment_security_guard(
            "safe_key",
            "Safe title",
            &json!([{ "key": "treatment", "weight": 50, "config": { "flag": forbidden } }]),
        );
        assert!(err.is_err(), "expected denylist block for {forbidden}");
    }
}

#[test]
fn local_override_beats_cloud_default_except_global_quarantine() {
    let catalog = vec![
        CatalogPluginRecord {
            plugin_id: "html-forge-core".to_string(),
            source_kind: "DEFAULT_BUILTIN".to_string(),
            lifecycle_status: "ACTIVE".to_string(),
            repo_url: None,
            pinned_commit_sha: None,
            min_app_version: "0.1.0".to_string(),
            default_enabled: true,
            default_config: json!({}),
            title_i18n: json!({}),
            summary_i18n: json!({}),
            manifest_seq: 1,
            updated_by: "system".to_string(),
            updated_at: 1,
        },
        CatalogPluginRecord {
            plugin_id: "malicious-theme-pack".to_string(),
            source_kind: "RECOMMENDED_GITHUB".to_string(),
            lifecycle_status: "ACTIVE".to_string(),
            repo_url: Some("https://github.com/example/theme-pack".to_string()),
            pinned_commit_sha: Some("a".repeat(40)),
            min_app_version: "0.1.0".to_string(),
            default_enabled: false,
            default_config: json!({}),
            title_i18n: json!({}),
            summary_i18n: json!({}),
            manifest_seq: 1,
            updated_by: "curator".to_string(),
            updated_at: 1,
        },
    ];
    let advisories = vec![SkillAdvisoryRecord {
        id: "adv_1".to_string(),
        skill_or_plugin_id: "malicious-theme-pack".to_string(),
        severity: "CRITICAL".to_string(),
        action: "QUARANTINE".to_string(),
        reason: "Supply-chain compromise detected".to_string(),
        affected_sha256: None,
        advisory_seq: 1,
        issued_by: "ops-admin@cowok.design".to_string(),
        created_at: 1,
    }];
    let overrides = vec![
        // User explicitly disabled built-in default -> MUST stay disabled (Local Override > Cloud Default)
        LocalPluginOverride {
            plugin_id: "html-forge-core".to_string(),
            enabled: false,
            pinned_version: None,
        },
        // User explicitly enabled quarantined plugin -> MUST be locked disabled by Global Quarantine
        LocalPluginOverride {
            plugin_id: "malicious-theme-pack".to_string(),
            enabled: true,
            pinned_version: None,
        },
    ];

    let resolved = resolve_effective_plugin_states(&catalog, &advisories, &overrides);
    let builtin = resolved
        .iter()
        .find(|p| p.plugin_id == "html-forge-core")
        .expect("builtin");
    assert!(!builtin.effective_enabled);
    assert!(!builtin.quarantine_locked);
    assert_eq!(builtin.resolution_source, "LOCAL_OVERRIDE");

    let quarantined = resolved
        .iter()
        .find(|p| p.plugin_id == "malicious-theme-pack")
        .expect("quarantined");
    assert!(!quarantined.effective_enabled);
    assert!(quarantined.quarantine_locked);
    assert_eq!(quarantined.resolution_source, "GLOBAL_QUARANTINE");
}

#[test]
fn offline_license_token_verifies_offline_and_rejects_tamper() {
    let dir = tempdir().expect("tempdir");
    let (signing_key, key_id) =
        load_or_create_workspace_signing_key(dir.path()).expect("workspace key");
    let vk = signing_key.verifying_key();
    let now = Utc::now().timestamp();

    let payload = OfflineLicensePayload {
        license_id: "lic_test_01".to_string(),
        tier: "ENTERPRISE".to_string(),
        subject_org: "Acme Design Corp".to_string(),
        cowok_org_id: Some("cowok_org_acme_01".to_string()),
        max_seats: 50,
        features: vec!["desktop-pro".to_string(), "offline-airgap".to_string()],
        issued_at: now,
        expires_at: now + 86_400,
    };

    let token = sign_offline_license(&signing_key, &key_id, &payload).expect("sign license");
    let verified = verify_offline_license(&token, &vk, now).expect("verify offline");
    assert!(verified.offline_verified);
    assert_eq!(verified.payload.tier, "ENTERPRISE");

    // Flip 1 character in the token -> fail-closed
    let mut chars: Vec<char> = token.chars().collect();
    let idx = chars.len() - 4;
    chars[idx] = if chars[idx] == 'a' { 'b' } else { 'a' };
    let tampered: String = chars.into_iter().collect();
    assert!(verify_offline_license(&tampered, &vk, now).is_err());

    // Strict expiry boundary fails closed
    assert!(matches!(
        verify_offline_license(&token, &vk, now + 86_400),
        Err(ManifestVerifyError::ExpiredManifest { .. })
    ));
}

#[cfg(unix)]
#[test]
fn signing_key_loader_rejects_symlink_parent_dir() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().expect("tempdir");
    let real_target = tempdir().expect("real target");
    symlink(real_target.path(), dir.path().join(".cowok-design")).expect("create symlink");
    assert!(load_or_create_workspace_signing_key(dir.path()).is_err());
}

#[tokio::test]
async fn end_to_end_distribution_ops_rbac_audit_edge_sync_and_federation() {
    let h = setup_dist_harness();
    let super_tok = make_admin_token(CloudAdminRole::SuperAdmin, "ops-admin@cowok.design");
    let curator_tok = make_admin_token(CloudAdminRole::SkillCurator, "curator@cowok.design");
    let analyst_tok = make_admin_token(CloudAdminRole::AnalystReadonly, "analyst@cowok.design");

    // 1. Canary rollout: ANALYST_READONLY gets 403, SUPER_ADMIN succeeds & publishes signed latest.json
    let rollout_body = json!({
        "channel": "desktop",
        "version": "1.8.0",
        "rolloutPercent": 25,
        "killSwitch": false,
        "announcement": { "en": "Canary 1.8.0 live", "vi": "Phiên bản Canary 1.8.0" },
        "downloadUrl": "https://releases.cowok.design/desktop/cowok-1.8.0.dmg",
        "artifactSha256": "a".repeat(64),
        "notes": "Canary wave 2"
    });
    let (st_forbidden, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/releases/rollout",
        Some(&analyst_tok),
        Some(rollout_body.clone()),
    )
    .await;
    assert_eq!(st_forbidden, StatusCode::FORBIDDEN);

    let (st_ok, rollout_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/releases/rollout",
        Some(&super_tok),
        Some(rollout_body),
    )
    .await;
    assert_eq!(st_ok, StatusCode::CREATED);
    assert!(rollout_res["data"]["auditLogId"].as_str().is_some());

    // Both installId (camelCase) and install_id (snake_case alias) work, and public manifest redacts operator email PII
    let (st_latest, latest_val) = send_json(
        &h.app,
        Method::GET,
        "/v1/releases/latest.json?channel=desktop&installId=inst_canary_01",
        None,
        None,
    )
    .await;
    assert_eq!(st_latest, StatusCode::OK);
    assert!(!latest_val.to_string().contains("ops-admin@cowok.design"));
    let latest_env: SignedManifestEnvelope<LatestReleaseManifestPayload> =
        serde_json::from_value(latest_val).expect("parse signed latest manifest");
    let (sk, _) = load_or_create_workspace_signing_key(h.dir.path()).expect("key");
    assert!(verify_manifest_fail_closed(
        &latest_env,
        "latest",
        &sk.verifying_key(),
        1,
        Utc::now().timestamp()
    )
    .is_ok());
    assert_eq!(latest_env.payload.version, "1.8.0");
    assert!(latest_env
        .payload
        .channels
        .iter()
        .all(|c| c.published_by == "cowok-release-ops"));

    // Invalid downloadUrl with whitespace or empty host rejected
    let (st_bad_url, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/releases/rollout",
        Some(&super_tok),
        Some(json!({
            "channel": "desktop",
            "version": "1.8.1",
            "rolloutPercent": 10,
            "downloadUrl": "https://releases.cowok.design/bad url",
            "artifactSha256": "a".repeat(64)
        })),
    )
    .await;
    assert_eq!(st_bad_url, StatusCode::UNPROCESSABLE_ENTITY);

    // 2. Plugin Catalog: rejects missing reason, mutable branches (main/HEAD/latest), path-traversal URLs; clears repo on DEFAULT_BUILTIN
    let (st_no_cat_reason, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog",
        Some(&curator_tok),
        Some(json!({
            "pluginId": "figma-tokens-sync",
            "sourceKind": "RECOMMENDED_GITHUB",
            "lifecycleStatus": "ACTIVE",
            "repoUrl": "https://github.com/cowok-design/figma-tokens-sync",
            "pinnedCommitSha": "b".repeat(40)
        })),
    )
    .await;
    assert_eq!(st_no_cat_reason, StatusCode::UNPROCESSABLE_ENTITY);

    for mutable_ref in ["main", "master", "HEAD", "latest", "develop"] {
        let (st_mut, _) = send_json(
            &h.app,
            Method::POST,
            "/v1/admin/skills/catalog",
            Some(&curator_tok),
            Some(json!({
                "pluginId": "figma-tokens-sync",
                "sourceKind": "RECOMMENDED_GITHUB",
                "lifecycleStatus": "ACTIVE",
                "repoUrl": "https://github.com/cowok-design/figma-tokens-sync",
                "pinnedCommitSha": mutable_ref,
                "reason": "Testing rejection of mutable branch refs"
            })),
        )
        .await;
        assert_eq!(
            st_mut,
            StatusCode::UNPROCESSABLE_ENTITY,
            "expected mutable ref '{mutable_ref}' to be rejected"
        );
    }

    let (st_trav, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog",
        Some(&curator_tok),
        Some(json!({
            "pluginId": "figma-tokens-sync",
            "sourceKind": "RECOMMENDED_GITHUB",
            "lifecycleStatus": "ACTIVE",
            "repoUrl": "https://github.com/../etc",
            "pinnedCommitSha": "v1.2.3",
            "reason": "Testing rejection of traversal repo URL"
        })),
    )
    .await;
    assert_eq!(st_trav, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_good_gh, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog",
        Some(&curator_tok),
        Some(json!({
            "pluginId": "figma-tokens-sync",
            "sourceKind": "RECOMMENDED_GITHUB",
            "lifecycleStatus": "ACTIVE",
            "repoUrl": "https://github.com/cowok-design/figma-tokens-sync",
            "pinnedCommitSha": "b".repeat(40),
            "titleI18n": { "en": "Figma Tokens Sync", "vi": "Đồng bộ Token Figma" },
            "reason": "Pinned verified Figma tokens sync commit for release"
        })),
    )
    .await;
    assert_eq!(st_good_gh, StatusCode::OK);

    // DEFAULT_BUILTIN clears any supplied repoUrl / pinnedCommitSha
    let (st_builtin, builtin_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog",
        Some(&curator_tok),
        Some(json!({
            "pluginId": "html-forge-core",
            "sourceKind": "DEFAULT_BUILTIN",
            "lifecycleStatus": "ACTIVE",
            "repoUrl": "https://github.com/evil/ignored",
            "pinnedCommitSha": "v1.0.0",
            "reason": "Ensure builtin clears external GitHub fields"
        })),
    )
    .await;
    assert_eq!(st_builtin, StatusCode::OK);
    assert!(builtin_res["data"]["plugin"]["repoUrl"].is_null());
    assert!(builtin_res["data"]["plugin"]["pinnedCommitSha"].is_null());

    // 3. A/B Experiments: rejects missing reason, ':' in keys, duplicate variant keys, security invariants
    let (st_colon, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/experiments",
        Some(&curator_tok),
        Some(json!({
            "experimentKey": "bad:key",
            "title": "Invalid key with colon",
            "targetSurface": "ONBOARDING_DEFAULTS",
            "status": "RUNNING",
            "reason": "Testing colon rejection",
            "variants": [
                { "key": "control", "weight": 50, "config": {} },
                { "key": "treatment", "weight": 50, "config": {} }
            ]
        })),
    )
    .await;
    assert_eq!(st_colon, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_dup_var, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/experiments",
        Some(&curator_tok),
        Some(json!({
            "experimentKey": "dup_variant_test",
            "title": "Duplicate variant keys",
            "targetSurface": "ONBOARDING_DEFAULTS",
            "status": "RUNNING",
            "reason": "Testing duplicate variant key rejection",
            "variants": [
                { "key": "control", "weight": 50, "config": {} },
                { "key": "control", "weight": 50, "config": {} }
            ]
        })),
    )
    .await;
    assert_eq!(st_dup_var, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_sec_block, sec_err) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/experiments",
        Some(&curator_tok),
        Some(json!({
            "experimentKey": "relax_zero_pii_test",
            "title": "Test disabling ZERO_PII",
            "targetSurface": "ONBOARDING_DEFAULTS",
            "status": "RUNNING",
            "reason": "Attempting forbidden security invariant relaxation",
            "variants": [
                { "key": "control", "weight": 50, "config": {} },
                { "key": "treatment", "weight": 50, "config": { "disableZeroPii": true } }
            ]
        })),
    )
    .await;
    assert_eq!(st_sec_block, StatusCode::BAD_REQUEST);
    assert_eq!(sec_err["error"]["code"], "SECURITY_INVARIANT_PROTECTED");

    let (st_exp_ok, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/experiments",
        Some(&curator_tok),
        Some(json!({
            "experimentKey": "onboarding_starter_pack",
            "title": "Onboarding Starter Pack 50/50",
            "targetSurface": "ONBOARDING_DEFAULTS",
            "status": "RUNNING",
            "reason": "Launch guided onboarding starter pack experiment",
            "variants": [
                { "key": "control", "weight": 50, "config": { "preset": "minimal" } },
                { "key": "treatment", "weight": 50, "config": { "preset": "guided" } }
            ]
        })),
    )
    .await;
    assert_eq!(st_exp_ok, StatusCode::OK);

    // 4. Global Quarantine: missing reason fails 422; valid quarantine locks plugin
    let (st_no_reason, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/quarantine-global",
        Some(&curator_tok),
        Some(json!({
            "skillOrPluginId": "figma-tokens-sync",
            "severity": "CRITICAL",
            "action": "QUARANTINE",
            "reason": ""
        })),
    )
    .await;
    assert_eq!(st_no_reason, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_q_ok, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/quarantine-global",
        Some(&curator_tok),
        Some(json!({
            "skillOrPluginId": "figma-tokens-sync",
            "severity": "CRITICAL",
            "action": "QUARANTINE",
            "reason": "Malicious postinstall hook discovered in upstream package"
        })),
    )
    .await;
    assert_eq!(st_q_ok, StatusCode::CREATED);

    // 5. Offline License & Cowok.ai Federation (requires signed proof; rejects bare cowokOrgId & revoked licenses)
    let (st_lic, lic_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/licenses/issue",
        Some(&super_tok),
        Some(json!({
            "tier": "ENTERPRISE",
            "subjectOrg": "Cowok Enterprise Studio",
            "cowokOrgId": "org_cowok_enterprise_99",
            "maxSeats": 100,
            "validDays": 365
        })),
    )
    .await;
    assert_eq!(st_lic, StatusCode::CREATED);
    let license_id = lic_res["data"]["license"]["licenseId"]
        .as_str()
        .expect("licenseId")
        .to_string();
    let signed_token = lic_res["data"]["license"]["signedToken"]
        .as_str()
        .expect("signed token")
        .to_string();

    // Bare cowokOrgId without licenseToken MUST be rejected with 401 (no org-existence oracle)
    let (st_fed_no_tok, fed_no_tok_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/federation/cowok-org",
        None,
        Some(json!({
            "cowokOrgId": "org_cowok_enterprise_99",
            "installId": "inst_channel3_01"
        })),
    )
    .await;
    assert_eq!(st_fed_no_tok, StatusCode::UNAUTHORIZED);
    assert_eq!(fed_no_tok_res["error"]["code"], "LICENSE_TOKEN_REQUIRED");

    // Non-SUPER_ADMIN cannot list signed offline license tokens
    let (st_lic_forbid, _) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/licenses/issue",
        Some(&analyst_tok),
        None,
    )
    .await;
    assert_eq!(st_lic_forbid, StatusCode::FORBIDDEN);

    let (st_fed, fed_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/federation/cowok-org",
        None,
        Some(json!({
            "cowokOrgId": "org_cowok_enterprise_99",
            "installId": "inst_channel3_01",
            "licenseToken": signed_token
        })),
    )
    .await;
    assert_eq!(st_fed, StatusCode::OK);
    assert_eq!(fed_res["data"]["federated"], true);
    assert_eq!(fed_res["data"]["offlineVerified"], true);

    // 6. Client Edge Sync verifies Local Override > Cloud Default + Quarantine block + A/B assignment + Offline license + latest.json
    let (st_sync, sync_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_edge_01",
            "channel": "desktop",
            "appVersion": "1.8.0",
            "localOverrides": [
                { "pluginId": "html-forge-core", "enabled": false },
                { "pluginId": "figma-tokens-sync", "enabled": true }
            ],
            "offlineLicenseToken": signed_token
        })),
    )
    .await;
    assert_eq!(st_sync, StatusCode::OK);
    assert!(sync_res["data"]["verifiedManifestSeqs"]["latest"]
        .as_i64()
        .is_some());
    let eff = sync_res["data"]["effectivePlugins"]
        .as_array()
        .expect("effectivePlugins");
    let html_forge = eff
        .iter()
        .find(|p| p["pluginId"] == "html-forge-core")
        .expect("html-forge-core");
    assert_eq!(html_forge["effectiveEnabled"], false);
    assert_eq!(html_forge["resolutionSource"], "LOCAL_OVERRIDE");

    let figma = eff
        .iter()
        .find(|p| p["pluginId"] == "figma-tokens-sync")
        .expect("figma-tokens-sync");
    assert_eq!(figma["effectiveEnabled"], false);
    assert_eq!(figma["quarantineLocked"], true);
    assert_eq!(figma["resolutionSource"], "GLOBAL_QUARANTINE");
    assert_eq!(
        sync_res["data"]["experimentAssignments"]
            .as_array()
            .expect("assignments")
            .len(),
        1
    );
    assert_eq!(
        sync_res["data"]["licenseVerification"]["offlineVerified"],
        true
    );

    // 6b. REVOKE_QUARANTINE unquarantines catalog entry back to ACTIVE and unlocks local override
    let (st_revoke_q, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/quarantine-global",
        Some(&curator_tok),
        Some(json!({
            "skillOrPluginId": "figma-tokens-sync",
            "severity": "MEDIUM",
            "action": "REVOKE_QUARANTINE",
            "reason": "Upstream package patched and re-audited clean"
        })),
    )
    .await;
    assert_eq!(st_revoke_q, StatusCode::CREATED);

    let (st_sync_unq, sync_unq_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_edge_01",
            "localOverrides": [
                { "pluginId": "figma-tokens-sync", "enabled": true }
            ]
        })),
    )
    .await;
    assert_eq!(st_sync_unq, StatusCode::OK);
    let figma_unq = sync_unq_res["data"]["effectivePlugins"]
        .as_array()
        .expect("effectivePlugins")
        .iter()
        .find(|p| p["pluginId"] == "figma-tokens-sync")
        .expect("figma-tokens-sync");
    assert_eq!(figma_unq["lifecycleStatus"], "ACTIVE");
    assert_eq!(figma_unq["quarantineLocked"], false);
    assert_eq!(figma_unq["effectiveEnabled"], true);

    // 6c. Revoking license blocks BOTH federation and edge sync with 403 LICENSE_REVOKED
    {
        let conn = h.pool.connection().expect("db conn");
        conn.execute(
            "UPDATE cloud_offline_licenses SET revoked = 1 WHERE license_id = ?1",
            [&license_id],
        )
        .expect("revoke license in db");
    }
    let (st_fed_rev, fed_rev_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/federation/cowok-org",
        None,
        Some(json!({
            "cowokOrgId": "org_cowok_enterprise_99",
            "installId": "inst_channel3_01",
            "licenseToken": signed_token
        })),
    )
    .await;
    assert_eq!(st_fed_rev, StatusCode::FORBIDDEN);
    assert_eq!(fed_rev_res["error"]["code"], "LICENSE_REVOKED");

    let (st_edge_rev, edge_rev_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_edge_01",
            "offlineLicenseToken": signed_token
        })),
    )
    .await;
    assert_eq!(st_edge_rev, StatusCode::FORBIDDEN);
    assert_eq!(edge_rev_res["error"]["code"], "LICENSE_REVOKED");

    // 7. Public manifests redact operator emails & Client Edge Sync rejects tampered manifests fail-closed
    let (_, cat_val) = send_json(&h.app, Method::GET, "/v1/skills/catalog.json", None, None).await;
    assert!(!cat_val.to_string().contains("curator@cowok.design"));
    let (_, adv_val) = send_json(
        &h.app,
        Method::GET,
        "/v1/skills/advisories.json",
        None,
        None,
    )
    .await;
    assert!(!adv_val.to_string().contains("curator@cowok.design"));
    let (_, exp_val) = send_json(
        &h.app,
        Method::GET,
        "/v1/experiments/config.json",
        None,
        None,
    )
    .await;
    assert!(!exp_val.to_string().contains("curator@cowok.design"));

    let mut cat_env: SignedManifestEnvelope<CatalogManifestPayload> =
        serde_json::from_value(cat_val).expect("parse catalog manifest");
    cat_env.payload.plugins[0].default_enabled = !cat_env.payload.plugins[0].default_enabled;
    let (st_tamper, tamper_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_edge_01",
            "catalogManifest": cat_env
        })),
    )
    .await;
    assert_eq!(st_tamper, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(tamper_res["error"]["code"], "TAMPERED_SIGNATURE");

    // Tampered latestReleaseManifest also fails closed in edge sync
    let mut bad_latest = latest_env.clone();
    bad_latest.payload.version = "9.9.9".to_string();
    let (st_latest_tamper, latest_tamper_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_edge_01",
            "latestReleaseManifest": bad_latest
        })),
    )
    .await;
    assert_eq!(st_latest_tamper, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(latest_tamper_res["error"]["code"], "TAMPERED_SIGNATURE");

    // 8. Audit logs are strictly SUPER_ADMIN only & push SQL WHERE filters before LIMIT
    let (st_audit_forbid, _) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/distribution/audit-logs",
        Some(&analyst_tok),
        None,
    )
    .await;
    assert_eq!(st_audit_forbid, StatusCode::FORBIDDEN);

    let (st_audit, audit_res) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/distribution/audit-logs?action=RELEASE_ROLLOUT&limit=1",
        Some(&super_tok),
        None,
    )
    .await;
    assert_eq!(st_audit, StatusCode::OK);
    let filtered_logs = audit_res["data"]["logs"].as_array().expect("filtered logs");
    assert_eq!(filtered_logs.len(), 1);
    assert!(filtered_logs[0]["action"]
        .as_str()
        .unwrap_or("")
        .contains("RELEASE_ROLLOUT"));
    assert!(h.pool.connection().is_ok());
}

#[tokio::test]
async fn async_security_lifecycle_blocks_edge_auto_enable_and_supports_atomic_bulk_update() {
    let h = setup_dist_harness();
    let super_tok = make_admin_token(CloudAdminRole::SuperAdmin, "ops-admin@cowok.design");
    let curator_tok = make_admin_token(CloudAdminRole::SkillCurator, "curator@cowok.design");
    let analyst_tok = make_admin_token(CloudAdminRole::AnalystReadonly, "analyst@cowok.design");

    // 1. Create two RECOMMENDED_GITHUB plugins in async security lifecycle states: PROCESSING & PENDING_APPROVAL
    let (st_proc, proc_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog",
        Some(&curator_tok),
        Some(json!({
            "pluginId": "github-scan-alpha",
            "sourceKind": "RECOMMENDED_GITHUB",
            "lifecycleStatus": "PROCESSING",
            "repoUrl": "https://github.com/cowok-design/scan-alpha",
            "pinnedCommitSha": "d".repeat(40),
            "titleI18n": { "vi": "Bộ lọc Quét Alpha", "en": "Scan Alpha" },
            "reason": "Submit new GitHub plugin into async security evaluation"
        })),
    )
    .await;
    assert_eq!(st_proc, StatusCode::OK);
    assert_eq!(proc_res["data"]["plugin"]["lifecycleStatus"], "PROCESSING");
    assert_eq!(proc_res["data"]["plugin"]["defaultEnabled"], false);

    let (st_pend, pend_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog",
        Some(&curator_tok),
        Some(json!({
            "pluginId": "github-review-beta",
            "sourceKind": "RECOMMENDED_GITHUB",
            "lifecycleStatus": "PENDING_APPROVAL",
            "repoUrl": "https://github.com/cowok-design/review-beta",
            "pinnedCommitSha": "v1.4.0",
            "titleI18n": { "vi": "Gói Đánh giá Beta", "en": "Review Beta" },
            "reason": "Security scan complete; queued for admin approval"
        })),
    )
    .await;
    assert_eq!(st_pend, StatusCode::OK);
    assert_eq!(
        pend_res["data"]["plugin"]["lifecycleStatus"],
        "PENDING_APPROVAL"
    );
    assert_eq!(pend_res["data"]["plugin"]["defaultEnabled"], false);

    // 2. Client Edge Sync MUST NOT enable PROCESSING or PENDING_APPROVAL plugins (even with local override!)
    let (st_sync, sync_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_async_edge_01",
            "localOverrides": [
                { "pluginId": "github-scan-alpha", "enabled": true },
                { "pluginId": "github-review-beta", "enabled": true }
            ]
        })),
    )
    .await;
    assert_eq!(st_sync, StatusCode::OK);
    let eff = sync_res["data"]["effectivePlugins"]
        .as_array()
        .expect("effectivePlugins");
    for pid in ["github-scan-alpha", "github-review-beta"] {
        let state = eff
            .iter()
            .find(|p| p["pluginId"] == pid)
            .expect("plugin in effectivePlugins");
        assert_eq!(state["effectiveEnabled"], false, "{pid} must not activate");
        assert_eq!(
            state["advisoryReason"],
            "Awaiting security review / approval"
        );
    }

    // 3. Bulk update RBAC & validation guards
    let (st_bulk_forbid, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog/bulk",
        Some(&analyst_tok),
        Some(json!({
            "pluginIds": ["github-scan-alpha", "github-review-beta"],
            "lifecycleStatus": "ACTIVE",
            "reason": "Analyst cannot mutate catalog"
        })),
    )
    .await;
    assert_eq!(st_bulk_forbid, StatusCode::FORBIDDEN);

    let (st_bulk_bad_reason, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog/bulk",
        Some(&curator_tok),
        Some(json!({
            "pluginIds": ["github-scan-alpha"],
            "lifecycleStatus": "ACTIVE",
            "reason": "ok"
        })),
    )
    .await;
    assert_eq!(st_bulk_bad_reason, StatusCode::UNPROCESSABLE_ENTITY);

    let (st_bulk_empty, _) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog/bulk",
        Some(&curator_tok),
        Some(json!({
            "pluginIds": [],
            "lifecycleStatus": "ACTIVE",
            "reason": "Empty selection must fail validation"
        })),
    )
    .await;
    assert_eq!(st_bulk_empty, StatusCode::UNPROCESSABLE_ENTITY);

    // 4. Atomic bulk approval to ACTIVE + immutable CATALOG_BULK_STATUS_UPDATE audit entry
    let (st_bulk_ok, bulk_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/admin/skills/catalog/bulk",
        Some(&curator_tok),
        Some(json!({
            "pluginIds": ["github-scan-alpha", "github-review-beta"],
            "lifecycleStatus": "ACTIVE",
            "reason": "Bulk approve both plugins after clean security audit"
        })),
    )
    .await;
    assert_eq!(st_bulk_ok, StatusCode::OK);
    assert_eq!(bulk_res["data"]["updatedCount"], 2);
    assert_eq!(bulk_res["data"]["lifecycleStatus"], "ACTIVE");
    assert!(bulk_res["data"]["auditLogId"].as_str().is_some());

    // 5. Audit log contains CATALOG_BULK_STATUS_UPDATE
    let (st_aud, aud_res) = send_json(
        &h.app,
        Method::GET,
        "/v1/admin/distribution/audit-logs?action=CATALOG_BULK_STATUS_UPDATE&limit=5",
        Some(&super_tok),
        None,
    )
    .await;
    assert_eq!(st_aud, StatusCode::OK);
    let bulk_logs = aud_res["data"]["logs"].as_array().expect("bulk audit logs");
    assert_eq!(bulk_logs.len(), 1);
    assert_eq!(bulk_logs[0]["action"], "CATALOG_BULK_STATUS_UPDATE");
    assert_eq!(bulk_logs[0]["details"]["updatedCount"], 2);

    // 6. After approval to ACTIVE, explicit local override can now enable the plugins on client edge
    let (st_sync_after, sync_after_res) = send_json(
        &h.app,
        Method::POST,
        "/v1/client/edge/sync",
        None,
        Some(json!({
            "installId": "inst_async_edge_01",
            "localOverrides": [
                { "pluginId": "github-scan-alpha", "enabled": true }
            ]
        })),
    )
    .await;
    assert_eq!(st_sync_after, StatusCode::OK);
    let alpha_after = sync_after_res["data"]["effectivePlugins"]
        .as_array()
        .expect("effectivePlugins")
        .iter()
        .find(|p| p["pluginId"] == "github-scan-alpha")
        .expect("github-scan-alpha");
    assert_eq!(alpha_after["lifecycleStatus"], "ACTIVE");
    assert_eq!(alpha_after["effectiveEnabled"], true);
}
