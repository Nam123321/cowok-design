// @story 02.4
// @implements AC-03, AC-05
use std::collections::BTreeSet;
use std::time::Instant;

use domain::token_budget::count_exact_bpe_tokens;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::locks::{is_valid_ssot_portal, SsotLockError};

/// Zone 1 orchestrator spine for `cowok-design-opt` (strictly `<= 250` cl100k BPE tokens).
pub const COWOK_DESIGN_OPT_SPINE: &str = "\
# cowok-design-opt (Orchestrator Spine)
1. SSOT Law: Load project tokens, portal locks, and registered UI patterns before generating code or layouts.
2. AppShell Lock: Respect active AppShell lease boundaries; never rewrite locked chrome without matching fencingToken.
3. Minimum Floor Discovery: Inspect Tier 1 (rule) -> Tier 2 (implementation guide) -> Tier 3 (examples on demand); enrich deeper edge cases rather than truncating.
4. Token Discipline: Use CSS semantic variables (--bg-surface, --text-primary, --action-primary); zero raw hex or dark: overrides.
5. Plain UX Copy: Surface concise user language without internal engine jargon.";

pub const COWOK_DESIGN_OPT_SPINE_MAX_TOKENS: usize = 250;

pub const CRAFT_SKILL_SLUGS: [&str; 7] = [
    "archify-diagram",
    "dashboard-ux",
    "impeccable-polish",
    "oklch-palette",
    "sketch-monochrome",
    "stageshow-slide",
    "typography",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftSkillSpec {
    pub slug: &'static str,
    pub tier1_rule: &'static str,
    pub tier2_guide: &'static str,
    pub tier3_examples: &'static str,
}

pub const CRAFT_SKILL_SPECS: [CraftSkillSpec; 7] = [
    CraftSkillSpec {
        slug: "archify-diagram",
        tier1_rule: "Rule[archify-diagram]: Enforce directional node hierarchy, >=24px lane spacing, and single-purpose edge labels.",
        tier2_guide: "Guide[archify-diagram]: Group nodes by bounded subsystem, keep bezier crossings minimal, bind node fill/stroke to CSS theme variables, and annotate latency/ownership badges cleanly.",
        tier3_examples: "Example[archify-diagram]: <svg data-diagram='flow'><g class='lane-core'><rect rx='8' stroke='var(--border-subtle)' fill='var(--bg-surface)'/></g></svg>",
    },
    CraftSkillSpec {
        slug: "dashboard-ux",
        tier1_rule: "Rule[dashboard-ux]: Enforce 12-column Bento grid rhythm, KPI scan priority in the top fold, and zero decorative clutter.",
        tier2_guide: "Guide[dashboard-ux]: Pair Metric Line summary KPIs with sortable DataTableView and RightDrawer inspection; ensure responsive collapse at <=768px with unambiguous state pills.",
        tier3_examples: "Example[dashboard-ux]: <div className='bento-grid-12'><MetricLine label='Active Portals' value='6' /><DataTableView stickyHeader /></div>",
    },
    CraftSkillSpec {
        slug: "impeccable-polish",
        tier1_rule: "Rule[impeccable-polish]: Enforce optical alignment, consistent 4px/8px spatial scale, and explicit loading/empty/error states.",
        tier2_guide: "Guide[impeccable-polish]: Verify >=4.5:1 WCAG contrast on interactive surfaces, smooth 150ms cubic-bezier transitions, 1.85px monoline SVG iconography, and non-shifting skeletons.",
        tier3_examples: "Example[impeccable-polish]: <button aria-busy={isSaving} data-ff-id='primary-action'><BrandIcon size={16} strokeWidth={1.85} />Save</button>",
    },
    CraftSkillSpec {
        slug: "oklch-palette",
        tier1_rule: "Rule[oklch-palette]: Enforce perceptually uniform OKLCH lightness steps paired into semantic Light/Dark CSS variables.",
        tier2_guide: "Guide[oklch-palette]: Anchor surface hierarchy by delta-L steps (L=0.98..0.92 light, L=0.14..0.22 dark), constrain chroma for readable body copy, and emit var(--*) tokens.",
        tier3_examples: "Example[oklch-palette]: :root { --bg-surface: oklch(0.985 0.004 250); --action-primary: oklch(0.58 0.19 258); }",
    },
    CraftSkillSpec {
        slug: "sketch-monochrome",
        tier1_rule: "Rule[sketch-monochrome]: Enforce high-contrast monochrome wireframe restraint with hairline 1px borders and single accent focus.",
        tier2_guide: "Guide[sketch-monochrome]: Prioritize structural hierarchy, typography weight, and negative space before color; limit chromatic ink strictly to primary CTA or critical status.",
        tier3_examples: "Example[sketch-monochrome]: <section style={{ border: '1px solid var(--border-subtle)', background: 'var(--bg-surface)' }}>Wireframe Frame</section>",
    },
    CraftSkillSpec {
        slug: "stageshow-slide",
        tier1_rule: "Rule[stageshow-slide]: Enforce 16:9 viewport framing, max 1 primary claim per slide, and >=24px outer safe margins.",
        tier2_guide: "Guide[stageshow-slide]: Use asymmetric 60/40 visual-to-copy split, crisp headline under 9 words, tabular metrics for numeric proof, and keyboard Left/Right navigation support.",
        tier3_examples: "Example[stageshow-slide]: <article aspect='16/9' padding='32px'><h1>Unified Design Lock</h1><p>Six portals synchronized cleanly.</p></article>",
    },
    CraftSkillSpec {
        slug: "typography",
        tier1_rule: "Rule[typography]: Enforce modular type scale (1.250 major third), tabular numerals on metrics, and 55-72ch measure.",
        tier2_guide: "Guide[typography]: Separate display sans from monospace IDs/hashes (`font-variant-numeric: tabular-nums`), keep body line-height 1.5, and avoid weights thinner than 400 on dark surfaces.",
        tier3_examples: "Example[typography]: .metric-num { font-variant-numeric: tabular-nums; letter-spacing: -0.02em; line-height: 1.15; }",
    },
];

pub fn lookup_craft_skill(slug: &str) -> Option<&'static CraftSkillSpec> {
    CRAFT_SKILL_SPECS.iter().find(|spec| spec.slug == slug)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRouteCommand {
    pub project_id: String,
    pub intent_summary: String,
    pub portal_id: String,
    #[serde(default)]
    pub requested_slugs: Option<Vec<String>>,
    #[serde(default = "default_max_spine_tokens")]
    pub max_spine_tokens: usize,
}

fn default_max_spine_tokens() -> usize {
    COWOK_DESIGN_OPT_SPINE_MAX_TOKENS
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRouteOutcome {
    pub portal_id: String,
    pub spine_tokens: usize,
    pub selected_slugs: Vec<String>,
    pub prefix_sha256: String,
    pub zone1_spine: String,
    pub zone2_craft_prefix: String,
    pub zone3_intent_tail: String,
    pub latency_ms: f64,
}

/// Computes length-prefixed SHA-256 over Zone 1 + Zone 2:
/// `SHA256(len_be(zone1) || zone1 || len_be(zone2) || zone2)`
pub fn compute_length_prefixed_prefix_sha256(zone1: &str, zone2: &str) -> String {
    let mut hasher = Sha256::new();
    let z1 = zone1.as_bytes();
    let z2 = zone2.as_bytes();
    hasher.update((z1.len() as u64).to_be_bytes());
    hasher.update(z1);
    hasher.update((z2.len() as u64).to_be_bytes());
    hasher.update(z2);
    format!("{:x}", hasher.finalize())
}

fn infer_slugs_from_intent(portal_id: &str, intent: &str) -> Vec<String> {
    let lower = intent.to_ascii_lowercase();
    let mut inferred = BTreeSet::new();
    if lower.contains("chart")
        || lower.contains("kpi")
        || lower.contains("dashboard")
        || lower.contains("bento")
    {
        inferred.insert("dashboard-ux".to_string());
    }
    if lower.contains("color")
        || lower.contains("palette")
        || lower.contains("theme")
        || lower.contains("oklch")
    {
        inferred.insert("oklch-palette".to_string());
    }
    if lower.contains("font")
        || lower.contains("type")
        || lower.contains("heading")
        || lower.contains("typography")
    {
        inferred.insert("typography".to_string());
    }
    if lower.contains("diagram") || lower.contains("graph") || lower.contains("architecture") {
        inferred.insert("archify-diagram".to_string());
    }
    if lower.contains("slide") || lower.contains("deck") || portal_id == "Presentation" {
        inferred.insert("stageshow-slide".to_string());
    }
    if lower.contains("wireframe") || lower.contains("sketch") || lower.contains("monochrome") {
        inferred.insert("sketch-monochrome".to_string());
    }
    if inferred.is_empty()
        || lower.contains("polish")
        || lower.contains("contrast")
        || lower.contains("a11y")
    {
        inferred.insert("impeccable-polish".to_string());
    }
    inferred.into_iter().collect()
}

/// Dynamic skill descriptor supplied by `UnifiedSkillCatalog` so `POST /api/v1/skills/route`
/// routes arbitrary discovered skills (including VI/EN intents, `data-viz-storyteller`,
/// `dashboard-design`, active plugins, and approved LEARN cards) without hardcoded slug ceilings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSkillEntry {
    pub slug: String,
    pub tier1_summary: String,
    pub tier2_body: String,
}

/// Deterministic 3-Zone Cache-Preserving Skill Router matching `SkillRouteResponseSchema`:
/// - Zone 1 (`cowok-design-opt` spine): strictly `<= max_spine_tokens <= 250` BPE tokens
/// - Zone 2 (selected skills prefix): deduplicated & lexicographically sorted by slug
/// - Zone 3 (volatile intent tail): intent summary, never alters `prefix_sha256`
pub struct CachePreservingSkillRouter;

impl CachePreservingSkillRouter {
    pub fn route(cmd: &SkillRouteCommand) -> Result<SkillRouteOutcome, SsotLockError> {
        Self::route_with_dynamic_catalog(cmd, &[], &[])
    }

    pub fn route_with_dynamic_catalog(
        cmd: &SkillRouteCommand,
        dynamic_entries: &[DynamicSkillEntry],
        auto_selected_slugs: &[String],
    ) -> Result<SkillRouteOutcome, SsotLockError> {
        let start = Instant::now();

        if cmd.project_id.trim().is_empty() || cmd.project_id.len() > 128 {
            return Err(SsotLockError::Validation {
                code: "INVALID_REQUEST",
                message: "projectId must be 1..=128 chars".to_string(),
            });
        }
        if !is_valid_ssot_portal(&cmd.portal_id) {
            return Err(SsotLockError::Validation {
                code: "INVALID_REQUEST",
                message: format!("Unsupported portalId: {}", cmd.portal_id),
            });
        }
        if cmd.intent_summary.trim().is_empty() || cmd.intent_summary.len() > 1000 {
            return Err(SsotLockError::Validation {
                code: "INVALID_REQUEST",
                message: "intentSummary must be 1..=1000 characters".to_string(),
            });
        }
        if cmd.max_spine_tokens == 0 || cmd.max_spine_tokens > COWOK_DESIGN_OPT_SPINE_MAX_TOKENS {
            return Err(SsotLockError::Validation {
                code: "SPINE_TOKEN_BUDGET_EXCEEDED",
                message: format!(
                    "maxSpineTokens must be between 1 and {COWOK_DESIGN_OPT_SPINE_MAX_TOKENS}"
                ),
            });
        }

        let spine_tokens = count_exact_bpe_tokens(COWOK_DESIGN_OPT_SPINE);
        if spine_tokens > cmd.max_spine_tokens {
            return Err(SsotLockError::Validation {
                code: "SPINE_TOKEN_BUDGET_EXCEEDED",
                message: format!(
                    "cowok-design-opt spine ({spine_tokens} tokens) exceeds budget ({})",
                    cmd.max_spine_tokens
                ),
            });
        }

        let has_dynamic = !dynamic_entries.is_empty();
        let candidate_slugs = match &cmd.requested_slugs {
            Some(list) => list.clone(),
            None if !auto_selected_slugs.is_empty() => auto_selected_slugs.to_vec(),
            None => infer_slugs_from_intent(&cmd.portal_id, &cmd.intent_summary),
        };
        let max_slugs = if has_dynamic { 16 } else { 7 };
        if candidate_slugs.len() > max_slugs {
            return Err(SsotLockError::Validation {
                code: "INVALID_REQUEST",
                message: format!("requestedSlugs supports at most {max_slugs} items"),
            });
        }

        let mut sorted_set = BTreeSet::new();
        for slug in &candidate_slugs {
            let trimmed = slug.trim();
            let in_dynamic = dynamic_entries.iter().any(|e| e.slug == trimmed);
            let in_legacy = lookup_craft_skill(trimmed).is_some();
            if !in_dynamic && !in_legacy {
                return Err(SsotLockError::Validation {
                    code: "INVALID_REQUEST",
                    message: format!("Unknown skill slug: {slug}"),
                });
            }
            sorted_set.insert(trimmed.to_string());
        }

        let selected_slugs: Vec<String> = sorted_set.into_iter().collect();
        let mut zone2_sections = Vec::with_capacity(selected_slugs.len());
        for slug in &selected_slugs {
            if let Some(dyn_entry) = dynamic_entries.iter().find(|e| e.slug == *slug) {
                let body_excerpt: String = dyn_entry.tier2_body.trim().chars().take(1200).collect();
                zone2_sections.push(format!(
                    "Rule[{}]: {}\n{}",
                    dyn_entry.slug,
                    dyn_entry.tier1_summary.trim(),
                    body_excerpt
                ));
            } else if let Some(spec) = lookup_craft_skill(slug.as_str()) {
                zone2_sections.push(format!("{}\n{}", spec.tier1_rule, spec.tier2_guide));
            }
        }

        let zone1_spine = COWOK_DESIGN_OPT_SPINE.to_string();
        let zone2_craft_prefix = if zone2_sections.is_empty() {
            String::new()
        } else {
            zone2_sections.join("\n---\n")
        };

        let prefix_sha256 =
            compute_length_prefixed_prefix_sha256(&zone1_spine, &zone2_craft_prefix);

        let zone3_intent_tail = format!(
            "portal={} | projectId={} | intent={}",
            cmd.portal_id,
            cmd.project_id,
            cmd.intent_summary.trim()
        );

        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(SkillRouteOutcome {
            portal_id: cmd.portal_id.clone(),
            spine_tokens,
            selected_slugs,
            prefix_sha256,
            zone1_spine,
            zone2_craft_prefix,
            zone3_intent_tail,
            latency_ms,
        })
    }
}

// ============================================================================
// Story 08.7: 2-Breakpoint Cache-Safe Prompt Compiler & 75% Cache-Safe Compaction
// ============================================================================

/// Default cache-safe compaction trigger threshold (75% of model context window).
pub const CACHE_SAFE_COMPACTION_THRESHOLD_PCT: usize = 75;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoBreakpointPromptInput {
    pub system_identity: String,
    pub tool_schemas_json: String,
    pub tier1_skill_index: String,
    pub design_tokens_sha256: String,
    pub catalog_sha256: String,
    #[serde(default)]
    pub jit_tier2_skills: Vec<(String, String)>,
    #[serde(default)]
    pub jit_tier3_references: Vec<(String, String, String)>,
    #[serde(default)]
    pub conversation_history: Vec<ConversationTurn>,
    pub user_turn: String,
    #[serde(default = "default_context_window_tokens")]
    pub context_window_tokens: usize,
}

fn default_context_window_tokens() -> usize {
    128_000
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledTwoBreakpointPrompt {
    /// Breakpoint 1: Strictly static, byte-invariant across turns when catalog & design hash are unchanged.
    pub zone1_static_prefix: String,
    /// Deterministic SHA-256 of `zone1_static_prefix`.
    pub zone1_prefix_sha256: String,
    /// Breakpoint 2 block: Cache-safe compacted `[CONVERSATION_HISTORY]` block (None when history is empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_breakpoint_block: Option<String>,
    /// Dynamic turn tail AFTER Breakpoint 2: JIT Tier-2 `SKILL.md` + Tier-3 `references/*.md` + `[USER_TURN]` (without repeating history).
    pub zone2_dynamic_tail: String,
    /// Combined Zone 2 payload (Breakpoint 2 history + dynamic turn tail) for single-string callers.
    pub zone2_turn_tail: String,
    pub activated_skills: Vec<String>,
    pub loaded_references: Vec<String>,
    pub compaction_applied: bool,
    pub zone1_tokens: usize,
    pub estimated_total_tokens: usize,
}

/// Cache-Safe History Compaction (`Story 08.7 AC-02`):
/// Preserves the leading turn prefix intact (so KV cache Breakpoint 2 prefix hits remain hot)
/// and compacts overflowing middle turns into a deterministic summary marker while retaining
/// the most recent turns verbatim whenever history exceeds `max_history_tokens`.
pub fn compact_history_cache_safe(
    history: &[ConversationTurn],
    max_history_tokens: usize,
) -> (Vec<ConversationTurn>, bool) {
    let total_tokens: usize = history
        .iter()
        .map(|t| count_exact_bpe_tokens(&t.content).saturating_add(4))
        .sum();
    if total_tokens <= max_history_tokens || history.len() <= 3 {
        return (history.to_vec(), false);
    }

    // Keep the very first turn (anchored prefix) + recent tail turns that fit budget,
    // compacting the middle span into a single deterministic digest turn.
    let head = history[0].clone();
    let head_tokens = count_exact_bpe_tokens(&head.content).saturating_add(4);
    let mut kept_tail: Vec<ConversationTurn> = Vec::new();
    let mut used = head_tokens.saturating_add(48); // reserve for compaction summary turn
    let mut split_idx = history.len();

    for idx in (1..history.len()).rev() {
        let turn_tokens = count_exact_bpe_tokens(&history[idx].content).saturating_add(4);
        if used.saturating_add(turn_tokens) > max_history_tokens && !kept_tail.is_empty() {
            break;
        }
        used = used.saturating_add(turn_tokens);
        kept_tail.push(history[idx].clone());
        split_idx = idx;
    }
    kept_tail.reverse();

    if split_idx <= 1 {
        return (history.to_vec(), false);
    }

    let compacted_slice = &history[1..split_idx];
    let mut summary_parts = Vec::with_capacity(compacted_slice.len());
    for turn in compacted_slice {
        let preview: String = turn
            .content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(80)
            .collect();
        summary_parts.push(format!("[{}: {}]", turn.role, preview));
    }
    let summary_turn = ConversationTurn {
        role: "system".to_string(),
        content: format!(
            "[CACHE_SAFE_COMPACTION: {} earlier turns summarized to stay <= {}% context budget] {}",
            compacted_slice.len(),
            CACHE_SAFE_COMPACTION_THRESHOLD_PCT,
            summary_parts.join(" | ")
        ),
    };

    let mut out = Vec::with_capacity(2 + kept_tail.len());
    out.push(head);
    out.push(summary_turn);
    out.extend(kept_tail);
    (out, true)
}

/// Compiles a 2-Breakpoint Cache-Safe prompt:
/// - **Zone 1 (`zone1_static_prefix`)**: System Identity + Tool Schemas + Tier-1 Skill Index (`<= 100 tok`/skill)
///   + `DESIGN.md`/`tokens.css` hash + `catalog_sha256`. 100% invariant across turns!
/// - **Zone 2 (`zone2_turn_tail`)**: Cache-safe compacted conversation history + JIT Tier-2 `SKILL.md`
///   bodies + lazy Tier-3 `references/*.md` + current `user_turn`.
pub fn compile_two_breakpoint_prompt(
    input: &TwoBreakpointPromptInput,
) -> CompiledTwoBreakpointPrompt {
    let zone1_static_prefix = format!(
        "=== ZONE 1: STATIC CACHED PREFIX ===\n\
         [SYSTEM_IDENTITY]\n{}\n\
         [TOOL_SCHEMAS]\n{}\n\
         [TIER1_SKILL_INDEX catalog_sha256={}]\n{}\n\
         [DESIGN_TOKENS_SHA256]\n{}",
        input.system_identity.trim(),
        input.tool_schemas_json.trim(),
        input.catalog_sha256.trim(),
        input.tier1_skill_index.trim(),
        input.design_tokens_sha256.trim(),
    );

    let mut hasher = Sha256::new();
    hasher.update(zone1_static_prefix.as_bytes());
    let zone1_prefix_sha256 = format!("{:x}", hasher.finalize());
    let zone1_tokens = count_exact_bpe_tokens(&zone1_static_prefix);

    // Build JIT Tier-2 + Tier-3 sections (strictly in Zone 2 Turn Tail!)
    let mut activated_skills = Vec::with_capacity(input.jit_tier2_skills.len());
    let mut tier2_blocks = Vec::with_capacity(input.jit_tier2_skills.len());
    for (skill_id, body) in &input.jit_tier2_skills {
        activated_skills.push(skill_id.clone());
        tier2_blocks.push(format!(
            "### [JIT_TIER2_SKILL id={skill_id}]\n{}",
            body.trim()
        ));
    }

    let mut loaded_references = Vec::with_capacity(input.jit_tier3_references.len());
    let mut tier3_blocks = Vec::with_capacity(input.jit_tier3_references.len());
    for (skill_id, ref_path, content) in &input.jit_tier3_references {
        loaded_references.push(format!("{skill_id}:{ref_path}"));
        tier3_blocks.push(format!(
            "### [JIT_TIER3_REF skill={skill_id} path={ref_path}]\n{}",
            content.trim()
        ));
    }

    let jit_section = [tier2_blocks.join("\n\n"), tier3_blocks.join("\n\n")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let jit_and_turn_tokens = count_exact_bpe_tokens(&jit_section)
        .saturating_add(count_exact_bpe_tokens(&input.user_turn));

    let threshold_budget = input
        .context_window_tokens
        .max(2048)
        .saturating_mul(CACHE_SAFE_COMPACTION_THRESHOLD_PCT)
        / 100;
    let history_budget = threshold_budget
        .saturating_sub(zone1_tokens)
        .saturating_sub(jit_and_turn_tokens)
        .max(256);

    let (compacted_history, compaction_applied) =
        compact_history_cache_safe(&input.conversation_history, history_budget);

    let history_block = compacted_history
        .iter()
        .map(|t| format!("[{}]: {}", t.role, t.content))
        .collect::<Vec<_>>()
        .join("\n");

    let history_breakpoint_block = if history_block.is_empty() {
        None
    } else {
        Some(format!("[CONVERSATION_HISTORY]\n{history_block}"))
    };

    let mut dynamic_parts = Vec::new();
    if !jit_section.is_empty() {
        dynamic_parts.push(format!("[JIT_LOADED_SKILLS_AND_REFERENCES]\n{jit_section}"));
    }
    dynamic_parts.push(format!("[USER_TURN]\n{}", input.user_turn.trim()));
    let zone2_dynamic_tail = dynamic_parts.join("\n\n");

    let zone2_turn_tail = match &history_breakpoint_block {
        Some(hist) => format!("{hist}\n\n{zone2_dynamic_tail}"),
        None => zone2_dynamic_tail.clone(),
    };

    let estimated_total_tokens =
        zone1_tokens.saturating_add(count_exact_bpe_tokens(&zone2_turn_tail));

    CompiledTwoBreakpointPrompt {
        zone1_static_prefix,
        zone1_prefix_sha256,
        history_breakpoint_block,
        zone2_dynamic_tail,
        zone2_turn_tail,
        activated_skills,
        loaded_references,
        compaction_applied,
        zone1_tokens,
        estimated_total_tokens,
    }
}

#[cfg(test)]
mod two_breakpoint_tests {
    use super::*;

    #[test]
    fn zone1_prefix_sha256_stays_100_percent_invariant_across_different_jit_skills_and_turns() {
        let base_system = "You are cowok-host-spine.";
        let base_tools = r#"[{"name":"de-tokens-oklch"}]"#;
        let base_index = "- dashboard-design [upstream_verbatim@v1.0.0]: Calm honest dashboards\n- data-viz-storyteller [design_craft@v2.1.0]: Insight-led charts";
        let design_hash = "a".repeat(64);
        let catalog_hash = "b".repeat(64);

        let turn1 = compile_two_breakpoint_prompt(&TwoBreakpointPromptInput {
            system_identity: base_system.to_string(),
            tool_schemas_json: base_tools.to_string(),
            tier1_skill_index: base_index.to_string(),
            design_tokens_sha256: design_hash.clone(),
            catalog_sha256: catalog_hash.clone(),
            jit_tier2_skills: vec![("data-viz-storyteller".into(), "# Full Tier-2 Body".into())],
            jit_tier3_references: vec![(
                "data-viz-storyteller".into(),
                "references/storytelling-arc.md".into(),
                "# 5 Core Patterns".into(),
            )],
            conversation_history: vec![],
            user_turn: "Tạo biểu đồ doanh thu kể chuyện dữ liệu cho CEO".to_string(),
            context_window_tokens: 128_000,
        });

        let turn2 = compile_two_breakpoint_prompt(&TwoBreakpointPromptInput {
            system_identity: base_system.to_string(),
            tool_schemas_json: base_tools.to_string(),
            tier1_skill_index: base_index.to_string(),
            design_tokens_sha256: design_hash.clone(),
            catalog_sha256: catalog_hash.clone(),
            jit_tier2_skills: vec![("dashboard-design".into(), "# Upstream MIT Body".into())],
            jit_tier3_references: vec![],
            conversation_history: vec![ConversationTurn {
                role: "user".into(),
                content: "Previous turn".into(),
            }],
            user_turn: "Build operational KPI dashboard".to_string(),
            context_window_tokens: 128_000,
        });

        assert_eq!(turn1.zone1_prefix_sha256, turn2.zone1_prefix_sha256);
        assert_eq!(turn1.zone1_static_prefix, turn2.zone1_static_prefix);
        assert_ne!(turn1.zone2_turn_tail, turn2.zone2_turn_tail);
        assert!(turn1.zone2_turn_tail.contains("JIT_TIER3_REF"));
        assert!(!turn1.zone1_static_prefix.contains("Full Tier-2 Body"));
        assert!(turn1.history_breakpoint_block.is_none());
        let turn2_hist = turn2
            .history_breakpoint_block
            .as_deref()
            .expect("non-empty history produces Breakpoint 2 block");
        assert!(turn2_hist.contains("[CONVERSATION_HISTORY]"));
        assert!(turn2_hist.contains("Previous turn"));
        assert!(!turn2.zone2_dynamic_tail.contains("[CONVERSATION_HISTORY]"));
        assert!(turn2.zone2_dynamic_tail.contains("[USER_TURN]"));
    }

    #[test]
    fn compacts_history_at_75_percent_threshold_preserving_head_turn() {
        let mut history = vec![ConversationTurn {
            role: "user".into(),
            content: "Initial project charter".into(),
        }];
        for i in 0..20 {
            history.push(ConversationTurn {
                role: if i % 2 == 0 { "assistant" } else { "user" }.into(),
                content: format!("Turn {i}: {}", "detailed analysis context ".repeat(40)),
            });
        }
        let compiled = compile_two_breakpoint_prompt(&TwoBreakpointPromptInput {
            system_identity: "spine".into(),
            tool_schemas_json: "[]".into(),
            tier1_skill_index: "- skill".into(),
            design_tokens_sha256: "0".repeat(64),
            catalog_sha256: "1".repeat(64),
            jit_tier2_skills: vec![],
            jit_tier3_references: vec![],
            conversation_history: history,
            user_turn: "Final request".into(),
            context_window_tokens: 2048,
        });
        assert!(compiled.compaction_applied);
        assert!(compiled.zone2_turn_tail.contains("Initial project charter"));
        assert!(compiled.zone2_turn_tail.contains("CACHE_SAFE_COMPACTION"));
    }
}
