// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (ADR-006 Decision Cascade Domain Contracts — 0% Float Leak by Type System)
use serde::{Deserialize, Serialize};

/// Hard attempt ceiling for the Decision Cascade oscillation circuit breaker (`MAX_ATTEMPTS = 3`).
pub const MAX_DECISION_ATTEMPTS: u8 = 3;

/// Canonical human-advisor escalation verdict emitted when the oscillation breaker trips.
pub const VERDICT_ESCALATE_HUMAN_ADVISOR: &str = "ESCALATE_HUMAN_ADVISOR";

/// Stage identifier persisted in `V21 decision_audit_log.cascade_stage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CascadeStage {
    #[serde(rename = "DECISION_1")]
    Decision1,
    #[serde(rename = "DECISION_2")]
    Decision2,
    #[serde(rename = "RUBRIC_SHIELD")]
    RubricShield,
}

impl CascadeStage {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Decision1 => "DECISION_1",
            Self::Decision2 => "DECISION_2",
            Self::RubricShield => "RUBRIC_SHIELD",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "DECISION_1" => Some(Self::Decision1),
            "DECISION_2" => Some(Self::Decision2),
            "RUBRIC_SHIELD" => Some(Self::RubricShield),
            _ => None,
        }
    }
}

/// Decision tier identifier persisted in `V21 decision_audit_log.tier_used`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionTier {
    #[serde(rename = "LOCAL_FTS5_BM25")]
    LocalFts5Bm25,
    #[serde(rename = "GEMINI_DECISION_1")]
    GeminiDecision1,
    #[serde(rename = "TYPESAFE_JEV")]
    TypesafeJev,
    #[serde(rename = "HEURISTIC_FALLBACK")]
    HeuristicFallback,
}

impl DecisionTier {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::LocalFts5Bm25 => "LOCAL_FTS5_BM25",
            Self::GeminiDecision1 => "GEMINI_DECISION_1",
            Self::TypesafeJev => "TYPESAFE_JEV",
            Self::HeuristicFallback => "HEURISTIC_FALLBACK",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "LOCAL_FTS5_BM25" => Some(Self::LocalFts5Bm25),
            "GEMINI_DECISION_1" => Some(Self::GeminiDecision1),
            "TYPESAFE_JEV" => Some(Self::TypesafeJev),
            "HEURISTIC_FALLBACK" => Some(Self::HeuristicFallback),
            _ => None,
        }
    }
}

/// Discrete confidence classification (`2.2. database-spec.md`) — strictly zero floats/logprobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfidenceKind {
    #[serde(rename = "ExactMatch")]
    ExactMatch,
    #[serde(rename = "ModelVerdict")]
    ModelVerdict,
    #[serde(rename = "DeterministicFallback")]
    DeterministicFallback,
}

impl ConfidenceKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::ExactMatch => "ExactMatch",
            Self::ModelVerdict => "ModelVerdict",
            Self::DeterministicFallback => "DeterministicFallback",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim() {
            "ExactMatch" => Some(Self::ExactMatch),
            "ModelVerdict" => Some(Self::ModelVerdict),
            "DeterministicFallback" => Some(Self::DeterministicFallback),
            _ => None,
        }
    }
}

/// Named stages in the 5-stage `DE -> Decision #1 -> DR/DE -> DE -> Decision #2` machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PipelineStageName {
    DePreFilter,
    Decision1,
    DrDeExecution,
    DePostCheck,
    Decision2,
    RubricShield,
    EscalateHumanAdvisor,
}

/// Discrete per-stage execution trace entry (strictly integer timing, zero floats).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageTraceEntry {
    pub stage: PipelineStageName,
    pub passed: bool,
    pub summary: String,
    pub duration_ms: u64,
}

/// Candidate item evaluated during routing or `RubricShield` pairwise comparison.
/// Uses integer basis points (`0..=10_000`) for deterministic DE tie-breaking — zero floats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionCandidate {
    pub id: String,
    pub label: String,
    pub text: String,
    #[serde(default)]
    pub de_score_bps: u32,
}

/// Pairwise input for `RubricShield` `(X, Y)` / `(Y, X)` position-swap verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RubricPairwiseInput {
    pub candidate_x: DecisionCandidate,
    pub candidate_y: DecisionCandidate,
    #[serde(default)]
    pub rubric_criteria: Vec<String>,
}

/// Outcome of `RubricShield` bidirectional `(X, Y)` and `(Y, X)` pairwise evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RubricPairwiseOutcome {
    pub winner_id: String,
    pub winner_label: String,
    pub forward_winner_id: String,
    pub swapped_winner_id: String,
    pub swap_consistent: bool,
    pub tie_break_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tie_break_reason: Option<String>,
}

fn default_attempt_number() -> u8 {
    1
}

/// Canonical request contract for `POST /api/v1/decision/route` and `DecisionCascadeClient`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionRouteInput {
    pub project_id: String,
    pub stage: CascadeStage,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub context: String,
    pub untrusted_artifact_sha256: String,
    #[serde(default = "default_attempt_number")]
    pub attempt_number: u8,
    #[serde(default)]
    pub prior_verdicts: Vec<String>,
    #[serde(default)]
    pub stale_file_sha256: Option<String>,
    #[serde(default)]
    pub current_file_sha256: Option<String>,
    #[serde(default)]
    pub candidates: Vec<DecisionCandidate>,
    #[serde(default)]
    pub rubric_pair: Option<RubricPairwiseInput>,
    #[serde(default)]
    pub prefer_external_tier: bool,
}

/// `VerdictShield` output contract.
/// Physically contains ONLY discrete enums, strings, booleans, and integers (`u8`/`u64`)
/// so Rust's type system guarantees `0%` float or raw logprob leakage to downstream prompts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerdictShieldOutput {
    pub verdict_label: String,
    pub confidence_kind: ConfidenceKind,
    pub tier_used: DecisionTier,
    pub cascade_stage: CascadeStage,
    pub attempt_number: u8,
    pub breaker_tripped: bool,
    pub escalated_to_advisor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubric_swap_consistent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubric_outcome: Option<RubricPairwiseOutcome>,
    pub egress_guarded: bool,
    pub stage_trace: Vec<StageTraceEntry>,
    pub latency_ms: u64,
}

/// Immutable audit row persisted in `V21 decision_audit_log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionAuditRecord {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub cascade_stage: CascadeStage,
    pub tier_used: DecisionTier,
    pub confidence_kind: ConfidenceKind,
    pub verdict_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubric_swap_consistent: Option<bool>,
    pub attempt_number: u8,
    pub breaker_tripped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_file_sha256: Option<String>,
    pub latency_ms: u64,
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_enums_match_v21_sql_check_constraints() {
        assert_eq!(
            serde_json::to_string(&CascadeStage::Decision1).unwrap(),
            "\"DECISION_1\""
        );
        assert_eq!(
            serde_json::to_string(&CascadeStage::Decision2).unwrap(),
            "\"DECISION_2\""
        );
        assert_eq!(
            serde_json::to_string(&CascadeStage::RubricShield).unwrap(),
            "\"RUBRIC_SHIELD\""
        );
        assert_eq!(
            serde_json::to_string(&DecisionTier::LocalFts5Bm25).unwrap(),
            "\"LOCAL_FTS5_BM25\""
        );
        assert_eq!(
            serde_json::to_string(&DecisionTier::GeminiDecision1).unwrap(),
            "\"GEMINI_DECISION_1\""
        );
        assert_eq!(
            serde_json::to_string(&DecisionTier::TypesafeJev).unwrap(),
            "\"TYPESAFE_JEV\""
        );
        assert_eq!(
            serde_json::to_string(&DecisionTier::HeuristicFallback).unwrap(),
            "\"HEURISTIC_FALLBACK\""
        );
        assert_eq!(
            serde_json::to_string(&ConfidenceKind::ExactMatch).unwrap(),
            "\"ExactMatch\""
        );
        assert_eq!(
            serde_json::to_string(&ConfidenceKind::ModelVerdict).unwrap(),
            "\"ModelVerdict\""
        );
        assert_eq!(
            serde_json::to_string(&ConfidenceKind::DeterministicFallback).unwrap(),
            "\"DeterministicFallback\""
        );
    }

    #[test]
    fn decision_route_input_rejects_unknown_fields() {
        let raw = r#"{
            "projectId": "11111111-1111-4111-8111-111111111111",
            "stage": "DECISION_1",
            "untrustedArtifactSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "rawLogprob": -0.021
        }"#;
        let parsed = serde_json::from_str::<DecisionRouteInput>(raw);
        assert!(
            parsed.is_err(),
            "deny_unknown_fields must reject rawLogprob"
        );
    }

    #[test]
    fn verdict_shield_output_serializes_with_zero_floats() {
        let output = VerdictShieldOutput {
            verdict_label: "ROUTE_CRAFT_SKILL".to_string(),
            confidence_kind: ConfidenceKind::ExactMatch,
            tier_used: DecisionTier::LocalFts5Bm25,
            cascade_stage: CascadeStage::Decision1,
            attempt_number: 1,
            breaker_tripped: false,
            escalated_to_advisor: false,
            rubric_swap_consistent: None,
            rubric_outcome: None,
            egress_guarded: true,
            stage_trace: vec![StageTraceEntry {
                stage: PipelineStageName::Decision1,
                passed: true,
                summary: "Exact lexical match".to_string(),
                duration_ms: 1,
            }],
            latency_ms: 2,
        };
        let val = serde_json::to_value(&output).expect("serialize");
        fn assert_no_floats(v: &serde_json::Value) {
            match v {
                serde_json::Value::Number(n) => {
                    assert!(
                        n.is_i64() || n.is_u64(),
                        "VerdictShieldOutput leaked non-integer float: {n}"
                    );
                }
                serde_json::Value::Array(arr) => {
                    for item in arr {
                        assert_no_floats(item);
                    }
                }
                serde_json::Value::Object(map) => {
                    for (_, child) in map {
                        assert_no_floats(child);
                    }
                }
                _ => {}
            }
        }
        assert_no_floats(&val);
    }
}
