use domain::*;
use ts_rs::TS;

#[test]
fn export_typescript_bindings() {
    let out_dir = std::path::Path::new("../../packages/shared-types/src/generated");
    std::fs::create_dir_all(out_dir).expect("Failed to create generated types directory");

    HarnessError::export().expect("Failed to export HarnessError");
    SafeJsInteger::export().expect("Failed to export SafeJsInteger");
    CanonicalSpecKey::export().expect("Failed to export CanonicalSpecKey");
    ProvenanceStatus::export().expect("Failed to export ProvenanceStatus");
    Project::export().expect("Failed to export Project");
    ProjectSpec::export().expect("Failed to export ProjectSpec");
    SpecRevision::export().expect("Failed to export SpecRevision");
    SpecProvenanceSummary::export().expect("Failed to export SpecProvenanceSummary");
    DesignIntentV1::export().expect("Failed to export DesignIntentV1");
    ChangeProposalV1::export().expect("Failed to export ChangeProposalV1");
    ConstraintProfile::export().expect("Failed to export ConstraintProfile");
    ScreenState::export().expect("Failed to export ScreenState");
    Transition::export().expect("Failed to export Transition");
    FlowGraph::export().expect("Failed to export FlowGraph");
    PresentationDeck::export().expect("Failed to export PresentationDeck");
    SkillPlugin::export().expect("Failed to export SkillPlugin");
    BotJob::export().expect("Failed to export BotJob");
    GateReport::export().expect("Failed to export GateReport");
    IconManifestEntry::export().expect("Failed to export IconManifestEntry");
    IconManifest::export().expect("Failed to export IconManifest");
    IconValidationReport::export().expect("Failed to export IconValidationReport");
    TypographyContract::export().expect("Failed to export TypographyContract");
    TypographyValidationReport::export().expect("Failed to export TypographyValidationReport");
    ComponentRegistryEntry::export().expect("Failed to export ComponentRegistryEntry");
    ComponentRegistryContract::export().expect("Failed to export ComponentRegistryContract");
    ComponentValidationReport::export().expect("Failed to export ComponentValidationReport");
    ProjectDesignContracts::export().expect("Failed to export ProjectDesignContracts");
    TaskHorizon::export().expect("Failed to export TaskHorizon");
    Department::export().expect("Failed to export Department");
    Portal::export().expect("Failed to export Portal");
    ReasonCode::export().expect("Failed to export ReasonCode");
    ContextSliceRequest::export().expect("Failed to export ContextSliceRequest");
    ContextSliceDecision::export().expect("Failed to export ContextSliceDecision");
    SpecSlice::export().expect("Failed to export SpecSlice");
    AuthorizedContextSlice::export().expect("Failed to export AuthorizedContextSlice");
    PriorityTier::export().expect("Failed to export PriorityTier");
    PromptSection::export().expect("Failed to export PromptSection");
    GovernedBudgetResult::export().expect("Failed to export GovernedBudgetResult");
    CompilationInput::export().expect("Failed to export CompilationInput");
    CompiledPromptPayload::export().expect("Failed to export CompiledPromptPayload");
    ModelProfileTier::export().expect("Failed to export ModelProfileTier");
    ReasoningEffort::export().expect("Failed to export ReasoningEffort");
    ToolComplexity::export().expect("Failed to export ToolComplexity");
    DegradationReason::export().expect("Failed to export DegradationReason");
    ModelToolDefinition::export().expect("Failed to export ModelToolDefinition");
    AdaptationRequest::export().expect("Failed to export AdaptationRequest");
    AdaptationResponse::export().expect("Failed to export AdaptationResponse");

    println!("All 47 domain types successfully exported to packages/shared-types/src/generated/");
}
