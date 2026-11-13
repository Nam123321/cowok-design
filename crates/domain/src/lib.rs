// @story 01.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
//! # Domain Crate for `cowok-design`
//!
//! Core domain entities, contracts, canonical spec keys, safe integer primitives,
//! and error taxonomy. This crate resides at Layer 0 of the architecture DAG
//! and has zero internal crate dependencies.

pub mod error;
pub mod icon_typography;
pub mod model_adapter;
pub mod models;
pub mod prompt_compiler;
pub mod safe_integer;
pub mod spec_key;
pub mod task_horizon;
pub mod token_budget;

pub use error::HarnessError;
pub use icon_typography::{
    ComponentRegistryContract, ComponentRegistryEntry, ComponentValidationReport, IconManifest,
    IconManifestEntry, IconValidationReport, ProjectDesignContracts, TypographyContract,
    TypographyValidationReport,
};
pub use model_adapter::{
    AdaptationRequest, AdaptationResponse, DegradationReason, ModelCapabilityAdapter,
    ModelProfileTier, ModelToolDefinition, ReasoningEffort, ToolComplexity,
};
pub use models::*;
pub use prompt_compiler::{CompilationInput, CompiledPromptPayload, DeterministicPromptCompiler};
pub use safe_integer::{SafeJsInteger, MAX_SAFE_INTEGER, MIN_SAFE_INTEGER};
pub use spec_key::CanonicalSpecKey;
pub use task_horizon::{
    AuthorizedContextSlice, ContextSliceDecision, ContextSliceRequest, Department, Portal,
    ReasonCode, SpecSlice, TaskHorizon, TaskHorizonRouter, VisibilityPolicy,
};
pub use token_budget::{
    estimate_tokens, truncate_utf8_to_tokens, GovernedBudgetResult, PriorityTier, PromptSection,
    TokenBudgetGovernor, TOKEN_BUDGET_CEILING,
};
