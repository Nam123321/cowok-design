// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
use context_compiler::{
    assemble_and_record, compile_context_only, inspect_prompt_from_args, list_context_telemetry,
    list_context_telemetry_scoped, BlockFormat, ContextAssemblyCommand, ContextCompilerError,
    ContextReasonCode, ContextTelemetryQueryDto, DomOutlineNode, Horizon, ModelProfile,
    OptionalContextBlock, PortalId, DEFAULT_MAX_IMAGE_TOKENS_PER_TURN, MAX_OUTLINE_BPE_TOKENS,
    MAX_TEXT_BPE_TOKENS, MIN_SSOT_DOM_RATIO,
};
use domain::{CanonicalSpecKey, ProvenanceStatus};
use spec_registry::{SpecRegistry, SqlitePool, SqliteWalConfig, UpsertSpecInput};
use tempfile::tempdir;
use uuid::Uuid;

fn seed_real_ssot_specs(reg: &SpecRegistry, proj_id: Uuid) -> Vec<Uuid> {
    let entries = [
        (
            CanonicalSpecKey::ProductOverview,
            "# Product Overview\nPortal-safe context compilation guarantees semantic token fidelity and strict tenant isolation across every portal surface including DeveloperPortal, SalesApp, MarketingWeb, Presentation, and TenantAdmin with deterministic SHA-256 KV-cache prefixes, immutable project scoping, atomic fallback guards, reproducible bundle digests, and full multi-portal boundary verification across every build and workbench session.\n",
        ),
        (
            CanonicalSpecKey::UiUxSpec,
            "# UI & UX Spec\nUse semantic tokens from warm paper palette, 12px card radius, subtle elevation, and registered MetricLine and RightDrawer patterns inside WorkbenchTopNav and ContextMeterPopover, enforcing WCAG AA contrast, focus ring indicators, keyboard navigation semantics, localized translation bundles across nine languages, and complete protection against raw utility-class drift.\n",
        ),
        (
            CanonicalSpecKey::Architecture,
            "# Architecture Spec\nStatic-Prefix / Dynamic-Tail compiler enforces <= 1500 BPE text tokens, <= 350 outline tokens, and >= 60% SSOT dominance across Profile A Frontier, Profile B Local Micro-Tools, and Profile C Offline FTS5 retrieval paths with fail-closed validation, atomic HTML and JSON preservation, deterministic BPE accounting, and lock-free read concurrency.\n",
        ),
        (
            CanonicalSpecKey::DataAndCopy,
            "# Data & Copy Spec\nAll visible strings use typed i18n locale keys across 9 languages with zero bare technical literals, composite idempotency verification, fail-closed cursor pagination signed by runtime HMAC secrets, strict Retry-After propagation headers, localized error remediation hints, and immutable SQLite telemetry ledger records stored atomically.\n",
        ),
        (
            CanonicalSpecKey::EpicsAndStories,
            "# Epics & Stories\nEvery vertical slice enforces five-layer traceability across SQLite WAL migrations, Rust domain compilers, Axum HTTP daemon endpoints, Next.js BFF proxies, and React workbench controls with end-to-end Playwright grounding, idempotent replay verification, strict schema parity tests, and complete traceability across acceptance criteria.\n",
        ),
        (
            CanonicalSpecKey::CrossDependencies,
            "# Cross Dependencies\nStory 02.1 consumes Story 01.2 project registry scoping and Story 01.3 versioned specs and active constraints while exposing deterministic context assembly telemetry, normalized prompt outlines, and fail-closed budget enforcement to downstream Story 02.4 and Story 09.2 engines across all active workspace portals.\n",
        ),
        (
            CanonicalSpecKey::ApiGuidelines,
            "# API Guidelines\nEvery mutating context endpoint enforces strict Zod validation, UUIDv4 Idempotency-Key verification, HMAC-SHA256 cursor binding, Retry-After busy backpressure, explicit malformed JSON and unbalanced HTML rejection, cross-tenant isolation guards, and fail-closed SQLite error mapping across daemon and Next.js proxy handlers.\n",
        ),
    ];
    let mut ids = Vec::with_capacity(entries.len());
    for (spec_key, md) in entries {
        let (rec, _) = reg
            .upsert_spec(UpsertSpecInput {
                project_id: proj_id,
                spec_key,
                markdown_content: md.to_string(),
                expected_sha256: None,
                status: ProvenanceStatus::Confirmed,
                provenance_summary_json: "{}".to_string(),
                source_project_id: None,
                source_revision_sha256: None,
                actor_kind: "HUMAN".to_string(),
                actor_id: "test-operator".to_string(),
                change_summary: "seed real Story 01.3 SSOT specs".to_string(),
            })
            .expect("seed real spec");
        ids.push(rec.id);
    }
    ids
}

fn make_sample_cmd(
    project_id: Uuid,
    portal_id: PortalId,
    profile: ModelProfile,
) -> ContextAssemblyCommand {
    ContextAssemblyCommand {
        project_id,
        tenant_id: "tenant-alpha".to_string(),
        idempotency_key: Some(Uuid::new_v4().to_string()),
        portal_id,
        horizon: Horizon::Mid,
        model_profile: profile,
        intent: "Assemble workbench layout header with semantic design tokens".to_string(),
        requested_image_tokens: 512,
        max_image_tokens_per_turn: DEFAULT_MAX_IMAGE_TOKENS_PER_TURN,
        source_ids: None,
        inspect_only: false,
        brand_tokens_prefix: String::new(),
        active_skill_prefix: String::new(),
        extra_constraints: vec!["no-tailwind".to_string(), "wcag-aa".to_string()],
        dom_outline_nodes: (0..18)
            .map(|i| DomOutlineNode {
                ff_id: format!("ff-node-{i:02}"),
                tag: "section".to_string(),
                role: Some("region".to_string()),
                summary: Some(format!("Workbench block {i}")),
            })
            .collect(),
        optional_blocks: vec![OptionalContextBlock {
            id: "card-json".to_string(),
            format: BlockFormat::Json,
            content: r#"{"component":"MetricLine","tokens":["--bg-surface","--text-primary"]}"#
                .to_string(),
            priority: 1,
        }],
    }
}

fn seed_project_in_temp(ws: &std::path::Path, tenant_id: &str) -> (SpecRegistry, Uuid) {
    let cowok_dir = ws.join(".cowok-design");
    std::fs::create_dir_all(&cowok_dir).expect("mkdir");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(
        cowok_dir.join("cowok-design.sqlite3"),
        ws,
    ))
    .expect("open wal");
    let reg = SpecRegistry::new(pool);
    let proj_id = Uuid::new_v4();
    let conn = reg.pool().connection().expect("conn");
    conn.execute(
        "INSERT INTO projects (id, tenant_id, slug, name, workspace_root, product_brief, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'brief', 'ACTIVE', 1000, 1000)",
        rusqlite::params![
            proj_id.to_string(),
            tenant_id,
            format!("slug-{}", &proj_id.to_string()[..8]),
            "Context Proj",
            ws.display().to_string()
        ],
    )
    .expect("insert project");
    (reg, proj_id)
}

#[test]
fn ac01_02_enforces_bpe_ceilings_ssot_ratio_and_kv_prefix_stability() {
    let mut cmd_turn1 = make_sample_cmd(Uuid::new_v4(), PortalId::SalesApp, ModelProfile::A);
    let out1 = compile_context_only(None, &cmd_turn1).expect("compile turn 1");
    assert!(out1.data.token_usage.outline <= MAX_OUTLINE_BPE_TOKENS);
    assert!(out1.data.token_usage.text_total <= MAX_TEXT_BPE_TOKENS);
    assert_eq!(
        out1.data.token_usage.text_total,
        out1.data.token_usage.static_prefix
            + out1.data.token_usage.dynamic_tail
            + out1.data.token_usage.outline
    );
    assert!(out1.data.token_usage.ssot_dom_ratio >= MIN_SSOT_DOM_RATIO);

    // Turn 2 changes intent and switches to Profile B, yet static prefix bytes and hash stay identical
    cmd_turn1.intent =
        "Update right drawer inspector spacing and verify keyboard navigation".to_string();
    cmd_turn1.model_profile = ModelProfile::B;
    let out2 = compile_context_only(None, &cmd_turn1).expect("compile turn 2");
    assert_eq!(out1.static_prefix, out2.static_prefix);
    assert_eq!(
        out1.data.static_prefix_sha256,
        out2.data.static_prefix_sha256
    );
    assert!(out2.downgraded_to_micro_tools);
    assert!(out2.adapted_tool_count <= 5);
}

#[test]
fn ac03_blocks_superadmin_cross_portal_leak_and_oversized_atomic_html() {
    let mut leak_cmd = make_sample_cmd(Uuid::new_v4(), PortalId::MarketingWeb, ModelProfile::A);
    leak_cmd.intent =
        "Render billing_subscriptions and audit_logs on public landing page".to_string();
    let err = compile_context_only(None, &leak_cmd).unwrap_err();
    assert!(matches!(
        err,
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    // Oversized irreducible HTML block never slices mid-tag; fails cleanly or prunes atomically
    let mut html_cmd = make_sample_cmd(Uuid::new_v4(), PortalId::DeveloperPortal, ModelProfile::A);
    let huge_body = "<span>block</span> ".repeat(700);
    html_cmd.optional_blocks = vec![OptionalContextBlock {
        id: "huge-html".to_string(),
        format: BlockFormat::Html,
        content: format!("<section>{huge_body}</section>"),
        priority: 0,
    }];
    let html_err = compile_context_only(None, &html_cmd).unwrap_err();
    assert!(matches!(
        html_err,
        ContextCompilerError::AtomicBlockOverBudget { .. }
    ));

    // Malformed JSON rejected deterministically
    let mut bad_json_cmd =
        make_sample_cmd(Uuid::new_v4(), PortalId::DeveloperPortal, ModelProfile::A);
    bad_json_cmd.optional_blocks = vec![OptionalContextBlock {
        id: "broken-json".to_string(),
        format: BlockFormat::Json,
        content: "{unclosed".to_string(),
        priority: 0,
    }];
    assert!(matches!(
        compile_context_only(None, &bad_json_cmd).unwrap_err(),
        ContextCompilerError::InvalidRequest { .. }
    ));
}

#[test]
fn ac04_persists_telemetry_idempotency_replay_cursor_pagination_and_cli_inspect() {
    let dir = tempdir().expect("tempdir");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");
    let real_source_ids = seed_real_ssot_specs(&reg, proj_id);
    let mut cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::C);
    cmd.dom_outline_nodes = (0..26)
        .map(|i| DomOutlineNode {
            ff_id: format!("ff-ac04-{i:02}"),
            tag: "section".to_string(),
            role: Some("region".to_string()),
            summary: Some(format!("Workbench region {i}")),
        })
        .collect();
    cmd.source_ids = Some(real_source_ids);
    cmd.intent = "Assemble design specs and token audit ".repeat(44);
    cmd.optional_blocks.push(OptionalContextBlock {
        id: "overflow-optional-telemetry".to_string(),
        format: BlockFormat::Json,
        content: format!(
            r#"{{"metrics":[{}]}}"#,
            (0..220)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        priority: 2,
    });
    cmd.requested_image_tokens = DEFAULT_MAX_IMAGE_TOKENS_PER_TURN;

    let first = assemble_and_record(&reg, &cmd).expect("assemble first");
    assert!(first.offline_fallback_active);
    assert_eq!(first.data.reason_code, ContextReasonCode::PrunedByBudget);
    assert_eq!(first.data.sources.requested, 7);
    assert_eq!(
        first.data.sources.requested,
        first.data.sources.included + first.data.sources.pruned
    );

    // Same idempotency key + fingerprint returns exact original receipt
    let replay = assemble_and_record(&reg, &cmd).expect("replay");
    assert_eq!(replay.data.telemetry_id, first.data.telemetry_id);
    assert_eq!(replay.data.assembled_at, first.data.assembled_at);

    // Same key with changed intent fails 409 IDEMPOTENCY_KEY_REUSED
    let mut mutated = cmd.clone();
    mutated.intent = "Mutated intent with reused idempotency key".to_string();
    assert!(matches!(
        assemble_and_record(&reg, &mutated).unwrap_err(),
        ContextCompilerError::IdempotencyKeyReused
    ));

    // Add second entry and test cursor pagination + cross-tenant cursor rejection
    mutated.idempotency_key = Some(Uuid::new_v4().to_string());
    let second = assemble_and_record(&reg, &mutated).expect("assemble second");
    assert_ne!(second.data.telemetry_id, first.data.telemetry_id);

    let page1 = list_context_telemetry(
        reg.pool(),
        "tenant-alpha",
        &ContextTelemetryQueryDto {
            project_id: proj_id,
            portal_id: Some(PortalId::DeveloperPortal),
            horizon: None,
            model_profile: Some(ModelProfile::C),
            cursor: None,
            limit: Some(1),
        },
    )
    .expect("page 1");
    assert_eq!(page1.items.len(), 1);
    let next = page1.page.next_cursor.expect("next cursor");

    // Cursor bound to filters must fail if filter changes or wrong tenant queries
    assert!(matches!(
        list_context_telemetry(
            reg.pool(),
            "tenant-alpha",
            &ContextTelemetryQueryDto {
                project_id: proj_id,
                portal_id: Some(PortalId::MarketingWeb),
                horizon: None,
                model_profile: Some(ModelProfile::C),
                cursor: Some(next.clone()),
                limit: Some(1),
            },
        )
        .unwrap_err(),
        ContextCompilerError::InvalidCursor
    ));

    let page2 = list_context_telemetry(
        reg.pool(),
        "tenant-alpha",
        &ContextTelemetryQueryDto {
            project_id: proj_id,
            portal_id: Some(PortalId::DeveloperPortal),
            horizon: None,
            model_profile: Some(ModelProfile::C),
            cursor: Some(next),
            limit: Some(1),
        },
    )
    .expect("page 2");
    assert_eq!(page2.items.len(), 1);
    assert_ne!(page1.items[0].telemetry_id, page2.items[0].telemetry_id);

    // CLI --inspect-prompt compiles real Story 01.3 SSOT specs and produces valid JSON with prefixSha256 & budgets
    let cli_json = inspect_prompt_from_args(
        &[
            "--inspect-prompt".to_string(),
            "--portal".to_string(),
            "DeveloperPortal".to_string(),
            "--profile".to_string(),
            "B".to_string(),
        ],
        dir.path(),
    )
    .expect("cli inspect");
    let parsed: serde_json::Value = serde_json::from_str(&cli_json).expect("valid json");
    assert_eq!(parsed["profile"], "B");
    assert_eq!(parsed["prefixSha256"].as_str().unwrap().len(), 64);
}

#[test]
fn remediation_r1_fts5_nfc_homoglyph_void_html_inspect_only_and_priority_zero() {
    let dir = tempdir().expect("tempdir");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");
    let specs_dir = dir.path().join("product/specs");
    std::fs::create_dir_all(&specs_dir).expect("mkdir specs");
    std::fs::write(
        specs_dir.join("03-ui-ux-spec.md"),
        "# UI Spec\nSemantic surface token palette warm paper and navigation rail.\n",
    )
    .expect("write ui spec");
    std::fs::write(
        specs_dir.join("08-security-tenancy.md"),
        "# Security\nMaster secret vault policy and superadmin keys.\n",
    )
    .expect("write sec spec");
    let conn = reg.pool().connection().expect("conn");
    conn.execute(
        "INSERT INTO project_specs (id, project_id, spec_key, file_path, status, version, content_sha256, summary_digest, provenance_summary_json, created_at, updated_at)
         VALUES (?1, ?2, '03-ui-ux-spec.md', 'product/specs/03-ui-ux-spec.md', 'CONFIRMED', 1, '', 'UI tokens navigation rail palette', '{}', 1000, 1000),
                (?3, ?2, '08-security-tenancy.md', 'product/specs/08-security-tenancy.md', 'CONFIRMED', 1, '', 'Security secrets', '{}', 1000, 1000)",
        rusqlite::params![Uuid::new_v4().to_string(), proj_id.to_string(), Uuid::new_v4().to_string()],
    )
    .expect("seed specs");

    // 1. Profile C uses FTS5 over allowed specs without panicking on empty sha256 or leaking 08-security-tenancy.md
    let mut fts_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::C);
    fts_cmd.intent = "caf\u{0065}\u{0301} navigation rail palette".to_string();
    fts_cmd.inspect_only = true;
    fts_cmd.optional_blocks = vec![OptionalContextBlock {
        id: "void-html".to_string(),
        format: BlockFormat::Html,
        content: r#"<div><img src="a.png"><br><input type="text"></div>"#.to_string(),
        priority: 0,
    }];
    let out = assemble_and_record(&reg, &fts_cmd).expect("compile fts5 + void html + inspect_only");
    assert!(out.dynamic_tail.contains("caf\u{00e9}"));
    assert!(out.dynamic_tail.contains("03-ui-ux-spec.md"));
    assert!(!out.dynamic_tail.contains("08-security-tenancy.md"));

    // inspect_only must not insert telemetry rows
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM context_assembly_telemetry WHERE project_id = ?1",
            rusqlite::params![proj_id.to_string()],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(count, 0);

    // 2. Homoglyph / zero-width evasion blocked on TenantAdmin and inside DomOutlineNode
    let mut homoglyph_cmd = make_sample_cmd(proj_id, PortalId::TenantAdmin, ModelProfile::A);
    homoglyph_cmd.intent = "Render S\u{03C5}perAdmin metrics".to_string();
    assert!(matches!(
        compile_context_only(None, &homoglyph_cmd).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    let mut node_leak_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    node_leak_cmd.dom_outline_nodes.push(DomOutlineNode {
        ff_id: "ff-leak".to_string(),
        tag: "div".to_string(),
        role: None,
        summary: Some("expose m\u{0430}ster_key".to_string()),
    });
    assert!(matches!(
        compile_context_only(None, &node_leak_cmd).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    // 3. Second priority-0 atomic block overflowing budget rejects instead of silent drop
    let mut overflow_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    overflow_cmd.optional_blocks = vec![
        OptionalContextBlock {
            id: "small-first".to_string(),
            format: BlockFormat::Json,
            content: r#"{"ok":true}"#.to_string(),
            priority: 0,
        },
        OptionalContextBlock {
            id: "overflow-second".to_string(),
            format: BlockFormat::Json,
            content: format!(
                r#"{{"items":[{}]}}"#,
                (0..1400)
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            priority: 0,
        },
    ];
    assert!(matches!(
        compile_context_only(None, &overflow_cmd).unwrap_err(),
        ContextCompilerError::AtomicBlockOverBudget { .. }
    ));

    // 4. Fullwidth/accented/Cyrillic confusables, node tag marker, void closing </br>, and DB constraint leak check
    let mut fullwidth_cmd = make_sample_cmd(proj_id, PortalId::MarketingWeb, ModelProfile::A);
    fullwidth_cmd.intent =
        "Expose \u{FF33}\u{FF55}\u{FF50}\u{FF45}\u{FF52}\u{FF21}\u{FF44}\u{FF4D}\u{FF49}\u{FF4E}"
            .to_string();
    assert!(matches!(
        compile_context_only(None, &fullwidth_cmd).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    let mut cyr_audit = make_sample_cmd(proj_id, PortalId::MarketingWeb, ModelProfile::A);
    cyr_audit.dom_outline_nodes.push(DomOutlineNode {
        ff_id: "ff-tag-leak".to_string(),
        tag: "audit_l\u{043E}gs".to_string(),
        role: None,
        summary: None,
    });
    assert!(matches!(
        compile_context_only(None, &cyr_audit).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    let mut bad_void = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    bad_void.optional_blocks = vec![OptionalContextBlock {
        id: "closing-void".to_string(),
        format: BlockFormat::Html,
        content: "<div></br></div>".to_string(),
        priority: 0,
    }];
    assert!(matches!(
        compile_context_only(None, &bad_void).unwrap_err(),
        ContextCompilerError::InvalidRequest { .. }
    ));

    let prof_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at)
         VALUES (?1, ?2, 'strict', 'css_vars', 'src/styles/tokens.css', 'icons.json', '[]', '{}', 1000)",
        rusqlite::params![prof_id, proj_id.to_string()],
    )
    .expect("insert constraint profile");
    conn.execute(
        "INSERT INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
         VALUES (?1, ?2, 'SEC_LEAK', 'ERROR', '{\"hint\":\"platform_secrets\"}', 1, 1000)",
        rusqlite::params![Uuid::new_v4().to_string(), prof_id],
    )
    .expect("insert db constraint");
    let db_leak_cmd = make_sample_cmd(proj_id, PortalId::MarketingWeb, ModelProfile::A);
    assert!(matches!(
        compile_context_only(Some(&reg), &db_leak_cmd).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    // 5. Round 3 regression: precomposed 'ẽ'/'ĩ', mismatched HTML tags, and fail-closed public list_context_telemetry
    let mut tilde_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    tilde_cmd.intent = "Leak sup\u{1ebd}radmin and aud\u{0129}t_logs".to_string();
    assert!(matches!(
        compile_context_only(None, &tilde_cmd).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    let mut mismatched_html = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    mismatched_html.optional_blocks = vec![OptionalContextBlock {
        id: "mismatched".to_string(),
        format: BlockFormat::Html,
        content: "<div></span>".to_string(),
        priority: 0,
    }];
    assert!(matches!(
        compile_context_only(None, &mismatched_html).unwrap_err(),
        ContextCompilerError::InvalidRequest { .. }
    ));

    // Public list_context_telemetry must reject SuperAdmin unless explicitly authorized via list_context_telemetry_scoped(..., true)
    let sa_query = ContextTelemetryQueryDto {
        project_id: proj_id,
        portal_id: Some(PortalId::SuperAdmin),
        horizon: None,
        model_profile: None,
        cursor: None,
        limit: Some(5),
    };
    assert!(matches!(
        list_context_telemetry(reg.pool(), "tenant-alpha", &sa_query).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));
    let scoped_ok = list_context_telemetry_scoped(reg.pool(), "tenant-alpha", &sa_query, true);
    assert!(scoped_ok.is_ok());

    // Priority-0 mandatory block fits when compacting genuine SSOT/intent headroom
    let dir_compact = tempdir().expect("tempdir compact");
    let (reg_compact, proj_compact) = seed_project_in_temp(dir_compact.path(), "tenant-alpha");
    let compact_spec_ids = seed_real_ssot_specs(&reg_compact, proj_compact);
    let mut compact_cmd = make_sample_cmd(proj_compact, PortalId::DeveloperPortal, ModelProfile::A);
    compact_cmd.dom_outline_nodes = (0..24)
        .map(|i| DomOutlineNode {
            ff_id: format!("ff-compact-{i:02}"),
            tag: "section".to_string(),
            role: Some("region".to_string()),
            summary: Some(format!("Authentic layout region {i}")),
        })
        .collect();
    compact_cmd.source_ids = Some(compact_spec_ids);
    compact_cmd.intent = "Compact long task ".repeat(110);
    compact_cmd.optional_blocks = vec![OptionalContextBlock {
        id: "compact-json".to_string(),
        format: BlockFormat::Json,
        content: format!(
            r#"{{"keys":[{}]}}"#,
            (0..60).map(|i| i.to_string()).collect::<Vec<_>>().join(",")
        ),
        priority: 0,
    }];
    let compacted = compile_context_only(Some(&reg_compact), &compact_cmd)
        .expect("priority-0 survives SSOT/intent compaction");
    assert_eq!(
        compacted.data.reason_code,
        ContextReasonCode::PrunedByBudget
    );
    assert!(compacted.dynamic_tail.contains("[block:compact-json]"));
    assert!(compacted.data.token_usage.text_total <= MAX_TEXT_BPE_TOKENS);
    assert!(compacted.data.token_usage.ssot_dom_ratio >= MIN_SSOT_DOM_RATIO);

    // 6. Round 5/7 regression: empty/inflated non-SSOT, Latin diacritics, fail-closed nonexistent sourceIds, saturated Profile C
    let mut no_reg_source_cmd =
        make_sample_cmd(proj_compact, PortalId::DeveloperPortal, ModelProfile::A);
    no_reg_source_cmd.source_ids = Some(vec![Uuid::new_v4()]);
    assert!(matches!(
        compile_context_only(None, &no_reg_source_cmd).unwrap_err(),
        ContextCompilerError::InvalidRequest { field: Some(f), .. } if f == "sourceIds"
    ));

    let mut bogus_reg_source_cmd =
        make_sample_cmd(proj_compact, PortalId::DeveloperPortal, ModelProfile::A);
    bogus_reg_source_cmd.dom_outline_nodes.clear();
    bogus_reg_source_cmd.source_ids = Some(vec![Uuid::new_v4()]);
    assert!(matches!(
        compile_context_only(Some(&reg_compact), &bogus_reg_source_cmd).unwrap_err(),
        ContextCompilerError::InvalidRequest { field: Some(f), .. } if f == "sourceIds"
    ));

    let mut empty_ssot_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    empty_ssot_cmd.dom_outline_nodes.clear();
    assert!(matches!(
        compile_context_only(None, &empty_ssot_cmd).unwrap_err(),
        ContextCompilerError::SsotDomRatioBelowMinimum { ratio } if ratio == 0.0
    ));

    let mut inflated_prefix_cmd =
        make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    inflated_prefix_cmd.dom_outline_nodes.clear();
    inflated_prefix_cmd.brand_tokens_prefix = (0..120)
        .map(|i| format!("--fake-tok-{i:03}:#abcdef;"))
        .collect::<Vec<_>>()
        .join(" ");
    inflated_prefix_cmd.active_skill_prefix = "custom-skill-padding ".repeat(80);
    assert!(matches!(
        compile_context_only(None, &inflated_prefix_cmd).unwrap_err(),
        ContextCompilerError::SsotDomRatioBelowMinimum { .. }
    ));

    let mut latin_diacritic_cmd =
        make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    latin_diacritic_cmd.intent = "Render supe\u{0159}admin and audit_\u{0142}ogs".to_string();
    assert!(matches!(
        compile_context_only(None, &latin_diacritic_cmd).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    let dir_sat = tempdir().expect("tempdir sat");
    let (reg_sat, proj_sat) = seed_project_in_temp(dir_sat.path(), "tenant-alpha");
    let sat_ids = seed_real_ssot_specs(&reg_sat, proj_sat);
    let mut saturated_profile_c =
        make_sample_cmd(proj_sat, PortalId::DeveloperPortal, ModelProfile::C);
    saturated_profile_c.dom_outline_nodes = (0..26)
        .map(|i| DomOutlineNode {
            ff_id: format!("ff-sat-{i:02}"),
            tag: "section".to_string(),
            role: Some("region".to_string()),
            summary: Some(format!("Saturated region {i}")),
        })
        .collect();
    saturated_profile_c.source_ids = Some(sat_ids);
    saturated_profile_c.intent = "semantic token audit isolation ".repeat(58);
    saturated_profile_c
        .optional_blocks
        .push(OptionalContextBlock {
            id: "overflow-optional-sat".to_string(),
            format: BlockFormat::Json,
            content: format!(
                r#"{{"sat":[{}]}}"#,
                (0..220)
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            priority: 2,
        });
    let sat_out = compile_context_only(Some(&reg_sat), &saturated_profile_c)
        .expect("saturated Profile C prunes cleanly within 1500 budget");
    assert_eq!(sat_out.data.reason_code, ContextReasonCode::PrunedByBudget);
    assert!(sat_out.data.token_usage.text_total <= MAX_TEXT_BPE_TOKENS);
    assert!(sat_out.data.token_usage.ssot_dom_ratio >= MIN_SSOT_DOM_RATIO);
}

#[test]
fn round8_source_ids_read_real_content_dedup_and_scan_cross_portal() {
    let dir = tempdir().expect("tempdir round8");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");
    let spec_ids = seed_real_ssot_specs(&reg, proj_id);
    let rec = reg
        .get_spec(proj_id, CanonicalSpecKey::ProductOverview)
        .expect("get spec")
        .expect("spec exists");

    let mut cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    cmd.dom_outline_nodes.clear();
    cmd.intent = "Summarize product".to_string();
    cmd.source_ids = Some(spec_ids.clone());
    let out = compile_context_only(Some(&reg), &cmd).expect("real sourceIds compile");
    assert!(out.dynamic_tail.contains("Portal-safe context compilation"));
    assert!(!out.dynamic_tail.contains(&rec.summary_digest));
    assert!(out.data.token_usage.ssot_dom_ratio >= MIN_SSOT_DOM_RATIO);

    let mut dedup_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    dedup_cmd.source_ids = Some(vec![spec_ids[0], spec_ids[0], spec_ids[0]]);
    let dedup_out = compile_context_only(Some(&reg), &dedup_cmd).expect("dedup compile");
    assert_eq!(dedup_out.data.sources.requested, 1);
    assert_eq!(
        dedup_out
            .dynamic_tail
            .matches("01-product-overview.md@")
            .count(),
        1
    );

    assert_allowed_spec_content_leak_blocked();
    assert_tampered_and_uncommitted_sources_fail_closed();
}

fn assert_allowed_spec_content_leak_blocked() {
    let dir = tempdir().expect("tempdir leak");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");
    let (rec, _) = reg
        .upsert_spec(UpsertSpecInput {
            project_id: proj_id,
            spec_key: CanonicalSpecKey::ProductOverview,
            markdown_content: "# Product\nRotate superadmin root_vault_keys before deploy.\n"
                .to_string(),
            expected_sha256: None,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "HUMAN".to_string(),
            actor_id: "test-operator".to_string(),
            change_summary: "seed privileged marker in allowed spec".to_string(),
        })
        .expect("upsert leak spec");

    let mut via_source = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    via_source.source_ids = Some(vec![rec.id]);
    assert!(matches!(
        compile_context_only(Some(&reg), &via_source).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));

    let via_slice = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    assert!(matches!(
        compile_context_only(Some(&reg), &via_slice).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));
}

fn assert_tampered_and_uncommitted_sources_fail_closed() {
    let dir = tempdir().expect("tempdir tamper");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");
    let ids = seed_real_ssot_specs(&reg, proj_id);
    std::fs::write(
        dir.path().join("product/specs/01-product-overview.md"),
        "# Tampered on disk\nModified outside CAS.\n",
    )
    .expect("tamper spec file");

    let mut tampered = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    tampered.source_ids = Some(vec![ids[0]]);
    assert!(matches!(
        compile_context_only(Some(&reg), &tampered).unwrap_err(),
        ContextCompilerError::TelemetryStorageUnavailable { .. }
    ));

    let conn = reg.pool().connection().expect("conn");
    conn.execute(
        "UPDATE project_specs SET content_sha256 = '' WHERE id = ?1",
        rusqlite::params![ids[1].to_string()],
    )
    .expect("clear sha on allowed row");
    let mut uncommitted = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    uncommitted.source_ids = Some(vec![ids[1]]);
    assert!(matches!(
        compile_context_only(Some(&reg), &uncommitted).unwrap_err(),
        ContextCompilerError::InvalidRequest { field: Some(f), .. } if f == "sourceIds"
    ));

    let denied_id = Uuid::new_v4();
    conn.execute(
        "INSERT INTO project_specs (id, project_id, spec_key, file_path, status, version, content_sha256, summary_digest, provenance_summary_json, created_at, updated_at)
         VALUES (?1, ?2, '02-database-spec.md', 'product/specs/02-database-spec.md', 'CONFIRMED', 1, 'deadbeef', 'digest', '{}', 1000, 1000)",
        rusqlite::params![denied_id.to_string(), proj_id.to_string()],
    )
    .expect("insert db spec row");
    let mut excluded_key = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    excluded_key.source_ids = Some(vec![denied_id]);
    assert!(matches!(
        compile_context_only(Some(&reg), &excluded_key).unwrap_err(),
        ContextCompilerError::CrossPortalLeakBlocked { .. }
    ));
}

fn upsert_single_spec(reg: &SpecRegistry, proj_id: Uuid, key: CanonicalSpecKey, md: &str) -> Uuid {
    let prev_sha = reg
        .get_spec(proj_id, key)
        .expect("lookup existing spec")
        .map(|rec| rec.content_sha256);
    let (rec, _) = reg
        .upsert_spec(UpsertSpecInput {
            project_id: proj_id,
            spec_key: key,
            markdown_content: md.to_string(),
            expected_sha256: prev_sha,
            status: ProvenanceStatus::Confirmed,
            provenance_summary_json: "{}".to_string(),
            source_project_id: None,
            source_revision_sha256: None,
            actor_kind: "HUMAN".to_string(),
            actor_id: "test-operator".to_string(),
            change_summary: "round9 structured excerpt test".to_string(),
        })
        .expect("upsert spec");
    rec.id
}

#[test]
fn round9_structured_spec_excerpts_preserve_json_html_atomicity() {
    let dir = tempdir().expect("tempdir round9");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");
    let big_json = format!(
        r#"{{"items":[{}]}}"#,
        (0..120)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let big_html = format!(
        "<section>{}</section>",
        "<div><span>atomic</span></div> ".repeat(30)
    );

    // 1 & 2. Oversized raw valid JSON / HTML fail closed atomically instead of blind 72-tok cut
    for (key, body) in [
        (CanonicalSpecKey::ProductOverview, big_json.as_str()),
        (CanonicalSpecKey::UiUxSpec, big_html.as_str()),
    ] {
        let sid = upsert_single_spec(&reg, proj_id, key, body);
        let mut cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
        cmd.source_ids = Some(vec![sid]);
        assert!(matches!(
            compile_context_only(Some(&reg), &cmd).unwrap_err(),
            ContextCompilerError::AtomicBlockOverBudget { .. }
        ));
    }

    // 3. Malformed raw JSON & unbalanced raw HTML fail closed with InvalidRequest(sourceIds)
    for (key, bad) in [
        (CanonicalSpecKey::ProductOverview, r#"{"unclosed":[1,2"#),
        (CanonicalSpecKey::UiUxSpec, "<section><div></section>"),
    ] {
        let sid = upsert_single_spec(&reg, proj_id, key, bad);
        let mut cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
        cmd.source_ids = Some(vec![sid]);
        assert!(matches!(
            compile_context_only(Some(&reg), &cmd).unwrap_err(),
            ContextCompilerError::InvalidRequest { field: Some(f), .. } if f == "sourceIds"
        ));
    }

    // 4. Small valid JSON spec <= 72 tokens emits intact JSON in dynamic_tail
    let small_json_id = upsert_single_spec(
        &reg,
        proj_id,
        CanonicalSpecKey::ProductOverview,
        r#"{"pattern":"MetricLine","radius":12}"#,
    );
    let mut small_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    small_cmd.source_ids = Some(vec![small_json_id]);
    let small_out = compile_context_only(Some(&reg), &small_cmd).expect("small valid JSON passes");
    assert!(small_out
        .dynamic_tail
        .contains(r#"{"pattern":"MetricLine","radius":12}"#));

    // 5. Markdown heading + prose before oversized fenced JSON falls back to heading/prose cleanly
    let fallback_md =
        format!("# Architecture Overview\nClean prose summary line.\n\n```json\n{big_json}\n```\n");
    let fb_id = upsert_single_spec(&reg, proj_id, CanonicalSpecKey::Architecture, &fallback_md);
    let mut fb_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    fb_cmd.source_ids = Some(vec![fb_id]);
    let fb_out = compile_context_only(Some(&reg), &fb_cmd).expect("markdown fallback succeeds");
    assert!(fb_out.dynamic_tail.contains("# Architecture Overview"));
    assert!(fb_out.dynamic_tail.contains("Clean prose summary line."));
    assert!(!fb_out.dynamic_tail.contains("```"));

    // 6. Markdown with ONLY an oversized fenced HTML block fails closed atomically
    let fenced_html = format!("```html\n{big_html}\n```\n");
    let fh_id = upsert_single_spec(&reg, proj_id, CanonicalSpecKey::DataAndCopy, &fenced_html);
    let mut fh_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    fh_cmd.source_ids = Some(vec![fh_id]);
    assert!(matches!(
        compile_context_only(Some(&reg), &fh_cmd).unwrap_err(),
        ContextCompilerError::AtomicBlockOverBudget { .. }
    ));
}

#[test]
fn round10_markdown_constructs_and_short_horizon_fenced_slices() {
    let dir = tempdir().expect("tempdir round10");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");

    // 1. OCR-021-R9-01: Markdown links, admonitions, tasks, autolinks, and inline-leading HTML succeed cleanly
    let md_rich = concat!(
        "[API Guidelines](./07-api-guidelines.md) summary\n",
        "[FR-14] Requirement\n",
        "[!NOTE] Guard\n",
        "[ref]: https://example.com\n",
        "[ ] task item\n",
        "<https://example.com>\n",
        "<b>Note:</b> inline bold prose\n",
    );
    let md_id = upsert_single_spec(&reg, proj_id, CanonicalSpecKey::ProductOverview, md_rich);
    let mut md_src = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    md_src.source_ids = Some(vec![md_id]);
    let ok_src = compile_context_only(Some(&reg), &md_src).expect("markdown constructs succeed");
    assert!(ok_src
        .dynamic_tail
        .contains("[API Guidelines](./07-api-guidelines.md)"));
    assert!(ok_src
        .dynamic_tail
        .contains("<b>Note:</b> inline bold prose"));

    let mut md_auto = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    md_auto.source_ids = None;
    let ok_auto = compile_context_only(Some(&reg), &md_auto).expect("auto slice markdown succeeds");
    assert!(ok_auto.dynamic_tail.contains("[FR-14] Requirement"));

    // Genuinely malformed JSON array still fails closed with InvalidRequest(sourceIds)
    let bad_arr_id = upsert_single_spec(
        &reg,
        proj_id,
        CanonicalSpecKey::ProductOverview,
        r#"["unclosed""#,
    );
    let mut bad_arr = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    bad_arr.source_ids = Some(vec![bad_arr_id]);
    assert!(matches!(
        compile_context_only(Some(&reg), &bad_arr).unwrap_err(),
        ContextCompilerError::InvalidRequest { field: Some(f), .. } if f == "sourceIds"
    ));

    // 2. OCR-021-R9-02: Horizon::Short (>600 tok) forces ContextSlicer byte truncation mid-fence without failing 422
    let dir2 = tempdir().expect("tempdir round10 short");
    let (reg2, proj2) = seed_project_in_temp(dir2.path(), "tenant-alpha");
    seed_real_ssot_specs(&reg2, proj2);
    let big_json = format!(
        r#"{{"items":[{}]}}"#,
        (0..220)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let fenced_data = format!(
        "# Data Spec\nClean summary of localized schemas and ledger records.\n\n```json\n{big_json}\n```\n"
    );
    upsert_single_spec(&reg2, proj2, CanonicalSpecKey::DataAndCopy, &fenced_data);
    let mut short_cmd = make_sample_cmd(proj2, PortalId::DeveloperPortal, ModelProfile::A);
    short_cmd.horizon = Horizon::Short;
    short_cmd.source_ids = None;
    short_cmd.dom_outline_nodes = (0..26)
        .map(|i| DomOutlineNode {
            ff_id: format!("ff-r10-{i:02}"),
            tag: "section".to_string(),
            role: Some("region".to_string()),
            summary: Some(format!("Short horizon node {i}")),
        })
        .collect();
    let short_out =
        compile_context_only(Some(&reg2), &short_cmd).expect("short horizon fenced slice succeeds");
    assert!(short_out.dynamic_tail.contains("# Data Spec"));
    assert!(!short_out.dynamic_tail.contains("```"));
    assert!(short_out.data.token_usage.text_total <= MAX_TEXT_BPE_TOKENS);
    assert!(short_out.data.token_usage.ssot_dom_ratio >= MIN_SSOT_DOM_RATIO);
}

#[test]
fn round11_html_url_attrs_and_multiline_inline_html_prose() {
    let dir = tempdir().expect("tempdir round11");
    let (reg, proj_id) = seed_project_in_temp(dir.path(), "tenant-alpha");

    // 1. Valid HTML with URL/namespace attributes passes intact; oversized URL-attr HTML fails atomically
    for valid_html in [
        r#"<div><a href="https://example.com">Docs</a></div>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0"/></svg>"#,
        r#"<img src="https://example.com/a.png">"#,
    ] {
        let sid = upsert_single_spec(&reg, proj_id, CanonicalSpecKey::ProductOverview, valid_html);
        let mut cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
        cmd.source_ids = Some(vec![sid]);
        let out = compile_context_only(Some(&reg), &cmd).expect("URL/namespace HTML compiles");
        assert!(out.dynamic_tail.contains(valid_html));
    }

    let big_links_html = format!(
        "<div>{}</div>",
        (0..20)
            .map(|i| format!(r#"<a href="https://example.com/item/{i}">L{i}</a>"#))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let big_sid = upsert_single_spec(
        &reg,
        proj_id,
        CanonicalSpecKey::ProductOverview,
        &big_links_html,
    );
    let mut big_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    big_cmd.source_ids = Some(vec![big_sid]);
    assert!(matches!(
        compile_context_only(Some(&reg), &big_cmd).unwrap_err(),
        ContextCompilerError::AtomicBlockOverBudget { .. }
    ));

    // 2. Markdown prose with trailing URL-attr <img ...> is atomic (never sliced mid-tag)
    let img_prose = format!(
        "# Visual Guide\n{} <img src=\"https://example.com/diagram.png\">\nTail word.",
        "concise summary word ".repeat(34)
    );
    let img_sid = upsert_single_spec(&reg, proj_id, CanonicalSpecKey::UiUxSpec, &img_prose);
    let mut img_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    img_cmd.source_ids = Some(vec![img_sid]);
    let img_out = compile_context_only(Some(&reg), &img_cmd).expect("atomic inline img prose");
    assert!(!img_out
        .dynamic_tail
        .contains("<img src=\"https://example.com/diagram.png"));

    // 3. Balanced HTML line ending with '>' followed directly by prose works in both source_ids Some & None
    let html_then_prose = concat!(
        "<b>Note:</b>\nFollow-up prose line.\n",
        "<!-- comment -->\nComment prose line.\n",
        "<hr>\nDivider prose line.\n",
        "<div>\n<span>hi</span>\n</div>\nTrailing prose after block.\n",
    );
    let hp_sid = upsert_single_spec(
        &reg,
        proj_id,
        CanonicalSpecKey::Architecture,
        html_then_prose,
    );
    let mut hp_src = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
    hp_src.source_ids = Some(vec![hp_sid]);
    let hp_out = compile_context_only(Some(&reg), &hp_src).expect("inline html followed by prose");
    assert!(hp_out.dynamic_tail.contains("<b>Note:</b>"));
    assert!(hp_out.dynamic_tail.contains("Follow-up prose line."));
    assert!(hp_out.dynamic_tail.contains("Trailing prose after block."));

    let dir_prose = tempdir().expect("tempdir round11 prose");
    let (reg_prose, proj_prose) = seed_project_in_temp(dir_prose.path(), "tenant-alpha");
    upsert_single_spec(
        &reg_prose,
        proj_prose,
        CanonicalSpecKey::Architecture,
        html_then_prose,
    );
    let mut hp_auto = make_sample_cmd(proj_prose, PortalId::DeveloperPortal, ModelProfile::A);
    hp_auto.source_ids = None;
    let hp_auto_out =
        compile_context_only(Some(&reg_prose), &hp_auto).expect("auto slice inline html + prose");
    assert!(hp_auto_out.dynamic_tail.contains("<b>Note:</b>"));

    // 4. Genuinely malformed HTML still fails closed with InvalidRequest(sourceIds)
    for bad_html in ["<section><div></section>", "<div>broken prose"] {
        let bad_sid = upsert_single_spec(&reg, proj_id, CanonicalSpecKey::DataAndCopy, bad_html);
        let mut bad_cmd = make_sample_cmd(proj_id, PortalId::DeveloperPortal, ModelProfile::A);
        bad_cmd.source_ids = Some(vec![bad_sid]);
        assert!(matches!(
            compile_context_only(Some(&reg), &bad_cmd).unwrap_err(),
            ContextCompilerError::InvalidRequest { field: Some(f), .. } if f == "sourceIds"
        ));
    }
}
