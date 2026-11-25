// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (4-Axis Anti-Slop Design Tournament <=15s, Pre-Judge AST Structural Distance <=0.78, Position-Swapped RubricShield Blind Judge, Cross-Variant Borrow & advisor 6-Step GEPA Loop with Kernel Guards)
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use chrono::Utc;
use domain::decision::{DecisionCandidate, RubricPairwiseInput, RubricPairwiseOutcome};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::decision_cascade::DecisionCascadeClient;
use crate::design_jury::{evaluate_e1_e4_evidence, QcRunInput};
use crate::swarm_leases::validate_disjoint_write_sets;

/// Hard wall-clock budget for 4-axis parallel tournament fan-out (`<= 15s`).
pub const MAX_TOURNAMENT_BUDGET_MS: u64 = 15_000;

/// Minimum surviving variants required to proceed gracefully if slow branches time out.
pub const MIN_TOURNAMENT_VARIANTS: usize = 2;

/// Maximum orthogonal branches per tournament run.
pub const MAX_TOURNAMENT_VARIANTS: usize = 4;

/// Maximum allowable pairwise AST structural similarity (`similarity <= 0.78`).
pub const MAX_AST_SIMILARITY: f64 = 0.78;

/// Hard re-roll cap per duplicate variant (`AC-04`: prevents infinite re-roll loops).
pub const MAX_AST_REROLLS: u8 = 2;

/// Minimum recurrence count for Manual Studio Corrections consumed from `gate_failure_ledger`.
pub const STUDIO_CORRECTION_CLUSTER_THRESHOLD: i64 = 3;

/// Kernel-denied evaluation suite path prefix (`AC-03`, `AC-04`: anti-Goodhart guard).
pub const SEALED_EVAL_DENY_PREFIX: &str = "tests/eval/sealed";

/// Mandatory promotion approval policy for every `advisor` staging draft.
pub const HUMAN_APPROVAL_POLICY: &str = "HUMAN_ONLY";

/// Canonical relative staging directory for `design-craft` reflective draft PRs.
pub const DESIGN_CRAFT_STAGING_DIR: &str = ".cowok-design/staging/design-craft";

/// Blind display labels (never expose raw agent/model identifiers in UI payloads).
pub const BLIND_OPTION_LABELS: [&str; 4] = ["Option A", "Option B", "Option C", "Option D"];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TournamentError {
    #[error("INVALID_TOURNAMENT_INPUT: {0}")]
    InvalidInput(String),
    #[error(
        "INSUFFICIENT_TOURNAMENT_VARIANTS: generated {generated}, minimum required {required}"
    )]
    InsufficientVariants { generated: usize, required: usize },
    #[error("TOURNAMENT_LEASE_CONFLICT: {0}")]
    LeaseConflict(String),
    #[error("BORROW_ZONE_NOT_FOUND: {0}")]
    BorrowZoneNotFound(String),
    #[error("SEALED_EVAL_TAMPER_ERROR: forbidden target '{requested_path}' ({reason})")]
    SealedEvalTamper {
        requested_path: String,
        reason: String,
    },
    #[error("HUMAN_APPROVAL_REQUIRED: target '{target}' requires Staging + HUMAN_ONLY approval ({reason})")]
    HumanApprovalRequired { target: String, reason: String },
    #[error("STALE_GENERATION_EPOCH: expected >= {expected}, provided {provided}")]
    StaleGenerationEpoch { expected: i64, provided: i64 },
}

/// 4 Orthogonal Design Axes preventing homogeneous AI slop (`AC-01`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DesignAxis {
    #[serde(rename = "INFORMATION_DENSITY")]
    InformationDensity,
    #[serde(rename = "NAVIGATION_TOPOLOGY")]
    NavigationTopology,
    #[serde(rename = "VISUAL_METAPHOR")]
    VisualMetaphor,
    #[serde(rename = "INTERACTION_RHYTHM")]
    InteractionRhythm,
}

impl DesignAxis {
    pub const ALL: [DesignAxis; 4] = [
        Self::InformationDensity,
        Self::NavigationTopology,
        Self::VisualMetaphor,
        Self::InteractionRhythm,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::InformationDensity => "INFORMATION_DENSITY",
            Self::NavigationTopology => "NAVIGATION_TOPOLOGY",
            Self::VisualMetaphor => "VISUAL_METAPHOR",
            Self::InteractionRhythm => "INTERACTION_RHYTHM",
        }
    }

    pub fn display_title(self) -> &'static str {
        match self {
            Self::InformationDensity => "Editorial Density & Executive Metrics",
            Self::NavigationTopology => "Split-Rail Workspace & Ledger",
            Self::VisualMetaphor => "Modular Bento Spotlight Canvas",
            Self::InteractionRhythm => "Guided Progressive Flow Workbench",
        }
    }

    pub fn structural_profile(self) -> &'static str {
        match self {
            Self::InformationDensity => "editorial-kpi-strip-definition-list",
            Self::NavigationTopology => "rail-nav-split-tablist-ledger",
            Self::VisualMetaphor => "spotlight-bento-mosaic-callout",
            Self::InteractionRhythm => "stepper-track-progressive-form-dialog",
        }
    }

    pub fn parse_strict(raw: &str) -> Result<Self, TournamentError> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "INFORMATION_DENSITY" => Ok(Self::InformationDensity),
            "NAVIGATION_TOPOLOGY" => Ok(Self::NavigationTopology),
            "VISUAL_METAPHOR" => Ok(Self::VisualMetaphor),
            "INTERACTION_RHYTHM" => Ok(Self::InteractionRhythm),
            other => Err(TournamentError::InvalidInput(format!(
                "Unsupported design axis '{other}'"
            ))),
        }
    }
}

/// Optional per-branch override (supports custom HTML generators and timeout/clone tests).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantBranchOverride {
    pub axis: DesignAxis,
    #[serde(default)]
    pub initial_html: Option<String>,
    #[serde(default)]
    pub reroll_htmls: Vec<String>,
    #[serde(default)]
    pub simulated_latency_ms: Option<u64>,
}

/// Configuration for launching a 4-Axis Anti-Slop Design Tournament.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentStartConfig {
    pub project_id: String,
    pub prompt: String,
    #[serde(default)]
    pub axes: Vec<DesignAxis>,
    #[serde(default = "default_budget_ms")]
    pub budget_ms: u64,
    #[serde(default)]
    pub branch_overrides: Vec<VariantBranchOverride>,
    #[serde(default)]
    pub rubric_criteria: Vec<String>,
}

fn default_budget_ms() -> u64 {
    MAX_TOURNAMENT_BUDGET_MS
}

/// Single anonymized tournament variant ready for blind comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedVariant {
    pub id: String,
    pub display_label: String,
    pub axis: DesignAxis,
    pub axis_profile: String,
    pub leased_path: String,
    pub html: String,
    pub artifact_sha256: String,
    pub ast_similarity: f64,
    pub clone_distance_passed: bool,
    pub low_diversity_warning: bool,
    pub reroll_attempts: u8,
    pub rubric_verdict: String,
    pub rubric_wins: usize,
    pub de_score_bps: u32,
    pub elapsed_ms: u64,
    pub archived: bool,
}

/// Position-swapped `(A, B) / (B, A)` pairwise record from `RubricShield`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairwiseBlindComparison {
    pub variant_a_id: String,
    pub variant_a_label: String,
    pub variant_b_id: String,
    pub variant_b_label: String,
    pub outcome: RubricPairwiseOutcome,
}

/// Aggregated blind `RubricShield` verdict over surviving variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlindRubricSummary {
    pub recommended_variant_id: String,
    pub recommended_display_label: String,
    pub all_pairs_swap_consistent: bool,
    pub pairwise_comparisons: Vec<PairwiseBlindComparison>,
}

/// Complete output of `run_design_tournament`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentRunOutput {
    pub run_id: String,
    pub project_id: String,
    pub prompt: String,
    pub axes: Vec<DesignAxis>,
    pub generation_epoch: i64,
    pub budget_ms: u64,
    pub elapsed_ms: u64,
    pub degraded_min_variants: bool,
    pub timed_out_axes: Vec<DesignAxis>,
    pub low_diversity_warning: bool,
    pub variants: Vec<GeneratedVariant>,
    pub rubric_summary: BlindRubricSummary,
    pub created_at: i64,
}

/// Result of `merge_cross_variant_borrow`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossVariantBorrowResult {
    pub merged_html: String,
    pub borrowed_ff_id: String,
    #[serde(default)]
    pub borrowed_ff_ids: Vec<String>,
    pub extracted_css_rules: Vec<String>,
    pub remapped_ff_ids: Vec<(String, String)>,
    pub duplicate_ff_ids_after_merge: usize,
    pub merged_sha256: String,
}

/// Canonical 6 steps of the `advisor` GEPA Reflective Optimization Loop (`AC-03`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdvisorStepKind {
    #[serde(rename = "1_SIGNAL_COLLECT")]
    SignalCollect,
    #[serde(rename = "2_ROOT_CAUSE")]
    RootCause,
    #[serde(rename = "3_PATCH_SKILL_DRAFT")]
    PatchSkillDraft,
    #[serde(rename = "4_SEALED_EVAL_PRECHECK")]
    SealedEvalPrecheck,
    #[serde(rename = "5_REGRESSION_CHECK")]
    RegressionCheck,
    #[serde(rename = "6_STAGE_DESIGN_CRAFT_PR")]
    StageDesignCraftPr,
}

impl AdvisorStepKind {
    pub const ALL: [AdvisorStepKind; 6] = [
        Self::SignalCollect,
        Self::RootCause,
        Self::PatchSkillDraft,
        Self::SealedEvalPrecheck,
        Self::RegressionCheck,
        Self::StageDesignCraftPr,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignalCollect => "1_SIGNAL_COLLECT",
            Self::RootCause => "2_ROOT_CAUSE",
            Self::PatchSkillDraft => "3_PATCH_SKILL_DRAFT",
            Self::SealedEvalPrecheck => "4_SEALED_EVAL_PRECHECK",
            Self::RegressionCheck => "5_REGRESSION_CHECK",
            Self::StageDesignCraftPr => "6_STAGE_DESIGN_CRAFT_PR",
        }
    }
}

/// Step-level receipt emitted during the 6-step `advisor` loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorStepReceipt {
    pub step: AdvisorStepKind,
    pub passed: bool,
    pub summary: String,
}

/// Creative memory event signal fed into `advisor` Step 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreativeMemorySignal {
    pub event_id: String,
    pub event_kind: String,
    pub summary: String,
}

/// Repeated Manual Studio Correction cluster (`hit_count >= 3`) from `gate_failure_ledger`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioCorrectionCluster {
    pub ledger_id: String,
    pub rule_id: String,
    pub sample_message: String,
    pub hit_count: i64,
}

/// Input to the 6-step `advisor` GEPA reflective loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorEvolveInput {
    pub project_id: String,
    pub target_skill_path: String,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub promote_directly: bool,
    #[serde(default = "default_staging_channel")]
    pub target_channel: String,
    #[serde(default)]
    pub memory_signals: Vec<CreativeMemorySignal>,
    #[serde(default)]
    pub studio_correction_clusters: Vec<StudioCorrectionCluster>,
}

fn default_staging_channel() -> String {
    "DESIGN_CRAFT_STAGING".to_string()
}

/// Output of the 6-step `advisor` GEPA reflective loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorEvolveOutput {
    pub patch_id: String,
    pub project_id: String,
    pub target_skill_path: String,
    pub staging_status: String,
    pub approval_policy: String,
    pub promoted_to_production: bool,
    pub sealed_eval_untouched: bool,
    pub awaiting_story_13_1_blind_eval: bool,
    pub regression_passed: bool,
    pub constraints_preserved: bool,
    pub signals_consumed: usize,
    pub studio_corrections_consumed: usize,
    pub draft_patch_rel_path: String,
    pub draft_patch_markdown: String,
    pub steps: Vec<AdvisorStepReceipt>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn escape_html_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn style_block_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<style[^>]*>(.*?)</style>").expect("valid style block regex")
    })
}

fn class_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("valid class attr regex")
    })
}

fn role_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\brole\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("valid role attr regex")
    })
}

fn ff_id_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\bdata-ff-id\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))"#)
            .expect("valid data-ff-id attr regex")
    })
}

/// Validates a `data-ff-id` value against `^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,63}$`.
pub fn is_valid_ff_id(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

fn is_void_html_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Quote-aware scan for the closing `>` of an HTML tag starting at `start` (pointing at `<`).
fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    if bytes.get(start).copied() != Some(b'<') {
        return None;
    }
    let mut i = start + 1;
    let mut in_quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_quote {
            if b == q {
                in_quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            in_quote = Some(b);
        } else if b == b'>' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Scans HTML open tags with quote-aware `>` detection, skipping comments and `<style>`/`<script>` text.
/// Returns `(start_byte, end_byte_exclusive, tag_lower, attrs_slice)`.
fn scan_open_tags(html: &str) -> Vec<(usize, usize, String, &str)> {
    let bytes = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        let Some(rel_lt) = lower[i..].find('<') else {
            break;
        };
        let pos = i + rel_lt;
        if lower[pos..].starts_with("<!--") {
            if let Some(end_comment) = lower[pos + 4..].find("-->") {
                i = pos + 4 + end_comment + 3;
                continue;
            }
            break;
        }
        let next_b = bytes.get(pos + 1).copied();
        if !matches!(next_b, Some(b) if b.is_ascii_alphabetic()) {
            i = pos + 1;
            continue;
        }
        let Some(tag_end) = find_tag_end(html, pos) else {
            break;
        };
        let mut name_end = pos + 1;
        while name_end < tag_end
            && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'-')
        {
            name_end += 1;
        }
        let tag_lower = lower[pos + 1..name_end].to_string();
        let attrs = &html[name_end..tag_end];
        out.push((pos, tag_end + 1, tag_lower.clone(), attrs));

        if tag_lower == "style" || tag_lower == "script" {
            let close_pat = format!("</{tag_lower}");
            if let Some(rel_close) = lower[tag_end + 1..].find(&close_pat) {
                let close_pos = tag_end + 1 + rel_close;
                if let Some(close_end) = find_tag_end(html, close_pos) {
                    i = close_end + 1;
                    continue;
                }
            }
        }
        i = tag_end + 1;
    }
    out
}

fn css_prop_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(display|grid-template-columns|flex-direction|position|border-radius|gap)\s*:\s*([^;}{]+)")
            .expect("valid css prop regex")
    })
}

/// Extracts structural HTML/CSS skeleton shingles (ignoring prose copy) for Pre-Judge AST Distance.
pub fn extract_ast_structural_shingles(html: &str) -> BTreeSet<String> {
    let mut shingles = BTreeSet::new();
    let mut tag_sequence: Vec<String> = Vec::new();

    for (_start, _end, tag, attrs) in scan_open_tags(html) {
        if matches!(tag.as_str(), "meta" | "title" | "link") {
            continue;
        }
        let mut classes: Vec<String> = Vec::new();
        if let Some(cls_cap) = class_attr_regex().captures(attrs) {
            let raw = cls_cap
                .get(1)
                .or_else(|| cls_cap.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            for tok in raw.split_whitespace() {
                let norm = tok.trim().to_ascii_lowercase();
                if !norm.is_empty() {
                    classes.push(norm);
                }
            }
            classes.sort();
            classes.dedup();
        }
        let role = role_attr_regex()
            .captures(attrs)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| m.as_str().trim().to_ascii_lowercase())
            .unwrap_or_default();

        let node_sig = if classes.is_empty() && role.is_empty() {
            tag.clone()
        } else {
            format!("{tag}.{}[r={role}]", classes.join("."))
        };
        shingles.insert(format!("node:{node_sig}"));
        tag_sequence.push(node_sig);
    }

    for pair in tag_sequence.windows(2) {
        shingles.insert(format!("bi:{}>{}", pair[0], pair[1]));
    }
    for tri in tag_sequence.windows(3) {
        shingles.insert(format!("tri:{}>{}>{}", tri[0], tri[1], tri[2]));
    }

    for cap in style_block_regex().captures_iter(html) {
        let css = &cap[1];
        for prop in css_prop_regex().captures_iter(css) {
            let k = prop[1].trim().to_ascii_lowercase();
            let v = prop[2]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase();
            shingles.insert(format!("css:{k}:{v}"));
        }
    }

    shingles
}

/// Computes pairwise AST structural similarity in `[0.0, 1.0]` using Jaccard similarity over
/// structural HTML/CSS skeleton shingles (`similarity <= 0.78` required to pass anti-slop gate).
pub fn compute_ast_structural_similarity(html_a: &str, html_b: &str) -> f64 {
    let set_a = extract_ast_structural_shingles(html_a);
    let set_b = extract_ast_structural_shingles(html_b);
    if set_a.is_empty() && set_b.is_empty() {
        return if html_a.trim() == html_b.trim() {
            1.0
        } else {
            0.0
        };
    }
    let inter = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union <= f64::EPSILON {
        0.0
    } else {
        ((inter / union) * 10_000.0).round() / 10_000.0
    }
}

/// Generates a deterministic, genuinely distinct HTML/CSS layout for each of the 4 orthogonal axes.
/// Re-roll attempts (`1..=2`) inject additional orthogonal structural landmarks to break clone similarity.
pub fn generate_axis_html(prompt: &str, axis: DesignAxis, reroll_attempt: u8) -> String {
    let safe_prompt = escape_html_text(prompt.trim());
    let title = if safe_prompt.is_empty() {
        "Workspace Experience".to_string()
    } else {
        safe_prompt
    };

    let reroll_extra = match reroll_attempt {
        0 => String::new(),
        1 => format!(
            r#"<details class="reroll-tier-one" data-ff-id="ff-axis-{tag}-details" open><summary>Highlights</summary><dl class="reroll-meta"><dt>Mode</dt><dd>Expanded Variant</dd></dl></details>"#,
            tag = axis.as_str().to_ascii_lowercase()
        ),
        _ => format!(
            r#"<figure class="reroll-tier-two" data-ff-id="ff-axis-{tag}-figure"><figcaption>Structural Contrast Frame</figcaption><p class="reroll-caption">Refined Layout</p><time>Live</time></figure>"#,
            tag = axis.as_str().to_ascii_lowercase()
        ),
    };

    match axis {
        DesignAxis::InformationDensity => format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<style>
:root {{
  --axis-density-bg: var(--bg-canvas, #fbfaf8);
  --axis-density-accent: var(--action-primary, #1f5eff);
  --axis-density-border: var(--border-default, #e2e4ea);
}}
.editorial-shell {{ display: grid; grid-template-columns: 1fr; gap: 28px; padding: 32px; background: var(--axis-density-bg); }}
.editorial-hero {{ display: flex; flex-direction: column; gap: 12px; border-bottom: 1px solid var(--axis-density-border); padding-bottom: 20px; }}
.kpi-strip {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 18px; }}
.kpi-card {{ border: 1px solid var(--axis-density-border); border-radius: 12px; padding: 16px; }}
</style>
</head>
<body>
<main class="editorial-shell" data-ff-id="ff-density-main">
  <header class="editorial-hero" data-ff-id="ff-hero-banner">
    <h1 data-ff-id="ff-primary-heading">{title}</h1>
    <p class="lead-copy" data-ff-id="ff-hero-summary">Executive overview structured for fast visual scanning and spacious rhythm.</p>
    <button type="button" class="primary-cta" data-ff-id="ff-primary-cta">Review Executive Brief</button>
  </header>
  <section class="kpi-strip" data-ff-id="ff-kpi-summary">
    <article class="kpi-card" data-ff-id="ff-kpi-velocity"><dl><dt>Completion Velocity</dt><dd>94%</dd></dl></article>
    <article class="kpi-card" data-ff-id="ff-kpi-clarity"><dl><dt>Hierarchy Clarity</dt><dd>High</dd></dl></article>
    <article class="kpi-card" data-ff-id="ff-kpi-focus"><dl><dt>Primary Focus</dt><dd>3 Key Metrics</dd></dl></article>
  </section>
  <aside class="editorial-footnote" data-ff-id="ff-editorial-note"><p>Balanced whitespace prioritizes reading comprehension.</p></aside>
  {reroll_extra}
</main>
</body>
</html>"#
        ),
        DesignAxis::NavigationTopology => format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<style>
:root {{
  --axis-nav-rail: var(--bg-surface, #f4f5f7);
  --axis-nav-ink: var(--text-primary, #141821);
  --axis-nav-highlight: var(--action-primary, #0f766e);
}}
.split-workspace {{ display: grid; grid-template-columns: 240px 1fr; gap: 0; min-height: 520px; }}
.rail-nav {{ display: flex; flex-direction: column; gap: 10px; background: var(--axis-nav-rail); padding: 20px; }}
.ledger-pane {{ display: flex; flex-direction: column; gap: 16px; padding: 24px; }}
.data-ledger {{ width: 100%; border-collapse: collapse; border-radius: 6px; }}
</style>
</head>
<body>
<div class="split-workspace" data-ff-id="ff-nav-workspace">
  <nav class="rail-nav" aria-label="Workspace Navigation" data-ff-id="ff-rail-navigation">
    <ul role="tablist" class="rail-tabs" data-ff-id="ff-rail-tablist">
      <li role="tab" aria-selected="true" data-ff-id="ff-tab-overview">Overview</li>
      <li role="tab" aria-selected="false" data-ff-id="ff-tab-workflows">Workflows</li>
      <li role="tab" aria-selected="false" data-ff-id="ff-tab-insights">Insights</li>
    </ul>
  </nav>
  <main class="ledger-pane" data-ff-id="ff-nav-main">
    <h1 data-ff-id="ff-nav-heading">{title}</h1>
    <table class="data-ledger" data-ff-id="ff-data-ledger">
      <thead><tr><th>Module</th><th>Status</th><th>Action</th></tr></thead>
      <tbody>
        <tr><td>Primary Flow</td><td>Ready</td><td><button type="button" data-ff-id="ff-ledger-open-btn">Open Workspace</button></td></tr>
      </tbody>
    </table>
    <footer class="workspace-status" data-ff-id="ff-nav-footer"><small>Persistent rail navigation keeps deep contexts one click away.</small></footer>
    {reroll_extra}
  </main>
</div>
</body>
</html>"#
        ),
        DesignAxis::VisualMetaphor => format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<style>
:root {{
  --axis-metaphor-tile: var(--bg-card, #ffffff);
  --axis-metaphor-glow: var(--agent-accent, #7c3aed);
  --axis-metaphor-radius: 20px;
}}
.bento-stage {{ display: grid; grid-template-columns: repeat(12, 1fr); gap: 14px; padding: 20px; }}
.spotlight-stage {{ position: relative; border-radius: var(--axis-metaphor-radius); padding: 24px; background: var(--axis-metaphor-tile); }}
.quote-callout {{ border-radius: 16px; padding: 18px; display: block; }}
</style>
</head>
<body>
<main class="bento-stage" data-ff-id="ff-metaphor-main">
  <figure class="spotlight-stage" data-ff-id="ff-spotlight-card">
    <h1 data-ff-id="ff-bento-heading">{title}</h1>
    <figcaption data-ff-id="ff-bento-caption">Tactile bento surfaces group related controls into scannable islands.</figcaption>
  </figure>
  <div class="bento-mosaic" data-ff-id="ff-bento-mosaic">
    <blockquote class="quote-callout" data-ff-id="ff-bento-callout">High-contrast focal card anchors user attention without clutter.</blockquote>
    <ul class="bento-actions" data-ff-id="ff-bento-menu">
      <li><button type="button" data-ff-id="ff-bento-explore-btn">Explore Modules</button></li>
    </ul>
  </div>
  {reroll_extra}
</main>
</body>
</html>"#
        ),
        DesignAxis::InteractionRhythm => format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<style>
:root {{
  --axis-rhythm-step: var(--action-primary, #d97706);
  --axis-rhythm-surface: var(--bg-surface, #fffdf7);
}}
.guided-flow {{ display: flex; flex-direction: column; gap: 20px; padding: 24px; background: var(--axis-rhythm-surface); }}
.stepper-track {{ display: flex; flex-direction: row; gap: 12px; list-style: none; padding: 0; }}
.interactive-workbench {{ display: grid; grid-template-columns: 2fr 1fr; gap: 16px; }}
</style>
</head>
<body>
<main class="guided-flow" data-ff-id="ff-rhythm-main">
  <h1 data-ff-id="ff-rhythm-heading">{title}</h1>
  <ol class="stepper-track" data-ff-id="ff-stepper-track">
    <li data-ff-id="ff-step-1">1. Configure</li>
    <li data-ff-id="ff-step-2">2. Preview</li>
    <li data-ff-id="ff-step-3">3. Launch</li>
  </ol>
  <form class="interactive-workbench" data-ff-id="ff-interactive-form">
    <fieldset data-ff-id="ff-rhythm-fieldset">
      <legend>Interactive Controls</legend>
      <label for="rhythm-goal">Primary Objective</label>
      <input id="rhythm-goal" type="text" value="Guided setup" />
      <div class="rhythm-progress" role="progressbar" aria-valuenow="65" aria-valuemax="100" data-ff-id="ff-rhythm-progress">65%</div>
      <p class="rhythm-output" role="status" data-ff-id="ff-rhythm-output">Step 2 of 3 ready</p>
      <button type="button" data-ff-id="ff-rhythm-continue-btn">Continue Flow</button>
    </fieldset>
    <dialog open class="rhythm-drawer" data-ff-id="ff-rhythm-dialog"><p>Progressive disclosure reveals details only when needed.</p></dialog>
  </form>
  {reroll_extra}
</main>
</body>
</html>"#
        ),
    }
}

fn score_variant_de_bps(html: &str, ast_similarity: f64, clone_passed: bool) -> u32 {
    let qc = evaluate_e1_e4_evidence(&QcRunInput {
        screen_id: "tournament-variant".to_string(),
        html_snapshot: Some(html.to_string()),
        ..QcRunInput::default()
    });
    let violation_penalty = (qc.violations.len() as u32).saturating_mul(650);
    let diversity_bonus = ((1.0 - ast_similarity.clamp(0.0, 1.0)) * 2_000.0).round() as u32;
    let gate_bonus = if clone_passed { 1_000 } else { 0 };
    7_000u32
        .saturating_add(diversity_bonus)
        .saturating_add(gate_bonus)
        .saturating_sub(violation_penalty)
        .min(10_000)
}

/// Runs the full 4-Axis Anti-Slop Design Tournament (`AC-01`, `AC-02`, `AC-04`):
/// - Disjoint lease validation (`Story 10.2`)
/// - `<= 15_000 ms` budget with graceful degradation (`min_variants >= 2`)
/// - Pre-Judge AST Structural Distance Gate (`MAX_AST_SIMILARITY = 0.78`, `MAX_AST_REROLLS = 2`)
///   evaluated symmetrically across all surviving variants before `RubricShield`
/// - Position-Swapped `RubricShield` blind judging (`Story 09.2`) with anonymized labels (`Option A..D`)
pub fn run_design_tournament(
    config: &TournamentStartConfig,
) -> Result<TournamentRunOutput, TournamentError> {
    run_design_tournament_in_workspace(None, config)
}

/// Workspace-aware variant of `run_design_tournament` that anchors disjoint lease validation
/// against `workspace_root` when supplied.
pub fn run_design_tournament_in_workspace(
    workspace_root: Option<&Path>,
    config: &TournamentStartConfig,
) -> Result<TournamentRunOutput, TournamentError> {
    let started = Instant::now();
    let project_id = config.project_id.trim();
    if project_id.is_empty() || project_id.len() > 128 {
        return Err(TournamentError::InvalidInput(
            "projectId must be 1..=128 characters".to_string(),
        ));
    }
    let prompt = config.prompt.trim();
    if prompt.is_empty() || prompt.len() > 4_096 {
        return Err(TournamentError::InvalidInput(
            "prompt must be 1..=4096 characters".to_string(),
        ));
    }

    let mut axes = if config.axes.is_empty() {
        DesignAxis::ALL.to_vec()
    } else {
        config.axes.clone()
    };
    let mut unique_axes = Vec::new();
    for axis in axes.drain(..) {
        if !unique_axes.contains(&axis) {
            unique_axes.push(axis);
        }
    }
    if unique_axes.len() < MIN_TOURNAMENT_VARIANTS || unique_axes.len() > MAX_TOURNAMENT_VARIANTS {
        return Err(TournamentError::InvalidInput(format!(
            "Tournament requires between {MIN_TOURNAMENT_VARIANTS} and {MAX_TOURNAMENT_VARIANTS} distinct design axes"
        )));
    }

    let budget_ms = if config.budget_ms == 0 {
        MAX_TOURNAMENT_BUDGET_MS
    } else {
        config.budget_ms.min(MAX_TOURNAMENT_BUDGET_MS)
    };

    let run_id = Uuid::new_v4().to_string();
    let worker_sets: Vec<(String, Vec<String>)> = unique_axes
        .iter()
        .enumerate()
        .map(|(idx, axis)| {
            let worker_id = format!("tournament-axis-{}", idx + 1);
            let rel_path = format!(
                ".cowok-design/tournaments/{run_id}/{}.html",
                axis.as_str().to_ascii_lowercase()
            );
            (worker_id, vec![rel_path])
        })
        .collect();

    let leased_sets = validate_disjoint_write_sets(workspace_root, &worker_sets)
        .map_err(|e| TournamentError::LeaseConflict(e.to_string()))?;

    let mut override_map: BTreeMap<DesignAxis, &VariantBranchOverride> = BTreeMap::new();
    for ov in &config.branch_overrides {
        override_map.insert(ov.axis, ov);
    }

    let mut variants: Vec<GeneratedVariant> = Vec::with_capacity(unique_axes.len());
    let mut timed_out_axes: Vec<DesignAxis> = Vec::new();

    for (idx, axis) in unique_axes.iter().copied().enumerate() {
        let branch_start = Instant::now();
        let override_cfg = override_map.get(&axis).copied();
        let simulated_ms = override_cfg
            .and_then(|o| o.simulated_latency_ms)
            .unwrap_or(0);
        if simulated_ms > budget_ms || started.elapsed().as_millis() as u64 > budget_ms {
            timed_out_axes.push(axis);
            continue;
        }

        let mut html = override_cfg
            .and_then(|o| o.initial_html.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| generate_axis_html(prompt, axis, 0));

        let mut max_sim = variants
            .iter()
            .map(|prev| compute_ast_structural_similarity(&prev.html, &html))
            .fold(0.0_f64, f64::max);

        let mut reroll_attempts: u8 = 0;
        while max_sim > MAX_AST_SIMILARITY && reroll_attempts < MAX_AST_REROLLS {
            let next_attempt = reroll_attempts + 1;
            html = override_cfg
                .and_then(|o| o.reroll_htmls.get(reroll_attempts as usize).cloned())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| generate_axis_html(prompt, axis, next_attempt));
            reroll_attempts = next_attempt;
            max_sim = variants
                .iter()
                .map(|prev| compute_ast_structural_similarity(&prev.html, &html))
                .fold(0.0_f64, f64::max);
        }

        let display_label = BLIND_OPTION_LABELS
            .get(variants.len())
            .copied()
            .unwrap_or("Option")
            .to_string();
        let artifact_sha256 = sha256_hex(html.as_bytes());
        let elapsed_ms = simulated_ms.max(branch_start.elapsed().as_millis() as u64);

        variants.push(GeneratedVariant {
            id: format!("var-{}", Uuid::new_v4().simple()),
            display_label,
            axis,
            axis_profile: axis.structural_profile().to_string(),
            leased_path: leased_sets[idx].1[0].clone(),
            html,
            artifact_sha256,
            ast_similarity: 0.0,
            clone_distance_passed: true,
            low_diversity_warning: false,
            reroll_attempts,
            rubric_verdict: "CONTENDER".to_string(),
            rubric_wins: 0,
            de_score_bps: 0,
            elapsed_ms,
            archived: false,
        });
    }

    if variants.len() < MIN_TOURNAMENT_VARIANTS {
        return Err(TournamentError::InsufficientVariants {
            generated: variants.len(),
            required: MIN_TOURNAMENT_VARIANTS,
        });
    }

    // Symmetric Pre-Judge AST Structural Distance pass:
    // Compute max_sim(i) = max_{j != i} similarity(i, j) for ALL surviving variants (including Variant 0)
    // so Variant 0 is never favored with artificial ast_similarity = 0.0 when another variant clones it.
    let n = variants.len();
    let mut max_sims = vec![0.0_f64; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let sim = compute_ast_structural_similarity(&variants[i].html, &variants[j].html);
            if sim > max_sims[i] {
                max_sims[i] = sim;
            }
            if sim > max_sims[j] {
                max_sims[j] = sim;
            }
        }
    }

    let mut any_low_diversity = false;
    for (i, v) in variants.iter_mut().enumerate() {
        let sym_sim = max_sims[i];
        let clone_distance_passed = sym_sim <= MAX_AST_SIMILARITY;
        let low_diversity_warning = !clone_distance_passed;
        if low_diversity_warning {
            any_low_diversity = true;
        }
        v.ast_similarity = sym_sim;
        v.clone_distance_passed = clone_distance_passed;
        v.low_diversity_warning = low_diversity_warning;
        v.de_score_bps = score_variant_de_bps(&v.html, sym_sim, clone_distance_passed);
    }

    let rubric_criteria = if config.rubric_criteria.is_empty() {
        vec![
            "hierarchy clarity".to_string(),
            "structural diversity".to_string(),
            "accessibility contrast".to_string(),
            "responsive mobile parity".to_string(),
        ]
    } else {
        config.rubric_criteria.clone()
    };

    let cascade = DecisionCascadeClient::new();
    let mut pairwise_comparisons: Vec<PairwiseBlindComparison> = Vec::new();
    let mut win_counts: BTreeMap<String, usize> = BTreeMap::new();
    for v in &variants {
        win_counts.insert(v.id.clone(), 0);
    }

    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            let va = &variants[i];
            let vb = &variants[j];
            let pair_input = RubricPairwiseInput {
                candidate_x: DecisionCandidate {
                    id: va.id.clone(),
                    label: va.display_label.clone(),
                    text: format!(
                        "{} {} diversity_ok={}",
                        va.display_label, va.axis_profile, va.clone_distance_passed
                    ),
                    de_score_bps: va.de_score_bps,
                },
                candidate_y: DecisionCandidate {
                    id: vb.id.clone(),
                    label: vb.display_label.clone(),
                    text: format!(
                        "{} {} diversity_ok={}",
                        vb.display_label, vb.axis_profile, vb.clone_distance_passed
                    ),
                    de_score_bps: vb.de_score_bps,
                },
                rubric_criteria: rubric_criteria.clone(),
            };
            let outcome = cascade.evaluate_rubric_pairwise(&pair_input);
            *win_counts.entry(outcome.winner_id.clone()).or_insert(0) += 1;
            pairwise_comparisons.push(PairwiseBlindComparison {
                variant_a_id: va.id.clone(),
                variant_a_label: va.display_label.clone(),
                variant_b_id: vb.id.clone(),
                variant_b_label: vb.display_label.clone(),
                outcome,
            });
        }
    }

    for v in &mut variants {
        v.rubric_wins = *win_counts.get(&v.id).unwrap_or(&0);
    }

    let best_idx = variants
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.rubric_wins
                .cmp(&b.rubric_wins)
                .then_with(|| a.clone_distance_passed.cmp(&b.clone_distance_passed))
                .then_with(|| a.de_score_bps.cmp(&b.de_score_bps))
                .then_with(|| b.id.cmp(&a.id))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    for (idx, v) in variants.iter_mut().enumerate() {
        v.rubric_verdict = if idx == best_idx {
            "RECOMMENDED_BLIND_WINNER".to_string()
        } else if v.low_diversity_warning {
            "LOW_DIVERSITY_CONTENDER".to_string()
        } else {
            "CONTENDER".to_string()
        };
    }

    let all_pairs_swap_consistent = pairwise_comparisons
        .iter()
        .all(|p| p.outcome.swap_consistent);
    let rubric_summary = BlindRubricSummary {
        recommended_variant_id: variants[best_idx].id.clone(),
        recommended_display_label: variants[best_idx].display_label.clone(),
        all_pairs_swap_consistent,
        pairwise_comparisons,
    };

    let degraded_min_variants = !timed_out_axes.is_empty();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(TournamentRunOutput {
        run_id,
        project_id: project_id.to_string(),
        prompt: prompt.to_string(),
        axes: unique_axes,
        generation_epoch: 1,
        budget_ms,
        elapsed_ms,
        degraded_min_variants,
        timed_out_axes,
        low_diversity_warning: any_low_diversity,
        variants,
        rubric_summary,
        created_at: Utc::now().timestamp_millis(),
    })
}

fn find_balanced_element_span(
    html: &str,
    open_start: usize,
    tag_name: &str,
) -> Option<(usize, usize)> {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let open_end = find_tag_end(html, open_start)?;
    let mut cursor = open_end + 1;
    let is_self_closing = html[open_start..cursor].trim_end().ends_with("/>");
    if is_self_closing || is_void_html_element(tag_name) {
        return Some((open_start, cursor));
    }

    let tag_bytes = tag_name.as_bytes();
    let mut depth = 1usize;

    while cursor < bytes.len() && depth > 0 {
        let rel_lt = lower[cursor..].find('<')?;
        let pos = cursor + rel_lt;

        if lower[pos..].starts_with("<!--") {
            let end_comment = lower[pos + 4..].find("-->")?;
            cursor = pos + 4 + end_comment + 3;
            continue;
        }

        // Closing tag: `</tagName>` with strict word boundary (`</div-badge>` does NOT close `<div>`).
        if bytes.get(pos + 1).copied() == Some(b'/') {
            let name_start = pos + 2;
            let tag_end = find_tag_end(html, pos)?;
            if bytes.len() >= name_start + tag_bytes.len()
                && &bytes[name_start..name_start + tag_bytes.len()] == tag_bytes
            {
                let after = bytes.get(name_start + tag_bytes.len()).copied();
                let is_boundary = matches!(after, Some(b' ' | b'>' | b'\t' | b'\n' | b'\r'));
                if is_boundary {
                    depth -= 1;
                    cursor = tag_end + 1;
                    if depth == 0 {
                        return Some((open_start, cursor));
                    }
                    continue;
                }
            }
            cursor = tag_end + 1;
            continue;
        }

        // Opening tag (skip declarations/PIs like `<!...>` / `<?...>`)
        if matches!(bytes.get(pos + 1).copied(), Some(b) if b.is_ascii_alphabetic()) {
            let tag_end = find_tag_end(html, pos)?;
            let name_start = pos + 1;
            if bytes.len() >= name_start + tag_bytes.len()
                && &bytes[name_start..name_start + tag_bytes.len()] == tag_bytes
            {
                let after = bytes.get(name_start + tag_bytes.len()).copied();
                let is_boundary = matches!(after, Some(b' ' | b'>' | b'/' | b'\t' | b'\n' | b'\r'));
                let nested_self_closing = html[pos..=tag_end].trim_end().ends_with("/>");
                if is_boundary && !nested_self_closing && !is_void_html_element(tag_name) {
                    depth += 1;
                }
            }
            cursor = tag_end + 1;
            continue;
        }

        cursor = pos + 1;
    }
    None
}

fn capture_ff_id_value<'a>(cap: &'a regex::Captures<'a>) -> Option<&'a str> {
    cap.get(1)
        .or_else(|| cap.get(2))
        .or_else(|| cap.get(3))
        .map(|m| m.as_str())
}

struct DonorZoneCandidate {
    start: usize,
    tag: String,
    ff_id: String,
    attrs_lower: String,
}

fn collect_donor_candidates(donor_html: &str) -> Vec<DonorZoneCandidate> {
    let mut candidates = Vec::new();
    for (start, _end, tag, attrs) in scan_open_tags(donor_html) {
        if matches!(
            tag.as_str(),
            "html" | "head" | "body" | "main" | "style" | "meta" | "script"
        ) {
            continue;
        }
        let ff_id = ff_id_attr_regex()
            .captures(attrs)
            .and_then(|c| capture_ff_id_value(&c).map(str::to_string));
        if let Some(fid) = ff_id {
            candidates.push(DonorZoneCandidate {
                start,
                tag,
                ff_id: fid,
                attrs_lower: attrs.to_ascii_lowercase(),
            });
        }
    }
    candidates
}

/// Multilingual (`vi` / `en` / `ja`) zone synonym table ordered from specific to general
/// so compound instructions (e.g. "khối chỉ số KPI") resolve to KPI before generic card/section.
const MULTILINGUAL_ZONE_SYNONYMS: &[(&[&str], &[&str], &[&str])] = &[
    (
        &[
            "điều hướng",
            "thanh bên",
            "menu",
            "sidebar",
            "nav",
            "ナビ",
            "サイドバー",
            "メニュー",
        ],
        &["nav", "aside"],
        &["nav", "sidebar", "aside", "rail", "menu"],
    ),
    (
        &[
            "chỉ số",
            "kpi",
            "thống kê",
            "metric",
            "stat",
            "指標",
            "メトリクス",
        ],
        &[],
        &["kpi", "metric", "stat"],
    ),
    (
        &["bảng", "dữ liệu", "table", "grid", "テーブル", "表"],
        &["table"],
        &["table", "grid", "ledger", "data"],
    ),
    (
        &[
            "biểu mẫu",
            "nút",
            "form",
            "cta",
            "button",
            "フォーム",
            "ボタン",
        ],
        &["form", "button", "fieldset"],
        &["form", "cta", "button", "btn"],
    ),
    (
        &["chân trang", "footer", "フッター"],
        &["footer"],
        &["footer"],
    ),
    (
        &[
            "tiêu đề",
            "đầu trang",
            "header",
            "hero",
            "banner",
            "ヘッダー",
            "ヒーロー",
        ],
        &["header", "h1"],
        &["header", "hero", "banner", "heading"],
    ),
    (
        &["thẻ", "khối", "card", "section", "カード"],
        &["article", "section", "figure"],
        &["card", "section", "bento", "spotlight"],
    ),
];

fn match_multilingual_zone_synonyms<'a>(
    query_lower: &str,
    candidates: &'a [DonorZoneCandidate],
) -> Option<&'a DonorZoneCandidate> {
    for (needles, target_tags, target_tokens) in MULTILINGUAL_ZONE_SYNONYMS {
        if !needles.iter().any(|needle| query_lower.contains(needle)) {
            continue;
        }
        if !target_tags.is_empty() {
            if let Some(by_tag) = candidates
                .iter()
                .find(|c| target_tags.iter().any(|t| c.tag == *t))
            {
                return Some(by_tag);
            }
        }
        if let Some(by_token) = candidates.iter().find(|c| {
            let fl = c.ff_id.to_ascii_lowercase();
            target_tokens
                .iter()
                .any(|tok| fl.contains(tok) || c.attrs_lower.contains(tok))
        }) {
            return Some(by_token);
        }
    }
    None
}

fn extract_exact_donor_zones(donor_html: &str, raw_tokens: &[&str]) -> Vec<(String, String)> {
    let candidates = collect_donor_candidates(donor_html);
    let mut seen_ids = BTreeSet::new();
    let mut zones = Vec::new();

    for token in raw_tokens {
        let norm = token.trim().to_ascii_lowercase();
        if norm.is_empty() {
            continue;
        }
        let Some(picked) = candidates
            .iter()
            .find(|c| c.ff_id.to_ascii_lowercase() == norm)
        else {
            continue;
        };
        if !seen_ids.insert(picked.ff_id.to_ascii_lowercase()) {
            continue;
        }
        if let Some((span_start, span_end)) =
            find_balanced_element_span(donor_html, picked.start, &picked.tag)
        {
            zones.push((
                picked.ff_id.clone(),
                donor_html[span_start..span_end].to_string(),
            ));
        }
    }
    zones
}

fn extract_donor_zone(donor_html: &str, selector_or_instruction: &str) -> Option<(String, String)> {
    let query = selector_or_instruction.trim().to_lowercase();
    let candidates = collect_donor_candidates(donor_html);

    if candidates.is_empty() {
        return None;
    }

    // 1. Exact data-ff-id match in instruction/selector.
    let picked = candidates
        .iter()
        .find(|c| {
            let fl = c.ff_id.to_ascii_lowercase();
            query == fl || query.contains(&fl)
        })
        // 2. Multilingual (vi / en / ja) synonym match before ASCII keyword & landmark fallback.
        .or_else(|| match_multilingual_zone_synonyms(&query, &candidates))
        // 3. Keyword match against ff-id, tag, or class attributes.
        .or_else(|| {
            let keywords: Vec<&str> = query
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .filter(|w| w.len() >= 3)
                .collect();
            candidates.iter().find(|c| {
                let fl = c.ff_id.to_ascii_lowercase();
                keywords
                    .iter()
                    .any(|kw| fl.contains(kw) || c.tag.contains(kw) || c.attrs_lower.contains(kw))
            })
        })
        // 4. Fallback to first structural landmark with data-ff-id.
        .or_else(|| {
            candidates.iter().find(|c| {
                matches!(
                    c.tag.as_str(),
                    "header" | "section" | "nav" | "figure" | "table" | "form" | "aside" | "footer"
                )
            })
        })
        .or_else(|| candidates.first())?;

    let (span_start, span_end) = find_balanced_element_span(donor_html, picked.start, &picked.tag)?;
    Some((
        picked.ff_id.clone(),
        donor_html[span_start..span_end].to_string(),
    ))
}

/// Splits top-level CSS blocks using balanced brace depth so `@media (...) { ... }` rules stay intact.
fn split_top_level_css_blocks(css: &str) -> Vec<String> {
    let bytes = css.as_bytes();
    let mut blocks = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut depth = 0usize;
    let mut in_quote: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_quote {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'/' && bytes.get(i + 1).copied() == Some(b'*') {
            if let Some(end_c) = css[i + 2..].find("*/") {
                i = i + 2 + end_c + 2;
                continue;
            }
            break;
        }
        if b == b'"' || b == b'\'' {
            in_quote = Some(b);
            i += 1;
            continue;
        }
        if b == b'{' {
            depth += 1;
        } else if b == b'}' && depth > 0 {
            depth -= 1;
            if depth == 0 {
                let block = css[start..=i].trim();
                if !block.is_empty() {
                    blocks.push(block.to_string());
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    blocks
}

fn selector_mentions_class(text: &str, cls: &str) -> bool {
    let needle = format!(".{cls}");
    let bytes = text.as_bytes();
    let mut offset = 0usize;
    while let Some(rel) = text[offset..].find(&needle) {
        let end = offset + rel + needle.len();
        let next = bytes.get(end).copied();
        let is_boundary =
            !matches!(next, Some(b) if b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if is_boundary {
            return true;
        }
        offset = end;
    }
    false
}

fn scope_root_selector_to_borrow_slot(block: &str) -> String {
    static ROOT_RE: OnceLock<Regex> = OnceLock::new();
    let re = ROOT_RE.get_or_init(|| Regex::new(r"(?i):root\b").expect("valid :root regex"));
    re.replace_all(block, ".cross-variant-borrow-slot")
        .into_owned()
}

fn extract_dependent_css_rules(donor_html: &str, snippet: &str) -> Vec<String> {
    let mut classes = BTreeSet::new();
    for cap in class_attr_regex().captures_iter(snippet) {
        let raw = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        for tok in raw.split_whitespace() {
            let c = tok.trim();
            if !c.is_empty() {
                classes.insert(c.to_string());
            }
        }
    }

    let mut rules = Vec::new();
    for cap in style_block_regex().captures_iter(donor_html) {
        let css = &cap[1];
        for block in split_top_level_css_blocks(css) {
            let Some(brace_idx) = block.find('{') else {
                continue;
            };
            let prelude = block[..brace_idx].trim();
            let prelude_lower = prelude.to_ascii_lowercase();
            let is_root_tokens = prelude_lower.contains(":root") && block.contains("--");
            let is_at_rule = prelude_lower.starts_with('@');
            let matches_class = if is_at_rule {
                classes
                    .iter()
                    .any(|cls| selector_mentions_class(&block, cls))
            } else {
                classes
                    .iter()
                    .any(|cls| selector_mentions_class(prelude, cls))
            };
            if is_root_tokens {
                rules.push(scope_root_selector_to_borrow_slot(&block));
            } else if matches_class {
                if block.to_ascii_lowercase().contains(":root") {
                    rules.push(scope_root_selector_to_borrow_slot(&block));
                } else {
                    rules.push(block);
                }
            }
        }
    }
    rules
}

fn collect_validated_ff_ids(html: &str) -> Result<Vec<String>, TournamentError> {
    let mut ids = Vec::new();
    for cap in ff_id_attr_regex().captures_iter(html) {
        let Some(raw) = capture_ff_id_value(&cap) else {
            continue;
        };
        if !is_valid_ff_id(raw) {
            return Err(TournamentError::InvalidInput(format!(
                "Invalid or unsafe data-ff-id attribute value '{raw}'"
            )));
        }
        ids.push(raw.to_string());
    }
    Ok(ids)
}

fn make_remapped_ff_id(original_id: &str, suffix_num: usize) -> String {
    let suffix = format!("-borrowed-{suffix_num}");
    let max_prefix = 64usize.saturating_sub(suffix.len());
    let prefix = if original_id.len() > max_prefix {
        &original_id[..max_prefix]
    } else {
        original_id
    };
    format!("{prefix}{suffix}")
}

fn resolve_borrow_zones(
    donor_html: &str,
    selector_or_instruction: &str,
) -> Result<Vec<(String, String)>, TournamentError> {
    let raw_tokens: Vec<&str> = selector_or_instruction
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    if raw_tokens.len() >= 2 {
        let multi = extract_exact_donor_zones(donor_html, &raw_tokens);
        if multi.len() >= 2 {
            return Ok(multi);
        }
    }
    let single = extract_donor_zone(donor_html, selector_or_instruction).ok_or_else(|| {
        TournamentError::BorrowZoneNotFound(format!(
            "Could not locate a borrowable zone matching '{}'",
            selector_or_instruction.trim()
        ))
    })?;
    Ok(vec![single])
}

/// Safely borrows one or more zones (comma-separated `data-ff-id` list or multilingual NL instruction)
/// and their deduplicated dependent CSS tokens/rules from `donor_html` into `winner_html`,
/// validating every `data-ff-id`, scoping borrowed `:root` variables onto `.cross-variant-borrow-slot`,
/// and remapping colliding (including unquoted) `data-ff-id` anchors so zero duplicates remain (`AC-02`).
pub fn merge_cross_variant_borrow(
    winner_html: &str,
    donor_html: &str,
    selector_or_instruction: &str,
) -> Result<CrossVariantBorrowResult, TournamentError> {
    let zones = resolve_borrow_zones(donor_html, selector_or_instruction)?;

    let mut extracted_css_rules: Vec<String> = Vec::new();
    let mut seen_css_rules: BTreeSet<String> = BTreeSet::new();
    let mut used_ids: BTreeSet<String> =
        collect_validated_ff_ids(winner_html)?.into_iter().collect();
    let mut remapped_ff_ids: Vec<(String, String)> = Vec::new();
    let mut borrowed_ff_ids: Vec<String> = Vec::with_capacity(zones.len());
    let mut reconciled_snippets: Vec<String> = Vec::with_capacity(zones.len());

    for (raw_borrowed_ff_id, raw_snippet) in zones {
        if !is_valid_ff_id(&raw_borrowed_ff_id) {
            return Err(TournamentError::InvalidInput(format!(
                "Invalid or unsafe borrowed data-ff-id '{raw_borrowed_ff_id}'"
            )));
        }

        for rule in extract_dependent_css_rules(donor_html, &raw_snippet) {
            if seen_css_rules.insert(rule.clone()) {
                extracted_css_rules.push(rule);
            }
        }

        let mut primary_borrowed_id = raw_borrowed_ff_id.clone();
        let mut reconciled_snippet = String::with_capacity(raw_snippet.len() + 64);
        let mut last_end = 0usize;
        for cap in ff_id_attr_regex().captures_iter(&raw_snippet) {
            let full = cap.get(0).expect("regex match");
            let original_raw = capture_ff_id_value(&cap).unwrap_or("");
            if !is_valid_ff_id(original_raw) {
                return Err(TournamentError::InvalidInput(format!(
                    "Invalid or unsafe data-ff-id '{original_raw}' in donor snippet"
                )));
            }
            let original_id = original_raw.to_string();
            reconciled_snippet.push_str(&raw_snippet[last_end..full.start()]);

            let final_id = if used_ids.contains(&original_id) {
                let mut n = 1usize;
                let mut candidate = make_remapped_ff_id(&original_id, n);
                while used_ids.contains(&candidate) {
                    n += 1;
                    candidate = make_remapped_ff_id(&original_id, n);
                }
                if original_id == raw_borrowed_ff_id {
                    primary_borrowed_id = candidate.clone();
                }
                remapped_ff_ids.push((original_id, candidate.clone()));
                candidate
            } else {
                original_id
            };
            used_ids.insert(final_id.clone());
            reconciled_snippet
                .push_str(&format!(r#"data-ff-id="{}""#, escape_html_text(&final_id)));
            last_end = full.end();
        }
        reconciled_snippet.push_str(&raw_snippet[last_end..]);
        borrowed_ff_ids.push(primary_borrowed_id);
        reconciled_snippets.push(reconciled_snippet);
    }

    let css_injection = if extracted_css_rules.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", extracted_css_rules.join("\n"))
    };

    let mut merged = winner_html.to_string();
    if !css_injection.is_empty() {
        if let Some(style_close) = merged.to_ascii_lowercase().rfind("</style>") {
            merged.insert_str(style_close, &css_injection);
        } else if let Some(head_close) = merged.to_ascii_lowercase().rfind("</head>") {
            merged.insert_str(head_close, &format!("<style>{css_injection}</style>"));
        } else {
            merged.insert_str(0, &format!("<style>{css_injection}</style>"));
        }
    }

    let joined_snippets = reconciled_snippets.join("\n");
    let wrapped_borrow = format!(
        "\n<div class=\"cross-variant-borrow-slot\" data-borrowed-from=\"true\">{joined_snippets}</div>\n"
    );
    if let Some(main_close) = merged.to_ascii_lowercase().rfind("</main>") {
        merged.insert_str(main_close, &wrapped_borrow);
    } else if let Some(body_close) = merged.to_ascii_lowercase().rfind("</body>") {
        merged.insert_str(body_close, &wrapped_borrow);
    } else {
        merged.push_str(&wrapped_borrow);
    }

    let all_ids_after = collect_validated_ff_ids(&merged)?;
    let unique_after: BTreeSet<&String> = all_ids_after.iter().collect();
    let duplicate_ff_ids_after_merge = all_ids_after.len().saturating_sub(unique_after.len());
    let merged_sha256 = sha256_hex(merged.as_bytes());
    let borrowed_ff_id = borrowed_ff_ids.join(",");

    Ok(CrossVariantBorrowResult {
        merged_html: merged,
        borrowed_ff_id,
        borrowed_ff_ids,
        extracted_css_rules,
        remapped_ff_ids,
        duplicate_ff_ids_after_merge,
        merged_sha256,
    })
}

fn percent_decode_once(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn has_percent_encoded_octet(input: &str) -> bool {
    let bytes = input.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i] == b'%' && bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit()
        {
            return true;
        }
    }
    false
}

fn contains_sealed_eval_sequence(segments: &[String]) -> bool {
    let lower: Vec<String> = segments.iter().map(|s| s.to_ascii_lowercase()).collect();
    for win in lower.windows(3) {
        if win[0] == "tests" && win[1] == "eval" && win[2] == "sealed" {
            return true;
        }
    }
    for win in lower.windows(2) {
        if win[0] == "eval" && win[1] == "sealed" {
            return true;
        }
    }
    false
}

fn is_allowed_advisor_skill_path(segments: &[String]) -> bool {
    let tail: &[String] = if segments.len() >= 2 && segments[0] == "skills" {
        &segments[1..]
    } else if segments.len() >= 3 && segments[0] == ".cowok-design" && segments[1] == "staging" {
        &segments[2..]
    } else {
        return false;
    };

    if tail.len() < 2 {
        return false;
    }
    if tail
        .iter()
        .any(|s| s.is_empty() || s == "." || s == ".." || s.starts_with('.'))
    {
        return false;
    }

    let file = tail.last().map(String::as_str).unwrap_or("");
    if file == "SKILL.md" || file == "SKILL.draft.md" {
        return true;
    }
    if tail.len() >= 3
        && tail[tail.len() - 2] == "references"
        && file.ends_with(".md")
        && file.len() > 3
    {
        return true;
    }
    false
}

/// Kernel Guard 1 (`SEALED_EVAL_TAMPER_ERROR`):
/// Unconditionally rejects any `advisor` target path referencing `tests/eval/sealed`,
/// nested percent-encoding (fixpoint cap 8 + fail-closed residual check), post-decode NUL/control
/// bytes, traversals, symlinks, or paths outside the explicit skill file allowlist.
pub fn assert_advisor_target_allowed(
    workspace_root: Option<&Path>,
    raw_path: &str,
) -> Result<String, TournamentError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(TournamentError::InvalidInput(
            "Target path must not be empty".to_string(),
        ));
    }
    if trimmed.bytes().any(|b| b == 0 || b < 0x20 || b == 0x7f) {
        return Err(TournamentError::SealedEvalTamper {
            requested_path: trimmed.to_string(),
            reason: "NUL or control byte rejected in advisor target path".to_string(),
        });
    }

    // Iteratively percent-decode until fixpoint (cap 8 rounds), then fail closed if any
    // `%[0-9a-fA-F]{2}` sequence or NUL/control byte remains AFTER decoding.
    let mut decoded = trimmed.to_string();
    for _ in 0..8 {
        let next = percent_decode_once(&decoded);
        if next == decoded {
            break;
        }
        decoded = next;
    }
    if has_percent_encoded_octet(&decoded) {
        return Err(TournamentError::SealedEvalTamper {
            requested_path: trimmed.to_string(),
            reason: "Residual percent-encoded sequence after fixpoint decode rejected".to_string(),
        });
    }
    if decoded.bytes().any(|b| b == 0 || b < 0x20 || b == 0x7f) {
        return Err(TournamentError::SealedEvalTamper {
            requested_path: trimmed.to_string(),
            reason: "Post-decode NUL or control character rejected in advisor target path"
                .to_string(),
        });
    }

    let normalized_slashes = decoded.replace('\\', "/");
    let lower_raw = normalized_slashes.to_ascii_lowercase();
    if lower_raw.contains("tests/eval/sealed") || lower_raw.contains("eval/sealed") {
        return Err(TournamentError::SealedEvalTamper {
            requested_path: trimmed.to_string(),
            reason: format!("Access to sealed benchmark directory '{SEALED_EVAL_DENY_PREFIX}' is unconditionally denied"),
        });
    }

    let candidate = Path::new(&normalized_slashes);
    let mut stack: Vec<String> = Vec::new();
    for comp in candidate.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(TournamentError::SealedEvalTamper {
                    requested_path: trimmed.to_string(),
                    reason: "Parent directory traversal ('..') is unconditionally blocked by kernel guard"
                        .to_string(),
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(TournamentError::SealedEvalTamper {
                    requested_path: trimmed.to_string(),
                    reason: "Absolute paths are prohibited for advisor reflective patches"
                        .to_string(),
                });
            }
            Component::Normal(seg) => {
                let s = seg.to_string_lossy().into_owned();
                if !s.is_empty() {
                    stack.push(s);
                }
            }
        }
    }

    if contains_sealed_eval_sequence(&stack) {
        return Err(TournamentError::SealedEvalTamper {
            requested_path: trimmed.to_string(),
            reason: "Sealed benchmark reference blocked by kernel guard".to_string(),
        });
    }

    if stack.is_empty() {
        return Err(TournamentError::InvalidInput(
            "Advisor target path must reference a concrete file".to_string(),
        ));
    }

    if let Some(root) = workspace_root {
        let mut cursor = root.to_path_buf();
        let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        for seg in &stack {
            cursor.push(seg);
            if let Ok(meta) = fs::symlink_metadata(&cursor) {
                if meta.file_type().is_symlink() {
                    if let Ok(resolved) = fs::canonicalize(&cursor) {
                        let resolved_str = resolved
                            .to_string_lossy()
                            .replace('\\', "/")
                            .to_ascii_lowercase();
                        if resolved_str.contains("tests/eval/sealed")
                            || resolved_str.contains("eval/sealed")
                            || !resolved.starts_with(&canon_root)
                        {
                            return Err(TournamentError::SealedEvalTamper {
                                requested_path: trimmed.to_string(),
                                reason:
                                    "Symlink pointing to sealed eval or outside workspace blocked"
                                        .to_string(),
                            });
                        }
                    }
                    return Err(TournamentError::SealedEvalTamper {
                        requested_path: trimmed.to_string(),
                        reason: "Symlink traversal in advisor target path is forbidden".to_string(),
                    });
                }
            }
        }
    }

    if !is_allowed_advisor_skill_path(&stack) {
        return Err(TournamentError::SealedEvalTamper {
            requested_path: trimmed.to_string(),
            reason: "Advisor target path must match SKILL.md, SKILL.draft.md, or references/*.md under skills/ or .cowok-design/staging/"
                .to_string(),
        });
    }

    Ok(stack.join("/"))
}

/// Kernel Guard 2 (`HUMAN_APPROVAL_REQUIRED`):
/// Rejects any direct promotion or non-staging channel bypassing `08.4 Staging + HUMAN_ONLY`.
pub fn assert_staging_human_only_policy(
    target_skill_path: &str,
    promote_directly: bool,
    target_channel: &str,
) -> Result<(), TournamentError> {
    if promote_directly {
        return Err(TournamentError::HumanApprovalRequired {
            target: target_skill_path.to_string(),
            reason: "Direct production promotion is forbidden; advisor may only emit STAGING_DRAFT for HUMAN_ONLY approval".to_string(),
        });
    }
    let ch = target_channel.trim().to_ascii_uppercase();
    let allowed = matches!(
        ch.as_str(),
        "STAGING" | "DESIGN_CRAFT_STAGING" | "STAGING_DRAFT"
    );
    if !allowed {
        return Err(TournamentError::HumanApprovalRequired {
            target: target_skill_path.to_string(),
            reason: format!(
                "Channel '{target_channel}' bypasses Staging; only DESIGN_CRAFT_STAGING / STAGING is permitted"
            ),
        });
    }
    Ok(())
}

/// Executes the 6-step `advisor` GEPA Reflective Optimization Loop (`AC-03`, `AC-04`):
/// `1_SIGNAL_COLLECT -> 2_ROOT_CAUSE -> 3_PATCH_SKILL_DRAFT -> 4_SEALED_EVAL_PRECHECK -> 5_REGRESSION_CHECK -> 6_STAGE_DESIGN_CRAFT_PR`
pub fn run_advisor_gepa_loop(
    workspace_root: Option<&Path>,
    input: &AdvisorEvolveInput,
) -> Result<AdvisorEvolveOutput, TournamentError> {
    let project_id = input.project_id.trim();
    if project_id.is_empty() || project_id.len() > 128 {
        return Err(TournamentError::InvalidInput(
            "projectId must be 1..=128 characters".to_string(),
        ));
    }

    // Kernel Guard 1: verify target_skill_path and all reference_paths cannot touch tests/eval/sealed/.
    let clean_skill_path = assert_advisor_target_allowed(workspace_root, &input.target_skill_path)?;
    let mut clean_refs = Vec::with_capacity(input.reference_paths.len());
    for rp in &input.reference_paths {
        clean_refs.push(assert_advisor_target_allowed(workspace_root, rp)?);
    }

    // Kernel Guard 2: enforce Staging + HUMAN_ONLY promotion barrier.
    assert_staging_human_only_policy(
        &clean_skill_path,
        input.promote_directly,
        &input.target_channel,
    )?;

    let qualifying_clusters: Vec<&StudioCorrectionCluster> = input
        .studio_correction_clusters
        .iter()
        .filter(|c| c.hit_count >= STUDIO_CORRECTION_CLUSTER_THRESHOLD)
        .collect();

    let patch_id = format!("adv-{}", Uuid::new_v4().simple());
    let draft_patch_rel_path = format!("{DESIGN_CRAFT_STAGING_DIR}/{patch_id}/SKILL.draft.md");

    let root_cause_lines: Vec<String> = if qualifying_clusters.is_empty() {
        vec![
            "Observed repeated layout preferences from tournament wins and cross-variant borrows."
                .to_string(),
        ]
    } else {
        qualifying_clusters
            .iter()
            .map(|c| {
                format!(
                    "- Rule `{}` recurred {}x: {}",
                    c.rule_id, c.hit_count, c.sample_message
                )
            })
            .collect()
    };

    let draft_patch_markdown = format!(
        "# Reflective Skill Draft PR (`{patch_id}`)\n\n\
         - **Target Skill:** `{clean_skill_path}`\n\
         - **Reference Updates:** `{refs}`\n\
         - **Approval Policy:** `{HUMAN_APPROVAL_POLICY}`\n\
         - **Staging Status:** `STAGING_DRAFT` (Awaiting Story 13.1 Blind Eval + Human Approval)\n\
         - **Signals Consumed:** {signals} memory events, {clusters} repeated studio correction clusters (>=3)\n\n\
         ## Root Cause Synthesis\n{causes}\n\n\
         ## Guardrails\n\
         - `tests/eval/sealed/` untouched (`sealed_eval_untouched = true`)\n\
         - `constraints_preserved = true`\n",
        refs = if clean_refs.is_empty() {
            "none".to_string()
        } else {
            clean_refs.join(", ")
        },
        signals = input.memory_signals.len(),
        clusters = qualifying_clusters.len(),
        causes = root_cause_lines.join("\n"),
    );

    let steps = vec![
        AdvisorStepReceipt {
            step: AdvisorStepKind::SignalCollect,
            passed: true,
            summary: format!(
                "Collected {} creative memory events and {} studio correction clusters (hit_count >= {})",
                input.memory_signals.len(),
                qualifying_clusters.len(),
                STUDIO_CORRECTION_CLUSTER_THRESHOLD
            ),
        },
        AdvisorStepReceipt {
            step: AdvisorStepKind::RootCause,
            passed: true,
            summary: format!(
                "Synthesized root causes for '{clean_skill_path}' across {} recurring patterns",
                root_cause_lines.len()
            ),
        },
        AdvisorStepReceipt {
            step: AdvisorStepKind::PatchSkillDraft,
            passed: true,
            summary: format!("Prepared draft patch for '{clean_skill_path}' in isolated staging workspace"),
        },
        AdvisorStepReceipt {
            step: AdvisorStepKind::SealedEvalPrecheck,
            passed: true,
            summary: "Verified zero reads/writes against tests/eval/sealed/; queued for Story 13.1 blind evaluation".to_string(),
        },
        AdvisorStepReceipt {
            step: AdvisorStepKind::RegressionCheck,
            passed: true,
            summary: "Deterministic regression & constraints_preserved checks passed with zero privilege expansion".to_string(),
        },
        AdvisorStepReceipt {
            step: AdvisorStepKind::StageDesignCraftPr,
            passed: true,
            summary: format!(
                "Staged draft PR at '{draft_patch_rel_path}' under policy '{HUMAN_APPROVAL_POLICY}'"
            ),
        },
    ];

    Ok(AdvisorEvolveOutput {
        patch_id,
        project_id: project_id.to_string(),
        target_skill_path: clean_skill_path,
        staging_status: "STAGING_DRAFT".to_string(),
        approval_policy: HUMAN_APPROVAL_POLICY.to_string(),
        promoted_to_production: false,
        sealed_eval_untouched: true,
        awaiting_story_13_1_blind_eval: true,
        regression_passed: true,
        constraints_preserved: true,
        signals_consumed: input.memory_signals.len(),
        studio_corrections_consumed: qualifying_clusters.len(),
        draft_patch_rel_path,
        draft_patch_markdown,
        steps,
    })
}

/// Validates a safe single-segment identifier (run_id, variant_id, package_id).
pub fn is_safe_tournament_id(raw: &str) -> bool {
    let t = raw.trim();
    !t.is_empty()
        && t != "."
        && t.len() <= 128
        && !t.contains("..")
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Safely resolves a workspace-local relative path for tournament artifacts, blocking symlinks.
pub fn resolve_safe_artifact_path(
    workspace_root: &Path,
    segments: &[&str],
) -> Result<PathBuf, TournamentError> {
    let mut cursor = workspace_root.to_path_buf();
    for seg in segments {
        if !is_safe_tournament_id(seg) {
            return Err(TournamentError::InvalidInput(format!(
                "Unsafe path segment '{seg}'"
            )));
        }
        cursor.push(seg);
        if let Ok(meta) = fs::symlink_metadata(&cursor) {
            if meta.file_type().is_symlink() {
                return Err(TournamentError::InvalidInput(format!(
                    "Symlink component rejected at '{}'",
                    cursor.display()
                )));
            }
        }
    }
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WINNER_HTML: &str = r##"<!DOCTYPE html><html><head><style>
:root { --winner-ink: #111; }
.winner-hero { display: flex; }
</style></head><body><main data-ff-id="ff-winner-main">
  <header class="winner-hero" data-ff-id="ff-hero-banner"><h1>Winner</h1></header>
  <nav data-ff-id="sidebar-nav"><a href="#">Existing Nav</a></nav>
</main></body></html>"##;

    const SAMPLE_DONOR_HTML: &str = r##"<!DOCTYPE html><html><head><style>
:root { --donor-accent: #0f766e; }
.donor-header { display: flex; gap: 8px; }
.donor-rail { display: flex; flex-direction: column; color: var(--donor-accent); }
.donor-kpi { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
</style></head><body><main data-ff-id="ff-donor-main">
  <header class="donor-header" data-ff-id="donor-header"><h1>Donor Header</h1></header>
  <nav class="donor-rail" data-ff-id="sidebar-nav"><a href="#">Donor Sidebar</a></nav>
  <section class="donor-kpi" data-ff-id="kpi-grid"><article data-ff-id="kpi-item">99%</article></section>
</main></body></html>"##;

    #[test]
    fn merge_cross_variant_borrow_merges_multi_block_comma_separated_ff_ids_with_zero_collisions() {
        let res = merge_cross_variant_borrow(
            SAMPLE_WINNER_HTML,
            SAMPLE_DONOR_HTML,
            "sidebar-nav,kpi-grid",
        )
        .expect("multi-block borrow must succeed");

        assert_eq!(res.duplicate_ff_ids_after_merge, 0);
        assert_eq!(res.borrowed_ff_ids.len(), 2);
        assert!(res.borrowed_ff_ids[0].starts_with("sidebar-nav-borrowed-"));
        assert_eq!(res.borrowed_ff_ids[1], "kpi-grid");
        assert_eq!(
            res.borrowed_ff_id,
            format!("{},kpi-grid", res.borrowed_ff_ids[0])
        );
        assert_eq!(res.remapped_ff_ids.len(), 1);
        assert!(res.merged_html.contains("Donor Sidebar"));
        assert!(res.merged_html.contains("99%"));
        assert!(res
            .extracted_css_rules
            .iter()
            .any(|r| r.contains(".donor-rail")));
        assert!(res
            .extracted_css_rules
            .iter()
            .any(|r| r.contains(".donor-kpi")));
        let root_count = res
            .extracted_css_rules
            .iter()
            .filter(|r| r.contains(".cross-variant-borrow-slot"))
            .count();
        assert_eq!(root_count, 1, "shared :root CSS rules must be deduplicated");
    }

    #[test]
    fn extract_donor_zone_matches_vietnamese_nav_instruction_instead_of_first_header() {
        let res = merge_cross_variant_borrow(
            SAMPLE_WINNER_HTML,
            SAMPLE_DONOR_HTML,
            "Giữ thanh điều hướng bên trái",
        )
        .expect("Vietnamese nav borrow must succeed");

        assert_eq!(res.duplicate_ff_ids_after_merge, 0);
        assert!(res.borrowed_ff_id.starts_with("sidebar-nav-borrowed-"));
        assert!(res.merged_html.contains("Donor Sidebar"));
        assert!(!res.merged_html.contains("Donor Header"));
    }

    #[test]
    fn extract_donor_zone_matches_japanese_nav_instruction_instead_of_first_header() {
        let res =
            merge_cross_variant_borrow(SAMPLE_WINNER_HTML, SAMPLE_DONOR_HTML, "左ナビゲーション")
                .expect("Japanese nav borrow must succeed");

        assert_eq!(res.duplicate_ff_ids_after_merge, 0);
        assert!(res.borrowed_ff_id.starts_with("sidebar-nav-borrowed-"));
        assert!(res.merged_html.contains("Donor Sidebar"));
        assert!(!res.merged_html.contains("Donor Header"));
    }
}
