// @story 04.1, 04.6
// @implements AC-01, AC-03, AC-04, AC-06
pub mod chat_cas_archive;
pub mod chat_cas_storage;
pub mod chat_cas_upload_handler;
pub mod chat_cas_vault;
pub mod context_assemble;
pub mod doctor_usage;
pub mod doctor_vault;
pub mod execution_coordinator;
pub mod execution_coordinator_sql;
pub mod execution_events;
pub mod execution_reaper;
pub mod execution_sessions;
pub mod execution_stream;
pub mod execution_sweeper;
pub mod fs_jail;
pub mod github_ide_integrations;
pub mod legacy_proposals;
pub mod legacy_router;
pub mod legacy_sdi;
pub mod legacy_sessions;
pub mod legacy_specs;
pub mod legacy_workbench_v6;
pub mod plugins;
pub mod plugins_cron;
pub mod presentation;
pub mod presentation_cms_routes;
pub mod presentation_copilot;
pub mod presentation_presenter;
pub mod presentation_storage;
pub mod project_trash;
pub mod ssot_registry;

use std::sync::Arc;

use axum::{
    extract::State,
    middleware,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use spec_registry::SpecRegistry;

use crate::daemon_auth::{
    cors_and_loopback_middleware, handle_issue_human_workbench_session, handle_issue_local_session,
    DaemonAuthState,
};
use crate::flow_routes::build_flow_subrouter;
use crate::lockfile::CURRENT_SCHEMA_VERSION;

pub use doctor_usage::*;
pub use doctor_vault::*;
pub use fs_jail::{FsBrowseEntry, FsBrowseQuery, FsBrowseResponse};

pub use legacy_proposals::*;
pub use legacy_sdi::*;
pub use legacy_sessions::*;
pub(crate) use legacy_specs::constant_time_eq;
pub use legacy_specs::*;
pub use project_trash::RestoreProjectResponse;
#[derive(Clone)]
pub struct DaemonAppState {
    pub registry: Arc<SpecRegistry>,
    pub host: String,
    pub port: u16,
    pub auth: Arc<DaemonAuthState>,
    pub ram_vault: crate::vault::RamVaultStore,
    pub active_workers: execution_reaper::ActiveWorkerRegistry,
    pub sse_tx: tokio::sync::broadcast::Sender<String>,
    pub cas_sha_locks:
        Arc<tokio::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl DaemonAppState {
    pub fn new(registry: Arc<SpecRegistry>, host: String, port: u16) -> Self {
        let auth = DaemonAuthState::from_registry_root_with_pool(
            registry.pool().workspace_root(),
            Some(registry.pool().clone()),
        );
        let (sse_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            registry,
            host,
            port,
            auth,
            ram_vault: crate::vault::RamVaultStore::default(),
            active_workers: execution_reaper::ActiveWorkerRegistry::new(),
            sse_tx,
            cas_sha_locks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn with_auth(
        registry: Arc<SpecRegistry>,
        host: String,
        port: u16,
        auth: Arc<DaemonAuthState>,
    ) -> Self {
        let (sse_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            registry,
            host,
            port,
            auth,
            ram_vault: crate::vault::RamVaultStore::default(),
            active_workers: execution_reaper::ActiveWorkerRegistry::new(),
            sse_tx,
            cas_sha_locks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub async fn cas_sha_lock(&self, sha256: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut map = self.cas_sha_locks.lock().await;
        Arc::clone(
            map.entry(sha256.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    }
}

pub(crate) fn state_auth(state: &DaemonAppState) -> Arc<DaemonAuthState> {
    Arc::clone(&state.auth)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub pid: u32,
    #[serde(default, skip_serializing)]
    pub daemon_nonce: String,
    #[serde(default)]
    pub schema_version: u32,
}

async fn handle_health(State(state): State<DaemonAppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "cowok-design-harness-daemon".to_string(),
        host: state.host,
        port: state.port,
        pid: std::process::id(),
        daemon_nonce: String::new(),
        schema_version: CURRENT_SCHEMA_VERSION,
    })
}

/// Builds the canonical Axum Router combining legacy routes (`Bearer <project.token>`),
/// `/api/v1/auth/local-session`, and the scoped Dual-Ingress `/api/v1/projects/:id/flow/*` router.
pub fn build_router(state: DaemonAppState) -> Router {
    let _ = ensure_ac06_tables(&state.registry);
    let active_paths_opt = state.registry.pool().connection().ok().and_then(|conn| {
        spec_registry::presentation_repo::list_active_slide_html_paths(&conn).ok()
    });
    let (active_paths, sweep_unreferenced) = match active_paths_opt {
        Some(paths) => (paths, true),
        None => (std::collections::HashSet::new(), false),
    };
    let mut sweep_roots = vec![state.registry.pool().workspace_root().to_path_buf()];
    if let Ok(conn) = state.registry.pool().connection() {
        if let Ok(project_roots) =
            spec_registry::presentation_repo::list_all_project_workspace_roots(&conn)
        {
            for r in project_roots {
                let pb = std::path::PathBuf::from(r);
                if !sweep_roots.contains(&pb) {
                    sweep_roots.push(pb);
                }
            }
        }
    }
    for root in &sweep_roots {
        let _ =
            presentation::sweep_orphan_slide_artifacts(root, &active_paths, 60, sweep_unreferenced);
    }
    let flow_router = build_flow_subrouter(state.clone());
    let protected_legacy_router = legacy_router::build_protected_legacy_router(state.clone());
    let cors_state = state.clone();
    Router::new()
        .route("/api/v1/health", get(handle_health))
        .route(
            "/api/v1/auth/local-session",
            post(handle_issue_local_session),
        )
        .route(
            "/api/v1/auth/workbench-session",
            post(handle_issue_human_workbench_session),
        )
        .merge(protected_legacy_router)
        .merge(flow_router)
        .layer(middleware::from_fn_with_state(
            cors_state,
            cors_and_loopback_middleware,
        ))
        .with_state(state)
}
