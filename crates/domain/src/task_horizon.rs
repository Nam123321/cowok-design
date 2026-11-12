// @story 02.1
// @implements AC-01, AC-02, AC-03
//! # Task Horizon Router & Cross-Department Context Slicer
//!
//! Provides deterministic task horizon classification (Short, Mid, Long),
//! department context slicing (ProductUi, MarketingDeck, ExecutiveReport),
//! and zero-trust visibility policies preventing SuperAdmin leakage to marketing/public targets.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[cfg(feature = "ts")]
use ts_rs::TS;

use crate::error::HarnessError;
use crate::spec_key::CanonicalSpecKey;

/// Task Horizon classification based on execution duration and scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum TaskHorizon {
    /// Short horizon (<= 30s): Micro-tweaks, token/copy updates, single-component inspection.
    Short,
    /// Mid horizon (<= 5m): Screen state transitions, multi-component layout assembly.
    Mid,
    /// Long horizon (>= 15m): Full portal synthesis, presentation deck generation, global refactor.
    Long,
}

impl TaskHorizon {
    /// Returns default token ceiling for this horizon.
    pub fn default_token_ceiling(&self) -> usize {
        match self {
            Self::Short => 600,
            Self::Mid => 1000,
            Self::Long => 1400,
        }
    }

    /// Returns SLA target in seconds.
    pub fn target_sla_seconds(&self) -> u32 {
        match self {
            Self::Short => 30,
            Self::Mid => 300,
            Self::Long => 900,
        }
    }
}

impl fmt::Display for TaskHorizon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short => write!(f, "short"),
            Self::Mid => write!(f, "mid"),
            Self::Long => write!(f, "long"),
        }
    }
}

impl FromStr for TaskHorizon {
    type Err = HarnessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().trim() {
            "short" | "quick" | "micro" => Ok(Self::Short),
            "mid" | "medium" | "standard" => Ok(Self::Mid),
            "long" | "deep" | "macro" | "full" => Ok(Self::Long),
            _ => Err(HarnessError::ValidationFailed {
                reason: format!(
                    "Invalid task horizon '{}'. Expected 'short', 'mid', or 'long'",
                    s
                ),
            }),
        }
    }
}

/// Target Department for context slicing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum Department {
    /// Product Application & UI Components
    ProductUi,
    /// Marketing Pitch Deck & Public Presentations (16:9)
    MarketingDeck,
    /// Executive Summary & Business Strategy Reports
    ExecutiveReport,
}

impl fmt::Display for Department {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProductUi => write!(f, "product_ui"),
            Self::MarketingDeck => write!(f, "marketing_deck"),
            Self::ExecutiveReport => write!(f, "executive_report"),
        }
    }
}

impl FromStr for Department {
    type Err = HarnessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let norm = s.to_ascii_lowercase().replace('-', "_");
        match norm.trim() {
            "product_ui" | "product" | "ui" => Ok(Self::ProductUi),
            "marketing_deck" | "marketing" | "deck" | "pitch" => Ok(Self::MarketingDeck),
            "executive_report" | "executive" | "report" | "management" => Ok(Self::ExecutiveReport),
            _ => Err(HarnessError::ValidationFailed {
                reason: format!(
                    "Invalid department '{}'. Expected 'product_ui', 'marketing_deck', or 'executive_report'",
                    s
                ),
            }),
        }
    }
}

/// Application Portal boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum Portal {
    SuperAdmin,
    TenantAdmin,
    SalesApp,
    MarketingWeb,
    DeveloperPortal,
    PresentationStudio,
}

impl Portal {
    /// Returns true if this portal is considered a public / non-admin boundary.
    pub fn is_public_facing(&self) -> bool {
        matches!(
            self,
            Self::MarketingWeb | Self::SalesApp | Self::PresentationStudio
        )
    }

    /// Returns true if this portal holds root platform privilege.
    pub fn is_super_admin(&self) -> bool {
        matches!(self, Self::SuperAdmin)
    }
}

/// Decision reason code for context slicing and filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    Success,
    RedactedSecurityBoundary,
    PortalMismatch,
    DepartmentExcluded,
    TokenBudgetExceeded,
    NoEligibleSpecs,
}

/// Request for JIT Context Slicing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ContextSliceRequest {
    pub task_input: String,
    pub override_horizon: Option<TaskHorizon>,
    pub department: Department,
    pub portal: Portal,
    pub affected_artifacts: Vec<String>,
}

/// Decision computed by the Context Slicer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ContextSliceDecision {
    pub horizon: TaskHorizon,
    pub department: Department,
    pub portal: Portal,
    pub allowed_spec_keys: Vec<CanonicalSpecKey>,
    pub denied_spec_keys: Vec<(CanonicalSpecKey, ReasonCode)>,
    pub token_ceiling: usize,
    pub primary_reason: ReasonCode,
}

/// Single spec slice admitted into the prompt context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct SpecSlice {
    pub spec_key: CanonicalSpecKey,
    pub title: String,
    pub content: String,
    pub estimated_tokens: usize,
    pub priority_rank: usize,
}

/// Authorized and audited context slice ready for Agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct AuthorizedContextSlice {
    pub project_id: Uuid,
    pub decision: ContextSliceDecision,
    pub slices: Vec<SpecSlice>,
    pub total_estimated_tokens: usize,
    pub atomic_complete: bool,
}

/// Deterministic 2-stage Task Horizon Router.
pub struct TaskHorizonRouter;

impl TaskHorizonRouter {
    /// Classifies task horizon deterministically.
    pub fn classify(
        task_input: &str,
        override_horizon: Option<TaskHorizon>,
        affected_artifacts_count: usize,
    ) -> TaskHorizon {
        if let Some(h) = override_horizon {
            return h;
        }

        let input_lower = task_input.to_ascii_lowercase();

        // Stage 1: Check for explicit Long horizon indicators
        let long_keywords = [
            "synthesize",
            "scaffold",
            "portal",
            "full flow",
            "cross-portal",
            "presentation deck",
            "16:9",
            "refactor",
            "migration",
            "architecture",
        ];
        if affected_artifacts_count >= 5 || long_keywords.iter().any(|k| input_lower.contains(k)) {
            return TaskHorizon::Long;
        }

        // Stage 2: Check for Short horizon indicators
        let short_keywords = [
            "fix", "tweak", "color", "copy", "typo", "inspect", "lint", "badge", "button", "label",
            "icon", "minor",
        ];
        if affected_artifacts_count <= 1 && short_keywords.iter().any(|k| input_lower.contains(k)) {
            return TaskHorizon::Short;
        }

        // Default to Mid horizon
        TaskHorizon::Mid
    }
}

/// Zero-Trust Visibility Policy Guardian.
pub struct VisibilityPolicy;

impl VisibilityPolicy {
    /// Resolves allowed and denied spec keys based on Department, Portal, and AC-03 anti-leakage invariants.
    pub fn evaluate(
        department: Department,
        portal: Portal,
    ) -> (Vec<CanonicalSpecKey>, Vec<(CanonicalSpecKey, ReasonCode)>) {
        let mut allowed = Vec::new();
        let mut denied = Vec::new();

        for key in CanonicalSpecKey::ALL {
            let (is_allowed, reason) = Self::check_spec_visibility(key, department, portal);
            if is_allowed {
                allowed.push(key);
            } else {
                denied.push((key, reason));
            }
        }

        (allowed, denied)
    }

    /// Evaluates visibility for a single spec key.
    /// Strictly enforces AC-03: Zero SuperAdmin context leakage to MarketingWeb / SalesApp / MarketingDeck.
    pub fn check_spec_visibility(
        key: CanonicalSpecKey,
        department: Department,
        portal: Portal,
    ) -> (bool, ReasonCode) {
        // AC-03 Boundary Guard: If request is from public/marketing portal, forbid admin-sensitive specs unconditionally
        if portal.is_public_facing()
            && matches!(
                key,
                CanonicalSpecKey::DatabaseSpec | CanonicalSpecKey::SecurityTenancy
            )
        {
            return (false, ReasonCode::RedactedSecurityBoundary);
        }

        // Department-driven inclusion matrix
        match department {
            Department::ProductUi => match key {
                CanonicalSpecKey::ProductOverview
                | CanonicalSpecKey::UiUxSpec
                | CanonicalSpecKey::Architecture
                | CanonicalSpecKey::DataAndCopy => (true, ReasonCode::Success),
                _ => (false, ReasonCode::DepartmentExcluded),
            },
            Department::MarketingDeck => match key {
                CanonicalSpecKey::ProductOverview
                | CanonicalSpecKey::UiUxSpec
                | CanonicalSpecKey::DataAndCopy => (true, ReasonCode::Success),
                _ => (false, ReasonCode::DepartmentExcluded),
            },
            Department::ExecutiveReport => match key {
                CanonicalSpecKey::ProductOverview
                | CanonicalSpecKey::DatabaseSpec
                | CanonicalSpecKey::Architecture
                | CanonicalSpecKey::DataAndCopy => (true, ReasonCode::Success),
                _ => (false, ReasonCode::DepartmentExcluded),
            },
        }
    }

    /// Terminal Redaction Filter: Asserts that a piece of Markdown content does not leak SuperAdmin directives.
    pub fn apply_terminal_redaction(content: &str, portal: Portal) -> Result<String, HarnessError> {
        if portal.is_public_facing() {
            let lower = content.to_ascii_lowercase();
            let forbidden_patterns = [
                "superadmin",
                "super_admin",
                "super admin",
                "super-admin",
                "master_key",
                "internal_admin_token",
                "infra_secret",
                "cluster_private",
                "p6_security_internal",
            ];
            for pat in &forbidden_patterns {
                if lower.contains(pat) {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "AC-03 Boundary Violation: Forbidden pattern '{}' detected in public-facing context slice!",
                            pat
                        ),
                    });
                }
            }

            // F-06 & F-10: Defensive anti-obfuscation filter against zero-width spaces, invisible unicode, and homoglyphs/confusables
            let is_invisible_or_separator = |c: char| -> bool {
                matches!(
                    c,
                    '\u{200B}'..='\u{200F}'
                    | '\u{2060}'..='\u{206F}'
                    | '\u{FEFF}'
                    | '\u{00AD}'
                    | '\u{034F}'
                    | '\u{180E}'
                ) || (c.is_control() && c != '\n' && c != '\r' && c != '\t')
            };

            let stripped_lower: String = lower
                .chars()
                .filter(|c| !is_invisible_or_separator(*c))
                .collect();
            for pat in &forbidden_patterns {
                if stripped_lower.contains(pat) {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "AC-03 Boundary Violation: Forbidden pattern '{}' detected via stripped anti-obfuscation filter!",
                            pat
                        ),
                    });
                }
            }

            // F-10: Homoglyph and Unicode confusable normalization to ASCII
            let dehomoglyphed: String =
                stripped_lower.chars().map(map_homoglyph_to_ascii).collect();
            for pat in &forbidden_patterns {
                if dehomoglyphed.contains(pat) {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "AC-03 Boundary Violation: Forbidden pattern '{}' detected via homoglyph confusable normalization!",
                            pat
                        ),
                    });
                }
            }

            // Also check collapsed alphanumeric characters to defeat arbitrary punctuation/delimiters
            let collapsed: String = dehomoglyphed
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            let collapsed_patterns = [
                "superadmin",
                "masterkey",
                "internaladmintoken",
                "infrasecret",
                "clusterprivate",
                "p6securityinternal",
            ];
            for pat in &collapsed_patterns {
                if collapsed.contains(pat) {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "AC-03 Boundary Violation: Forbidden pattern '{}' detected via collapsed anti-obfuscation filter!",
                            pat
                        ),
                    });
                }
            }
        }
        Ok(content.to_string())
    }
}

/// Normalizes common Unicode homoglyphs and confusables (Cyrillic, Greek, Fullwidth) to ASCII.
fn map_homoglyph_to_ascii(c: char) -> char {
    match c {
        // 'a'
        'а' | 'ä' | 'à' | 'á' | 'â' | 'ã' | 'å' | 'α' | 'ａ' => 'a',
        // 'b'
        'Ь' | 'ь' | 'β' | 'ｂ' => 'b',
        // 'c'
        'с' | 'ç' | 'ć' | 'č' | 'ｃ' => 'c',
        // 'd'
        'ԁ' | 'đ' | 'ď' | 'ｄ' => 'd',
        // 'e'
        'е' | 'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'ε' | 'є' | 'ｅ' => {
            'e'
        }
        // 'f'
        'ｆ' => 'f',
        // 'g'
        'ɡ' | 'ġ' | 'ğ' | 'ｇ' => 'g',
        // 'h'
        'һ' | 'ｈ' => 'h',
        // 'i'
        'і' | 'í' | 'ì' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' | 'ı' | 'ι' | 'ｉ' => 'i',
        // 'j'
        'ј' | 'ĵ' | 'ｊ' => 'j',
        // 'k'
        'к' | 'κ' | 'ķ' | 'ｋ' => 'k',
        // 'l'
        'ӏ' | 'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' | 'ｌ' => 'l',
        // 'm'
        'м' | 'ｍ' => 'm',
        // 'n'
        'п' | 'ո' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' | 'ｎ' => 'n',
        // 'o'
        'о' | 'ö' | 'ò' | 'ó' | 'ô' | 'õ' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'ο' | 'ｏ' => 'o',
        // 'p'
        'р' | 'ρ' | 'ｐ' => 'p',
        // 'q'
        'ｑ' => 'q',
        // 'r'
        'г' | 'ѓ' | 'ř' | 'ŕ' | 'ŗ' | 'ｒ' => 'r',
        // 's'
        'ѕ' | 'ś' | 'ŝ' | 'ş' | 'š' | 'ｓ' => 's',
        // 't'
        'т' | 'ţ' | 'ť' | 'ŧ' | 'τ' | 'ｔ' => 't',
        // 'u'
        'υ' | 'μ' | 'ü' | 'ù' | 'ú' | 'û' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ｕ' => 'u',
        // 'v'
        'ν' | 'ѵ' | 'ｖ' => 'v',
        // 'w'
        'ѡ' | 'ԝ' | 'ｗ' => 'w',
        // 'x'
        'х' | 'χ' | 'ｘ' => 'x',
        // 'y'
        'у' | 'ý' | 'ÿ' | 'ŷ' | 'γ' | 'ｙ' => 'y',
        // 'z'
        'ź' | 'ż' | 'ž' | 'ｚ' => 'z',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_01_router_classifies_short_task() {
        let h = TaskHorizonRouter::classify("fix button hover color", None, 1);
        assert_eq!(h, TaskHorizon::Short);
        assert_eq!(h.default_token_ceiling(), 600);
    }

    #[test]
    fn ut_02_router_classifies_long_task() {
        let h =
            TaskHorizonRouter::classify("synthesize marketing pitch deck presentation", None, 5);
        assert_eq!(h, TaskHorizon::Long);
        assert_eq!(h.default_token_ceiling(), 1400);
    }

    #[test]
    fn ut_03_router_respects_explicit_override() {
        let h =
            TaskHorizonRouter::classify("synthesize whole portal", Some(TaskHorizon::Short), 10);
        assert_eq!(h, TaskHorizon::Short);
    }

    #[test]
    fn ut_04_router_defaults_to_mid_horizon() {
        let h = TaskHorizonRouter::classify("add user profile settings tab", None, 3);
        assert_eq!(h, TaskHorizon::Mid);
    }

    #[test]
    fn ut_05_visibility_policy_department_isolation() {
        let (allowed_ui, _) =
            VisibilityPolicy::evaluate(Department::ProductUi, Portal::TenantAdmin);
        assert!(allowed_ui.contains(&CanonicalSpecKey::UiUxSpec));
        assert!(allowed_ui.contains(&CanonicalSpecKey::Architecture));

        let (allowed_mkt, _) =
            VisibilityPolicy::evaluate(Department::MarketingDeck, Portal::MarketingWeb);
        assert!(allowed_mkt.contains(&CanonicalSpecKey::ProductOverview));
        assert!(!allowed_mkt.contains(&CanonicalSpecKey::Architecture));
    }

    #[test]
    fn ut_06_ac03_boundary_guard_redacts_superadmin_leakage() {
        let sensitive_content =
            "This is a public marketing deck but contains superadmin master_key configuration.";
        let res =
            VisibilityPolicy::apply_terminal_redaction(sensitive_content, Portal::MarketingWeb);
        assert!(res.is_err());
        if let Err(HarnessError::BoundaryViolation { reason }) = res {
            assert!(reason.contains("AC-03 Boundary Violation"));
        } else {
            panic!("Expected BoundaryViolation error");
        }
    }

    #[test]
    fn ut_07_ac03_boundary_guard_passes_safe_content() {
        let safe_content =
            "Welcome to cowok-design: The AI-native design workbench for modern teams.";
        let res = VisibilityPolicy::apply_terminal_redaction(safe_content, Portal::MarketingWeb);
        assert!(res.is_ok());
    }

    #[test]
    fn ut_08_superadmin_portal_has_full_spec_access() {
        let (allowed, denied) =
            VisibilityPolicy::evaluate(Department::ProductUi, Portal::SuperAdmin);
        assert!(!allowed.is_empty());
        for (_, reason) in denied {
            assert_ne!(reason, ReasonCode::RedactedSecurityBoundary);
        }
    }

    #[test]
    fn ut_09_ac03_anti_obfuscation_catches_zero_width_and_delimiters() {
        // Zero-width space evasion attack from Codex F-06
        let obfuscated = "Product overview with Super\u{200b}Admin master\u{200b}_key: secret";
        let res = VisibilityPolicy::apply_terminal_redaction(obfuscated, Portal::MarketingWeb);
        assert!(res.is_err());

        // Invisible control character / BOM evasion
        let bom_obfuscated = "Internal directive: super\u{FEFF}admin key";
        let res2 = VisibilityPolicy::apply_terminal_redaction(bom_obfuscated, Portal::MarketingWeb);
        assert!(res2.is_err());
    }

    #[test]
    fn ut_10_ac03_homoglyph_confusable_evasion_caught() {
        // Greek and Cyrillic homoglyph evasion attack from Codex F-10:
        // 'υ' (U+03C5 Greek upsilon), 'а' (U+0430 Cyrillic a)
        let homoglyph_content = "SυperAdmin mаster_key: secret";
        let res =
            VisibilityPolicy::apply_terminal_redaction(homoglyph_content, Portal::MarketingWeb);
        assert!(res.is_err());
        if let Err(HarnessError::BoundaryViolation { reason }) = res {
            assert!(reason.contains("AC-03 Boundary Violation"));
        } else {
            panic!("Expected BoundaryViolation error for homoglyph attack");
        }
    }
}
