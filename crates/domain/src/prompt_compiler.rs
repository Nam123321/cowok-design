// @story 02.2
// @implements AC-01, AC-02, AC-03
//! # Deterministic Prompt Compiler
//!
//! Assembles system instructions, security constraints, 9-spec context slices,
//! and user task instructions into a single deterministic prompt payload with
//! bit-level reproducibility, verified SHA-256 digest, and hard token budget enforcement (<= 1,500 tokens).

use crate::error::HarnessError;
use crate::task_horizon::AuthorizedContextSlice;
use crate::token_budget::{
    count_exact_bpe_tokens, estimate_tokens, score_relevance, PriorityTier, PromptSection,
    TokenBudgetGovernor, TOKEN_BUDGET_CEILING,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "ts")]
use ts_rs::TS;

/// Input for prompt compilation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct CompilationInput {
    pub project_id: String,
    pub system_instruction: String,
    pub project_constraints: Vec<String>,
    pub context_slice: Option<AuthorizedContextSlice>,
    pub task_instructions: String,
    pub optional_components: Vec<String>,
}

/// Output payload of the deterministic prompt compilation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct CompiledPromptPayload {
    pub compiled_prompt: String,
    pub token_count: usize,
    pub token_ceiling: usize,
    pub sha256_digest: String,
    pub pruned_sections: Vec<String>,
    pub compressed_sections: Vec<String>,
}

/// Deterministic Prompt Compiler with embedded Token Budget Governor
#[derive(Debug, Clone)]
pub struct DeterministicPromptCompiler {
    governor: TokenBudgetGovernor,
}

impl Default for DeterministicPromptCompiler {
    fn default() -> Self {
        Self::new(TOKEN_BUDGET_CEILING)
    }
}

impl DeterministicPromptCompiler {
    pub fn new(ceiling: usize) -> Self {
        // F-01: Hard-clamp token ceiling to <= 1,500 tokens as mandated by Story 02.2
        let safe_ceiling = ceiling.min(TOKEN_BUDGET_CEILING);
        Self {
            governor: TokenBudgetGovernor::new(safe_ceiling),
        }
    }

    pub fn ceiling(&self) -> usize {
        self.governor.ceiling()
    }

    /// Compiles prompt deterministically, enforcing bit-level reproducibility
    /// and strict <= 1,500 token budget.
    pub fn compile(
        &self,
        mut input: CompilationInput,
    ) -> Result<CompiledPromptPayload, HarnessError> {
        let mut sections: Vec<PromptSection> = Vec::new();

        // 1. System Instruction (P0 - Invariant)
        let sys_content = input.system_instruction.trim().to_string();
        if !sys_content.is_empty() {
            sections.push(PromptSection::new(
                "p0-system",
                PriorityTier::P0Invariant,
                "SYSTEM INSTRUCTIONS",
                format!("# SYSTEM INSTRUCTIONS\n{}", sys_content),
            ));
        }

        // 2. Project Constraints (P0 - Invariant)
        input.project_constraints.sort();
        input.project_constraints.dedup();
        if !input.project_constraints.is_empty() {
            let mut constraint_lines = Vec::new();
            for c in &input.project_constraints {
                let trimmed = c.trim();
                if !trimmed.is_empty() {
                    constraint_lines.push(format!("- [CONSTRAINT] {}", trimmed));
                }
            }
            if !constraint_lines.is_empty() {
                sections.push(PromptSection::new(
                    "p0-constraints",
                    PriorityTier::P0Invariant,
                    "PROJECT CONSTRAINTS",
                    format!("# PROJECT CONSTRAINTS\n{}", constraint_lines.join("\n")),
                ));
            }
        }

        // 3. 9-Spec Context Slices (P1 / P3)
        if let Some(slice) = input.context_slice {
            let mut sorted_slices = slice.slices;
            // Deterministic sort: by priority_rank first, then spec_key canonical string
            sorted_slices.sort_by(|a, b| {
                a.priority_rank
                    .cmp(&b.priority_rank)
                    .then_with(|| a.spec_key.as_str().cmp(b.spec_key.as_str()))
            });

            for spec in sorted_slices {
                let spec_key_str = spec.spec_key.as_str();
                let priority = if spec.priority_rank <= 3 {
                    PriorityTier::P1TaskCore
                } else {
                    PriorityTier::P3Supplementary
                };

                let spec_text = format!(
                    "## Spec: {} - {}\n{}",
                    spec_key_str,
                    spec.title.trim(),
                    spec.content.trim()
                );

                sections.push(PromptSection::new(
                    format!("spec-{}", spec_key_str.to_lowercase()),
                    priority,
                    format!("SPEC: {}", spec_key_str),
                    spec_text,
                ));
            }
        }

        // 4. Task Instructions (P1 - Core Task)
        let task_content = input.task_instructions.trim().to_string();
        if !task_content.is_empty() {
            sections.push(PromptSection::new(
                "p1-task",
                PriorityTier::P1TaskCore,
                "TASK INSTRUCTIONS",
                format!("# TASK INSTRUCTIONS\n{}", task_content),
            ));
        }

        // 5. Optional Components / UI Elements (P2 - Optional UI) scored by semantic relevance (F-03)
        let mut scored_components: Vec<(usize, String)> = input
            .optional_components
            .into_iter()
            .map(|c| (score_relevance(&task_content, &c), c))
            .collect();

        // Sort descending by relevance score, then ascending by component name for determinism
        scored_components.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored_components.dedup_by(|a, b| a.1 == b.1);

        if !scored_components.is_empty() {
            let mut comp_lines = Vec::new();
            for (score, comp) in &scored_components {
                let trimmed = comp.trim();
                if !trimmed.is_empty() {
                    comp_lines.push(format!("- [COMPONENT relevance={}] {}", score, trimmed));
                }
            }
            if !comp_lines.is_empty() {
                sections.push(PromptSection::new(
                    "p2-components",
                    PriorityTier::P2OptionalUi,
                    "OPTIONAL COMPONENTS",
                    format!("# OPTIONAL COMPONENTS\n{}", comp_lines.join("\n")),
                ));
            }
        }

        // P0 Invariant check: compute P0 assembled text and token count upfront
        let p0_parts: Vec<&str> = sections
            .iter()
            .filter(|s| s.priority == PriorityTier::P0Invariant)
            .map(|s| s.content.as_str())
            .filter(|c| !c.is_empty())
            .collect();
        let p0_joined = p0_parts.join("\n\n");
        let p0_tokens = estimate_tokens(&p0_joined);
        if p0_tokens > self.ceiling() {
            return Err(HarnessError::PromptBudgetExceeded {
                tokens: p0_tokens,
                ceiling: self.ceiling(),
            });
        }

        // Enforce Token Budget via Governor
        let governed = self.governor.govern_sections(sections)?;

        // Deterministic Assembly: build prompt body
        let assembled_parts: Vec<&str> = governed
            .sections
            .iter()
            .map(|s| s.content.as_str())
            .filter(|c| !c.is_empty())
            .collect();

        let mut compiled_prompt = assembled_parts.join("\n\n");
        let mut total_tokens = estimate_tokens(&compiled_prompt);

        // F-01 & F-04: Post-assembly token invariant enforcement without violating P0 invariant.
        // In case "\n\n" separators or rounding cause total_tokens > ceiling, safely clamp
        // only non-P0 content, never trimming P0 invariant sections.
        if total_tokens > self.ceiling() {
            let p0_len = p0_joined.chars().count();
            let mut current_chars = compiled_prompt.chars().count();

            while total_tokens > self.ceiling() && current_chars > p0_len {
                let excess = total_tokens - self.ceiling();
                let trim_chars = (excess * 4).max(1);
                let target_chars = current_chars.saturating_sub(trim_chars).max(p0_len);
                compiled_prompt = compiled_prompt.chars().take(target_chars).collect();
                compiled_prompt = compiled_prompt.trim_end().to_string();
                current_chars = compiled_prompt.chars().count();
                total_tokens = estimate_tokens(&compiled_prompt);
                if current_chars <= p0_len {
                    break;
                }
            }

            // If even at P0 alone it exceeds ceiling, fail closed
            if total_tokens > self.ceiling() {
                return Err(HarnessError::PromptBudgetExceeded {
                    tokens: total_tokens,
                    ceiling: self.ceiling(),
                });
            }
        }

        // Final exact BPE token verification before releasing payload
        let verified_bpe_tokens = count_exact_bpe_tokens(&compiled_prompt);
        if verified_bpe_tokens > self.ceiling() {
            return Err(HarnessError::PromptBudgetExceeded {
                tokens: verified_bpe_tokens,
                ceiling: self.ceiling(),
            });
        }

        // Compute SHA-256 digest of compiled prompt string
        let mut hasher = Sha256::new();
        hasher.update(compiled_prompt.as_bytes());
        let digest_bytes = hasher.finalize();
        let sha256_digest = format!("{:x}", digest_bytes);

        Ok(CompiledPromptPayload {
            compiled_prompt,
            token_count: verified_bpe_tokens,
            token_ceiling: self.ceiling(),
            sha256_digest,
            pruned_sections: governed.pruned_section_ids,
            compressed_sections: governed.compressed_section_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_key::CanonicalSpecKey;
    use crate::task_horizon::{
        ContextSliceDecision, Department, Portal, ReasonCode, SpecSlice, TaskHorizon,
    };
    use uuid::Uuid;

    #[test]
    fn ut_01_compilation_is_deterministic_and_reproducible() {
        let compiler = DeterministicPromptCompiler::default();

        let make_input = || CompilationInput {
            project_id: "prj_test_01".into(),
            system_instruction: "You are the Cowok-Design AI assistant.".into(),
            project_constraints: vec![
                "No Node Buffer in browser code".into(),
                "Embedded SQLite WAL only".into(),
            ],
            context_slice: Some(AuthorizedContextSlice {
                project_id: Uuid::new_v4(),
                decision: ContextSliceDecision {
                    horizon: TaskHorizon::Mid,
                    department: Department::ProductUi,
                    portal: Portal::TenantAdmin,
                    allowed_spec_keys: vec![
                        CanonicalSpecKey::DatabaseSpec,
                        CanonicalSpecKey::Architecture,
                    ],
                    denied_spec_keys: vec![],
                    token_ceiling: 1400,
                    primary_reason: ReasonCode::Success,
                },
                slices: vec![
                    SpecSlice {
                        spec_key: CanonicalSpecKey::DatabaseSpec,
                        title: "Database SQLite WAL spec".into(),
                        content: "CREATE TABLE PROJECTS...".into(),
                        estimated_tokens: 150,
                        priority_rank: 1,
                    },
                    SpecSlice {
                        spec_key: CanonicalSpecKey::Architecture,
                        title: "Core Architecture spec".into(),
                        content: "Rust Daemon Core Layer 0...".into(),
                        estimated_tokens: 200,
                        priority_rank: 2,
                    },
                ],
                total_estimated_tokens: 350,
                atomic_complete: true,
            }),
            task_instructions: "Build Token Budget Governor for Epic 02".into(),
            optional_components: vec!["MetricLine".into(), "DataTableView".into()],
        };

        let result1 = compiler.compile(make_input()).unwrap();
        let result2 = compiler.compile(make_input()).unwrap();

        assert_eq!(result1.compiled_prompt, result2.compiled_prompt);
        assert_eq!(result1.sha256_digest, result2.sha256_digest);
        assert_eq!(result1.token_count, result2.token_count);
        assert!(result1.token_count <= 1500);
    }

    #[test]
    fn ut_02_preserves_determinism_regardless_of_slice_insertion_order() {
        let compiler = DeterministicPromptCompiler::default();
        let pid = Uuid::new_v4();

        let s1 = SpecSlice {
            spec_key: CanonicalSpecKey::DatabaseSpec,
            title: "Database Spec".into(),
            content: "CREATE TABLE...".into(),
            estimated_tokens: 50,
            priority_rank: 1,
        };
        let s2 = SpecSlice {
            spec_key: CanonicalSpecKey::Architecture,
            title: "Architecture Spec".into(),
            content: "Layered Daemon...".into(),
            estimated_tokens: 50,
            priority_rank: 2,
        };

        let input1 = CompilationInput {
            project_id: "prj_01".into(),
            system_instruction: "Sys".into(),
            project_constraints: vec!["B".into(), "A".into()],
            context_slice: Some(AuthorizedContextSlice {
                project_id: pid,
                decision: ContextSliceDecision {
                    horizon: TaskHorizon::Short,
                    department: Department::ProductUi,
                    portal: Portal::TenantAdmin,
                    allowed_spec_keys: vec![
                        CanonicalSpecKey::DatabaseSpec,
                        CanonicalSpecKey::Architecture,
                    ],
                    denied_spec_keys: vec![],
                    token_ceiling: 1400,
                    primary_reason: ReasonCode::Success,
                },
                slices: vec![s1.clone(), s2.clone()],
                total_estimated_tokens: 100,
                atomic_complete: true,
            }),
            task_instructions: "Task".into(),
            optional_components: vec!["Y".into(), "X".into()],
        };

        let mut input2 = input1.clone();
        // Scramble slice and constraints order in input2
        input2.project_constraints = vec!["A".into(), "B".into()];
        input2.optional_components = vec!["X".into(), "Y".into()];
        if let Some(ref mut slice) = input2.context_slice {
            slice.slices = vec![s2, s1];
        }

        let res1 = compiler.compile(input1).unwrap();
        let res2 = compiler.compile(input2).unwrap();

        assert_eq!(res1.compiled_prompt, res2.compiled_prompt);
        assert_eq!(res1.sha256_digest, res2.sha256_digest);
    }

    #[test]
    fn ut_03_enforces_budget_ceiling_on_massive_inputs() {
        let compiler = DeterministicPromptCompiler::new(200);

        let input = CompilationInput {
            project_id: "prj_large".into(),
            system_instruction: "Standard system instructions".into(),
            project_constraints: vec!["Constraint 1".into(), "Constraint 2".into()],
            context_slice: Some(AuthorizedContextSlice {
                project_id: Uuid::new_v4(),
                decision: ContextSliceDecision {
                    horizon: TaskHorizon::Long,
                    department: Department::ProductUi,
                    portal: Portal::TenantAdmin,
                    allowed_spec_keys: vec![CanonicalSpecKey::DatabaseSpec],
                    denied_spec_keys: vec![],
                    token_ceiling: 1400,
                    primary_reason: ReasonCode::Success,
                },
                slices: vec![SpecSlice {
                    spec_key: CanonicalSpecKey::DatabaseSpec,
                    title: "DB Spec".into(),
                    content: "Long content ".repeat(80),
                    estimated_tokens: 800,
                    priority_rank: 5, // Supplementary
                }],
                total_estimated_tokens: 800,
                atomic_complete: true,
            }),
            task_instructions: "User task instruction ".repeat(50),
            optional_components: (1..20).map(|i| format!("Component_{}", i)).collect(),
        };

        let res = compiler.compile(input).unwrap();
        assert!(res.token_count <= 200);
        assert!(!res.pruned_sections.is_empty() || !res.compressed_sections.is_empty());
    }

    #[test]
    fn ut_04_compiler_fails_closed_without_corrupting_p0() {
        // F-01 verification: If P0 alone exceeds budget, fail closed with PromptBudgetExceeded, never truncate P0
        let compiler = DeterministicPromptCompiler::new(20);
        let input = CompilationInput {
            project_id: "prj_p0_overflow".into(),
            system_instruction:
                "A very long system instruction that definitely exceeds twenty tokens by itself"
                    .into(),
            project_constraints: vec!["Strict rule 1".into(), "Strict rule 2".into()],
            context_slice: None,
            task_instructions: "Short task".into(),
            optional_components: vec![],
        };

        let err = compiler.compile(input).unwrap_err();
        assert_eq!(err.kind_str(), "PROMPT_BUDGET_EXCEEDED");
    }
}
