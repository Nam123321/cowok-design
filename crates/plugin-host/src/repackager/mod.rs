// @story 08.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07
pub mod repackager_engine;
pub mod security_scanner;
pub mod video_intake;

pub use repackager_engine::{
    load_verified_staged_bundle_from_dir, manifest_to_skill_plugin, validate_safe_segment,
    DiscoveryStub, LockedSkillEntry, RepackageReceipt, SkillRepackager, SkillsLockFile,
    StagedBundleReceipt, MAX_DISCOVERY_STUB_BYTES,
};
pub use security_scanner::{
    classify_and_validate_source_asset, scan_external_script_security, scan_skill_markdown_safety,
    validate_skill_metadata_text, RepackageAssetBucket, RepackageSecurityError,
};
pub use video_intake::{
    VideoFrameCheckpoint, VideoIntakeError, VideoIntakeProcessor, VideoStoryboardReceipt,
};
