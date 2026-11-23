// @story 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::path::PathBuf;

use axum::{
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use plugin_host::{
    execute_in_ephemeral_tool_jail, list_gate_failure_ledger_scoped, list_user_skills_scoped,
    package_and_register_user_skill, record_gate_failure_and_evaluate, run_builtin_de_suite,
    ActorKind as DomainActorKind, ArchifyBfsError, BuiltInDeTool, DeRunReport, DeSuiteInput,
    EphemeralRuntime, EphemeralToolRequest, GateFailureLedgerEntry, ImportUserSkillRequest,
    LearnFilterError, PackagedUserSkillReceipt, RecordGateFailureInput, StagedJailFile,
    TokenPairInput, ToolJailAuditReceipt, ToolJailError, UserSkillPackagerError,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::plugin_store::verify_project_scope;
use uuid::Uuid;

use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, FlowPrincipal, HttpActorKind, LegacyAuthKind,
    VerifiedActorContext, VerifiedLegacyIngress,
};
use crate::routes::DaemonAppState;

const MAX_ARTIFACT_CONTENT_BYTES: usize = 512 * 1024;

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

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn ok_resp<T: Serialize>(data: T, request_id: String) -> Response {
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

fn err_resp(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    field: Option<&str>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.into(),
                field: field.map(str::to_string),
                request_id,
                details: None,
            },
        }),
    )
        .into_response()
}

fn resolve_caller(
    state: &DaemonAppState,
    headers: &HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    request_id: &str,
) -> Result<(FlowPrincipal, VerifiedActorContext), Response> {
    if let (Some(Extension(principal)), Some(Extension(actor))) = (flow_principal, actor_ctx) {
        return Ok((principal, actor));
    }
    if let Some(Extension(legacy)) = legacy_ingress {
        // C-01: Defense-in-depth — reject raw legacy Bearer tokens unless verified superadmin bearer
        if legacy.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
            return Err(err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Legacy bearer authentication is not permitted on DE runtime endpoints",
                None,
                request_id.to_string(),
            ));
        }

        let principal = legacy.principal;
        let human_header = headers
            .get("x-cowok-human-session")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let session_header = headers
            .get("x-cowok-session-token")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let is_human = human_header
            .and_then(|tok| state.auth.get_valid_human_session(tok))
            .or_else(|| session_header.and_then(|tok| state.auth.get_valid_human_session(tok)))
            .is_some_and(|hp| {
                hp.tenant_id == principal.tenant_id && hp.user_id == principal.user_id
            });

        let kind = if is_human {
            HttpActorKind::HumanWorkbenchSession
        } else if headers
            .get("x-cowok-client-kind")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("mcp"))
            || principal.user_id.to_ascii_lowercase().starts_with("mcp-")
        {
            HttpActorKind::McpClient
        } else {
            HttpActorKind::AgentHmac
        };
        return Ok((principal, VerifiedActorContext { kind }));
    }
    Err(err_resp(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        request_id.to_string(),
    ))
}

fn to_domain_actor(kind: HttpActorKind) -> DomainActorKind {
    match kind {
        HttpActorKind::HumanWorkbenchSession => DomainActorKind::HumanWorkbenchSession,
        HttpActorKind::AgentHmac => DomainActorKind::AgentHmac,
        HttpActorKind::McpClient => DomainActorKind::McpClient,
        HttpActorKind::SuperadminCli => DomainActorKind::SuperadminCli,
    }
}

fn map_scope_err(err: domain::HarnessError, request_id: String) -> Response {
    match err {
        domain::HarnessError::ProjectNotFound { project_id } => err_resp(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            format!("Project {project_id} not found for tenant"),
            Some("projectId"),
            request_id,
        ),
        domain::HarnessError::SqliteBusy { .. } => err_resp(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            "Storage is temporarily busy; retry shortly",
            None,
            request_id,
        ),
        domain::HarnessError::PathTraversalDenied { .. } => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PATH_JAIL_VIOLATION",
            "Target path escapes project workspace boundary",
            Some("targetArtifactPath"),
            request_id,
        ),
        _ => err_resp(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "Request validation failed",
            None,
            request_id,
        ),
    }
}

/// H-01: Resolves the project's own workspace_root from SQLite (never falls back across tenants).
fn resolve_project_workspace_root(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
    daemon_fallback_root: &std::path::Path,
) -> Result<PathBuf, domain::HarnessError> {
    let raw_root: Option<Option<String>> = conn
        .query_row(
            "SELECT workspace_root FROM projects
             WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| domain::HarnessError::ValidationFailed {
            reason: "Failed to query project workspace scope".to_string(),
        })?;

    let Some(ws_opt) = raw_root else {
        return Err(domain::HarnessError::ProjectNotFound {
            project_id: project_id.to_string(),
        });
    };

    let candidate = match ws_opt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => {
            let p = PathBuf::from(raw);
            if p.is_absolute() {
                p
            } else {
                if p.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    return Err(domain::HarnessError::PathTraversalDenied {
                        path: project_id.to_string(),
                    });
                }
                daemon_fallback_root.join(p)
            }
        }
        None => daemon_fallback_root.to_path_buf(),
    };

    if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
        if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
            return Err(domain::HarnessError::PathTraversalDenied {
                path: project_id.to_string(),
            });
        }
    }
    Ok(candidate)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpRunBuiltInDeRequest {
    pub project_id: String,
    pub tools: Vec<BuiltInDeTool>,
    pub target_artifact_path: String,
    #[serde(default)]
    pub content_override: Option<String>,
    #[serde(default)]
    pub token_pairs: Vec<TokenPairInput>,
}

pub async fn handle_run_builtin_de(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    Json(body): Json<HttpRunBuiltInDeRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, _) = match resolve_caller(
        &state,
        &headers,
        flow_principal,
        actor_ctx,
        legacy_ingress,
        &request_id,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };

    let project_id = body.project_id.trim().to_string();
    if project_id.is_empty() || body.tools.is_empty() {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId and at least one DE tool are required",
            Some("tools"),
            request_id,
        );
    }
    if let Some(ref content) = body.content_override {
        if content.len() > MAX_ARTIFACT_CONTENT_BYTES {
            return err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "contentOverride exceeds 512 KiB limit",
                Some("contentOverride"),
                request_id,
            );
        }
    }

    let pool = state.registry.pool().clone();
    let daemon_root = pool.workspace_root().to_path_buf();
    let tenant_id = principal.tenant_id.clone();

    let run_res = tokio::task::spawn_blocking(
        move || -> Result<DeRunReport, Result<domain::HarnessError, ArchifyBfsError>> {
            let conn = pool.connection().map_err(Ok)?;
            let project_root =
                resolve_project_workspace_root(&conn, &tenant_id, &project_id, &daemon_root)
                    .map_err(Ok)?;
            let input = DeSuiteInput {
                workspace_root: &project_root,
                target_artifact_path: body.target_artifact_path.trim(),
                content_override: body.content_override.as_deref(),
                tools: &body.tools,
                token_pairs: &body.token_pairs,
            };
            run_builtin_de_suite(&input).map_err(Err)
        },
    )
    .await;

    match run_res {
        Ok(Ok(report)) => ok_resp(report, request_id),
        Ok(Err(Ok(herr))) => map_scope_err(herr, request_id),
        Ok(Err(Err(ArchifyBfsError::SymlinkRejected(_))))
        | Ok(Err(Err(ArchifyBfsError::PathTraversal(_)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PATH_JAIL_VIOLATION",
            "Target artifact path rejected by workspace path jail",
            Some("targetArtifactPath"),
            request_id,
        ),
        Ok(Err(Err(ArchifyBfsError::InvalidIr(msg)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            msg,
            Some("targetArtifactPath"),
            request_id,
        ),
        Ok(Err(Err(ArchifyBfsError::Io(_)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Target artifact could not be read within project workspace",
            Some("targetArtifactPath"),
            request_id,
        ),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "DE runtime worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUserSkillsQuery {
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserSkillsListData {
    items: Vec<PackagedUserSkillReceipt>,
}

pub async fn handle_list_user_skills(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<ListUserSkillsQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, _) = match resolve_caller(
        &state,
        &headers,
        flow_principal,
        actor_ctx,
        legacy_ingress,
        &request_id,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };

    let pool = state.registry.pool().clone();
    let tenant_id = principal.tenant_id.clone();
    let project_id = query
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let res = tokio::task::spawn_blocking(
        move || -> Result<Vec<PackagedUserSkillReceipt>, Result<domain::HarnessError, UserSkillPackagerError>> {
            let conn = pool.connection().map_err(Ok)?;
            if let Some(ref pid) = project_id {
                verify_project_scope(&conn, &tenant_id, pid).map_err(Ok)?;
            }
            list_user_skills_scoped(&conn, &tenant_id, project_id.as_deref()).map_err(Err)
        },
    )
    .await;

    match res {
        Ok(Ok(items)) => ok_resp(UserSkillsListData { items }, request_id),
        Ok(Err(Ok(herr))) => map_scope_err(herr, request_id),
        Ok(Err(Err(_))) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            "Failed to query registered user skills",
            None,
            request_id,
        ),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "User skill query worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpImportRepoRequest {
    #[serde(default)]
    pub project_id: Option<String>,
    pub skill_slug: String,
    pub repo_source_url: String,
    pub license_spdx: String,
    pub description: String,
    pub body_markdown: String,
    #[serde(default)]
    pub attached_scripts: Vec<String>,
}

pub async fn handle_import_user_skill_repo(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    Json(body): Json<HttpImportRepoRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, _) = match resolve_caller(
        &state,
        &headers,
        flow_principal,
        actor_ctx,
        legacy_ingress,
        &request_id,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let tenant_id = principal.tenant_id.clone();
    let trimmed_project_id = body
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let res = tokio::task::spawn_blocking(
        move || -> Result<PackagedUserSkillReceipt, Result<domain::HarnessError, UserSkillPackagerError>> {
            let conn = pool.connection().map_err(Ok)?;
            if let Some(ref pid) = trimmed_project_id {
                verify_project_scope(&conn, &tenant_id, pid).map_err(Ok)?;
            }
            package_and_register_user_skill(
                &conn,
                &tenant_id,
                &ImportUserSkillRequest {
                    project_id: trimmed_project_id,
                    skill_slug: body.skill_slug,
                    repo_source_url: body.repo_source_url,
                    license_spdx: body.license_spdx,
                    description: body.description,
                    body_markdown: body.body_markdown,
                    attached_scripts: body.attached_scripts,
                },
            )
            .map_err(Err)
        },
    )
    .await;

    match res {
        Ok(Ok(receipt)) => ok_resp(receipt, request_id),
        Ok(Err(Ok(herr))) => map_scope_err(herr, request_id),
        Ok(Err(Err(UserSkillPackagerError::SsrfBlocked(msg)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SSRF_URL_BLOCKED",
            msg,
            Some("repoSourceUrl"),
            request_id,
        ),
        Ok(Err(Err(UserSkillPackagerError::LicenseRejected { license }))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LICENSE_REJECTED",
            format!("License '{license}' is disallowed for skill import"),
            Some("licenseSpdx"),
            request_id,
        ),
        Ok(Err(Err(UserSkillPackagerError::SecurityRejected(msg)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SECURITY_SCAN_REJECTED",
            msg,
            Some("bodyMarkdown"),
            request_id,
        ),
        Ok(Err(Err(UserSkillPackagerError::InvalidSlug(msg)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            msg,
            Some("skillSlug"),
            request_id,
        ),
        Ok(Err(Err(UserSkillPackagerError::Storage(_)))) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            "Failed to persist packaged user skill",
            None,
            request_id,
        ),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "User skill packager worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpExecuteEphemeralToolRequest {
    pub project_id: String,
    pub tool_name: String,
    pub runtime: EphemeralRuntime,
    pub script_source: String,
    #[serde(default = "default_ephemeral_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub staged_files: Vec<StagedJailFile>,
}

fn default_ephemeral_timeout_ms() -> u64 {
    3_000
}

/// Strictly `HUMAN_ONLY` endpoint (`ActorKind::HumanWorkbenchSession`).
/// Any Agent HMAC, MCP client, or CLI bearer actor receives `403 HUMAN_APPROVAL_REQUIRED`.
pub async fn handle_execute_ephemeral_tool(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    Json(body): Json<HttpExecuteEphemeralToolRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, actor) = match resolve_caller(
        &state,
        &headers,
        flow_principal,
        actor_ctx,
        legacy_ingress,
        &request_id,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };

    if actor.kind != HttpActorKind::HumanWorkbenchSession {
        return err_resp(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            "Ephemeral tool execution strictly requires HumanWorkbenchSession (HUMAN_ONLY)",
            None,
            request_id,
        );
    }
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let jail_parent = pool
        .workspace_root()
        .join(".cowok-design")
        .join("ephemeral-jail");
    let tenant_id = principal.tenant_id.clone();
    let domain_actor = to_domain_actor(actor.kind);
    let project_id = body.project_id.trim().to_string();

    let res = tokio::task::spawn_blocking(
        move || -> Result<ToolJailAuditReceipt, Result<domain::HarnessError, ToolJailError>> {
            let conn = pool.connection().map_err(Ok)?;
            verify_project_scope(&conn, &tenant_id, &project_id).map_err(Ok)?;
            execute_in_ephemeral_tool_jail(
                domain_actor,
                &jail_parent,
                &EphemeralToolRequest {
                    tool_name: body.tool_name,
                    runtime: body.runtime,
                    script_source: body.script_source,
                    timeout_ms: body.timeout_ms,
                    staged_files: body.staged_files,
                },
            )
            .map_err(Err)
        },
    )
    .await;

    match res {
        Ok(Ok(receipt)) => ok_resp(receipt, request_id),
        Ok(Err(Ok(herr))) => map_scope_err(herr, request_id),
        Ok(Err(Err(ToolJailError::HumanApprovalRequired { .. }))) => err_resp(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            "Ephemeral tool execution strictly requires HumanWorkbenchSession",
            None,
            request_id,
        ),
        Ok(Err(Err(ToolJailError::CapabilityBlocked { rule_id, reason }))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "AST_CAPABILITY_BLOCKED",
            format!("[{rule_id}] {reason}"),
            Some("scriptSource"),
            request_id,
        ),
        Ok(Err(Err(ToolJailError::TimeoutKilled { pgid, timeout_ms }))) => err_resp(
            StatusCode::REQUEST_TIMEOUT,
            "JAIL_TIMEOUT_KILLED",
            format!("Process group -{pgid} killed after {timeout_ms}ms timeout"),
            Some("timeoutMs"),
            request_id,
        ),
        Ok(Err(Err(ToolJailError::SymlinkOrHardlinkRejected { .. })))
        | Ok(Err(Err(ToolJailError::PathTraversal { .. }))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PATH_JAIL_VIOLATION",
            "Unsafe path or symlink/hardlink rejected inside ephemeral jail",
            None,
            request_id,
        ),
        Ok(Err(Err(ToolJailError::ScriptTooLarge { actual, max }))) => err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            format!("Script size {actual} exceeds max {max} bytes"),
            Some("scriptSource"),
            request_id,
        ),
        Ok(Err(Err(ToolJailError::Runtime(_)))) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Ephemeral tool jail runtime execution failed",
            None,
            request_id,
        ),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Ephemeral tool worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnLedgerQuery {
    pub project_id: String,
    #[serde(default)]
    pub qualifying_only: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LearnLedgerListData {
    project_id: String,
    entries: Vec<GateFailureLedgerEntry>,
    min_hit_count: i64,
    promote_slpr_threshold: f64,
    retire_slpr_threshold: f64,
}

pub async fn handle_list_learn_ledger(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<LearnLedgerQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, _) = match resolve_caller(
        &state,
        &headers,
        flow_principal,
        actor_ctx,
        legacy_ingress,
        &request_id,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };

    let project_id = query.project_id.trim().to_string();
    if project_id.is_empty() {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId query parameter is required",
            Some("projectId"),
            request_id,
        );
    }

    let pool = state.registry.pool().clone();
    let tenant_id = principal.tenant_id.clone();
    let qualifying_only = query.qualifying_only;
    let project_id_for_query = project_id.clone();

    let res = tokio::task::spawn_blocking(
        move || -> Result<Vec<GateFailureLedgerEntry>, Result<domain::HarnessError, LearnFilterError>> {
            let conn = pool.connection().map_err(Ok)?;
            verify_project_scope(&conn, &tenant_id, &project_id_for_query).map_err(Ok)?;
            list_gate_failure_ledger_scoped(&conn, &tenant_id, &project_id_for_query, qualifying_only)
                .map_err(Err)
        },
    )
    .await;

    match res {
        Ok(Ok(entries)) => ok_resp(
            LearnLedgerListData {
                project_id,
                entries,
                min_hit_count: plugin_host::LEARN_MIN_HIT_COUNT,
                promote_slpr_threshold: plugin_host::LEARN_PROMOTE_SLPR_THRESHOLD,
                retire_slpr_threshold: plugin_host::LEARN_RETIRE_SLPR_THRESHOLD,
            },
            request_id,
        ),
        Ok(Err(Ok(herr))) => map_scope_err(herr, request_id),
        Ok(Err(Err(_))) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            "Failed to query LEARN failure ledger",
            None,
            request_id,
        ),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "LEARN ledger worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpRecordLearnLedgerRequest {
    pub project_id: String,
    pub rule_id: String,
    pub failure_message: String,
    pub slpr_score: f64,
    #[serde(default)]
    pub source_evidence: Option<String>,
}

pub async fn handle_record_learn_ledger(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    actor_ctx: Option<Extension<VerifiedActorContext>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    Json(body): Json<HttpRecordLearnLedgerRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, _) = match resolve_caller(
        &state,
        &headers,
        flow_principal,
        actor_ctx,
        legacy_ingress,
        &request_id,
    ) {
        Ok(pair) => pair,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let tenant_id = principal.tenant_id.clone();
    let project_id = body.project_id.trim().to_string();

    let res = tokio::task::spawn_blocking(
        move || -> Result<GateFailureLedgerEntry, Result<domain::HarnessError, LearnFilterError>> {
            let conn = pool.connection().map_err(Ok)?;
            verify_project_scope(&conn, &tenant_id, &project_id).map_err(Ok)?;
            record_gate_failure_and_evaluate(
                &conn,
                &tenant_id,
                &RecordGateFailureInput {
                    project_id,
                    rule_id: body.rule_id,
                    failure_message: body.failure_message,
                    slpr_score: body.slpr_score,
                    source_evidence: body.source_evidence,
                },
            )
            .map_err(Err)
        },
    )
    .await;

    match res {
        Ok(Ok(entry)) => ok_resp(entry, request_id),
        Ok(Err(Ok(herr))) => map_scope_err(herr, request_id),
        Ok(Err(Err(LearnFilterError::SmugglingBlocked { code, reason }))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LEARN_SMUGGLING_BLOCKED",
            format!("[{code}] {reason}"),
            Some("failureMessage"),
            request_id,
        ),
        Ok(Err(Err(LearnFilterError::ScopeCapacityExceeded { current, cap }))) => err_resp(
            StatusCode::CONFLICT,
            "LEARN_SCOPE_CAPACITY_EXCEEDED",
            format!("Active LEARN cards {current} reached cap {cap}"),
            None,
            request_id,
        ),
        Ok(Err(Err(LearnFilterError::InvalidInput(msg)))) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            msg,
            None,
            request_id,
        ),
        Ok(Err(Err(LearnFilterError::Storage(_)))) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            "Failed to persist LEARN gate failure ledger entry",
            None,
            request_id,
        ),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "LEARN ledger worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// Builds the Story 08.5 Split DE+DR Plugin Runtime, Ephemeral Tool Jail & System-1 LEARN router.
pub fn build_de_runtime_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/de/run", post(handle_run_builtin_de))
        .route(
            "/api/v1/plugins/import-repo",
            get(handle_list_user_skills).post(handle_import_user_skill_repo),
        )
        .route(
            "/api/v1/plugins/ephemeral-tools/execute",
            post(handle_execute_ephemeral_tool),
        )
        .route(
            "/api/v1/learn/ledger",
            get(handle_list_learn_ledger).post(handle_record_learn_ledger),
        )
        .route(
            "/api/v1/learn/ledger/record",
            post(handle_record_learn_ledger),
        )
}
