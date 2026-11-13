// @story 03.2
// @implements AC-01, AC-02, AC-03
//! Layer-0 Domain Contracts for Project-Scoped Iconology, Typography & Component Registry.

use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

/// Canonical icon entry inside an icon-manifest.json catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct IconManifestEntry {
    pub name: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Project-scoped or flagship icon manifest catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct IconManifest {
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
    pub count: usize,
    #[serde(default)]
    pub icons: Vec<IconManifestEntry>,
}

/// Result of validating an icon identifier against the active project icon manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct IconValidationReport {
    pub requested_icon: String,
    pub is_valid: bool,
    pub suggestions: Vec<String>,
}

/// Project typography & WCAG contrast contract persisted in `constraint_profiles.typography_contract_json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct TypographyContract {
    pub allowed_font_families: Vec<String>,
    pub allowed_font_weights: Vec<u32>,
    pub allowed_font_sizes: Vec<String>,
    pub min_wcag_contrast: f64,
}

impl Default for TypographyContract {
    fn default() -> Self {
        Self {
            allowed_font_families: vec![
                "Geist".to_string(),
                "Inter".to_string(),
                "system-ui".to_string(),
                "monospace".to_string(),
            ],
            allowed_font_weights: vec![400, 500, 600, 700],
            allowed_font_sizes: vec![
                "12px".to_string(),
                "14px".to_string(),
                "16px".to_string(),
                "18px".to_string(),
                "20px".to_string(),
                "24px".to_string(),
                "32px".to_string(),
            ],
            min_wcag_contrast: 4.5,
        }
    }
}

/// Result of validating typography properties against `TypographyContract`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct TypographyValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

/// Component entry inside a project's `07_COMPONENT_REGISTRY` contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ComponentRegistryEntry {
    pub name: String,
    #[serde(default)]
    pub source_spec: Option<String>,
    #[serde(default)]
    pub allowed_props: Vec<String>,
}

/// Project-scoped UI Component Registry contract (`07_COMPONENT_REGISTRY`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ComponentRegistryContract {
    #[serde(default)]
    pub components: Vec<ComponentRegistryEntry>,
    #[serde(default)]
    pub forbid_unregistered: bool,
}

/// Result of validating a JSX component identifier against `07_COMPONENT_REGISTRY`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ComponentValidationReport {
    pub requested_component: String,
    pub is_valid: bool,
    pub suggestions: Vec<String>,
}

/// Unified project design registry contracts persisted in SQLite `constraint_profiles`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ProjectDesignContracts {
    pub icon_manifest_path: String,
    #[serde(default)]
    pub allowed_icon_set: Vec<String>,
    #[serde(default)]
    pub typography_contract: TypographyContract,
    #[serde(default)]
    pub component_registry: ComponentRegistryContract,
}
