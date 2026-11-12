// @story 02.2
// @implements AC-01, AC-02, AC-03
//! # Token Budget Governor (<= 1,500 Input Tokens)
//!
//! Enforces a strict input token limit (ceiling <= 1,500 tokens) using deterministic
//! 4-tier priority truncation, UTF-8 character-boundary safety, and conservative token estimation.

use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

use crate::error::HarnessError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ts")]
use ts_rs::TS;

static CL100K_BPE: OnceLock<Option<CoreBPE>> = OnceLock::new();

/// Resolves standard OpenAI cl100k_base BPE tokenizer singleton.
pub fn get_cl100k_bpe() -> Option<&'static CoreBPE> {
    CL100K_BPE
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
}

/// Hard ceiling for compiled input tokens as mandated by Epic 02
pub const TOKEN_BUDGET_CEILING: usize = 1500;

/// Standard token counter using OpenAI cl100k_base BPE tokenizer.
/// Returns exact token count using standard BPE vocabulary.
pub fn count_exact_bpe_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if let Some(bpe) = get_cl100k_bpe() {
        bpe.encode_ordinary(text).len()
    } else {
        estimate_tokens(text)
    }
}

/// Splits an identifier or string into constituent words, handling CamelCase, PascalCase,
/// snake_case, kebab-case, and non-alphanumeric separators.
pub fn split_identifier_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut prev_char: Option<char> = None;

    for c in s.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
            prev_char = None;
            continue;
        }

        if let Some(prev) = prev_char {
            // Split on lowercase -> uppercase (e.g. dataTable -> data, Table)
            // or uppercase sequence followed by lowercase (e.g. XMLParser -> XML, Parser)
            if (prev.is_lowercase() && c.is_uppercase())
                || (!current.is_empty()
                    && prev.is_uppercase()
                    && c.is_lowercase()
                    && current.len() > 1)
            {
                if prev.is_uppercase() && c.is_lowercase() && current.len() > 1 {
                    let last_char = current.pop().unwrap();
                    words.push(current.to_lowercase());
                    current.clear();
                    current.push(last_char);
                } else {
                    words.push(current.to_lowercase());
                    current.clear();
                }
            }
        }
        current.push(c);
        prev_char = Some(c);
    }
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    words
}

/// Scores semantic relevance between a task instruction and an optional component/icon.
/// Higher score indicates higher relevance to the user's task.
pub fn score_relevance(task: &str, component: &str) -> usize {
    let task_lower = task.to_lowercase();
    let comp_lower = component.to_lowercase();

    let mut score = 0;
    if task_lower.contains(&comp_lower) || comp_lower.contains(&task_lower) {
        score += 20;
    }

    let comp_words = split_identifier_words(component);
    let task_words = split_identifier_words(task);

    for cw in &comp_words {
        if cw.len() < 2 {
            continue;
        }
        if task_lower.contains(cw.as_str()) {
            score += 10;
        }
        for tw in &task_words {
            if tw.len() >= 2 {
                if cw == tw {
                    score += 15;
                } else if cw.contains(tw) || tw.contains(cw) {
                    score += 5;
                }
            }
        }
    }
    score
}

/// Estimates tokens for prompt sections.
/// Uses standard OpenAI cl100k_base BPE tokenizer when available.
/// Fallback is mathematically bounded by UTF-8 byte length (guaranteed upper bound in byte-level BPE).
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    if let Some(bpe) = get_cl100k_bpe() {
        return bpe.encode_ordinary(text).len();
    }
    // Strict mathematical upper bound: in any byte-level BPE tokenizer,
    // the number of tokens is strictly <= text.len() (number of bytes).
    text.len()
}

/// Truncates string to estimated token budget safely at UTF-8 character boundaries.
pub fn truncate_utf8_to_tokens(text: &str, max_tokens: usize, suffix: &str) -> String {
    if max_tokens == 0 {
        return String::new();
    }
    let current_tokens = estimate_tokens(text);
    if current_tokens <= max_tokens {
        return text.to_string();
    }

    let suffix_tokens = estimate_tokens(suffix);
    let target_body_tokens = max_tokens.saturating_sub(suffix_tokens);
    let approx_max_chars = target_body_tokens * 7 / 2; // ~ 3.5 chars per token

    let mut truncated: String = text.chars().take(approx_max_chars).collect();
    // Ensure final estimate with suffix does not exceed max_tokens
    while !truncated.is_empty() && estimate_tokens(&format!("{}{}", truncated, suffix)) > max_tokens
    {
        truncated.pop();
    }

    format!("{}{}", truncated, suffix)
}

/// 4-Tier Priority Classification for Deterministic Pruning
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum PriorityTier {
    /// P0: System prompt, security invariants, and project constraints (NEVER pruned)
    #[serde(rename = "P0_INVARIANT")]
    P0Invariant = 0,
    /// P1: Task instructions and core horizon context (pruned last)
    #[serde(rename = "P1_TASK_CORE")]
    P1TaskCore = 1,
    /// P2: UI mockups, icons, and optional components (compressed to summaries)
    #[serde(rename = "P2_OPTIONAL_UI")]
    P2OptionalUi = 2,
    /// P3: Supplementary references, appendix specs, logs (pruned first)
    #[serde(rename = "P3_SUPPLEMENTARY")]
    P3Supplementary = 3,
}

/// Structured Section of a Prompt candidate for compilation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct PromptSection {
    pub id: String,
    pub priority: PriorityTier,
    pub title: String,
    pub content: String,
}

impl PromptSection {
    pub fn new(
        id: impl Into<String>,
        priority: PriorityTier,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            priority,
            title: title.into(),
            content: content.into(),
        }
    }

    pub fn tokens(&self) -> usize {
        estimate_tokens(&self.content)
    }
}

/// Result of governing the token budget across prompt sections
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct GovernedBudgetResult {
    pub sections: Vec<PromptSection>,
    pub total_tokens: usize,
    pub token_ceiling: usize,
    pub pruned_section_ids: Vec<String>,
    pub compressed_section_ids: Vec<String>,
}

/// Governor enforcing <= 1,500 input tokens
#[derive(Debug, Clone)]
pub struct TokenBudgetGovernor {
    ceiling: usize,
}

impl Default for TokenBudgetGovernor {
    fn default() -> Self {
        Self::new(TOKEN_BUDGET_CEILING)
    }
}

impl TokenBudgetGovernor {
    pub fn new(ceiling: usize) -> Self {
        Self {
            ceiling: ceiling.min(TOKEN_BUDGET_CEILING),
        }
    }

    pub fn ceiling(&self) -> usize {
        self.ceiling
    }

    /// Evaluates and deterministically prunes sections to fit strictly within `ceiling` tokens.
    ///
    /// Multi-pass pruning order:
    /// 1. Pass 1: Prune all P3 sections in reverse order.
    /// 2. Pass 2: Compress P2 sections to concise summaries.
    /// 3. Pass 3: Drop P2 sections completely if still over budget.
    /// 4. Pass 4: Truncate P1 content at safe UTF-8 character boundary.
    /// 5. Invariant Check: If P0 alone exceeds ceiling, fail closed with `PromptBudgetExceeded`.
    pub fn govern_sections(
        &self,
        mut sections: Vec<PromptSection>,
    ) -> Result<GovernedBudgetResult, HarnessError> {
        let mut pruned_section_ids = Vec::new();
        let mut compressed_section_ids = Vec::new();

        let separator_tokens = |secs: &[PromptSection]| -> usize {
            let non_empty = secs.iter().filter(|s| !s.content.trim().is_empty()).count();
            if non_empty > 1 {
                estimate_tokens(&"\n\n".repeat(non_empty - 1))
            } else {
                0
            }
        };

        // Check P0 invariant: P0 alone (plus internal separators) cannot exceed ceiling
        let p0_non_empty = sections
            .iter()
            .filter(|s| s.priority == PriorityTier::P0Invariant && !s.content.trim().is_empty())
            .count();
        let p0_sep = if p0_non_empty > 1 {
            estimate_tokens(&"\n\n".repeat(p0_non_empty - 1))
        } else {
            0
        };
        let p0_tokens: usize = sections
            .iter()
            .filter(|s| s.priority == PriorityTier::P0Invariant)
            .map(|s| s.tokens())
            .sum::<usize>()
            + p0_sep;

        if p0_tokens > self.ceiling {
            return Err(HarnessError::PromptBudgetExceeded {
                tokens: p0_tokens,
                ceiling: self.ceiling,
            });
        }

        // Calculate initial token sum including separators
        let mut current_total: usize =
            sections.iter().map(|s| s.tokens()).sum::<usize>() + separator_tokens(&sections);
        if current_total <= self.ceiling {
            return Ok(GovernedBudgetResult {
                sections,
                total_tokens: current_total,
                token_ceiling: self.ceiling,
                pruned_section_ids,
                compressed_section_ids,
            });
        }

        // Pass 1: Prune P3 sections (Supplementary)
        if current_total > self.ceiling {
            let mut retained = Vec::new();
            for sec in sections {
                if sec.priority == PriorityTier::P3Supplementary {
                    pruned_section_ids.push(sec.id.clone());
                } else {
                    retained.push(sec);
                }
            }
            sections = retained;
            current_total =
                sections.iter().map(|s| s.tokens()).sum::<usize>() + separator_tokens(&sections);
        }

        if current_total <= self.ceiling {
            return Ok(GovernedBudgetResult {
                sections,
                total_tokens: current_total,
                token_ceiling: self.ceiling,
                pruned_section_ids,
                compressed_section_ids,
            });
        }

        // Pass 2: Compress P2 sections (Optional UI / Components) by preserving highest relevance items
        if current_total > self.ceiling {
            for sec in sections.iter_mut() {
                if sec.priority == PriorityTier::P2OptionalUi {
                    let lines: Vec<&str> = sec.content.lines().collect();
                    if lines.len() > 3 {
                        let header = lines[0]; // e.g. "# OPTIONAL COMPONENTS"
                                               // Retain up to 3 top components (sorted by relevance)
                        let retained_items: Vec<&str> =
                            lines.iter().skip(1).take(3).copied().collect();
                        let omitted_count = lines.len().saturating_sub(1 + retained_items.len());
                        sec.content = format!(
                            "{}\n{}\n- [Summary: {} lower-relevance components omitted for token efficiency]",
                            header,
                            retained_items.join("\n"),
                            omitted_count
                        );
                        compressed_section_ids.push(sec.id.clone());
                    }
                }
            }
            current_total =
                sections.iter().map(|s| s.tokens()).sum::<usize>() + separator_tokens(&sections);
        }

        if current_total <= self.ceiling {
            return Ok(GovernedBudgetResult {
                sections,
                total_tokens: current_total,
                token_ceiling: self.ceiling,
                pruned_section_ids,
                compressed_section_ids,
            });
        }

        // Pass 3: Drop P2 sections completely if compression wasn't enough
        if current_total > self.ceiling {
            let mut retained = Vec::new();
            for sec in sections {
                if sec.priority == PriorityTier::P2OptionalUi {
                    pruned_section_ids.push(sec.id.clone());
                } else {
                    retained.push(sec);
                }
            }
            sections = retained;
            current_total =
                sections.iter().map(|s| s.tokens()).sum::<usize>() + separator_tokens(&sections);
        }

        if current_total <= self.ceiling {
            return Ok(GovernedBudgetResult {
                sections,
                total_tokens: current_total,
                token_ceiling: self.ceiling,
                pruned_section_ids,
                compressed_section_ids,
            });
        }

        // Pass 4: Truncate P1 sections (Task Core)
        // Allocate remaining available tokens to P1 taking separator overhead into account
        let non_p1_tokens: usize = sections
            .iter()
            .filter(|s| s.priority != PriorityTier::P1TaskCore)
            .map(|s| s.tokens())
            .sum();

        let sep_tokens = separator_tokens(&sections);
        let available_for_p1 = self.ceiling.saturating_sub(non_p1_tokens + sep_tokens);
        let p1_count = sections
            .iter()
            .filter(|s| s.priority == PriorityTier::P1TaskCore)
            .count();

        if p1_count > 0 {
            let budget_per_p1 = available_for_p1 / p1_count;
            for sec in sections.iter_mut() {
                if sec.priority == PriorityTier::P1TaskCore && sec.tokens() > budget_per_p1 {
                    sec.content = truncate_utf8_to_tokens(
                        &sec.content,
                        budget_per_p1,
                        "\n... [truncated to fit token ceiling]",
                    );
                    compressed_section_ids.push(sec.id.clone());
                }
            }
            current_total =
                sections.iter().map(|s| s.tokens()).sum::<usize>() + separator_tokens(&sections);
        }

        // Final safety clamp: if still over ceiling by even 1 token, trim the largest non-P0 section
        while current_total > self.ceiling {
            if let Some(sec) = sections
                .iter_mut()
                .filter(|s| s.priority != PriorityTier::P0Invariant && !s.content.is_empty())
                .max_by_key(|s| s.tokens())
            {
                let trimmed_target = sec
                    .tokens()
                    .saturating_sub(current_total - self.ceiling + 1);
                sec.content = truncate_utf8_to_tokens(&sec.content, trimmed_target, "");
                current_total = sections.iter().map(|s| s.tokens()).sum::<usize>()
                    + separator_tokens(&sections);
            } else {
                break;
            }
        }

        // Invariant guarantee: P0 sections must never be pruned. If current_total still > self.ceiling, fail closed.
        if current_total > self.ceiling {
            return Err(HarnessError::PromptBudgetExceeded {
                tokens: current_total,
                ceiling: self.ceiling,
            });
        }

        Ok(GovernedBudgetResult {
            sections,
            total_tokens: current_total,
            token_ceiling: self.ceiling,
            pruned_section_ids,
            compressed_section_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_01_token_estimation_empty_and_normal() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("a"), 1);
        // "hello world" in standard cl100k_base BPE is 2 tokens ("hello", " world")
        assert_eq!(estimate_tokens("hello world"), 2);
    }

    #[test]
    fn ut_02_safe_utf8_truncation_preserves_multibyte_characters() {
        // Vietnamese accents and emoji
        let text =
            "Xin chào các kỹ sư hệ thống Cowok-Design! 🚀 Đây là bài kiểm thử biên tiếng Việt.";
        let truncated = truncate_utf8_to_tokens(text, 5, "...");
        assert!(estimate_tokens(&truncated) <= 5);
        // Ensure no panic on char boundary
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn ut_03_governor_passes_unmodified_when_under_ceiling() {
        let governor = TokenBudgetGovernor::new(1500);
        let sections = vec![
            PromptSection::new("p0", PriorityTier::P0Invariant, "System", "System core"),
            PromptSection::new("p1", PriorityTier::P1TaskCore, "Task", "Do task"),
        ];

        let result = governor.govern_sections(sections).unwrap();
        assert!(result.total_tokens <= 1500);
        assert!(result.pruned_section_ids.is_empty());
        assert!(result.compressed_section_ids.is_empty());
    }

    #[test]
    fn ut_04_governor_prunes_p3_first() {
        let governor = TokenBudgetGovernor::new(50);
        let sections = vec![
            PromptSection::new(
                "p0",
                PriorityTier::P0Invariant,
                "System",
                "System prompt under 20 tokens",
            ),
            PromptSection::new(
                "p3",
                PriorityTier::P3Supplementary,
                "Supplementary",
                "A very long supplementary text that exceeds the ceiling. ".repeat(10),
            ),
        ];

        let result = governor.govern_sections(sections).unwrap();
        assert!(result.total_tokens <= 50);
        assert_eq!(result.pruned_section_ids, vec!["p3"]);
        assert_eq!(result.sections.len(), 1);
        assert_eq!(result.sections[0].id, "p0");
    }

    #[test]
    fn ut_05_governor_compresses_p2_components() {
        let governor = TokenBudgetGovernor::new(60);
        let sections = vec![
            PromptSection::new(
                "p0",
                PriorityTier::P0Invariant,
                "System",
                "System prompt under 20 tokens",
            ),
            PromptSection::new(
                "p2",
                PriorityTier::P2OptionalUi,
                "UI Components",
                "Button 1\nButton 2\nInput 1\nModal 1\nCard 1\nTable 1\nDrawer 1\nMenu 1\n"
                    .repeat(5),
            ),
        ];

        let result = governor.govern_sections(sections).unwrap();
        assert!(result.total_tokens <= 60);
        assert_eq!(result.compressed_section_ids, vec!["p2"]);
        assert!(result.sections[1].content.contains("[Summary:"));
    }

    #[test]
    fn ut_06_governor_fails_closed_when_p0_alone_exceeds_ceiling() {
        let governor = TokenBudgetGovernor::new(10);
        let sections = vec![PromptSection::new(
            "p0",
            PriorityTier::P0Invariant,
            "System",
            "This invariant system section is way too long for a 10 token ceiling",
        )];

        let err = governor.govern_sections(sections).unwrap_err();
        assert_eq!(err.kind_str(), "PROMPT_BUDGET_EXCEEDED");
    }

    #[test]
    fn ut_07_governor_hard_clamps_ceiling_to_1500() {
        // F-02 verification: even if constructed with > 1500, ceiling must clamp to 1500
        let governor = TokenBudgetGovernor::new(2000);
        assert_eq!(governor.ceiling(), 1500);

        let governor_default = TokenBudgetGovernor::default();
        assert_eq!(governor_default.ceiling(), 1500);
    }

    #[test]
    fn ut_08_score_relevance_splits_camel_case_identifiers() {
        // F-03 verification: camelCase / PascalCase names like DataTableView must match "render a data table"
        let task = "render a data table";
        let score_table = score_relevance(task, "DataTableView");
        let score_accordion = score_relevance(task, "Accordion");
        let score_avatar = score_relevance(task, "Avatar");

        assert!(
            score_table > 0,
            "DataTableView should have non-zero relevance score"
        );
        assert!(
            score_table > score_accordion,
            "DataTableView score ({}) should exceed Accordion ({})",
            score_table,
            score_accordion
        );
        assert!(
            score_table > score_avatar,
            "DataTableView score ({}) should exceed Avatar ({})",
            score_table,
            score_avatar
        );
    }

    #[test]
    fn ut_09_estimate_tokens_conservative_upper_bound_on_unicode_and_emojis() {
        // Standard BPE tokens for 750 4-byte Unicode emojis
        let emojis = "🔥".repeat(750);
        let tokens = estimate_tokens(&emojis);
        assert!(
            tokens >= 750,
            "750 emojis should be at least 750 tokens, got {}",
            tokens
        );

        // Standard English text estimation remains accurate and conservative
        let ascii_text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let ascii_tokens = estimate_tokens(&ascii_text);
        assert!(
            ascii_tokens >= 80,
            "English text should have standard token count, got {}",
            ascii_tokens
        );
    }

    #[test]
    fn ut_10_unmerged_ascii_words_never_underestimate() {
        // F-02 verification: unmerged random 12-char ASCII words
        let gibberish = "zqxjkbvwtpmr ".repeat(10);
        let tokens = estimate_tokens(&gibberish);
        assert!(
            tokens >= 50,
            "BPE token count must accurately reflect unmerged sequences: {}",
            tokens
        );
    }
}
