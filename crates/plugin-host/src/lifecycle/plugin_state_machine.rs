// @story 08.3
// @implements AC-01, AC-02, AC-03, AC-05
use crate::lifecycle::breaking_diff::{
    detect_breaking_change, parse_semver_strict, BreakingDiffReport, SemVerParseError,
};
use crate::models::SkillManifest;
use crate::repackager::repackager_engine::{
    load_verified_staged_bundle_from_dir, validate_safe_segment,
};
use crate::repackager::security_scanner::{
    classify_and_validate_source_asset, scan_external_script_security, scan_skill_markdown_safety,
    validate_skill_metadata_text, RepackageAssetBucket,
};
use crate::validator::parse_and_validate_skill_frontmatter;
use domain::models::SkillPlugin;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

const RESERVED_ROOT_NAMES: &[&str] = &[
    "skill.md",
    "plugin.json",
    "discovery-stub.json",
    "skills-lock.json",
];
const SCRIPT_EXTENSIONS: &[&str] = &[
    "py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "sh", "bash", "zsh", "rb", "pl",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        export,
        export_to = "../../../packages/shared-types/src/generated/",
        rename_all = "SCREAMING_SNAKE_CASE"
    )
)]
pub enum PluginState {
    Discovered,
    Validated,
    Installed,
    Active,
    Deprecated,
    Archived,
}

impl PluginState {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Discovered => "DISCOVERED",
            Self::Validated => "VALIDATED",
            Self::Installed => "INSTALLED",
            Self::Active => "ACTIVE",
            Self::Deprecated => "DEPRECATED",
            Self::Archived => "ARCHIVED",
        }
    }

    /// Parses canonical SCREAMING_SNAKE_CASE states, Story 04.3/04.4 'ENABLED' alias, and PascalCase.
    pub fn parse_compat(raw: &str) -> Result<Self, PluginLifecycleError> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "DISCOVERED" => Ok(Self::Discovered),
            "VALIDATED" => Ok(Self::Validated),
            "INSTALLED" => Ok(Self::Installed),
            "ACTIVE" | "ENABLED" => Ok(Self::Active),
            "DEPRECATED" => Ok(Self::Deprecated),
            "ARCHIVED" => Ok(Self::Archived),
            _ => Err(PluginLifecycleError::UnknownPluginState(raw.to_string())),
        }
    }
}

impl Serialize for PluginState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_wire_str())
    }
}

impl<'de> Deserialize<'de> for PluginState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse_compat(&raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct StagedScriptFile {
    pub rel_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct StagedAssetFile {
    pub rel_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct StagedPluginBundle {
    pub manifest: SkillManifest,
    pub skill_md: String,
    pub scripts: Vec<StagedScriptFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<StagedScriptFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<StagedAssetFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ManifestUpdateOutcome {
    Applied {
        report: BreakingDiffReport,
    },
    StagedForReview {
        active_version: String,
        candidate_version: String,
        reasons: Vec<String>,
        report: BreakingDiffReport,
    },
    IdempotentStaged {
        active_version: String,
        candidate_version: String,
        reasons: Vec<String>,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PluginLifecycleError {
    #[error("Unknown plugin state: '{0}'")]
    UnknownPluginState(String),
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: PluginState, to: PluginState },
    #[error("Quarantined plugin cannot be reactivated from DEPRECATED to ACTIVE: {reason}")]
    QuarantinedReactivationForbidden { reason: String },
    #[error("RevisionConflict: expected revision {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("Pending staging candidate must be promoted or discarded before submitting a different update")]
    PendingStagingMustBeResolved,
    #[error("Non-active state {state:?} cannot apply breaking change or downgrade directly ({})", reasons.join("; "))]
    NonActiveBreakingUpdateRejected {
        state: PluginState,
        reasons: Vec<String>,
    },
    #[error("No staged breaking-change candidate available to promote or discard")]
    NoStagedCandidate,
    #[error("StagingPromotionRejected: {0}")]
    StagingPromotionRejected(String),
    #[error("InvalidQuarantineReason: quarantine reason must not be empty")]
    InvalidQuarantineReason,
    #[error("Invalid manifest field: {0}")]
    InvalidManifestField(String),
    #[error("InvalidPersistedState: {0}")]
    InvalidPersistedState(String),
    #[error("{0}")]
    InvalidSemVer(String),
}

impl From<SemVerParseError> for PluginLifecycleError {
    fn from(value: SemVerParseError) -> Self {
        Self::InvalidSemVer(value.to_string())
    }
}

fn default_scope_segment() -> String {
    "local-default".to_string()
}

fn derive_plugin_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "unnamed-plugin".to_string()
    } else {
        let capped: String = trimmed.chars().take(64).collect();
        let clean = capped.trim_matches('-');
        if clean.is_empty() {
            "unnamed-plugin".to_string()
        } else {
            clean.to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedPluginSnapshot {
    pub id: String,
    #[serde(default = "default_scope_segment")]
    pub tenant_id: String,
    #[serde(default = "default_scope_segment")]
    pub project_id: String,
    #[serde(default)]
    pub plugin_slug: String,
    pub name: String,
    pub version: String,
    pub state: PluginState,
    pub revision: u64,
    pub breaking_change_isolated: bool,
    pub staging_version: Option<String>,
    pub staging_manifest: Option<SkillManifest>,
    #[serde(default)]
    pub staging_diff_reasons: Vec<String>,
    pub manifest: SkillManifest,
    pub quarantined: bool,
    pub quarantine_reason: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        export,
        export_to = "../../../packages/shared-types/src/generated/",
        rename = "ManagedPluginRecord"
    )
)]
pub struct ManagedPlugin {
    id: String,
    tenant_id: String,
    project_id: String,
    plugin_slug: String,
    name: String,
    version: String,
    state: PluginState,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    revision: u64,
    breaking_change_isolated: bool,
    staging_version: Option<String>,
    staging_manifest: Option<SkillManifest>,
    staging_diff_reasons: Vec<String>,
    manifest: SkillManifest,
    quarantined: bool,
    quarantine_reason: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    updated_at: i64,
}

impl<'de> Deserialize<'de> for ManagedPlugin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let snap = ManagedPluginSnapshot::deserialize(deserializer)?;
        Self::from_persisted(snap).map_err(serde::de::Error::custom)
    }
}

fn is_disallowed_path_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c as u32,
            0x200B..=0x200F | 0x2028..=0x202E | 0x2066..=0x2069 | 0xFEFF | 0xE0001..=0xE007F
        )
}

fn validate_safe_rel_bundle_path(
    rel_path: &str,
    kind: &str,
) -> Result<String, PluginLifecycleError> {
    if rel_path.is_empty()
        || rel_path.starts_with('/')
        || rel_path.contains('\\')
        || rel_path.chars().any(is_disallowed_path_char)
        || rel_path
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(PluginLifecycleError::StagingPromotionRejected(format!(
            "Unsafe staged {kind} path: '{rel_path}'"
        )));
    }
    let lower = rel_path.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    if RESERVED_ROOT_NAMES.contains(&base) {
        return Err(PluginLifecycleError::StagingPromotionRejected(format!(
            "Reserved bundle filename disallowed in {kind}: '{rel_path}'"
        )));
    }
    Ok(base.to_string())
}

fn validate_safe_rel_script_path(rel_path: &str) -> Result<(), PluginLifecycleError> {
    let base = validate_safe_rel_bundle_path(rel_path, "script")?;
    let ext = base.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if !SCRIPT_EXTENSIONS.contains(&ext) {
        return Err(PluginLifecycleError::StagingPromotionRejected(format!(
            "Unsupported staged script extension on '{rel_path}'"
        )));
    }
    Ok(())
}

fn normalize_manifest(mut manifest: SkillManifest) -> SkillManifest {
    manifest.name = manifest.name.trim().to_string();
    manifest.version = manifest.version.trim().to_string();
    manifest.description = manifest.description.trim().to_string();
    manifest.entry_point = manifest.entry_point.trim().to_string();
    manifest
}

fn validate_manifest_contract(manifest: &SkillManifest) -> Result<(), PluginLifecycleError> {
    if manifest.name.trim().is_empty()
        || manifest.description.trim().is_empty()
        || manifest.entry_point.trim().is_empty()
    {
        return Err(PluginLifecycleError::InvalidManifestField(
            "name, description, and entry_point must not be empty".to_string(),
        ));
    }
    if !(1..=3).contains(&manifest.risk_tier) {
        return Err(PluginLifecycleError::InvalidManifestField(format!(
            "risk_tier {} out of valid 1..=3 contract range",
            manifest.risk_tier
        )));
    }
    if let Some(params) = manifest.parameters.as_ref() {
        if !params.is_null() && !params.is_object() && !params.is_boolean() {
            return Err(PluginLifecycleError::InvalidManifestField(
                "parameters must be a JSON object, boolean schema, or null".to_string(),
            ));
        }
    }
    validate_skill_metadata_text("manifest.name", &manifest.name)
        .map_err(|e| PluginLifecycleError::InvalidManifestField(e.to_string()))?;
    validate_skill_metadata_text("manifest.description", &manifest.description)
        .map_err(|e| PluginLifecycleError::InvalidManifestField(e.to_string()))?;
    if let Some(overview) = manifest.prompt_overview.as_deref() {
        scan_skill_markdown_safety("manifest.prompt_overview", overview)
            .map_err(|e| PluginLifecycleError::InvalidManifestField(e.to_string()))?;
    }
    parse_semver_strict(&manifest.version)?;
    Ok(())
}

fn validate_staged_bundle_contents(
    bundle: &StagedPluginBundle,
) -> Result<(), PluginLifecycleError> {
    let fm = parse_and_validate_skill_frontmatter(&bundle.skill_md).map_err(|e| {
        PluginLifecycleError::StagingPromotionRejected(format!("Invalid SKILL.md: {e}"))
    })?;
    let fm_caps: BTreeSet<&str> = fm
        .capabilities_required
        .iter()
        .map(String::as_str)
        .collect();
    let man_caps: BTreeSet<&str> = bundle
        .manifest
        .capabilities_required
        .iter()
        .map(String::as_str)
        .collect();
    let fm_entry = fm.entry_point.as_deref().unwrap_or("SKILL.md");
    if fm.name != bundle.manifest.name
        || fm.version.as_deref() != Some(bundle.manifest.version.as_str())
        || fm.description != bundle.manifest.description
        || fm_entry != bundle.manifest.entry_point
        || fm.risk_tier != Some(bundle.manifest.risk_tier)
        || fm_caps != man_caps
    {
        return Err(PluginLifecycleError::StagingPromotionRejected(
            "SKILL.md frontmatter diverges from staged manifest (identity, entry_point, risk_tier, or capabilities)".to_string(),
        ));
    }
    scan_skill_markdown_safety("SKILL.md", &bundle.skill_md).map_err(|e| {
        PluginLifecycleError::StagingPromotionRejected(format!("SKILL.md safety failed: {e}"))
    })?;

    let mut seen_paths = BTreeSet::new();
    for script in &bundle.scripts {
        validate_safe_rel_script_path(&script.rel_path)?;
        if !seen_paths.insert(script.rel_path.clone()) {
            return Err(PluginLifecycleError::StagingPromotionRejected(format!(
                "Duplicate staged script path: '{}'",
                script.rel_path
            )));
        }
        scan_external_script_security(&script.rel_path, &script.content).map_err(|e| {
            PluginLifecycleError::StagingPromotionRejected(format!(
                "Script '{}' rejected: {e}",
                script.rel_path
            ))
        })?;
    }
    if bundle.manifest.entry_point != "SKILL.md"
        && !seen_paths.contains(&bundle.manifest.entry_point)
    {
        return Err(PluginLifecycleError::StagingPromotionRejected(format!(
            "Manifest entry_point '{}' missing from verified staged scripts",
            bundle.manifest.entry_point
        )));
    }
    let mut seen_refs = BTreeSet::new();
    for reference in &bundle.references {
        validate_safe_rel_bundle_path(&reference.rel_path, "reference")?;
        if !seen_refs.insert(reference.rel_path.clone()) {
            return Err(PluginLifecycleError::StagingPromotionRejected(format!(
                "Duplicate staged reference path: '{}'",
                reference.rel_path
            )));
        }
        let bucket =
            classify_and_validate_source_asset(&reference.rel_path, reference.content.as_bytes())
                .map_err(|e| {
                PluginLifecycleError::StagingPromotionRejected(format!(
                    "Reference '{}' rejected: {e}",
                    reference.rel_path
                ))
            })?;
        if bucket != RepackageAssetBucket::References {
            return Err(PluginLifecycleError::StagingPromotionRejected(format!(
                "Staged reference '{}' is not a valid reference document",
                reference.rel_path
            )));
        }
        scan_skill_markdown_safety(&reference.rel_path, &reference.content).map_err(|e| {
            PluginLifecycleError::StagingPromotionRejected(format!(
                "Reference '{}' rejected: {e}",
                reference.rel_path
            ))
        })?;
    }
    let mut seen_assets = BTreeSet::new();
    for asset in &bundle.assets {
        validate_safe_rel_bundle_path(&asset.rel_path, "asset")?;
        if !seen_assets.insert(asset.rel_path.clone()) {
            return Err(PluginLifecycleError::StagingPromotionRejected(format!(
                "Duplicate staged asset path: '{}'",
                asset.rel_path
            )));
        }
        let bucket =
            classify_and_validate_source_asset(&asset.rel_path, &asset.bytes).map_err(|e| {
                PluginLifecycleError::StagingPromotionRejected(format!(
                    "Asset '{}' rejected: {e}",
                    asset.rel_path
                ))
            })?;
        match bucket {
            RepackageAssetBucket::AssetsText => {
                let text = std::str::from_utf8(&asset.bytes).map_err(|_| {
                    PluginLifecycleError::StagingPromotionRejected(format!(
                        "Text asset '{}' must be valid UTF-8",
                        asset.rel_path
                    ))
                })?;
                scan_skill_markdown_safety(&asset.rel_path, text).map_err(|e| {
                    PluginLifecycleError::StagingPromotionRejected(format!(
                        "Text asset '{}' rejected: {e}",
                        asset.rel_path
                    ))
                })?;
            }
            RepackageAssetBucket::AssetsBinaryImage => {}
            _ => {
                return Err(PluginLifecycleError::StagingPromotionRejected(format!(
                    "Staged asset '{}' is not a valid asset file",
                    asset.rel_path
                )));
            }
        }
    }
    Ok(())
}

impl ManagedPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Result<Self, PluginLifecycleError> {
        let trimmed_name = name.trim();
        let manifest = SkillManifest {
            name: trimmed_name.to_string(),
            version: version.to_string(),
            description: format!("Managed plugin {trimmed_name}"),
            entry_point: "SKILL.md".to_string(),
            capabilities_required: Vec::new(),
            risk_tier: 2,
            parameters: None,
            tags: Vec::new(),
            prompt_overview: None,
        };
        Self::with_manifest(id, manifest)
    }

    pub fn with_manifest(id: &str, manifest: SkillManifest) -> Result<Self, PluginLifecycleError> {
        let slug = derive_plugin_slug(&manifest.name);
        Self::with_scope("local-default", "local-default", id, &slug, manifest)
    }

    pub fn with_scope(
        tenant_id: &str,
        project_id: &str,
        id: &str,
        plugin_slug: &str,
        manifest: SkillManifest,
    ) -> Result<Self, PluginLifecycleError> {
        let clean = normalize_manifest(manifest);
        let [tid, pid, cid, slug] = [
            tenant_id.trim(),
            project_id.trim(),
            id.trim(),
            plugin_slug.trim(),
        ];
        if tid.is_empty() || pid.is_empty() || cid.is_empty() || slug.is_empty() {
            return Err(PluginLifecycleError::InvalidManifestField(
                "tenant_id, project_id, id, and plugin_slug must not be empty".to_string(),
            ));
        }
        validate_safe_segment("plugin_slug", slug)
            .map_err(PluginLifecycleError::InvalidManifestField)?;
        validate_manifest_contract(&clean)?;
        Ok(Self {
            id: cid.to_string(),
            tenant_id: tid.to_string(),
            project_id: pid.to_string(),
            plugin_slug: slug.to_string(),
            name: clean.name.clone(),
            version: clean.version.clone(),
            state: PluginState::Discovered,
            revision: 1,
            breaking_change_isolated: false,
            staging_version: None,
            staging_manifest: None,
            staging_diff_reasons: Vec::new(),
            manifest: clean,
            quarantined: false,
            quarantine_reason: None,
            updated_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Rehydrates a persisted SQLite/serialized plugin record while enforcing every domain invariant.
    pub fn from_persisted(snap: ManagedPluginSnapshot) -> Result<Self, PluginLifecycleError> {
        let clean = normalize_manifest(snap.manifest);
        validate_manifest_contract(&clean)?;
        let tid = snap.tenant_id.trim().to_string();
        let pid = snap.project_id.trim().to_string();
        let cid = snap.id.trim().to_string();
        let slug_raw = snap.plugin_slug.trim();
        let slug = if slug_raw.is_empty() {
            derive_plugin_slug(&clean.name)
        } else {
            slug_raw.to_string()
        };
        if cid.is_empty() || tid.is_empty() || pid.is_empty() || snap.revision < 1 {
            return Err(PluginLifecycleError::InvalidPersistedState(
                "Empty identity/scope or revision < 1".to_string(),
            ));
        }
        if snap.version.trim() != clean.version || snap.name.trim() != clean.name {
            return Err(PluginLifecycleError::InvalidPersistedState(
                "Persisted version/name diverges from manifest".to_string(),
            ));
        }
        if snap.quarantined
            && (snap.state == PluginState::Active
                || snap
                    .quarantine_reason
                    .as_deref()
                    .is_none_or(|r| r.trim().is_empty())
                || snap.breaking_change_isolated
                || snap.staging_manifest.is_some())
        {
            return Err(PluginLifecycleError::InvalidPersistedState(
                "Quarantined plugin violates non-ACTIVE, reason, or clean-staging invariant"
                    .to_string(),
            ));
        }
        let clean_staging = match snap.staging_manifest {
            Some(sm) => {
                let norm_sm = normalize_manifest(sm);
                validate_manifest_contract(&norm_sm)?;
                if !snap.breaking_change_isolated
                    || snap.state != PluginState::Active
                    || snap.staging_version.as_deref() != Some(norm_sm.version.as_str())
                {
                    return Err(PluginLifecycleError::InvalidPersistedState(
                        "Staged manifest invariant mismatch (requires Active + breaking_change_isolated + matching staging_version)".to_string(),
                    ));
                }
                Some(norm_sm)
            }
            None => {
                if snap.breaking_change_isolated || snap.staging_version.is_some() {
                    return Err(PluginLifecycleError::InvalidPersistedState(
                        "Isolated staging flags set without staging_manifest".to_string(),
                    ));
                }
                None
            }
        };
        Ok(Self {
            id: cid,
            tenant_id: tid,
            project_id: pid,
            plugin_slug: slug,
            name: clean.name.clone(),
            version: clean.version.clone(),
            state: snap.state,
            revision: snap.revision,
            breaking_change_isolated: snap.breaking_change_isolated,
            staging_version: snap.staging_version,
            staging_manifest: clean_staging,
            staging_diff_reasons: snap.staging_diff_reasons,
            manifest: clean,
            quarantined: snap.quarantined,
            quarantine_reason: snap.quarantine_reason.map(|r| r.trim().to_string()),
            updated_at: snap.updated_at,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    pub fn plugin_slug(&self) -> &str {
        &self.plugin_slug
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn state(&self) -> PluginState {
        self.state
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn breaking_change_isolated(&self) -> bool {
        self.breaking_change_isolated
    }
    pub fn staging_version(&self) -> Option<&str> {
        self.staging_version.as_deref()
    }
    pub fn staging_manifest(&self) -> Option<&SkillManifest> {
        self.staging_manifest.as_ref()
    }
    pub fn staging_diff_reasons(&self) -> &[String] {
        &self.staging_diff_reasons
    }
    pub fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }
    pub fn quarantined(&self) -> bool {
        self.quarantined
    }
    pub fn quarantine_reason(&self) -> Option<&str> {
        self.quarantine_reason.as_deref()
    }
    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    /// Projects active lifecycle manifest onto backward-compatible `domain::models::SkillPlugin` (AC-02).
    pub fn to_domain_plugin(&self) -> SkillPlugin {
        let mut dto = SkillPlugin::from_core(
            self.id.clone(),
            self.manifest.name.clone(),
            self.version.clone(),
            self.manifest.entry_point.clone(),
            self.manifest.capabilities_required.clone(),
        );
        dto.plugin_slug = Some(self.plugin_slug.clone());
        dto.lifecycle_state = Some(self.state.as_wire_str().to_string());
        dto.tenant_id = Some(self.tenant_id.clone());
        dto.project_id = Some(self.project_id.clone());
        dto.revision = i64::try_from(self.revision)
            .ok()
            .and_then(|v| domain::SafeJsInteger::new(v).ok());
        dto.breaking_change_isolated = Some(self.breaking_change_isolated);
        dto.quarantined = Some(self.quarantined);
        dto
    }

    fn check_revision(&self, expected: u64) -> Result<(), PluginLifecycleError> {
        if expected != self.revision {
            return Err(PluginLifecycleError::RevisionConflict {
                expected,
                actual: self.revision,
            });
        }
        Ok(())
    }

    fn clear_staging_slot(&mut self) {
        self.breaking_change_isolated = false;
        self.staging_version = None;
        self.staging_manifest = None;
        self.staging_diff_reasons.clear();
    }

    pub fn transition_to_checked(
        &mut self,
        next: PluginState,
        expected_revision: u64,
    ) -> Result<(), PluginLifecycleError> {
        self.check_revision(expected_revision)?;
        if self.quarantined && next != PluginState::Archived {
            return Err(PluginLifecycleError::QuarantinedReactivationForbidden {
                reason: self
                    .quarantine_reason
                    .clone()
                    .unwrap_or_else(|| "security quarantine".to_string()),
            });
        }
        let valid = matches!(
            (self.state, next),
            (PluginState::Discovered, PluginState::Validated)
                | (PluginState::Discovered, PluginState::Archived)
                | (PluginState::Validated, PluginState::Installed)
                | (PluginState::Validated, PluginState::Archived)
                | (PluginState::Installed, PluginState::Active)
                | (PluginState::Installed, PluginState::Archived)
                | (PluginState::Active, PluginState::Deprecated)
                | (PluginState::Active, PluginState::Archived)
                | (PluginState::Deprecated, PluginState::Archived)
                | (PluginState::Deprecated, PluginState::Active)
        );
        if !valid {
            return Err(PluginLifecycleError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        if self.state == PluginState::Active && next != PluginState::Active {
            self.clear_staging_slot();
        }
        self.state = next;
        self.revision += 1;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Quarantines a compromised plugin, drops any pending staging candidate, and locks reactivation.
    pub fn quarantine(
        &mut self,
        reason: &str,
        expected_revision: u64,
    ) -> Result<(), PluginLifecycleError> {
        self.check_revision(expected_revision)?;
        let trimmed = reason.trim();
        if trimmed.is_empty() {
            return Err(PluginLifecycleError::InvalidQuarantineReason);
        }
        self.quarantined = true;
        self.quarantine_reason = Some(trimmed.to_string());
        self.clear_staging_slot();
        if self.state == PluginState::Active {
            self.state = PluginState::Deprecated;
        }
        self.revision += 1;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Transactional structural + SemVer update handler isolating breaking changes when ACTIVE.
    pub fn apply_manifest_update(
        &mut self,
        candidate_raw: SkillManifest,
        expected_revision: u64,
        force_breaking_hint: bool,
    ) -> Result<ManifestUpdateOutcome, PluginLifecycleError> {
        self.check_revision(expected_revision)?;
        if self.quarantined {
            return Err(PluginLifecycleError::QuarantinedReactivationForbidden {
                reason: self
                    .quarantine_reason
                    .clone()
                    .unwrap_or_else(|| "quarantined".to_string()),
            });
        }
        if self.state == PluginState::Archived {
            return Err(PluginLifecycleError::InvalidTransition {
                from: PluginState::Archived,
                to: PluginState::Archived,
            });
        }
        let candidate = normalize_manifest(candidate_raw);
        validate_manifest_contract(&candidate)?;

        if self.breaking_change_isolated {
            if self.staging_manifest.as_ref() == Some(&candidate) {
                return Ok(ManifestUpdateOutcome::IdempotentStaged {
                    active_version: self.version.clone(),
                    candidate_version: candidate.version,
                    reasons: self.staging_diff_reasons.clone(),
                });
            }
            return Err(PluginLifecycleError::PendingStagingMustBeResolved);
        }

        let report = detect_breaking_change(&self.manifest, &candidate, force_breaking_hint)?;
        if report.is_breaking {
            if self.state != PluginState::Active {
                return Err(PluginLifecycleError::NonActiveBreakingUpdateRejected {
                    state: self.state,
                    reasons: report.reasons,
                });
            }
            self.breaking_change_isolated = true;
            self.staging_version = Some(candidate.version.clone());
            self.staging_manifest = Some(candidate.clone());
            self.staging_diff_reasons = report.reasons.clone();
            self.revision += 1;
            self.updated_at = chrono::Utc::now().timestamp();
            return Ok(ManifestUpdateOutcome::StagedForReview {
                active_version: self.version.clone(),
                candidate_version: candidate.version,
                reasons: report.reasons.clone(),
                report,
            });
        }

        self.name = candidate.name.clone();
        self.version = candidate.version.clone();
        self.manifest = candidate;
        self.revision += 1;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(ManifestUpdateOutcome::Applied { report })
    }

    /// Promotes staged breaking candidate ONLY after frontmatter, risk_tier, capabilities, IPI, and script security checks pass.
    pub fn promote_staging(
        &mut self,
        bundle: &StagedPluginBundle,
        expected_revision: u64,
    ) -> Result<(), PluginLifecycleError> {
        self.check_revision(expected_revision)?;
        if self.quarantined {
            return Err(PluginLifecycleError::QuarantinedReactivationForbidden {
                reason: self
                    .quarantine_reason
                    .clone()
                    .unwrap_or_else(|| "quarantined".to_string()),
            });
        }
        if !self.breaking_change_isolated || self.state != PluginState::Active {
            return Err(PluginLifecycleError::NoStagedCandidate);
        }
        let Some(expected_manifest) = self.staging_manifest.as_ref() else {
            return Err(PluginLifecycleError::NoStagedCandidate);
        };
        let norm_bundle = StagedPluginBundle {
            manifest: normalize_manifest(bundle.manifest.clone()),
            skill_md: bundle.skill_md.clone(),
            scripts: bundle.scripts.clone(),
            references: bundle.references.clone(),
            assets: bundle.assets.clone(),
        };
        if expected_manifest != &norm_bundle.manifest {
            return Err(PluginLifecycleError::StagingPromotionRejected(
                "Staged bundle manifest does not match isolated candidate".to_string(),
            ));
        }
        validate_staged_bundle_contents(&norm_bundle)?;

        self.name = norm_bundle.manifest.name.clone();
        self.version = norm_bundle.manifest.version.clone();
        self.manifest = norm_bundle.manifest;
        self.clear_staging_slot();
        self.revision += 1;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }

    /// Verifies an on-disk repackaged staging directory (`O_NOFOLLOW`, `nlink == 1`, `skills-lock.json`) and promotes it atomically.
    pub fn promote_verified_staging_dir(
        &mut self,
        dir: &Path,
        expected_package_sha256: Option<&str>,
        expected_revision: u64,
    ) -> Result<(), PluginLifecycleError> {
        let bundle = load_verified_staged_bundle_from_dir(dir, expected_package_sha256)
            .map_err(PluginLifecycleError::StagingPromotionRejected)?;
        self.promote_staging(&bundle, expected_revision)
    }

    pub fn discard_staging(&mut self, expected_revision: u64) -> Result<(), PluginLifecycleError> {
        self.check_revision(expected_revision)?;
        if !self.breaking_change_isolated {
            return Err(PluginLifecycleError::NoStagedCandidate);
        }
        self.clear_staging_slot();
        self.revision += 1;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_lifecycle_transitions_with_cas() {
        let mut p = ManagedPlugin::new("plug-1", "sdi-clarifier", "1.0.0").unwrap();
        assert_eq!(p.state(), PluginState::Discovered);

        p.transition_to_checked(PluginState::Validated, 1).unwrap();
        p.transition_to_checked(PluginState::Installed, 2).unwrap();
        p.transition_to_checked(PluginState::Active, 3).unwrap();
        p.transition_to_checked(PluginState::Deprecated, 4).unwrap();
        p.transition_to_checked(PluginState::Archived, 5).unwrap();
        assert_eq!(p.state(), PluginState::Archived);
        assert_eq!(p.to_domain_plugin().version, "1.0.0");
    }

    #[test]
    fn test_ac03_breaking_change_isolation_returns_outcome() {
        let mut p = ManagedPlugin::new("plug-3", "spec-syn", "1.0.0").unwrap();
        p.transition_to_checked(PluginState::Validated, 1).unwrap();
        p.transition_to_checked(PluginState::Installed, 2).unwrap();
        p.transition_to_checked(PluginState::Active, 3).unwrap();

        let mut cand = p.manifest().clone();
        cand.version = "2.0.0".to_string();
        let out = p.apply_manifest_update(cand, p.revision(), false).unwrap();
        assert!(matches!(out, ManifestUpdateOutcome::StagedForReview { .. }));
        assert_eq!(p.version(), "1.0.0");
        assert!(p.breaking_change_isolated());
        assert_eq!(p.staging_version(), Some("2.0.0"));
    }
}
