// @story 12.2
// @implements AC-01, AC-02, AC-03, AC-04 (TDD Security & Functional Suite: HMAC Capability Trust Boundary, Ed25519 Sidecar/Updater Integrity, O_NOFOLLOW + nlink==1 Extraction Guard, 5-Host Universal Plugin Pack, Secret-Preserving Drift Repair & Tauri v2 Bundle Verifier)
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ed25519_dalek::SigningKey;
use harness_daemon::cli::commands::{execute_cli_command, format_json_error, format_json_success};
use harness_daemon::cli::CliCommand;
use harness_daemon::plugin_pack::{
    execute_universal_install, install_universal_pack, load_canonical_manifest,
    validate_canonical_manifest, AGENTS_MD_BEGIN_MARKER, AGENTS_MD_END_MARKER,
    CANONICAL_FEEDBACK_TOOL, CANONICAL_HUMAN_ONLY_TOOLS, MAX_EXISTING_CONFIG_BYTES,
};
use harness_daemon::routes::distribution_ops_crypto::verifying_key_base64;
use harness_daemon::tauri_trust::{
    extract_verified_entries_jailed, sha256_hex, sign_sidecar_integrity_manifest,
    verify_sidecar_and_updater_integrity, CapabilityError, CapabilityScope,
    CapabilityVerifyOptions, SidecarArtifactEntry, SidecarArtifactKind, SidecarIntegrityManifest,
    TrustBoundaryError, VerifiedArchiveEntry, WorkspaceCapabilityRegistry,
};
use serde_json::Value;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn make_signed_test_manifest(
    signing_key: &SigningKey,
    artifact_path: &Path,
    relative_path: &str,
    release_seq: i64,
    issued_at: i64,
    expires_at: i64,
) -> harness_daemon::routes::distribution_ops_crypto::SignedManifestEnvelope<SidecarIntegrityManifest>
{
    let bytes = fs::read(artifact_path).expect("read artifact");
    let manifest = SidecarIntegrityManifest {
        channel: "desktop".to_string(),
        app_version: "0.1.0".to_string(),
        min_compatible_version: "0.1.0".to_string(),
        release_seq,
        artifacts: vec![SidecarArtifactEntry {
            artifact_id: "cowok-design-sidecar".to_string(),
            target_triple: "aarch64-apple-darwin".to_string(),
            relative_path: relative_path.to_string(),
            byte_size: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            kind: SidecarArtifactKind::SidecarBinary,
        }],
    };
    sign_sidecar_integrity_manifest(
        signing_key,
        "cowok-desktop-ed25519-test",
        issued_at,
        expires_at,
        manifest,
    )
    .expect("sign sidecar integrity manifest")
}

#[test]
fn ut_12_2_01_hmac_capability_issue_verify_and_raw_path_redaction() {
    let tmp = TempDir::new().expect("tempdir");
    let sensitive_parent = tmp.path().join("private-org-parent-secret");
    let workspace = sensitive_parent.join("acme-brand-kit");
    fs::create_dir_all(&workspace).expect("create workspace");
    let canon_ws = workspace.canonicalize().expect("canon workspace");

    let registry = WorkspaceCapabilityRegistry::new();
    let now = 1_760_000_000i64;
    let token = registry
        .issue_capability(&workspace, CapabilityScope::WorkspaceWrite, Some(600), now)
        .expect("issue capability token");

    assert!(token.handle_id.starts_with("cap_"));
    assert_eq!(token.display_label, "acme-brand-kit");

    let serialized = serde_json::to_string(&token).expect("serialize token");
    let canon_ws_str = canon_ws.to_string_lossy();
    assert!(
        !serialized.contains(canon_ws_str.as_ref()),
        "raw canonical path leaked in token JSON: {serialized}"
    );
    assert!(
        !serialized.contains("private-org-parent-secret"),
        "parent host segment leaked in token JSON: {serialized}"
    );

    // Rust main resolves exact canonical workspace path for Write and downgraded Read
    let resolved_write = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceWrite,
                single_use: false,
                now_unix: now + 10,
            },
        )
        .expect("verify write scope");
    assert_eq!(resolved_write, canon_ws);

    let resolved_read = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceRead,
                single_use: false,
                now_unix: now + 20,
            },
        )
        .expect("verify downgraded read scope");
    assert_eq!(resolved_read, canon_ws);
}

#[test]
fn ut_12_2_02_hmac_capability_forgery_tampering_and_wrong_scope() {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path().join("project-alpha");
    fs::create_dir_all(&ws).expect("create ws");

    let registry = WorkspaceCapabilityRegistry::new();
    let now = 1_760_000_100i64;
    let token = registry
        .issue_capability(&ws, CapabilityScope::WorkspaceRead, Some(300), now)
        .expect("issue read token");

    // 1-bit flip in HMAC tag
    let mut flipped_mac = token.clone();
    let first = flipped_mac.hmac_tag_hex.as_bytes()[0];
    let replacement = if first == b'a' { "b" } else { "a" };
    flipped_mac.hmac_tag_hex.replace_range(0..1, replacement);
    let err = registry
        .verify_and_resolve(
            &flipped_mac,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceRead,
                single_use: false,
                now_unix: now + 5,
            },
        )
        .expect_err("flipped MAC must fail closed");
    assert_eq!(err.code(), "HMAC_SIGNATURE_MISMATCH");

    // Tampered display_label
    let mut tampered_label = token.clone();
    tampered_label.display_label = "evil-label".to_string();
    let err = registry
        .verify_and_resolve(
            &tampered_label,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceRead,
                single_use: false,
                now_unix: now + 5,
            },
        )
        .expect_err("tampered display label must fail");
    assert_eq!(err.code(), "HMAC_SIGNATURE_MISMATCH");

    // Tampered handle_id
    let mut tampered_handle = token.clone();
    tampered_handle.handle_id = format!("cap_{}", "0".repeat(32));
    let err = registry
        .verify_and_resolve(
            &tampered_handle,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceRead,
                single_use: false,
                now_unix: now + 5,
            },
        )
        .expect_err("tampered handle must fail");
    assert_eq!(err.code(), "HMAC_SIGNATURE_MISMATCH");

    // Scope escalation (WorkspaceRead used for WorkspaceWrite)
    let err = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceWrite,
                single_use: false,
                now_unix: now + 5,
            },
        )
        .expect_err("scope escalation must fail");
    assert_eq!(err.code(), "SCOPE_MISMATCH");
}

#[test]
fn ut_12_2_03_hmac_capability_expiry_and_replay_rejection() {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path().join("expiry-replay-ws");
    fs::create_dir_all(&ws).expect("create ws");

    let registry = WorkspaceCapabilityRegistry::new();
    let now = 1_760_001_000i64;
    let token = registry
        .issue_capability(&ws, CapabilityScope::WorkspaceWrite, Some(120), now)
        .expect("issue token");

    // Exact expiry boundary (expires_at == now_unix) fails closed
    let err = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceWrite,
                single_use: false,
                now_unix: now + 120,
            },
        )
        .expect_err("exact boundary must expire");
    assert_eq!(err.code(), "CAPABILITY_EXPIRED");

    // Clock skew rejection
    let err = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceWrite,
                single_use: false,
                now_unix: now - 60,
            },
        )
        .expect_err("future issued_at beyond skew must fail");
    assert_eq!(err.code(), "CLOCK_SKEW_REJECTED");

    // Single-use nonce replay detection
    let _first = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceWrite,
                single_use: true,
                now_unix: now + 10,
            },
        )
        .expect("first single-use verification passes");
    let replay_err = registry
        .verify_and_resolve(
            &token,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceWrite,
                single_use: true,
                now_unix: now + 11,
            },
        )
        .expect_err("replayed single-use nonce must fail");
    assert_eq!(replay_err.code(), "CAPABILITY_REPLAY_DETECTED");

    // Post-revocation rejection
    let revocable = registry
        .issue_capability(&ws, CapabilityScope::WorkspaceRead, Some(120), now)
        .expect("issue revocable token");
    assert!(registry.revoke_handle(&revocable.handle_id));
    let revoked_err = registry
        .verify_and_resolve(
            &revocable,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::WorkspaceRead,
                single_use: false,
                now_unix: now + 5,
            },
        )
        .expect_err("revoked handle must fail");
    assert_eq!(revoked_err.code(), "CAPABILITY_REVOKED");
}

#[test]
fn ut_12_2_04_ed25519_sidecar_and_updater_integrity_pass_and_tamper() {
    let tmp = TempDir::new().expect("tempdir");
    let bin_dir = tmp.path().join("binaries");
    fs::create_dir_all(&bin_dir).expect("create binaries dir");
    let sidecar_path = bin_dir.join("cowok-design");
    fs::write(&sidecar_path, b"#!/bin/sh\necho cowok-design-v0.1.0\n").expect("write sidecar");

    let trusted_sk = SigningKey::from_bytes(&[0x42u8; 32]);
    let trusted_vk = trusted_sk.verifying_key();
    let untrusted_sk = SigningKey::from_bytes(&[0x99u8; 32]);

    let now = 1_760_005_000i64;
    let envelope = make_signed_test_manifest(
        &trusted_sk,
        &sidecar_path,
        "binaries/cowok-design",
        5,
        now,
        now + 3600,
    );

    // Happy path
    let receipts =
        verify_sidecar_and_updater_integrity(&envelope, &trusted_vk, tmp.path(), 4, now + 10)
            .expect("valid signed sidecar must pass");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].verified);
    assert!(receipts[0].no_follow_checked);
    assert_eq!(receipts[0].nlink_verified, 1);

    // Anti-rollback attack (manifest_seq <= last_applied_seq)
    let rollback_err =
        verify_sidecar_and_updater_integrity(&envelope, &trusted_vk, tmp.path(), 5, now + 10)
            .expect_err("equal or lower sequence must fail rollback check");
    assert_eq!(rollback_err.code(), "ROLLBACK_ATTACK_DETECTED");

    // Untrusted key swap
    let rogue_envelope = make_signed_test_manifest(
        &untrusted_sk,
        &sidecar_path,
        "binaries/cowok-design",
        6,
        now,
        now + 3600,
    );
    let key_err =
        verify_sidecar_and_updater_integrity(&rogue_envelope, &trusted_vk, tmp.path(), 5, now + 10)
            .expect_err("untrusted signer must fail closed");
    assert_eq!(key_err.code(), "TAMPERED_SIGNATURE");

    // 1-byte binary tamper (same length, different byte)
    fs::write(&sidecar_path, b"#!/bin/sh\necho cowok-design-v0.1.9\n").expect("tamper sidecar");
    let sha_err =
        verify_sidecar_and_updater_integrity(&envelope, &trusted_vk, tmp.path(), 4, now + 10)
            .expect_err("tampered binary must fail SHA-256 verification");
    assert_eq!(sha_err.code(), "SHA256_MISMATCH");
}

#[cfg(unix)]
#[test]
fn ut_12_2_05_nofollow_and_nlink_hardlink_symlink_extraction_rejection() {
    use std::os::unix::fs::symlink;

    let tmp = TempDir::new().expect("tempdir");
    let outside = TempDir::new().expect("outside dir");
    let outside_secret = outside.path().join("outside-file.bin");
    fs::write(&outside_secret, b"external-payload-bytes").expect("write outside");

    let trusted_sk = SigningKey::from_bytes(&[0x33u8; 32]);
    let trusted_vk = trusted_sk.verifying_key();
    let now = 1_760_010_000i64;

    // (a) External-target symlink sidecar rejected via O_NOFOLLOW
    let symlink_target = tmp.path().join("linked-sidecar");
    symlink(&outside_secret, &symlink_target).expect("create symlink");
    let symlink_env = make_signed_test_manifest(
        &trusted_sk,
        &outside_secret,
        "linked-sidecar",
        1,
        now,
        now + 600,
    );
    let err =
        verify_sidecar_and_updater_integrity(&symlink_env, &trusted_vk, tmp.path(), 0, now + 1)
            .expect_err("symlink artifact must be rejected");
    assert_eq!(err.code(), "SYMLINK_REJECTED");

    // (b) Hardlink (nlink == 2) rejected
    let real_bin = tmp.path().join("real-sidecar.bin");
    fs::write(&real_bin, b"real-sidecar-bytes").expect("write real bin");
    let hardlinked = tmp.path().join("hardlinked-sidecar.bin");
    fs::hard_link(&real_bin, &hardlinked).expect("create hardlink");
    let hl_env = make_signed_test_manifest(
        &trusted_sk,
        &hardlinked,
        "hardlinked-sidecar.bin",
        2,
        now,
        now + 600,
    );
    let err = verify_sidecar_and_updater_integrity(&hl_env, &trusted_vk, tmp.path(), 1, now + 1)
        .expect_err("hardlinked artifact must be rejected");
    assert!(matches!(
        err,
        TrustBoundaryError::HardlinkRejected { nlink: 2 }
    ));
    assert_eq!(err.code(), "HARDLINK_REJECTED");

    // (c) Jailed extraction rejects `../escape`, symlinked parent directory, and hardlinked target
    let sandbox = tmp.path().join("extract-sandbox");
    fs::create_dir_all(&sandbox).expect("create sandbox");

    let traversal_err = extract_verified_entries_jailed(
        &sandbox,
        &[VerifiedArchiveEntry {
            relative_path: "../escaped.txt".to_string(),
            bytes: b"pwn".to_vec(),
            expected_sha256: None,
            unix_mode: 0o644,
        }],
    )
    .expect_err("path traversal must be denied");
    assert_eq!(traversal_err.code(), "PATH_TRAVERSAL_DENIED");
    assert!(!tmp.path().join("escaped.txt").exists());

    let sym_dir = sandbox.join("linked-dir");
    symlink(outside.path(), &sym_dir).expect("create symlinked dir");
    let sym_dir_err = extract_verified_entries_jailed(
        &sandbox,
        &[VerifiedArchiveEntry {
            relative_path: "linked-dir/injected.txt".to_string(),
            bytes: b"pwn".to_vec(),
            expected_sha256: None,
            unix_mode: 0o644,
        }],
    )
    .expect_err("symlinked parent dir must be rejected");
    assert_eq!(sym_dir_err.code(), "SYMLINK_REJECTED");
    assert!(!outside.path().join("injected.txt").exists());

    // Pre-existing hardlink inside sandbox rejected
    let existing_target = sandbox.join("preexisting.txt");
    fs::write(&existing_target, b"original").expect("write original");
    let alias_hardlink = sandbox.join("alias.txt");
    fs::hard_link(&existing_target, &alias_hardlink).expect("create sandbox hardlink");
    let hl_extract_err = extract_verified_entries_jailed(
        &sandbox,
        &[VerifiedArchiveEntry {
            relative_path: "preexisting.txt".to_string(),
            bytes: b"overwrite".to_vec(),
            expected_sha256: None,
            unix_mode: 0o644,
        }],
    )
    .expect_err("hardlinked destination file must be rejected");
    assert_eq!(hl_extract_err.code(), "HARDLINK_REJECTED");
}

#[test]
fn ut_12_2_06_universal_pack_emits_all_five_hosts_from_canonical_manifest() {
    let tmp = TempDir::new().expect("tempdir");
    let install_root = tmp.path().join("install-root");
    let workspace = tmp.path().join("workspace");
    fs::create_dir_all(&workspace).expect("create workspace");

    let report = install_universal_pack("all", &install_root, &workspace, None)
        .expect("install all 5 hosts");
    assert_eq!(report.targets.len(), 5);
    assert!(report.total_files_written >= 35);

    // 1. Cowok.ai
    assert!(install_root.join("plugin.json").is_file());
    assert!(install_root.join(".cowok-ai/mcp.json").is_file());
    assert!(install_root
        .join("skills/phase-0-intent-framing/SKILL.md")
        .is_file());
    assert!(install_root
        .join("skills/craft/impeccable-polish/SKILL.md")
        .is_file());

    // 2. Claude Code / Desktop
    assert!(install_root.join(".claude-plugin/plugin.json").is_file());
    assert!(install_root.join(".mcp.json").is_file());
    assert!(install_root.join("agents/design-orchestrator.md").is_file());

    // 3. OpenAI Codex CLI
    assert!(install_root.join(".codex/config.toml").is_file());
    assert!(install_root
        .join(".codex/skills/phase-2-brand-visual-studio/SKILL.md")
        .is_file());
    assert!(install_root.join("AGENTS.md").is_file());

    // 4. Google Antigravity IDE
    assert!(install_root
        .join("antigravity-plugin/plugin.json")
        .is_file());
    assert!(install_root.join("antigravity-plugin/.mcp.json").is_file());
    assert!(install_root
        .join("antigravity-plugin/skills/phase-4-verify-handoff/SKILL.md")
        .is_file());
    assert!(install_root
        .join("antigravity-plugin/agents/design-auditor.md")
        .is_file());

    // 5. Cursor
    assert!(install_root.join(".cursor/mcp.json").is_file());
    assert!(install_root
        .join(".cursor/rules/cowok-design.mdc")
        .is_file());
    assert!(install_root
        .join(".cursor/rules/cowok-security-human-only.mdc")
        .is_file());

    for target in &report.targets {
        assert_eq!(target.skills_emitted, 7);
    }
}

#[test]
fn ut_12_2_07_human_only_gates_telemetry_channels_and_feedback_tool_preserved() {
    let tmp = TempDir::new().expect("tempdir");
    let install_root = tmp.path().join("all-hosts");
    let workspace = tmp.path().join("ws");
    fs::create_dir_all(&workspace).expect("create ws");

    let report =
        install_universal_pack("all", &install_root, &workspace, None).expect("install all hosts");

    let expected_channels = [
        ("cowok", "cowok_ai", ".cowok-ai/mcp.json"),
        ("claude", "claude_code", ".mcp.json"),
        ("antigravity", "antigravity", "antigravity-plugin/.mcp.json"),
        ("cursor", "cursor", ".cursor/mcp.json"),
    ];

    for (target_name, channel, mcp_rel) in expected_channels {
        let t = report
            .targets
            .iter()
            .find(|r| r.target == target_name)
            .expect("target report present");
        assert_eq!(t.channel, channel);
        assert!(t.feedback_tool_exposed);
        for locked in CANONICAL_HUMAN_ONLY_TOOLS {
            assert!(t.human_only_tools_preserved.iter().any(|h| h == locked));
        }

        let raw = fs::read_to_string(install_root.join(mcp_rel)).expect("read mcp json");
        let parsed: Value = serde_json::from_str(&raw).expect("parse mcp json");
        let entry = &parsed["mcpServers"]["cowok-design"];
        assert_eq!(entry["env"]["COWOK_TELEMETRY_CHANNEL"], channel);
        assert_eq!(entry["feedbackTool"], CANONICAL_FEEDBACK_TOOL);
        let human_only_arr = entry["humanOnlyTools"].as_array().expect("humanOnlyTools");
        for locked in CANONICAL_HUMAN_ONLY_TOOLS {
            assert!(human_only_arr.iter().any(|v| v == locked));
        }
    }

    // Verify Codex TOML + AGENTS.md
    let codex_toml = fs::read_to_string(install_root.join(".codex/config.toml")).expect("toml");
    assert!(codex_toml.contains("COWOK_TELEMETRY_CHANNEL = \"codex\""));
    assert!(codex_toml.contains("COWOK_FEEDBACK_TOOL = \"cowok_submit_feedback\""));
    for locked in CANONICAL_HUMAN_ONLY_TOOLS {
        assert!(codex_toml.contains(locked));
    }
    let agents_md = fs::read_to_string(install_root.join("AGENTS.md")).expect("agents md");
    assert!(agents_md.contains(CANONICAL_FEEDBACK_TOOL));
    assert!(agents_md.contains("`codex`"));
    for locked in CANONICAL_HUMAN_ONLY_TOOLS {
        assert!(agents_md.contains(locked));
    }

    // Verify tampered HUMAN_ONLY manifest fails closed
    let mut bad_manifest = load_canonical_manifest().expect("load canonical");
    if let Some(tool) = bad_manifest
        .mcp_tools
        .iter_mut()
        .find(|t| t.tool_name == "ephemeral_tool_forge")
    {
        tool.mcp_callable = true;
    }
    let err = validate_canonical_manifest(&bad_manifest)
        .expect_err("enabling HUMAN_ONLY tool must fail closed");
    assert_eq!(err.code(), "SECURITY_INVARIANT_VIOLATION");
}

#[test]
fn ut_12_2_08_drift_repair_preserves_existing_secrets_and_custom_keys() {
    let tmp = TempDir::new().expect("tempdir");
    let install_root = tmp.path().join("drift-root");
    let workspace = tmp.path().join("moved-workspace");
    fs::create_dir_all(install_root.join(".cursor")).expect("mkdir cursor");
    fs::create_dir_all(install_root.join(".codex")).expect("mkdir codex");
    fs::create_dir_all(&workspace).expect("mkdir workspace");

    // Seed stale Cursor MCP config with sibling server & benign fixture tokens
    let stale_cursor = serde_json::json!({
        "customUserSetting": "keep-me-intact",
        "mcpServers": {
            "github-mcp": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-github"],
                "env": {
                    "GITHUB_TOKEN": "fixture-github-token-12208"
                }
            },
            "cowok-design": {
                "command": "/old/stale/bin/cowok-design",
                "args": ["mcp", "serve"],
                "env": {
                    "COWOK_TELEMETRY_CHANNEL": "cli",
                    "COWOK_WORKSPACE_ROOT": "/old/deleted/workspace",
                    "USER_CUSTOM_EXTRA": "preserve-this-env"
                }
            }
        }
    });
    fs::write(
        install_root.join(".cursor/mcp.json"),
        serde_json::to_string_pretty(&stale_cursor).expect("serialize cursor"),
    )
    .expect("write stale cursor");

    // Seed stale Codex TOML & AGENTS.md with custom sibling server & user prose
    let stale_codex_toml = r#"# User custom Codex config
model = "o3"

[mcp_servers.openai_tools]
command = "openai-mcp"
args = ["serve"]

[mcp_servers.openai_tools.env]
OPENAI_API_KEY = "fixture-openai-token-12208"

[mcp_servers.cowok_design]
command = "/old/stale/cowok-design"
args = ["mcp", "serve"]
url = "http://127.0.0.1:4170/api/v1/mcp/rpc"
transport = "stdio"

[mcp_servers.cowok_design.env]
COWOK_WORKSPACE_ROOT = "/old/deleted/workspace"
"#;
    fs::write(install_root.join(".codex/config.toml"), stale_codex_toml)
        .expect("write stale codex toml");

    let stale_agents_md = format!(
        "# Team Custom Instructions\n\nNever delete our custom design guidelines.\n\n{AGENTS_MD_BEGIN_MARKER}\nstale cowok block\n{AGENTS_MD_END_MARKER}\n\n## Footer Notes\nKeep footer intact.\n"
    );
    fs::write(install_root.join("AGENTS.md"), stale_agents_md).expect("write stale agents md");

    let cursor_report = install_universal_pack("cursor", &install_root, &workspace, None)
        .expect("repair cursor drift");
    assert!(cursor_report.any_drift_repaired);
    let c_target = &cursor_report.targets[0];
    assert!(c_target.drift_detected);
    assert!(c_target.drift_repaired);
    assert!(c_target.preserved_keys_count >= 2);
    assert!(c_target.backup_id.is_some());

    let repaired_cursor: Value = serde_json::from_str(
        &fs::read_to_string(install_root.join(".cursor/mcp.json")).expect("read repaired cursor"),
    )
    .expect("parse repaired cursor");
    assert_eq!(repaired_cursor["customUserSetting"], "keep-me-intact");
    assert_eq!(
        repaired_cursor["mcpServers"]["github-mcp"]["env"]["GITHUB_TOKEN"],
        "fixture-github-token-12208"
    );
    assert_eq!(
        repaired_cursor["mcpServers"]["cowok-design"]["command"],
        "cowok-design"
    );
    assert_eq!(
        repaired_cursor["mcpServers"]["cowok-design"]["env"]["COWOK_TELEMETRY_CHANNEL"],
        "cursor"
    );
    assert_eq!(
        repaired_cursor["mcpServers"]["cowok-design"]["env"]["USER_CUSTOM_EXTRA"],
        "preserve-this-env"
    );

    let codex_report = install_universal_pack("codex", &install_root, &workspace, None)
        .expect("repair codex drift");
    assert!(codex_report.any_drift_repaired);
    let x_target = &codex_report.targets[0];
    assert!(x_target.drift_detected);
    assert!(x_target.drift_repaired);
    assert!(x_target.backup_id.is_some());

    let repaired_toml =
        fs::read_to_string(install_root.join(".codex/config.toml")).expect("read repaired toml");
    assert!(repaired_toml.contains("OPENAI_API_KEY = \"fixture-openai-token-12208\""));
    assert!(repaired_toml.contains("[mcp_servers.openai_tools]"));
    assert!(repaired_toml.contains("command = \"cowok-design\""));
    assert!(!repaired_toml.contains("/old/stale/cowok-design"));

    let repaired_agents =
        fs::read_to_string(install_root.join("AGENTS.md")).expect("read repaired agents");
    assert!(repaired_agents.contains("Never delete our custom design guidelines."));
    assert!(repaired_agents.contains("Keep footer intact."));
    assert!(!repaired_agents.contains("stale cowok block"));
    assert!(repaired_agents.contains("cowok_submit_feedback"));
}

#[test]
fn ut_12_2_09_cli_install_subcommand_json_envelope_and_symlink_fail_closed() {
    let tmp = TempDir::new().expect("tempdir");
    let out_dir = tmp.path().join("cli-out");
    let ws = tmp.path().join("cli-ws");
    fs::create_dir_all(&ws).expect("create ws");

    let batch = execute_universal_install("all", &out_dir, &ws, None)
        .expect("execute_universal_install all");
    let envelope_json = format_json_success(&batch);
    let parsed: Value = serde_json::from_str(&envelope_json).expect("parse success envelope");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["data"]["targets"].as_array().map(Vec::len), Some(5));
    assert!(parsed["requestId"].as_str().is_some_and(|s| !s.is_empty()));

    // Invalid target fails closed
    let invalid_err = execute_universal_install("unknown-host", &out_dir, &ws, None)
        .expect_err("invalid target must fail");
    assert!(invalid_err.contains("INVALID_INSTALL_TARGET"));
    let err_env: Value = serde_json::from_str(&format_json_error(
        "INVALID_INSTALL_TARGET",
        &invalid_err,
        Some("target"),
    ))
    .expect("parse error envelope");
    assert_eq!(err_env["error"]["code"], "INVALID_INSTALL_TARGET");

    // Symlinked install root fails closed on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let real_outside = tmp.path().join("real-outside");
        fs::create_dir_all(&real_outside).expect("mkdir real_outside");
        let symlink_out = tmp.path().join("symlink-out");
        symlink(&real_outside, &symlink_out).expect("symlink out");
        let sym_err = execute_universal_install("cursor", &symlink_out, &ws, None)
            .expect_err("symlinked install_root must fail closed");
        assert!(sym_err.contains("SYMLINK_REJECTED"));
        assert!(!real_outside.join(".cursor/mcp.json").exists());
    }
}

#[test]
fn ut_12_2_10_tauri_bundle_config_capabilities_and_verifier_script() {
    let root = repo_root();
    let tauri_conf: Value = serde_json::from_str(
        &fs::read_to_string(root.join("packaging/tauri/tauri.conf.json"))
            .expect("read tauri.conf.json"),
    )
    .expect("parse tauri.conf.json");

    let targets = tauri_conf["bundle"]["targets"]
        .as_array()
        .expect("bundle.targets array");
    for expected in ["dmg", "msi", "appimage"] {
        assert!(targets.iter().any(|v| v == expected));
    }
    assert_eq!(
        tauri_conf["bundle"]["externalBin"][0],
        "binaries/cowok-design"
    );
    assert_eq!(tauri_conf["app"]["withGlobalTauri"], false);
    assert_eq!(tauri_conf["app"]["windows"][0]["dragDropEnabled"], true);
    assert_eq!(tauri_conf["plugins"]["updater"]["active"], true);

    let capabilities: Value = serde_json::from_str(
        &fs::read_to_string(root.join("packaging/tauri/capabilities/default.json"))
            .expect("read capabilities/default.json"),
    )
    .expect("parse capabilities/default.json");
    let perms_str = serde_json::to_string(&capabilities["permissions"]).expect("serialize perms");
    assert!(
        !perms_str.contains("fs:allow-"),
        "raw fs:allow-* permissions are forbidden in Tauri v2 capabilities"
    );

    let root_cargo = fs::read_to_string(root.join("Cargo.toml")).expect("read root Cargo.toml");
    assert!(
        !root_cargo.contains("packaging/tauri"),
        "packaging/tauri must remain excluded from root workspace members"
    );

    let verifier_script = root.join("packaging/tauri/scripts/verify-desktop-bundle.mjs");
    let output = Command::new("node")
        .arg(&verifier_script)
        .arg("--check-manifest")
        .output()
        .expect("run verify-desktop-bundle.mjs");
    assert!(
        output.status.success(),
        "verify-desktop-bundle.mjs failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed_out: Value =
        serde_json::from_slice(&output.stdout).expect("parse verifier script JSON");
    assert_eq!(parsed_out["ok"], true);
}

#[test]
fn ut_12_2_11_f01_f02_f07_f08_tauri_ipc_capability_csp_and_build_rs_hardening() {
    let root = repo_root();

    // F-08: packaging/tauri/build.rs exists and invokes tauri_build::build()
    let build_rs = fs::read_to_string(root.join("packaging/tauri/build.rs"))
        .expect("packaging/tauri/build.rs must exist");
    assert!(build_rs.contains("tauri_build::build()"));

    // F-01: capabilities/default.json removes dialog:allow-open and pins literal shell args
    let capabilities: Value = serde_json::from_str(
        &fs::read_to_string(root.join("packaging/tauri/capabilities/default.json"))
            .expect("read capabilities/default.json"),
    )
    .expect("parse capabilities/default.json");
    let perms = capabilities["permissions"]
        .as_array()
        .expect("permissions array");
    for perm in perms {
        if let Some(s) = perm.as_str() {
            assert_ne!(s, "dialog:allow-open");
            assert_ne!(s, "shell:allow-execute");
            assert_ne!(s, "shell:allow-spawn");
            assert_ne!(s, "shell:allow-stdin-write");
            assert!(!s.starts_with("fs:"));
        } else if let Some(obj) = perm.as_object() {
            if obj.get("identifier").and_then(Value::as_str) == Some("shell:allow-execute") {
                let allow = obj["allow"].as_array().expect("allow array");
                assert_eq!(allow.len(), 1);
                let args = allow[0]["args"].as_array().expect("literal args array");
                assert!(!args.is_empty());
                assert!(args.iter().all(|a| a.is_string()));
            }
        }
    }

    // F-01 & F-02: packaging/tauri/src/main.rs never accepts raw paths or WebView pubkey
    let tauri_main =
        fs::read_to_string(root.join("packaging/tauri/src/main.rs")).expect("read tauri main.rs");
    assert!(!tauri_main.contains("fn issue_workspace_capability"));
    assert!(!tauri_main.contains("fn import_dropped_asset_capability"));
    assert!(!tauri_main.contains("selected_path: String"));
    assert!(!tauri_main.contains("dropped_path: String"));
    assert!(!tauri_main.contains("trusted_pubkey_b64"));
    assert!(tauri_main.contains("fn pick_workspace_capability"));
    assert!(tauri_main.contains("blocking_pick_folder()"));
    assert!(tauri_main.contains("CapabilityScope::WorkspaceRead"));
    assert!(tauri_main.contains("WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. })"));
    assert!(tauri_main.contains("cowok://dropped-capabilities"));
    assert!(tauri_main.contains("fn list_dropped_capabilities"));
    assert!(tauri_main.contains("option_env!(\"COWOK_RELEASE_ED25519_PUBKEY_B64\")"));
    assert!(tauri_main.contains("UPDATER_PUBKEY_NOT_PINNED"));

    // F-07: Strict CSP in packaging/tauri/tauri.conf.json
    let tauri_conf: Value = serde_json::from_str(
        &fs::read_to_string(root.join("packaging/tauri/tauri.conf.json"))
            .expect("read tauri.conf.json"),
    )
    .expect("parse tauri.conf.json");
    let csp = tauri_conf["app"]["security"]["csp"]
        .as_str()
        .expect("csp string");
    assert_eq!(
        csp,
        "default-src 'self'; script-src 'self'; worker-src 'self'; frame-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: http://127.0.0.1:*; connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*; object-src 'none'; base-uri 'self'"
    );
}

#[test]
fn ut_12_2_12_f03_f08_verify_desktop_bundle_cli_and_signed_fail_closed() {
    let root = repo_root();
    let script = root.join("packaging/tauri/scripts/verify-desktop-bundle.mjs");

    // F-08: Unknown CLI flag must fail closed with exit code 2
    let unknown_out = Command::new("node")
        .arg(&script)
        .arg("--bogus-flag")
        .output()
        .expect("run unknown flag");
    assert_eq!(unknown_out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown_out.stderr).contains("UNKNOWN_CLI_FLAG"));

    // F-08: --release rejects placeholder pubkey
    let release_out = Command::new("node")
        .arg(&script)
        .arg("--release")
        .output()
        .expect("run --release");
    assert!(!release_out.status.success());
    assert!(String::from_utf8_lossy(&release_out.stderr).contains("PLACEHOLDER_UPDATER_PUBKEY"));

    // F-08: --sidecar-bin also rejects placeholder pubkey
    let tmp = TempDir::new().expect("tempdir");
    let dummy_bin = tmp.path().join("cowok-design");
    fs::write(&dummy_bin, b"#!/bin/sh\necho ok\n").expect("write dummy bin");
    let sidecar_out = Command::new("node")
        .arg(&script)
        .arg("--sidecar-bin")
        .arg(&dummy_bin)
        .output()
        .expect("run --sidecar-bin");
    assert!(!sidecar_out.status.success());
    assert!(String::from_utf8_lossy(&sidecar_out.stderr).contains("PLACEHOLDER_UPDATER_PUBKEY"));

    // F-03: --verify-signed happy path + anti-rollback + expiry + schema + channel + byteSize + symlink
    let bin_dir = tmp.path().join("binaries");
    fs::create_dir_all(&bin_dir).expect("mkdir binaries");
    let sidecar = bin_dir.join("cowok-design");
    fs::write(&sidecar, b"cowok-sidecar-payload-v1").expect("write sidecar");

    let sk = SigningKey::from_bytes(&[0x51u8; 32]);
    let pk_b64 = verifying_key_base64(&sk.verifying_key());
    let now = chrono::Utc::now().timestamp();
    let valid_env =
        make_signed_test_manifest(&sk, &sidecar, "binaries/cowok-design", 5, now, now + 3600);
    let manifest_path = tmp.path().join("valid-manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&valid_env).expect("ser valid"),
    )
    .expect("write valid manifest");

    // Happy path with --min-seq 4
    let ok_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&manifest_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .arg("--min-seq")
        .arg("4")
        .output()
        .expect("run verify-signed ok");
    assert!(
        ok_out.status.success(),
        "expected verify-signed pass, stderr={}",
        String::from_utf8_lossy(&ok_out.stderr)
    );

    // Anti-rollback with --min-seq 5 fails closed
    let rb_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&manifest_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .arg("--min-seq")
        .arg("5")
        .output()
        .expect("run rollback check");
    assert!(!rb_out.status.success());
    assert!(String::from_utf8_lossy(&rb_out.stderr).contains("ROLLBACK_ATTACK_DETECTED"));

    // Expired manifest fails closed
    let expired_env = make_signed_test_manifest(
        &sk,
        &sidecar,
        "binaries/cowok-design",
        6,
        now - 120,
        now - 10,
    );
    let expired_path = tmp.path().join("expired-manifest.json");
    fs::write(
        &expired_path,
        serde_json::to_string_pretty(&expired_env).expect("ser expired"),
    )
    .expect("write expired");
    let exp_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&expired_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .output()
        .expect("run expired check");
    assert!(!exp_out.status.success());
    assert!(String::from_utf8_lossy(&exp_out.stderr).contains("EXPIRED_MANIFEST"));

    // Wrong schemaVersion fails closed
    let mut bad_schema_val: Value = serde_json::to_value(&valid_env).expect("to value");
    bad_schema_val["schemaVersion"] = serde_json::json!(2);
    let bad_schema_path = tmp.path().join("bad-schema.json");
    fs::write(&bad_schema_path, bad_schema_val.to_string()).expect("write bad schema");
    let bs_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&bad_schema_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .output()
        .expect("run bad schema");
    assert!(!bs_out.status.success());
    assert!(String::from_utf8_lossy(&bs_out.stderr).contains("INVALID_SCHEMA_VERSION"));

    // Empty artifacts fails closed
    let mut empty_arts: Value = serde_json::to_value(&valid_env).expect("to value");
    empty_arts["payload"]["artifacts"] = serde_json::json!([]);
    let empty_arts_path = tmp.path().join("empty-artifacts.json");
    fs::write(&empty_arts_path, empty_arts.to_string()).expect("write empty artifacts");
    let ea_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&empty_arts_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .output()
        .expect("run empty artifacts");
    assert!(!ea_out.status.success());
    assert!(String::from_utf8_lossy(&ea_out.stderr).contains("EMPTY_ARTIFACTS"));

    // Backslash path traversal rejected
    let backslash_env =
        make_signed_test_manifest(&sk, &sidecar, "binaries\\cowok-design", 7, now, now + 3600);
    let bs_rel_path = tmp.path().join("backslash-manifest.json");
    fs::write(
        &bs_rel_path,
        serde_json::to_string_pretty(&backslash_env).expect("ser bs"),
    )
    .expect("write bs");
    let bsr_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&bs_rel_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .output()
        .expect("run backslash check");
    assert!(!bsr_out.status.success());
    assert!(String::from_utf8_lossy(&bsr_out.stderr).contains("PATH_TRAVERSAL_DENIED"));

    // Byte size mismatch rejected
    let wrong_size_manifest = SidecarIntegrityManifest {
        channel: "desktop".to_string(),
        app_version: "0.1.0".to_string(),
        min_compatible_version: "0.1.0".to_string(),
        release_seq: 8,
        artifacts: vec![SidecarArtifactEntry {
            artifact_id: "cowok-design-sidecar".to_string(),
            target_triple: "aarch64-apple-darwin".to_string(),
            relative_path: "binaries/cowok-design".to_string(),
            byte_size: 999,
            sha256: sha256_hex(b"cowok-sidecar-payload-v1"),
            kind: SidecarArtifactKind::SidecarBinary,
        }],
    };
    let wrong_size_env =
        sign_sidecar_integrity_manifest(&sk, "test-kid", now, now + 3600, wrong_size_manifest)
            .expect("sign wrong size");
    let ws_path = tmp.path().join("wrong-size.json");
    fs::write(
        &ws_path,
        serde_json::to_string_pretty(&wrong_size_env).expect("ser ws"),
    )
    .expect("write ws");
    let ws_out = Command::new("node")
        .arg(&script)
        .arg("--verify-signed")
        .arg(&ws_path)
        .arg("--pubkey")
        .arg(&pk_b64)
        .arg("--root")
        .arg(tmp.path())
        .output()
        .expect("run wrong size");
    assert!(!ws_out.status.success());
    assert!(String::from_utf8_lossy(&ws_out.stderr).contains("BYTE_SIZE_MISMATCH"));

    // Symlinked ancestor directory rejected on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = TempDir::new().expect("outside");
        let outside_bin = outside.path().join("cowok-design");
        fs::write(&outside_bin, b"cowok-sidecar-payload-v1").expect("write outside bin");
        symlink(outside.path(), tmp.path().join("linked-bin-dir")).expect("symlink dir");
        let sym_env = make_signed_test_manifest(
            &sk,
            &outside_bin,
            "linked-bin-dir/cowok-design",
            9,
            now,
            now + 3600,
        );
        let sym_path = tmp.path().join("symlink-ancestor.json");
        fs::write(
            &sym_path,
            serde_json::to_string_pretty(&sym_env).expect("ser sym"),
        )
        .expect("write sym");
        let sym_out = Command::new("node")
            .arg(&script)
            .arg("--verify-signed")
            .arg(&sym_path)
            .arg("--pubkey")
            .arg(&pk_b64)
            .arg("--root")
            .arg(tmp.path())
            .output()
            .expect("run symlink ancestor");
        assert!(!sym_out.status.success());
        assert!(String::from_utf8_lossy(&sym_out.stderr).contains("SYMLINK_REJECTED"));
    }
}

#[test]
fn ut_12_2_13_f04_f09_replay_cache_saturation_handle_pruning_and_framed_hmac() {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path().join("replay-bound-ws");
    fs::create_dir_all(&ws).expect("create ws");

    // F-04: Bounded replay cache never evicts unexpired nonces; fails closed with ReplayCacheSaturated
    let registry = WorkspaceCapabilityRegistry::with_limits(2, 3);
    let now = 1_760_100_000i64;

    let t1 = registry
        .issue_capability(&ws, CapabilityScope::ExportArtifact, Some(60), now)
        .expect("issue t1");
    let t2 = registry
        .issue_capability(&ws, CapabilityScope::ExportArtifact, Some(60), now)
        .expect("issue t2");
    let t3 = registry
        .issue_capability(&ws, CapabilityScope::ExportArtifact, Some(120), now)
        .expect("issue t3");

    // Handle registry is at capacity (3/3): issuing a 4th unexpired handle fails closed
    let cap_err = registry
        .issue_capability(&ws, CapabilityScope::WorkspaceRead, Some(60), now)
        .expect_err("saturated handle map must fail closed");
    assert_eq!(cap_err.code(), "HANDLE_REGISTRY_SATURATED");

    // Consume t1 and t2 -> replay cache is at capacity (2/2) with unexpired nonces
    registry
        .verify_and_resolve(
            &t1,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::ExportArtifact,
                single_use: true,
                now_unix: now + 5,
            },
        )
        .expect("consume t1");
    registry
        .verify_and_resolve(
            &t2,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::ExportArtifact,
                single_use: true,
                now_unix: now + 5,
            },
        )
        .expect("consume t2");
    assert_eq!(registry.active_replay_count(), 2);

    // Replaying t1 is STILL detected (never evicted)
    let replay_err = registry
        .verify_and_resolve(
            &t1,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::ExportArtifact,
                single_use: true,
                now_unix: now + 6,
            },
        )
        .expect_err("replayed t1 must fail");
    assert_eq!(replay_err, CapabilityError::CapabilityReplayDetected);

    // Consuming a 3rd unexpired single-use token fails closed with ReplayCacheSaturated
    let sat_err = registry
        .verify_and_resolve(
            &t3,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::ExportArtifact,
                single_use: true,
                now_unix: now + 10,
            },
        )
        .expect_err("saturated replay cache must fail closed");
    assert_eq!(sat_err, CapabilityError::ReplayCacheSaturated);
    assert_eq!(sat_err.code(), "REPLAY_CACHE_SATURATED");

    // Once t1 and t2 expire (now + 65 > now + 60), expired nonces & handles are pruned cleanly
    registry
        .verify_and_resolve(
            &t3,
            &CapabilityVerifyOptions {
                required_scope: CapabilityScope::ExportArtifact,
                single_use: true,
                now_unix: now + 65,
            },
        )
        .expect("t3 succeeds after expired nonces are pruned");
    assert_eq!(registry.active_handle_count(), 1);
    assert_eq!(registry.active_replay_count(), 1);

    // F-09: Control characters and unsafe delimiters in display_label rejected before HMAC
    let t_fresh = registry
        .issue_capability(&ws, CapabilityScope::WorkspaceRead, Some(120), now + 70)
        .expect("issue fresh");
    for bad_label in [
        "ws\ninjected",
        "ws\rinjected",
        "ws\x1binjected",
        "ws|pipe:col",
    ] {
        let mut forged = t_fresh.clone();
        forged.display_label = bad_label.to_string();
        let err = registry
            .verify_and_resolve(
                &forged,
                &CapabilityVerifyOptions {
                    required_scope: CapabilityScope::WorkspaceRead,
                    single_use: false,
                    now_unix: now + 75,
                },
            )
            .expect_err("malformed display_label must be rejected");
        assert_eq!(err.code(), "INVALID_TOKEN_FORMAT");
    }
}

#[test]
fn ut_12_2_14_f05_agents_md_control_char_and_marker_injection_defense() {
    let tmp = TempDir::new().expect("tempdir");
    let install_root = tmp.path().join("install-root");
    let good_ws = tmp.path().join("good-ws");
    fs::create_dir_all(&good_ws).expect("mkdir good_ws");

    // Control characters in workspace_root / install_root / binary_path rejected at entry
    let ctrl_ws = tmp.path().join("evil\n<!-- END COWOK-DESIGN-PACK -->");
    let err = install_universal_pack("codex", &install_root, &ctrl_ws, None)
        .expect_err("newline in workspace_root must fail closed");
    assert_eq!(err.code(), "INVALID_INSTALL_INPUT");

    let ctrl_install = tmp.path().join("evil\rinstall");
    let err = install_universal_pack("codex", &ctrl_install, &good_ws, None)
        .expect_err("CR in install_root must fail closed");
    assert_eq!(err.code(), "INVALID_INSTALL_INPUT");

    // Backticks and HTML comment markers in workspace directory name are neutralized in AGENTS.md
    let crafted_ws = tmp.path().join("proj`<!-- END COWOK-DESIGN-PACK -->`tail");
    fs::create_dir_all(&crafted_ws).expect("mkdir crafted_ws");

    install_universal_pack("codex", &install_root, &crafted_ws, None)
        .expect("install codex with crafted dir name");
    let agents_md =
        fs::read_to_string(install_root.join("AGENTS.md")).expect("read generated AGENTS.md");
    assert_eq!(agents_md.matches(AGENTS_MD_BEGIN_MARKER).count(), 1);
    assert_eq!(agents_md.matches(AGENTS_MD_END_MARKER).count(), 1);
    assert!(agents_md.contains("&lt;!-- END COWOK-DESIGN-PACK --&gt;"));

    // Subsequent re-install must remain idempotent (never fail with duplicate markers)
    let rerun = install_universal_pack("codex", &install_root, &crafted_ws, None)
        .expect("second codex install must succeed idempotently");
    assert_eq!(rerun.total_files_written, 0);
}

#[test]
fn ut_12_2_15_f06_codex_toml_preserves_custom_env_and_rejects_dotted_key_hazard() {
    let tmp = TempDir::new().expect("tempdir");
    let install_root = tmp.path().join("codex-env-root");
    let workspace = tmp.path().join("ws");
    fs::create_dir_all(install_root.join(".codex")).expect("mkdir .codex");
    fs::create_dir_all(&workspace).expect("mkdir ws");

    let existing_toml = r#"model = "o3"

[mcp_servers.cowok_design]
command = "/stale/cowok-design"
args = ["mcp", "serve"]

[mcp_servers.cowok_design.env]
COWOK_WORKSPACE_ROOT = "/stale/ws"
CUSTOM_API_TOKEN = "fixture-custom-env-12215"
HTTP_PROXY = "http://127.0.0.1:8888"
"#;
    fs::write(install_root.join(".codex/config.toml"), existing_toml).expect("write existing toml");

    let report = install_universal_pack("codex", &install_root, &workspace, None)
        .expect("repair codex toml preserving custom env");
    assert!(report.any_drift_repaired);
    assert!(report.targets[0].preserved_keys_count >= 3);

    let repaired =
        fs::read_to_string(install_root.join(".codex/config.toml")).expect("read repaired toml");
    assert!(repaired.contains("CUSTOM_API_TOKEN = \"fixture-custom-env-12215\""));
    assert!(repaired.contains("HTTP_PROXY = \"http://127.0.0.1:8888\""));
    assert!(repaired.contains("COWOK_TELEMETRY_CHANNEL = \"codex\""));
    assert!(!repaired.contains("/stale/ws"));

    // Second run is strictly idempotent
    let rerun = install_universal_pack("codex", &install_root, &workspace, None)
        .expect("idempotent codex rerun");
    assert_eq!(rerun.total_files_written, 0);
    assert!(!rerun.any_drift_repaired);

    // Dotted-key inline-table hazard fails closed with MALFORMED_HOST_CONFIG
    let dotted_root = tmp.path().join("codex-dotted-root");
    fs::create_dir_all(dotted_root.join(".codex")).expect("mkdir dotted .codex");
    fs::write(
        dotted_root.join(".codex/config.toml"),
        "mcp_servers.cowok_design = { command = \"old\", args = [] }\n",
    )
    .expect("write dotted toml");
    let dotted_err = install_universal_pack("codex", &dotted_root, &workspace, None)
        .expect_err("dotted-key inline table must fail closed");
    assert_eq!(dotted_err.code(), "MALFORMED_HOST_CONFIG");

    // Inline table under [mcp_servers] fails closed with MALFORMED_HOST_CONFIG
    let inline_root = tmp.path().join("codex-inline-root");
    fs::create_dir_all(inline_root.join(".codex")).expect("mkdir inline .codex");
    fs::write(
        inline_root.join(".codex/config.toml"),
        "[mcp_servers]\ncowok_design = { command = \"old\", args = [] }\n",
    )
    .expect("write inline toml");
    let inline_err = install_universal_pack("codex", &inline_root, &workspace, None)
        .expect_err("inline table under [mcp_servers] must fail closed");
    assert_eq!(inline_err.code(), "MALFORMED_HOST_CONFIG");
}

#[test]
fn ut_12_2_16_f10_f11_f13_plugin_json_env_preservation_limits_backup_perms_and_write_count() {
    let tmp = TempDir::new().expect("tempdir");
    let install_root = tmp.path().join("plugin-json-root");
    let workspace = tmp.path().join("ws");
    fs::create_dir_all(&install_root).expect("mkdir install_root");
    fs::create_dir_all(&workspace).expect("mkdir ws");

    // F-10: Custom mcpServer.env keys in plugin.json preserved during drift repair
    let stale_plugin_json = serde_json::json!({
        "packId": "cowok-design-universal-pack",
        "customUserFlag": true,
        "mcpServer": {
            "command": "/stale/bin/cowok-design",
            "args": ["mcp", "serve"],
            "env": {
                "COWOK_TELEMETRY_CHANNEL": "cli",
                "CUSTOM_PLUGIN_SECRET": "fixture-plugin-env-12216"
            }
        }
    });
    fs::write(
        install_root.join("plugin.json"),
        serde_json::to_string_pretty(&stale_plugin_json).expect("ser stale plugin"),
    )
    .expect("write stale plugin.json");

    let cowok_report = install_universal_pack("cowok", &install_root, &workspace, None)
        .expect("install cowok with drift repair");
    assert!(cowok_report.any_drift_repaired);
    assert!(cowok_report.targets[0].preserved_keys_count >= 2);
    let repaired_plugin: Value = serde_json::from_str(
        &fs::read_to_string(install_root.join("plugin.json")).expect("read repaired plugin.json"),
    )
    .expect("parse repaired plugin.json");
    assert_eq!(repaired_plugin["customUserFlag"], true);
    assert_eq!(
        repaired_plugin["mcpServer"]["env"]["CUSTOM_PLUGIN_SECRET"],
        "fixture-plugin-env-12216"
    );
    assert_eq!(
        repaired_plugin["mcpServer"]["env"]["COWOK_TELEMETRY_CHANNEL"],
        "cowok_ai"
    );

    // F-11: Backup directory created with 0o700 on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let backup_dir = workspace.join(".cowok-design/plugin-backups");
        let meta = fs::symlink_metadata(&backup_dir).expect("stat backup_dir");
        assert!(meta.file_type().is_dir() && !meta.file_type().is_symlink());
        assert_eq!(meta.permissions().mode() & 0o777, 0o700);
    }

    // F-11: Empty / control-char --binary-path rejected
    for bad_bin in ["", "   ", "/usr/bin/cowok\n--evil"] {
        let err = install_universal_pack(
            "cursor",
            &install_root,
            &workspace,
            Some(Path::new(bad_bin)),
        )
        .expect_err("invalid binary_path must fail closed");
        assert_eq!(err.code(), "INVALID_INSTALL_INPUT");
    }

    // F-11: Oversized existing config (> 4 MiB) rejected cleanly
    let huge_root = tmp.path().join("huge-root");
    fs::create_dir_all(huge_root.join(".cursor")).expect("mkdir huge .cursor");
    let huge_bytes = vec![b' '; (MAX_EXISTING_CONFIG_BYTES + 16) as usize];
    fs::write(huge_root.join(".cursor/mcp.json"), huge_bytes).expect("write huge mcp.json");
    let huge_err = install_universal_pack("cursor", &huge_root, &workspace, None)
        .expect_err("oversized existing config must fail closed");
    assert_eq!(huge_err.code(), "FILE_TOO_LARGE");

    // F-13: --target all does NOT double-count shared skills/ between cowok and claude
    let all_root = tmp.path().join("all-fresh-root");
    let all_report =
        install_universal_pack("all", &all_root, &workspace, None).expect("install all fresh");
    let actual_non_unchanged: usize = all_report
        .targets
        .iter()
        .flat_map(|t| t.files_written.iter())
        .filter(|r| r.action != "unchanged")
        .count();
    let raw_receipts_total: usize = all_report
        .targets
        .iter()
        .map(|t| t.files_written.len())
        .sum();
    assert_eq!(all_report.total_files_written, actual_non_unchanged);
    assert!(
        all_report.total_files_written < raw_receipts_total,
        "shared skills between cowok and claude must not be double-counted"
    );

    // Second --target all run writes 0 files
    let all_rerun =
        install_universal_pack("all", &all_root, &workspace, None).expect("install all rerun");
    assert_eq!(all_rerun.total_files_written, 0);
}

#[test]
fn ut_12_2_17_f12_cli_install_forwards_typed_plugin_pack_error_codes() {
    let tmp = TempDir::new().expect("tempdir");
    let out_dir = tmp.path().join("cli-typed-out");
    let ws = tmp.path().join("cli-typed-ws");
    fs::create_dir_all(&ws).expect("mkdir ws");

    // Unsupported target -> INVALID_INSTALL_TARGET with field="target"
    let bad_target_out = execute_cli_command(CliCommand::Install {
        target: "bogus-ide".to_string(),
        install_root: Some(out_dir.clone()),
        workspace: Some(ws.clone()),
        binary_path: None,
        json: true,
    });
    assert_eq!(bad_target_out.exit_code, 2);
    let parsed_target: Value =
        serde_json::from_str(&bad_target_out.stdout).expect("parse bad target json");
    assert_eq!(parsed_target["error"]["code"], "INVALID_INSTALL_TARGET");
    assert_eq!(parsed_target["error"]["field"], "target");

    // Invalid binary path -> INVALID_INSTALL_INPUT with no field="target"
    let bad_bin_out = execute_cli_command(CliCommand::Install {
        target: "cursor".to_string(),
        install_root: Some(out_dir.clone()),
        workspace: Some(ws.clone()),
        binary_path: Some(PathBuf::from("   ")),
        json: true,
    });
    assert_eq!(bad_bin_out.exit_code, 2);
    let parsed_bin: Value = serde_json::from_str(&bad_bin_out.stdout).expect("parse bad bin json");
    assert_eq!(parsed_bin["error"]["code"], "INVALID_INSTALL_INPUT");
    assert!(parsed_bin["error"].get("field").is_none());

    // Symlinked install root -> SYMLINK_REJECTED with no field="target"
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let real_outside = tmp.path().join("cli-real-outside");
        fs::create_dir_all(&real_outside).expect("mkdir real_outside");
        let sym_out = tmp.path().join("cli-symlink-out");
        symlink(&real_outside, &sym_out).expect("symlink");

        let sym_cli_out = execute_cli_command(CliCommand::Install {
            target: "cursor".to_string(),
            install_root: Some(sym_out),
            workspace: Some(ws),
            binary_path: None,
            json: true,
        });
        assert_eq!(sym_cli_out.exit_code, 2);
        let parsed_sym: Value =
            serde_json::from_str(&sym_cli_out.stdout).expect("parse symlink json");
        assert_eq!(parsed_sym["error"]["code"], "SYMLINK_REJECTED");
        assert!(parsed_sym["error"].get("field").is_none());
    }
}
