//! plugin-host: Core engine for agentskills.io plugin management,
//! deterministic Triangle Linter, per-run skill staging, capability registry,
//! Zero-Trust skill repackager engine, and 6-state lifecycle with LEARN self-improving loop.

// @story 08.1, 08.2, 08.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07
pub mod lifecycle;
pub mod models;
pub mod registry;
pub mod repackager;
pub mod staging;
pub mod triangle_linter;
pub mod validator;

pub use lifecycle::{
    detect_breaking_change, map_storage_constraint_error, parse_semver_strict, ActorKind,
    BreakingDiffReport, CardDecisionAction, CardStatus, ConsumedNonceRecord, DomainStorageAction,
    HumanApproverToken, HumanSessionAuthority, KnowledgeCard, LearnCardCategory, LearnError,
    LearnPipeline, ManagedPlugin, ManagedPluginSnapshot, ManifestUpdateOutcome, MintTokenInput,
    PluginLifecycleError, PluginState, PromptInjectionResult, ProposeCardInput, SemVer,
    StagedAssetFile, StagedPluginBundle, StagedScriptFile, StorageConstraintError,
    StorageConstraintTarget, MAX_LEARN_PROMPT_TOKENS,
};
pub use models::{
    LintIssue, LintIssueCategory, LintSeverity, SkillManifest, StagingRunReceipt,
    TriangleLintReport,
};
pub use registry::SkillRegistry;
pub use repackager::{
    classify_and_validate_source_asset, load_verified_staged_bundle_from_dir,
    manifest_to_skill_plugin, scan_external_script_security, scan_skill_markdown_safety,
    validate_safe_segment, validate_skill_metadata_text, DiscoveryStub, LockedSkillEntry,
    RepackageAssetBucket, RepackageReceipt, RepackageSecurityError, SkillRepackager,
    SkillsLockFile, StagedBundleReceipt, VideoFrameCheckpoint, VideoIntakeError,
    VideoIntakeProcessor, VideoStoryboardReceipt, MAX_DISCOVERY_STUB_BYTES,
};
pub use staging::{ensure_safe_dir_no_symlink, validate_staged_prompt_content, PerRunStager};
pub use triangle_linter::{
    auto_fix_issues, check_geometric_and_wcag_rules, scan_slop_and_tells, TriangleLinter,
};
pub use validator::{
    estimate_token_count, parse_and_validate_skill_frontmatter, validate_skill_frontmatter,
    FrontmatterMetadata, ValidationError,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
