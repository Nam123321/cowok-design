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
    if lower.contains("chart") || lower.contains("kpi") || lower.contains("dashboard") || lower.contains("bento") {
        inferred.insert("dashboard-ux".to_string());
    }
    if lower.contains("color") || lower.contains("palette") || lower.contains("theme") || lower.contains("oklch") {
        inferred.insert("oklch-palette".to_string());
    }
    if lower.contains("font") || lower.contains("type") || lower.contains("heading") || lower.contains("typography") {
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
    if inferred.is_empty() || lower.contains("polish") || lower.contains("contrast") || lower.contains("a11y") {
        inferred.insert("impeccable-polish".to_string());
    }
    inferred.into_iter().collect()
}

/// Deterministic 3-Zone Cache-Preserving Skill Router matching `SkillRouteResponseSchema`:
/// - Zone 1 (`cowok-design-opt` spine): strictly `<= max_spine_tokens <= 250` BPE tokens
/// - Zone 2 (canonical craft skills prefix): deduplicated & lexicographically sorted by slug
/// - Zone 3 (volatile intent tail): intent summary, never alters `prefix_sha256`
pub struct CachePreservingSkillRouter;

impl CachePreservingSkillRouter {
    pub fn route(cmd: &SkillRouteCommand) -> Result<SkillRouteOutcome, SsotLockError> {
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

        let candidate_slugs = match &cmd.requested_slugs {
            Some(list) => list.clone(),
            None => infer_slugs_from_intent(&cmd.portal_id, &cmd.intent_summary),
        };
        if candidate_slugs.len() > 7 {
            return Err(SsotLockError::Validation {
                code: "INVALID_REQUEST",
                message: "requestedSlugs supports at most 7 items".to_string(),
            });
        }

        let mut sorted_set = BTreeSet::new();
        for slug in &candidate_slugs {
            if lookup_craft_skill(slug.as_str()).is_none() {
                return Err(SsotLockError::Validation {
                    code: "INVALID_REQUEST",
                    message: format!("Unknown craft skill slug: {slug}"),
                });
            }
            sorted_set.insert(slug.clone());
        }

        let selected_slugs: Vec<String> = sorted_set.into_iter().collect();
        let mut zone2_sections = Vec::with_capacity(selected_slugs.len());
        for slug in &selected_slugs {
            if let Some(spec) = lookup_craft_skill(slug.as_str()) {
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
