// @story 11.2
// @implements AC-01, AC-02, AC-03 (Bidirectional JSON-RPC 2.0 MCP Server: stdio/HTTP-SSE framing, 4-Tier Permissions, EC-LEARN-001 HUMAN_ONLY Block, get_app_state, atomic apply_batch, query_predicate, Self-Healing ToolError & Dynamic Resources)
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::solvers::{
    list_banner_presets, resolve_run_scoped_bundle, solve_banner_grid, solve_grounded_diagram,
    BannerSolveRequest, GroundedDiagramRequest, MAX_NODES,
};

pub const JSONRPC_VERSION: &str = "2.0";
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

pub const ERR_PARSE: i32 = -32700;
pub const ERR_INVALID_REQUEST: i32 = -32600;
pub const ERR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERR_INVALID_PARAMS: i32 = -32602;
pub const ERR_INTERNAL: i32 = -32603;
pub const ERR_HUMAN_APPROVAL_REQUIRED: i32 = -32003;
pub const ERR_TOOL_DISABLED: i32 = -32004;
pub const ERR_SELF_HEALING_TOOL: i32 = -32010;

pub const WORKBENCH_APPROVAL_DEEP_LINK_BASE: &str =
    "cowok-design://workbench/settings/mcp?approvalRequired=true&tool=";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PermissionTier {
    ReadOnly,
    Inspect,
    RevisePropose,
    HumanOnly,
}

impl PermissionTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "READ_ONLY",
            Self::Inspect => "INSPECT",
            Self::RevisePropose => "REVISE_PROPOSE",
            Self::HumanOnly => "HUMAN_ONLY",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "READ_ONLY" => Some(Self::ReadOnly),
            "INSPECT" => Some(Self::Inspect),
            "REVISE_PROPOSE" | "REVISE / PROPOSE" => Some(Self::RevisePropose),
            "HUMAN_ONLY" | "FULL_CREATE / HUMAN_ONLY" | "FULL_CREATE" => Some(Self::HumanOnly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActorKind {
    McpClient,
    HumanWorkbenchSession,
}

impl ActorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::McpClient => "MCP_CLIENT",
            Self::HumanWorkbenchSession => "HUMAN_WORKBENCH_SESSION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivityStatus {
    Ok,
    SelfHealingError,
    DeniedHumanOnly,
    ValidationError,
}

impl ActivityStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::SelfHealingError => "SELF_HEALING_ERROR",
            Self::DeniedHumanOnly => "DENIED_HUMAN_ONLY",
            Self::ValidationError => "VALIDATION_ERROR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPolicyRecord {
    pub tool_name: String,
    pub permission_tier: PermissionTier,
    pub enabled: bool,
    pub mcp_callable: bool,
    pub description: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfHealingToolError {
    pub code: String,
    pub message: String,
    pub recovery_hint: String,
    pub valid_examples: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workbench_deep_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_step: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rolled_back_steps: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchOutcome {
    pub response: JsonRpcResponse,
    pub method_name: String,
    pub tool_name: Option<String>,
    pub permission_tier: PermissionTier,
    pub status: ActivityStatus,
    pub jsonrpc_code: Option<i32>,
    pub latency_ms: u64,
    pub idempotency_key: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasNodeState {
    pub id: String,
    pub kind: String,
    pub parent_id: Option<String>,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffImageSummary {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub deleted: Vec<String>,
    pub preview_svg_data_uri: String,
    pub diff_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyBatchResult {
    pub applied: bool,
    pub dry_run: bool,
    pub idempotent_replay: bool,
    pub version: u64,
    pub resolved_aliases: BTreeMap<String, String>,
    pub operations_executed: usize,
    pub diff_image: DiffImageSummary,
    pub issues: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchOperation {
    op: String,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    target_id: Option<String>,
    #[serde(default)]
    parent_id: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyBatchArgs {
    operations: Vec<BatchOperation>,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// Canonical default 4-tier tool policies seeded into SQLite V28 `mcp_tool_policies`.
pub fn default_tool_policies() -> Vec<ToolPolicyRecord> {
    let now = Utc::now().timestamp_millis();
    vec![
        ToolPolicyRecord {
            tool_name: "get_app_state".to_string(),
            permission_tier: PermissionTier::ReadOnly,
            enabled: true,
            mcp_callable: true,
            description: "Read current Cowok-Design workspace canvas nodes, document version, and layout issues.".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "resolve_run_bundle".to_string(),
            permission_tier: PermissionTier::ReadOnly,
            enabled: true,
            mcp_callable: true,
            description: "Resolve the allowed tool bundle scoped to a specific design task.".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "query_predicate".to_string(),
            permission_tier: PermissionTier::Inspect,
            enabled: true,
            mcp_callable: true,
            description: "Query canvas nodes by predicate such as has-problem('text-truncated') or kind('card').".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "banner_solver".to_string(),
            permission_tier: PermissionTier::Inspect,
            enabled: true,
            mcp_callable: true,
            description: "Compute collision-free banner grid layouts across presets G1-G15.".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "grounded_diagram_elk".to_string(),
            permission_tier: PermissionTier::Inspect,
            enabled: true,
            mcp_callable: true,
            description: "Compute deterministic layered architecture diagram layouts.".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "apply_batch".to_string(),
            permission_tier: PermissionTier::RevisePropose,
            enabled: true,
            mcp_callable: true,
            description: "Apply atomic canvas batch edits with @alias references, dry-run preview, diff image, and automatic rollback.".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "ephemeral_tool_forge".to_string(),
            permission_tier: PermissionTier::HumanOnly,
            enabled: false,
            mcp_callable: false,
            description: "Create isolated helper scripts (requires direct in-app confirmation).".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "learn_card_promote".to_string(),
            permission_tier: PermissionTier::HumanOnly,
            enabled: false,
            mcp_callable: false,
            description: "Approve and promote learned design rules (requires direct in-app confirmation).".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "staging_plugin_promote".to_string(),
            permission_tier: PermissionTier::HumanOnly,
            enabled: false,
            mcp_callable: false,
            description: "Promote staged extensions to the active workspace (requires direct in-app confirmation).".to_string(),
            updated_at: now,
        },
        ToolPolicyRecord {
            tool_name: "workspace_destructive_reset".to_string(),
            permission_tier: PermissionTier::HumanOnly,
            enabled: false,
            mcp_callable: false,
            description: "Reset the entire workspace state (requires direct in-app confirmation).".to_string(),
            updated_at: now,
        },
    ]
}

fn is_inherently_human_only_name(tool_name: &str) -> bool {
    let lower = tool_name.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "ephemeral_tool_forge"
            | "learn_card_promote"
            | "staging_plugin_promote"
            | "workspace_destructive_reset"
    ) || lower.contains("ephemeral_tool")
        || lower.contains("learn_promote")
        || lower.contains("learn_card")
        || lower.contains("staging_promote")
        || lower.contains("human_only")
}

fn compute_node_problems(node: &CanvasNodeState) -> Vec<String> {
    let mut problems = Vec::new();
    let est_text_width = (node.text.chars().count() as f64) * 7.5;
    if !node.text.is_empty() && est_text_width > node.width {
        problems.push("text-truncated".to_string());
    }
    if node.width < 16.0 || node.height < 16.0 {
        problems.push("min-size-violation".to_string());
    }
    problems
}

fn build_diff_image(
    added: Vec<String>,
    updated: Vec<String>,
    deleted: Vec<String>,
    nodes: &BTreeMap<String, CanvasNodeState>,
) -> DiffImageSummary {
    let mut rects = String::new();
    for node in nodes.values().take(64) {
        let stroke = if added.contains(&node.id) {
            "#16a34a"
        } else if updated.contains(&node.id) {
            "#2563eb"
        } else {
            "#64748b"
        };
        rects.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>",
            node.x.max(0.0),
            node.y.max(0.0),
            node.width.max(1.0),
            node.height.max(1.0),
            stroke
        ));
    }
    let svg =
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1200 720\">{rects}</svg>");
    let mut hasher = Sha256::new();
    hasher.update(svg.as_bytes());
    let diff_sha256 = format!("{:x}", hasher.finalize());
    let encoded_svg = svg.replace('#', "%23").replace('"', "'");
    DiffImageSummary {
        added,
        updated,
        deleted,
        preview_svg_data_uri: format!("data:image/svg+xml;utf8,{encoded_svg}"),
        diff_sha256,
    }
}

fn resolve_ref(raw: &str, aliases: &BTreeMap<String, String>) -> Result<String, String> {
    if raw.starts_with('@') {
        aliases
            .get(raw)
            .cloned()
            .ok_or_else(|| format!("Unresolved alias '{raw}' in apply_batch"))
    } else {
        Ok(raw.to_string())
    }
}

struct WorkspaceDocumentState {
    project_id: String,
    version: u64,
    nodes: BTreeMap<String, CanvasNodeState>,
    idempotency_cache: HashMap<String, ApplyBatchResult>,
    policies: BTreeMap<String, ToolPolicyRecord>,
    skills: BTreeMap<String, String>,
}

impl WorkspaceDocumentState {
    fn new(project_id: impl Into<String>) -> Self {
        let mut nodes = BTreeMap::new();
        let mut hero = CanvasNodeState {
            id: "node-hero-title".to_string(),
            kind: "heading".to_string(),
            parent_id: None,
            text: "Cowok-Design Compiler-Grade Bidirectional MCP Workbench Header Title"
                .to_string(),
            x: 32.0,
            y: 32.0,
            width: 240.0,
            height: 44.0,
            problems: Vec::new(),
        };
        hero.problems = compute_node_problems(&hero);
        nodes.insert(hero.id.clone(), hero);

        let mut cta = CanvasNodeState {
            id: "node-primary-cta".to_string(),
            kind: "button".to_string(),
            parent_id: None,
            text: "Deploy".to_string(),
            x: 32.0,
            y: 96.0,
            width: 140.0,
            height: 40.0,
            problems: Vec::new(),
        };
        cta.problems = compute_node_problems(&cta);
        nodes.insert(cta.id.clone(), cta);

        let mut policies = BTreeMap::new();
        for p in default_tool_policies() {
            policies.insert(p.tool_name.clone(), p);
        }

        let mut skills = BTreeMap::new();
        skills.insert(
            "design-craft".to_string(),
            "# Cowok Design Craft Skill\n- Use semantic CSS variables (`var(--bg-card)`, `var(--text-primary)`).\n- Use `<BrandIcon>` with 1.85px Monoline Outline SVG sprite.\n- Zero Tailwind classes in workbench surfaces.\n".to_string(),
        );
        skills.insert(
            "banner-solver".to_string(),
            "# Banner Solver G1-G15 Skill\nCall `banner_solver` with preset `G1`..`G15` and canvas dimensions to obtain guaranteed collision-free coordinates.\n".to_string(),
        );
        skills.insert(
            "grounded-diagram".to_string(),
            format!(
                "# Grounded Diagram ELK Skill\nCall `grounded_diagram_elk` with nodes (<= {MAX_NODES}) and edges for deterministic Sugiyama layered layouts with repo symbol grounding.\n"
            ),
        );

        Self {
            project_id: project_id.into(),
            version: 1,
            nodes,
            idempotency_cache: HashMap::new(),
            policies,
            skills,
        }
    }
}

pub struct McpGatewayEngine {
    inner: Mutex<WorkspaceDocumentState>,
}

impl Default for McpGatewayEngine {
    fn default() -> Self {
        Self::new("default-workbench")
    }
}

impl McpGatewayEngine {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            inner: Mutex::new(WorkspaceDocumentState::new(project_id)),
        }
    }

    pub fn register_skill(&self, slug: impl Into<String>, markdown: impl Into<String>) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.skills.insert(slug.into(), markdown.into());
        }
    }

    /// Synchronizes persisted SQLite tool policies into the gateway engine while enforcing
    /// immutable `HUMAN_ONLY` isolation (`mcp_callable = false`).
    pub fn sync_tool_policies(&self, overrides: &[ToolPolicyRecord]) {
        if let Ok(mut guard) = self.inner.lock() {
            for rec in overrides {
                let mut clean = rec.clone();
                if clean.permission_tier == PermissionTier::HumanOnly
                    || is_inherently_human_only_name(&clean.tool_name)
                {
                    clean.permission_tier = PermissionTier::HumanOnly;
                    clean.mcp_callable = false;
                }
                guard.policies.insert(clean.tool_name.clone(), clean);
            }
        }
    }

    pub fn list_policies(&self) -> Vec<ToolPolicyRecord> {
        self.inner
            .lock()
            .map(|g| g.policies.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn dispatch(&self, req: JsonRpcRequest, actor: ActorKind) -> DispatchOutcome {
        let started = Instant::now();
        let req_id = req.id.clone().unwrap_or(Value::Null);

        if req.jsonrpc != JSONRPC_VERSION {
            let err = JsonRpcResponse::err(
                req_id,
                ERR_INVALID_REQUEST,
                "Invalid JSON-RPC version; expected '2.0'",
                None,
            );
            return DispatchOutcome {
                response: err,
                method_name: req.method,
                tool_name: None,
                permission_tier: PermissionTier::ReadOnly,
                status: ActivityStatus::ValidationError,
                jsonrpc_code: Some(ERR_INVALID_REQUEST),
                latency_ms: started.elapsed().as_millis() as u64,
                idempotency_key: None,
                summary: "Invalid JSON-RPC version".to_string(),
            };
        }

        match req.method.as_str() {
            "initialize" => self.handle_initialize(req_id, started),
            "ping" => DispatchOutcome {
                response: JsonRpcResponse::ok(req_id, json!({ "pong": true })),
                method_name: "ping".to_string(),
                tool_name: None,
                permission_tier: PermissionTier::ReadOnly,
                status: ActivityStatus::Ok,
                jsonrpc_code: None,
                latency_ms: started.elapsed().as_millis() as u64,
                idempotency_key: None,
                summary: "ping ok".to_string(),
            },
            "tools/list" => self.handle_tools_list(req_id, actor, started),
            "tools/call" => self.handle_tools_call(req_id, req.params, actor, started),
            "resources/list" => self.handle_resources_list(req_id, started),
            "resources/read" => self.handle_resources_read(req_id, req.params, started),
            other => DispatchOutcome {
                response: JsonRpcResponse::err(
                    req_id,
                    ERR_METHOD_NOT_FOUND,
                    format!("Method '{other}' not found"),
                    Some(json!({
                        "supportedMethods": ["initialize", "ping", "tools/list", "tools/call", "resources/list", "resources/read"]
                    })),
                ),
                method_name: other.to_string(),
                tool_name: None,
                permission_tier: PermissionTier::ReadOnly,
                status: ActivityStatus::ValidationError,
                jsonrpc_code: Some(ERR_METHOD_NOT_FOUND),
                latency_ms: started.elapsed().as_millis() as u64,
                idempotency_key: None,
                summary: format!("Unknown method {other}"),
            },
        }
    }

    fn handle_initialize(&self, id: Value, started: Instant) -> DispatchOutcome {
        let result = json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "serverInfo": {
                "name": "cowok-design-mcp-gateway",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "tools": { "listChanged": true },
                "resources": { "subscribe": false, "listChanged": true }
            }
        });
        DispatchOutcome {
            response: JsonRpcResponse::ok(id, result),
            method_name: "initialize".to_string(),
            tool_name: None,
            permission_tier: PermissionTier::ReadOnly,
            status: ActivityStatus::Ok,
            jsonrpc_code: None,
            latency_ms: started.elapsed().as_millis() as u64,
            idempotency_key: None,
            summary: "MCP initialize handshake complete".to_string(),
        }
    }

    fn handle_tools_list(&self, id: Value, actor: ActorKind, started: Instant) -> DispatchOutcome {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return DispatchOutcome {
                    response: JsonRpcResponse::err(id, ERR_INTERNAL, "Engine lock poisoned", None),
                    method_name: "tools/list".to_string(),
                    tool_name: None,
                    permission_tier: PermissionTier::ReadOnly,
                    status: ActivityStatus::ValidationError,
                    jsonrpc_code: Some(ERR_INTERNAL),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: "Engine lock poisoned".to_string(),
                };
            }
        };

        let tools: Vec<Value> = guard
            .policies
            .values()
            .filter(|policy| {
                // EC-LEARN-001: Strictly hide HUMAN_ONLY and non-callable/disabled tools from ActorKind::McpClient
                if actor == ActorKind::McpClient {
                    policy.permission_tier != PermissionTier::HumanOnly
                        && policy.mcp_callable
                        && policy.enabled
                } else {
                    true
                }
            })
            .map(|policy| {
                json!({
                    "name": policy.tool_name,
                    "description": policy.description,
                    "permissionTier": policy.permission_tier.as_str(),
                    "inputSchema": tool_input_schema(&policy.tool_name),
                })
            })
            .collect();

        let count = tools.len();
        DispatchOutcome {
            response: JsonRpcResponse::ok(id, json!({ "tools": tools })),
            method_name: "tools/list".to_string(),
            tool_name: None,
            permission_tier: PermissionTier::ReadOnly,
            status: ActivityStatus::Ok,
            jsonrpc_code: None,
            latency_ms: started.elapsed().as_millis() as u64,
            idempotency_key: None,
            summary: format!("Listed {count} tools for {}", actor.as_str()),
        }
    }

    fn handle_tools_call(
        &self,
        id: Value,
        params: Option<Value>,
        actor: ActorKind,
        started: Instant,
    ) -> DispatchOutcome {
        let Some(params_obj) = params.and_then(|v| v.as_object().cloned()) else {
            return self_healing_outcome(
                id,
                None,
                PermissionTier::ReadOnly,
                started,
                None,
                SelfHealingToolError {
                    code: "INVALID_TOOL_CALL_PARAMS".to_string(),
                    message: "tools/call requires a JSON object with 'name' and 'arguments'."
                        .to_string(),
                    recovery_hint: "Pass {\"name\": \"get_app_state\", \"arguments\": {}}."
                        .to_string(),
                    valid_examples: vec![
                        json!({ "name": "get_app_state", "arguments": {} }),
                        json!({ "name": "query_predicate", "arguments": { "predicate": "has-problem('text-truncated')" } }),
                    ],
                    workbench_deep_link: None,
                    failed_step: None,
                    rolled_back_steps: None,
                },
            );
        };

        let Some(tool_name) = params_obj
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        else {
            return self_healing_outcome(
                id,
                None,
                PermissionTier::ReadOnly,
                started,
                None,
                SelfHealingToolError {
                    code: "MISSING_TOOL_NAME".to_string(),
                    message: "Missing non-empty 'name' in tools/call params.".to_string(),
                    recovery_hint: "Call tools/list first to inspect available tool names."
                        .to_string(),
                    valid_examples: vec![json!({ "name": "get_app_state", "arguments": {} })],
                    workbench_deep_link: None,
                    failed_step: None,
                    rolled_back_steps: None,
                },
            );
        };

        let arguments = params_obj
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let idempotency_key = arguments
            .get("idempotencyKey")
            .or_else(|| arguments.get("idempotency_key"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let policy_snapshot = {
            let guard = match self.inner.lock() {
                Ok(g) => g,
                Err(_) => {
                    return DispatchOutcome {
                        response: JsonRpcResponse::err(
                            id,
                            ERR_INTERNAL,
                            "Engine lock poisoned",
                            None,
                        ),
                        method_name: "tools/call".to_string(),
                        tool_name: Some(tool_name),
                        permission_tier: PermissionTier::ReadOnly,
                        status: ActivityStatus::ValidationError,
                        jsonrpc_code: Some(ERR_INTERNAL),
                        latency_ms: started.elapsed().as_millis() as u64,
                        idempotency_key,
                        summary: "Engine lock poisoned".to_string(),
                    };
                }
            };
            guard.policies.get(&tool_name).cloned()
        };

        // EC-LEARN-001: UNCONDITIONAL HUMAN_ONLY rejection for ActorKind::McpClient.
        // Zero approval token / override parameter bypass is ever honored.
        let is_human_only = is_inherently_human_only_name(&tool_name)
            || policy_snapshot
                .as_ref()
                .is_some_and(|p| p.permission_tier == PermissionTier::HumanOnly);

        if is_human_only && actor == ActorKind::McpClient {
            let deep_link = format!("{WORKBENCH_APPROVAL_DEEP_LINK_BASE}{tool_name}");
            let err_payload = SelfHealingToolError {
                code: "HUMAN_APPROVAL_REQUIRED".to_string(),
                message: format!(
                    "Tool '{tool_name}' is restricted to HUMAN_ONLY (EC-LEARN-001) and is unconditionally denied for ActorKind::McpClient. Approval tokens passed via MCP are ignored."
                ),
                recovery_hint: "Direct the human operator to open the Workbench deep-link and execute the approval interactively in HumanWorkbenchSession.".to_string(),
                valid_examples: vec![json!({ "workbenchDeepLink": deep_link })],
                workbench_deep_link: Some(deep_link),
                failed_step: None,
                rolled_back_steps: None,
            };
            return DispatchOutcome {
                response: JsonRpcResponse::err(
                    id,
                    ERR_HUMAN_APPROVAL_REQUIRED,
                    "HUMAN_APPROVAL_REQUIRED",
                    serde_json::to_value(&err_payload).ok(),
                ),
                method_name: "tools/call".to_string(),
                tool_name: Some(tool_name.clone()),
                permission_tier: PermissionTier::HumanOnly,
                status: ActivityStatus::DeniedHumanOnly,
                jsonrpc_code: Some(ERR_HUMAN_APPROVAL_REQUIRED),
                latency_ms: started.elapsed().as_millis() as u64,
                idempotency_key,
                summary: format!("Blocked HUMAN_ONLY tool '{tool_name}' for MCP_CLIENT"),
            };
        }

        if let Some(ref pol) = policy_snapshot {
            if (!pol.enabled || !pol.mcp_callable) && actor == ActorKind::McpClient {
                let err_payload = SelfHealingToolError {
                    code: "TOOL_DISABLED".to_string(),
                    message: format!("Tool '{tool_name}' is currently disabled in MCP policy settings."),
                    recovery_hint: "Enable the tool in Workbench Settings -> MCP Tool Permissions before retrying.".to_string(),
                    valid_examples: vec![json!({ "method": "tools/list" })],
                    workbench_deep_link: Some("cowok-design://workbench/settings/mcp".to_string()),
                    failed_step: None,
                    rolled_back_steps: None,
                };
                return DispatchOutcome {
                    response: JsonRpcResponse::err(
                        id,
                        ERR_TOOL_DISABLED,
                        format!("Tool '{tool_name}' is disabled"),
                        serde_json::to_value(&err_payload).ok(),
                    ),
                    method_name: "tools/call".to_string(),
                    tool_name: Some(tool_name.clone()),
                    permission_tier: pol.permission_tier,
                    status: ActivityStatus::SelfHealingError,
                    jsonrpc_code: Some(ERR_TOOL_DISABLED),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key,
                    summary: format!("Rejected disabled tool '{tool_name}'"),
                };
            }
        }

        match tool_name.as_str() {
            "get_app_state" => self.exec_get_app_state(id, started),
            "apply_batch" => self.exec_apply_batch(id, arguments, started, idempotency_key),
            "query_predicate" => self.exec_query_predicate(id, arguments, started),
            "banner_solver" => self.exec_banner_solver(id, arguments, started),
            "grounded_diagram_elk" => self.exec_grounded_diagram(id, arguments, started),
            "resolve_run_bundle" => self.exec_resolve_run_bundle(id, arguments, started),
            unknown => self_healing_outcome(
                id,
                Some(unknown.to_string()),
                PermissionTier::ReadOnly,
                started,
                idempotency_key,
                SelfHealingToolError {
                    code: "UNKNOWN_TOOL".to_string(),
                    message: format!("Tool '{unknown}' is not registered in mcp-gateway."),
                    recovery_hint: "Call tools/list to inspect callable tools.".to_string(),
                    valid_examples: vec![
                        json!({ "name": "get_app_state", "arguments": {} }),
                        json!({ "name": "apply_batch", "arguments": { "dryRun": true, "operations": [] } }),
                        json!({ "name": "banner_solver", "arguments": { "preset": "G1", "width": 1200, "height": 630 } }),
                    ],
                    workbench_deep_link: None,
                    failed_step: None,
                    rolled_back_steps: None,
                },
            ),
        }
    }

    fn exec_get_app_state(&self, id: Value, started: Instant) -> DispatchOutcome {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return DispatchOutcome {
                    response: JsonRpcResponse::err(id, ERR_INTERNAL, "Engine lock poisoned", None),
                    method_name: "tools/call".to_string(),
                    tool_name: Some("get_app_state".to_string()),
                    permission_tier: PermissionTier::ReadOnly,
                    status: ActivityStatus::ValidationError,
                    jsonrpc_code: Some(ERR_INTERNAL),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: "Engine lock poisoned".to_string(),
                };
            }
        };
        let nodes: Vec<CanvasNodeState> = guard.nodes.values().cloned().collect();
        let problem_count: usize = nodes.iter().map(|n| n.problems.len()).sum();
        let latency_ms = started.elapsed().as_millis() as u64;
        let payload = json!({
            "projectId": guard.project_id,
            "version": guard.version,
            "nodeCount": nodes.len(),
            "problemCount": problem_count,
            "nodes": nodes,
            "latencyMs": latency_ms,
        });
        DispatchOutcome {
            response: JsonRpcResponse::ok(
                id,
                json!({
                    "content": [{ "type": "json", "json": payload }],
                    "structuredContent": payload,
                }),
            ),
            method_name: "tools/call".to_string(),
            tool_name: Some("get_app_state".to_string()),
            permission_tier: PermissionTier::ReadOnly,
            status: ActivityStatus::Ok,
            jsonrpc_code: None,
            latency_ms,
            idempotency_key: None,
            summary: format!("get_app_state v{} ({} nodes)", guard.version, nodes.len()),
        }
    }

    fn exec_apply_batch(
        &self,
        id: Value,
        arguments: Value,
        started: Instant,
        idempotency_key: Option<String>,
    ) -> DispatchOutcome {
        // Normalize snake_case compatibility (`dry_run`, `idempotency_key`, `target_id`, `parent_id`)
        let normalized_args = normalize_batch_args(arguments);
        let args: ApplyBatchArgs = match serde_json::from_value(normalized_args) {
            Ok(a) => a,
            Err(err) => {
                return self_healing_outcome(
                    id,
                    Some("apply_batch".to_string()),
                    PermissionTier::RevisePropose,
                    started,
                    idempotency_key,
                    SelfHealingToolError {
                        code: "INVALID_BATCH_PAYLOAD".to_string(),
                        message: format!("Failed to parse apply_batch arguments: {err}"),
                        recovery_hint: "Provide an 'operations' array of create_node/update_node/delete_node/set_text/set_bounds items.".to_string(),
                        valid_examples: vec![json!({
                            "dryRun": true,
                            "idempotencyKey": "batch-001",
                            "operations": [
                                { "op": "create_node", "alias": "@hero", "kind": "card", "text": "Hero", "x": 32, "y": 32, "width": 320, "height": 120 },
                                { "op": "set_text", "targetId": "@hero", "text": "Updated Hero" }
                            ]
                        })],
                        workbench_deep_link: None,
                        failed_step: None,
                        rolled_back_steps: None,
                    },
                );
            }
        };

        let effective_key = args
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                idempotency_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            });
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return DispatchOutcome {
                    response: JsonRpcResponse::err(id, ERR_INTERNAL, "Engine lock poisoned", None),
                    method_name: "tools/call".to_string(),
                    tool_name: Some("apply_batch".to_string()),
                    permission_tier: PermissionTier::RevisePropose,
                    status: ActivityStatus::ValidationError,
                    jsonrpc_code: Some(ERR_INTERNAL),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: effective_key,
                    summary: "Engine lock poisoned".to_string(),
                };
            }
        };

        if !args.dry_run {
            if let Some(ref key) = effective_key {
                if let Some(cached) = guard.idempotency_cache.get(key) {
                    let mut replay = cached.clone();
                    replay.idempotent_replay = true;
                    let payload = serde_json::to_value(&replay).unwrap_or(Value::Null);
                    return DispatchOutcome {
                        response: JsonRpcResponse::ok(
                            id,
                            json!({
                                "content": [{ "type": "json", "json": payload }],
                                "structuredContent": payload,
                            }),
                        ),
                        method_name: "tools/call".to_string(),
                        tool_name: Some("apply_batch".to_string()),
                        permission_tier: PermissionTier::RevisePropose,
                        status: ActivityStatus::Ok,
                        jsonrpc_code: None,
                        latency_ms: started.elapsed().as_millis() as u64,
                        idempotency_key: effective_key,
                        summary: "apply_batch idempotent replay".to_string(),
                    };
                }
            }
        }

        // Transactional copy-on-write snapshot so failure at step k rolls back steps 1..k atomically.
        let mut working_nodes = guard.nodes.clone();
        let mut aliases: BTreeMap<String, String> = BTreeMap::new();
        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut deleted = Vec::new();

        for (idx, op) in args.operations.iter().enumerate() {
            let step_num = idx + 1;
            let step_result: Result<(), String> = (|| -> Result<(), String> {
                match op.op.as_str() {
                    "create_node" => {
                        let node_id = op
                            .id
                            .clone()
                            .unwrap_or_else(|| format!("node-{}-{}", guard.version + 1, step_num));
                        if working_nodes.contains_key(&node_id) {
                            return Err(format!("Node '{node_id}' already exists"));
                        }
                        let width = op.width.unwrap_or(240.0);
                        let height = op.height.unwrap_or(80.0);
                        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0
                        {
                            return Err(
                                "width and height must be positive finite numbers".to_string()
                            );
                        }
                        let parent_id = match op.parent_id.as_deref() {
                            Some(raw_parent) => Some(resolve_ref(raw_parent, &aliases)?),
                            None => None,
                        };
                        if let Some(ref pid) = parent_id {
                            if !working_nodes.contains_key(pid) {
                                return Err(format!("parent '{pid}' not found"));
                            }
                        }
                        let mut node = CanvasNodeState {
                            id: node_id.clone(),
                            kind: op.kind.clone().unwrap_or_else(|| "card".to_string()),
                            parent_id,
                            text: op.text.clone().unwrap_or_default(),
                            x: op.x.unwrap_or(0.0),
                            y: op.y.unwrap_or(0.0),
                            width,
                            height,
                            problems: Vec::new(),
                        };
                        node.problems = compute_node_problems(&node);
                        if let Some(ref alias) = op.alias {
                            let normalized_alias = if alias.starts_with('@') {
                                alias.clone()
                            } else {
                                format!("@{alias}")
                            };
                            aliases.insert(normalized_alias, node_id.clone());
                        }
                        working_nodes.insert(node_id.clone(), node);
                        added.push(node_id);
                        Ok(())
                    }
                    "update_node" | "set_text" | "set_bounds" => {
                        let raw_target = op.target_id.as_deref().or(op.id.as_deref()).unwrap_or("");
                        if raw_target.is_empty() {
                            return Err("Missing targetId for mutation op".to_string());
                        }
                        let resolved = resolve_ref(raw_target, &aliases)?;
                        let Some(node) = working_nodes.get_mut(&resolved) else {
                            return Err(format!("Target node '{resolved}' does not exist"));
                        };
                        if let Some(ref t) = op.text {
                            node.text = t.clone();
                        }
                        if let Some(x) = op.x {
                            node.x = x;
                        }
                        if let Some(y) = op.y {
                            node.y = y;
                        }
                        if let Some(w) = op.width {
                            if !w.is_finite() || w <= 0.0 {
                                return Err("width must be > 0".to_string());
                            }
                            node.width = w;
                        }
                        if let Some(h) = op.height {
                            if !h.is_finite() || h <= 0.0 {
                                return Err("height must be > 0".to_string());
                            }
                            node.height = h;
                        }
                        node.problems = compute_node_problems(node);
                        updated.push(resolved);
                        Ok(())
                    }
                    "delete_node" => {
                        let raw_target = op.target_id.as_deref().or(op.id.as_deref()).unwrap_or("");
                        if raw_target.is_empty() {
                            return Err("Missing targetId for delete_node".to_string());
                        }
                        let resolved = resolve_ref(raw_target, &aliases)?;
                        if working_nodes.remove(&resolved).is_some() {
                            deleted.push(resolved);
                            Ok(())
                        } else {
                            Err(format!("Cannot delete missing node '{resolved}'"))
                        }
                    }
                    unsupported => Err(format!(
                        "Unsupported operation '{unsupported}'. Allowed: create_node, update_node, delete_node, set_text, set_bounds."
                    )),
                }
            })();

            if let Err(reason) = step_result {
                // Live state `guard.nodes` was never touched -> atomic rollback guaranteed!
                return self_healing_outcome(
                    id,
                    Some("apply_batch".to_string()),
                    PermissionTier::RevisePropose,
                    started,
                    effective_key,
                    SelfHealingToolError {
                        code: "BATCH_STEP_FAILED".to_string(),
                        message: format!(
                            "Step {step_num} failed: {reason}. Atomically rolled back {idx} prior step(s)."
                        ),
                        recovery_hint: "Fix the failing step or run with dryRun: true after inspecting get_app_state.".to_string(),
                        valid_examples: vec![json!({
                            "dryRun": true,
                            "operations": [
                                { "op": "create_node", "alias": "@card", "kind": "card", "text": "Hello", "width": 240, "height": 80 },
                                { "op": "set_text", "targetId": "@card", "text": "Updated" }
                            ]
                        })],
                        workbench_deep_link: None,
                        failed_step: Some(step_num),
                        rolled_back_steps: Some(idx),
                    },
                );
            }
        }

        let issues: Vec<Value> = working_nodes
            .values()
            .filter(|n| !n.problems.is_empty())
            .map(|n| {
                json!({
                    "nodeId": n.id,
                    "problems": n.problems,
                })
            })
            .collect();

        let diff_image = build_diff_image(added, updated, deleted, &working_nodes);
        let next_version = if args.dry_run {
            guard.version
        } else {
            guard.version += 1;
            guard.nodes = working_nodes;
            guard.version
        };

        let result = ApplyBatchResult {
            applied: !args.dry_run,
            dry_run: args.dry_run,
            idempotent_replay: false,
            version: next_version,
            resolved_aliases: aliases,
            operations_executed: args.operations.len(),
            diff_image,
            issues,
        };

        if !args.dry_run {
            if let Some(ref key) = effective_key {
                guard.idempotency_cache.insert(key.clone(), result.clone());
            }
        }

        let payload = serde_json::to_value(&result).unwrap_or(Value::Null);
        DispatchOutcome {
            response: JsonRpcResponse::ok(
                id,
                json!({
                    "content": [{ "type": "json", "json": payload }],
                    "structuredContent": payload,
                }),
            ),
            method_name: "tools/call".to_string(),
            tool_name: Some("apply_batch".to_string()),
            permission_tier: PermissionTier::RevisePropose,
            status: ActivityStatus::Ok,
            jsonrpc_code: None,
            latency_ms: started.elapsed().as_millis() as u64,
            idempotency_key: effective_key,
            summary: format!(
                "apply_batch {} ({} ops, v{})",
                if args.dry_run { "dry_run" } else { "committed" },
                args.operations.len(),
                next_version
            ),
        }
    }

    fn exec_query_predicate(
        &self,
        id: Value,
        arguments: Value,
        started: Instant,
    ) -> DispatchOutcome {
        let predicate = arguments
            .get("predicate")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if predicate.is_empty() {
            return self_healing_outcome(
                id,
                Some("query_predicate".to_string()),
                PermissionTier::Inspect,
                started,
                None,
                SelfHealingToolError {
                    code: "INVALID_PREDICATE".to_string(),
                    message: "Missing 'predicate' string in query_predicate arguments.".to_string(),
                    recovery_hint:
                        "Pass a predicate like has-problem('text-truncated') or kind('button')."
                            .to_string(),
                    valid_examples: vec![
                        json!({ "predicate": "has-problem('text-truncated')" }),
                        json!({ "predicate": "kind('button')" }),
                        json!({ "predicate": "all" }),
                    ],
                    workbench_deep_link: None,
                    failed_step: None,
                    rolled_back_steps: None,
                },
            );
        }

        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return DispatchOutcome {
                    response: JsonRpcResponse::err(id, ERR_INTERNAL, "Engine lock poisoned", None),
                    method_name: "tools/call".to_string(),
                    tool_name: Some("query_predicate".to_string()),
                    permission_tier: PermissionTier::Inspect,
                    status: ActivityStatus::ValidationError,
                    jsonrpc_code: Some(ERR_INTERNAL),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: "Engine lock poisoned".to_string(),
                };
            }
        };

        let matched: Result<Vec<CanvasNodeState>, String> = if predicate == "all" {
            Ok(guard.nodes.values().cloned().collect())
        } else if let Some(problem_kind) = extract_fn_arg(predicate, "has-problem") {
            Ok(guard
                .nodes
                .values()
                .filter(|n| n.problems.iter().any(|p| p == &problem_kind))
                .cloned()
                .collect())
        } else if let Some(node_kind) = extract_fn_arg(predicate, "kind") {
            Ok(guard
                .nodes
                .values()
                .filter(|n| n.kind.eq_ignore_ascii_case(&node_kind))
                .cloned()
                .collect())
        } else {
            Err(format!("Unsupported predicate syntax '{predicate}'"))
        };

        match matched {
            Ok(nodes) => {
                let count = nodes.len();
                let payload = json!({
                    "predicate": predicate,
                    "matchCount": count,
                    "matches": nodes,
                });
                DispatchOutcome {
                    response: JsonRpcResponse::ok(
                        id,
                        json!({
                            "content": [{ "type": "json", "json": payload }],
                            "structuredContent": payload,
                        }),
                    ),
                    method_name: "tools/call".to_string(),
                    tool_name: Some("query_predicate".to_string()),
                    permission_tier: PermissionTier::Inspect,
                    status: ActivityStatus::Ok,
                    jsonrpc_code: None,
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: format!("query_predicate({predicate}) -> {count} matches"),
                }
            }
            Err(msg) => self_healing_outcome(
                id,
                Some("query_predicate".to_string()),
                PermissionTier::Inspect,
                started,
                None,
                SelfHealingToolError {
                    code: "INVALID_PREDICATE".to_string(),
                    message: msg,
                    recovery_hint: "Use has-problem('<issue>'), kind('<nodeKind>'), or 'all'."
                        .to_string(),
                    valid_examples: vec![
                        json!({ "predicate": "has-problem('text-truncated')" }),
                        json!({ "predicate": "kind('heading')" }),
                    ],
                    workbench_deep_link: None,
                    failed_step: None,
                    rolled_back_steps: None,
                },
            ),
        }
    }

    fn exec_banner_solver(&self, id: Value, arguments: Value, started: Instant) -> DispatchOutcome {
        let req: BannerSolveRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(err) => {
                return self_healing_outcome(
                    id,
                    Some("banner_solver".to_string()),
                    PermissionTier::Inspect,
                    started,
                    None,
                    SelfHealingToolError {
                        code: "INVALID_BANNER_REQUEST".to_string(),
                        message: format!("Invalid banner_solver arguments: {err}"),
                        recovery_hint:
                            "Provide preset ('G1'..'G15'), width, height, and optional slots."
                                .to_string(),
                        valid_examples: vec![json!({
                            "preset": "G10",
                            "width": 1200,
                            "height": 630,
                            "padding": 32,
                            "gap": 16
                        })],
                        workbench_deep_link: None,
                        failed_step: None,
                        rolled_back_steps: None,
                    },
                );
            }
        };

        match solve_banner_grid(&req) {
            Ok(solved) => {
                let payload = serde_json::to_value(&solved).unwrap_or(Value::Null);
                DispatchOutcome {
                    response: JsonRpcResponse::ok(
                        id,
                        json!({
                            "content": [{ "type": "json", "json": payload }],
                            "structuredContent": payload,
                        }),
                    ),
                    method_name: "tools/call".to_string(),
                    tool_name: Some("banner_solver".to_string()),
                    permission_tier: PermissionTier::Inspect,
                    status: ActivityStatus::Ok,
                    jsonrpc_code: None,
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: format!(
                        "banner_solver {} ({} placements)",
                        solved.preset,
                        solved.placements.len()
                    ),
                }
            }
            Err(err) => {
                let presets: Vec<String> = list_banner_presets()
                    .into_iter()
                    .map(|(k, _)| k.to_string())
                    .collect();
                self_healing_outcome(
                    id,
                    Some("banner_solver".to_string()),
                    PermissionTier::Inspect,
                    started,
                    None,
                    SelfHealingToolError {
                        code: "BANNER_SOLVER_ERROR".to_string(),
                        message: err.to_string(),
                        recovery_hint: format!(
                            "Select one of the deterministic presets: {}",
                            presets.join(", ")
                        ),
                        valid_examples: vec![
                            json!({ "preset": "G1", "width": 1200, "height": 630 }),
                        ],
                        workbench_deep_link: None,
                        failed_step: None,
                        rolled_back_steps: None,
                    },
                )
            }
        }
    }

    fn exec_grounded_diagram(
        &self,
        id: Value,
        arguments: Value,
        started: Instant,
    ) -> DispatchOutcome {
        let req: GroundedDiagramRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(err) => {
                return self_healing_outcome(
                    id,
                    Some("grounded_diagram_elk".to_string()),
                    PermissionTier::Inspect,
                    started,
                    None,
                    SelfHealingToolError {
                        code: "INVALID_DIAGRAM_REQUEST".to_string(),
                        message: format!("Invalid grounded_diagram_elk arguments: {err}"),
                        recovery_hint: format!(
                            "Provide 'nodes' (max {MAX_NODES}) and 'edges' arrays."
                        ),
                        valid_examples: vec![json!({
                            "direction": "RIGHT",
                            "nodes": [{ "id": "api", "label": "API" }, { "id": "db", "label": "SQLite WAL" }],
                            "edges": [{ "source": "api", "target": "db" }]
                        })],
                        workbench_deep_link: None,
                        failed_step: None,
                        rolled_back_steps: None,
                    },
                );
            }
        };

        match solve_grounded_diagram(&req) {
            Ok(solved) => {
                let payload = serde_json::to_value(&solved).unwrap_or(Value::Null);
                DispatchOutcome {
                    response: JsonRpcResponse::ok(
                        id,
                        json!({
                            "content": [{ "type": "json", "json": payload }],
                            "structuredContent": payload,
                        }),
                    ),
                    method_name: "tools/call".to_string(),
                    tool_name: Some("grounded_diagram_elk".to_string()),
                    permission_tier: PermissionTier::Inspect,
                    status: ActivityStatus::Ok,
                    jsonrpc_code: None,
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: format!(
                        "grounded_diagram_elk {} nodes / {} layers",
                        solved.node_count, solved.layer_count
                    ),
                }
            }
            Err(err) => self_healing_outcome(
                id,
                Some("grounded_diagram_elk".to_string()),
                PermissionTier::Inspect,
                started,
                None,
                SelfHealingToolError {
                    code: "DIAGRAM_SOLVER_ERROR".to_string(),
                    message: err.to_string(),
                    recovery_hint: format!(
                        "Reduce node count to <= {MAX_NODES} and verify every edge references declared node ids."
                    ),
                    valid_examples: vec![json!({
                        "direction": "RIGHT",
                        "nodes": [{ "id": "a", "label": "A" }, { "id": "b", "label": "B" }],
                        "edges": [{ "source": "a", "target": "b" }]
                    })],
                    workbench_deep_link: None,
                    failed_step: None,
                    rolled_back_steps: None,
                },
            ),
        }
    }

    fn exec_resolve_run_bundle(
        &self,
        id: Value,
        arguments: Value,
        started: Instant,
    ) -> DispatchOutcome {
        let task_kind = arguments
            .get("taskKind")
            .or_else(|| arguments.get("task_kind"))
            .and_then(Value::as_str)
            .unwrap_or("general");
        let bundle = resolve_run_scoped_bundle(task_kind);
        let payload = serde_json::to_value(&bundle).unwrap_or(Value::Null);
        DispatchOutcome {
            response: JsonRpcResponse::ok(
                id,
                json!({
                    "content": [{ "type": "json", "json": payload }],
                    "structuredContent": payload,
                }),
            ),
            method_name: "tools/call".to_string(),
            tool_name: Some("resolve_run_bundle".to_string()),
            permission_tier: PermissionTier::ReadOnly,
            status: ActivityStatus::Ok,
            jsonrpc_code: None,
            latency_ms: started.elapsed().as_millis() as u64,
            idempotency_key: None,
            summary: format!("resolve_run_bundle({})", bundle.task_kind),
        }
    }

    fn handle_resources_list(&self, id: Value, started: Instant) -> DispatchOutcome {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return DispatchOutcome {
                    response: JsonRpcResponse::err(id, ERR_INTERNAL, "Engine lock poisoned", None),
                    method_name: "resources/list".to_string(),
                    tool_name: None,
                    permission_tier: PermissionTier::ReadOnly,
                    status: ActivityStatus::ValidationError,
                    jsonrpc_code: Some(ERR_INTERNAL),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: "Engine lock poisoned".to_string(),
                };
            }
        };

        let mut resources = vec![
            json!({
                "uri": "cowok-design://rules",
                "name": "Cowok-Design Constitutional Rules",
                "mimeType": "text/markdown",
            }),
            json!({
                "uri": "ui://cowok-design/brief-and-preview.html",
                "name": "Cowok-Design Brief & Live Preview",
                "mimeType": "text/html",
            }),
        ];
        for slug in guard.skills.keys() {
            resources.push(json!({
                "uri": format!("cowok://skills/{slug}"),
                "name": format!("Skill: {slug}"),
                "mimeType": "text/markdown",
            }));
        }

        let count = resources.len();
        DispatchOutcome {
            response: JsonRpcResponse::ok(id, json!({ "resources": resources })),
            method_name: "resources/list".to_string(),
            tool_name: None,
            permission_tier: PermissionTier::ReadOnly,
            status: ActivityStatus::Ok,
            jsonrpc_code: None,
            latency_ms: started.elapsed().as_millis() as u64,
            idempotency_key: None,
            summary: format!("Listed {count} MCP resources"),
        }
    }

    fn handle_resources_read(
        &self,
        id: Value,
        params: Option<Value>,
        started: Instant,
    ) -> DispatchOutcome {
        let uri = params
            .as_ref()
            .and_then(|p| p.get("uri"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return DispatchOutcome {
                    response: JsonRpcResponse::err(id, ERR_INTERNAL, "Engine lock poisoned", None),
                    method_name: "resources/read".to_string(),
                    tool_name: None,
                    permission_tier: PermissionTier::ReadOnly,
                    status: ActivityStatus::ValidationError,
                    jsonrpc_code: Some(ERR_INTERNAL),
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: "Engine lock poisoned".to_string(),
                };
            }
        };

        if uri == "cowok-design://rules" {
            let rules_md = "# Cowok-Design MCP Rules\n\
                1. **4-Tier Permission Model**: `READ_ONLY`, `INSPECT`, `REVISE_PROPOSE`, `HUMAN_ONLY`.\n\
                2. **EC-LEARN-001**: `HUMAN_ONLY` tools are strictly hidden and unconditionally rejected for `ActorKind::McpClient`.\n\
                3. **Atomic Batching**: Always pass `idempotencyKey` and preview with `dryRun: true` before committing mutations.\n\
                4. **Visual Standards**: Semantic CSS variables only, `<BrandIcon>` monoline SVG sprite, zero Tailwind.\n";
            return DispatchOutcome {
                response: JsonRpcResponse::ok(
                    id,
                    json!({
                        "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": rules_md }]
                    }),
                ),
                method_name: "resources/read".to_string(),
                tool_name: None,
                permission_tier: PermissionTier::ReadOnly,
                status: ActivityStatus::Ok,
                jsonrpc_code: None,
                latency_ms: started.elapsed().as_millis() as u64,
                idempotency_key: None,
                summary: "Read cowok-design://rules".to_string(),
            };
        }

        if uri == "ui://cowok-design/brief-and-preview.html" {
            let items: String = guard
                .nodes
                .values()
                .map(|n| {
                    format!(
                        "<li><strong>{}</strong> ({}) — {}</li>",
                        escape_html(&n.id),
                        escape_html(&n.kind),
                        escape_html(&n.text)
                    )
                })
                .collect();
            let html = format!(
                "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Cowok-Design Brief &amp; Preview</title></head><body><h1>Project: {}</h1><p>Version: {}</p><ul>{}</ul></body></html>",
                escape_html(&guard.project_id),
                guard.version,
                items
            );
            return DispatchOutcome {
                response: JsonRpcResponse::ok(
                    id,
                    json!({
                        "contents": [{ "uri": uri, "mimeType": "text/html", "text": html }]
                    }),
                ),
                method_name: "resources/read".to_string(),
                tool_name: None,
                permission_tier: PermissionTier::ReadOnly,
                status: ActivityStatus::Ok,
                jsonrpc_code: None,
                latency_ms: started.elapsed().as_millis() as u64,
                idempotency_key: None,
                summary: "Read ui://cowok-design/brief-and-preview.html".to_string(),
            };
        }

        if let Some(slug) = uri.strip_prefix("cowok://skills/") {
            if let Some(md) = guard.skills.get(slug) {
                return DispatchOutcome {
                    response: JsonRpcResponse::ok(
                        id,
                        json!({
                            "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": md }]
                        }),
                    ),
                    method_name: "resources/read".to_string(),
                    tool_name: None,
                    permission_tier: PermissionTier::ReadOnly,
                    status: ActivityStatus::Ok,
                    jsonrpc_code: None,
                    latency_ms: started.elapsed().as_millis() as u64,
                    idempotency_key: None,
                    summary: format!("Read {uri}"),
                };
            }
        }

        self_healing_outcome(
            id,
            None,
            PermissionTier::ReadOnly,
            started,
            None,
            SelfHealingToolError {
                code: "RESOURCE_NOT_FOUND".to_string(),
                message: format!("MCP resource '{uri}' was not found."),
                recovery_hint: "Call resources/list to retrieve valid cowok-design://, cowok://skills/*, or ui:// URIs.".to_string(),
                valid_examples: vec![
                    json!({ "uri": "cowok-design://rules" }),
                    json!({ "uri": "cowok://skills/design-craft" }),
                    json!({ "uri": "ui://cowok-design/brief-and-preview.html" }),
                ],
                workbench_deep_link: None,
                failed_step: None,
                rolled_back_steps: None,
            },
        )
    }
}

fn self_healing_outcome(
    id: Value,
    tool_name: Option<String>,
    tier: PermissionTier,
    started: Instant,
    idempotency_key: Option<String>,
    err: SelfHealingToolError,
) -> DispatchOutcome {
    let summary = format!("{}: {}", err.code, err.message);
    DispatchOutcome {
        response: JsonRpcResponse::err(
            id,
            ERR_SELF_HEALING_TOOL,
            err.message.clone(),
            serde_json::to_value(&err).ok(),
        ),
        method_name: "tools/call".to_string(),
        tool_name,
        permission_tier: tier,
        status: ActivityStatus::SelfHealingError,
        jsonrpc_code: Some(ERR_SELF_HEALING_TOOL),
        latency_ms: started.elapsed().as_millis() as u64,
        idempotency_key,
        summary,
    }
}

fn normalize_batch_args(mut val: Value) -> Value {
    if let Some(obj) = val.as_object_mut() {
        if let Some(v) = obj.remove("dry_run") {
            obj.entry("dryRun").or_insert(v);
        }
        if let Some(v) = obj.remove("idempotency_key") {
            obj.entry("idempotencyKey").or_insert(v);
        }
        if let Some(Value::Array(ops)) = obj.get_mut("operations") {
            for item in ops.iter_mut() {
                if let Some(op_obj) = item.as_object_mut() {
                    if let Some(v) = op_obj.remove("target_id") {
                        op_obj.entry("targetId").or_insert(v);
                    }
                    if let Some(v) = op_obj.remove("parent_id") {
                        op_obj.entry("parentId").or_insert(v);
                    }
                }
            }
        }
    }
    val
}

fn extract_fn_arg(predicate: &str, fn_name: &str) -> Option<String> {
    let rest = predicate.strip_prefix(fn_name)?.trim();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?.trim();
    let unquoted = inner
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .or_else(|| inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
        .unwrap_or(inner)
        .trim();
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

fn escape_html(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn tool_input_schema(tool_name: &str) -> Value {
    match tool_name {
        "get_app_state" => {
            json!({ "type": "object", "properties": {}, "additionalProperties": false })
        }
        "apply_batch" => json!({
            "type": "object",
            "required": ["operations"],
            "properties": {
                "dryRun": { "type": "boolean" },
                "idempotencyKey": { "type": "string" },
                "operations": { "type": "array" }
            }
        }),
        "query_predicate" => json!({
            "type": "object",
            "required": ["predicate"],
            "properties": { "predicate": { "type": "string" } }
        }),
        "banner_solver" => json!({
            "type": "object",
            "required": ["preset"],
            "properties": {
                "preset": { "type": "string", "description": "G1..G15" },
                "width": { "type": "number" },
                "height": { "type": "number" }
            }
        }),
        "grounded_diagram_elk" => json!({
            "type": "object",
            "required": ["nodes"],
            "properties": {
                "direction": { "type": "string", "enum": ["RIGHT", "DOWN"] },
                "nodes": { "type": "array", "maxItems": MAX_NODES },
                "edges": { "type": "array" }
            }
        }),
        "resolve_run_bundle" => json!({
            "type": "object",
            "required": ["taskKind"],
            "properties": { "taskKind": { "type": "string" } }
        }),
        _ => json!({ "type": "object" }),
    }
}

/// Parses an MCP `stdio` frame supporting both `Content-Length: N\r\n\r\n{...}` (with optional
/// sibling headers like `Content-Type`) and newline-delimited JSON.
#[allow(clippy::result_large_err)]
pub fn parse_stdio_frame(raw: &str) -> Result<JsonRpcRequest, JsonRpcResponse> {
    let input = raw.trim_start_matches(['\r', '\n']);
    let first_line = input.lines().next().unwrap_or("");
    let is_header_frame = !input.trim_start().starts_with('{')
        && first_line.split_once(':').is_some_and(|(name, _)| {
            let trimmed_name = name.trim();
            !trimmed_name.is_empty()
                && trimmed_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });

    let json_slice = if is_header_frame {
        let Some((header_block, body)) = input
            .split_once("\r\n\r\n")
            .or_else(|| input.split_once("\n\n"))
        else {
            return Err(JsonRpcResponse::err(
                Value::Null,
                ERR_PARSE,
                "Malformed Content-Length stdio frame header",
                None,
            ));
        };

        let mut declared_len: Option<usize> = None;
        for line in header_block.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let Some((header_name, header_val)) = line.split_once(':') else {
                return Err(JsonRpcResponse::err(
                    Value::Null,
                    ERR_PARSE,
                    "Malformed stdio header line",
                    None,
                ));
            };
            if header_name.trim().eq_ignore_ascii_case("content-length") {
                let parsed: usize = header_val.trim().parse().map_err(|_| {
                    JsonRpcResponse::err(
                        Value::Null,
                        ERR_PARSE,
                        "Invalid Content-Length integer header",
                        None,
                    )
                })?;
                if declared_len.is_some_and(|prev| prev != parsed) {
                    return Err(JsonRpcResponse::err(
                        Value::Null,
                        ERR_PARSE,
                        "Conflicting duplicate Content-Length headers",
                        None,
                    ));
                }
                declared_len = Some(parsed);
            }
        }

        let Some(declared_len) = declared_len else {
            return Err(JsonRpcResponse::err(
                Value::Null,
                ERR_PARSE,
                "Missing Content-Length stdio frame header",
                None,
            ));
        };

        let Some(body_bytes) = body.as_bytes().get(..declared_len) else {
            return Err(JsonRpcResponse::err(
                Value::Null,
                ERR_PARSE,
                "Truncated Content-Length stdio frame payload",
                None,
            ));
        };

        std::str::from_utf8(body_bytes).map_err(|_| {
            JsonRpcResponse::err(
                Value::Null,
                ERR_PARSE,
                "Invalid UTF-8 boundary in Content-Length stdio frame payload",
                None,
            )
        })?
    } else {
        input.trim()
    };

    serde_json::from_str::<JsonRpcRequest>(json_slice).map_err(|err| {
        JsonRpcResponse::err(
            Value::Null,
            ERR_PARSE,
            format!("JSON-RPC parse error: {err}"),
            None,
        )
    })
}

/// Formats a JSON-RPC response for MCP `stdio` (Content-Length header or NDJSON line).
pub fn format_stdio_frame(resp: &JsonRpcResponse, use_content_length: bool) -> String {
    let body = serde_json::to_string(resp).unwrap_or_else(|_| "{}".to_string());
    if use_content_length {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
    } else {
        format!("{body}\n")
    }
}

/// Formats a JSON-RPC response as a Streamable HTTP/SSE `message` event.
pub fn format_sse_event(resp: &JsonRpcResponse) -> String {
    let body = serde_json::to_string(resp).unwrap_or_else(|_| "{}".to_string());
    format!("event: message\ndata: {body}\n\n")
}
