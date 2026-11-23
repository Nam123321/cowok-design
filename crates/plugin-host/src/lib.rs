//! plugin-host: Core engine for agentskills.io plugin management,
//! deterministic Triangle Linter, per-run skill staging, capability registry,
//! Zero-Trust skill repackager engine, and 6-state lifecycle with LEARN self-improving loop.

// @story 08.1, 08.2, 08.3, 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07
pub mod de_tools;
pub mod learn_filter;
pub mod lifecycle;
pub mod models;
pub mod registry;
pub mod repackager;
pub mod staging;
pub mod tool_jail;
pub mod triangle_linter;
pub mod user_skill_packager;
pub mod validator;

pub use de_tools::{
    archify_from_text, bundle_artifacts_bfs, render_archify_svg, run_builtin_de_suite,
    run_intake_kind_cap, run_nfa_packs, run_notes_check, run_scan_ui, run_tokens_oklch,
    ArchifyBfsError, ArchifyEdge, ArchifyGraphIr, ArchifyNode, ArchifySvgOutput,
    ArtifactBundleReport, BuiltInDeTool, DeFinding, DeFindingSeverity, DeRunReport, DeSuiteInput,
    DeToolResult, IntakeCapReport, IntakeFileSpec, NfaPackKind, NfaPackReport, NotesCheckReport,
    TokenPairInput, TokensCheckReport, DE_NFA_SLA_MS, GET_ARTIFACT_MAX_DEPTH,
    GET_ARTIFACT_MAX_FILES,
};
pub use learn_filter::{
    check_learn_smuggling_safety, compute_jev_noul_fingerprint, evaluate_hysteresis_transition,
    list_gate_failure_ledger_scoped, record_gate_failure_and_evaluate, GateFailureLedgerEntry,
    LearnFilterError, LedgerStatus, RecordGateFailureInput, LEARN_MIN_HIT_COUNT,
    LEARN_PROMOTE_SLPR_THRESHOLD, LEARN_RETIRE_SLPR_THRESHOLD, LEARN_SCOPE_MAX_CARDS,
};
pub use tool_jail::{
    assert_workspace_no_symlinks_or_hardlinks, execute_in_ephemeral_tool_jail,
    scan_ephemeral_script_capabilities, EphemeralRuntime, EphemeralToolRequest, StagedJailFile,
    ToolJailAuditReceipt, ToolJailError, TOOL_JAIL_DEFAULT_TIMEOUT_MS, TOOL_JAIL_MAX_TIMEOUT_MS,
};
pub use user_skill_packager::{
    build_jit_skill_markdown, classify_spdx_license, list_user_skills_scoped,
    package_and_register_user_skill, validate_import_repo_ssrf, ImportUserSkillRequest,
    LicenseClassification, PackagedUserSkillReceipt, SecurityScanVerdict, UserSkillPackagerError,
    USER_SKILL_MAX_HEADER_TOKENS,
};

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
