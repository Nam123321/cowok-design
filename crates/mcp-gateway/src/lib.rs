// @story 11.2
// @implements AC-01, AC-02, AC-03, AC-04 (Compiler-Grade Bidirectional Open MCP Server, Run-Scoped Tool Bundles, Deterministic Solvers & Safe 1-Click Host Installer)
//! `mcp-gateway` crate for `cowok-design`.

pub mod host_installer;
pub mod protocol;
pub mod solvers;

pub use host_installer::{
    install_host_config, list_supported_hosts, merge_codex_toml_config, merge_json_host_config,
    reject_symlinks_in_path, rollback_host_install, HostClientId, HostConfigFormat,
    HostInstallError, HostInstallOptions, HostInstallReceipt, HostRollbackReceipt, HostTargetInfo,
    COWOK_SERVER_KEY, COWOK_TOML_KEY,
};
pub use protocol::{
    default_tool_policies, format_sse_event, format_stdio_frame, parse_stdio_frame, ActivityStatus,
    ActorKind, ApplyBatchResult, CanvasNodeState, DiffImageSummary, DispatchOutcome, JsonRpcError,
    JsonRpcRequest, JsonRpcResponse, McpGatewayEngine, PermissionTier, SelfHealingToolError,
    ToolPolicyRecord, ERR_HUMAN_APPROVAL_REQUIRED, ERR_INTERNAL, ERR_INVALID_PARAMS,
    ERR_INVALID_REQUEST, ERR_METHOD_NOT_FOUND, ERR_PARSE, ERR_SELF_HEALING_TOOL, ERR_TOOL_DISABLED,
    JSONRPC_VERSION, MCP_PROTOCOL_VERSION, WORKBENCH_APPROVAL_DEEP_LINK_BASE,
};
pub use solvers::{
    list_banner_presets, resolve_run_scoped_bundle, solve_banner_grid, solve_grounded_diagram,
    BannerSlotInput, BannerSlotPlacement, BannerSolveRequest, BannerSolveResult, DiagramEdgeInput,
    DiagramNodeInput, DiagramPoint, GroundedDiagramRequest, GroundedDiagramResult,
    PositionedDiagramNode, RoutedDiagramEdge, RunScopedToolBundle, RunTaskKind, SolverError,
    MAX_BANNER_SLOTS, MAX_EDGES, MAX_NODES,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
