// @story 08.7
// @implements AC-01, AC-05 (Daemon HTTP routes for GET /api/v1/skills/catalog & POST /api/v1/skills/catalog/reload)
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::{
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use plugin_host::{
    UnifiedSkillCatalog, UnifiedSkillCatalogSnapshot, CANONICAL_DESIGN_CRAFT_RELATIVE_DIR,
    HOST_SKILLS_RELATIVE_DIR,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, FlowPrincipal, LegacyAuthKind,
    VerifiedLegacyIngress,
};
use crate::routes::DaemonAppState;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsCatalogQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

type CatalogCacheKey = (PathBuf, PathBuf, String, Option<String>);
const MAX_CATALOG_CACHE_ENTRIES: usize = 64;

#[derive(Default)]
struct BoundedCatalogCache {
    order: VecDeque<CatalogCacheKey>,
    entries: HashMap<CatalogCacheKey, UnifiedSkillCatalog>,
}

impl BoundedCatalogCache {
    fn get(&mut self, key: &CatalogCacheKey) -> Option<UnifiedSkillCatalog> {
        let val = self.entries.get(key).cloned()?;
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
            self.order.push_back(key.clone());
        }
        Some(val)
    }

    fn insert(&mut self, key: CatalogCacheKey, value: UnifiedSkillCatalog) {
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
        }
        while self.entries.len() >= MAX_CATALOG_CACHE_ENTRIES {
            if let Some(evicted) = self.order.pop_front() {
                self.entries.remove(&evicted);
            } else {
                break;
            }
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
    }

    fn invalidate_tenant(&mut self, tenant_id: &str) {
        let target = tenant_id.trim();
        self.order.retain(|k| k.2 != target);
        self.entries.retain(|k, _| k.2 != target);
    }
}

fn catalog_cache() -> &'static Mutex<BoundedCatalogCache> {
    static CACHE: OnceLock<Mutex<BoundedCatalogCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BoundedCatalogCache::default()))
}

/// Invalidates all cached `UnifiedSkillCatalog` entries and Gemini `cachedContents/*` handles
/// for `tenant_id` (invoked after plugin install/promote/toggle/quarantine or LEARN card approve/reject).
pub fn invalidate_catalog_cache_for_tenant(tenant_id: &str) {
    let mut guard = catalog_cache().lock().unwrap_or_else(|p| p.into_inner());
    guard.invalidate_tenant(tenant_id);
    bot_fleet::shared_gemini_cache_registry().invalidate_tenant(tenant_id);
}

fn verify_project_owned_by_tenant(
    state: &DaemonAppState,
    tenant_id: &str,
    project_id: &str,
) -> Result<(), (StatusCode, &'static str, String)> {
    let conn = state.registry.pool().connection().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            format!("Storage connection unavailable: {e}"),
        )
    })?;
    let found: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
    if found.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found for caller tenant".to_string(),
        ));
    }
    Ok(())
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
                field: None,
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
                "Legacy bearer authentication is not permitted on skills catalog endpoints",
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

fn looks_like_skill_repo_root(dir: &Path) -> bool {
    dir.join(CANONICAL_DESIGN_CRAFT_RELATIVE_DIR).is_dir()
        || dir.join(HOST_SKILLS_RELATIVE_DIR).is_dir()
        || dir.join("packages/builtin-skills/design-craft").is_dir()
}

/// Resolves the canonical repository root containing `packages/design-craft` and `packages/host-skills`.
pub fn resolve_catalog_repo_root(state: &DaemonAppState) -> PathBuf {
    let ws = state.registry.pool().workspace_root();
    if looks_like_skill_repo_root(ws) {
        return ws.to_path_buf();
    }
    if let Ok(env_root) = std::env::var("COWOK_REPO_ROOT") {
        let pb = PathBuf::from(env_root);
        if looks_like_skill_repo_root(&pb) {
            return pb;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if looks_like_skill_repo_root(&cwd) {
            return cwd;
        }
    }
    let compile_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    if let Some(root) = compile_root {
        if looks_like_skill_repo_root(&root) {
            return root;
        }
    }
    ws.to_path_buf()
}

/// Discovers (or reads from cache) the `UnifiedSkillCatalog` for `(tenant_id, project_id)`.
pub fn load_or_discover_catalog(
    state: &DaemonAppState,
    tenant_id: &str,
    project_id: Option<&str>,
    force_reload: bool,
) -> UnifiedSkillCatalog {
    let repo_root = resolve_catalog_repo_root(state);
    let db_path = state.registry.pool().db_path().to_path_buf();
    let cache_key = (
        db_path,
        repo_root.clone(),
        tenant_id.to_string(),
        project_id.map(str::to_string),
    );
    if !force_reload {
        let mut guard = catalog_cache().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = guard.get(&cache_key) {
            return existing;
        }
    }
    let conn = state.registry.pool().connection().ok();
    let fresh = UnifiedSkillCatalog::discover(&repo_root, conn.as_ref(), tenant_id, project_id);
    let mut guard = catalog_cache().lock().unwrap_or_else(|p| p.into_inner());
    guard.insert(cache_key, fresh.clone());
    fresh
}

/// `GET /api/v1/skills/catalog`
pub async fn handle_get_skills_catalog(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<SkillsCatalogQuery>,
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
    let project_id = query
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let search_q = query.q.clone();

    let res = tokio::task::spawn_blocking(
        move || -> Result<UnifiedSkillCatalogSnapshot, (StatusCode, &'static str, String)> {
            if let Some(ref pid) = project_id {
                verify_project_owned_by_tenant(&state, &tenant_id, pid)?;
            }
            let catalog =
                load_or_discover_catalog(&state, &tenant_id, project_id.as_deref(), false);
            Ok(
                match search_q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(q) => catalog.filter_snapshot(q),
                    None => catalog.snapshot(),
                },
            )
        },
    )
    .await;

    match res {
        Ok(Ok(snapshot)) => json_ok(snapshot, request_id),
        Ok(Err((status, code, msg))) => json_err(status, code, msg, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Skills catalog discovery worker failed unexpectedly",
            request_id,
        ),
    }
}

/// `POST /api/v1/skills/catalog/reload`
pub async fn handle_reload_skills_catalog(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<SkillsCatalogQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }
    let tenant_id = principal.tenant_id.clone();
    let project_id = query
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let res = tokio::task::spawn_blocking(
        move || -> Result<UnifiedSkillCatalogSnapshot, (StatusCode, &'static str, String)> {
            if let Some(ref pid) = project_id {
                verify_project_owned_by_tenant(&state, &tenant_id, pid)?;
            }
            let catalog = load_or_discover_catalog(&state, &tenant_id, project_id.as_deref(), true);
            Ok(catalog.snapshot())
        },
    )
    .await;

    match res {
        Ok(Ok(snapshot)) => json_ok(snapshot, request_id),
        Ok(Err((status, code, msg))) => json_err(status, code, msg, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Skills catalog reload worker failed unexpectedly",
            request_id,
        ),
    }
}

pub fn router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/skills/catalog", get(handle_get_skills_catalog))
        .route(
            "/api/v1/skills/catalog/reload",
            post(handle_reload_skills_catalog),
        )
}

pub fn build_skills_catalog_router() -> Router<DaemonAppState> {
    router()
}
