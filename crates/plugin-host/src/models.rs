use serde::{Deserialize, Serialize};

/// Skill manifest conforming to agentskills.io standard
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub entry_point: String,
    #[serde(default)]
    pub capabilities_required: Vec<String>,
    #[serde(default)]
    pub risk_tier: u8,
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub prompt_overview: Option<String>,
}

/// Execution receipt for a staged run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagingRunReceipt {
    pub run_id: String,
    pub staged_skills: Vec<String>,
    pub stable_prompt_hash: String,
    pub staged_dir: String,
    pub timestamp: i64,
}

/// Severity of a lint issue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

/// Category of a lint issue in the Triangle Linter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintIssueCategory {
    AntiAiSlop,
    GeometricLayout,
    WcagContrast,
    SyntaxIntegrity,
}

/// Individual issue detected by the Triangle Linter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintIssue {
    pub rule_id: String,
    pub category: LintIssueCategory,
    pub severity: LintSeverity,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub snippet: Option<String>,
    pub auto_fixable: bool,
}

/// Comprehensive report emitted by the Triangle Linter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriangleLintReport {
    pub total_issues: usize,
    pub new_in_this_call: usize,
    pub fixed_issues: usize,
    pub passed: bool,
    pub duration_ms: u64,
    pub issues: Vec<LintIssue>,
}
