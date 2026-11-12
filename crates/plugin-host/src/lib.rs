//! plugin-host: Core engine for agentskills.io plugin management,
//! deterministic Triangle Linter, per-run skill staging, capability registry,
//! Zero-Trust skill repackager engine, and 6-state lifecycle with LEARN self-improving loop.

pub mod lifecycle;
pub mod models;
pub mod registry;
pub mod repackager;
pub mod staging;
pub mod triangle_linter;
pub mod validator;

pub use lifecycle::{
    CardStatus, KnowledgeCard, LearnCardCategory, LearnPipeline, ManagedPlugin,
    PluginLifecycleError, PluginState,
};
pub use models::{
    LintIssue, LintIssueCategory, LintSeverity, SkillManifest, StagingRunReceipt,
    TriangleLintReport,
};
pub use registry::SkillRegistry;
pub use repackager::{
    DiscoveryStub, RepackageReceipt, SkillRepackager, VideoFrameCheckpoint, VideoIntakeProcessor,
    VideoStoryboardReceipt,
};
pub use staging::PerRunStager;
pub use triangle_linter::{
    auto_fix_issues, check_geometric_and_wcag_rules, scan_slop_and_tells, TriangleLinter,
};
pub use validator::{estimate_token_count, validate_skill_frontmatter, ValidationError};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
