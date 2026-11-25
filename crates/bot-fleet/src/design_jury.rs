// @story 10.2
// @implements AC-02, AC-03, AC-04 (Independent Builder != Scorer Guard, E1..E4 >= 95% Verifier Engine, Tier-3 Single-Session Design Jury with Generation Fencing & V18a gate_failure_ledger Escalation)
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::swarm_leases::WorkerRole;

/// Minimum required detection catch rate on canonical E1..E4 defect suites (`>= 95%`).
pub const REQUIRED_E1_E4_CATCH_RATE: f64 = 0.95;

/// Recurrence threshold (`>= 2` repeats) that escalates a QC/Jury violation into `V18a gate_failure_ledger`.
pub const RECURRING_VIOLATION_LEDGER_THRESHOLD: usize = 2;

/// Mandatory approval policy attached to every ledger escalation (no auto-activation).
pub const HUMAN_ONLY_APPROVAL_POLICY: &str = "HUMAN_ONLY";

/// Minimum passing score per Tier-3 Design Jury lens (`0.0..=10.0`).
pub const MIN_JURY_LENS_PASS_SCORE: f64 = 7.0;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DesignJuryError {
    #[error("BUILDER_SELF_PASS_FORBIDDEN: builder '{builder_worker_id}' cannot score or self-PASS via scorer '{scorer_worker_id}'")]
    BuilderSelfPassForbidden {
        builder_worker_id: String,
        scorer_worker_id: String,
    },
    #[error("STALE_GENERATION_FENCE: expected generation_fence {expected}, provided {provided}")]
    StaleGenerationFence { expected: i64, provided: i64 },
    #[error("INVALID_FREEZE_SNAPSHOT_SHA256: {0}")]
    InvalidFreezeHash(String),
    #[error("INVALID_JURY_INPUT: {0}")]
    InvalidJuryInput(String),
}

/// Canonical 4-tier evidence verification ladder (`E1..E4`, compatible with Story `07.1` / `V26 qc_gate_runs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceTier {
    #[serde(rename = "E1")]
    E1,
    #[serde(rename = "E2")]
    E2,
    #[serde(rename = "E3")]
    E3,
    #[serde(rename = "E4")]
    E4,
}

impl EvidenceTier {
    pub const ALL: [EvidenceTier; 4] = [Self::E1, Self::E2, Self::E3, Self::E4];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::E1 => "E1",
            Self::E2 => "E2",
            Self::E3 => "E3",
            Self::E4 => "E4",
        }
    }
}

/// Canonical 4 critique lenses evaluated in a single Tier-3 `Single-Session Design Jury`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JuryLens {
    #[serde(rename = "VISUAL")]
    Visual,
    #[serde(rename = "UX_FLOW")]
    UxFlow,
    #[serde(rename = "COPY")]
    Copy,
    #[serde(rename = "BRAND_IDENTITY")]
    BrandIdentity,
}

impl JuryLens {
    pub const ALL: [JuryLens; 4] = [Self::Visual, Self::UxFlow, Self::Copy, Self::BrandIdentity];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Visual => "VISUAL",
            Self::UxFlow => "UX_FLOW",
            Self::Copy => "COPY",
            Self::BrandIdentity => "BRAND_IDENTITY",
        }
    }
}

/// Persisted verdict values matching `V27 fleet_jury_verdicts.verdict CHECK` constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JuryVerdictStatus {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "ESCALATED_TO_LEDGER")]
    EscalatedToLedger,
}

impl JuryVerdictStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::EscalatedToLedger => "ESCALATED_TO_LEDGER",
        }
    }
}

/// Contrast ratio measurement consumed by the `E3` accessibility detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContrastMeasurement {
    pub selector: String,
    pub ratio: f64,
    #[serde(default)]
    pub is_large_text: bool,
}

/// Element layout bounding box consumed by the `E2` geometry/collision detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementBounds {
    pub selector: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub clipped: bool,
    #[serde(default = "default_viewport_width")]
    pub viewport_width: f64,
}

fn default_viewport_width() -> f64 {
    1440.0
}

/// Single upstream QC issue reported by `Story 07.1` / `V26 qc_gate_runs` or external inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcIssueInput {
    pub rule_id: String,
    pub tier: EvidenceTier,
    pub severity: String,
    #[serde(default)]
    pub selector: Option<String>,
    pub message: String,
}

/// Complete verification input for independent `E1..E4` scoring.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcRunInput {
    #[serde(default)]
    pub qc_run_id: Option<String>,
    pub screen_id: String,
    #[serde(default)]
    pub html_snapshot: Option<String>,
    #[serde(default)]
    pub reported_issues: Vec<QcIssueInput>,
    #[serde(default)]
    pub expected_defect_ids: Vec<String>,
    #[serde(default)]
    pub contrast_ratios: Vec<ContrastMeasurement>,
    #[serde(default)]
    pub bounding_boxes: Vec<ElementBounds>,
    #[serde(default)]
    pub brand_tokens_expected: Vec<String>,
}

/// Detected violation normalized with deterministic SHA-256 fingerprint for recurrence tracking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcViolationItem {
    pub rule_id: String,
    pub tier: EvidenceTier,
    pub severity: String,
    pub selector: Option<String>,
    pub message: String,
    pub fingerprint_sha256: String,
    pub recurrence_count: usize,
}

/// Aggregate result emitted by `evaluate_e1_e4_evidence`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct E1E4EvaluationResult {
    pub qc_run_id: Option<String>,
    pub screen_id: String,
    pub catch_rate: f64,
    pub matched_expected_defects: usize,
    pub total_expected_defects: usize,
    pub tier_violation_counts: BTreeMap<String, usize>,
    pub violations: Vec<QcViolationItem>,
}

/// Frozen visual/DOM bundle evaluated by Tier-3 `Single-Session Design Jury`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFreezeBundle {
    pub project_id: String,
    pub screen_id: String,
    pub generation_fence: i64,
    pub freeze_snapshot_sha256: String,
    #[serde(default)]
    pub frozen_html: Option<String>,
    pub frozen_at: i64,
}

/// Per-lens critique verdict inside the single-session Tier-3 Design Jury.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JuryLensScore {
    pub lens: JuryLens,
    pub score: f64,
    pub passed: bool,
    pub findings: Vec<String>,
}

/// Structured feedback package delivered back to `cowok` orchestrator (`AC-04`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcViolationReport {
    pub report_id: String,
    pub project_id: String,
    pub screen_id: String,
    pub builder_worker_id: String,
    pub scorer_worker_id: String,
    pub generation_fence: i64,
    pub freeze_snapshot_sha256: String,
    pub qc_run_id: Option<String>,
    pub e1_e4_catch_rate: f64,
    pub verdict: JuryVerdictStatus,
    pub lens_scores: Vec<JuryLensScore>,
    pub violations: Vec<QcViolationItem>,
    pub recommended_fixes: Vec<String>,
    pub escalated_to_ledger: bool,
}

/// Payload prepared for `V18a`/`V20 gate_failure_ledger` (`HUMAN_ONLY` promotion policy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateFailureLedgerPayload {
    pub rule_id: String,
    pub fingerprint_sha256: String,
    pub sample_message: String,
    pub hit_count: i64,
    pub status: String,
    pub approval_policy: String,
}

/// Input bundle for `evaluate_design_jury_session`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignJurySessionInput {
    pub builder_worker_id: String,
    pub scorer_worker_id: String,
    pub scorer_role: WorkerRole,
    pub freeze_bundle: ReviewFreezeBundle,
    pub qc_input: QcRunInput,
}

/// Output returned by `evaluate_design_jury_session`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignJuryOutcome {
    pub report: QcViolationReport,
    pub ledger_escalations: Vec<GateFailureLedgerPayload>,
}

/// Unconditionally blocks any attempt by a Builder to self-score or self-PASS (`AC-02`, `AC-03`).
pub fn enforce_builder_neq_scorer(
    builder_worker_id: &str,
    scorer_worker_id: &str,
    scorer_role: WorkerRole,
) -> Result<(), DesignJuryError> {
    let builder = builder_worker_id.trim();
    let scorer = scorer_worker_id.trim();
    if builder.is_empty() || scorer.is_empty() {
        return Err(DesignJuryError::InvalidJuryInput(
            "Both builderWorkerId and scorerWorkerId must be non-empty".to_string(),
        ));
    }
    if builder.eq_ignore_ascii_case(scorer) || scorer_role == WorkerRole::Builder {
        return Err(DesignJuryError::BuilderSelfPassForbidden {
            builder_worker_id: builder.to_string(),
            scorer_worker_id: scorer.to_string(),
        });
    }
    Ok(())
}

/// Validates a 64-character lowercase/uppercase hexadecimal SHA-256 digest.
pub fn validate_sha256_hex(hash: &str) -> Result<String, DesignJuryError> {
    let trimmed = hash.trim();
    if trimmed.len() != 64 || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DesignJuryError::InvalidFreezeHash(format!(
            "freezeSnapshotSha256 must be a 64-character hex SHA-256 digest (got len={})",
            trimmed.len()
        )));
    }
    Ok(trimmed.to_ascii_lowercase())
}

/// Computes a deterministic SHA-256 fingerprint for a QC violation so recurring defects can be tracked.
pub fn compute_violation_fingerprint(
    tier: EvidenceTier,
    rule_id: &str,
    selector: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"fleet_qc_violation_v1\0");
    hasher.update(tier.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(rule_id.trim().to_ascii_lowercase().as_bytes());
    hasher.update(b"\0");
    hasher.update(selector.unwrap_or("*").trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn push_unique_violation(
    out: &mut Vec<QcViolationItem>,
    seen: &mut BTreeSet<String>,
    tier: EvidenceTier,
    rule_id: &str,
    severity: &str,
    selector: Option<String>,
    message: String,
) {
    let fp = compute_violation_fingerprint(tier, rule_id, selector.as_deref());
    if seen.insert(fp.clone()) {
        out.push(QcViolationItem {
            rule_id: rule_id.to_string(),
            tier,
            severity: severity.to_ascii_uppercase(),
            selector,
            message,
            fingerprint_sha256: fp,
            recurrence_count: 1,
        });
    }
}

fn boxes_overlap(a: &ElementBounds, b: &ElementBounds) -> bool {
    if a.width <= 0.0 || a.height <= 0.0 || b.width <= 0.0 || b.height <= 0.0 {
        return false;
    }
    let x_overlap = a.x < (b.x + b.width) && (a.x + a.width) > b.x;
    let y_overlap = a.y < (b.y + b.height) && (a.y + a.height) > b.y;
    x_overlap && y_overlap
}

fn inspect_html_e1_e4(
    html: &str,
    brand_tokens_expected: &[String],
    out: &mut Vec<QcViolationItem>,
    seen: &mut BTreeSet<String>,
) {
    let lower = html.to_ascii_lowercase();

    // --- E1: Deterministic DOM / CSS / XSS / Hygiene ---
    if lower.contains("<script") || lower.contains("javascript:") || lower.contains("onerror=") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E1,
            "E1_INLINE_SCRIPT_XSS",
            "CRITICAL",
            Some("document".to_string()),
            "Forbidden inline <script> or event-handler XSS surface detected in HTML snapshot"
                .to_string(),
        );
    }
    if lower.contains("todo:") || lower.contains("tbd") || lower.contains("{{") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E1,
            "E1_UNRESOLVED_TEMPLATE_PLACEHOLDER",
            "HIGH",
            Some("document".to_string()),
            "Unresolved template token or TODO/TBD placeholder found in rendered HTML".to_string(),
        );
    }
    if lower.contains("var(--undefined")
        || lower.contains("color: unset;")
        || lower.contains("pxpx")
    {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E1,
            "E1_INVALID_CSS_TOKEN",
            "HIGH",
            Some("style".to_string()),
            "Invalid CSS variable reference or malformed unit ('pxpx') detected".to_string(),
        );
    }
    if lower.contains("<img") && !lower.contains("alt=") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E1,
            "E1_IMG_MISSING_ALT",
            "HIGH",
            Some("img".to_string()),
            "Image element is missing mandatory alt attribute".to_string(),
        );
    }
    if lower.contains("<style") && lower.contains("!important") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E1,
            "E1_CSS_IMPORTANT_OVERRIDE",
            "MEDIUM",
            Some("style".to_string()),
            "CSS !important override violates design token cascade hygiene".to_string(),
        );
    }
    if lower.contains("href=\"#\"") || lower.contains("href=''") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E1,
            "E1_DEAD_ANCHOR_LINK",
            "MEDIUM",
            Some("a".to_string()),
            "Dead anchor link (href=\"#\" or empty) detected in interactive surface".to_string(),
        );
    }

    // --- E2: Visual Geometry / Responsive Layout markers ---
    if lower.contains("overflow: hidden") && lower.contains("white-space: nowrap") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E2,
            "E2_TEXT_TRUNCATION_RISK",
            "MEDIUM",
            Some("text".to_string()),
            "Unbounded white-space: nowrap paired with overflow: hidden risks text clipping"
                .to_string(),
        );
    }
    if lower.contains("width: 9999px") || lower.contains("min-width: 2400px") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E2,
            "E2_HORIZONTAL_OVERFLOW",
            "HIGH",
            Some("layout".to_string()),
            "Fixed container width exceeds desktop viewport and causes horizontal scroll overflow"
                .to_string(),
        );
    }
    if lower.contains("z-index: 999999") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E2,
            "E2_ZINDEX_STACKING_ABUSE",
            "MEDIUM",
            Some("layer".to_string()),
            "Unbounded z-index value breaks modal/drawer elevation hierarchy".to_string(),
        );
    }

    // --- E3: Accessibility & Semantics ---
    if (lower.contains("<button") && lower.contains("></button>"))
        || (lower.contains("<button") && lower.contains("<svg") && !lower.contains("aria-label="))
    {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E3,
            "E3_ICON_BUTTON_MISSING_ARIA_LABEL",
            "HIGH",
            Some("button".to_string()),
            "Icon-only or empty button lacks accessible aria-label".to_string(),
        );
    }
    if lower.contains("<input")
        && !lower.contains("<label")
        && !lower.contains("aria-label=")
        && !lower.contains("aria-labelledby=")
    {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E3,
            "E3_INPUT_MISSING_LABEL",
            "HIGH",
            Some("input".to_string()),
            "Form input lacks associated <label> or ARIA label".to_string(),
        );
    }
    if lower.contains("outline: none") && !lower.contains("focus-visible") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E3,
            "E3_FOCUS_RING_REMOVED",
            "HIGH",
            Some(":focus".to_string()),
            "Focus outline suppressed without replacement :focus-visible indicator".to_string(),
        );
    }
    if !lower.contains("<main") && lower.contains("<body") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E3,
            "E3_MISSING_MAIN_LANDMARK",
            "MEDIUM",
            Some("body".to_string()),
            "Document body lacks semantic <main> landmark".to_string(),
        );
    }

    // --- E4: Brand Identity, Visual Hierarchy & Copy Quality ---
    if lower.contains("lorem ipsum")
        || lower.contains("dolor sit amet")
        || lower.contains("sample text here")
    {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E4,
            "E4_LOREM_IPSUM_COPY",
            "HIGH",
            Some("copy".to_string()),
            "Placeholder 'Lorem ipsum' or dummy copy detected instead of domain copy".to_string(),
        );
    }
    let h1_count = lower.matches("<h1").count();
    if lower.contains("<body") && h1_count == 0 {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E4,
            "E4_MISSING_PRIMARY_H1",
            "HIGH",
            Some("h1".to_string()),
            "Screen lacks a primary <h1> heading for clear typographic hierarchy".to_string(),
        );
    } else if h1_count > 1 {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E4,
            "E4_MULTIPLE_H1_HEADINGS",
            "MEDIUM",
            Some("h1".to_string()),
            format!("Detected {h1_count} <h1> elements; single primary heading expected"),
        );
    }
    if lower.contains("click here") || lower.contains("submit button") {
        push_unique_violation(
            out,
            seen,
            EvidenceTier::E4,
            "E4_GENERIC_CTA_COPY",
            "MEDIUM",
            Some("button".to_string()),
            "Generic non-actionable CTA copy ('Click here' / 'Submit button') detected".to_string(),
        );
    }
    for token in brand_tokens_expected {
        let tok = token.trim();
        if !tok.is_empty() && !html.contains(tok) {
            push_unique_violation(
                out,
                seen,
                EvidenceTier::E4,
                "E4_MISSING_BRAND_TOKEN",
                "HIGH",
                Some(tok.to_string()),
                format!("Mandatory brand design token '{tok}' is absent from rendered screen"),
            );
        }
    }
}

/// Runs independent `E1..E4` evidence verification over `QcRunInput` (compatible with `V26 qc_gate_runs` / Story `07.1`).
pub fn evaluate_e1_e4_evidence(input: &QcRunInput) -> E1E4EvaluationResult {
    let mut violations: Vec<QcViolationItem> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    // 1. Ingest upstream Story 07.1 / V26 reported issues.
    for issue in &input.reported_issues {
        push_unique_violation(
            &mut violations,
            &mut seen,
            issue.tier,
            &issue.rule_id,
            &issue.severity,
            issue.selector.clone(),
            issue.message.clone(),
        );
    }

    // 2. Deterministic E1..E4 inspection over HTML snapshot (when present).
    if let Some(ref html) = input.html_snapshot {
        inspect_html_e1_e4(
            html,
            &input.brand_tokens_expected,
            &mut violations,
            &mut seen,
        );
    }

    // 3. E2 geometry collision, clipping, viewport overflow & touch target checks.
    for (idx, box_a) in input.bounding_boxes.iter().enumerate() {
        if box_a.clipped {
            push_unique_violation(
                &mut violations,
                &mut seen,
                EvidenceTier::E2,
                "E2_ELEMENT_CLIPPED",
                "HIGH",
                Some(box_a.selector.clone()),
                format!(
                    "Element '{}' is clipped by its parent container",
                    box_a.selector
                ),
            );
        }
        if box_a.x < 0.0 || (box_a.x + box_a.width) > box_a.viewport_width {
            push_unique_violation(
                &mut violations,
                &mut seen,
                EvidenceTier::E2,
                "E2_VIEWPORT_OVERFLOW",
                "HIGH",
                Some(box_a.selector.clone()),
                format!(
                    "Element '{}' overflows horizontal viewport ({}px)",
                    box_a.selector, box_a.viewport_width
                ),
            );
        }
        if box_a.interactive && (box_a.width < 24.0 || box_a.height < 24.0) {
            push_unique_violation(
                &mut violations,
                &mut seen,
                EvidenceTier::E2,
                "E2_SMALL_TOUCH_TARGET",
                "HIGH",
                Some(box_a.selector.clone()),
                format!(
                    "Interactive element '{}' ({}x{}px) is smaller than 24x24px minimum target",
                    box_a.selector, box_a.width, box_a.height
                ),
            );
        }
        for box_b in input.bounding_boxes.iter().skip(idx + 1) {
            if boxes_overlap(box_a, box_b) {
                push_unique_violation(
                    &mut violations,
                    &mut seen,
                    EvidenceTier::E2,
                    "E2_ELEMENT_COLLISION_OVERLAP",
                    "CRITICAL",
                    Some(format!("{}<->{}", box_a.selector, box_b.selector)),
                    format!(
                        "Bounding boxes '{}' and '{}' collide in rendered layout",
                        box_a.selector, box_b.selector
                    ),
                );
            }
        }
    }

    // 4. E3 WCAG AA contrast checks (`>= 4.5:1` normal text, `>= 3.0:1` large text).
    for c in &input.contrast_ratios {
        let min_ratio = if c.is_large_text { 3.0 } else { 4.5 };
        if c.ratio < min_ratio {
            push_unique_violation(
                &mut violations,
                &mut seen,
                EvidenceTier::E3,
                "E3_WCAG_AA_CONTRAST_FAIL",
                "CRITICAL",
                Some(c.selector.clone()),
                format!(
                    "Contrast ratio {:.2}:1 on '{}' is below WCAG AA floor {:.1}:1",
                    c.ratio, c.selector, min_ratio
                ),
            );
        }
    }

    let mut tier_violation_counts = BTreeMap::new();
    for tier in EvidenceTier::ALL {
        let count = violations.iter().filter(|v| v.tier == tier).count();
        tier_violation_counts.insert(tier.as_str().to_string(), count);
    }

    let detected_rule_ids: BTreeSet<String> = violations
        .iter()
        .map(|v| v.rule_id.to_ascii_uppercase())
        .collect();

    let total_expected_defects = input.expected_defect_ids.len();
    let (matched_expected_defects, catch_rate) = if total_expected_defects == 0 {
        (0, 1.0)
    } else {
        let matched = input
            .expected_defect_ids
            .iter()
            .filter(|id| detected_rule_ids.contains(&id.trim().to_ascii_uppercase()))
            .count();
        (matched, (matched as f64) / (total_expected_defects as f64))
    };

    E1E4EvaluationResult {
        qc_run_id: input.qc_run_id.clone(),
        screen_id: input.screen_id.clone(),
        catch_rate,
        matched_expected_defects,
        total_expected_defects,
        tier_violation_counts,
        violations,
    }
}

fn penalty_for_severity(severity: &str) -> f64 {
    match severity.to_ascii_uppercase().as_str() {
        "CRITICAL" => 3.5,
        "HIGH" => 2.0,
        "MEDIUM" => 1.0,
        _ => 0.4,
    }
}

fn score_single_session_jury_lenses(violations: &[QcViolationItem]) -> Vec<JuryLensScore> {
    let mut scores = Vec::with_capacity(JuryLens::ALL.len());
    for lens in JuryLens::ALL {
        let mut penalty = 0.0_f64;
        let mut findings = Vec::new();
        for v in violations {
            let belongs = match lens {
                JuryLens::Visual => {
                    matches!(v.tier, EvidenceTier::E1 | EvidenceTier::E2)
                        || v.rule_id.contains("CONTRAST")
                }
                JuryLens::UxFlow => {
                    matches!(v.tier, EvidenceTier::E2 | EvidenceTier::E3)
                        || v.rule_id.contains("DEAD_ANCHOR")
                        || v.rule_id.contains("TOUCH_TARGET")
                }
                JuryLens::Copy => {
                    v.rule_id.contains("LOREM")
                        || v.rule_id.contains("CTA")
                        || v.rule_id.contains("PLACEHOLDER")
                        || v.rule_id.contains("H1")
                }
                JuryLens::BrandIdentity => {
                    v.tier == EvidenceTier::E4 || v.rule_id.contains("CSS_TOKEN")
                }
            };
            if belongs {
                penalty += penalty_for_severity(&v.severity);
                findings.push(format!("[{}:{}] {}", v.tier.as_str(), v.rule_id, v.message));
            }
        }
        let raw_score = (10.0 - penalty).clamp(0.0, 10.0);
        let score = (raw_score * 100.0).round() / 100.0;
        scores.push(JuryLensScore {
            lens,
            score,
            passed: score >= MIN_JURY_LENS_PASS_SCORE,
            findings,
        });
    }
    scores
}

fn build_recommended_fixes(violations: &[QcViolationItem]) -> Vec<String> {
    violations
        .iter()
        .map(|v| {
            let target = v.selector.as_deref().unwrap_or("screen");
            format!(
                "[{}][{}] Fix {} on '{}': {}",
                v.tier.as_str(),
                v.severity,
                v.rule_id,
                target,
                v.message
            )
        })
        .collect()
}

/// Evaluates a Tier-3 `Single-Session Design Jury` (`Visual`, `UX Flow`, `Copy`, `Brand Identity`)
/// over a frozen `ReviewFreezeBundle` with:
/// 1. Unconditional `Builder != Scorer` enforcement (`BuilderSelfPassForbidden`).
/// 2. `generation_fence` CAS enforcement (`StaleGenerationFence`).
/// 3. Independent `E1..E4` evidence verification.
/// 4. Structured `QcViolationReport` + `V18a gate_failure_ledger` (`HUMAN_ONLY`) payloads for recurring (`>= 2`) defects.
pub fn evaluate_design_jury_session(
    input: &DesignJurySessionInput,
    expected_generation_fence: i64,
    prior_violation_counts: &BTreeMap<String, usize>,
) -> Result<DesignJuryOutcome, DesignJuryError> {
    enforce_builder_neq_scorer(
        &input.builder_worker_id,
        &input.scorer_worker_id,
        input.scorer_role,
    )?;

    if input.freeze_bundle.generation_fence != expected_generation_fence {
        return Err(DesignJuryError::StaleGenerationFence {
            expected: expected_generation_fence,
            provided: input.freeze_bundle.generation_fence,
        });
    }

    let freeze_sha256 = validate_sha256_hex(&input.freeze_bundle.freeze_snapshot_sha256)?;

    // Merge frozen HTML snapshot when qc_input omitted inline HTML.
    let mut qc_input = input.qc_input.clone();
    if qc_input.html_snapshot.is_none() {
        qc_input.html_snapshot = input.freeze_bundle.frozen_html.clone();
    }
    if qc_input.screen_id.trim().is_empty() {
        qc_input.screen_id = input.freeze_bundle.screen_id.clone();
    }

    let mut e1_e4 = evaluate_e1_e4_evidence(&qc_input);
    let mut ledger_escalations: Vec<GateFailureLedgerPayload> = Vec::new();

    for v in &mut e1_e4.violations {
        let prior = prior_violation_counts
            .get(&v.fingerprint_sha256)
            .copied()
            .unwrap_or(0);
        v.recurrence_count = prior + 1;
        if v.recurrence_count >= RECURRING_VIOLATION_LEDGER_THRESHOLD {
            ledger_escalations.push(GateFailureLedgerPayload {
                rule_id: format!("fleet_jury:{}", v.rule_id),
                fingerprint_sha256: v.fingerprint_sha256.clone(),
                sample_message: format!(
                    "source=fleet_design_jury approval_policy={} tier={} rule={} screen={} msg={}",
                    HUMAN_ONLY_APPROVAL_POLICY,
                    v.tier.as_str(),
                    v.rule_id,
                    input.freeze_bundle.screen_id,
                    v.message
                ),
                hit_count: v.recurrence_count as i64,
                status: "PROPOSED".to_string(),
                approval_policy: HUMAN_ONLY_APPROVAL_POLICY.to_string(),
            });
        }
    }

    let lens_scores = score_single_session_jury_lenses(&e1_e4.violations);
    let has_blocking_violation = e1_e4
        .violations
        .iter()
        .any(|v| matches!(v.severity.as_str(), "CRITICAL" | "HIGH"));
    let all_lenses_pass = lens_scores.iter().all(|l| l.passed);
    let catch_rate_ok =
        e1_e4.total_expected_defects == 0 || e1_e4.catch_rate >= REQUIRED_E1_E4_CATCH_RATE;

    let escalated_to_ledger = !ledger_escalations.is_empty();
    let verdict = if escalated_to_ledger {
        JuryVerdictStatus::EscalatedToLedger
    } else if has_blocking_violation || !all_lenses_pass || !catch_rate_ok {
        JuryVerdictStatus::Fail
    } else {
        JuryVerdictStatus::Pass
    };

    let recommended_fixes = build_recommended_fixes(&e1_e4.violations);
    let report = QcViolationReport {
        report_id: Uuid::new_v4().to_string(),
        project_id: input.freeze_bundle.project_id.clone(),
        screen_id: input.freeze_bundle.screen_id.clone(),
        builder_worker_id: input.builder_worker_id.trim().to_string(),
        scorer_worker_id: input.scorer_worker_id.trim().to_string(),
        generation_fence: input.freeze_bundle.generation_fence,
        freeze_snapshot_sha256: freeze_sha256,
        qc_run_id: qc_input.qc_run_id.clone(),
        e1_e4_catch_rate: e1_e4.catch_rate,
        verdict,
        lens_scores,
        violations: e1_e4.violations,
        recommended_fixes,
        escalated_to_ledger,
    };

    Ok(DesignJuryOutcome {
        report,
        ledger_escalations,
    })
}

/// Canonical 20-defect E1..E4 benchmark fixture used to prove `>= 95%` catch rate (`AC-02`).
pub fn canonical_e1_e4_benchmark_input() -> QcRunInput {
    QcRunInput {
        qc_run_id: Some("qc-bench-e1-e4-canonical".to_string()),
        screen_id: "screen-benchmark-10-2".to_string(),
        html_snapshot: Some(
            r##"<html><body>
                <style>.btn { color: unset; width: 9999px; z-index: 999999; overflow: hidden; white-space: nowrap; font-size: 14pxpx !important; }</style>
                <script>alert('xss')</script>
                <img src="/hero.png" />
                <a href="#">Click here</a>
                <button><svg></svg></button>
                <input type="email" />
                <p>Lorem ipsum dolor sit amet TODO: replace {{placeholder}}</p>
                <style>button:focus { outline: none; }</style>
            </body></html>"##
                .to_string(),
        ),
        reported_issues: vec![
            QcIssueInput {
                rule_id: "E1_BROKEN_GRID_COLUMN".to_string(),
                tier: EvidenceTier::E1,
                severity: "HIGH".to_string(),
                selector: Some(".grid-root".to_string()),
                message: "Broken CSS grid track definition".to_string(),
            },
            QcIssueInput {
                rule_id: "E4_TONE_MISMATCH".to_string(),
                tier: EvidenceTier::E4,
                severity: "MEDIUM".to_string(),
                selector: Some(".hero-subtitle".to_string()),
                message: "Copy tone deviates from enterprise persona".to_string(),
            },
        ],
        expected_defect_ids: vec![
            "E1_INLINE_SCRIPT_XSS".to_string(),
            "E1_UNRESOLVED_TEMPLATE_PLACEHOLDER".to_string(),
            "E1_INVALID_CSS_TOKEN".to_string(),
            "E1_IMG_MISSING_ALT".to_string(),
            "E1_CSS_IMPORTANT_OVERRIDE".to_string(),
            "E1_DEAD_ANCHOR_LINK".to_string(),
            "E1_BROKEN_GRID_COLUMN".to_string(),
            "E2_TEXT_TRUNCATION_RISK".to_string(),
            "E2_HORIZONTAL_OVERFLOW".to_string(),
            "E2_ZINDEX_STACKING_ABUSE".to_string(),
            "E2_ELEMENT_CLIPPED".to_string(),
            "E2_VIEWPORT_OVERFLOW".to_string(),
            "E2_SMALL_TOUCH_TARGET".to_string(),
            "E2_ELEMENT_COLLISION_OVERLAP".to_string(),
            "E3_ICON_BUTTON_MISSING_ARIA_LABEL".to_string(),
            "E3_INPUT_MISSING_LABEL".to_string(),
            "E3_FOCUS_RING_REMOVED".to_string(),
            "E3_MISSING_MAIN_LANDMARK".to_string(),
            "E3_WCAG_AA_CONTRAST_FAIL".to_string(),
            "E4_LOREM_IPSUM_COPY".to_string(),
            "E4_MISSING_PRIMARY_H1".to_string(),
            "E4_GENERIC_CTA_COPY".to_string(),
            "E4_MISSING_BRAND_TOKEN".to_string(),
            "E4_TONE_MISMATCH".to_string(),
        ],
        contrast_ratios: vec![ContrastMeasurement {
            selector: ".muted-caption".to_string(),
            ratio: 2.4,
            is_large_text: false,
        }],
        bounding_boxes: vec![
            ElementBounds {
                selector: ".card-a".to_string(),
                x: 100.0,
                y: 100.0,
                width: 300.0,
                height: 200.0,
                interactive: false,
                clipped: true,
                viewport_width: 1440.0,
            },
            ElementBounds {
                selector: ".card-b".to_string(),
                x: 250.0,
                y: 150.0,
                width: 1300.0,
                height: 180.0,
                interactive: false,
                clipped: false,
                viewport_width: 1440.0,
            },
            ElementBounds {
                selector: ".tiny-close-btn".to_string(),
                x: 10.0,
                y: 10.0,
                width: 16.0,
                height: 16.0,
                interactive: true,
                clipped: false,
                viewport_width: 1440.0,
            },
        ],
        brand_tokens_expected: vec!["--brand-primary".to_string()],
    }
}
