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

    println!("All 17 domain types successfully exported to packages/shared-types/src/generated/");
}
