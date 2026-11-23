// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (ADR-006 Rust DecisionCascadeClient ~450 LOC)
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use domain::decision::{
    CascadeStage, ConfidenceKind, DecisionCandidate, DecisionRouteInput, DecisionTier,
    PipelineStageName, RubricPairwiseInput, RubricPairwiseOutcome, StageTraceEntry,
    VerdictShieldOutput, MAX_DECISION_ATTEMPTS, VERDICT_ESCALATE_HUMAN_ADVISOR,
};
use regex::Regex;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::byok_gateway::{
    canonical_provider_id, resolve_and_pin_provider_endpoint, ByokCredential,
};
use crate::normalizer::extract_fenced_or_raw_json;
use crate::posix_sandbox::redact_secrets_from_text;

pub const EGRESS_MAX_PROMPT_BYTES: usize = 4_096;
pub const EGRESS_MAX_CONTEXT_BYTES: usize = 65_536;
pub const MAX_EXTERNAL_RESPONSE_BYTES: usize = 262_144;
const MAX_PRIOR_VERDICTS: usize = 16;
const MAX_CANDIDATES: usize = 32;
const MAX_CANDIDATE_LABEL_LEN: usize = 128;
const MAX_DE_SCORE_BPS: u32 = 10_000;
const DEFAULT_EXTERNAL_TIMEOUT_MS: u64 = 2_500;
const BM25_K1: f64 = 1.5;
const BM25_B: f64 = 0.75;
const BM25_EXACT_SCORE_FLOOR: f64 = 1.15;
const BM25_EXACT_MARGIN_RATIO: f64 = 1.25;

pub type VaultLookupFn = Arc<dyn Fn(&str) -> Vec<ByokCredential> + Send + Sync>;
pub type DrDeHookFn = Arc<dyn Fn(&DrDeHookInput) -> DrDeHookResult + Send + Sync>;

#[derive(Debug, Clone)]
pub struct DrDeHookInput {
    pub project_id: String,
    pub decision_1_verdict: String,
    pub untrusted_artifact_sha256: String,
    pub attempt_number: u8,
}

#[derive(Debug, Clone)]
pub struct DrDeHookResult {
    pub passed: bool,
    pub de_checks_passed: bool,
    pub summary: String,
    pub suggested_verdict: Option<String>,
}

#[derive(Debug, Error)]
pub enum DecisionCascadeError {
    #[error("STALE_FILE_OCC: expected {expected_sha256}, current {current_sha256}")]
    StaleFileConflict {
        expected_sha256: String,
        current_sha256: String,
    },
    #[error("INVALID_REQUEST: {0}")]
    InvalidInput(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressGuardedPayload {
    pub nonce: String,
    pub sanitized_prompt: String,
    pub wrapped_context: String,
    pub prompt_truncated: bool,
    pub context_truncated: bool,
    pub secrets_redacted: bool,
}

fn pii_and_secret_regexes() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
            r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[^\r\n]*",
            r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b",
            r"\b\d{3}-\d{2}-\d{4}\b",
            r"\b(?:\+?\d{1,3}[\s.-]?)?\(?\d{3}\)?[\s.-]\d{3}[\s.-]\d{4}\b",
            r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
            r"\bAIza[0-9A-Za-z_-]{20,}\b",
            r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
            r"\bgh[pousr]_[A-Za-z0-9]{20,}\b",
            r"\bgithub_pat_[A-Za-z0-9_]{20,}\b",
            r"\b[sr]k_(?:live|test)_[A-Za-z0-9]{16,}\b",
            r"\bcwk_(?:sess|human)_[A-Za-z0-9_-]{8,}\b",
            r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b",
            r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{8,}",
            r"\bsk-(?:proj-|ant-|or-v1-)?[A-Za-z0-9_-]{16,}\b",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("valid static egress regex"))
        .collect()
    })
}

fn closing_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)<\s*/\s*untrusted_artifact_bytes").expect("valid closing tag regex")
    })
}

fn opening_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)<\s*untrusted_artifact_bytes").expect("valid opening tag regex")
    })
}

fn truncate_utf8_bytes(input: &str, max_bytes: usize) -> (String, bool) {
    if input.len() <= max_bytes {
        return (input.to_string(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    (input[..end].to_string(), true)
}

fn scrub_secrets_and_pii_with_vault(
    raw: &str,
    vault_secrets: &[Zeroizing<String>],
) -> (String, bool) {
    let base = redact_secrets_from_text(raw, vault_secrets);
    let mut changed = base != raw;
    let mut out = base;
    for re in pii_and_secret_regexes() {
        let next = re.replace_all(&out, "[REDACTED]");
        if next.as_ref() != out.as_str() {
            changed = true;
            out = next.into_owned();
        }
    }
    (out, changed)
}

fn scrub_secrets_and_pii(raw: &str) -> (String, bool) {
    scrub_secrets_and_pii_with_vault(raw, &[])
}

fn escape_untrusted_artifact_tags(raw: &str) -> String {
    let no_close = closing_tag_regex().replace_all(raw, "&lt;/untrusted_artifact_bytes");
    opening_tag_regex()
        .replace_all(&no_close, "&lt;untrusted_artifact_bytes")
        .into_owned()
}

/// Hardens untrusted prompt & context before any external decision API call:
/// strips secrets/PII, escapes opening/closing XML tags before byte truncation to
/// `<= 4 KiB` prompt / `<= 64 KiB` context, and wraps untrusted bytes in a nonce-tagged fence.
pub fn egress_guard(prompt: &str, context: &str) -> EgressGuardedPayload {
    let (scrubbed_prompt, prompt_redacted) = scrub_secrets_and_pii(prompt);
    let (scrubbed_context, context_redacted) = scrub_secrets_and_pii(context);
    let escaped_prompt = escape_untrusted_artifact_tags(&scrubbed_prompt);
    let (capped_prompt, prompt_truncated) =
        truncate_utf8_bytes(&escaped_prompt, EGRESS_MAX_PROMPT_BYTES);

    let nonce = Uuid::new_v4().to_string();
    let open_tag = format!("<untrusted_artifact_bytes nonce=\"{nonce}\">");
    let close_tag = "</untrusted_artifact_bytes>";
    let overhead = open_tag.len() + close_tag.len();
    let escaped_context = escape_untrusted_artifact_tags(&scrubbed_context);
    let inner_budget = EGRESS_MAX_CONTEXT_BYTES.saturating_sub(overhead);
    let (capped_inner, context_truncated) = truncate_utf8_bytes(&escaped_context, inner_budget);
    let wrapped_context = format!("{open_tag}{capped_inner}{close_tag}");

    EgressGuardedPayload {
        nonce,
        sanitized_prompt: capped_prompt,
        wrapped_context,
        prompt_truncated,
        context_truncated,
        secrets_redacted: prompt_redacted || context_redacted,
    }
}

fn is_valid_sha256_hex(raw: &str) -> bool {
    let t = raw.trim();
    t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn verify_stale_file_occ(
    expected_sha256: Option<&str>,
    current_sha256: Option<&str>,
) -> Result<(), DecisionCascadeError> {
    if let Some(exp) = expected_sha256 {
        let e = exp.trim().to_ascii_lowercase();
        if !e.is_empty() {
            let c = current_sha256
                .map(|cur| cur.trim().to_ascii_lowercase())
                .unwrap_or_default();
            if c.is_empty() || e != c {
                return Err(DecisionCascadeError::StaleFileConflict {
                    expected_sha256: e,
                    current_sha256: c,
                });
            }
        }
    }
    Ok(())
}

/// Detects alternating oscillation (`A -> B -> A` where `A != B`) across prior + current verdicts.
pub fn detect_verdict_oscillation(prior_verdicts: &[String], current_verdict: &str) -> bool {
    let mut seq: Vec<String> = prior_verdicts
        .iter()
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    let cur = current_verdict.trim().to_ascii_uppercase();
    if !cur.is_empty() {
        seq.push(cur);
    }
    if seq.len() < 3 {
        return false;
    }
    seq.windows(3).any(|w| w[0] == w[2] && w[0] != w[1])
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|tok| tok.len() >= 2)
        .map(str::to_string)
        .collect()
}

struct InternalBm25Hit {
    label: String,
    score: f64,
    runner_up_score: f64,
}

fn default_routing_corpus() -> Vec<DecisionCandidate> {
    vec![
        DecisionCandidate {
            id: "route-craft".into(),
            label: "ROUTE_CRAFT_SKILL".into(),
            text: "design hero landing page craft typography color palette brand aesthetic layout component".into(),
            de_score_bps: 8_200,
        },
        DecisionCandidate {
            id: "route-opt".into(),
            label: "ROUTE_OPT_SKILL".into(),
            text: "optimize audit contrast wcag accessibility spacing polish refactor lint fix performance".into(),
            de_score_bps: 8_100,
        },
        DecisionCandidate {
            id: "route-direct".into(),
            label: "ROUTE_DIRECT_EDIT".into(),
            text: "surgical patch edit copy text wording rename button label quick tweak update slot".into(),
            de_score_bps: 8_000,
        },
        DecisionCandidate {
            id: "accept-artifact".into(),
            label: "ACCEPT_ARTIFACT".into(),
            text: "accept approve ship pass clean verified zero blocker complete ready merge".into(),
            de_score_bps: 8_500,
        },
        DecisionCandidate {
            id: "retry-de-fix".into(),
            label: "RETRY_WITH_DE_FIX".into(),
            text: "retry repair violation broken token missing contrast error failure lint rule fix".into(),
            de_score_bps: 7_800,
        },
        DecisionCandidate {
            id: "clarify-intent".into(),
            label: "ROUTE_CLARIFY_INTENT".into(),
            text: "ambiguous clarify questions requirements brief direction unknown scope explore".into(),
            de_score_bps: 7_600,
        },
    ]
}

fn rank_local_bm25(query: &str, candidates: &[DecisionCandidate]) -> Option<InternalBm25Hit> {
    if candidates.is_empty() {
        return None;
    }
    let q_tokens = tokenize(query);
    if q_tokens.is_empty() {
        return None;
    }
    let docs: Vec<Vec<String>> = candidates
        .iter()
        .map(|c| tokenize(&format!("{} {}", c.label.replace('_', " "), c.text)))
        .collect();
    let n_docs = docs.len() as f64;
    let avg_dl = (docs.iter().map(Vec::len).sum::<usize>() as f64 / n_docs).max(1.0);

    let mut df: HashMap<&str, usize> = HashMap::new();
    for doc in &docs {
        let unique: HashSet<&str> = doc.iter().map(String::as_str).collect();
        for term in unique {
            *df.entry(term).or_insert(0) += 1;
        }
    }

    let mut scored: Vec<(usize, f64)> = docs
        .iter()
        .enumerate()
        .map(|(idx, doc)| {
            let dl = doc.len() as f64;
            let mut tf_map: HashMap<&str, usize> = HashMap::new();
            for t in doc {
                *tf_map.entry(t.as_str()).or_insert(0) += 1;
            }
            let mut score = 0.0_f64;
            for q in &q_tokens {
                let tf = *tf_map.get(q.as_str()).unwrap_or(&0) as f64;
                if tf > 0.0 {
                    let n_qi = *df.get(q.as_str()).unwrap_or(&0) as f64;
                    let idf = ((n_docs - n_qi + 0.5) / (n_qi + 0.5) + 1.0).ln();
                    let denom = tf + BM25_K1 * (1.0 - BM25_B + BM25_B * (dl / avg_dl));
                    score += idf * ((tf * (BM25_K1 + 1.0)) / denom);
                }
            }
            (idx, score)
        })
        .collect();

    scored.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| {
                candidates[b.0]
                    .de_score_bps
                    .cmp(&candidates[a.0].de_score_bps)
            })
            .then_with(|| candidates[a.0].id.cmp(&candidates[b.0].id))
    });

    let (best_idx, best_score) = scored[0];
    let runner_up = scored.get(1).map(|x| x.1).unwrap_or(0.0);
    Some(InternalBm25Hit {
        label: candidates[best_idx].label.clone(),
        score: best_score,
        runner_up_score: runner_up,
    })
}

/// Deterministic `RubricShield` judge with custom pairwise scorer callback.
/// Runs both `(X, Y)` and `(Y, X)`. On any position-swap disagreement, breaks the tie
/// deterministically using `de_score_bps` (and `id` lexicographical tie-breaker) — never random.
pub fn evaluate_rubric_pairwise_with_judge<F>(
    pair: &RubricPairwiseInput,
    judge: F,
) -> RubricPairwiseOutcome
where
    F: Fn(&DecisionCandidate, &DecisionCandidate, &[String]) -> String,
{
    let fwd_raw = judge(&pair.candidate_x, &pair.candidate_y, &pair.rubric_criteria);
    let swp_raw = judge(&pair.candidate_y, &pair.candidate_x, &pair.rubric_criteria);

    let normalize_id = |raw: &str| -> Option<String> {
        let trimmed = raw.trim();
        if trimmed == pair.candidate_x.id {
            Some(pair.candidate_x.id.clone())
        } else if trimmed == pair.candidate_y.id {
            Some(pair.candidate_y.id.clone())
        } else {
            None
        }
    };
    let forward_opt = normalize_id(&fwd_raw);
    let swapped_opt = normalize_id(&swp_raw);
    let swap_consistent = matches!((&forward_opt, &swapped_opt), (Some(f), Some(s)) if f == s);
    let forward_winner_id = forward_opt.unwrap_or_else(|| "INVALID_JUDGE_OUTPUT".to_string());
    let swapped_winner_id = swapped_opt.unwrap_or_else(|| "INVALID_JUDGE_OUTPUT".to_string());

    if swap_consistent {
        let winner = if forward_winner_id == pair.candidate_x.id {
            &pair.candidate_x
        } else {
            &pair.candidate_y
        };
        return RubricPairwiseOutcome {
            winner_id: winner.id.clone(),
            winner_label: winner.label.clone(),
            forward_winner_id,
            swapped_winner_id,
            swap_consistent: true,
            tie_break_applied: false,
            tie_break_reason: None,
        };
    }

    // Deterministic DE-score tie-break on position-swap disagreement (zero randomness).
    let (winner, reason) = if pair.candidate_x.de_score_bps > pair.candidate_y.de_score_bps {
        (&pair.candidate_x, "DE_SCORE_HIGHER_X")
    } else if pair.candidate_y.de_score_bps > pair.candidate_x.de_score_bps {
        (&pair.candidate_y, "DE_SCORE_HIGHER_Y")
    } else if pair.candidate_x.id <= pair.candidate_y.id {
        (&pair.candidate_x, "DE_SCORE_TIED_LEXICOGRAPHIC_ID")
    } else {
        (&pair.candidate_y, "DE_SCORE_TIED_LEXICOGRAPHIC_ID")
    };

    RubricPairwiseOutcome {
        winner_id: winner.id.clone(),
        winner_label: winner.label.clone(),
        forward_winner_id,
        swapped_winner_id,
        swap_consistent: false,
        tie_break_applied: true,
        tie_break_reason: Some(reason.to_string()),
    }
}

/// Built-in order-invariant BM25 + DE-score pairwise judge for `RubricShield`.
fn default_order_invariant_judge(
    first: &DecisionCandidate,
    second: &DecisionCandidate,
    criteria: &[String],
) -> String {
    let query = criteria.join(" ");
    let score_for = |c: &DecisionCandidate| -> (u64, u32) {
        let hit = rank_local_bm25(&query, std::slice::from_ref(c));
        let bm25_milli = hit.map(|h| (h.score * 1000.0).round() as u64).unwrap_or(0);
        // Combine DE deterministic score (bps) and lexical rubric relevance symmetrically.
        let total = u64::from(c.de_score_bps)
            .saturating_mul(2)
            .saturating_add(bm25_milli);
        (total, c.de_score_bps)
    };
    let s1 = score_for(first);
    let s2 = score_for(second);
    match s1.cmp(&s2) {
        std::cmp::Ordering::Greater => first.id.clone(),
        std::cmp::Ordering::Less => second.id.clone(),
        std::cmp::Ordering::Equal => {
            if first.id <= second.id {
                first.id.clone()
            } else {
                second.id.clone()
            }
        }
    }
}

/// ADR-006 System-1 Decision Cascade Client (~450 LOC, replaces legacy `crates/jev-engine`).
pub struct DecisionCascadeClient {
    vault_lookup: Option<VaultLookupFn>,
    dr_de_hook: DrDeHookFn,
    external_timeout_ms: u64,
    allow_loopback_for_test: bool,
}

impl Default for DecisionCascadeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionCascadeClient {
    pub fn new() -> Self {
        Self {
            vault_lookup: None,
            dr_de_hook: Arc::new(|input| DrDeHookResult {
                passed: true,
                de_checks_passed: true,
                summary: format!(
                    "DR/DE hook completed for verdict '{}' (attempt {})",
                    input.decision_1_verdict, input.attempt_number
                ),
                suggested_verdict: None,
            }),
            external_timeout_ms: DEFAULT_EXTERNAL_TIMEOUT_MS,
            allow_loopback_for_test: false,
        }
    }

    pub fn with_vault_lookup(mut self, lookup: VaultLookupFn) -> Self {
        self.vault_lookup = Some(lookup);
        self
    }

    pub fn with_dr_de_hook(mut self, hook: DrDeHookFn) -> Self {
        self.dr_de_hook = hook;
        self
    }

    pub fn with_external_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.external_timeout_ms = timeout_ms.clamp(200, 10_000);
        self
    }

    pub fn with_allow_loopback_for_test(mut self, allow: bool) -> Self {
        self.allow_loopback_for_test = allow;
        self
    }

    /// Evaluates a `RubricShield` pairwise comparison symmetrically (`(X, Y)` & `(Y, X)`).
    pub fn evaluate_rubric_pairwise(&self, pair: &RubricPairwiseInput) -> RubricPairwiseOutcome {
        evaluate_rubric_pairwise_with_judge(pair, default_order_invariant_judge)
    }

    /// Executes the full 5-Stage Decision Cascade:
    /// `DE pre-filter -> Decision #1 -> DR/DE execution -> DE post-check -> Decision #2`
    /// with 3-Tier Cascade (`LocalFts5Bm25 -> GeminiDecision1/TypesafeJev -> HeuristicFallback`),
    /// `egress_guard`, `VerdictShield` (0% float leak), `RubricShield`, `MAX_ATTEMPTS = 3` breaker,
    /// and `StaleFile` SHA-256 OCC.
    pub async fn route(
        &self,
        tenant_id: &str,
        input: &DecisionRouteInput,
    ) -> Result<VerdictShieldOutput, DecisionCascadeError> {
        let total_start = Instant::now();
        let mut trace = Vec::with_capacity(5);

        // --- Stage 1: DE Pre-Filter (schema, SHA-256, StaleFile OCC, egress_guard) ---
        let s1 = Instant::now();
        validate_route_input(input)?;
        let effective_current_sha = input
            .current_file_sha256
            .as_deref()
            .unwrap_or(&input.untrusted_artifact_sha256);
        verify_stale_file_occ(
            input.stale_file_sha256.as_deref(),
            Some(effective_current_sha),
        )?;
        let non_empty_prior_count = input
            .prior_verdicts
            .iter()
            .filter(|v| !v.trim().is_empty())
            .count();
        let effective_attempt = input.attempt_number.max(
            non_empty_prior_count
                .saturating_add(1)
                .min(usize::from(u8::MAX)) as u8,
        );
        let persisted_attempt = effective_attempt.clamp(1, MAX_DECISION_ATTEMPTS);

        let guarded = egress_guard(&input.prompt, &input.context);
        trace.push(StageTraceEntry {
            stage: PipelineStageName::DePreFilter,
            passed: true,
            summary: format!(
                "OCC verified; egress_guard sealed (nonce={}, redacted={})",
                &guarded.nonce[..8],
                guarded.secrets_redacted
            ),
            duration_ms: s1.elapsed().as_millis() as u64,
        });

        // --- Stage 2: Decision #1 (or RubricShield) via 3-Tier Cascade ---
        let s2 = Instant::now();
        let mut rubric_outcome: Option<RubricPairwiseOutcome> = None;
        let (d1_verdict, mut confidence_kind, mut tier_used) =
            if input.stage == CascadeStage::RubricShield {
                let pair = input.rubric_pair.as_ref().ok_or_else(|| {
                    DecisionCascadeError::InvalidInput(
                        "rubricPair is required when stage is RUBRIC_SHIELD".to_string(),
                    )
                })?;
                let outcome = self.evaluate_rubric_pairwise(pair);
                let label = outcome.winner_label.clone();
                let conf = if outcome.swap_consistent {
                    ConfidenceKind::ExactMatch
                } else {
                    ConfidenceKind::DeterministicFallback
                };
                rubric_outcome = Some(outcome);
                (label, conf, DecisionTier::LocalFts5Bm25)
            } else {
                self.resolve_three_tier(tenant_id, input, &guarded).await
            };
        trace.push(StageTraceEntry {
            stage: if input.stage == CascadeStage::RubricShield {
                PipelineStageName::RubricShield
            } else {
                PipelineStageName::Decision1
            },
            passed: true,
            summary: format!("verdict={} tier={}", d1_verdict, tier_used.as_db_str()),
            duration_ms: s2.elapsed().as_millis() as u64,
        });

        // --- Stage 3: DR/DE Execution Hook ---
        let s3 = Instant::now();
        let hook_res = (self.dr_de_hook)(&DrDeHookInput {
            project_id: input.project_id.clone(),
            decision_1_verdict: d1_verdict.clone(),
            untrusted_artifact_sha256: input.untrusted_artifact_sha256.clone(),
            attempt_number: persisted_attempt,
        });
        trace.push(StageTraceEntry {
            stage: PipelineStageName::DrDeExecution,
            passed: hook_res.passed,
            summary: hook_res.summary.clone(),
            duration_ms: s3.elapsed().as_millis() as u64,
        });

        // --- Stage 4: DE Deterministic Post-Check ---
        let s4 = Instant::now();
        let candidate_verdict = hook_res
            .suggested_verdict
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| d1_verdict.clone());
        let post_ok = hook_res.de_checks_passed && !candidate_verdict.trim().is_empty();
        trace.push(StageTraceEntry {
            stage: PipelineStageName::DePostCheck,
            passed: post_ok,
            summary: if post_ok {
                "Deterministic post-check passed".to_string()
            } else {
                "Deterministic post-check flagged retry/escalation".to_string()
            },
            duration_ms: s4.elapsed().as_millis() as u64,
        });

        // --- Stage 5: Decision #2 + MAX_ATTEMPTS = 3 Oscillation Breaker + VerdictShield ---
        let s5 = Instant::now();
        let effective_verdict = if post_ok {
            candidate_verdict
        } else {
            "RETRY_WITH_DE_FIX".to_string()
        };
        let oscillating = detect_verdict_oscillation(&input.prior_verdicts, &effective_verdict);
        let max_attempts_hit = effective_attempt >= MAX_DECISION_ATTEMPTS;
        let breaker_tripped = oscillating || max_attempts_hit;

        let final_verdict = if breaker_tripped {
            confidence_kind = ConfidenceKind::DeterministicFallback;
            if tier_used == DecisionTier::GeminiDecision1 || tier_used == DecisionTier::TypesafeJev
            {
                tier_used = DecisionTier::HeuristicFallback;
            }
            VERDICT_ESCALATE_HUMAN_ADVISOR.to_string()
        } else if !post_ok {
            confidence_kind = ConfidenceKind::DeterministicFallback;
            effective_verdict
        } else {
            effective_verdict
        };

        trace.push(StageTraceEntry {
            stage: if breaker_tripped {
                PipelineStageName::EscalateHumanAdvisor
            } else {
                PipelineStageName::Decision2
            },
            passed: !breaker_tripped,
            summary: if breaker_tripped {
                format!(
                    "Breaker tripped (attempt={}, oscillating={}) -> {}",
                    effective_attempt, oscillating, VERDICT_ESCALATE_HUMAN_ADVISOR
                )
            } else {
                format!("Sealed final verdict={final_verdict}")
            },
            duration_ms: s5.elapsed().as_millis() as u64,
        });

        Ok(seal_verdict_shield(
            final_verdict,
            confidence_kind,
            tier_used,
            input.stage,
            persisted_attempt,
            breaker_tripped,
            rubric_outcome,
            trace,
            total_start.elapsed().as_millis() as u64,
        ))
    }

    async fn resolve_three_tier(
        &self,
        tenant_id: &str,
        input: &DecisionRouteInput,
        guarded: &EgressGuardedPayload,
    ) -> (String, ConfidenceKind, DecisionTier) {
        let corpus = if input.candidates.is_empty() {
            default_routing_corpus()
        } else {
            input.candidates.clone()
        };
        let combined_query = format!("{} {}", guarded.sanitized_prompt, input.context);
        let bm25_hit = rank_local_bm25(&combined_query, &corpus);

        // Tier 1: Local FTS5 BM25 exact lexical hit (<5ms offline, zero network).
        if !input.prefer_external_tier {
            if let Some(ref hit) = bm25_hit {
                let clear_margin = hit.runner_up_score <= f64::EPSILON
                    || (hit.score / hit.runner_up_score) >= BM25_EXACT_MARGIN_RATIO;
                if hit.score >= BM25_EXACT_SCORE_FLOOR && clear_margin {
                    return (
                        hit.label.clone(),
                        ConfidenceKind::ExactMatch,
                        DecisionTier::LocalFts5Bm25,
                    );
                }
            }
        }

        // Tier 2: Gemini Decision-1 / TypeSafe JEV via VaultStore credentials + SSRF pin.
        if let Some((verdict, tier)) = self
            .try_external_decision_tier(tenant_id, &corpus, guarded)
            .await
        {
            return (verdict, ConfidenceKind::ModelVerdict, tier);
        }

        // Tier 3: Offline / Graceful Fallback (best local BM25 hit or deterministic DE-score top).
        if let Some(hit) = bm25_hit {
            if hit.score > 0.0 {
                return (
                    hit.label,
                    ConfidenceKind::DeterministicFallback,
                    DecisionTier::LocalFts5Bm25,
                );
            }
        }
        let fallback_label = corpus
            .iter()
            .max_by(|a, b| {
                a.de_score_bps
                    .cmp(&b.de_score_bps)
                    .then_with(|| b.id.cmp(&a.id))
            })
            .map(|c| c.label.clone())
            .unwrap_or_else(|| "ROUTE_CLARIFY_INTENT".to_string());
        (
            fallback_label,
            ConfidenceKind::DeterministicFallback,
            DecisionTier::HeuristicFallback,
        )
    }

    async fn try_external_decision_tier(
        &self,
        tenant_id: &str,
        corpus: &[DecisionCandidate],
        guarded: &EgressGuardedPayload,
    ) -> Option<(String, DecisionTier)> {
        let lookup = self.vault_lookup.as_ref()?;
        let creds = lookup(tenant_id);
        if creds.is_empty() {
            return None;
        }
        let allowed_labels: Vec<&str> = corpus.iter().map(|c| c.label.as_str()).collect();
        let vault_secrets: Vec<Zeroizing<String>> = creds
            .iter()
            .filter(|c| !c.api_key.trim().is_empty())
            .map(|c| c.api_key.clone())
            .collect();
        let safe_labels_for_prompt: Vec<String> = corpus
            .iter()
            .map(|c| {
                let (scrubbed, _) = scrub_secrets_and_pii_with_vault(&c.label, &vault_secrets);
                escape_untrusted_artifact_tags(&scrubbed)
            })
            .collect();
        // Prefer Gemini (`gemini-decision-1`), then any other canonical BYOK provider (`TYPESAFE_JEV`).
        let chosen = creds
            .iter()
            .find(|c| canonical_provider_id(&c.provider_id) == "gemini")
            .or_else(|| creds.first())?;

        let provider = canonical_provider_id(&chosen.provider_id);
        let tier = if provider == "gemini" {
            DecisionTier::GeminiDecision1
        } else {
            DecisionTier::TypesafeJev
        };
        let url = resolve_decision_endpoint_url(chosen, &provider)?;
        let (host, pinned_addr) =
            resolve_and_pin_provider_endpoint(&url, &provider, self.allow_loopback_for_test)
                .await
                .ok()?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.external_timeout_ms))
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(&host, &[pinned_addr])
            .build()
            .ok()?;

        let raw_prompt_body = format!(
            "Classify into one discrete verdict_label from {:?}. Return strict JSON {{\"verdict_label\":\"...\"}} with zero floats.\nPrompt: {}\nContext: {}",
            safe_labels_for_prompt, guarded.sanitized_prompt, guarded.wrapped_context
        );
        let prompt_body = redact_secrets_from_text(&raw_prompt_body, &vault_secrets);

        let req_builder = if provider == "gemini" {
            let mut b = client
                .post(&url)
                .header("content-type", "application/json")
                .json(&json!({
                    "contents": [{ "role": "user", "parts": [{ "text": prompt_body }] }],
                    "generationConfig": { "temperature": 0, "responseMimeType": "application/json" }
                }));
            if !chosen.api_key.is_empty() {
                b = b.header("x-goog-api-key", chosen.api_key.as_str());
            }
            b
        } else {
            let model = chosen
                .model_id
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or("typesafe-jev-v1");
            let mut b = client
                .post(&url)
                .header("content-type", "application/json")
                .json(&json!({
                    "model": model,
                    "temperature": 0,
                    "messages": [{ "role": "user", "content": prompt_body }]
                }));
            if !chosen.api_key.is_empty() {
                b = b.header(
                    "authorization",
                    format!("Bearer {}", chosen.api_key.as_str()),
                );
            }
            b
        };

        let resp = req_builder.send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body_text = read_bounded_response_text(resp, MAX_EXTERNAL_RESPONSE_BYTES).await?;
        let parsed_label = extract_discrete_verdict_label(&body_text, &allowed_labels)?;
        Some((parsed_label, tier))
    }
}

fn validate_candidate(c: &DecisionCandidate, context: &str) -> Result<(), DecisionCascadeError> {
    if c.id.trim().is_empty() {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "{context}.id must not be empty"
        )));
    }
    if c.label.trim().is_empty() {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "{context}.label must not be empty"
        )));
    }
    if c.label.len() > MAX_CANDIDATE_LABEL_LEN {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "{context}.label exceeds {MAX_CANDIDATE_LABEL_LEN} bytes"
        )));
    }
    if c.de_score_bps > MAX_DE_SCORE_BPS {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "{context}.deScoreBps must be <= {MAX_DE_SCORE_BPS}"
        )));
    }
    Ok(())
}

fn validate_route_input(input: &DecisionRouteInput) -> Result<(), DecisionCascadeError> {
    if input.project_id.trim().is_empty() {
        return Err(DecisionCascadeError::InvalidInput(
            "projectId must not be empty".to_string(),
        ));
    }
    if !is_valid_sha256_hex(&input.untrusted_artifact_sha256) {
        return Err(DecisionCascadeError::InvalidInput(
            "untrustedArtifactSha256 must be a 64-character hex SHA-256 digest".to_string(),
        ));
    }
    if let Some(ref stale) = input.stale_file_sha256 {
        if !is_valid_sha256_hex(stale) {
            return Err(DecisionCascadeError::InvalidInput(
                "staleFileSha256 must be a 64-character hex SHA-256 digest".to_string(),
            ));
        }
    }
    if let Some(ref current) = input.current_file_sha256 {
        if !is_valid_sha256_hex(current) {
            return Err(DecisionCascadeError::InvalidInput(
                "currentFileSha256 must be a 64-character hex SHA-256 digest".to_string(),
            ));
        }
    }
    if input.attempt_number == 0 || input.attempt_number > MAX_DECISION_ATTEMPTS {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "attemptNumber must be between 1 and {MAX_DECISION_ATTEMPTS}"
        )));
    }
    if input.prior_verdicts.len() > MAX_PRIOR_VERDICTS {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "priorVerdicts exceeds maximum length of {MAX_PRIOR_VERDICTS}"
        )));
    }
    if input.candidates.len() > MAX_CANDIDATES {
        return Err(DecisionCascadeError::InvalidInput(format!(
            "candidates exceeds maximum length of {MAX_CANDIDATES}"
        )));
    }
    for (idx, cand) in input.candidates.iter().enumerate() {
        validate_candidate(cand, &format!("candidates[{idx}]"))?;
    }
    if let Some(ref pair) = input.rubric_pair {
        validate_candidate(&pair.candidate_x, "rubricPair.candidateX")?;
        validate_candidate(&pair.candidate_y, "rubricPair.candidateY")?;
        if pair.candidate_x.id.trim() == pair.candidate_y.id.trim() {
            return Err(DecisionCascadeError::InvalidInput(
                "rubricPair candidateX.id and candidateY.id must be distinct".to_string(),
            ));
        }
    }
    Ok(())
}

async fn read_bounded_response_text(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Option<String> {
    if let Some(cl) = resp.content_length() {
        if cl > max_bytes as u64 {
            return None;
        }
    }
    let mut buf = Vec::with_capacity(max_bytes.min(8_192));
    while let Some(chunk) = resp.chunk().await.ok()? {
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return None;
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf).ok()
}

fn resolve_decision_endpoint_url(cred: &ByokCredential, provider: &str) -> Option<String> {
    if let Some(ref ep) = cred.endpoint_url {
        let trimmed = ep.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    match provider {
        "gemini" => Some(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-decision-1:generateContent"
                .to_string(),
        ),
        "openai" => Some("https://api.openai.com/v1/chat/completions".to_string()),
        "claude" => Some("https://api.anthropic.com/v1/messages".to_string()),
        "openrouter" => Some("https://openrouter.ai/api/v1/chat/completions".to_string()),
        "deepseek" => Some("https://api.deepseek.com/chat/completions".to_string()),
        _ => None,
    }
}

/// Extracts ONLY a discrete allowlisted `verdict_label` from upstream response JSON.
/// All probability/float/logprob fields in the upstream payload are physically discarded.
fn extract_discrete_verdict_label(body: &str, allowed: &[&str]) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(body).ok()?;
    let direct = val
        .get("verdict_label")
        .or_else(|| val.get("verdictLabel"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let nested_text = val
        .pointer("/candidates/0/content/parts/0/text")
        .or_else(|| val.pointer("/choices/0/message/content"))
        .or_else(|| val.pointer("/content/0/text"))
        .and_then(|v| v.as_str());

    let candidate = direct.or_else(|| {
        nested_text.and_then(|txt| {
            extract_fenced_or_raw_json(txt)
                .0
                .and_then(|inner| {
                    inner
                        .get("verdict_label")
                        .or_else(|| inner.get("verdictLabel"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .or_else(|| Some(txt.trim().to_string()))
        })
    })?;

    let normalized = candidate.trim().to_ascii_uppercase();
    if allowed.is_empty() {
        return (!normalized.is_empty()).then_some(normalized);
    }
    allowed
        .iter()
        .find(|lbl| lbl.eq_ignore_ascii_case(&normalized))
        .map(|lbl| (*lbl).to_string())
}

/// `VerdictShield` sealing function: constructs the type-safe float-free `VerdictShieldOutput`.
#[allow(clippy::too_many_arguments)]
pub fn seal_verdict_shield(
    verdict_label: String,
    confidence_kind: ConfidenceKind,
    tier_used: DecisionTier,
    cascade_stage: CascadeStage,
    attempt_number: u8,
    breaker_tripped: bool,
    rubric_outcome: Option<RubricPairwiseOutcome>,
    stage_trace: Vec<StageTraceEntry>,
    latency_ms: u64,
) -> VerdictShieldOutput {
    let rubric_swap_consistent = rubric_outcome.as_ref().map(|r| r.swap_consistent);
    VerdictShieldOutput {
        verdict_label,
        confidence_kind,
        tier_used,
        cascade_stage,
        attempt_number,
        breaker_tripped,
        escalated_to_advisor: breaker_tripped,
        rubric_swap_consistent,
        rubric_outcome,
        egress_guarded: true,
        stage_trace,
        latency_ms,
    }
}
