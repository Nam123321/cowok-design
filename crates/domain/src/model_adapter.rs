// @story 02.3
// @implements AC-01, AC-02, AC-03
//! # 3-Profile Model Capability Adapter & Controlled Tool Forging
//!
//! Provides automated adaptation of prompts, tool definitions, schema complexity,
//! and reasoning strategies across 3 distinct model capability tiers:
//! - **Profile A (Frontier Reasoning)**: Deep chain-of-thought, rich nested schemas, ephemeral tools (`cowok-sol` / `specialist`).
//! - **Profile B (Fast Coding)**: Action-oriented, standard flat schemas, balanced speed (`cowok-terra` / `advisor`).
//! - **Profile C (Compact / Motion)**: Micro-tools (<=4 primitive parameters, <=5 tools), minimal token footprint (`cowok-luna` / `micro`).
//!
//! Includes automated graceful degradation (`AC-03`) to Profile B with deterministic micro-tools
//! when tool syntax errors or complexity overflow occur.

use crate::error::HarnessError;
use crate::token_budget::{estimate_tokens, truncate_utf8_to_tokens, TOKEN_BUDGET_CEILING};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

#[cfg(feature = "ts")]
use ts_rs::TS;

/// Three Canonical Model Capability Profiles as defined in FR-27
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ModelProfileTier {
    #[serde(rename = "frontier_reasoning")]
    ProfileA_Frontier,
    #[serde(rename = "fast_coding")]
    ProfileB_FastCoding,
    #[serde(rename = "compact_motion")]
    ProfileC_Compact,
}

impl ModelProfileTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProfileA_Frontier => "frontier_reasoning",
            Self::ProfileB_FastCoding => "fast_coding",
            Self::ProfileC_Compact => "compact_motion",
        }
    }

    pub fn reasoning_effort(&self) -> ReasoningEffort {
        match self {
            Self::ProfileA_Frontier => ReasoningEffort::High,
            Self::ProfileB_FastCoding => ReasoningEffort::Medium,
            Self::ProfileC_Compact => ReasoningEffort::Low,
        }
    }

    pub fn tool_complexity(&self) -> ToolComplexity {
        match self {
            Self::ProfileA_Frontier => ToolComplexity::FullNestedSchema,
            Self::ProfileB_FastCoding => ToolComplexity::FlatOrStandardSchema,
            Self::ProfileC_Compact => ToolComplexity::MicroTools,
        }
    }

    pub fn max_prompt_budget(&self) -> usize {
        match self {
            Self::ProfileA_Frontier => TOKEN_BUDGET_CEILING, // 1500 tokens
            Self::ProfileB_FastCoding => TOKEN_BUDGET_CEILING, // 1500 tokens
            Self::ProfileC_Compact => 1000,                  // Compact ceiling
        }
    }

    pub fn max_context_window(&self) -> usize {
        match self {
            Self::ProfileA_Frontier => 128_000,
            Self::ProfileB_FastCoding => 64_000,
            Self::ProfileC_Compact => 16_000,
        }
    }

    pub fn supports_ephemeral_tool_forging(&self) -> bool {
        matches!(self, Self::ProfileA_Frontier)
    }
}

/// Reasoning effort tier requested or resolved
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ReasoningEffort {
    High,
    Medium,
    Low,
}

/// Tool schema complexity mode supported by the profile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum ToolComplexity {
    FullNestedSchema,
    FlatOrStandardSchema,
    MicroTools,
}

/// Reason why degradation or profile fallback was triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub enum DegradationReason {
    SyntaxErrorRetry,
    ToolComplexityOverflow,
    DisallowedProfileConstraint,
    TokenCeilingExceeded,
}

/// Model Tool Definition with parameters schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct ModelToolDefinition {
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub is_micro_tool: bool,
}

/// Inbound Adaptation Request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct AdaptationRequest {
    pub project_id: Uuid,
    pub requested_profile: ModelProfileTier,
    pub raw_system_prompt: String,
    pub tools: Vec<ModelToolDefinition>,
    #[serde(default)]
    pub disallowed_profiles: Vec<ModelProfileTier>,
    #[serde(default)]
    pub syntax_error_reported: bool,
    #[serde(default)]
    pub max_budget_override: Option<usize>,
}

/// Outbound Adaptation Response
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct AdaptationResponse {
    pub active_profile: ModelProfileTier,
    pub reasoning_effort: ReasoningEffort,
    pub system_prompt: String,
    pub tools: Vec<ModelToolDefinition>,
    pub token_budget: usize,
    pub estimated_tokens: usize,
    pub degraded: bool,
    pub degradation_reason: Option<DegradationReason>,
    pub sha256_digest: String,
}

/// Model Capability Adapter Engine
#[derive(Debug, Clone, Default)]
pub struct ModelCapabilityAdapter;

impl ModelCapabilityAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Adapt prompt, tools, and reasoning settings according to profile and constraints
    pub fn adapt(&self, req: &AdaptationRequest) -> Result<AdaptationResponse, HarnessError> {
        let mut active_profile = req.requested_profile;
        let mut degraded = false;
        let mut degradation_reason: Option<DegradationReason> = None;

        // 1. Check PROJECT_CONSTRAINTS: disallowed profiles
        if req.disallowed_profiles.contains(&active_profile) {
            degraded = true;
            degradation_reason = Some(DegradationReason::DisallowedProfileConstraint);
            active_profile = self.resolve_allowed_profile(&req.disallowed_profiles)?;
        }

        // 2. AC-03 Edge Case: Graceful Degradation on Syntax Error
        // If the model reported a syntax/malformed call error, fall back to Profile B (or Profile C if Profile B is disallowed) with micro-tools
        let force_micro_tools = if req.syntax_error_reported {
            degraded = true;
            degradation_reason = Some(DegradationReason::SyntaxErrorRetry);
            if !req
                .disallowed_profiles
                .contains(&ModelProfileTier::ProfileB_FastCoding)
            {
                active_profile = ModelProfileTier::ProfileB_FastCoding;
            } else if !req
                .disallowed_profiles
                .contains(&ModelProfileTier::ProfileC_Compact)
            {
                active_profile = ModelProfileTier::ProfileC_Compact;
            } else {
                active_profile = self.resolve_allowed_profile(&req.disallowed_profiles)?;
            }
            true
        } else {
            active_profile == ModelProfileTier::ProfileC_Compact
        };

        // 3. Format system prompt prefix according to active profile
        let system_prompt_prefix = match active_profile {
            ModelProfileTier::ProfileA_Frontier => {
                "// PROFILE A: FRONTIER REASONING (DEEP DELIBERATION & ARCHITECTURAL INVARIANTS)\n\
                 // Mandate: Perform rigorous step-by-step reasoning before mutating files. Validate all boundaries.\n\n"
            }
            ModelProfileTier::ProfileB_FastCoding => {
                "// PROFILE B: FAST CODING (DIRECT IMPLEMENTATION & ATOMIC EDITS)\n\
                 // Mandate: Deliver precise code changes directly with minimal overhead.\n\n"
            }
            ModelProfileTier::ProfileC_Compact => {
                "// PROFILE C: COMPACT (MICRO-TOOLS & MINIMAL PAYLOAD)\n"
            }
        };

        let mut system_prompt = format!("{}{}", system_prompt_prefix, req.raw_system_prompt.trim());

        // 4. Transform tools according to profile and micro-tools requirement
        let mut tools = req.tools.clone();
        // Deterministic sort by tool name
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        if force_micro_tools {
            let (micro_tools, was_flattened) = self.convert_to_micro_tools(&tools, 4, 5);
            tools = micro_tools;
            if was_flattened && degradation_reason.is_none() {
                degraded = true;
                degradation_reason = Some(DegradationReason::ToolComplexityOverflow);
            }
        } else if active_profile == ModelProfileTier::ProfileB_FastCoding {
            // Profile B: Flatten nested object schemas (depth > 2)
            tools = self.flatten_deep_schemas(&tools);
        }

        // 5. Enforce token budget ceiling
        let budget_ceiling = req
            .max_budget_override
            .unwrap_or_else(|| active_profile.max_prompt_budget())
            .min(TOKEN_BUDGET_CEILING);

        // Budget calculation loop: Prune non-essential tools if token count exceeds ceiling
        let mut estimated_tokens = self.calculate_total_tokens(&system_prompt, &tools);

        if estimated_tokens > budget_ceiling && !tools.is_empty() {
            degraded = true;
            if degradation_reason.is_none() {
                degradation_reason = Some(DegradationReason::TokenCeilingExceeded);
            }
            while estimated_tokens > budget_ceiling && !tools.is_empty() {
                tools.pop();
                estimated_tokens = self.calculate_total_tokens(&system_prompt, &tools);
            }
        }

        // If system prompt alone exceeds budget ceiling, truncate instructions safely
        if estimated_tokens > budget_ceiling {
            degraded = true;
            if degradation_reason.is_none() {
                degradation_reason = Some(DegradationReason::TokenCeilingExceeded);
            }
            let suffix = if budget_ceiling >= 4 {
                " ... [truncated]"
            } else {
                ""
            };
            system_prompt = truncate_utf8_to_tokens(&system_prompt, budget_ceiling, suffix);
            estimated_tokens = self.calculate_total_tokens(&system_prompt, &tools);

            // Hard clamp guarantee: if still > budget_ceiling (e.g. tiny budget), aggressively prune characters
            while estimated_tokens > budget_ceiling && !system_prompt.is_empty() {
                let current_len = system_prompt.chars().count();
                if current_len == 0 {
                    break;
                }
                let target_len = current_len.saturating_sub(1);
                system_prompt = system_prompt.chars().take(target_len).collect();
                estimated_tokens = self.calculate_total_tokens(&system_prompt, &tools);
            }
        }

        // 6. Compute deterministic SHA-256 digest
        let sha256_digest = self.compute_sha256_digest(
            active_profile,
            &system_prompt,
            &tools,
            active_profile.reasoning_effort(),
            degraded,
        );

        Ok(AdaptationResponse {
            active_profile,
            reasoning_effort: active_profile.reasoning_effort(),
            system_prompt,
            tools,
            token_budget: budget_ceiling,
            estimated_tokens,
            degraded,
            degradation_reason,
            sha256_digest,
        })
    }

    fn resolve_allowed_profile(
        &self,
        disallowed: &[ModelProfileTier],
    ) -> Result<ModelProfileTier, HarnessError> {
        if !disallowed.contains(&ModelProfileTier::ProfileB_FastCoding) {
            Ok(ModelProfileTier::ProfileB_FastCoding)
        } else if !disallowed.contains(&ModelProfileTier::ProfileC_Compact) {
            Ok(ModelProfileTier::ProfileC_Compact)
        } else if !disallowed.contains(&ModelProfileTier::ProfileA_Frontier) {
            Ok(ModelProfileTier::ProfileA_Frontier)
        } else {
            Err(HarnessError::ModelAdaptationFailed {
                reason: "All model profiles are disallowed by project constraints".to_string(),
            })
        }
    }

    /// Convert tools to MicroTools: primitive parameters only, max_params per tool, max_tools limit
    fn convert_to_micro_tools(
        &self,
        tools: &[ModelToolDefinition],
        max_params: usize,
        max_tools: usize,
    ) -> (Vec<ModelToolDefinition>, bool) {
        let mut converted = Vec::new();
        let mut was_flattened = false;

        for tool in tools.iter().take(max_tools) {
            let (flattened_params, did_flatten) =
                flatten_json_schema_to_primitives(&tool.parameters, max_params);
            if did_flatten {
                was_flattened = true;
            }
            converted.push(ModelToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: flattened_params,
                is_micro_tool: true,
            });
        }

        if tools.len() > max_tools {
            was_flattened = true;
        }

        (converted, was_flattened)
    }

    /// Flatten nested schemas for standard fast coding profile
    fn flatten_deep_schemas(&self, tools: &[ModelToolDefinition]) -> Vec<ModelToolDefinition> {
        tools
            .iter()
            .map(|t| {
                let (params, _) = flatten_json_schema_to_primitives(&t.parameters, 8);
                ModelToolDefinition {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: params,
                    is_micro_tool: false,
                }
            })
            .collect()
    }

    fn calculate_total_tokens(&self, prompt: &str, tools: &[ModelToolDefinition]) -> usize {
        let prompt_tokens = estimate_tokens(prompt);
        let tools_json = serde_json::to_string(tools).unwrap_or_default();
        let tools_tokens = estimate_tokens(&tools_json);
        prompt_tokens + tools_tokens
    }

    fn compute_sha256_digest(
        &self,
        profile: ModelProfileTier,
        system_prompt: &str,
        tools: &[ModelToolDefinition],
        effort: ReasoningEffort,
        degraded: bool,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(profile.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(format!("{:?}", effort).as_bytes());
        hasher.update(b":");
        hasher.update(if degraded { b"degraded" } else { b"standard" });
        hasher.update(b":");
        hasher.update(system_prompt.as_bytes());
        hasher.update(b":");
        let tools_json = serde_json::to_string(tools).unwrap_or_default();
        hasher.update(tools_json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn flatten_schema_level(
    prefix: &str,
    schema: &serde_json::Value,
    is_parent_req: bool,
    flat_props: &mut BTreeMap<String, serde_json::Value>,
    required: &mut Vec<serde_json::Value>,
    max_params: usize,
    did_flatten: &mut bool,
) {
    if flat_props.len() >= max_params {
        *did_flatten = true;
        return;
    }

    let current_required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| i.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (k, v) in props {
            if flat_props.len() >= max_params {
                *did_flatten = true;
                break;
            }

            let full_key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{}_{}", prefix, k)
            };

            let prop_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("string");
            let child_is_req = is_parent_req && current_required.contains(k);

            let has_sub_props = v.get("properties").and_then(|p| p.as_object()).is_some();
            if (prop_type == "object" || has_sub_props) && has_sub_props {
                *did_flatten = true;
                flatten_schema_level(
                    &full_key,
                    v,
                    child_is_req,
                    flat_props,
                    required,
                    max_params,
                    did_flatten,
                );
            } else {
                if !prefix.is_empty() || prop_type == "object" || prop_type == "array" {
                    *did_flatten = true;
                }
                let primitive_type = match prop_type {
                    "integer" | "number" => "number",
                    "boolean" => "boolean",
                    _ => "string",
                };
                flat_props.insert(
                    full_key.clone(),
                    serde_json::json!({
                        "type": primitive_type,
                        "description": v.get("description").and_then(|d| d.as_str()).unwrap_or("")
                    }),
                );

                if child_is_req {
                    if !required
                        .iter()
                        .any(|r: &serde_json::Value| r.as_str() == Some(&full_key))
                    {
                        required.push(serde_json::Value::String(full_key));
                    }
                }
            }
        }
    }
}

/// Flatten a JSON Schema parameters object into flat primitive properties
fn flatten_json_schema_to_primitives(
    schema: &serde_json::Value,
    max_params: usize,
) -> (serde_json::Value, bool) {
    let mut flat_props = BTreeMap::new();
    let mut required = Vec::new();
    let mut did_flatten = false;

    flatten_schema_level(
        "",
        schema,
        true,
        &mut flat_props,
        &mut required,
        max_params,
        &mut did_flatten,
    );

    let flattened = serde_json::json!({
        "type": "object",
        "properties": flat_props,
        "required": required
    });
    (flattened, did_flatten)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tools() -> Vec<ModelToolDefinition> {
        vec![
            ModelToolDefinition {
                name: "edit_file".to_string(),
                description: "Edits target text inside a file".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute file path" },
                        "options": {
                            "type": "object",
                            "properties": {
                                "backup": { "type": "boolean" },
                                "retries": { "type": "integer" }
                            }
                        }
                    },
                    "required": ["path"]
                }),
                is_micro_tool: false,
            },
            ModelToolDefinition {
                name: "read_file".to_string(),
                description: "Reads file content".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute file path" }
                    },
                    "required": ["path"]
                }),
                is_micro_tool: false,
            },
        ]
    }

    #[test]
    fn test_profile_a_frontier_adaptation() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileA_Frontier,
            raw_system_prompt: "You are the specialist agent.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![],
            syntax_error_reported: false,
            max_budget_override: None,
        };

        let res = adapter.adapt(&req).expect("Adaptation should succeed");
        assert_eq!(res.active_profile, ModelProfileTier::ProfileA_Frontier);
        assert_eq!(res.reasoning_effort, ReasoningEffort::High);
        assert!(!res.degraded);
        assert_eq!(res.degradation_reason, None);
        assert!(res.system_prompt.contains("PROFILE A: FRONTIER REASONING"));
        assert_eq!(res.tools.len(), 2);
        assert_eq!(res.token_budget, TOKEN_BUDGET_CEILING);
        assert!(!res.sha256_digest.is_empty());
    }

    #[test]
    fn test_profile_b_fast_coding_adaptation() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileB_FastCoding,
            raw_system_prompt: "Implement code changes swiftly.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![],
            syntax_error_reported: false,
            max_budget_override: None,
        };

        let res = adapter.adapt(&req).expect("Adaptation should succeed");
        assert_eq!(res.active_profile, ModelProfileTier::ProfileB_FastCoding);
        assert_eq!(res.reasoning_effort, ReasoningEffort::Medium);
        assert!(!res.degraded);
        assert!(res.system_prompt.contains("PROFILE B: FAST CODING"));
        assert_eq!(res.token_budget, TOKEN_BUDGET_CEILING);
    }

    #[test]
    fn test_profile_c_compact_micro_tools_adaptation() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileC_Compact,
            raw_system_prompt: "Micro model instructions.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![],
            syntax_error_reported: false,
            max_budget_override: None,
        };

        let res = adapter.adapt(&req).expect("Adaptation should succeed");
        assert_eq!(res.active_profile, ModelProfileTier::ProfileC_Compact);
        assert_eq!(res.reasoning_effort, ReasoningEffort::Low);
        assert_eq!(res.token_budget, 1000);
        assert!(res.system_prompt.contains("PROFILE C: COMPACT"));
        for tool in &res.tools {
            assert!(tool.is_micro_tool);
        }
        // Verify schema flattening took place
        let edit_tool = res.tools.iter().find(|t| t.name == "edit_file").unwrap();
        let props = edit_tool.parameters.get("properties").unwrap();
        assert!(props.get("options_backup").is_some());
    }

    #[test]
    fn test_ac03_graceful_degradation_on_syntax_error() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileC_Compact,
            raw_system_prompt: "Micro model that tripped a tool syntax error.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![],
            syntax_error_reported: true, // Trigger AC-03
            max_budget_override: None,
        };

        let res = adapter.adapt(&req).expect("Adaptation should succeed");
        // Must gracefully degrade to Profile B FastCoding with micro-tools
        assert_eq!(res.active_profile, ModelProfileTier::ProfileB_FastCoding);
        assert_eq!(res.reasoning_effort, ReasoningEffort::Medium);
        assert!(res.degraded);
        assert_eq!(
            res.degradation_reason,
            Some(DegradationReason::SyntaxErrorRetry)
        );
        for tool in &res.tools {
            assert!(tool.is_micro_tool);
        }
    }

    #[test]
    fn test_project_constraint_disallowed_profile_clamp() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileA_Frontier,
            raw_system_prompt: "Specialist instructions in restricted environment.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![ModelProfileTier::ProfileA_Frontier], // Air-gap restriction
            syntax_error_reported: false,
            max_budget_override: None,
        };

        let res = adapter.adapt(&req).expect("Adaptation should succeed");
        assert_eq!(res.active_profile, ModelProfileTier::ProfileB_FastCoding);
        assert!(res.degraded);
        assert_eq!(
            res.degradation_reason,
            Some(DegradationReason::DisallowedProfileConstraint)
        );
    }

    #[test]
    fn test_deterministic_sha256_digest() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileA_Frontier,
            raw_system_prompt: "Deterministic hash test prompt.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![],
            syntax_error_reported: false,
            max_budget_override: None,
        };

        let res1 = adapter.adapt(&req).unwrap();
        let res2 = adapter.adapt(&req).unwrap();
        assert_eq!(res1.sha256_digest, res2.sha256_digest);
    }

    #[test]
    fn test_empty_tools_handling() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileA_Frontier,
            raw_system_prompt: "Prompt without tools.".to_string(),
            tools: vec![],
            disallowed_profiles: vec![],
            syntax_error_reported: false,
            max_budget_override: None,
        };

        let res = adapter
            .adapt(&req)
            .expect("Should succeed with empty tools");
        assert!(res.tools.is_empty());
        assert!(!res.degraded);
    }

    #[test]
    fn test_f02_syntax_error_fallback_respects_disallowed_profile_b() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileA_Frontier,
            raw_system_prompt: "Frontier model with syntax error.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![ModelProfileTier::ProfileB_FastCoding], // Profile B forbidden
            syntax_error_reported: true,
            max_budget_override: None,
        };

        let res = adapter.adapt(&req).expect("Should degrade to Profile C");
        assert_eq!(res.active_profile, ModelProfileTier::ProfileC_Compact);
        assert!(res.degraded);
        assert_eq!(
            res.degradation_reason,
            Some(DegradationReason::SyntaxErrorRetry)
        );
    }

    #[test]
    fn test_f03_tiny_budget_override_never_exceeds_budget() {
        let adapter = ModelCapabilityAdapter::new();
        let req = AdaptationRequest {
            project_id: Uuid::new_v4(),
            requested_profile: ModelProfileTier::ProfileA_Frontier,
            raw_system_prompt:
                "Extremely long system prompt that must be clamped under tiny budget.".to_string(),
            tools: sample_tools(),
            disallowed_profiles: vec![],
            syntax_error_reported: false,
            max_budget_override: Some(2), // Tiny 2 token budget
        };

        let res = adapter.adapt(&req).expect("Should clamp under tiny budget");
        assert!(res.estimated_tokens <= res.token_budget);
        assert_eq!(res.token_budget, 2);
    }

    #[test]
    fn test_f04_nested_required_properties_preserved_in_micro_tools() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "options": {
                    "type": "object",
                    "properties": {
                        "backup": { "type": "boolean" },
                        "mode": { "type": "string" }
                    },
                    "required": ["backup"]
                },
                "path": { "type": "string" }
            },
            "required": ["path", "options"]
        });

        let (flat, did_flatten) = flatten_json_schema_to_primitives(&schema, 4);
        assert!(did_flatten);
        let req_arr = flat.get("required").unwrap().as_array().unwrap();
        let req_strs: Vec<&str> = req_arr.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            req_strs.contains(&"options_backup"),
            "options_backup must be preserved in required"
        );
        assert!(
            !req_strs.contains(&"options_mode"),
            "optional child options_mode must NOT be marked required (F-05)"
        );
        assert!(
            req_strs.contains(&"path"),
            "path must be preserved in required"
        );
    }

    #[test]
    fn test_f06_deeply_nested_schema_flattening() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "options": {
                    "type": "object",
                    "properties": {
                        "advanced": {
                            "type": "object",
                            "properties": {
                                "enabled": { "type": "boolean" },
                                "retries": { "type": "integer" }
                            },
                            "required": ["enabled"]
                        },
                        "label": { "type": "string" }
                    },
                    "required": ["advanced"]
                }
            },
            "required": ["options"]
        });

        let (flat, did_flatten) = flatten_json_schema_to_primitives(&schema, 5);
        assert!(did_flatten);
        let props = flat.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("options_advanced_enabled"));
        assert_eq!(
            props
                .get("options_advanced_enabled")
                .unwrap()
                .get("type")
                .unwrap(),
            "boolean"
        );
        assert!(props.contains_key("options_advanced_retries"));
        assert_eq!(
            props
                .get("options_advanced_retries")
                .unwrap()
                .get("type")
                .unwrap(),
            "number"
        );
        assert!(props.contains_key("options_label"));

        let req_arr = flat.get("required").unwrap().as_array().unwrap();
        let req_strs: Vec<&str> = req_arr.iter().filter_map(|v| v.as_str()).collect();
        assert!(req_strs.contains(&"options_advanced_enabled"));
        assert!(!req_strs.contains(&"options_advanced_retries"));
        assert!(!req_strs.contains(&"options_label"));
    }
}
