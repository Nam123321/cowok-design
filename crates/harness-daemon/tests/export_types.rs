// @story 08.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
#[cfg(feature = "ts")]
#[test]
fn export_harness_daemon_plugin_typescript_bindings() {
    use harness_daemon::daemon_auth::HttpActorKind;
    use harness_daemon::routes::plugins::{
        CompiledPromptSnapshotDto, CronScheduleStatusDto, HumanOnlyDecisionReceiptDto,
        KnowledgeCardDto, PluginSummaryDto,
    };
    use ts_rs::TS;

    let out_dir = std::path::Path::new("../../packages/shared-types/src/generated");
    std::fs::create_dir_all(out_dir).expect("create generated types dir");

    HttpActorKind::export().expect("export HttpActorKind");
    PluginSummaryDto::export().expect("export PluginSummaryDto");
    CompiledPromptSnapshotDto::export().expect("export CompiledPromptSnapshotDto");
    KnowledgeCardDto::export().expect("export KnowledgeCardDto");
    HumanOnlyDecisionReceiptDto::export().expect("export HumanOnlyDecisionReceiptDto");
    CronScheduleStatusDto::export().expect("export CronScheduleStatusDto");
}
