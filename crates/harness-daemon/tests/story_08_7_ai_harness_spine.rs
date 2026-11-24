// @story 08.7
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Zero-Hardcode AI Harness Spine Integration Verification)
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use bot_fleet::{
    build_gemini_cached_content_create_payload, build_provider_wire_payload, builtin_seed_models,
    default_model_for_provider, resolve_cache_strategy, resolve_model_for_provider, ByokCredential,
    ByokGatewayRouter, CacheStrategy, CooldownTracker, GeminiCachedContentCreateRequest,
    ModelRegistryEntry, ModelTier, PromptCacheHints, DEFAULT_GEMINI_CACHE_TTL_SECS,
};
use chrono::Utc;
use harness_daemon::build_router;
use harness_daemon::daemon_auth::FlowPrincipal;
use harness_daemon::routes::learn_studio_corrections::{
    record_studio_correction_in_db, RecordStudioCorrectionRequest, StudioCorrectionError,
    StudioCorrectionKind,
};
use harness_daemon::routes::models_registry::load_model_registry_from_conn;
use harness_daemon::routes::orchestrator::compile_orchestrator_turn_prompt;
use harness_daemon::routes::skills_catalog::invalidate_catalog_cache_for_tenant;
use harness_daemon::routes::DaemonAppState;
use http_body_util::BodyExt;
use plugin_host::{SkillSourceBucket, UnifiedCatalogError, UnifiedSkillCatalog};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use spec_registry::skill_router::{
    compile_two_breakpoint_prompt, ConversationTurn, TwoBreakpointPromptInput,
};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;
use zeroize::Zeroizing;

const V24_SQL: &str = include_str!(
    "../../spec-registry/migrations/V24__story_08_7_model_registry_and_studio_corrections.sql"
);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("harness-daemon resides in crates/harness-daemon")
        .to_path_buf()
}

fn setup_pool_with_v24() -> (TempDir, SqlitePool, String, String) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("story_08_7.sqlite");
    let pool =
        SqlitePool::open_wal(SqliteWalConfig::new(&db_path, dir.path())).expect("open WAL pool");
    let conn = pool.connection().expect("conn");
    conn.execute_batch(V24_SQL)
        .expect("apply V24 additive migration");
    let project_a = "00000000-0000-4000-8000-000000000870".to_string();
    let project_b = "00000000-0000-4000-8000-000000000871".to_string();
    conn.execute(
        "INSERT INTO projects (id, tenant_id, name, slug, workspace_root, product_brief, status, created_at, updated_at) \
         VALUES (?1, 'tenant-087', 'Story 08.7 Harness A', 'story-087-a', '.', 'Brief A', 'ACTIVE', 1763960000000, 1763960000000)",
        params![project_a],
    )
    .expect("seed project a");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, name, slug, workspace_root, product_brief, status, created_at, updated_at) \
         VALUES (?1, 'tenant-087', 'Story 08.7 Harness B', 'story-087-b', '.', 'Brief B', 'ACTIVE', 1763960000000, 1763960000000)",
        params![project_b],
    )
    .expect("seed project b");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, name, slug, workspace_root, product_brief, status, created_at, updated_at) \
         VALUES ('00000000-0000-4000-8000-000000000872', 'tenant-other', 'Other Tenant Project', 'other-087', '.', 'Brief', 'ACTIVE', 1763960000000, 1763960000000)",
        [],
    )
    .expect("seed other tenant project");
    (dir, pool, project_a, project_b)
}

async fn send_json(
    router: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value) {
    let raw = body
        .map(|v| serde_json::to_vec(&v).unwrap())
        .unwrap_or_default();
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4170");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    if !raw.is_empty() {
        b = b.header("content-type", "application/json");
    }
    let resp = router
        .clone()
        .oneshot(b.body(Body::from(raw)).unwrap())
        .await
        .expect("oneshot");
    let st = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn ac01_and_ac05_unified_catalog_4_sources_reload_delta_vi_en_routing_and_http_guards() {
    let (_dir, pool, project_id, _project_b) = setup_pool_with_v24();
    let conn = pool.connection().expect("conn");

    // 1. Initial discovery across disk 3 buckets + SQLite (0 plugins / 0 cards yet)
    let cat_a =
        UnifiedSkillCatalog::discover(&repo_root(), Some(&conn), "tenant-087", Some(&project_id));
    let snap_a = cat_a.snapshot();
    assert!(
        *snap_a
            .bucket_counts
            .get(&SkillSourceBucket::HostSkills)
            .unwrap_or(&0)
            >= 3
    );
    assert!(
        *snap_a
            .bucket_counts
            .get(&SkillSourceBucket::DesignCraft)
            .unwrap_or(&0)
            >= 20
    );
    assert!(
        *snap_a
            .bucket_counts
            .get(&SkillSourceBucket::UpstreamVerbatim)
            .unwrap_or(&0)
            >= 1
    );
    assert_eq!(
        *snap_a
            .bucket_counts
            .get(&SkillSourceBucket::SqlitePlugin)
            .unwrap_or(&0),
        0
    );
    assert_eq!(
        *snap_a
            .bucket_counts
            .get(&SkillSourceBucket::LearnCard)
            .unwrap_or(&0),
        0
    );

    // 2. Insert an ACTIVE plugin + an ACCEPTED HUMAN_ONLY LEARN card -> reload shifts catalog_sha256 cleanly
    conn.execute(
        "INSERT INTO skill_plugins (id, tenant_id, project_id, plugin_slug, version, lifecycle_state, source_type, source_repo_url, manifest_json, updated_at) \
         VALUES ('plg-087', 'tenant-087', ?1, 'fintech-ledger-charts', '1.0.0', 'ACTIVE', 'local', 'local://fintech', '{\"description\":\"Fintech ledger reconciliation charts\",\"tags\":[\"fintech\",\"chart\"]}', 1763960000001)",
        params![project_id],
    )
    .expect("insert active plugin");
    conn.execute(
        "INSERT INTO knowledge_cards (id, tenant_id, project_id, category, title, content, content_sha256, source_evidence, status, created_at) \
         VALUES ('kc-087', 'tenant-087', ?1, 'RULE', 'Always label delta percentage', 'Include explicit delta % on executive bars.', ?2, 'bridge1', 'ACCEPTED', 1763960000002)",
        params![project_id, "a".repeat(64)],
    )
    .expect("insert accepted learn card");

    let cat_b =
        UnifiedSkillCatalog::discover(&repo_root(), Some(&conn), "tenant-087", Some(&project_id));
    let snap_b = cat_b.snapshot();
    assert_eq!(
        *snap_b
            .bucket_counts
            .get(&SkillSourceBucket::SqlitePlugin)
            .unwrap_or(&0),
        1
    );
    assert_eq!(
        *snap_b
            .bucket_counts
            .get(&SkillSourceBucket::LearnCard)
            .unwrap_or(&0),
        1
    );
    assert_ne!(snap_a.catalog_sha256, snap_b.catalog_sha256);
    assert_ne!(snap_a.tier1_index_sha256, snap_b.tier1_index_sha256);

    // 3. Vietnamese + English multilingual routing
    let vi_matches =
        cat_b.route_skills_multilingual("Vẽ biểu đồ kể chuyện dữ liệu doanh thu CEO", 5);
    assert!(vi_matches
        .iter()
        .any(|m| m.skill_id == "data-viz-storyteller"));

    let en_matches = cat_b.route_skills_multilingual("Build honest operational KPI dashboard", 5);
    assert!(en_matches
        .iter()
        .any(|m| m.skill_id == "dashboard-design" || m.skill_id == "dashboard-ux"));

    // 4. Tier-3 lazy reference loading + path-traversal jail
    let ref_body = cat_b
        .load_tier3_reference("data-viz-storyteller", "references/html-canvas-contract.md")
        .expect("lazy load html-canvas-contract.md");
    assert!(ref_body.contains("data-ff-role"));
    assert!(matches!(
        cat_b.load_tier3_reference("data-viz-storyteller", "../../plugin.json"),
        Err(UnifiedCatalogError::PathTraversal(_))
    ));
    drop(conn);

    // 5. HTTP checks: GET /api/v1/skills/catalog project ownership (404 cross-tenant) + POST /api/v1/skills/route dynamic VI routing
    let registry = Arc::new(SpecRegistry::new(pool));
    let state = DaemonAppState::new(registry, "127.0.0.1".into(), 4170);
    let sess_tok = format!("sess-087-{}", Uuid::new_v4());
    state.auth.insert_session(
        sess_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-087".into(),
            user_id: "designer-087".into(),
            role: "editor".into(),
        },
        Utc::now().timestamp() + 3600,
    );
    invalidate_catalog_cache_for_tenant("tenant-087");
    let router = build_router(state);

    let (st_cross, _) = send_json(
        &router,
        "GET",
        "/api/v1/skills/catalog?projectId=00000000-0000-4000-8000-000000000872",
        &[("x-cowok-session-token", &sess_tok)],
        None,
    )
    .await;
    assert_eq!(st_cross, StatusCode::NOT_FOUND);

    let (st_ok, body_cat) = send_json(
        &router,
        "GET",
        &format!("/api/v1/skills/catalog?projectId={project_id}"),
        &[("x-cowok-session-token", &sess_tok)],
        None,
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert!(body_cat["data"]["totalSkills"].as_u64().unwrap() >= 25);

    // POST /api/v1/skills/route now dynamically routes VI intents & arbitrary skills (no longer limited to static 7-item array!)
    let (st_route_vi, body_route_vi) = send_json(
        &router,
        "POST",
        "/api/v1/skills/route",
        &[("x-cowok-session-token", &sess_tok)],
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "intentSummary": "Vẽ biểu đồ kể chuyện dữ liệu doanh thu cho CEO",
            "maxSpineTokens": 250
        })),
    )
    .await;
    assert_eq!(
        st_route_vi,
        StatusCode::OK,
        "VI route failed: {body_route_vi}"
    );
    let slugs: Vec<&str> = body_route_vi["data"]["selectedSlugs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        slugs.contains(&"data-viz-storyteller"),
        "POST /api/v1/skills/route must route VI intent to data-viz-storyteller, got {slugs:?}"
    );

    let (st_route_explicit, body_route_explicit) = send_json(
        &router,
        "POST",
        "/api/v1/skills/route",
        &[("x-cowok-session-token", &sess_tok)],
        Some(json!({
            "projectId": project_id,
            "portalId": "DeveloperPortal",
            "intentSummary": "Render executive chart and dashboard",
            "requestedSlugs": ["data-viz-storyteller", "dashboard-design", "plugin:fintech-ledger-charts"],
            "maxSpineTokens": 250
        })),
    )
    .await;
    assert_eq!(
        st_route_explicit,
        StatusCode::OK,
        "explicit dynamic slugs failed: {body_route_explicit}"
    );
    assert_eq!(
        body_route_explicit["data"]["selectedSlugs"],
        json!([
            "dashboard-design",
            "data-viz-storyteller",
            "plugin:fintech-ledger-charts"
        ])
    );
}

#[test]
fn ac02_zone1_prefix_sha256_is_100_percent_invariant_across_three_distinct_turns_and_orchestrator()
{
    let (_dir, pool, project_id, _project_b) = setup_pool_with_v24();
    let registry = Arc::new(SpecRegistry::new(pool));
    let state = DaemonAppState::new(registry, "127.0.0.1".into(), 0);

    let (csha1, p1) = compile_orchestrator_turn_prompt(
        &state,
        "tenant-087",
        &project_id,
        "Tạo biểu đồ kể chuyện dữ liệu doanh thu cho CEO",
        vec![],
    );
    let (csha2, p2) = compile_orchestrator_turn_prompt(
        &state,
        "tenant-087",
        &project_id,
        "Design operational KPI dashboard with filters",
        vec![ConversationTurn {
            role: "user".into(),
            content: "Turn 1 message".into(),
        }],
    );
    let (csha3, p3) = compile_orchestrator_turn_prompt(
        &state,
        "tenant-087",
        &project_id,
        "Soạn slide thuyết trình 16:9 với hệ thống màu sắc OKLCH",
        vec![],
    );

    assert_eq!(csha1, csha2);
    assert_eq!(csha2, csha3);
    assert_eq!(p1.zone1_prefix_sha256, p2.zone1_prefix_sha256);
    assert_eq!(p2.zone1_prefix_sha256, p3.zone1_prefix_sha256);
    assert!(p1.zone1_static_prefix.contains("de-tokens-oklch"));
    assert_ne!(p1.zone2_turn_tail, p2.zone2_turn_tail);
    assert_ne!(p2.zone2_turn_tail, p3.zone2_turn_tail);
    assert!(p1
        .activated_skills
        .iter()
        .any(|s| s == "data-viz-storyteller"));

    // Verify 75% cache-safe compaction directly
    let long_history: Vec<ConversationTurn> = (0..18)
        .map(|i| ConversationTurn {
            role: if i % 2 == 0 { "user" } else { "assistant" }.into(),
            content: format!("Turn {i}: {}", "analytical context payload ".repeat(35)),
        })
        .collect();
    let compacted = compile_two_breakpoint_prompt(&TwoBreakpointPromptInput {
        system_identity: "cowok-host-spine".into(),
        tool_schemas_json: "[]".into(),
        tier1_skill_index: "- data-viz-storyteller".into(),
        design_tokens_sha256: "1".repeat(64),
        catalog_sha256: "2".repeat(64),
        jit_tier2_skills: vec![],
        jit_tier3_references: vec![],
        conversation_history: long_history,
        user_turn: "Summarize".into(),
        context_window_tokens: 2048,
    });
    assert!(compacted.compaction_applied);
    assert!(compacted.zone2_turn_tail.contains("Turn 0:"));
}

#[tokio::test]
async fn ac03_and_ac05_dynamic_model_registry_tenant_overrides_and_wire_native_provider_cache() {
    let (_dir, pool, _project_id, _project_b) = setup_pool_with_v24();
    let conn = pool.connection().expect("conn");

    // 1. Verify V24 seeded all 6 canonical providers and default_model_for_provider delegates cleanly
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM model_registry_profiles", [], |r| {
            r.get(0)
        })
        .expect("count model_registry_profiles");
    assert_eq!(count, 6);
    assert_eq!(default_model_for_provider("claude"), "claude-sonnet-4-5");
    assert_eq!(default_model_for_provider("gemini"), "gemini-2.5-pro");

    // 2. Tenant override: disable seeded `deepseek-chat` (enabled = 0) AND promote custom `claude-opus-4-6` as default
    conn.execute(
        "INSERT INTO model_registry_profiles (id, tenant_id, provider_id, model_id, display_name, tier, context_window, supports_prompt_cache, cache_strategy, is_default, enabled, updated_at) \
         VALUES ('t-disable-deepseek', 'tenant-087', 'deepseek', 'deepseek-chat', 'Disabled DeepSeek', 'tier2_fast', 128000, 0, 'none', 0, 0, 1763960000010)",
        [],
    )
    .expect("disable deepseek for tenant-087");
    conn.execute(
        "INSERT INTO model_registry_profiles (id, tenant_id, provider_id, model_id, display_name, tier, context_window, supports_prompt_cache, cache_strategy, is_default, enabled, updated_at) \
         VALUES ('t-claude-opus', 'tenant-087', 'claude', 'claude-opus-4-6', 'Claude Opus 4.6', 'tier1_frontier', 200000, 1, 'anthropic_ephemeral', 1, 1, 1763960000011)",
        [],
    )
    .expect("override claude default for tenant-087");

    let tenant_models = load_model_registry_from_conn(&conn, "tenant-087");
    assert!(
        !tenant_models.iter().any(|m| m.model_id == "deepseek-chat"),
        "Tenant row with enabled = 0 MUST disable inherited local-default seed model"
    );
    let claude_resolved = resolve_model_for_provider(&tenant_models, "claude").unwrap();
    assert_eq!(
        claude_resolved.model_id, "claude-opus-4-6",
        "Tenant is_default = 1 override MUST win over local-default seed default"
    );
    let seed_claude = tenant_models
        .iter()
        .find(|m| m.model_id == "claude-sonnet-4-5")
        .expect("seed claude still present as non-default");
    assert!(
        !seed_claude.is_default_for_provider,
        "local-default seed must be demoted when tenant overrides provider default"
    );

    // 3. Gemini cachedContent omits systemInstruction to prevent HTTP 400 + create payload helper
    let gem_hints = PromptCacheHints {
        project_id: "proj-087".into(),
        catalog_sha256: "a".repeat(64),
        zone1_static_prefix: Some("ZONE1_SYSTEM".into()),
        cached_content_name: Some("cachedContents/cc-087".into()),
        history_breakpoint_block: None,
    };
    let gem_cached_wire = build_provider_wire_payload(
        "gemini",
        "gemini-2.5-pro",
        "Turn",
        CacheStrategy::GeminiCachedContent,
        Some(&gem_hints),
    );
    assert_eq!(
        gem_cached_wire["cachedContent"],
        json!("cachedContents/cc-087")
    );
    assert!(gem_cached_wire.get("systemInstruction").is_none());

    let create_req =
        build_gemini_cached_content_create_payload(&GeminiCachedContentCreateRequest {
            model: "gemini-2.5-pro".into(),
            display_name: "cw-087".into(),
            system_instruction: "ZONE1_SYSTEM".into(),
            ttl_secs: DEFAULT_GEMINI_CACHE_TTL_SECS,
        });
    assert_eq!(create_req["model"], json!("models/gemini-2.5-pro"));
    assert_eq!(create_req["ttl"], json!("3600s"));

    // 4. Wire-mock test: custom OpenAI endpoint returns 501 on prompt_cache_key -> router retries uncached cleanly!
    let custom_entries = vec![ModelRegistryEntry {
        id: "custom-claude".into(),
        provider_id: "claude".into(),
        model_id: "claude-opus-4-6".into(),
        display_name: "Claude Opus 4.6".into(),
        tier: ModelTier::Tier1Frontier,
        context_window: 200_000,
        supports_prompt_cache: true,
        cache_strategy: CacheStrategy::AnthropicEphemeral,
        is_default_for_provider: true,
        enabled: true,
    }];
    assert_eq!(
        resolve_model_for_provider(&custom_entries, "claude")
            .unwrap()
            .model_id,
        "claude-opus-4-6"
    );
    assert_eq!(
        resolve_cache_strategy(&builtin_seed_models(), "openai", "gpt-4.1"),
        CacheStrategy::OpenaiCacheKey
    );

    let captured: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let cap_clone = Arc::clone(&captured);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new()
        .route(
            "/chat/completions",
            post(
                move |State(store): State<Arc<Mutex<Vec<Value>>>>, Json(body): Json<Value>| async move {
                    store.lock().unwrap().push(body.clone());
                    if body.get("prompt_cache_key").is_some() {
                        return (
                            StatusCode::NOT_IMPLEMENTED,
                            Json(json!({ "error": "prompt_cache_key not implemented" })),
                        );
                    }
                    (
                        StatusCode::OK,
                        Json(json!({
                            "choices": [{ "message": { "content": "uncached-ok" } }]
                        })),
                    )
                },
            ),
        )
        .with_state(cap_clone);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let router = ByokGatewayRouter::with_loopback_test_endpoints(CooldownTracker::new())
        .with_model_registry(tenant_models);
    let hints = PromptCacheHints {
        project_id: "proj-087".into(),
        catalog_sha256: "deadbeef01234567deadbeef01234567deadbeef01234567deadbeef01234567".into(),
        zone1_static_prefix: Some("STATIC_PREFIX".into()),
        cached_content_name: None,
        history_breakpoint_block: None,
    };
    let result = router
        .execute_with_failover_and_cache(
            "tenant-087",
            "User prompt",
            Some("openai"),
            None,
            &[ByokCredential {
                provider_id: "openai".into(),
                api_key: Zeroizing::new("sk-test".into()),
                endpoint_url: Some(format!("http://127.0.0.1:{port}")),
                model_id: None,
            }],
            5000,
            Some(&hints),
        )
        .await
        .expect("501 fallback must succeed");

    assert_eq!(result.raw_content, "uncached-ok");
    assert!(result.cache_telemetry.fallback_uncached);
    let calls = captured.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0]["prompt_cache_key"],
        json!("cw:proj-087:deadbeef01234567")
    );
    assert!(calls[1].get("prompt_cache_key").is_none());
}

#[tokio::test]
async fn ac04_bridge1_real_http_and_db_enforces_human_session_project_isolation_optout_zero_pii_and_escalation(
) {
    let (_dir, pool, project_a, project_b) = setup_pool_with_v24();
    let sig = "chart_type_override:bar:tick-bar";

    // 1. Direct DB tests via real `record_studio_correction_in_db`:
    //    - Strict Zero-PII blocks phone numbers (`+1555...`) and names/paths (`john.doe/acct`)
    {
        let mut conn = pool.connection().expect("conn");
        for bad_val in ["+15551234567", "john.doe/acct", "alice@example.com"] {
            let err = record_studio_correction_in_db(
                &mut conn,
                "tenant-087",
                &RecordStudioCorrectionRequest {
                    project_id: project_a.clone(),
                    correction_kind: StudioCorrectionKind::ChartTypeOverride,
                    signature_key: sig.to_string(),
                    from_value: Some(bad_val.to_string()),
                    to_value: Some("tick-bar".to_string()),
                    skill_id: Some("data-viz-storyteller".to_string()),
                },
            )
            .unwrap_err();
            assert!(
                matches!(
                    err,
                    StudioCorrectionError::Validation {
                        field: "fromValue",
                        ..
                    }
                ),
                "Expected validation error for '{bad_val}', got {err:?}"
            );
        }

        // Cross-tenant project ownership check MUST run BEFORE opt-out short-circuit
        conn.execute(
            "INSERT INTO telemetry_preferences (id, install_id, telemetry_enabled, updated_at) VALUES ('pref-1', 'inst-087', 0, 1763960000005)",
            [],
        )
        .expect("set telemetry opt-out");
        let err_cross_optout = record_studio_correction_in_db(
            &mut conn,
            "tenant-087",
            &RecordStudioCorrectionRequest {
                project_id: "00000000-0000-4000-8000-000000000872".to_string(),
                correction_kind: StudioCorrectionKind::ChartTypeOverride,
                signature_key: sig.to_string(),
                from_value: Some("bar".to_string()),
                to_value: Some("tick-bar".to_string()),
                skill_id: Some("data-viz-storyteller".to_string()),
            },
        )
        .unwrap_err();
        assert!(matches!(
            err_cross_optout,
            StudioCorrectionError::ProjectNotFound(_)
        ));

        // Valid owned project with opt-out returns hit_count == 0, opt_out_skipped == true
        let optout_res = record_studio_correction_in_db(
            &mut conn,
            "tenant-087",
            &RecordStudioCorrectionRequest {
                project_id: project_a.clone(),
                correction_kind: StudioCorrectionKind::ChartTypeOverride,
                signature_key: sig.to_string(),
                from_value: Some("bar".to_string()),
                to_value: Some("tick-bar".to_string()),
                skill_id: Some("data-viz-storyteller".to_string()),
            },
        )
        .expect("opt-out response");
        assert_eq!(optout_res.hit_count, 0);
        assert!(optout_res.opt_out_skipped);
        assert!(!optout_res.escalated_to_ledger);

        // Re-enable telemetry for HTTP end-to-end escalation tests
        conn.execute("UPDATE telemetry_preferences SET telemetry_enabled = 1", [])
            .expect("re-enable telemetry");
    }

    // 2. Real Axum HTTP router tests (`POST /api/v1/learn/studio-corrections`):
    let registry = Arc::new(SpecRegistry::new(pool.clone()));
    let state = DaemonAppState::new(registry, "127.0.0.1".into(), 4170);

    // Non-human session (Agent/regular session token) MUST be denied with 403 HUMAN_APPROVAL_REQUIRED
    let agent_tok = format!("agent-sess-{}", Uuid::new_v4());
    state.auth.insert_session(
        agent_tok.clone(),
        FlowPrincipal {
            tenant_id: "tenant-087".into(),
            user_id: "agent-worker".into(),
            role: "editor".into(),
        },
        Utc::now().timestamp() + 3600,
    );

    // Interactive HumanWorkbenchSession MUST succeed
    let human_tok = state.auth.issue_human_workbench_session(
        FlowPrincipal {
            tenant_id: "tenant-087".into(),
            user_id: "human-designer".into(),
            role: "editor".into(),
        },
        Utc::now().timestamp() + 3600,
    );
    let router = build_router(state);

    let payload_a = json!({
        "projectId": project_a,
        "correctionKind": "chart_type_override",
        "signatureKey": sig,
        "fromValue": "bar",
        "toValue": "tick-bar",
        "skillId": "data-viz-storyteller"
    });

    let (st_agent, body_agent) = send_json(
        &router,
        "POST",
        "/api/v1/learn/studio-corrections",
        &[("x-cowok-session-token", &agent_tok)],
        Some(payload_a.clone()),
    )
    .await;
    assert_eq!(st_agent, StatusCode::FORBIDDEN);
    assert_eq!(body_agent["error"]["code"], "HUMAN_APPROVAL_REQUIRED");

    // Hit 1 & Hit 2 on project_a
    let (st1, b1) = send_json(
        &router,
        "POST",
        "/api/v1/learn/studio-corrections",
        &[("x-cowok-session-token", &human_tok)],
        Some(payload_a.clone()),
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "hit 1 failed: {b1}");
    assert_eq!(b1["data"]["hitCount"], 1);
    assert_eq!(b1["data"]["escalatedToLedger"], false);

    let (st2, b2) = send_json(
        &router,
        "POST",
        "/api/v1/learn/studio-corrections",
        &[("x-cowok-session-token", &human_tok)],
        Some(payload_a.clone()),
    )
    .await;
    assert_eq!(st2, StatusCode::OK);
    assert_eq!(b2["data"]["hitCount"], 2);
    assert_eq!(b2["data"]["escalatedToLedger"], false);

    // Cross-project isolation: same signatureKey on project_b MUST start at hitCount == 1 (NEVER collide with project_a!)
    let payload_b = json!({
        "projectId": project_b,
        "correctionKind": "chart_type_override",
        "signatureKey": sig,
        "fromValue": "bar",
        "toValue": "tick-bar",
        "skillId": "data-viz-storyteller"
    });
    let (st_b1, b_b1) = send_json(
        &router,
        "POST",
        "/api/v1/learn/studio-corrections",
        &[("x-cowok-session-token", &human_tok)],
        Some(payload_b),
    )
    .await;
    assert_eq!(st_b1, StatusCode::OK);
    assert_eq!(b_b1["data"]["hitCount"], 1);
    assert_eq!(b_b1["data"]["escalatedToLedger"], false);

    // Hit 3 on project_a -> escalates to V18a/V20 gate_failure_ledger with PROPOSED + HUMAN_ONLY
    let (st3, b3) = send_json(
        &router,
        "POST",
        "/api/v1/learn/studio-corrections",
        &[("x-cowok-session-token", &human_tok)],
        Some(payload_a),
    )
    .await;
    assert_eq!(st3, StatusCode::OK);
    assert_eq!(b3["data"]["hitCount"], 3);
    assert_eq!(b3["data"]["escalatedToLedger"], true);
    let ledger_id = b3["data"]["ledgerEntryId"]
        .as_str()
        .expect("ledgerEntryId at hitCount == 3");

    let conn = pool.connection().expect("conn");
    let (status, msg): (String, String) = conn
        .query_row(
            "SELECT status, sample_message FROM gate_failure_ledger WHERE id = ?1",
            params![ledger_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("ledger row");
    assert_eq!(status, "PROPOSED");
    assert!(msg.contains("HUMAN_ONLY"));

    let active_plugins: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*) FROM skill_plugins WHERE tenant_id = 'tenant-087' AND lifecycle_state = 'ACTIVE'",
            [],
            |r| r.get(0),
        )
        .optional()
        .expect("count active plugins");
    assert_eq!(active_plugins.unwrap_or(0), 0);
}
