//! Default built-in plugin mount for the standalone `design-craft` 4-pillar repo.
//! Validates `plugin.json`, upstream REA `optionalPeers`, 7 Builder + 6 QC tools,
//! 5 agent contracts (including read-only `design-auditor`), and agentskills.io
//! `<= 100`-token `SKILL.md` frontmatter caps.

// @story 08.6
// @implements AC-01, AC-04, AC-05, AC-06

use crate::lifecycle::parse_semver_strict;
use crate::models::SkillManifest;
use crate::registry::SkillRegistry;
use crate::repackager::security_scanner::scan_skill_markdown_safety;
use crate::validator::{
    parse_and_validate_skill_frontmatter, FrontmatterMetadata, ValidationError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const DESIGN_CRAFT_PLUGIN_NAME: &str = "design-craft";
pub const DESIGN_CRAFT_SPEC_STANDARD: &str = "agentskills.io/v1";
pub const DESIGN_CRAFT_HOST_ID: &str = "cowok-design";
pub const DESIGN_CRAFT_RELATIVE_DIR: &str = "packages/builtin-skills/design-craft";
pub const SIBLING_FORGED_SKILLS_RELATIVE_DIR: &str = "packages/builtin-skills/skills";

pub const REQUIRED_BUILDER_TOOLS: &[&str] = &[
    "de-phase-router",
    "de-intake",
    "de-scan-sketch",
    "de-tokens-oklch",
    "de-brand-preview",
    "de-extract-contract",
    "de-patch-dom",
];

pub const REQUIRED_QC_AUDIT_TOOLS: &[&str] = &[
    "qc-a11y-contrast",
    "qc-anti-slop-linter",
    "qc-layout-overflow",
    "qc-chart-and-data",
    "qc-spec-parity",
    "qc-popular-coverage",
];

pub const REQUIRED_AGENTS: &[&str] = &[
    "design-orchestrator",
    "flow-architect",
    "brand-stylist",
    "ui-crafter",
    "design-auditor",
];

pub const REQUIRED_HOST_UPGRADE_KEYS: &[&str] =
    &["de-tokens-oklch", "de-patch-dom", "qc-layout-overflow"];

#[derive(Debug, Error)]
pub enum DefaultMountError {
    #[error("design-craft bundle directory not found at {0}")]
    BundleNotFound(PathBuf),
    #[error("symlink rejected under Zero-Trust mount policy: {0}")]
    SymlinkRejected(PathBuf),
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid plugin.json manifest: {0}")]
    InvalidManifest(String),
    #[error("invalid optionalPeers configuration: {0}")]
    InvalidOptionalPeer(String),
    #[error("missing required asset: {0}")]
    MissingAsset(String),
    #[error("agent contract violation: {0}")]
    AgentContractViolation(String),
    #[error("SKILL.md validation failed for {path}: {source}")]
    SkillValidation {
        path: String,
        #[source]
        source: ValidationError,
    },
    #[error("SKILL.md safety scan failed for {path}: {reason}")]
    SkillSafety { path: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalPeerSpec {
    pub id: String,
    pub kind: String,
    pub repo_url: String,
    #[serde(default)]
    pub install_hint: Option<String>,
    pub purpose: String,
    pub fallback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignCraftCapabilities {
    pub builder_tools: Vec<String>,
    pub qc_audit_tools: Vec<String>,
    pub host_upgrade_map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignCraftPluginManifest {
    pub name: String,
    pub version: String,
    pub spec_standard: String,
    pub description: String,
    pub default_for_hosts: Vec<String>,
    pub phase_skill_map: BTreeMap<String, Vec<String>>,
    pub optional_peers: Vec<OptionalPeerSpec>,
    pub capabilities: DesignCraftCapabilities,
    pub agents: Vec<String>,
    pub mcp_server: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedSkillSummary {
    pub name: String,
    pub relative_path: String,
    pub tokens: usize,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Debug)]
pub struct DesignCraftDefaultMount {
    pub bundle_dir: PathBuf,
    pub manifest: DesignCraftPluginManifest,
    pub skills: Vec<MountedSkillSummary>,
    pub registry: SkillRegistry,
    pub bundle_fingerprint_sha256: String,
    pub default_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentFrontmatter {
    name: String,
    role: String,
    product_write_allowed: bool,
    #[serde(default)]
    write_scope: Vec<String>,
    read_only: bool,
}

fn is_valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_valid_github_repo_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let mut parts = rest.split('/');
    let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let valid_seg = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    };
    valid_seg(owner) && valid_seg(repo)
}

fn extract_bounded_yaml_frontmatter(raw: &str) -> Option<&str> {
    let content = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let after_open = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))?;
    let mut offset = 0usize;
    for segment in after_open.split_inclusive('\n') {
        let line_no_nl = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line_no_nl.strip_suffix('\r').unwrap_or(line_no_nl);
        if line == "---" {
            return Some(after_open[..offset].trim());
        }
        offset += segment.len();
    }
    if after_open[offset..].trim_end_matches('\r') == "---" {
        return Some(after_open[..offset].trim());
    }
    None
}

fn verify_confined_dir_no_symlink(
    base_dir: &Path,
    canonical_base_dir: &Path,
    rel: &str,
) -> Result<Option<PathBuf>, DefaultMountError> {
    if rel.is_empty() || rel.contains('\\') {
        return Err(DefaultMountError::InvalidManifest(format!(
            "unsafe relative directory path '{rel}'"
        )));
    }
    let mut cursor = base_dir.to_path_buf();
    let mut count = 0usize;
    for comp in Path::new(rel).components() {
        match comp {
            std::path::Component::Normal(seg) => {
                count += 1;
                cursor.push(seg);
                let meta = match std::fs::symlink_metadata(&cursor) {
                    Ok(m) => m,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(source) => {
                        return Err(DefaultMountError::Io {
                            path: cursor,
                            source,
                        })
                    }
                };
                if meta.file_type().is_symlink() {
                    return Err(DefaultMountError::SymlinkRejected(cursor));
                }
            }
            _ => {
                return Err(DefaultMountError::InvalidManifest(format!(
                    "unsafe relative directory path '{rel}'"
                )));
            }
        }
    }
    if count == 0 {
        return Err(DefaultMountError::InvalidManifest(format!(
            "unsafe relative directory path '{rel}'"
        )));
    }
    let meta = std::fs::symlink_metadata(&cursor).map_err(|source| DefaultMountError::Io {
        path: cursor.clone(),
        source,
    })?;
    if !meta.is_dir() {
        return Ok(None);
    }
    let canonical = std::fs::canonicalize(&cursor).map_err(|source| DefaultMountError::Io {
        path: cursor.clone(),
        source,
    })?;
    if !canonical.starts_with(canonical_base_dir) {
        return Err(DefaultMountError::SymlinkRejected(cursor));
    }
    Ok(Some(cursor))
}

fn verify_confined_no_symlink(
    bundle_dir: &Path,
    canonical_bundle_dir: &Path,
    rel: &str,
) -> Result<PathBuf, DefaultMountError> {
    if rel.is_empty() || rel.contains('\\') {
        return Err(DefaultMountError::InvalidManifest(format!(
            "unsafe relative asset path '{rel}'"
        )));
    }
    let mut cursor = bundle_dir.to_path_buf();
    let mut component_count = 0usize;
    for comp in Path::new(rel).components() {
        match comp {
            std::path::Component::Normal(seg) => {
                component_count += 1;
                cursor.push(seg);
                let meta = match std::fs::symlink_metadata(&cursor) {
                    Ok(m) => m,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(DefaultMountError::MissingAsset(rel.to_string()));
                    }
                    Err(source) => {
                        return Err(DefaultMountError::Io {
                            path: cursor,
                            source,
                        });
                    }
                };
                if meta.file_type().is_symlink() {
                    return Err(DefaultMountError::SymlinkRejected(cursor));
                }
            }
            _ => {
                return Err(DefaultMountError::InvalidManifest(format!(
                    "unsafe relative asset path '{rel}'"
                )));
            }
        }
    }
    if component_count == 0 {
        return Err(DefaultMountError::InvalidManifest(format!(
            "unsafe relative asset path '{rel}'"
        )));
    }
    let final_meta =
        std::fs::symlink_metadata(&cursor).map_err(|source| DefaultMountError::Io {
            path: cursor.clone(),
            source,
        })?;
    if !final_meta.is_file() {
        return Err(DefaultMountError::MissingAsset(rel.to_string()));
    }
    let canonical_abs = std::fs::canonicalize(&cursor).map_err(|source| DefaultMountError::Io {
        path: cursor.clone(),
        source,
    })?;
    if !canonical_abs.starts_with(canonical_bundle_dir) {
        return Err(DefaultMountError::SymlinkRejected(cursor));
    }
    Ok(cursor)
}

fn read_confined_file(
    bundle_dir: &Path,
    canonical_bundle_dir: &Path,
    rel: &str,
) -> Result<String, DefaultMountError> {
    let abs = verify_confined_no_symlink(bundle_dir, canonical_bundle_dir, rel)?;
    read_file_no_follow(&abs)
}

fn read_file_no_follow(path: &Path) -> Result<String, DefaultMountError> {
    use std::io::Read;
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = opts.open(path).map_err(|source| DefaultMountError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|source| DefaultMountError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(content)
}

fn validate_manifest_contract(
    bundle_dir: &Path,
    canonical_bundle_dir: &Path,
    manifest: &DesignCraftPluginManifest,
) -> Result<BTreeSet<String>, DefaultMountError> {
    if manifest.name != DESIGN_CRAFT_PLUGIN_NAME {
        return Err(DefaultMountError::InvalidManifest(format!(
            "expected name '{}', found '{}'",
            DESIGN_CRAFT_PLUGIN_NAME, manifest.name
        )));
    }
    parse_semver_strict(&manifest.version).map_err(|e| {
        DefaultMountError::InvalidManifest(format!("invalid semver '{}': {e}", manifest.version))
    })?;
    if manifest.spec_standard != DESIGN_CRAFT_SPEC_STANDARD {
        return Err(DefaultMountError::InvalidManifest(format!(
            "expected specStandard '{}', found '{}'",
            DESIGN_CRAFT_SPEC_STANDARD, manifest.spec_standard
        )));
    }
    if !manifest
        .default_for_hosts
        .iter()
        .any(|h| h == DESIGN_CRAFT_HOST_ID)
    {
        return Err(DefaultMountError::InvalidManifest(format!(
            "defaultForHosts must include '{}'",
            DESIGN_CRAFT_HOST_ID
        )));
    }
    validate_capabilities(&manifest.capabilities)?;
    let mut hashed_assets = BTreeSet::new();
    let fallback_rel =
        validate_optional_peers(bundle_dir, canonical_bundle_dir, &manifest.optional_peers)?;
    hashed_assets.insert(fallback_rel);
    let agent_assets = validate_agents(bundle_dir, canonical_bundle_dir, &manifest.agents)?;
    hashed_assets.extend(agent_assets);
    let core_assets = validate_core_assets(bundle_dir, canonical_bundle_dir, manifest)?;
    hashed_assets.extend(core_assets);
    Ok(hashed_assets)
}

fn validate_capabilities(caps: &DesignCraftCapabilities) -> Result<(), DefaultMountError> {
    let builders: BTreeSet<&str> = caps.builder_tools.iter().map(String::as_str).collect();
    for required in REQUIRED_BUILDER_TOOLS {
        if !builders.contains(required) {
            return Err(DefaultMountError::InvalidManifest(format!(
                "missing required builder tool '{required}'"
            )));
        }
    }
    let qc_tools: BTreeSet<&str> = caps.qc_audit_tools.iter().map(String::as_str).collect();
    for required in REQUIRED_QC_AUDIT_TOOLS {
        if !qc_tools.contains(required) {
            return Err(DefaultMountError::InvalidManifest(format!(
                "missing required qcAuditTool '{required}'"
            )));
        }
    }
    for key in REQUIRED_HOST_UPGRADE_KEYS {
        if !caps.host_upgrade_map.contains_key(*key) {
            return Err(DefaultMountError::InvalidManifest(format!(
                "hostUpgradeMap missing required native upgrade entry '{key}'"
            )));
        }
    }
    Ok(())
}

fn validate_optional_peers(
    bundle_dir: &Path,
    canonical_bundle_dir: &Path,
    peers: &[OptionalPeerSpec],
) -> Result<String, DefaultMountError> {
    let Some(rea) = peers.iter().find(|p| p.id == "rea-upstream") else {
        return Err(DefaultMountError::InvalidOptionalPeer(
            "optionalPeers must declare 'rea-upstream'".to_string(),
        ));
    };
    if rea.kind != "upstream-github-mcp" {
        return Err(DefaultMountError::InvalidOptionalPeer(format!(
            "rea-upstream kind must be 'upstream-github-mcp', found '{}'",
            rea.kind
        )));
    }
    if !is_valid_github_repo_url(&rea.repo_url) {
        return Err(DefaultMountError::InvalidOptionalPeer(format!(
            "rea-upstream repoUrl must match https://github.com/<owner>/<repo>, found '{}'",
            rea.repo_url
        )));
    }
    if let Some(hint) = &rea.install_hint {
        let expected = format!("git clone {}", rea.repo_url);
        if hint != &expected {
            return Err(DefaultMountError::InvalidOptionalPeer(format!(
                "rea-upstream installHint must equal '{expected}', found '{hint}'"
            )));
        }
    }
    verify_confined_no_symlink(bundle_dir, canonical_bundle_dir, &rea.fallback).map_err(|err| {
        match err {
            DefaultMountError::InvalidManifest(msg) => DefaultMountError::InvalidOptionalPeer(msg),
            other => other,
        }
    })?;
    Ok(rea.fallback.clone())
}

fn validate_agents(
    bundle_dir: &Path,
    canonical_bundle_dir: &Path,
    agents: &[String],
) -> Result<Vec<String>, DefaultMountError> {
    let declared: BTreeSet<&str> = agents.iter().map(String::as_str).collect();
    for req in REQUIRED_AGENTS {
        if !declared.contains(req) {
            return Err(DefaultMountError::AgentContractViolation(format!(
                "missing required agent '{req}' in plugin.json"
            )));
        }
    }
    let mut agent_rels = Vec::with_capacity(declared.len());
    for agent_name in declared {
        if !is_valid_agent_name(agent_name) {
            return Err(DefaultMountError::AgentContractViolation(format!(
                "invalid agent name '{agent_name}' (must match ^[a-z0-9-]+$)"
            )));
        }
        let rel = format!("agents/{agent_name}.md");
        let content = read_confined_file(bundle_dir, canonical_bundle_dir, &rel)?;
        scan_skill_markdown_safety(&rel, &content).map_err(|e| DefaultMountError::SkillSafety {
            path: rel.clone(),
            reason: e.to_string(),
        })?;
        let yaml = extract_bounded_yaml_frontmatter(&content).ok_or_else(|| {
            DefaultMountError::AgentContractViolation(format!(
                "{rel} must start with bounded YAML frontmatter (--- ... ---)"
            ))
        })?;
        let fm: AgentFrontmatter = serde_yaml::from_str(yaml).map_err(|e| {
            DefaultMountError::AgentContractViolation(format!(
                "invalid YAML frontmatter in {rel}: {e}"
            ))
        })?;
        if fm.name != agent_name || fm.role.trim().is_empty() {
            return Err(DefaultMountError::AgentContractViolation(format!(
                "{rel} frontmatter name/role mismatch for '{agent_name}'"
            )));
        }
        if agent_name == "design-auditor"
            && (fm.product_write_allowed || !fm.read_only || !fm.write_scope.is_empty())
        {
            return Err(DefaultMountError::AgentContractViolation(
                "design-auditor frontmatter must enforce product_write_allowed: false, read_only: true, and write_scope: []"
                    .to_string(),
            ));
        }
        agent_rels.push(rel);
    }
    Ok(agent_rels)
}

fn validate_core_assets(
    bundle_dir: &Path,
    canonical_bundle_dir: &Path,
    manifest: &DesignCraftPluginManifest,
) -> Result<Vec<String>, DefaultMountError> {
    let required_files = [
        manifest.mcp_server.as_str(),
        "flows/cognitive-progressive.yaml",
        "bin/design-craft.mjs",
        "engine/path-jail.mjs",
        "engine/builder-tools.mjs",
        "engine/qc-tools.mjs",
        "catalogs/popular-components.fe.json",
        "catalogs/popular-components.deck.json",
        "catalogs/reference-archetypes.json",
        "catalogs/domain-blueprints.json",
        "rubrics/design-qc-rubric.json",
    ];
    let mut validated = Vec::with_capacity(required_files.len());
    for rel in required_files {
        verify_confined_no_symlink(bundle_dir, canonical_bundle_dir, rel)?;
        validated.push(rel.to_string());
    }
    Ok(validated)
}

fn collect_skill_files(
    dir: &Path,
    canonical_root: &Path,
    acc: &mut Vec<PathBuf>,
) -> Result<(), DefaultMountError> {
    let dir_meta = std::fs::symlink_metadata(dir).map_err(|source| DefaultMountError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    if dir_meta.file_type().is_symlink() {
        return Err(DefaultMountError::SymlinkRejected(dir.to_path_buf()));
    }
    let canonical_dir = std::fs::canonicalize(dir).map_err(|source| DefaultMountError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    if !canonical_dir.starts_with(canonical_root) {
        return Err(DefaultMountError::SymlinkRejected(dir.to_path_buf()));
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|source| DefaultMountError::Io {
            path: dir.to_path_buf(),
            source,
        })?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for entry in entries {
        let meta = std::fs::symlink_metadata(&entry).map_err(|source| DefaultMountError::Io {
            path: entry.clone(),
            source,
        })?;
        if meta.file_type().is_symlink() {
            return Err(DefaultMountError::SymlinkRejected(entry));
        }
        if meta.is_dir() {
            collect_skill_files(&entry, canonical_root, acc)?;
        } else if meta.is_file() && entry.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            let canonical_entry =
                std::fs::canonicalize(&entry).map_err(|source| DefaultMountError::Io {
                    path: entry.clone(),
                    source,
                })?;
            if !canonical_entry.starts_with(canonical_root) {
                return Err(DefaultMountError::SymlinkRejected(entry));
            }
            acc.push(entry);
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn to_skill_manifest(meta: &FrontmatterMetadata) -> SkillManifest {
    SkillManifest {
        name: meta.name.clone(),
        version: meta.version.clone().unwrap_or_else(|| "1.0.0".to_string()),
        description: meta.description.clone(),
        entry_point: meta
            .entry_point
            .clone()
            .unwrap_or_else(|| "SKILL.md".to_string()),
        capabilities_required: meta.capabilities_required.clone(),
        risk_tier: meta.risk_tier.unwrap_or(1),
        parameters: None,
        tags: meta.tags.clone(),
        prompt_overview: None,
    }
}

fn mount_skill_entries(
    skill_entries: Vec<(String, PathBuf)>,
    registry: &mut SkillRegistry,
    skills: &mut Vec<MountedSkillSummary>,
    bundle_hasher: &mut Sha256,
) -> Result<(), DefaultMountError> {
    let mut seen_names = BTreeSet::new();
    for (rel, file_path) in skill_entries {
        let raw = read_file_no_follow(&file_path)?;
        let meta = parse_and_validate_skill_frontmatter(&raw).map_err(|source| {
            DefaultMountError::SkillValidation {
                path: rel.clone(),
                source,
            }
        })?;
        scan_skill_markdown_safety(&rel, &raw).map_err(|e| DefaultMountError::SkillSafety {
            path: rel.clone(),
            reason: e.to_string(),
        })?;
        if !seen_names.insert(meta.name.clone()) {
            continue;
        }
        let digest = sha256_hex(raw.as_bytes());
        bundle_hasher.update(rel.as_bytes());
        bundle_hasher.update(b"\0");
        bundle_hasher.update(digest.as_bytes());
        bundle_hasher.update(b"\n");
        registry.register_with_hash(to_skill_manifest(&meta), Some(digest.clone()));
        skills.push(MountedSkillSummary {
            name: meta.name,
            relative_path: rel,
            tokens: meta.tokens,
            bytes: meta.bytes,
            sha256: digest,
        });
    }
    Ok(())
}

/// Mounts and validates `packages/builtin-skills/design-craft` as the default
/// built-in plugin repository for `cowok-design`.
pub fn mount_default_design_craft_plugin(
    repo_root: &Path,
) -> Result<DesignCraftDefaultMount, DefaultMountError> {
    let canonical_repo_root = std::fs::canonicalize(repo_root).map_err(|_| {
        DefaultMountError::BundleNotFound(repo_root.join(DESIGN_CRAFT_RELATIVE_DIR))
    })?;
    let Some(bundle_dir) =
        verify_confined_dir_no_symlink(repo_root, &canonical_repo_root, DESIGN_CRAFT_RELATIVE_DIR)?
    else {
        return Err(DefaultMountError::BundleNotFound(
            repo_root.join(DESIGN_CRAFT_RELATIVE_DIR),
        ));
    };
    let canonical_bundle_dir =
        std::fs::canonicalize(&bundle_dir).map_err(|source| DefaultMountError::Io {
            path: bundle_dir.clone(),
            source,
        })?;

    let manifest_raw = read_confined_file(&bundle_dir, &canonical_bundle_dir, "plugin.json")?;
    let manifest: DesignCraftPluginManifest = serde_json::from_str(&manifest_raw)
        .map_err(|e| DefaultMountError::InvalidManifest(e.to_string()))?;
    let core_assets = validate_manifest_contract(&bundle_dir, &canonical_bundle_dir, &manifest)?;

    let Some(skills_root) =
        verify_confined_dir_no_symlink(&bundle_dir, &canonical_bundle_dir, "skills")?
    else {
        return Err(DefaultMountError::MissingAsset("skills/".to_string()));
    };
    let mut bundle_skill_files = Vec::new();
    collect_skill_files(&skills_root, &canonical_bundle_dir, &mut bundle_skill_files)?;
    if bundle_skill_files.is_empty() {
        return Err(DefaultMountError::MissingAsset(
            "skills/**/SKILL.md".to_string(),
        ));
    }

    let mut skill_entries: Vec<(String, PathBuf)> = bundle_skill_files
        .into_iter()
        .map(|file_path| {
            let rel = file_path
                .strip_prefix(&bundle_dir)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");
            (rel, file_path)
        })
        .collect();

    if let Some(sibling_root) = verify_confined_dir_no_symlink(
        repo_root,
        &canonical_repo_root,
        SIBLING_FORGED_SKILLS_RELATIVE_DIR,
    )? {
        let canonical_sibling_root =
            std::fs::canonicalize(&sibling_root).map_err(|source| DefaultMountError::Io {
                path: sibling_root.clone(),
                source,
            })?;
        let mut sibling_files = Vec::new();
        collect_skill_files(&sibling_root, &canonical_sibling_root, &mut sibling_files)?;
        for file_path in sibling_files {
            let suffix = file_path
                .strip_prefix(&sibling_root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");
            skill_entries.push((format!("skills/craft/{suffix}"), file_path));
        }
    }

    let mut registry = SkillRegistry::new();
    let mut skills = Vec::with_capacity(skill_entries.len());
    let mut bundle_hasher = Sha256::new();
    bundle_hasher.update(b"plugin.json\0");
    bundle_hasher.update(sha256_hex(manifest_raw.as_bytes()).as_bytes());
    bundle_hasher.update(b"\n");

    for asset_rel in &core_assets {
        let asset_raw = read_confined_file(&bundle_dir, &canonical_bundle_dir, asset_rel)?;
        bundle_hasher.update(asset_rel.as_bytes());
        bundle_hasher.update(b"\0");
        bundle_hasher.update(sha256_hex(asset_raw.as_bytes()).as_bytes());
        bundle_hasher.update(b"\n");
    }

    mount_skill_entries(
        skill_entries,
        &mut registry,
        &mut skills,
        &mut bundle_hasher,
    )?;

    for (phase, phase_skills) in &manifest.phase_skill_map {
        for skill_name in phase_skills {
            if registry.skill_md_sha256(skill_name).is_none() {
                return Err(DefaultMountError::MissingAsset(format!(
                    "phaseSkillMap '{phase}' references unmounted skill '{skill_name}'"
                )));
            }
        }
    }

    Ok(DesignCraftDefaultMount {
        bundle_dir,
        manifest,
        skills,
        registry,
        bundle_fingerprint_sha256: format!("{:x}", bundle_hasher.finalize()),
        default_enabled: true,
    })
}
