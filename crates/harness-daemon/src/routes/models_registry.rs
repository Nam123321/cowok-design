// @story 08.7
// @implements AC-03 (Daemon HTTP route for GET /api/v1/models/registry backed by V24 model_registry_profiles)
use axum::{
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use bot_fleet::{
    builtin_seed_models, canonical_provider_id, list_registry_models, CacheStrategy,
    ModelRegistryEntry, ModelTier,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::{
    is_valid_superadmin_bearer, FlowPrincipal, LegacyAuthKind, VerifiedLegacyIngress,
};
use crate::routes::DaemonAppState;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsRegistryQuery {
    #[serde(default)]
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsRegistryPayload {
    pub models: Vec<ModelRegistryEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
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

fn json_ok<T: Serialize>(data: T, request_id: String) -> Response {
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

fn json_err(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.into(),
                request_id,
            },
        }),
    )
        .into_response()
}

fn extract_principal_strict(
    headers: &HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    request_id: &str,
) -> Result<FlowPrincipal, Response> {
    if let Some(Extension(principal)) = flow_principal {
        return Ok(principal);
    }
    if let Some(Extension(legacy)) = legacy_ingress {
        if legacy.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
            return Err(json_err(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Legacy bearer authentication is not permitted on models registry endpoints",
                request_id.to_string(),
            ));
        }
        return Ok(legacy.principal);
    }
    Err(json_err(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        request_id.to_string(),
    ))
}

const MAX_ID_LEN: usize = 128;
const MAX_PROVIDER_ID_LEN: usize = 32;
const MAX_MODEL_ID_LEN: usize = 128;
const MAX_DISPLAY_NAME_LEN: usize = 128;

fn truncate_bound(s: &str, max_chars: usize) -> String {
    s.trim().chars().take(max_chars).collect()
}

/// Loads enabled dynamic model profiles for `tenant_id` (merged with `local-default`),
/// falling back cleanly to `builtin_seed_models()` if `model_registry_profiles` is absent or empty.
///
/// Tenant override semantics:
/// 1. Loads both `tenant_id` and `'local-default'` rows WITHOUT filtering `enabled = 1` in SQL.
/// 2. Merges in Rust so a tenant row with `enabled = 0` properly disables an inherited seed model.
/// 3. Whenever a tenant defines an enabled row with `is_default = 1` for a provider, demotes
///    `'local-default'` seed defaults for that provider so the tenant override ALWAYS wins.
/// 4. Enforces Zod-aligned length bounds (`id/modelId/displayName <= 128`, `providerId <= 32`).
pub fn load_model_registry_from_conn(
    conn: &Connection,
    tenant_id: &str,
) -> Vec<ModelRegistryEntry> {
    let sql = "SELECT id, tenant_id, provider_id, model_id, display_name, tier, context_window, \
                      supports_prompt_cache, cache_strategy, is_default, enabled \
               FROM model_registry_profiles \
               WHERE tenant_id IN (?1, 'local-default') \
               ORDER BY CASE WHEN tenant_id = ?1 THEN 0 ELSE 1 END, provider_id ASC, is_default DESC, model_id ASC";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return builtin_seed_models();
    };
    let Ok(rows) = stmt.query_map(params![tenant_id], |r| {
        let row_tenant: String = r.get(1)?;
        let raw_id: String = r.get(0)?;
        let raw_provider: String = r.get(2)?;
        let raw_model: String = r.get(3)?;
        let raw_display: String = r.get(4)?;
        let tier_str: String = r.get(5)?;
        let ctx_win: i64 = r.get(6)?;
        let supports_cache: i64 = r.get(7)?;
        let strat_str: String = r.get(8)?;
        let is_default: i64 = r.get(9)?;
        let enabled: i64 = r.get(10)?;
        Ok((
            row_tenant,
            ModelRegistryEntry {
                id: truncate_bound(&raw_id, MAX_ID_LEN),
                provider_id: truncate_bound(&raw_provider, MAX_PROVIDER_ID_LEN),
                model_id: truncate_bound(&raw_model, MAX_MODEL_ID_LEN),
                display_name: truncate_bound(&raw_display, MAX_DISPLAY_NAME_LEN),
                tier: ModelTier::from_db_str(&tier_str),
                context_window: ctx_win.clamp(4096, 2_000_000) as usize,
                supports_prompt_cache: supports_cache != 0,
                cache_strategy: CacheStrategy::from_db_str(&strat_str),
                is_default_for_provider: is_default != 0,
                enabled: enabled != 0,
            },
        ))
    }) else {
        return builtin_seed_models();
    };

    let caller_tenant = tenant_id.trim();
    let mut raw_count = 0usize;
    let mut seen = std::collections::HashSet::new();
    let mut merged: Vec<(bool, ModelRegistryEntry)> = Vec::new();
    let mut tenant_default_providers = std::collections::HashSet::new();

    for (row_tenant, entry) in rows.flatten() {
        raw_count += 1;
        if entry.id.is_empty() || entry.provider_id.is_empty() || entry.model_id.is_empty() {
            continue;
        }
        let canon_provider = canonical_provider_id(&entry.provider_id);
        let key = (canon_provider.clone(), entry.model_id.clone());
        if seen.insert(key) {
            let is_tenant_row =
                row_tenant.trim() == caller_tenant && caller_tenant != "local-default";
            if is_tenant_row && entry.enabled && entry.is_default_for_provider {
                tenant_default_providers.insert(canon_provider);
            }
            merged.push((is_tenant_row, entry));
        }
    }

    if raw_count == 0 {
        return builtin_seed_models();
    }

    let mut active = Vec::with_capacity(merged.len());
    for (is_tenant_row, mut entry) in merged {
        if !entry.enabled {
            continue;
        }
        let canon_provider = canonical_provider_id(&entry.provider_id);
        if !is_tenant_row && tenant_default_providers.contains(&canon_provider) {
            entry.is_default_for_provider = false;
        }
        active.push(entry);
    }
    active
}

pub fn load_tenant_model_registry(
    state: &DaemonAppState,
    tenant_id: &str,
) -> Vec<ModelRegistryEntry> {
    match state.registry.pool().connection() {
        Ok(conn) => load_model_registry_from_conn(&conn, tenant_id),
        Err(_) => builtin_seed_models(),
    }
}

/// `GET /api/v1/models/registry`
pub async fn handle_get_models_registry(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<ModelsRegistryQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
    let tenant_id = principal.tenant_id.clone();
    let provider_filter = query
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let res = tokio::task::spawn_blocking(move || {
        let entries = load_tenant_model_registry(&state, &tenant_id);
        list_registry_models(&entries, provider_filter.as_deref())
    })
    .await;

    match res {
        Ok(models) => json_ok(ModelsRegistryPayload { models }, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Model registry lookup worker failed unexpectedly",
            request_id,
        ),
    }
}

pub fn router() -> Router<DaemonAppState> {
    Router::new().route("/api/v1/models/registry", get(handle_get_models_registry))
}

pub fn build_models_registry_router() -> Router<DaemonAppState> {
    router()
}
