// @story 12.2
// @implements AC-02, AC-03 (Universal Multi-Harness Plugin Pack Compiler for Cowok.ai, Claude Code, OpenAI Codex CLI, Google Antigravity IDE & Cursor with HUMAN_ONLY Preservation, Per-Host Telemetry Channel Labeling, cowok_submit_feedback Exposure & Secret-Preserving Auto-Sync Drift Repair)
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

use chrono::Utc;
use mcp_gateway::{
    default_tool_policies, merge_codex_toml_config, merge_json_host_config, HostClientId,
    PermissionTier, COWOK_SERVER_KEY, COWOK_TOML_KEY,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::static_spa::EmbeddedBuiltinSkillsRegistry;
use crate::support_edge::VALID_CHANNELS;
use crate::tauri_trust::{
    open_verified_regular_file_nofollow_nlink1, reject_symlink_ancestors, sha256_hex,
    validate_relative_sandbox_path, write_atomic_nofollow_nlink1, TrustBoundaryError,
};

pub const EMBEDDED_CANONICAL_PACK_MANIFEST_JSON: &str =
    include_str!("../../../packaging/plugins/canonical-manifest.json");

const AGENT_DESIGN_ORCHESTRATOR: &str =
    include_str!("../../../packages/builtin-skills/design-craft/agents/design-orchestrator.md");
const AGENT_FLOW_ARCHITECT: &str =
    include_str!("../../../packages/builtin-skills/design-craft/agents/flow-architect.md");
const AGENT_BRAND_STYLIST: &str =
    include_str!("../../../packages/builtin-skills/design-craft/agents/brand-stylist.md");
const AGENT_UI_CRAFTER: &str =
    include_str!("../../../packages/builtin-skills/design-craft/agents/ui-crafter.md");
const AGENT_DESIGN_AUDITOR: &str =
    include_str!("../../../packages/builtin-skills/design-craft/agents/design-auditor.md");

pub const AGENTS_MD_BEGIN_MARKER: &str = "<!-- BEGIN COWOK-DESIGN-PACK -->";
pub const AGENTS_MD_END_MARKER: &str = "<!-- END COWOK-DESIGN-PACK -->";
pub const MAX_EXISTING_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

pub const CANONICAL_HUMAN_ONLY_TOOLS: [&str; 4] = [
    "ephemeral_tool_forge",
    "learn_card_promote",
    "staging_plugin_promote",
    "workspace_destructive_reset",
];
pub const CANONICAL_FEEDBACK_TOOL: &str = "cowok_submit_feedback";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHostTarget {
    Cowok,
    Claude,
    Codex,
    Antigravity,
    Cursor,
}

pub type UniversalPluginHost = PluginHostTarget;

impl PluginHostTarget {
    pub const ALL: &'static [PluginHostTarget] = &[
        Self::Cowok,
        Self::Claude,
        Self::Codex,
        Self::Antigravity,
        Self::Cursor,
    ];

    pub fn parse(raw: &str) -> Result<Vec<Self>, PluginPackError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cowok" | "cowok_ai" | "cowok-ai" => Ok(vec![Self::Cowok]),
            "claude" | "claude_code" | "claude-code" => Ok(vec![Self::Claude]),
            "codex" | "openai-codex" => Ok(vec![Self::Codex]),
            "antigravity" | "google-antigravity" => Ok(vec![Self::Antigravity]),
            "cursor" => Ok(vec![Self::Cursor]),
            "all" => Ok(Self::ALL.to_vec()),
            other => Err(PluginPackError::UnsupportedTarget(other.to_string())),
        }
    }

    pub fn cli_flag(&self) -> &'static str {
        match self {
            Self::Cowok => "cowok",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
            Self::Cursor => "cursor",
        }
    }

    /// Exact match against `crate::support_edge::VALID_CHANNELS`.
    pub fn telemetry_channel(&self) -> &'static str {
        match self {
            Self::Cowok => "cowok_ai",
            Self::Claude => "claude_code",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
            Self::Cursor => "cursor",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Cowok => "Cowok.ai",
            Self::Claude => "Claude Code / Desktop",
            Self::Codex => "OpenAI Codex CLI",
            Self::Antigravity => "Google Antigravity IDE",
            Self::Cursor => "Cursor",
        }
    }

    pub fn mcp_host_client_id(&self) -> HostClientId {
        match self {
            Self::Cowok => HostClientId::CowokAi,
            Self::Claude => HostClientId::ClaudeCode,
            Self::Codex => HostClientId::Codex,
            Self::Antigravity => HostClientId::Antigravity,
            Self::Cursor => HostClientId::Cursor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMcpToolEntry {
    pub tool_name: String,
    pub permission_tier: String,
    pub mcp_callable: bool,
    pub locked_human_only: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniversalPluginPackManifest {
    pub schema_version: u32,
    pub pack_id: String,
    pub version: String,
    pub spec_standard: String,
    #[serde(default)]
    pub description: String,
    pub mcp_command: String,
    pub mcp_args: Vec<String>,
    pub mcp_transport: String,
    pub default_daemon_url: String,
    pub feedback_tool_name: String,
    pub human_only_tools: Vec<String>,
    #[serde(default)]
    pub de_tools: Vec<String>,
    pub mcp_tools: Vec<CanonicalMcpToolEntry>,
    pub agents: Vec<String>,
    pub skill_ids: Vec<String>,
    pub security_invariants: Vec<String>,
}

pub type CanonicalPluginManifest = UniversalPluginPackManifest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmittedFileReceipt {
    pub relative_path: String,
    pub sha256: String,
    pub byte_size: usize,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallTargetReport {
    pub target: String,
    pub channel: String,
    pub display_name: String,
    pub install_root: String,
    pub files_written: Vec<EmittedFileReceipt>,
    pub drift_detected: bool,
    pub drift_repaired: bool,
    pub preserved_keys_count: usize,
    pub human_only_tools_preserved: Vec<String>,
    pub feedback_tool_exposed: bool,
    pub skills_emitted: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
    pub installed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniversalInstallBatchReport {
    pub pack_id: String,
    pub pack_version: String,
    pub canonical_manifest_sha256: String,
    pub targets: Vec<InstallTargetReport>,
    pub total_files_written: usize,
    pub any_drift_repaired: bool,
    pub generated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPackError {
    UnsupportedTarget(String),
    InvalidInput(String),
    InvalidCanonicalManifest(String),
    SecurityInvariantViolation(String),
    MalformedExistingConfig { path: String, reason: String },
    TrustBoundary(TrustBoundaryError),
    Io(String),
}

impl PluginPackError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedTarget(_) => "INVALID_INSTALL_TARGET",
            Self::InvalidInput(_) => "INVALID_INSTALL_INPUT",
            Self::InvalidCanonicalManifest(_) => "INVALID_CANONICAL_MANIFEST",
            Self::SecurityInvariantViolation(_) => "SECURITY_INVARIANT_VIOLATION",
            Self::MalformedExistingConfig { .. } => "MALFORMED_HOST_CONFIG",
            Self::TrustBoundary(e) => e.code(),
            Self::Io(_) => "PLUGIN_PACK_IO_ERROR",
        }
    }
}

impl fmt::Display for PluginPackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget(t) => write!(
                f,
                "Unsupported install target '{t}'; expected one of: cowok, claude, codex, antigravity, cursor, all"
            ),
            Self::InvalidInput(msg) => write!(f, "Invalid install input: {msg}"),
            Self::InvalidCanonicalManifest(msg) => {
                write!(f, "Invalid canonical plugin manifest: {msg}")
            }
            Self::SecurityInvariantViolation(msg) => {
                write!(f, "Security invariant violation: {msg}")
            }
            Self::MalformedExistingConfig { path, reason } => {
                write!(f, "Malformed existing host config at '{path}': {reason}")
            }
            Self::TrustBoundary(e) => write!(f, "{e}"),
            Self::Io(msg) => write!(f, "Plugin pack I/O error: {msg}"),
        }
    }
}

impl std::error::Error for PluginPackError {}

impl From<TrustBoundaryError> for PluginPackError {
    fn from(value: TrustBoundaryError) -> Self {
        Self::TrustBoundary(value)
    }
}

/// Loads and validates the compiled-in canonical Universal Plugin Pack manifest,
/// cross-checking all 7 built-in craft skills, 7 DE tools, 4 immutable `HUMAN_ONLY` locks,
/// and `cowok_submit_feedback` exposure.
pub fn load_canonical_manifest() -> Result<UniversalPluginPackManifest, PluginPackError> {
    let mut manifest: UniversalPluginPackManifest =
        serde_json::from_str(EMBEDDED_CANONICAL_PACK_MANIFEST_JSON)
            .map_err(|e| PluginPackError::InvalidCanonicalManifest(e.to_string()))?;

    let embedded_skills = EmbeddedBuiltinSkillsRegistry::load();
    if manifest.skill_ids.is_empty() {
        manifest.skill_ids = embedded_skills
            .skills()
            .iter()
            .map(|s| s.id.clone())
            .collect();
    }
    if manifest.de_tools.is_empty() {
        manifest.de_tools = embedded_skills.de_tools().to_vec();
    }
    if manifest.mcp_tools.is_empty() {
        manifest.mcp_tools = default_tool_policies()
            .into_iter()
            .map(|p| CanonicalMcpToolEntry {
                tool_name: p.tool_name,
                permission_tier: p.permission_tier.as_str().to_string(),
                mcp_callable: p.mcp_callable,
                locked_human_only: p.permission_tier == PermissionTier::HumanOnly,
                description: p.description,
            })
            .collect();
    }

    validate_canonical_manifest(&manifest)?;
    Ok(manifest)
}

/// Fails closed if any `HUMAN_ONLY` gate is weakened (including lookalike/aliasing attempts),
/// if `cowok_submit_feedback` is missing/un-callable, or if skill/channel invariants drift.
pub fn validate_canonical_manifest(
    manifest: &UniversalPluginPackManifest,
) -> Result<(), PluginPackError> {
    for target in PluginHostTarget::ALL {
        if !VALID_CHANNELS.contains(&target.telemetry_channel()) {
            return Err(PluginPackError::SecurityInvariantViolation(format!(
                "host '{}' telemetry channel '{}' is not whitelisted in support_edge::VALID_CHANNELS",
                target.cli_flag(),
                target.telemetry_channel()
            )));
        }
    }

    if manifest.feedback_tool_name != CANONICAL_FEEDBACK_TOOL {
        return Err(PluginPackError::SecurityInvariantViolation(format!(
            "feedbackToolName must be '{CANONICAL_FEEDBACK_TOOL}'"
        )));
    }

    let mut feedback_ok = false;
    for tool in &manifest.mcp_tools {
        let folded = normalize_ascii_tool_name(&tool.tool_name);
        if is_inherently_human_only_folded(&folded)
            && (tool.mcp_callable
                || tool.permission_tier != "HUMAN_ONLY"
                || !tool.locked_human_only)
        {
            return Err(PluginPackError::SecurityInvariantViolation(format!(
                "HUMAN_ONLY tool '{}' cannot be mcpCallable or downgraded",
                tool.tool_name
            )));
        }
        if tool.tool_name == CANONICAL_FEEDBACK_TOOL {
            if !tool.mcp_callable || tool.locked_human_only {
                return Err(PluginPackError::SecurityInvariantViolation(
                    "cowok_submit_feedback must remain mcpCallable=true".to_string(),
                ));
            }
            feedback_ok = true;
        }
    }
    if !feedback_ok {
        return Err(PluginPackError::SecurityInvariantViolation(
            "canonical manifest must expose cowok_submit_feedback".to_string(),
        ));
    }

    for required in CANONICAL_HUMAN_ONLY_TOOLS {
        if !manifest.human_only_tools.iter().any(|t| t == required) {
            return Err(PluginPackError::SecurityInvariantViolation(format!(
                "missing immutable HUMAN_ONLY tool '{required}' in humanOnlyTools"
            )));
        }
        let matching = manifest
            .mcp_tools
            .iter()
            .find(|t| t.tool_name == required)
            .ok_or_else(|| {
                PluginPackError::SecurityInvariantViolation(format!(
                    "missing immutable HUMAN_ONLY tool '{required}' in mcpTools"
                ))
            })?;
        if matching.mcp_callable
            || matching.permission_tier != "HUMAN_ONLY"
            || !matching.locked_human_only
        {
            return Err(PluginPackError::SecurityInvariantViolation(format!(
                "HUMAN_ONLY invariant broken for '{required}'"
            )));
        }
    }

    let embedded_skills = EmbeddedBuiltinSkillsRegistry::load();
    if manifest.skill_ids.len() < embedded_skills.skills().len() {
        return Err(PluginPackError::InvalidCanonicalManifest(
            "canonical manifest must include all 7 embedded craft skills".to_string(),
        ));
    }
    Ok(())
}

/// Primary CLI entrypoint ready for wiring in `cli/commands.rs` (`cowok-design install --target ...`).
pub fn execute_universal_install(
    target_raw: &str,
    install_root: &Path,
    workspace_root: &Path,
    binary_path: Option<&Path>,
) -> Result<UniversalInstallBatchReport, String> {
    install_universal_pack(target_raw, install_root, workspace_root, binary_path)
        .map_err(|e| format!("{}: {e}", e.code()))
}

fn validate_install_string_input(label: &str, raw: &str) -> Result<(), PluginPackError> {
    if raw.trim().is_empty() {
        return Err(PluginPackError::InvalidInput(format!(
            "{label} must not be empty"
        )));
    }
    if raw.chars().any(char::is_control) {
        return Err(PluginPackError::InvalidInput(format!(
            "{label} must not contain ASCII control characters"
        )));
    }
    Ok(())
}

/// Typed installation engine emitting 1..5 native host plugin packs from the single canonical
/// manifest with `O_NOFOLLOW` + `nlink == 1` protection and secret-preserving drift repair.
pub fn install_universal_pack(
    target_raw: &str,
    install_root: &Path,
    workspace_root: &Path,
    binary_path: Option<&Path>,
) -> Result<UniversalInstallBatchReport, PluginPackError> {
    let hosts = PluginHostTarget::parse(target_raw)?;
    let manifest = load_canonical_manifest()?;
    let skills_registry = EmbeddedBuiltinSkillsRegistry::load();

    validate_install_string_input("install_root", &install_root.to_string_lossy())?;
    validate_install_string_input("workspace_root", &workspace_root.to_string_lossy())?;
    let command_str = match binary_path {
        Some(p) => {
            let s = p.to_string_lossy().into_owned();
            validate_install_string_input("binary_path", &s)?;
            s
        }
        None => {
            validate_install_string_input("mcp_command", &manifest.mcp_command)?;
            manifest.mcp_command.clone()
        }
    };

    reject_symlink_ancestors(install_root)?;
    if !install_root.exists() {
        fs::create_dir_all(install_root)
            .map_err(|e| PluginPackError::Io(format!("create install_root failed: {e}")))?;
    }
    let root_meta = fs::symlink_metadata(install_root)
        .map_err(|e| PluginPackError::Io(format!("stat install_root failed: {e}")))?;
    if root_meta.file_type().is_symlink() || !root_meta.file_type().is_dir() {
        return Err(PluginPackError::TrustBoundary(
            TrustBoundaryError::SymlinkRejected,
        ));
    }
    let canon_install_root = install_root
        .canonicalize()
        .map_err(|e| PluginPackError::Io(format!("canonicalize install_root failed: {e}")))?;
    validate_install_string_input("install_root", &canon_install_root.to_string_lossy())?;

    reject_symlink_ancestors(workspace_root)?;
    if !workspace_root.exists() {
        fs::create_dir_all(workspace_root)
            .map_err(|e| PluginPackError::Io(format!("create workspace_root failed: {e}")))?;
    }
    let ws_meta = fs::symlink_metadata(workspace_root)
        .map_err(|e| PluginPackError::Io(format!("stat workspace_root failed: {e}")))?;
    if ws_meta.file_type().is_symlink() || !ws_meta.file_type().is_dir() {
        return Err(PluginPackError::TrustBoundary(
            TrustBoundaryError::SymlinkRejected,
        ));
    }
    let canon_workspace = workspace_root
        .canonicalize()
        .map_err(|e| PluginPackError::Io(format!("canonicalize workspace_root failed: {e}")))?;
    validate_install_string_input("workspace_root", &canon_workspace.to_string_lossy())?;

    let backup_root = canon_workspace.join(".cowok-design").join("plugin-backups");

    let mut targets = Vec::with_capacity(hosts.len());
    let mut total_files_written = 0usize;
    let mut any_drift_repaired = false;

    for host in hosts {
        let report = install_single_host(
            host,
            &manifest,
            &skills_registry,
            &canon_install_root,
            &canon_workspace,
            &backup_root,
            &command_str,
        )?;
        total_files_written += report
            .files_written
            .iter()
            .filter(|r| r.action != "unchanged")
            .count();
        if report.drift_repaired {
            any_drift_repaired = true;
        }
        targets.push(report);
    }

    Ok(UniversalInstallBatchReport {
        pack_id: manifest.pack_id,
        pack_version: manifest.version,
        canonical_manifest_sha256: sha256_hex(EMBEDDED_CANONICAL_PACK_MANIFEST_JSON.as_bytes()),
        targets,
        total_files_written,
        any_drift_repaired,
        generated_at: Utc::now().timestamp_millis(),
    })
}

struct HostWriteOutcome {
    receipt: EmittedFileReceipt,
    drift_detected: bool,
    drift_repaired: bool,
    preserved_keys_count: usize,
    backup_id: Option<String>,
}

fn install_single_host(
    host: PluginHostTarget,
    manifest: &UniversalPluginPackManifest,
    skills_registry: &EmbeddedBuiltinSkillsRegistry,
    canon_install_root: &Path,
    canon_workspace: &Path,
    backup_root: &Path,
    command_str: &str,
) -> Result<InstallTargetReport, PluginPackError> {
    let mut files_written = Vec::new();
    let mut drift_detected = false;
    let mut drift_repaired = false;
    let mut preserved_keys_count = 0usize;
    let mut backup_id: Option<String> = None;

    let mut absorb_outcome = |out: HostWriteOutcome| {
        files_written.push(out.receipt);
        drift_detected |= out.drift_detected;
        drift_repaired |= out.drift_repaired;
        preserved_keys_count += out.preserved_keys_count;
        if backup_id.is_none() {
            backup_id = out.backup_id;
        }
    };

    let workspace_str = canon_workspace.to_string_lossy().into_owned();
    let skills_emitted = match host {
        PluginHostTarget::Cowok => {
            let plugin_out = write_merged_json_manifest(
                canon_install_root,
                "plugin.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                "skills",
                None,
                backup_root,
            )?;
            absorb_outcome(plugin_out);

            let mcp_out = write_merged_json_mcp_config(
                canon_install_root,
                ".cowok-ai/mcp.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                backup_root,
            )?;
            absorb_outcome(mcp_out);

            emit_embedded_skills(
                canon_install_root,
                "skills",
                skills_registry,
                &mut files_written,
            )?
        }
        PluginHostTarget::Claude => {
            let plugin_out = write_merged_json_manifest(
                canon_install_root,
                ".claude-plugin/plugin.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                "../skills",
                Some("../agents"),
                backup_root,
            )?;
            absorb_outcome(plugin_out);

            let mcp_out = write_merged_json_mcp_config(
                canon_install_root,
                ".mcp.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                backup_root,
            )?;
            absorb_outcome(mcp_out);

            let count = emit_embedded_skills(
                canon_install_root,
                "skills",
                skills_registry,
                &mut files_written,
            )?;
            emit_embedded_agents(canon_install_root, "agents", &mut files_written)?;
            count
        }
        PluginHostTarget::Codex => {
            let toml_out = write_merged_codex_toml(
                canon_install_root,
                ".codex/config.toml",
                manifest,
                command_str,
                &workspace_str,
                backup_root,
            )?;
            absorb_outcome(toml_out);

            let agents_md_out = write_marker_bounded_agents_md(
                canon_install_root,
                "AGENTS.md",
                manifest,
                &workspace_str,
                backup_root,
            )?;
            absorb_outcome(agents_md_out);

            emit_embedded_skills(
                canon_install_root,
                ".codex/skills",
                skills_registry,
                &mut files_written,
            )?
        }
        PluginHostTarget::Antigravity => {
            let plugin_out = write_merged_json_manifest(
                canon_install_root,
                "antigravity-plugin/plugin.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                "./skills",
                Some("./agents"),
                backup_root,
            )?;
            absorb_outcome(plugin_out);

            let mcp_out = write_merged_json_mcp_config(
                canon_install_root,
                "antigravity-plugin/.mcp.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                backup_root,
            )?;
            absorb_outcome(mcp_out);

            let count = emit_embedded_skills(
                canon_install_root,
                "antigravity-plugin/skills",
                skills_registry,
                &mut files_written,
            )?;
            emit_embedded_agents(
                canon_install_root,
                "antigravity-plugin/agents",
                &mut files_written,
            )?;
            count
        }
        PluginHostTarget::Cursor => {
            let mcp_out = write_merged_json_mcp_config(
                canon_install_root,
                ".cursor/mcp.json",
                host,
                manifest,
                command_str,
                &workspace_str,
                backup_root,
            )?;
            absorb_outcome(mcp_out);

            let craft_mdc = render_cursor_craft_rule_mdc(manifest, skills_registry);
            files_written.push(write_static_pack_file(
                canon_install_root,
                ".cursor/rules/cowok-design.mdc",
                craft_mdc.as_bytes(),
            )?);

            let security_mdc = render_cursor_security_rule_mdc(manifest);
            files_written.push(write_static_pack_file(
                canon_install_root,
                ".cursor/rules/cowok-security-human-only.mdc",
                security_mdc.as_bytes(),
            )?);
            skills_registry.skills().len()
        }
    };

    Ok(InstallTargetReport {
        target: host.cli_flag().to_string(),
        channel: host.telemetry_channel().to_string(),
        display_name: host.display_name().to_string(),
        install_root: canon_install_root.display().to_string(),
        files_written,
        drift_detected,
        drift_repaired,
        preserved_keys_count,
        human_only_tools_preserved: manifest.human_only_tools.clone(),
        feedback_tool_exposed: manifest
            .mcp_tools
            .iter()
            .any(|t| t.tool_name == CANONICAL_FEEDBACK_TOOL && t.mcp_callable),
        skills_emitted,
        backup_id,
        installed_at: Utc::now().timestamp_millis(),
    })
}

fn emit_embedded_skills(
    canon_install_root: &Path,
    skills_prefix: &str,
    skills_registry: &EmbeddedBuiltinSkillsRegistry,
    files_written: &mut Vec<EmittedFileReceipt>,
) -> Result<usize, PluginPackError> {
    for skill in skills_registry.skills() {
        let rel = format!("{skills_prefix}/{}/SKILL.md", skill.id);
        let receipt = write_static_pack_file(canon_install_root, &rel, skill.content.as_bytes())?;
        files_written.push(receipt);
    }
    Ok(skills_registry.skills().len())
}

fn emit_embedded_agents(
    canon_install_root: &Path,
    agents_prefix: &str,
    files_written: &mut Vec<EmittedFileReceipt>,
) -> Result<(), PluginPackError> {
    let agents: [(&str, &str); 5] = [
        ("design-orchestrator.md", AGENT_DESIGN_ORCHESTRATOR),
        ("flow-architect.md", AGENT_FLOW_ARCHITECT),
        ("brand-stylist.md", AGENT_BRAND_STYLIST),
        ("ui-crafter.md", AGENT_UI_CRAFTER),
        ("design-auditor.md", AGENT_DESIGN_AUDITOR),
    ];
    for (filename, content) in agents {
        let rel = format!("{agents_prefix}/{filename}");
        let receipt = write_static_pack_file(canon_install_root, &rel, content.as_bytes())?;
        files_written.push(receipt);
    }
    Ok(())
}

fn read_existing_utf8_nofollow(path: &Path) -> Result<Option<String>, PluginPackError> {
    match fs::symlink_metadata(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(PluginPackError::Io(format!(
                "stat existing file failed: {err}"
            )));
        }
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(PluginPackError::TrustBoundary(
                    TrustBoundaryError::SymlinkRejected,
                ));
            }
        }
    }
    let (file, _nlink, len) = open_verified_regular_file_nofollow_nlink1(path)?;
    if len > MAX_EXISTING_CONFIG_BYTES {
        return Err(PluginPackError::TrustBoundary(
            TrustBoundaryError::FileTooLarge {
                actual: len,
                max: MAX_EXISTING_CONFIG_BYTES,
            },
        ));
    }
    let mut bytes = Vec::with_capacity((len as usize).min(64 * 1024));
    file.take(MAX_EXISTING_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| PluginPackError::Io(format!("read existing file failed: {e}")))?;
    if (bytes.len() as u64) > MAX_EXISTING_CONFIG_BYTES {
        return Err(PluginPackError::TrustBoundary(
            TrustBoundaryError::FileTooLarge {
                actual: bytes.len() as u64,
                max: MAX_EXISTING_CONFIG_BYTES,
            },
        ));
    }
    let buf = String::from_utf8(bytes).map_err(|e| PluginPackError::MalformedExistingConfig {
        path: path.display().to_string(),
        reason: format!("file is not valid UTF-8: {e}"),
    })?;
    Ok(Some(buf))
}

fn write_backup_if_needed(
    backup_root: &Path,
    host: PluginHostTarget,
    existing_bytes: &[u8],
) -> Result<String, PluginPackError> {
    reject_symlink_ancestors(backup_root)?;
    if !backup_root.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(backup_root)
            .map_err(|e| PluginPackError::Io(format!("create backup_root failed: {e}")))?;
    }
    reject_symlink_ancestors(backup_root)?;
    let backup_meta = fs::symlink_metadata(backup_root)
        .map_err(|e| PluginPackError::Io(format!("stat backup_root failed: {e}")))?;
    if backup_meta.file_type().is_symlink() || !backup_meta.file_type().is_dir() {
        return Err(PluginPackError::TrustBoundary(
            TrustBoundaryError::SymlinkRejected,
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(backup_root, fs::Permissions::from_mode(0o700)).map_err(|e| {
            PluginPackError::Io(format!("set backup_root permissions 0700 failed: {e}"))
        })?;
    }
    let canon_backup_root = backup_root
        .canonicalize()
        .map_err(|e| PluginPackError::Io(format!("canonicalize backup_root failed: {e}")))?;
    let backup_id = Uuid::new_v4().to_string();
    let filename = format!(
        "{}-{}-{}.bak",
        host.cli_flag(),
        Utc::now().timestamp_millis(),
        backup_id
    );
    let backup_path = canon_backup_root.join(filename);
    write_atomic_nofollow_nlink1(&canon_backup_root, &backup_path, existing_bytes, 0o600)?;
    Ok(backup_id)
}

fn write_static_pack_file(
    canon_install_root: &Path,
    rel_str: &str,
    bytes: &[u8],
) -> Result<EmittedFileReceipt, PluginPackError> {
    let rel = validate_relative_sandbox_path(rel_str)?;
    let target = canon_install_root.join(&rel);
    let existing = read_existing_utf8_nofollow(&target)?;
    let action = match existing {
        None => "created",
        Some(ref old) if old.as_bytes() == bytes => "unchanged",
        Some(_) => "updated",
    };
    if action != "unchanged" {
        write_atomic_nofollow_nlink1(canon_install_root, &target, bytes, 0o644)?;
    }
    Ok(EmittedFileReceipt {
        relative_path: rel_str.to_string(),
        sha256: sha256_hex(bytes),
        byte_size: bytes.len(),
        action: action.to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
fn write_merged_json_manifest(
    canon_install_root: &Path,
    rel_str: &str,
    host: PluginHostTarget,
    manifest: &UniversalPluginPackManifest,
    command_str: &str,
    workspace_str: &str,
    skills_dir_ref: &str,
    agents_dir_ref: Option<&str>,
    backup_root: &Path,
) -> Result<HostWriteOutcome, PluginPackError> {
    let rel = validate_relative_sandbox_path(rel_str)?;
    let target = canon_install_root.join(&rel);
    let existing = read_existing_utf8_nofollow(&target)?;

    let mut root_map = Map::new();
    let mut had_managed_entry = false;
    let mut preserved_keys_count = 0usize;

    if let Some(ref raw) = existing {
        if !raw.trim().is_empty() {
            let parsed: Value = serde_json::from_str(raw).map_err(|e| {
                PluginPackError::MalformedExistingConfig {
                    path: rel_str.to_string(),
                    reason: e.to_string(),
                }
            })?;
            let obj =
                parsed
                    .as_object()
                    .ok_or_else(|| PluginPackError::MalformedExistingConfig {
                        path: rel_str.to_string(),
                        reason: "root plugin manifest must be a JSON object".to_string(),
                    })?;
            had_managed_entry =
                obj.get("packId").is_some() || obj.get("telemetryChannel").is_some();
            for (k, v) in obj {
                root_map.insert(k.clone(), v.clone());
            }
        }
    }

    let managed_keys: [&str; 15] = [
        "schemaVersion",
        "packId",
        "name",
        "version",
        "specStandard",
        "hostTarget",
        "telemetryChannel",
        "skillsDir",
        "agentsDir",
        "skillIds",
        "deTools",
        "agents",
        "feedbackTool",
        "humanOnlyTools",
        "mcpTools",
    ];
    for key in root_map.keys() {
        if !managed_keys.contains(&key.as_str()) && key != "mcpServer" {
            preserved_keys_count += 1;
        }
    }

    let mut preserved_mcp_env: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(env_obj) = root_map
        .get("mcpServer")
        .and_then(Value::as_object)
        .and_then(|srv| srv.get("env"))
        .and_then(Value::as_object)
    {
        for (ek, ev) in env_obj {
            if ek != "COWOK_TELEMETRY_CHANNEL" && ek != "COWOK_WORKSPACE_ROOT" {
                preserved_mcp_env.insert(ek.clone(), ev.clone());
                preserved_keys_count += 1;
            }
        }
    }

    root_map.insert("schemaVersion".to_string(), json!(manifest.schema_version));
    root_map.insert("packId".to_string(), json!(manifest.pack_id));
    root_map.insert("name".to_string(), json!("cowok-design"));
    root_map.insert("version".to_string(), json!(manifest.version));
    root_map.insert("specStandard".to_string(), json!(manifest.spec_standard));
    root_map.insert("hostTarget".to_string(), json!(host.cli_flag()));
    root_map.insert(
        "telemetryChannel".to_string(),
        json!(host.telemetry_channel()),
    );
    root_map.insert("skillsDir".to_string(), json!(skills_dir_ref));
    if let Some(agents_ref) = agents_dir_ref {
        root_map.insert("agentsDir".to_string(), json!(agents_ref));
    }
    root_map.insert("skillIds".to_string(), json!(manifest.skill_ids));
    root_map.insert("deTools".to_string(), json!(manifest.de_tools));
    root_map.insert("agents".to_string(), json!(manifest.agents));
    root_map.insert(
        "feedbackTool".to_string(),
        json!(manifest.feedback_tool_name),
    );
    root_map.insert(
        "humanOnlyTools".to_string(),
        json!(manifest.human_only_tools),
    );
    root_map.insert("mcpTools".to_string(), json!(manifest.mcp_tools));
    let mut mcp_env_map = Map::new();
    for (k, v) in preserved_mcp_env {
        mcp_env_map.insert(k, v);
    }
    mcp_env_map.insert(
        "COWOK_TELEMETRY_CHANNEL".to_string(),
        json!(host.telemetry_channel()),
    );
    mcp_env_map.insert("COWOK_WORKSPACE_ROOT".to_string(), json!(workspace_str));
    root_map.insert(
        "mcpServer".to_string(),
        json!({
            "command": command_str,
            "args": manifest.mcp_args,
            "transport": manifest.mcp_transport,
            "url": manifest.default_daemon_url,
            "env": Value::Object(mcp_env_map)
        }),
    );

    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(root_map))
            .map_err(|e| PluginPackError::InvalidCanonicalManifest(e.to_string()))?
    );
    finalize_managed_write(
        canon_install_root,
        &target,
        rel_str,
        host,
        existing.as_deref(),
        &rendered,
        had_managed_entry,
        preserved_keys_count,
        backup_root,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_merged_json_mcp_config(
    canon_install_root: &Path,
    rel_str: &str,
    host: PluginHostTarget,
    manifest: &UniversalPluginPackManifest,
    command_str: &str,
    workspace_str: &str,
    backup_root: &Path,
) -> Result<HostWriteOutcome, PluginPackError> {
    let rel = validate_relative_sandbox_path(rel_str)?;
    let target = canon_install_root.join(&rel);
    let existing = read_existing_utf8_nofollow(&target)?;
    let existing_raw = existing.as_deref().unwrap_or("");

    let mut had_managed_entry = false;
    let mut preserved_keys_count = 0usize;
    let mut preserved_cowok_env: BTreeMap<String, Value> = BTreeMap::new();

    if !existing_raw.trim().is_empty() {
        let parsed: Value = serde_json::from_str(existing_raw).map_err(|e| {
            PluginPackError::MalformedExistingConfig {
                path: rel_str.to_string(),
                reason: e.to_string(),
            }
        })?;
        let root_obj =
            parsed
                .as_object()
                .ok_or_else(|| PluginPackError::MalformedExistingConfig {
                    path: rel_str.to_string(),
                    reason: "root JSON config must be an object".to_string(),
                })?;
        for (k, _) in root_obj {
            if k != "mcpServers" {
                preserved_keys_count += 1;
            }
        }
        if let Some(servers) = root_obj.get("mcpServers").and_then(Value::as_object) {
            for (srv_key, srv_val) in servers {
                if srv_key == COWOK_SERVER_KEY {
                    had_managed_entry = true;
                    if let Some(env_obj) = srv_val.get("env").and_then(Value::as_object) {
                        for (ek, ev) in env_obj {
                            if ek != "COWOK_TELEMETRY_CHANNEL" && ek != "COWOK_WORKSPACE_ROOT" {
                                preserved_cowok_env.insert(ek.clone(), ev.clone());
                                preserved_keys_count += 1;
                            }
                        }
                    }
                } else {
                    preserved_keys_count += 1;
                }
            }
        }
    }

    let base_merged = merge_json_host_config(
        existing_raw,
        host.mcp_host_client_id(),
        command_str,
        &manifest.mcp_args,
        &manifest.default_daemon_url,
        rel_str,
    )
    .map_err(|e| PluginPackError::MalformedExistingConfig {
        path: rel_str.to_string(),
        reason: e.to_string(),
    })?;

    let mut root_val: Value = serde_json::from_str(&base_merged).map_err(|e| {
        PluginPackError::MalformedExistingConfig {
            path: rel_str.to_string(),
            reason: e.to_string(),
        }
    })?;

    if let Some(entry_obj) = root_val
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .and_then(|s| s.get_mut(COWOK_SERVER_KEY))
        .and_then(Value::as_object_mut)
    {
        let mut env_map = Map::new();
        for (k, v) in preserved_cowok_env {
            env_map.insert(k, v);
        }
        env_map.insert(
            "COWOK_TELEMETRY_CHANNEL".to_string(),
            json!(host.telemetry_channel()),
        );
        env_map.insert("COWOK_WORKSPACE_ROOT".to_string(), json!(workspace_str));
        entry_obj.insert("env".to_string(), Value::Object(env_map));
        entry_obj.insert(
            "telemetryChannel".to_string(),
            json!(host.telemetry_channel()),
        );
        entry_obj.insert(
            "feedbackTool".to_string(),
            json!(manifest.feedback_tool_name),
        );
        entry_obj.insert(
            "humanOnlyTools".to_string(),
            json!(manifest.human_only_tools),
        );
    }

    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&root_val)
            .map_err(|e| PluginPackError::InvalidCanonicalManifest(e.to_string()))?
    );

    finalize_managed_write(
        canon_install_root,
        &target,
        rel_str,
        host,
        existing.as_deref(),
        &rendered,
        had_managed_entry,
        preserved_keys_count,
        backup_root,
    )
}

fn write_merged_codex_toml(
    canon_install_root: &Path,
    rel_str: &str,
    manifest: &UniversalPluginPackManifest,
    command_str: &str,
    workspace_str: &str,
    backup_root: &Path,
) -> Result<HostWriteOutcome, PluginPackError> {
    let rel = validate_relative_sandbox_path(rel_str)?;
    let target = canon_install_root.join(&rel);
    let existing = read_existing_utf8_nofollow(&target)?;
    let existing_raw = existing.as_deref().unwrap_or("");

    let inspection = inspect_existing_codex_toml(existing_raw, rel_str)?;

    let base_merged = merge_codex_toml_config(
        existing_raw,
        command_str,
        &manifest.mcp_args,
        &manifest.default_daemon_url,
    )
    .map_err(|e| PluginPackError::MalformedExistingConfig {
        path: rel_str.to_string(),
        reason: e.to_string(),
    })?;

    let mut env_stanza = format!(
        "\n[mcp_servers.{COWOK_TOML_KEY}.env]\nCOWOK_TELEMETRY_CHANNEL = \"codex\"\nCOWOK_WORKSPACE_ROOT = \"{}\"\nCOWOK_FEEDBACK_TOOL = \"{}\"\nCOWOK_HUMAN_ONLY_TOOLS = \"{}\"\n",
        escape_toml_basic_string(workspace_str),
        escape_toml_basic_string(&manifest.feedback_tool_name),
        escape_toml_basic_string(&manifest.human_only_tools.join(",")),
    );
    for (raw_key, raw_val) in inspection.preserved_cowok_env.values() {
        env_stanza.push_str(raw_key);
        env_stanza.push_str(" = ");
        env_stanza.push_str(raw_val);
        env_stanza.push('\n');
    }
    let rendered = format!("{}{env_stanza}", base_merged.trim_end());

    // Validate the final rendered TOML parses cleanly through the structural TOML parser.
    let _ = merge_codex_toml_config(
        &rendered,
        command_str,
        &manifest.mcp_args,
        &manifest.default_daemon_url,
    )
    .map_err(|e| PluginPackError::MalformedExistingConfig {
        path: rel_str.to_string(),
        reason: format!("merged Codex TOML failed structural validation: {e}"),
    })?;

    finalize_managed_write(
        canon_install_root,
        &target,
        rel_str,
        PluginHostTarget::Codex,
        existing.as_deref(),
        &rendered,
        inspection.had_managed_entry,
        inspection.preserved_keys_count,
        backup_root,
    )
}

fn write_marker_bounded_agents_md(
    canon_install_root: &Path,
    rel_str: &str,
    manifest: &UniversalPluginPackManifest,
    workspace_str: &str,
    backup_root: &Path,
) -> Result<HostWriteOutcome, PluginPackError> {
    let rel = validate_relative_sandbox_path(rel_str)?;
    let target = canon_install_root.join(&rel);
    let existing = read_existing_utf8_nofollow(&target)?;
    let managed_block = render_codex_agents_md_block(manifest, workspace_str);

    let (rendered, had_managed_entry, preserved_keys_count) = match existing.as_deref() {
        None => (format!("{managed_block}\n"), false, 0usize),
        Some(raw) => merge_agents_md_markers(raw, &managed_block, rel_str)?,
    };

    finalize_managed_write(
        canon_install_root,
        &target,
        rel_str,
        PluginHostTarget::Codex,
        existing.as_deref(),
        &rendered,
        had_managed_entry,
        preserved_keys_count,
        backup_root,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_managed_write(
    canon_install_root: &Path,
    target: &Path,
    rel_str: &str,
    host: PluginHostTarget,
    existing: Option<&str>,
    rendered: &str,
    had_managed_entry: bool,
    preserved_keys_count: usize,
    backup_root: &Path,
) -> Result<HostWriteOutcome, PluginPackError> {
    let new_bytes = rendered.as_bytes();
    let changed = existing.is_none_or(|old| old.as_bytes() != new_bytes);
    let drift_detected = had_managed_entry && changed;

    let backup_id = if changed {
        if let Some(old_str) = existing {
            Some(write_backup_if_needed(
                backup_root,
                host,
                old_str.as_bytes(),
            )?)
        } else {
            None
        }
    } else {
        None
    };

    let action = if existing.is_none() {
        "created"
    } else if !changed {
        "unchanged"
    } else if drift_detected {
        "drift_repaired"
    } else {
        "updated"
    };

    if changed {
        write_atomic_nofollow_nlink1(canon_install_root, target, new_bytes, 0o600)?;
    }

    Ok(HostWriteOutcome {
        receipt: EmittedFileReceipt {
            relative_path: rel_str.to_string(),
            sha256: sha256_hex(new_bytes),
            byte_size: new_bytes.len(),
            action: action.to_string(),
        },
        drift_detected,
        drift_repaired: drift_detected,
        preserved_keys_count,
        backup_id,
    })
}

struct CodexTomlInspection {
    had_managed_entry: bool,
    preserved_keys_count: usize,
    preserved_cowok_env: BTreeMap<String, (String, String)>,
}

fn strip_toml_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_double = false;
    let mut in_single = false;
    let mut escaped = false;
    for (idx, &b) in bytes.iter().enumerate() {
        if in_double {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_double = false;
            }
        } else if in_single {
            if b == b'\'' {
                in_single = false;
            }
        } else {
            match b {
                b'"' => in_double = true,
                b'\'' => in_single = true,
                b'#' => return &line[..idx],
                _ => {}
            }
        }
    }
    line
}

fn split_toml_assignment(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut in_double = false;
    let mut in_single = false;
    let mut escaped = false;
    for (idx, &b) in bytes.iter().enumerate() {
        if in_double {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_double = false;
            }
        } else if in_single {
            if b == b'\'' {
                in_single = false;
            }
        } else {
            match b {
                b'"' => in_double = true,
                b'\'' => in_single = true,
                b'=' => return Some((line[..idx].trim(), line[idx + 1..].trim())),
                _ => {}
            }
        }
    }
    None
}

fn split_toml_key_path(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_double = false;
    let mut in_single = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if in_double {
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double = false;
            } else {
                current.push(ch);
            }
        } else if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
        } else {
            match ch {
                '"' => in_double = true,
                '\'' => in_single = true,
                '.' => {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                    current.clear();
                }
                other => current.push(other),
            }
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    parts
}

fn is_cowok_toml_section(section: &[String]) -> bool {
    section.len() >= 2 && section[0] == "mcp_servers" && section[1] == COWOK_TOML_KEY
}

fn inspect_existing_codex_toml(
    raw: &str,
    path_for_err: &str,
) -> Result<CodexTomlInspection, PluginPackError> {
    let mut had_managed_entry = false;
    let mut preserved_keys_count = 0usize;
    let mut preserved_cowok_env: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut current_section: Vec<String> = Vec::new();

    for line in raw.lines() {
        let stripped = strip_toml_inline_comment(line).trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with('[') && stripped.ends_with(']') {
            let inner = if stripped.starts_with("[[") && stripped.ends_with("]]") {
                &stripped[2..stripped.len() - 2]
            } else {
                &stripped[1..stripped.len() - 1]
            };
            let section = split_toml_key_path(inner);
            if is_cowok_toml_section(&section) {
                had_managed_entry = true;
            } else {
                preserved_keys_count += 1;
            }
            current_section = section;
            continue;
        }

        if let Some((lhs, rhs)) = split_toml_assignment(stripped) {
            let key_segs = split_toml_key_path(lhs);
            let is_dotted_or_inline_cowok_hazard = (current_section.is_empty()
                && ((key_segs.len() >= 2
                    && key_segs[0] == "mcp_servers"
                    && key_segs[1] == COWOK_TOML_KEY)
                    || (key_segs.len() == 1
                        && key_segs[0] == "mcp_servers"
                        && rhs.trim_start().starts_with('{')
                        && rhs.contains(COWOK_TOML_KEY))))
                || (current_section.len() == 1
                    && current_section[0] == "mcp_servers"
                    && key_segs.first().map(String::as_str) == Some(COWOK_TOML_KEY))
                || (current_section.len() == 2
                    && current_section[0] == "mcp_servers"
                    && current_section[1] == COWOK_TOML_KEY
                    && key_segs.first().map(String::as_str) == Some("env"));

            if is_dotted_or_inline_cowok_hazard {
                return Err(PluginPackError::MalformedExistingConfig {
                    path: path_for_err.to_string(),
                    reason: format!(
                        "unsupported dotted-key or inline-table declaration for mcp_servers.{COWOK_TOML_KEY}; use [mcp_servers.{COWOK_TOML_KEY}] / [mcp_servers.{COWOK_TOML_KEY}.env] table syntax"
                    ),
                });
            }

            let in_cowok_env = current_section.len() == 3
                && current_section[0] == "mcp_servers"
                && current_section[1] == COWOK_TOML_KEY
                && current_section[2] == "env";

            if in_cowok_env {
                let normalized_key = key_segs.join(".");
                if !normalized_key.is_empty() && !normalized_key.starts_with("COWOK_") {
                    preserved_cowok_env.insert(normalized_key, (lhs.to_string(), rhs.to_string()));
                    preserved_keys_count += 1;
                }
            } else if !is_cowok_toml_section(&current_section) {
                preserved_keys_count += 1;
            }
        }
    }

    Ok(CodexTomlInspection {
        had_managed_entry,
        preserved_keys_count,
        preserved_cowok_env,
    })
}

fn merge_agents_md_markers(
    existing_raw: &str,
    managed_block: &str,
    path_for_err: &str,
) -> Result<(String, bool, usize), PluginPackError> {
    let mut begin_lines: Vec<(usize, usize)> = Vec::new();
    let mut end_lines: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for chunk in existing_raw.split_inclusive('\n') {
        let trimmed = chunk.trim();
        if trimmed == AGENTS_MD_BEGIN_MARKER {
            begin_lines.push((offset, offset + chunk.len()));
        } else if trimmed == AGENTS_MD_END_MARKER {
            end_lines.push((offset, offset + chunk.len()));
        }
        offset += chunk.len();
    }
    if offset < existing_raw.len() {
        let tail = &existing_raw[offset..];
        let trimmed = tail.trim();
        if trimmed == AGENTS_MD_BEGIN_MARKER {
            begin_lines.push((offset, existing_raw.len()));
        } else if trimmed == AGENTS_MD_END_MARKER {
            end_lines.push((offset, existing_raw.len()));
        }
    }

    match (begin_lines.as_slice(), end_lines.as_slice()) {
        ([], []) => {
            let preserved_sections = count_nonempty_prose_lines(existing_raw);
            let trimmed = existing_raw.trim_end();
            let merged = if trimmed.is_empty() {
                format!("{managed_block}\n")
            } else {
                format!("{trimmed}\n\n{managed_block}\n")
            };
            Ok((merged, false, preserved_sections))
        }
        ([(b_start, _)], [(_, e_end)]) if *b_start < *e_end => {
            let before = &existing_raw[..*b_start];
            let after = &existing_raw[*e_end..];
            let preserved = count_nonempty_prose_lines(before) + count_nonempty_prose_lines(after);
            let mut merged = String::with_capacity(existing_raw.len() + managed_block.len());
            merged.push_str(before);
            merged.push_str(managed_block);
            if !after.is_empty() && !after.starts_with('\n') {
                merged.push('\n');
            }
            merged.push_str(after);
            if !merged.ends_with('\n') {
                merged.push('\n');
            }
            Ok((merged, true, preserved))
        }
        _ => Err(PluginPackError::MalformedExistingConfig {
            path: path_for_err.to_string(),
            reason: "unbalanced or duplicated COWOK-DESIGN-PACK markers in AGENTS.md".to_string(),
        }),
    }
}

fn count_nonempty_prose_lines(raw: &str) -> usize {
    raw.lines().filter(|l| !l.trim().is_empty()).count()
}

fn sanitize_agents_md_inline_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_control() || ch == '`' {
            continue;
        }
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

fn render_codex_agents_md_block(
    manifest: &UniversalPluginPackManifest,
    workspace_str: &str,
) -> String {
    let safe_workspace = sanitize_agents_md_inline_value(workspace_str);
    let skills_list = manifest
        .skill_ids
        .iter()
        .map(|s| format!("- `.codex/skills/{s}/SKILL.md`"))
        .collect::<Vec<_>>()
        .join("\n");
    let de_tools_list = manifest.de_tools.join(", ");
    let human_only_list = manifest
        .human_only_tools
        .iter()
        .map(|t| format!("- `{t}` (`permissionTier: HUMAN_ONLY`, `mcpCallable: false`)"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{AGENTS_MD_BEGIN_MARKER}\n## Cowok Design — Universal Multi-Harness Pack (Codex CLI)\n\n- **Pack ID:** `{}` (`v{}`)\n- **Telemetry Channel:** `codex`\n- **MCP Server:** `[mcp_servers.cowok_design]` (`stdio`)\n- **Workspace Root:** `{safe_workspace}`\n- **Feedback Tool:** `{}` (`mcpCallable: true`, Zero-PII scrubbed)\n- **7 Rust DE Tools:** {de_tools_list}\n\n### Embedded Craft Skills (7)\n{skills_list}\n\n### Immutable HUMAN_ONLY Gates (Never Bypassed via MCP)\n{human_only_list}\n{AGENTS_MD_END_MARKER}",
        manifest.pack_id, manifest.version, manifest.feedback_tool_name
    )
}

fn render_cursor_craft_rule_mdc(
    manifest: &UniversalPluginPackManifest,
    skills_registry: &EmbeddedBuiltinSkillsRegistry,
) -> String {
    let skills_summary = skills_registry
        .skills()
        .iter()
        .map(|s| {
            format!(
                "- `{}` (family: `{}`, sha256: `{}`)",
                s.id,
                s.family,
                &s.sha256[..12]
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "---\ndescription: Cowok Design Universal Multi-Harness Design-Craft & MCP Rule\nalwaysApply: true\ntelemetryChannel: cursor\npackId: {}\npackVersion: {}\nfeedbackTool: {}\n---\n\n# Cowok Design Craft Operating Rules (Cursor)\n\n- **MCP Server:** `cowok-design` (`stdio` via `.cursor/mcp.json`, channel `cursor`)\n- **Feedback Tool:** Invoke `{}` (`mcpCallable: true`) to submit Zero-PII support tickets or template requests.\n- **7 Rust DE Tools:** {}\n\n## Embedded Craft Skills (7)\n{}\n",
        manifest.pack_id,
        manifest.version,
        manifest.feedback_tool_name,
        manifest.feedback_tool_name,
        manifest.de_tools.join(", "),
        skills_summary
    )
}

fn render_cursor_security_rule_mdc(manifest: &UniversalPluginPackManifest) -> String {
    let locked = manifest
        .human_only_tools
        .iter()
        .map(|t| format!("- `{t}` -> `permissionTier: HUMAN_ONLY`, `mcpCallable: false`"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "---\ndescription: Cowok Design Immutable HUMAN_ONLY Security Gate Policy\nalwaysApply: true\ntelemetryChannel: cursor\n---\n\n# Immutable HUMAN_ONLY Security Gates\n\nExternal IDE agents MUST NEVER attempt to automate, bypass, or re-enable the following tools via MCP:\n\n{locked}\n\nAll destructive or governance-promotion actions require direct confirmation inside the Cowok Design Desktop/Web Workbench.\n"
    )
}

fn escape_toml_basic_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 8);
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) == 0x7F => {
                use std::fmt::Write as _;
                let _ = write!(&mut out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn normalize_ascii_tool_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_control()
            || matches!(
                ch,
                '\u{00AD}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{FEFF}'
            )
        {
            continue;
        }
        let mapped = match ch {
            'А' | 'а' | 'Α' | 'α' => 'a',
            'С' | 'с' => 'c',
            'Е' | 'е' | 'Ε' | 'ε' => 'e',
            'Н' | 'н' | 'Η' => 'h',
            'І' | 'і' | 'Ι' | 'ι' => 'i',
            'М' | 'м' | 'Μ' => 'm',
            'О' | 'о' | 'Ο' | 'ο' => 'o',
            'Р' | 'р' | 'Ρ' | 'ρ' => 'p',
            'Т' | 'т' | 'Τ' | 'τ' => 't',
            'Х' | 'х' | 'Χ' | 'χ' => 'x',
            other => other.to_ascii_lowercase(),
        };
        if mapped.is_ascii_alphanumeric() {
            out.push(mapped);
        }
    }
    out
}

fn is_inherently_human_only_folded(folded: &str) -> bool {
    folded.contains("ephemeraltool")
        || folded.contains("learncard")
        || folded.contains("learnpromote")
        || folded.contains("stagingplugin")
        || folded.contains("stagingpromote")
        || folded.contains("workspacedestructive")
        || folded.contains("humanonly")
}
