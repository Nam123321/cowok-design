use serde::{Deserialize, Serialize};

fn default_entry_point() -> String {
    "SKILL.md".to_string()
}

const fn default_risk_tier() -> u8 {
    1
}

/// Skill manifest conforming to agentskills.io standard
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default = "default_entry_point")]
    pub entry_point: String,
    #[serde(default)]
    pub capabilities_required: Vec<String>,
    #[serde(default = "default_risk_tier")]
    pub risk_tier: u8,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | boolean | null"))]
    pub parameters: Option<serde_json::Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub prompt_overview: Option<String>,
}

/// Execution receipt for a staged run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct StagingRunReceipt {
    pub run_id: String,
    pub staged_skills: Vec<String>,
    pub stable_prompt_hash: String,
    pub staged_dir: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub timestamp: i64,
}

/// Severity of a lint issue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

/// Category of a lint issue in the Triangle Linter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum LintIssueCategory {
    AntiAiSlop,
    GeometricLayout,
    WcagContrast,
    SyntaxIntegrity,
}

/// Individual issue detected by the Triangle Linter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct LintIssue {
    pub rule_id: String,
    pub category: LintIssueCategory,
    pub severity: LintSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number | null"))]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number | null"))]
    pub column: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub snippet: Option<String>,
    pub auto_fixable: bool,
}

/// Comprehensive report emitted by the Triangle Linter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct TriangleLintReport {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_issues: usize,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub new_in_this_call: usize,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub fixed_issues: usize,
    pub passed: bool,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub duration_ms: u64,
    pub issues: Vec<LintIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fixed_content: Option<String>,
}
