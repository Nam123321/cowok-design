// @story 11.2
// @implements AC-01, AC-02, AC-03 (Unit & Integration Tests for mcp-gateway Protocol, EC-LEARN-001 HUMAN_ONLY Block, Atomic apply_batch Rollback, Solvers & Safe Host Installer)
use std::fs;
use std::time::Instant;

use mcp_gateway::{
    default_tool_policies, format_sse_event, format_stdio_frame, install_host_config,
    list_banner_presets, merge_codex_toml_config, parse_stdio_frame, resolve_run_scoped_bundle,
    rollback_host_install, solve_banner_grid, solve_grounded_diagram, ActivityStatus, ActorKind,
    BannerSolveRequest, DiagramEdgeInput, DiagramNodeInput, GroundedDiagramRequest, HostClientId,
    HostInstallError, HostInstallOptions, JsonRpcRequest, McpGatewayEngine, PermissionTier,
    SolverError, ERR_HUMAN_APPROVAL_REQUIRED, ERR_PARSE, ERR_SELF_HEALING_TOOL, ERR_TOOL_DISABLED,
    JSONRPC_VERSION, MAX_NODES, WORKBENCH_APPROVAL_DEEP_LINK_BASE,
};
use serde_json::json;
use tempfile::TempDir;

fn make_rpc(id: i64, method: &str, params: Option<serde_json::Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

#[test]
fn t1_tools_list_hides_human_only_and_calls_reject_with_32003_no_token_bypass() {
    let engine = McpGatewayEngine::new("proj-11-2");
    let list_out = engine.dispatch(make_rpc(1, "tools/list", None), ActorKind::McpClient);
    assert_eq!(list_out.status, ActivityStatus::Ok);
    let tools = list_out.response.result.unwrap()["tools"]
        .as_array()
        .cloned()
        .unwrap();
    assert!(!tools.is_empty());
    for tool in &tools {
        assert_ne!(tool["permissionTier"].as_str(), Some("HUMAN_ONLY"));
        assert_ne!(tool["name"].as_str(), Some("ephemeral_tool_forge"));
        assert_ne!(tool["name"].as_str(), Some("learn_card_promote"));
    }

    // Even when MCP client supplies a forged approval_token, HUMAN_ONLY MUST reject unconditionally
    for privileged in [
        "ephemeral_tool_forge",
        "learn_card_promote",
        "staging_plugin_promote",
    ] {
        let call = make_rpc(
            2,
            "tools/call",
            Some(json!({
                "name": privileged,
                "arguments": {
                    "approval_token": "forged-human-token-xyz",
                    "human_override": true
                }
            })),
        );
        let out = engine.dispatch(call, ActorKind::McpClient);
        assert_eq!(out.status, ActivityStatus::DeniedHumanOnly);
        let err = out.response.error.expect("expected JSON-RPC error");
        assert_eq!(err.code, ERR_HUMAN_APPROVAL_REQUIRED);
        assert_eq!(err.message, "HUMAN_APPROVAL_REQUIRED");
        let data = err.data.expect("expected structured data");
        let link = data["workbenchDeepLink"].as_str().unwrap_or_default();
        assert!(link.starts_with(WORKBENCH_APPROVAL_DEEP_LINK_BASE));
        assert!(link.contains(privileged));
    }
}

#[test]
fn t2_get_app_state_fast_and_query_predicate_and_self_healing_error() {
    let engine = McpGatewayEngine::new("proj-speed");
    let t0 = Instant::now();
    let state_out = engine.dispatch(
        make_rpc(
            10,
            "tools/call",
            Some(json!({ "name": "get_app_state", "arguments": {} })),
        ),
        ActorKind::McpClient,
    );
    assert!(
        t0.elapsed().as_millis() < 500,
        "get_app_state must be <500ms"
    );
    assert_eq!(state_out.status, ActivityStatus::Ok);
    let state_sc = &state_out.response.result.unwrap()["structuredContent"];
    assert_eq!(state_sc["projectId"], "proj-speed");
    assert!(state_sc["nodeCount"].as_u64().unwrap() >= 2);

    // Predicate query: has-problem('text-truncated')
    let query_out = engine.dispatch(
        make_rpc(
            11,
            "tools/call",
            Some(json!({
                "name": "query_predicate",
                "arguments": { "predicate": "has-problem('text-truncated')" }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(query_out.status, ActivityStatus::Ok);
    let qsc = &query_out.response.result.unwrap()["structuredContent"];
    assert!(qsc["matchCount"].as_u64().unwrap() >= 1);

    // Invalid predicate returns Self-Healing ToolError with recovery_hint and valid_examples
    let bad_query = engine.dispatch(
        make_rpc(
            12,
            "tools/call",
            Some(json!({
                "name": "query_predicate",
                "arguments": { "predicate": "bad-fn(??)" }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(bad_query.status, ActivityStatus::SelfHealingError);
    let err = bad_query.response.error.unwrap();
    assert_eq!(err.code, ERR_SELF_HEALING_TOOL);
    let data = err.data.unwrap();
    assert!(data["recoveryHint"].as_str().unwrap().len() > 10);
    assert!(!data["validExamples"].as_array().unwrap().is_empty());
}

#[test]
fn t3_apply_batch_alias_dry_run_idempotency_and_atomic_rollback() {
    let engine = McpGatewayEngine::new("proj-batch");

    // 1. Dry-run does not mutate live state
    let dry_out = engine.dispatch(
        make_rpc(
            20,
            "tools/call",
            Some(json!({
                "name": "apply_batch",
                "arguments": {
                    "dry_run": true,
                    "operations": [
                        { "op": "create_node", "alias": "@panel", "kind": "card", "text": "Preview", "width": 300, "height": 120 },
                        { "op": "set_text", "target_id": "@panel", "text": "Preview Updated" }
                    ]
                }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(dry_out.status, ActivityStatus::Ok);
    let dry_sc = &dry_out.response.result.unwrap()["structuredContent"];
    assert_eq!(dry_sc["dryRun"], true);
    assert_eq!(dry_sc["applied"], false);
    assert_eq!(dry_sc["version"], 1);
    assert!(dry_sc["diffImage"]["previewSvgDataUri"]
        .as_str()
        .unwrap()
        .starts_with("data:image/svg+xml"));

    // 2. Atomic rollback: step 1 creates @temp, step 2 fails on missing @ghost -> step 1 is rolled back!
    let fail_out = engine.dispatch(
        make_rpc(
            21,
            "tools/call",
            Some(json!({
                "name": "apply_batch",
                "arguments": {
                    "dry_run": false,
                    "operations": [
                        { "op": "create_node", "id": "node-should-rollback", "alias": "@temp", "kind": "card", "text": "Temp", "width": 200, "height": 80 },
                        { "op": "set_text", "target_id": "@ghost_alias", "text": "Boom" }
                    ]
                }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(fail_out.status, ActivityStatus::SelfHealingError);
    let err_data = fail_out.response.error.unwrap().data.unwrap();
    assert_eq!(err_data["code"], "BATCH_STEP_FAILED");
    assert_eq!(err_data["failedStep"], 2);
    assert_eq!(err_data["rolledBackSteps"], 1);

    // Verify node-should-rollback does NOT exist in live state
    let verify_state = engine.dispatch(
        make_rpc(
            22,
            "tools/call",
            Some(json!({ "name": "get_app_state", "arguments": {} })),
        ),
        ActorKind::McpClient,
    );
    let nodes = verify_state.response.result.unwrap()["structuredContent"]["nodes"]
        .as_array()
        .cloned()
        .unwrap();
    assert!(!nodes.iter().any(|n| n["id"] == "node-should-rollback"));

    // 3. Commit with idempotency_key + replay
    let commit_call = make_rpc(
        23,
        "tools/call",
        Some(json!({
            "name": "apply_batch",
            "arguments": {
                "dry_run": false,
                "idempotency_key": "idem-key-11-2",
                "operations": [
                    { "op": "create_node", "id": "node-committed", "alias": "@card1", "kind": "card", "text": "Initial", "width": 280, "height": 90 },
                    { "op": "set_text", "target_id": "@card1", "text": "Final" }
                ]
            }
        })),
    );
    let first = engine.dispatch(commit_call.clone(), ActorKind::McpClient);
    assert_eq!(first.status, ActivityStatus::Ok);
    assert_eq!(
        first.response.result.as_ref().unwrap()["structuredContent"]["version"],
        2
    );
    assert_eq!(
        first.response.result.as_ref().unwrap()["structuredContent"]["idempotentReplay"],
        false
    );

    let replay = engine.dispatch(commit_call, ActorKind::McpClient);
    assert_eq!(replay.status, ActivityStatus::Ok);
    assert_eq!(
        replay.response.result.as_ref().unwrap()["structuredContent"]["version"],
        2
    );
    assert_eq!(
        replay.response.result.as_ref().unwrap()["structuredContent"]["idempotentReplay"],
        true
    );
}

#[test]
fn t4_resources_and_stdio_sse_framing() {
    let engine = McpGatewayEngine::new("proj-res");
    let raw_cl = r#"{"jsonrpc":"2.0","id":30,"method":"resources/list"}"#;
    let framed = format!("Content-Length: {}\r\n\r\n{}", raw_cl.len(), raw_cl);
    let parsed = parse_stdio_frame(&framed).expect("parse Content-Length frame");
    let list_out = engine.dispatch(parsed, ActorKind::McpClient);
    let uris: Vec<String> = list_out.response.result.as_ref().unwrap()["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["uri"].as_str().map(str::to_string))
        .collect();
    assert!(uris.contains(&"cowok-design://rules".to_string()));
    assert!(uris.contains(&"cowok://skills/design-craft".to_string()));
    assert!(uris.contains(&"ui://cowok-design/brief-and-preview.html".to_string()));

    let cl_out = format_stdio_frame(&list_out.response, true);
    assert!(cl_out.starts_with("Content-Length: "));
    let sse_out = format_sse_event(&list_out.response);
    assert!(sse_out.starts_with("event: message\ndata: "));

    for uri in [
        "cowok-design://rules",
        "cowok://skills/design-craft",
        "ui://cowok-design/brief-and-preview.html",
    ] {
        let read_out = engine.dispatch(
            make_rpc(31, "resources/read", Some(json!({ "uri": uri }))),
            ActorKind::McpClient,
        );
        assert_eq!(read_out.status, ActivityStatus::Ok);
        let text = read_out.response.result.unwrap()["contents"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(!text.is_empty());
    }
}

#[test]
fn t5_run_scoped_bundles_banner_g1_g15_and_grounded_diagram_dos_cap() {
    let bundle = resolve_run_scoped_bundle("banner_generate");
    assert!(bundle.excludes_human_only);
    assert!(bundle.allowed_tools.contains(&"banner_solver".to_string()));
    assert!(!bundle
        .allowed_tools
        .contains(&"ephemeral_tool_forge".to_string()));

    // Verify all 15 banner grid presets produce collision-free deterministic layouts
    let presets = list_banner_presets();
    assert_eq!(presets.len(), 15);
    for (preset_id, _) in presets {
        let res = solve_banner_grid(&BannerSolveRequest {
            preset: preset_id.to_string(),
            width: 1200.0,
            height: 630.0,
            padding: 32.0,
            gap: 16.0,
            slots: vec![],
        })
        .expect("preset solve");
        assert!(
            res.collision_free,
            "Preset {preset_id} must be collision-free"
        );
        assert!(!res.placements.is_empty());
    }

    // Grounded diagram solver works deterministically and enforces MAX_NODES = 256 DoS cap
    let diagram = solve_grounded_diagram(&GroundedDiagramRequest {
        direction: Some("RIGHT".to_string()),
        node_spacing: Some(24.0),
        layer_spacing: Some(80.0),
        nodes: vec![
            DiagramNodeInput {
                id: "gateway".into(),
                label: "MCP Gateway".into(),
                width: None,
                height: None,
                source_file: Some("crates/mcp-gateway/src/protocol.rs".into()),
            },
            DiagramNodeInput {
                id: "sqlite".into(),
                label: "SQLite WAL".into(),
                width: None,
                height: None,
                source_file: None,
            },
        ],
        edges: vec![DiagramEdgeInput {
            id: None,
            source: "gateway".into(),
            target: "sqlite".into(),
            label: Some("V28".into()),
        }],
        repo_extract_symbols: vec!["sqlite".into()],
    })
    .expect("diagram solve");
    assert_eq!(diagram.node_count, 2);
    assert_eq!(diagram.layer_count, 2);
    assert!(diagram.nodes.iter().all(|n| n.grounded));

    let too_many_nodes: Vec<DiagramNodeInput> = (0..=MAX_NODES)
        .map(|i| DiagramNodeInput {
            id: format!("n-{i}"),
            label: format!("Node {i}"),
            width: None,
            height: None,
            source_file: None,
        })
        .collect();
    let dos_err = solve_grounded_diagram(&GroundedDiagramRequest {
        direction: None,
        node_spacing: None,
        layer_spacing: None,
        nodes: too_many_nodes,
        edges: vec![],
        repo_extract_symbols: vec![],
    })
    .expect_err("must reject > MAX_NODES");
    assert!(matches!(dos_err, SolverError::MaxNodesExceeded { .. }));
}

#[test]
fn t6_host_installer_json_toml_preserve_siblings_reject_symlink_and_rollback() {
    let tmp = TempDir::new().expect("tempdir");
    let backup_dir = tmp.path().join("backups");

    // 1. JSON host (Cursor) preserves sibling keys and sibling servers + rolls back cleanly
    let cursor_cfg = tmp.path().join("cursor-mcp.json");
    let original_json = "{\n  \"theme\": \"warm\",\n  \"mcpServers\": {\n    \"github\": { \"command\": \"npx\", \"args\": [\"-y\", \"github-mcp\"] }\n  }\n}\n";
    fs::write(&cursor_cfg, original_json).unwrap();

    let receipt = install_host_config(&HostInstallOptions {
        host: HostClientId::Cursor,
        config_path: cursor_cfg.clone(),
        backup_dir: backup_dir.clone(),
        server_command: "cowok".into(),
        server_args: vec!["mcp".into(), "serve".into()],
        server_url: "http://127.0.0.1:4171/api/v1/mcp/rpc".into(),
    })
    .expect("install cursor");
    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&cursor_cfg).unwrap()).unwrap();
    assert_eq!(merged["theme"], "warm");
    assert_eq!(merged["mcpServers"]["github"]["command"], "npx");
    assert_eq!(merged["mcpServers"]["cowok-design"]["command"], "cowok");

    let rb = rollback_host_install(
        HostClientId::Cursor,
        &cursor_cfg,
        std::path::Path::new(&receipt.backup_path),
        &receipt.before_sha256,
    )
    .expect("rollback cursor");
    assert_eq!(rb.restored_sha256, receipt.before_sha256);
    assert_eq!(fs::read_to_string(&cursor_cfg).unwrap(), original_json);

    // 2. TOML host (Codex) preserves sibling tables and comments
    let codex_cfg = tmp.path().join("config.toml");
    let original_toml =
        "# User Codex config\nmodel = \"gpt-5\"\n\n[mcp_servers.other_tool]\ncommand = \"other\"\n";
    fs::write(&codex_cfg, original_toml).unwrap();

    let codex_receipt = install_host_config(&HostInstallOptions {
        host: HostClientId::Codex,
        config_path: codex_cfg.clone(),
        backup_dir: backup_dir.clone(),
        server_command: "cowok".into(),
        server_args: vec!["mcp".into(), "serve".into()],
        server_url: "http://127.0.0.1:4171/api/v1/mcp/rpc".into(),
    })
    .expect("install codex");
    let merged_toml = fs::read_to_string(&codex_cfg).unwrap();
    assert!(merged_toml.contains("[mcp_servers.other_tool]"));
    assert!(merged_toml.contains("[mcp_servers.cowok_design]"));
    assert!(merged_toml.contains("model = \"gpt-5\""));

    rollback_host_install(
        HostClientId::Codex,
        &codex_cfg,
        std::path::Path::new(&codex_receipt.backup_path),
        &codex_receipt.before_sha256,
    )
    .expect("rollback codex");
    assert_eq!(fs::read_to_string(&codex_cfg).unwrap(), original_toml);

    // 3. O_NOFOLLOW symlink rejection
    #[cfg(unix)]
    {
        let secret_target = tmp.path().join("secret-target.json");
        fs::write(&secret_target, "{}").unwrap();
        let symlink_path = tmp.path().join("evil-link.json");
        std::os::unix::fs::symlink(&secret_target, &symlink_path).unwrap();

        let err = install_host_config(&HostInstallOptions {
            host: HostClientId::ClaudeCode,
            config_path: symlink_path,
            backup_dir,
            server_command: "cowok".into(),
            server_args: vec!["mcp".into()],
            server_url: "http://127.0.0.1:4171/api/v1/mcp/rpc".into(),
        })
        .expect_err("must reject symlink");
        assert!(matches!(err, HostInstallError::SymlinkRejected { .. }));
    }
}

#[test]
fn t7_parse_stdio_frame_multi_header_trailing_newline_and_utf8_boundary() {
    // Multi-header (Content-Type + lowercase content-length), leading CRLF, and trailing newline counted in Content-Length
    let body_with_nl = "{\"jsonrpc\":\"2.0\",\"id\":41,\"method\":\"ping\"}\n";
    let multi_hdr = format!(
        "\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\ncontent-length: {}\r\n\r\n{}",
        body_with_nl.len(),
        body_with_nl
    );
    let parsed = parse_stdio_frame(&multi_hdr).expect("multi-header frame with trailing newline");
    assert_eq!(parsed.method, "ping");

    // Multi-byte UTF-8 boundary split MUST return ERR_PARSE cleanly without panicking
    let utf8_body = "{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"Xin chào 🚀\"}";
    let emoji_byte_idx = utf8_body.find('🚀').expect("emoji present");
    let mid_char_len = emoji_byte_idx + 1; // cuts right inside the 4-byte UTF-8 sequence
    let bad_utf8_frame = format!("Content-Length: {mid_char_len}\r\n\r\n{utf8_body}");
    let err = parse_stdio_frame(&bad_utf8_frame).expect_err("must reject mid-UTF-8 boundary");
    assert_eq!(err.error.expect("rpc error").code, ERR_PARSE);

    // Truncated body returns ERR_PARSE cleanly
    let trunc_frame = format!(
        "Content-Length: {}\r\n\r\n{}",
        body_with_nl.len() + 50,
        body_with_nl
    );
    let trunc_err = parse_stdio_frame(&trunc_frame).expect_err("must reject truncated frame");
    assert_eq!(trunc_err.error.expect("rpc error").code, ERR_PARSE);

    // Header block without Content-Length returns ERR_PARSE cleanly
    let missing_cl =
        "Content-Type: application/json\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}";
    let missing_err =
        parse_stdio_frame(missing_cl).expect_err("must reject missing Content-Length");
    assert_eq!(missing_err.error.expect("rpc error").code, ERR_PARSE);
}

#[test]
fn t8_codex_toml_comment_aware_merge_control_escaping_and_malformed_rejection() {
    // Inline comments on both our section and sibling section MUST preserve sibling and avoid duplicate cowok_design
    let existing = "# User config\nmodel = \"gpt-5\"\n\n[mcp_servers.cowok_design] # managed\ncommand = \"old-cmd\"\nargs = [\"old\"]\n\n[mcp_servers.github] # keep\ncommand = \"npx\"\nargs = [\"-y\", \"github-mcp\"]\n";
    let merged = merge_codex_toml_config(
        existing,
        "cowok\tbin",
        &["mcp\nserve".into(), "arg\r\u{0007}\"\\".into()],
        "http://127.0.0.1:4171/api/v1/mcp/rpc",
    )
    .expect("valid comment-aware merge");

    assert_eq!(
        merged.matches("[mcp_servers.cowok_design]").count(),
        1,
        "must not duplicate cowok_design section"
    );
    assert!(
        merged.contains("[mcp_servers.github] # keep"),
        "must preserve sibling section with inline comment"
    );
    assert!(merged.contains("command = \"npx\""));
    assert!(!merged.contains("old-cmd"));
    assert!(merged.contains("command = \"cowok\\tbin\""));
    assert!(merged.contains("mcp\\nserve"));
    assert!(merged.contains("\\u0007"));

    // Malformed TOML inputs MUST fail with HostInstallError::MalformedConfig
    for broken in [
        "[mcp_servers.github\ncommand = \"npx\"\n",
        "model = \"unterminated\n",
        "this is not valid toml syntax\n",
        "[mcp_servers.github]\nargs = [\"unclosed\"\n",
    ] {
        let err = merge_codex_toml_config(
            broken,
            "cowok",
            &["mcp".into()],
            "http://127.0.0.1:4171/api/v1/mcp/rpc",
        )
        .expect_err("must reject malformed TOML");
        assert!(matches!(err, HostInstallError::MalformedConfig { .. }));
    }
}

#[test]
fn t9_rollback_newly_created_host_config_unlinks_and_acquires_lock() {
    let tmp = TempDir::new().expect("tempdir");
    let backup_dir = tmp.path().join("backups");
    let fresh_cfg = tmp.path().join("fresh-cursor.json");
    assert!(!fresh_cfg.exists());

    let receipt = install_host_config(&HostInstallOptions {
        host: HostClientId::Cursor,
        config_path: fresh_cfg.clone(),
        backup_dir: backup_dir.clone(),
        server_command: "cowok".into(),
        server_args: vec!["mcp".into(), "serve".into()],
        server_url: "http://127.0.0.1:4171/api/v1/mcp/rpc".into(),
    })
    .expect("install fresh cursor config");
    assert!(receipt.created_new);
    assert!(fresh_cfg.exists());

    let rb = rollback_host_install(
        HostClientId::Cursor,
        &fresh_cfg,
        std::path::Path::new(&receipt.backup_path),
        &receipt.before_sha256,
    )
    .expect("rollback fresh cursor config");
    assert_eq!(rb.restored_sha256, receipt.before_sha256);
    assert!(
        !fresh_cfg.exists(),
        "rolling back a newly created config must unlink it instead of leaving a 0-byte corrupt JSON file"
    );
    assert!(
        backup_dir.join(".cursor.install.lock").exists(),
        "rollback must acquire the same advisory lock file in backup_dir"
    );
}

#[test]
fn t10_disabled_non_human_only_tool_returns_32004_not_32003() {
    let engine = McpGatewayEngine::new("proj-disabled");
    let mut policies = default_tool_policies();
    for pol in &mut policies {
        if pol.tool_name == "banner_solver" {
            pol.enabled = false;
            pol.mcp_callable = false;
        }
    }
    engine.sync_tool_policies(&policies);

    let out = engine.dispatch(
        make_rpc(
            50,
            "tools/call",
            Some(json!({
                "name": "banner_solver",
                "arguments": { "preset": "G1", "width": 1200, "height": 630 }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(out.status, ActivityStatus::SelfHealingError);
    assert_eq!(out.permission_tier, PermissionTier::Inspect);
    assert_eq!(out.jsonrpc_code, Some(ERR_TOOL_DISABLED));
    let err = out.response.error.expect("expected disabled tool error");
    assert_eq!(err.code, ERR_TOOL_DISABLED);
    assert_ne!(err.code, ERR_HUMAN_APPROVAL_REQUIRED);
}

#[test]
fn t11_empty_or_whitespace_idempotency_key_does_not_poison_cache() {
    let engine = McpGatewayEngine::new("proj-empty-idem");

    let first = engine.dispatch(
        make_rpc(
            60,
            "tools/call",
            Some(json!({
                "name": "apply_batch",
                "arguments": {
                    "dryRun": false,
                    "idempotencyKey": "",
                    "operations": [
                        { "op": "create_node", "id": "node-empty-1", "kind": "card", "text": "A", "width": 200, "height": 80 }
                    ]
                }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(first.status, ActivityStatus::Ok);
    assert_eq!(first.idempotency_key, None);
    assert_eq!(
        first.response.result.as_ref().unwrap()["structuredContent"]["version"],
        2
    );
    assert_eq!(
        first.response.result.as_ref().unwrap()["structuredContent"]["idempotentReplay"],
        false
    );

    let second = engine.dispatch(
        make_rpc(
            61,
            "tools/call",
            Some(json!({
                "name": "apply_batch",
                "arguments": {
                    "dryRun": false,
                    "idempotencyKey": "   ",
                    "operations": [
                        { "op": "create_node", "id": "node-empty-2", "kind": "card", "text": "B", "width": 200, "height": 80 }
                    ]
                }
            })),
        ),
        ActorKind::McpClient,
    );
    assert_eq!(second.status, ActivityStatus::Ok);
    assert_eq!(second.idempotency_key, None);
    assert_eq!(
        second.response.result.as_ref().unwrap()["structuredContent"]["version"],
        3
    );
    assert_eq!(
        second.response.result.as_ref().unwrap()["structuredContent"]["idempotentReplay"],
        false
    );
}

#[test]
fn t12_codex_toml_non_ascii_utf8_multiline_literal_and_bare_scalar_no_panic() {
    // Valid TOML containing multi-byte UTF-8 inside '''...''' MUST merge cleanly without panicking
    let utf8_multiline = "# Tiếng Việt\nnote = '''Xin chào thế giới — kiểm tra ệ và 🚀'''\n\n[mcp_servers.github]\ncommand = \"npx\"\nargs = [\"-y\", \"github-mcp\"]\n";
    let merged = merge_codex_toml_config(
        utf8_multiline,
        "cowok",
        &["mcp".into(), "serve".into()],
        "http://127.0.0.1:4171/api/v1/mcp/rpc",
    )
    .expect("must parse non-ASCII UTF-8 inside multiline literal string cleanly");
    assert!(merged.contains("kiểm tra ệ và 🚀"));
    assert!(merged.contains("[mcp_servers.cowok_design]"));
    assert!(merged.contains("[mcp_servers.github]"));

    // Malformed >=19-byte non-ASCII bare scalar whose byte index 10/11 splits a UTF-8 char
    // MUST return HostInstallError::MalformedConfig cleanly without panicking
    let non_ascii_bare = "bad_scalar = aaaaaaaaệaaaaaaaa\n";
    let err = merge_codex_toml_config(
        non_ascii_bare,
        "cowok",
        &["mcp".into()],
        "http://127.0.0.1:4171/api/v1/mcp/rpc",
    )
    .expect_err("must reject non-ASCII bare scalar without panicking");
    assert!(matches!(err, HostInstallError::MalformedConfig { .. }));
}
