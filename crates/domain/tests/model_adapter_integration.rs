// @story 02.3
// @implements AC-01, AC-02, AC-03
//! Integration test suite for Story 02.3: 3-Profile Model Capability Adapter
//! (`crates/domain/src/model_adapter.rs`).

use domain::{
    AdaptationRequest, DegradationReason, ModelCapabilityAdapter, ModelProfileTier,
    ModelToolDefinition, ReasoningEffort, TOKEN_BUDGET_CEILING,
};
use uuid::Uuid;

fn build_complex_tools() -> Vec<ModelToolDefinition> {
    vec![
        ModelToolDefinition {
            name: "patch_component_ast".to_string(),
            description: "Applies structured AST mutation to a UI component".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "component_path": { "type": "string", "description": "Target file path" },
                    "nested_config": {
                        "type": "object",
                        "properties": {
                            "preserve_comments": { "type": "boolean" },
                            "max_depth": { "type": "integer" }
                        },
                        "required": ["preserve_comments"]
                    }
                },
                "required": ["component_path", "nested_config"]
            }),
            is_micro_tool: false,
        },
        ModelToolDefinition {
            name: "inspect_tokens".to_string(),
            description: "Reads design token definitions".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "token_group": { "type": "string" }
                },
                "required": ["token_group"]
            }),
            is_micro_tool: false,
        },
    ]
}

#[test]
fn it_02_3_ac01_and_ac02_three_profile_adaptation_and_constraint_enforcement() {
    let adapter = ModelCapabilityAdapter::new();
    let project_id = Uuid::new_v4();

    // Profile A: Frontier Reasoning
    let req_a = AdaptationRequest {
        project_id,
        requested_profile: ModelProfileTier::ProfileA_Frontier,
        raw_system_prompt: "Synthesize multi-portal architecture.".to_string(),
        tools: build_complex_tools(),
        disallowed_profiles: vec![],
        syntax_error_reported: false,
        max_budget_override: None,
    };
    let res_a = adapter.adapt(&req_a).expect("adapt Profile A");
    assert_eq!(res_a.active_profile, ModelProfileTier::ProfileA_Frontier);
    assert_eq!(res_a.reasoning_effort, ReasoningEffort::High);
    assert_eq!(res_a.token_budget, TOKEN_BUDGET_CEILING);
    assert!(!res_a.degraded);

    // AC-02 Constraint Enforcement: Disallow Profile A -> clamp to Profile B
    let req_clamped = AdaptationRequest {
        disallowed_profiles: vec![ModelProfileTier::ProfileA_Frontier],
        ..req_a
    };
    let res_clamped = adapter.adapt(&req_clamped).expect("adapt clamped");
    assert_eq!(
        res_clamped.active_profile,
        ModelProfileTier::ProfileB_FastCoding
    );
    assert!(res_clamped.degraded);
    assert_eq!(
        res_clamped.degradation_reason,
        Some(DegradationReason::DisallowedProfileConstraint)
    );
}

#[test]
fn it_02_3_ac03_graceful_degradation_to_profile_b_with_micro_tools_on_syntax_error() {
    let adapter = ModelCapabilityAdapter::new();
    let req = AdaptationRequest {
        project_id: Uuid::new_v4(),
        requested_profile: ModelProfileTier::ProfileC_Compact,
        raw_system_prompt: "Compact model encountered malformed tool call.".to_string(),
        tools: build_complex_tools(),
        disallowed_profiles: vec![],
        syntax_error_reported: true,
        max_budget_override: None,
    };

    let res = adapter
        .adapt(&req)
        .expect("graceful degradation on syntax error");
    assert_eq!(res.active_profile, ModelProfileTier::ProfileB_FastCoding);
    assert_eq!(res.reasoning_effort, ReasoningEffort::Medium);
    assert!(res.degraded);
    assert_eq!(
        res.degradation_reason,
        Some(DegradationReason::SyntaxErrorRetry)
    );
    assert!(res.tools.iter().all(|t| t.is_micro_tool));
}
