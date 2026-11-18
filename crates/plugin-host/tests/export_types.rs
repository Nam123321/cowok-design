// @story 08.3
// @implements AC-02
#[cfg(feature = "ts")]
#[test]
fn export_plugin_host_typescript_bindings() {
    use plugin_host::*;
    use ts_rs::TS;

    let out_dir = std::path::Path::new("../../packages/shared-types/src/generated");
    std::fs::create_dir_all(out_dir).expect("create generated types dir");

    SkillManifest::export().expect("export SkillManifest");
    StagingRunReceipt::export().expect("export StagingRunReceipt");
    LintSeverity::export().expect("export LintSeverity");
    LintIssueCategory::export().expect("export LintIssueCategory");
    LintIssue::export().expect("export LintIssue");
    TriangleLintReport::export().expect("export TriangleLintReport");
    DiscoveryStub::export().expect("export DiscoveryStub");
    LockedSkillEntry::export().expect("export LockedSkillEntry");
    SkillsLockFile::export().expect("export SkillsLockFile");
    RepackageReceipt::export().expect("export RepackageReceipt");
    VideoFrameCheckpoint::export().expect("export VideoFrameCheckpoint");
    VideoStoryboardReceipt::export().expect("export VideoStoryboardReceipt");
    SemVer::export().expect("export SemVer");
    BreakingDiffReport::export().expect("export BreakingDiffReport");
    PluginState::export().expect("export PluginState");
    StagedScriptFile::export().expect("export StagedScriptFile");
    StagedAssetFile::export().expect("export StagedAssetFile");
    StagedPluginBundle::export().expect("export StagedPluginBundle");
    ManifestUpdateOutcome::export().expect("export ManifestUpdateOutcome");
    ManagedPlugin::export().expect("export ManagedPlugin");
    LearnCardCategory::export().expect("export LearnCardCategory");
    CardStatus::export().expect("export CardStatus");
    CardDecisionAction::export().expect("export CardDecisionAction");
    ActorKind::export().expect("export ActorKind");
    HumanApproverToken::export().expect("export HumanApproverToken");
    ConsumedNonceRecord::export().expect("export ConsumedNonceRecord");
    KnowledgeCard::export().expect("export KnowledgeCard");
    PromptInjectionResult::export().expect("export PromptInjectionResult");
    StorageConstraintTarget::export().expect("export StorageConstraintTarget");
    StorageConstraintError::export().expect("export StorageConstraintError");
    DomainStorageAction::export().expect("export DomainStorageAction");
}
