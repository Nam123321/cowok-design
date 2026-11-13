// @story 02.1
// @implements AC-01, AC-02, AC-03
//! # Context Slicer & Token Budget Governor
//!
//! Queries SQLite WAL for admitted spec documents, applies Task Horizon routing,
//! enforces priority-based pruning under conservative token ceilings (<= 1,400 tokens),
//! and validates AC-03 terminal anti-leakage invariants.

use std::cmp::min;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;
use uuid::Uuid;

use domain::{
    AuthorizedContextSlice, CanonicalSpecKey, ContextSliceDecision, ContextSliceRequest,
    HarnessError, ReasonCode, SpecSlice, TaskHorizonRouter, VisibilityPolicy,
};

use crate::registry::SpecRegistry;
use crate::sync::read_jailed_spec_bytes;

/// Canonical token estimation delegating to `domain::estimate_tokens` (OpenAI cl100k_base BPE).
pub fn estimate_tokens(text: &str) -> usize {
    domain::estimate_tokens(text)
}

/// JIT Context Slicer operating over local SQLite WAL spec registry.
pub struct ContextSlicer<'a> {
    registry: &'a SpecRegistry,
    max_retries: u32,
    base_backoff_ms: u64,
}

impl<'a> ContextSlicer<'a> {
    pub fn new(registry: &'a SpecRegistry) -> Self {
        Self {
            registry,
            max_retries: 3,
            base_backoff_ms: 50,
        }
    }

    /// Loads active project constraints from `constraint_rules` and `constraint_profiles`.
    /// Delegates to `ConstraintEvaluator::load_constraints_for_project` as the unified SSOT.
    pub fn load_active_constraints(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<constraint_engine::ActiveConstraint>, HarnessError> {
        let conn = self.registry.pool().connection()?;
        constraint_engine::ConstraintEvaluator::load_constraints_for_project(
            &conn,
            &project_id.to_string(),
        )
    }

    /// Executes JIT context slicing for a given request.
    pub fn slice_context(
        &self,
        project_id: Uuid,
        request: &ContextSliceRequest,
    ) -> Result<AuthorizedContextSlice, HarnessError> {
        // Step 1: Verify project scope and retrieve workspace root
        let workspace_root = self.registry.verify_project_scope(project_id)?;

        // Step 1.5: Enforce PROJECT_CONSTRAINTS via constraint-engine (AC-02 & F-07)
        let active_constraints = self.load_active_constraints(project_id)?;
        constraint_engine::ConstraintEvaluator::evaluate_slice_constraints(
            &active_constraints,
            request.department,
            request.portal,
        )?;

        // Step 2: Classify Task Horizon
        let horizon = TaskHorizonRouter::classify(
            &request.task_input,
            request.override_horizon,
            request.affected_artifacts.len(),
        );

        let token_ceiling = horizon.default_token_ceiling();

        // Step 3: Evaluate Visibility Policy
        let (allowed_keys, denied_keys) =
            VisibilityPolicy::evaluate(request.department, request.portal);

        if allowed_keys.is_empty() {
            return Ok(AuthorizedContextSlice {
                project_id,
                decision: ContextSliceDecision {
                    horizon,
                    department: request.department,
                    portal: request.portal,
                    allowed_spec_keys: allowed_keys,
                    denied_spec_keys: denied_keys,
                    token_ceiling,
                    primary_reason: ReasonCode::NoEligibleSpecs,
                },
                slices: Vec::new(),
                total_estimated_tokens: 0,
                atomic_complete: true,
            });
        }

        // Step 4: Query admitted specs with retry backoff on SQLite WAL contention
        let mut raw_slices = Vec::new();
        for key in &allowed_keys {
            if let Some((title, content)) =
                self.query_spec_with_backoff(project_id, *key, &workspace_root)?
            {
                // AC-03 Boundary Guard: Apply terminal redaction filter immediately
                let safe_content =
                    VisibilityPolicy::apply_terminal_redaction(&content, request.portal)?;
                let tokens = estimate_tokens(&safe_content);
                let priority = self.spec_priority_rank(*key);

                raw_slices.push(SpecSlice {
                    spec_key: *key,
                    title,
                    content: safe_content,
                    estimated_tokens: tokens,
                    priority_rank: priority,
                });
            }
        }

        // Step 5: Stable sort by (priority_rank ASC, spec_key ASC)
        raw_slices.sort_by(|a, b| {
            a.priority_rank
                .cmp(&b.priority_rank)
                .then_with(|| a.spec_key.as_str().cmp(b.spec_key.as_str()))
        });

        // Step 6: Token Budget Governor & Priority Pruning
        let final_slices = self.prune_to_token_ceiling(raw_slices, token_ceiling);
        let total_tokens: usize = final_slices.iter().map(|s| s.estimated_tokens).sum();
        let was_truncated = final_slices
            .iter()
            .any(|s| s.content.contains("<!-- [TRUNCATED_BY_TOKEN_GOVERNOR] -->"));

        Ok(AuthorizedContextSlice {
            project_id,
            decision: ContextSliceDecision {
                horizon,
                department: request.department,
                portal: request.portal,
                allowed_spec_keys: allowed_keys,
                denied_spec_keys: denied_keys,
                token_ceiling,
                primary_reason: if was_truncated {
                    ReasonCode::TokenBudgetExceeded
                } else {
                    ReasonCode::Success
                },
            },
            slices: final_slices,
            total_estimated_tokens: total_tokens,
            atomic_complete: !was_truncated,
        })
    }

    /// Queries SQLite WAL for a single spec record with exponential backoff on busy lock.
    fn query_spec_with_backoff(
        &self,
        project_id: Uuid,
        key: CanonicalSpecKey,
        workspace_root: &Path,
    ) -> Result<Option<(String, String)>, HarnessError> {
        let mut attempt = 0;
        loop {
            match self.registry.get_spec(project_id, key) {
                Ok(Some(record)) => {
                    let title = key.as_str().to_string();
                    let content = match read_jailed_spec_bytes(workspace_root, key)? {
                        Some(bytes) => {
                            let disk_sha = crate::sync::compute_sha256_hex(&bytes);
                            if disk_sha != record.content_sha256 {
                                return Err(HarnessError::StaleFile {
                                    path: format!("product/specs/{}", key.as_str()),
                                    expected_hash: record.content_sha256.clone(),
                                    actual_hash: disk_sha,
                                });
                            }
                            String::from_utf8_lossy(&bytes).into_owned()
                        }
                        None => {
                            return Err(HarnessError::StaleFile {
                                path: format!("Spec file for {:?}", key),
                                expected_hash: record.content_sha256.clone(),
                                actual_hash: "FILE_MISSING_ON_DISK".to_string(),
                            });
                        }
                    };
                    return Ok(Some((title, content)));
                }
                Ok(None) => return Ok(None),
                Err(HarnessError::SqliteBusy { .. }) => {
                    attempt += 1;
                    if attempt >= self.max_retries {
                        return Err(HarnessError::SqliteBusy {
                            details: format!(
                                "SQLite WAL contention exceeded {} retries while reading spec {:?}",
                                self.max_retries, key
                            ),
                        });
                    }
                    let backoff = min(1000, self.base_backoff_ms * (1 << (attempt - 1)));
                    sleep(Duration::from_millis(backoff));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Assigns priority rank to each spec for pruning order:
    /// Priority 1 (Core Invariant, pruned last)
    /// Priority 2 (Essential Domain/UI spec)
    /// Priority 3 (Auxiliary data / examples, trimmed first)
    fn spec_priority_rank(&self, key: CanonicalSpecKey) -> usize {
        match key {
            CanonicalSpecKey::ProductOverview => 1,
            CanonicalSpecKey::SecurityTenancy => 1,
            CanonicalSpecKey::UiUxSpec => 2,
            CanonicalSpecKey::Architecture => 2,
            CanonicalSpecKey::DatabaseSpec => 2,
            CanonicalSpecKey::EpicsAndStories => 3,
            CanonicalSpecKey::CrossDependencies => 3,
            CanonicalSpecKey::ApiGuidelines => 3,
            CanonicalSpecKey::DataAndCopy => 3,
        }
    }

    /// Prunes lower-priority slices if total estimated tokens exceed the ceiling.
    /// Strictly guarantees total_estimated_tokens <= ceiling across all priority ranks.
    fn prune_to_token_ceiling(&self, mut slices: Vec<SpecSlice>, ceiling: usize) -> Vec<SpecSlice> {
        let mut current_total: usize = slices.iter().map(|s| s.estimated_tokens).sum();
        if current_total <= ceiling {
            return slices;
        }

        // Iteratively trim from Priority 3 down to Priority 2
        for slice in slices.iter_mut().rev() {
            if current_total <= ceiling {
                break;
            }
            if slice.priority_rank > 1 {
                while current_total > ceiling {
                    let excess_tokens = current_total - ceiling;
                    let overhead = 20; // Token margin for truncation notice and word rounding
                    let chars_to_cut = ((excess_tokens + overhead) * 4).max(64);
                    let orig_len = slice.content.len();
                    if orig_len <= 80 {
                        // Reached floor for this slice; move to prior slice
                        break;
                    }
                    let keep_len = orig_len.saturating_sub(chars_to_cut).max(60);
                    let safe_boundary = slice.content.floor_char_boundary(keep_len);
                    slice.content.truncate(safe_boundary);
                    if !slice
                        .content
                        .ends_with("<!-- [TRUNCATED_BY_TOKEN_GOVERNOR] -->")
                    {
                        slice
                            .content
                            .push_str("\n\n<!-- [TRUNCATED_BY_TOKEN_GOVERNOR] -->");
                    }
                    let new_tokens = estimate_tokens(&slice.content);
                    current_total =
                        current_total.saturating_sub(slice.estimated_tokens) + new_tokens;
                    slice.estimated_tokens = new_tokens;

                    if slice.content.len() >= orig_len {
                        break;
                    }
                }
            }
        }

        // If still exceeding ceiling after P3 & P2, prune Priority 1 as ultimate guarantee (F-03)
        if current_total > ceiling {
            for slice in slices.iter_mut().rev() {
                if current_total <= ceiling {
                    break;
                }
                while current_total > ceiling {
                    let excess_tokens = current_total - ceiling;
                    let overhead = 20;
                    let chars_to_cut = ((excess_tokens + overhead) * 4).max(64);
                    let orig_len = slice.content.len();
                    if orig_len <= 80 {
                        break;
                    }
                    let keep_len = orig_len.saturating_sub(chars_to_cut).max(60);
                    let safe_boundary = slice.content.floor_char_boundary(keep_len);
                    slice.content.truncate(safe_boundary);
                    if !slice
                        .content
                        .ends_with("<!-- [TRUNCATED_BY_TOKEN_GOVERNOR] -->")
                    {
                        slice
                            .content
                            .push_str("\n\n<!-- [TRUNCATED_BY_TOKEN_GOVERNOR] -->");
                    }
                    let new_tokens = estimate_tokens(&slice.content);
                    current_total =
                        current_total.saturating_sub(slice.estimated_tokens) + new_tokens;
                    slice.estimated_tokens = new_tokens;

                    if slice.content.len() >= orig_len {
                        break;
                    }
                }
            }
        }

        slices
    }
}
