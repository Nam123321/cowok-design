// @story 11.2
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon HTTP/SSE Router for /api/v1/mcp/rpc, /api/v1/mcp/install, /api/v1/mcp/tools/toggle & /api/v1/mcp/external-activity)
use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use axum::{
    body::Bytes,
    extract::{Extension, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use mcp_gateway::{
    default_tool_policies, format_sse_event, install_host_config, list_supported_hosts,
    reject_symlinks_in_path, rollback_host_install, ActorKind, HostClientId, HostConfigFormat,
    HostInstallError, HostInstallOptions, JsonRpcRequest, McpGatewayEngine, PermissionTier,
    ToolPolicyRecord, WORKBENCH_APPROVAL_DEEP_LINK_BASE,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, HttpActorKind, LegacyAuthKind, VerifiedActorContext,
    VerifiedLegacyIngress,
};

const MAX_MCP_BODY_BYTES: usize = 262_144;
const DEFAULT_ACTIVITY_LIMIT: u32 = 50;
const MAX_ACTIVITY_LIMIT: u32 = 200;
const MAX_SCOPED_MCP_ENGINES: usize = 256;

type McpEngineScopeKey = (String, String, String);

struct ScopedMcpEngineStore {
    engines: HashMap<McpEngineScopeKey, Arc<McpGatewayEngine>>,
    insertion_order: VecDeque<McpEngineScopeKey>,
}

impl ScopedMcpEngineStore {
    fn new() -> Self {
        Self {
            engines: HashMap::new(),
            insertion_order: VecDeque::new(),
        }
    }

    fn get_or_create(&mut self, key: McpEngineScopeKey) -> Arc<McpGatewayEngine> {
        if let Some(existing) = self.engines.get(&key) {
            return Arc::clone(existing);
        }
        while self.engines.len() >= MAX_SCOPED_MCP_ENGINES {
            if let Some(evicted) = self.insertion_order.pop_front() {
                self.engines.remove(&evicted);
            } else {
                break;
            }
        }
        let project_label = key.2.clone();
        let engine = Arc::new(McpGatewayEngine::new(project_label));
        self.insertion_order.push_back(key.clone());
        self.engines.insert(key, Arc::clone(&engine));
        engine
    }

    fn sync_policies_for_workspace(&self, workspace_root: &str, policies: &[ToolPolicyRecord]) {
        for ((ws, _, _), engine) in &self.engines {
            if ws == workspace_root {
                engine.sync_tool_policies(policies);
            }
        }
    }
}

fn scoped_mcp_engines() -> &'static Mutex<ScopedMcpEngineStore> {
    static STORE: OnceLock<Mutex<ScopedMcpEngineStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(ScopedMcpEngineStore::new()))
}

fn resolve_scoped_mcp_engine(
    workspace_root: &Path,
    tenant_id: &str,
    project_id: Option<&str>,
) -> Arc<McpGatewayEngine> {
    let ws_key = workspace_root.to_string_lossy().into_owned();
    let tenant_trimmed = tenant_id.trim();
    let tenant_key = if tenant_trimmed.is_empty() {
        "local-default".to_string()
    } else {
        tenant_trimmed.to_string()
    };
    let project_key = project_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("default-workbench")
        .to_string();
    let scope_key = (ws_key, tenant_key, project_key.clone());
    match scoped_mcp_engines().lock() {
        Ok(mut store) => store.get_or_create(scope_key),
        Err(_) => Arc::new(McpGatewayEngine::new(project_key)),
    }
}

fn sync_workspace_mcp_policies(workspace_root: &Path, policies: &[ToolPolicyRecord]) {
    let ws_key = workspace_root.to_string_lossy().into_owned();
    if let Ok(store) = scoped_mcp_engines().lock() {
        store.sync_policies_for_workspace(&ws_key, policies);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolPolicyDto {
    pub tool_name: String,
    pub permission_tier: String,
    pub enabled: bool,
    pub mcp_callable: bool,
    pub description: String,
    pub locked_human_only: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpExternalActivityDto {
    pub id: String,
    pub project_id: Option<String>,
    pub actor_kind: String,
    pub host_client: String,
    pub method_name: String,
    pub tool_name: Option<String>,
    pub permission_tier: String,
    pub status: String,
    pub jsonrpc_code: Option<i32>,
    pub latency_ms: i64,
    pub idempotency_key: Option<String>,
    pub summary: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHostBackupDto {
    pub id: String,
    pub host_id: String,
    pub config_path: String,
    pub backup_path: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub rolled_back: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHostStatusDto {
    pub host_id: String,
    pub display_name: String,
    pub config_format: HostConfigFormat,
    pub default_relative_path: String,
    pub detected: bool,
    pub installed: bool,
    pub latest_backup_id: Option<String>,
    pub latest_after_sha256: Option<String>,
    pub last_installed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpExternalActivityData {
    pub policies: Vec<McpToolPolicyDto>,
    pub activities: Vec<McpExternalActivityDto>,
    pub hosts: Vec<McpHostStatusDto>,
    pub backups: Vec<McpHostBackupDto>,
    pub server_url: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpHostInstallRequest {
    pub host_id: String,
    #[serde(default = "default_install_action")]
    pub action: String,
    #[serde(default)]
    pub backup_id: Option<String>,
    #[serde(default)]
    pub config_path_override: Option<String>,
}

fn default_install_action() -> String {
    "install".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHostInstallData {
    pub action: String,
    pub host_id: String,
    pub display_name: String,
    pub config_path: String,
    pub backup_id: String,
    pub backup_path: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub rolled_back: bool,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolToggleRequest {
    pub tool_name: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub mcp_callable: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpExternalActivityQuery {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub host_client: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    field: Option<String>,
    details: Option<serde_json::Value>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
                field,
                request_id,
                details,
            },
        }),
    )
        .into_response()
}

fn ok_response<T: Serialize>(data: T, request_id: String) -> Response {
    (
        StatusCode::OK,
        Json(ApiSuccessEnvelope {
            ok: true,
            data,
            request_id,
        }),
    )
        .into_response()
}

fn resolve_principal<'a>(
    caller: Option<&'a VerifiedLegacyIngress>,
    principal_ext: Option<&'a FlowPrincipal>,
    headers: &HeaderMap,
    request_id: &str,
) -> Result<&'a FlowPrincipal, Response> {
    if let Some(v) = caller {
        if v.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
            return Err(error_response(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "MCP gateway routes require verified session token, loopback HMAC, or superadmin Bearer",
                None,
                None,
                request_id.to_string(),
            ));
        }
        return Ok(&v.principal);
    }
    if let Some(p) = principal_ext {
        return Ok(p);
    }
    Err(error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        None,
        request_id.to_string(),
    ))
}

fn require_write_role(principal: &FlowPrincipal, request_id: &str) -> Result<(), Response> {
    if !matches!(
        principal.role.trim().to_ascii_lowercase().as_str(),
        "editor" | "admin" | "owner"
    ) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Mutating MCP gateway configuration requires editor or admin role",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn ensure_default_policies_seeded(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for policy in default_tool_policies() {
        let enabled_int: i64 = if policy.enabled { 1 } else { 0 };
        let callable_int: i64 = if policy.permission_tier == PermissionTier::HumanOnly {
            0
        } else if policy.mcp_callable {
            1
        } else {
            0
        };
        tx.execute(
            "INSERT OR IGNORE INTO mcp_tool_policies (tool_name, permission_tier, enabled, mcp_callable, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                policy.tool_name,
                policy.permission_tier.as_str(),
                enabled_int,
                callable_int,
                policy.updated_at
            ],
        )?;
    }
    tx.commit()
}

fn load_tool_policies(conn: &Connection) -> Result<Vec<ToolPolicyRecord>, rusqlite::Error> {
    let desc_map: HashMap<String, String> = default_tool_policies()
        .into_iter()
        .map(|p| (p.tool_name, p.description))
        .collect();
    let mut stmt = conn.prepare(
        "SELECT tool_name, permission_tier, enabled, mcp_callable, updated_at
         FROM mcp_tool_policies
         ORDER BY
            CASE permission_tier
                WHEN 'READ_ONLY' THEN 1
                WHEN 'INSPECT' THEN 2
                WHEN 'REVISE_PROPOSE' THEN 3
                ELSE 4
            END,
            tool_name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let tool_name: String = row.get(0)?;
        let tier_raw: String = row.get(1)?;
        let enabled: i64 = row.get(2)?;
        let mcp_callable: i64 = row.get(3)?;
        let updated_at: i64 = row.get(4)?;
        let permission_tier = PermissionTier::parse(&tier_raw).unwrap_or(PermissionTier::HumanOnly);
        let description = desc_map
            .get(&tool_name)
            .cloned()
            .unwrap_or_else(|| format!("MCP tool {tool_name}"));
        Ok(ToolPolicyRecord {
            tool_name,
            permission_tier,
            enabled: enabled != 0,
            mcp_callable: permission_tier != PermissionTier::HumanOnly && mcp_callable != 0,
            description,
            updated_at,
        })
    })?;

    let mut out = Vec::new();
    for item in rows {
        out.push(item?);
    }
    Ok(out)
}

fn policy_to_dto(rec: &ToolPolicyRecord) -> McpToolPolicyDto {
    let locked = rec.permission_tier == PermissionTier::HumanOnly;
    McpToolPolicyDto {
        tool_name: rec.tool_name.clone(),
        permission_tier: rec.permission_tier.as_str().to_string(),
        enabled: rec.enabled,
        mcp_callable: !locked && rec.mcp_callable,
        description: rec.description.clone(),
        locked_human_only: locked,
        updated_at: rec.updated_at,
    }
}

struct ActivityInsert<'a> {
    project_id: Option<&'a str>,
    actor_kind: &'a str,
    host_client: &'a str,
    method_name: &'a str,
    tool_name: Option<&'a str>,
    permission_tier: &'a str,
    status: &'a str,
    jsonrpc_code: Option<i32>,
    latency_ms: i64,
    idempotency_key: Option<&'a str>,
    summary: &'a str,
}

fn insert_activity_log_immediate(
    conn: &mut Connection,
    entry: ActivityInsert<'_>,
) -> Result<(), rusqlite::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    tx.execute(
        "INSERT INTO mcp_external_activity_logs
            (id, project_id, actor_kind, host_client, method_name, tool_name, permission_tier,
             status, jsonrpc_code, latency_ms, idempotency_key, summary, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            id,
            entry.project_id,
            entry.actor_kind,
            entry.host_client,
            entry.method_name,
            entry.tool_name,
            entry.permission_tier,
            entry.status,
            entry.jsonrpc_code,
            entry.latency_ms,
            entry.idempotency_key,
            entry.summary,
            now
        ],
    )?;
    tx.commit()
}

fn resolve_home_base(workspace_root: &Path) -> PathBuf {
    std::env::var_os("COWOK_MCP_HOST_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join(".cowok-design").join("mcp-hosts"))
}

const BLOCKED_OVERRIDE_DIRS: &[&str] = &[".ssh", ".gnupg", ".git", ".cowok-design"];
const BLOCKED_OVERRIDE_FILES: &[&str] = &[
    "cargo.toml",
    "package.json",
    "package-lock.json",
    "tsconfig.json",
    ".zshrc",
    ".bashrc",
    ".profile",
    ".bash_profile",
    ".zprofile",
    "authorized_keys",
    "id_rsa",
    "id_ed25519",
];

fn validate_safe_override_target(
    host: HostClientId,
    candidate: &Path,
    raw: &str,
) -> Result<(), HostInstallError> {
    for comp in candidate.components() {
        if let Component::Normal(seg) = comp {
            let lower = seg.to_string_lossy().to_ascii_lowercase();
            if BLOCKED_OVERRIDE_DIRS.contains(&lower.as_str()) {
                return Err(HostInstallError::MalformedConfig {
                    path: raw.to_string(),
                    reason: format!(
                        "configPathOverride cannot target sensitive directory '{lower}'"
                    ),
                });
            }
        }
    }

    let Some(file_name) = candidate.file_name().and_then(|s| s.to_str()) else {
        return Err(HostInstallError::MalformedConfig {
            path: raw.to_string(),
            reason: "configPathOverride must point to a file path".to_string(),
        });
    };
    let lower_file = file_name.to_ascii_lowercase();
    if BLOCKED_OVERRIDE_FILES.contains(&lower_file.as_str()) {
        return Err(HostInstallError::MalformedConfig {
            path: raw.to_string(),
            reason: format!("configPathOverride cannot target protected file '{file_name}'"),
        });
    }

    let expected_ext = match host.config_format() {
        HostConfigFormat::Toml => "toml",
        HostConfigFormat::Json => "json",
    };
    let actual_ext = candidate
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase);
    if actual_ext.as_deref() != Some(expected_ext) {
        return Err(HostInstallError::MalformedConfig {
            path: raw.to_string(),
            reason: format!(
                "configPathOverride for host '{}' must use '.{expected_ext}' extension",
                host.as_str()
            ),
        });
    }
    Ok(())
}

fn resolve_safe_host_config_path(
    host: HostClientId,
    override_opt: Option<&str>,
    workspace_root: &Path,
) -> Result<PathBuf, HostInstallError> {
    if let Some(raw) = override_opt.map(str::trim).filter(|s| !s.is_empty()) {
        let candidate = PathBuf::from(raw);
        reject_symlinks_in_path(&candidate)?;
        let temp_dir = std::env::temp_dir();
        let home_dir = resolve_home_base(workspace_root);
        let in_allowed_root = candidate.starts_with(workspace_root)
            || candidate.starts_with(&temp_dir)
            || candidate.starts_with(&home_dir);
        if !candidate.is_absolute() || !in_allowed_root {
            return Err(HostInstallError::MalformedConfig {
                path: raw.to_string(),
                reason: "configPathOverride must be an absolute path inside workspace, temp dir, or home dir".to_string(),
            });
        }
        validate_safe_override_target(host, &candidate, raw)?;
        return Ok(candidate);
    }
    let home = resolve_home_base(workspace_root);
    let target = home.join(host.default_relative_path());
    reject_symlinks_in_path(&target)?;
    Ok(target)
}

fn resolve_verified_actor_kind(
    state: &DaemonAppState,
    headers: &HeaderMap,
    caller: Option<&VerifiedLegacyIngress>,
    actor_ext: Option<&VerifiedActorContext>,
) -> ActorKind {
    let explicit_mcp_header = headers
        .get("x-cowok-client-kind")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("mcp"));
    if explicit_mcp_header {
        return ActorKind::McpClient;
    }
    if let Some(ctx) = actor_ext {
        return match ctx.kind {
            HttpActorKind::HumanWorkbenchSession => ActorKind::HumanWorkbenchSession,
            HttpActorKind::AgentHmac | HttpActorKind::McpClient | HttpActorKind::SuperadminCli => {
                ActorKind::McpClient
            }
        };
    }
    if let Some(v) = caller {
        match v.kind {
            LegacyAuthKind::SessionToken | LegacyAuthKind::HmacV2 => {
                let is_human = headers
                    .get("x-cowok-human-session")
                    .or_else(|| headers.get("x-cowok-session-token"))
                    .and_then(|hv| hv.to_str().ok())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .and_then(|tok| state.auth.get_valid_human_session(tok))
                    .is_some_and(|hp| {
                        hp.tenant_id == v.principal.tenant_id && hp.user_id == v.principal.user_id
                    });
                if is_human {
                    return ActorKind::HumanWorkbenchSession;
                }
            }
            LegacyAuthKind::Bearer => return ActorKind::McpClient,
        }
    }
    ActorKind::McpClient
}

fn resolve_rpc_actor(headers: &HeaderMap, actor_ext: Option<&VerifiedActorContext>) -> ActorKind {
    let explicit_mcp_header = headers
        .get("x-cowok-client-kind")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("mcp"));
    if explicit_mcp_header {
        return ActorKind::McpClient;
    }
    match actor_ext.map(|a| a.kind) {
        Some(HttpActorKind::HumanWorkbenchSession) => ActorKind::HumanWorkbenchSession,
        // Fail closed: all external MCP / agent / bearer / session callers are ActorKind::McpClient
        _ => ActorKind::McpClient,
    }
}

fn extract_host_client_label(headers: &HeaderMap) -> String {
    headers
        .get("x-mcp-host-client")
        .or_else(|| headers.get("user-agent"))
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(64).collect())
        .unwrap_or_else(|| "external-mcp-client".to_string())
}

pub async fn handle_mcp_rpc(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    actor_ext: Option<Extension<VerifiedActorContext>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if body.len() > MAX_MCP_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "MCP JSON-RPC payload exceeds 256 KiB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let rpc_req: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_JSONRPC",
                &format!("Failed to parse JSON-RPC 2.0 request: {err}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let actor = resolve_rpc_actor(&headers, actor_ext.as_ref().map(|e| &e.0));
    let host_client = extract_host_client_label(&headers);
    let project_id = headers
        .get("x-cowok-project-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let engine = resolve_scoped_mcp_engine(
        state.registry.pool().workspace_root(),
        &principal.tenant_id,
        project_id.as_deref(),
    );
    if let Ok(mut conn) = state.registry.pool().connection() {
        let _ = ensure_default_policies_seeded(&mut conn);
        if let Ok(policies) = load_tool_policies(&conn) {
            engine.sync_tool_policies(&policies);
        }
    }

    let outcome = engine.dispatch(rpc_req, actor);

    if let Ok(mut conn) = state.registry.pool().connection() {
        let _ = insert_activity_log_immediate(
            &mut conn,
            ActivityInsert {
                project_id: project_id.as_deref(),
                actor_kind: actor.as_str(),
                host_client: &host_client,
                method_name: &outcome.method_name,
                tool_name: outcome.tool_name.as_deref(),
                permission_tier: outcome.permission_tier.as_str(),
                status: outcome.status.as_str(),
                jsonrpc_code: outcome.jsonrpc_code,
                latency_ms: outcome.latency_ms as i64,
                idempotency_key: outcome.idempotency_key.as_deref(),
                summary: &outcome.summary,
            },
        );
    }

    let wants_sse = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/event-stream"));

    if wants_sse {
        let sse_payload = format_sse_event(&outcome.response);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")],
            sse_payload,
        )
            .into_response();
    }

    (StatusCode::OK, Json(outcome.response)).into_response()
}

pub async fn handle_mcp_tools_toggle(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(principal, &request_id) {
        return resp;
    }
    if body.len() > MAX_MCP_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Request body too large",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: McpToolToggleRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(err) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid tool toggle payload: {err}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(err) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                &format!("SQLite connection error: {err}"),
                None,
                None,
                request_id,
            );
        }
    };

    if let Err(err) = ensure_default_policies_seeded(&mut conn) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &format!("Failed to seed MCP tool policies: {err}"),
            None,
            None,
            request_id,
        );
    }

    let existing: Option<(String, i64, i64)> = conn
        .query_row(
            "SELECT permission_tier, enabled, mcp_callable FROM mcp_tool_policies WHERE tool_name = ?1",
            params![req.tool_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .unwrap_or(None);

    let Some((tier_raw, cur_enabled, cur_callable)) = existing else {
        return error_response(
            StatusCode::NOT_FOUND,
            "TOOL_NOT_FOUND",
            &format!("MCP tool '{}' does not exist", req.tool_name),
            Some("toolName".to_string()),
            None,
            request_id,
        );
    };

    let tier = PermissionTier::parse(&tier_raw).unwrap_or(PermissionTier::HumanOnly);
    if tier == PermissionTier::HumanOnly
        && (req.mcp_callable == Some(true) || req.enabled == Some(true))
    {
        let deep_link = format!("{WORKBENCH_APPROVAL_DEEP_LINK_BASE}{}", req.tool_name);
        let _ = insert_activity_log_immediate(
            &mut conn,
            ActivityInsert {
                project_id: None,
                actor_kind: ActorKind::McpClient.as_str(),
                host_client: &extract_host_client_label(&headers),
                method_name: "mcp/tools/toggle",
                tool_name: Some(&req.tool_name),
                permission_tier: "HUMAN_ONLY",
                status: "DENIED_HUMAN_ONLY",
                jsonrpc_code: Some(mcp_gateway::ERR_HUMAN_APPROVAL_REQUIRED),
                latency_ms: 0,
                idempotency_key: None,
                summary: &format!(
                    "Rejected attempt to enable HUMAN_ONLY tool '{}' for MCP",
                    req.tool_name
                ),
            },
        );
        return error_response(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            "HUMAN_ONLY tools are permanently locked offline from ActorKind::McpClient (EC-LEARN-001)",
            Some("mcpCallable".to_string()),
            Some(serde_json::json!({
                "toolName": req.tool_name,
                "permissionTier": "HUMAN_ONLY",
                "workbenchDeepLink": deep_link,
            })),
            request_id,
        );
    }

    let next_enabled = req.enabled.unwrap_or(cur_enabled != 0);
    let next_callable = if tier == PermissionTier::HumanOnly {
        false
    } else {
        req.mcp_callable.unwrap_or(cur_callable != 0)
    };
    let now = Utc::now().timestamp_millis();

    let update_res = (|| -> Result<(), rusqlite::Error> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE mcp_tool_policies SET enabled = ?1, mcp_callable = ?2, updated_at = ?3 WHERE tool_name = ?4",
            params![
                if next_enabled { 1 } else { 0 },
                if next_callable { 1 } else { 0 },
                now,
                req.tool_name
            ],
        )?;
        tx.commit()
    })();

    if let Err(err) = update_res {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &format!("Failed to update tool policy: {err}"),
            None,
            None,
            request_id,
        );
    }

    let policies = load_tool_policies(&conn).unwrap_or_default();
    sync_workspace_mcp_policies(state.registry.pool().workspace_root(), &policies);
    let updated = policies
        .iter()
        .find(|p| p.tool_name == req.tool_name)
        .map(policy_to_dto)
        .unwrap_or(McpToolPolicyDto {
            tool_name: req.tool_name,
            permission_tier: tier.as_str().to_string(),
            enabled: next_enabled,
            mcp_callable: next_callable,
            description: String::new(),
            locked_human_only: tier == PermissionTier::HumanOnly,
            updated_at: now,
        });

    ok_response(updated, request_id)
}

pub async fn handle_mcp_install(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    actor_ext: Option<Extension<VerifiedActorContext>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(principal, &request_id) {
        return resp;
    }
    if body.len() > MAX_MCP_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Host install request payload exceeds 256 KiB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let logged_actor = resolve_verified_actor_kind(
        &state,
        &headers,
        caller.as_ref().map(|e| &e.0),
        actor_ext.as_ref().map(|e| &e.0),
    );

    let req: McpHostInstallRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(err) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid host install request: {err}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let host = match HostClientId::parse(&req.host_id) {
        Ok(h) => h,
        Err(err) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNSUPPORTED_HOST",
                &err.to_string(),
                Some("hostId".to_string()),
                None,
                request_id,
            );
        }
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(err) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                &format!("SQLite connection error: {err}"),
                None,
                None,
                request_id,
            );
        }
    };

    let workspace_root = state.registry.pool().workspace_root().to_path_buf();
    let backup_dir = workspace_root
        .join(".cowok-design")
        .join("mcp-host-backups");

    let action = req.action.trim().to_ascii_lowercase();
    if action == "rollback" {
        let backup_row: Option<(String, String, String, String, String)> =
            if let Some(ref bid) = req.backup_id {
                conn.query_row(
                    "SELECT id, config_path, backup_path, before_sha256, after_sha256
                 FROM mcp_host_install_backups
                 WHERE id = ?1 AND host_id = ?2 AND rolled_back = 0",
                    params![bid, host.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .unwrap_or(None)
            } else {
                conn.query_row(
                    "SELECT id, config_path, backup_path, before_sha256, after_sha256
                 FROM mcp_host_install_backups
                 WHERE host_id = ?1 AND rolled_back = 0
                 ORDER BY created_at DESC LIMIT 1",
                    params![host.as_str()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()
                .unwrap_or(None)
            };

        let Some((backup_id, cfg_path, bkp_path, before_sha, after_sha)) = backup_row else {
            return error_response(
                StatusCode::NOT_FOUND,
                "BACKUP_NOT_FOUND",
                &format!(
                    "No active backup available to roll back for host '{}'",
                    host.as_str()
                ),
                Some("backupId".to_string()),
                None,
                request_id,
            );
        };

        let rb = match rollback_host_install(
            host,
            Path::new(&cfg_path),
            Path::new(&bkp_path),
            &before_sha,
        ) {
            Ok(r) => r,
            Err(err) => return map_host_install_error(err, request_id),
        };

        let mark_res = (|| -> Result<(), rusqlite::Error> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE mcp_host_install_backups SET rolled_back = 1 WHERE id = ?1",
                params![backup_id],
            )?;
            tx.commit()
        })();
        if let Err(err) = mark_res {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                &format!("Failed to mark backup rolled back: {err}"),
                None,
                None,
                request_id,
            );
        }

        let _ = insert_activity_log_immediate(
            &mut conn,
            ActivityInsert {
                project_id: None,
                actor_kind: logged_actor.as_str(),
                host_client: host.as_str(),
                method_name: "mcp/install/rollback",
                tool_name: None,
                permission_tier: "REVISE_PROPOSE",
                status: "OK",
                jsonrpc_code: None,
                latency_ms: 0,
                idempotency_key: None,
                summary: &format!(
                    "Rolled back {} MCP config from backup {backup_id}",
                    host.display_name()
                ),
            },
        );

        return ok_response(
            McpHostInstallData {
                action: "rollback".to_string(),
                host_id: host.as_str().to_string(),
                display_name: host.display_name().to_string(),
                config_path: rb.config_path,
                backup_id,
                backup_path: rb.backup_path,
                before_sha256: before_sha,
                after_sha256: after_sha,
                rolled_back: true,
                timestamp: rb.rolled_back_at,
            },
            request_id,
        );
    }

    if action != "install" {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ACTION",
            "action must be 'install' or 'rollback'",
            Some("action".to_string()),
            None,
            request_id,
        );
    }

    let config_path = match resolve_safe_host_config_path(
        host,
        req.config_path_override.as_deref(),
        &workspace_root,
    ) {
        Ok(p) => p,
        Err(err) => return map_host_install_error(err, request_id),
    };

    let server_url = format!("http://{}:{}/api/v1/mcp/rpc", state.host, state.port);
    let receipt = match install_host_config(&HostInstallOptions {
        host,
        config_path,
        backup_dir,
        server_command: "cowok".to_string(),
        server_args: vec!["mcp".to_string(), "serve".to_string()],
        server_url,
    }) {
        Ok(r) => r,
        Err(err) => return map_host_install_error(err, request_id),
    };

    let persist_res = (|| -> Result<(), rusqlite::Error> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO mcp_host_install_backups
                (id, host_id, config_path, backup_path, before_sha256, after_sha256, rolled_back, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                receipt.backup_id,
                receipt.host_id,
                receipt.config_path,
                receipt.backup_path,
                receipt.before_sha256,
                receipt.after_sha256,
                receipt.installed_at
            ],
        )?;
        tx.commit()
    })();
    if let Err(err) = persist_res {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &format!("Failed to record host install backup: {err}"),
            None,
            None,
            request_id,
        );
    }

    let _ = insert_activity_log_immediate(
        &mut conn,
        ActivityInsert {
            project_id: None,
            actor_kind: logged_actor.as_str(),
            host_client: host.as_str(),
            method_name: "mcp/install",
            tool_name: None,
            permission_tier: "REVISE_PROPOSE",
            status: "OK",
            jsonrpc_code: None,
            latency_ms: 0,
            idempotency_key: None,
            summary: &format!(
                "Installed Cowok-Design MCP into {} (backup {})",
                host.display_name(),
                &receipt.before_sha256[..12.min(receipt.before_sha256.len())]
            ),
        },
    );

    ok_response(
        McpHostInstallData {
            action: "install".to_string(),
            host_id: receipt.host_id,
            display_name: receipt.display_name,
            config_path: receipt.config_path,
            backup_id: receipt.backup_id,
            backup_path: receipt.backup_path,
            before_sha256: receipt.before_sha256,
            after_sha256: receipt.after_sha256,
            rolled_back: false,
            timestamp: receipt.installed_at,
        },
        request_id,
    )
}

fn map_host_install_error(err: HostInstallError, request_id: String) -> Response {
    match err {
        HostInstallError::SymlinkRejected { path } => error_response(
            StatusCode::FORBIDDEN,
            "SYMLINK_REJECTED",
            &format!("Refused symlink path '{path}' (O_NOFOLLOW)"),
            Some("configPathOverride".to_string()),
            None,
            request_id,
        ),
        HostInstallError::UnsupportedHost { host_id } => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_HOST",
            &format!("Unsupported host '{host_id}'"),
            Some("hostId".to_string()),
            None,
            request_id,
        ),
        HostInstallError::MalformedConfig { path, reason } => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "MALFORMED_HOST_CONFIG",
            &format!("Malformed host config at '{path}': {reason}"),
            Some("configPathOverride".to_string()),
            None,
            request_id,
        ),
        HostInstallError::ChecksumMismatch {
            backup_path,
            expected,
            actual,
        } => error_response(
            StatusCode::CONFLICT,
            "BACKUP_CHECKSUM_MISMATCH",
            &format!(
                "Backup '{backup_path}' checksum mismatch (expected {expected}, got {actual})"
            ),
            Some("backupId".to_string()),
            None,
            request_id,
        ),
        HostInstallError::BackupNotFound { path } => error_response(
            StatusCode::NOT_FOUND,
            "BACKUP_NOT_FOUND",
            &format!("Backup file not found: '{path}'"),
            Some("backupId".to_string()),
            None,
            request_id,
        ),
        HostInstallError::Io(io_err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "HOST_INSTALL_IO_ERROR",
            &format!("Host config I/O failure: {io_err}"),
            None,
            None,
            request_id,
        ),
    }
}

pub async fn handle_mcp_external_activity(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    headers: HeaderMap,
    Query(query): Query<McpExternalActivityQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    if let Err(resp) = resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        return resp;
    }

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(err) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                &format!("SQLite connection error: {err}"),
                None,
                None,
                request_id,
            );
        }
    };

    if let Err(err) = ensure_default_policies_seeded(&mut conn) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &format!("Failed to seed MCP tool policies: {err}"),
            None,
            None,
            request_id,
        );
    }

    let policies_raw = load_tool_policies(&conn).unwrap_or_default();
    sync_workspace_mcp_policies(state.registry.pool().workspace_root(), &policies_raw);
    let policies: Vec<McpToolPolicyDto> = policies_raw.iter().map(policy_to_dto).collect();

    let limit = query
        .limit
        .unwrap_or(DEFAULT_ACTIVITY_LIMIT)
        .clamp(1, MAX_ACTIVITY_LIMIT) as i64;
    let host_filter = query
        .host_client
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let status_filter = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let activities =
        query_activity_logs(&conn, limit, host_filter, status_filter).unwrap_or_default();
    let backups = query_host_backups(&conn).unwrap_or_default();

    let home_base = resolve_home_base(state.registry.pool().workspace_root());
    let supported = list_supported_hosts(Some(&home_base));
    let hosts: Vec<McpHostStatusDto> = supported
        .into_iter()
        .map(|target| {
            let latest_active = backups
                .iter()
                .find(|b| b.host_id == target.host_id && !b.rolled_back);
            McpHostStatusDto {
                host_id: target.host_id,
                display_name: target.display_name,
                config_format: target.config_format,
                default_relative_path: target.default_relative_path,
                detected: target.detected,
                installed: latest_active.is_some(),
                latest_backup_id: latest_active.map(|b| b.id.clone()),
                latest_after_sha256: latest_active.map(|b| b.after_sha256.clone()),
                last_installed_at: latest_active.map(|b| b.created_at),
            }
        })
        .collect();

    let server_url = format!("http://{}:{}/api/v1/mcp/rpc", state.host, state.port);
    ok_response(
        McpExternalActivityData {
            policies,
            activities,
            hosts,
            backups,
            server_url,
            generated_at: Utc::now().to_rfc3339(),
        },
        request_id,
    )
}

fn query_activity_logs(
    conn: &Connection,
    limit: i64,
    host_filter: Option<&str>,
    status_filter: Option<&str>,
) -> Result<Vec<McpExternalActivityDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, actor_kind, host_client, method_name, tool_name,
                permission_tier, status, jsonrpc_code, latency_ms, idempotency_key, summary, created_at
         FROM mcp_external_activity_logs
         WHERE (?1 IS NULL OR host_client = ?1)
           AND (?2 IS NULL OR status = ?2)
         ORDER BY created_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![host_filter, status_filter, limit], |row| {
        Ok(McpExternalActivityDto {
            id: row.get(0)?,
            project_id: row.get(1)?,
            actor_kind: row.get(2)?,
            host_client: row.get(3)?,
            method_name: row.get(4)?,
            tool_name: row.get(5)?,
            permission_tier: row.get(6)?,
            status: row.get(7)?,
            jsonrpc_code: row.get(8)?,
            latency_ms: row.get(9)?,
            idempotency_key: row.get(10)?,
            summary: row.get(11)?,
            created_at: row.get(12)?,
        })
    })?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item?);
    }
    Ok(out)
}

fn query_host_backups(conn: &Connection) -> Result<Vec<McpHostBackupDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, host_id, config_path, backup_path, before_sha256, after_sha256, rolled_back, created_at
         FROM mcp_host_install_backups
         ORDER BY created_at DESC
         LIMIT 50",
    )?;
    let rows = stmt.query_map([], |row| {
        let rb: i64 = row.get(6)?;
        Ok(McpHostBackupDto {
            id: row.get(0)?,
            host_id: row.get(1)?,
            config_path: row.get(2)?,
            backup_path: row.get(3)?,
            before_sha256: row.get(4)?,
            after_sha256: row.get(5)?,
            rolled_back: rb != 0,
            created_at: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item?);
    }
    Ok(out)
}

pub fn mcp_gateway_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/mcp/rpc", post(handle_mcp_rpc))
        .route("/api/v1/mcp/install", post(handle_mcp_install))
        .route("/api/v1/mcp/tools/toggle", post(handle_mcp_tools_toggle))
        .route(
            "/api/v1/mcp/external-activity",
            get(handle_mcp_external_activity),
        )
}

pub fn build_mcp_gateway_router() -> Router<DaemonAppState> {
    mcp_gateway_router()
}
