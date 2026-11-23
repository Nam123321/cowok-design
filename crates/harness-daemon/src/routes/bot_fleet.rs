// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon /api/v1/bot-fleet/execute & /api/v1/bot-fleet/providers/status)
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bot_fleet::{
    canonical_provider_id, validate_model_id, validate_session_id, BotFleetError, ByokCredential,
    ByokGatewayRouter, CliAdapterRegistry, CliAdapterStatusDto, CooldownTracker,
    PgidProcessSupervisor, PostExecutionNormalizer, ProviderCooldownDto, CANONICAL_BYOK_PROVIDERS,
};
use chrono::Utc;
use domain::HarnessError;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress,
};
use crate::vault::VaultStore;

const MAX_EXECUTE_BODY_BYTES: usize = 262_144;
const MAX_PROMPT_LEN: usize = 120_000;
const MIN_TIMEOUT_MS: u64 = 500;
const MAX_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_CONCURRENT_BYOS_EXECUTIONS: usize = 8;

fn gateway_cooldowns() -> CooldownTracker {
    static TRACKER: OnceLock<CooldownTracker> = OnceLock::new();
    TRACKER.get_or_init(CooldownTracker::new).clone()
}

fn byos_concurrency_semaphore() -> Arc<Semaphore> {
    static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
    Arc::clone(SEM.get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_BYOS_EXECUTIONS))))
}

pub fn shared_cooldown_tracker() -> CooldownTracker {
    gateway_cooldowns()
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BotFleetExecuteRequest {
    pub project_id: String,
    pub session_id: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default = "default_exec_mode")]
    pub mode: String,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    pub prompt: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_exec_mode() -> String {
    "auto".to_string()
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotFleetExecuteData {
    pub execution_id: String,
    pub mode: String,
    pub adapter_id: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub normalized_content: String,
    pub extracted_json: Option<serde_json::Value>,
    pub ansi_sequences_stripped: usize,
    pub json_blocks_extracted: usize,
    pub svg_viewboxes_injected: usize,
    pub cooldown_applied: bool,
    pub failover_providers: Vec<String>,
    pub pgid_reaped: bool,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub executed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotFleetProvidersStatusData {
    pub sandbox_protocol: String,
    pub session_dir_isolated: bool,
    pub adapter_count: usize,
    pub ready_adapter_count: usize,
    pub adapters: Vec<CliAdapterStatusDto>,
    pub byok_providers: Vec<ProviderCooldownDto>,
    pub checked_at: String,
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

/// Resolves authenticated principal and explicitly rejects non-superadmin legacy Bearer tokens (defense-in-depth C-01).
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
                "Bot fleet routes require verified session token, loopback HMAC, or superadmin Bearer",
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
            "Execution requires editor or admin role",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn verify_project_access(
    state: &DaemonAppState,
    project_id: &str,
    headers: &HeaderMap,
    caller: Option<&VerifiedLegacyIngress>,
    principal: &FlowPrincipal,
    request_id: &str,
) -> Result<String, Response> {
    let trimmed = project_id.trim();
    let project_uuid = Uuid::parse_str(trimmed).map_err(|_| {
        error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be a valid UUID",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        )
    })?;

    if let Some(verified) = caller {
        match verify_anti_idor_with_caller(&state.registry, project_uuid, headers, Some(verified)) {
            Ok(_) => {}
            Err(HarnessError::ProjectNotFound { .. }) => {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "PROJECT_NOT_FOUND",
                    "Project not found",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
            Err(HarnessError::BoundaryViolation { .. }) => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "PROJECT_TENANT_MISMATCH",
                    "Cross-tenant project access forbidden",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
            Err(_) => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "FORBIDDEN",
                    "Project access denied",
                    Some("projectId".to_string()),
                    None,
                    request_id.to_string(),
                ));
            }
        }
    }

    let conn = state.registry.pool().connection().map_err(|_| {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            "Storage connection unavailable",
            None,
            None,
            request_id.to_string(),
        )
    })?;
    let tenant_id: String = conn
        .query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![trimmed],
            |r| r.get(0),
        )
        .map_err(|_| {
            error_response(
                StatusCode::NOT_FOUND,
                "PROJECT_NOT_FOUND",
                "Project not found",
                Some("projectId".to_string()),
                None,
                request_id.to_string(),
            )
        })?;

    if let Some(verified) = caller {
        if matches!(
            verified.kind,
            LegacyAuthKind::HmacV2 | LegacyAuthKind::SessionToken
        ) && verified.principal.tenant_id.trim() != tenant_id.trim()
        {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "PROJECT_TENANT_MISMATCH",
                "Cross-tenant project access forbidden",
                Some("projectId".to_string()),
                None,
                request_id.to_string(),
            ));
        }
    } else if principal.tenant_id.trim() != tenant_id.trim() {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "Cross-tenant project access forbidden",
            Some("projectId".to_string()),
            None,
            request_id.to_string(),
        ));
    }

    Ok(tenant_id)
}

/// Loads tenant-scoped BYOK credentials strictly allowlisted to `CANONICAL_BYOK_PROVIDERS`
/// from both encrypted `VaultStore` (`0o600`) and in-memory `RamVaultStore` (`ram_only`).
pub(crate) fn load_tenant_byok_credentials(
    state: &DaemonAppState,
    tenant_id: &str,
) -> (Vec<ByokCredential>, Vec<String>) {
    let ws_root = state.registry.pool().workspace_root();
    let store = VaultStore::from_workspace_root(ws_root);
    let mut endpoints: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
    let mut configured_names: Vec<String> = Vec::new();

    if let Ok(conn) = state.registry.pool().connection() {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT provider_id, endpoint_url, model_id, credential_ref
             FROM agent_settings
             WHERE tenant_id = ?1",
        ) {
            if let Ok(rows) = stmt.query_map(params![tenant_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    let (pid, ep, model, cred_ref) = row;
                    let canon = canonical_provider_id(&pid);
                    if !CANONICAL_BYOK_PROVIDERS.contains(&canon.as_str()) {
                        continue;
                    }
                    let ep_opt = (!ep.trim().is_empty()).then(|| ep.trim().to_string());
                    let model_opt = (!model.trim().is_empty()).then(|| model.trim().to_string());
                    if !cred_ref.trim().is_empty() || (canon == "byos" && ep_opt.is_some()) {
                        configured_names.push(canon.clone());
                    }
                    endpoints.insert(canon, (ep_opt, model_opt));
                }
            }
        }
    }

    let mut dedup_creds: BTreeMap<String, ByokCredential> = BTreeMap::new();
    let scoped_prefix = format!("tenant::{}::", tenant_id.trim());

    // 1. Load encrypted 0o600 VaultStore secrets (strict allowlist only; zero internal ring/PAT leakage)
    if let Ok(secrets) = store.unseal_secrets(None) {
        for (raw_key, secret_val) in secrets {
            let trimmed_secret = secret_val.trim();
            if trimmed_secret.is_empty() {
                continue;
            }
            let provider_opt = if let Some(rest) = raw_key.strip_prefix(&scoped_prefix) {
                Some(rest.to_string())
            } else if tenant_id.trim() == "local-default" && !raw_key.starts_with("tenant::") {
                Some(raw_key.clone())
            } else {
                None
            };
            let Some(raw_provider) = provider_opt else {
                continue;
            };
            let canon = canonical_provider_id(&raw_provider);
            if !CANONICAL_BYOK_PROVIDERS.contains(&canon.as_str()) {
                continue;
            }
            let (ep_opt, model_opt) = endpoints.get(&canon).cloned().unwrap_or((None, None));
            configured_names.push(canon.clone());
            dedup_creds.insert(
                canon.clone(),
                ByokCredential {
                    provider_id: canon,
                    api_key: Zeroizing::new(trimmed_secret.to_string()),
                    endpoint_url: ep_opt,
                    model_id: model_opt,
                },
            );
        }
    }

    // 2. Merge RAM-only vault secrets (`ram_only` vault mode) with same strict allowlist & canonicalization
    for (raw_provider, secret_val) in state.ram_vault.list_tenant_secrets(tenant_id) {
        let trimmed_secret = secret_val.trim();
        if trimmed_secret.is_empty() {
            continue;
        }
        let canon = canonical_provider_id(&raw_provider);
        if !CANONICAL_BYOK_PROVIDERS.contains(&canon.as_str()) {
            continue;
        }
        let (ep_opt, model_opt) = endpoints.get(&canon).cloned().unwrap_or((None, None));
        configured_names.push(canon.clone());
        dedup_creds.insert(
            canon.clone(),
            ByokCredential {
                provider_id: canon,
                api_key: Zeroizing::new(trimmed_secret.to_string()),
                endpoint_url: ep_opt,
                model_id: model_opt,
            },
        );
    }

    // 3. Include BYOS custom gateway when configured with explicit endpointUrl (even without bearer token)
    if let Some((Some(ep), model_opt)) = endpoints.get("byos") {
        dedup_creds
            .entry("byos".to_string())
            .or_insert_with(|| ByokCredential {
                provider_id: "byos".to_string(),
                api_key: Zeroizing::new(String::new()),
                endpoint_url: Some(ep.clone()),
                model_id: model_opt.clone(),
            });
    }

    configured_names.sort();
    configured_names.dedup();
    (dedup_creds.into_values().collect(), configured_names)
}

async fn execute_byos_adapter(
    execution_id: String,
    session_id: String,
    chosen_adapter: String,
    model_id: Option<String>,
    prompt: String,
    timeout_ms: u64,
    request_id: String,
) -> Response {
    let Ok(_permit) = byos_concurrency_semaphore().try_acquire_owned() else {
        let mut resp = error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            "BYOS execution concurrency limit reached; retry shortly",
            None,
            Some(
                serde_json::json!({ "reason": "CONCURRENCY_SATURATED", "maxConcurrent": MAX_CONCURRENT_BYOS_EXECUTIONS }),
            ),
            request_id,
        );
        resp.headers_mut()
            .insert("retry-after", "5".parse().expect("valid retry-after"));
        return resp;
    };

    let spec = match CliAdapterRegistry::build_sandbox_spec(
        &chosen_adapter,
        prompt,
        model_id.as_deref(),
        timeout_ms,
        None,
    ) {
        Ok(s) => s,
        Err(BotFleetError::InvalidModelId(msg)) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &msg,
                Some("modelId".to_string()),
                None,
                request_id,
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &e.to_string(),
                Some("adapterId".to_string()),
                Some(
                    serde_json::json!({ "reason": "BINARY_NOT_INSTALLED", "adapterId": chosen_adapter }),
                ),
                request_id,
            );
        }
    };

    let output = match PgidProcessSupervisor::execute_ephemeral(session_id, spec).await {
        Ok(out) => out,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                &e.to_string(),
                None,
                None,
                request_id,
            );
        }
    };

    let raw_text = if output.stdout.trim().is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let norm = PostExecutionNormalizer::normalize(raw_text);
    ok_response(
        BotFleetExecuteData {
            execution_id,
            mode: "byos".to_string(),
            adapter_id: Some(chosen_adapter),
            provider_id: None,
            model_id,
            normalized_content: norm.normalized_content,
            extracted_json: norm.extracted_json,
            ansi_sequences_stripped: norm.ansi_sequences_stripped,
            json_blocks_extracted: norm.json_blocks_extracted,
            svg_viewboxes_injected: norm.svg_viewboxes_injected,
            cooldown_applied: false,
            failover_providers: Vec::new(),
            pgid_reaped: output.pgid_reaped,
            timed_out: output.timed_out,
            duration_ms: output.duration_ms,
            executed_at: Utc::now().timestamp_millis(),
        },
        request_id,
    )
}

/// `GET /api/v1/bot-fleet/providers/status`
pub async fn handle_get_bot_fleet_providers_status(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };

    let state_clone = state.clone();
    let tenant_id = principal.tenant_id.clone();
    let tenant_for_snap = tenant_id.clone();
    let (adapters, configured_byok) = tokio::task::spawn_blocking(move || {
        let adapters = CliAdapterRegistry::probe_all();
        let (_, configured) = load_tenant_byok_credentials(&state_clone, &tenant_id);
        (adapters, configured)
    })
    .await
    .unwrap_or_else(|_| (Vec::new(), Vec::new()));

    let ready_adapter_count = adapters.iter().filter(|a| a.sandbox_ready).count();
    let byok_providers = gateway_cooldowns().snapshot(&tenant_for_snap, &configured_byok);

    ok_response(
        BotFleetProvidersStatusData {
            sandbox_protocol: "posix_0600_fd_pgid".to_string(),
            session_dir_isolated: true,
            adapter_count: adapters.len(),
            ready_adapter_count,
            adapters,
            byok_providers,
            checked_at: Utc::now().to_rfc3339(),
        },
        request_id,
    )
}

/// `POST /api/v1/bot-fleet/execute`
pub async fn handle_bot_fleet_execute(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    principal_ext: Option<Extension<FlowPrincipal>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal = match resolve_principal(
        caller.as_ref().map(|e| &e.0),
        principal_ext.as_ref().map(|e| &e.0),
        &headers,
        &request_id,
    ) {
        Ok(p) => p.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(&principal, &request_id) {
        return resp;
    }
    if body.len() > MAX_EXECUTE_BODY_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Bot fleet execute request body exceeds 256 KiB limit",
            Some("body".to_string()),
            None,
            request_id,
        );
    }

    let req: BotFleetExecuteRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(e) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid bot-fleet execute payload: {e}"),
                Some("body".to_string()),
                None,
                request_id,
            );
        }
    };

    let prompt = req.prompt.trim();
    if prompt.is_empty() || prompt.len() > MAX_PROMPT_LEN {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "prompt must be between 1 and 120,000 characters",
            Some("prompt".to_string()),
            None,
            request_id,
        );
    }
    let safe_session_id = match validate_session_id(&req.session_id) {
        Ok(sid) => sid,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "sessionId must match ^[A-Za-z0-9._-]+$ without '..'",
                Some("sessionId".to_string()),
                None,
                request_id,
            );
        }
    };
    if let Some(ref raw_model) = req.model_id {
        let is_gemini = req
            .provider_id
            .as_deref()
            .is_some_and(|p| canonical_provider_id(p) == "gemini");
        if validate_model_id(raw_model, !is_gemini).is_err() {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "modelId failed strict format validation",
                Some("modelId".to_string()),
                None,
                request_id,
            );
        }
    }

    let mode = req.mode.trim().to_ascii_lowercase();
    if !matches!(mode.as_str(), "byos" | "byok" | "auto") {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "mode must be 'byos', 'byok', or 'auto'",
            Some("mode".to_string()),
            None,
            request_id,
        );
    }
    let timeout_ms = req.timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);

    let tenant_id = match verify_project_access(
        &state,
        &req.project_id,
        &headers,
        caller.as_ref().map(|e| &e.0),
        &principal,
        &request_id,
    ) {
        Ok(tid) => tid,
        Err(resp) => return resp,
    };

    let execution_id = Uuid::new_v4().to_string();
    let explicit_byos = mode == "byos" || (mode == "auto" && req.adapter_id.is_some());

    if explicit_byos {
        let adapter_id = req
            .adapter_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                CliAdapterRegistry::probe_all()
                    .into_iter()
                    .find(|a| a.sandbox_ready)
                    .map(|a| a.id)
            });
        let Some(chosen_adapter) = adapter_id else {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "No ready BYOS CLI adapter found on host",
                Some("adapterId".to_string()),
                Some(serde_json::json!({ "reason": "NO_PROVIDER_CONFIGURED" })),
                request_id,
            );
        };
        return execute_byos_adapter(
            execution_id,
            safe_session_id,
            chosen_adapter,
            req.model_id,
            prompt.to_string(),
            timeout_ms,
            request_id,
        )
        .await;
    }

    // Load tenant-scoped BYOK credentials from encrypted VaultStore + RAM-only VaultStore
    let state_clone = state.clone();
    let tid_clone = tenant_id.clone();
    let (creds, _) =
        tokio::task::spawn_blocking(move || load_tenant_byok_credentials(&state_clone, &tid_clone))
            .await
            .unwrap_or_else(|_| (Vec::new(), Vec::new()));

    // In mode == "auto", if no BYOK creds exist, automatically fall back to first ready BYOS CLI adapter (M-04)
    if creds.is_empty() {
        if mode == "auto" {
            let ready_adapter = tokio::task::spawn_blocking(|| {
                CliAdapterRegistry::probe_all()
                    .into_iter()
                    .find(|a| a.sandbox_ready)
                    .map(|a| a.id)
            })
            .await
            .ok()
            .flatten();
            if let Some(chosen_adapter) = ready_adapter {
                return execute_byos_adapter(
                    execution_id,
                    safe_session_id,
                    chosen_adapter,
                    req.model_id,
                    prompt.to_string(),
                    timeout_ms,
                    request_id,
                )
                .await;
            }
        }
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "No BYOK provider credentials configured in VaultStore",
            Some("providerId".to_string()),
            Some(serde_json::json!({ "reason": "NO_PROVIDER_CONFIGURED" })),
            request_id,
        );
    }

    let router = ByokGatewayRouter::new(gateway_cooldowns());
    match router
        .execute_with_failover(
            &tenant_id,
            prompt,
            req.provider_id.as_deref(),
            req.model_id.as_deref(),
            &creds,
            timeout_ms,
        )
        .await
    {
        Ok(res) => {
            let norm = PostExecutionNormalizer::normalize(&res.raw_content);
            ok_response(
                BotFleetExecuteData {
                    execution_id,
                    mode: "byok".to_string(),
                    adapter_id: None,
                    provider_id: Some(res.provider_id),
                    model_id: Some(res.model_id),
                    normalized_content: norm.normalized_content,
                    extracted_json: norm.extracted_json,
                    ansi_sequences_stripped: norm.ansi_sequences_stripped,
                    json_blocks_extracted: norm.json_blocks_extracted,
                    svg_viewboxes_injected: norm.svg_viewboxes_injected,
                    cooldown_applied: res.cooldown_applied,
                    failover_providers: res.failover_providers,
                    pgid_reaped: false,
                    timed_out: false,
                    duration_ms: res.duration_ms,
                    executed_at: Utc::now().timestamp_millis(),
                },
                request_id,
            )
        }
        Err(BotFleetError::RateLimitedOrUnavailable {
            message,
            cooldown_applied,
            failover_providers,
            retry_after_secs,
        }) => {
            let retry_hdr = retry_after_secs.unwrap_or(5).max(1).to_string();
            let mut resp = error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                &message,
                Some("providerId".to_string()),
                Some(serde_json::json!({
                    "reason": "RATE_LIMITED_OR_UNAVAILABLE",
                    "cooldownApplied": cooldown_applied,
                    "failoverProviders": failover_providers,
                })),
                request_id,
            );
            if let Ok(hv) = retry_hdr.parse() {
                resp.headers_mut().insert("retry-after", hv);
            }
            resp
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &e.to_string(),
            None,
            None,
            request_id,
        ),
    }
}

/// Canonical Axum sub-router for Story 11.1 (`POST /api/v1/bot-fleet/execute`, `GET /api/v1/bot-fleet/providers/status`).
/// Merge inside `crates/harness-daemon/src/routes/legacy_router.rs` under `build_protected_legacy_router`.
pub fn build_bot_fleet_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/bot-fleet/execute", post(handle_bot_fleet_execute))
        .route(
            "/api/v1/bot-fleet/providers/status",
            get(handle_get_bot_fleet_providers_status),
        )
}
