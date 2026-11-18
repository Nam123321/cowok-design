// @story 08.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
pub mod breaking_diff;
pub mod learn_pipeline;
pub mod plugin_state_machine;

pub use breaking_diff::{detect_breaking_change, parse_semver_strict, BreakingDiffReport, SemVer};
pub use learn_pipeline::{
    map_storage_constraint_error, ActorKind, CardDecisionAction, CardStatus, ConsumedNonceRecord,
    DomainStorageAction, HumanApproverToken, HumanSessionAuthority, KnowledgeCard,
    LearnCardCategory, LearnError, LearnPipeline, MintTokenInput, PromptInjectionResult,
    ProposeCardInput, StorageConstraintError, StorageConstraintTarget, MAX_LEARN_PROMPT_TOKENS,
};
pub use plugin_state_machine::{
    ManagedPlugin, ManagedPluginSnapshot, ManifestUpdateOutcome, PluginLifecycleError, PluginState,
    StagedAssetFile, StagedPluginBundle, StagedScriptFile,
};
