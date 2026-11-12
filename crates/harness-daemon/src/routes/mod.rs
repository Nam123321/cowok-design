// @story 04.1
// @implements AC-01, AC-03, AC-04, AC-06
pub mod legacy_proposals;
pub mod legacy_sdi;
pub mod legacy_sessions;
pub mod legacy_specs;

use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use spec_registry::SpecRegistry;

use crate::daemon_auth::{handle_issue_local_session, DaemonAuthState};
use crate::flow_routes::build_flow_subrouter;
use crate::lockfile::CURRENT_SCHEMA_VERSION;

pub use legacy_proposals::*;
pub use legacy_sdi::*;
pub use legacy_sessions::*;
pub use legacy_specs::*;

use legacy_proposals::{
    handle_apply_proposal, handle_create_pr, handle_create_proposal, handle_create_snapshot,
    handle_download_export, handle_export_snapshots, handle_list_proposals, handle_list_snapshots,
    handle_reject_proposal, handle_revert_snapshot, handle_validate_proposal,
};
use legacy_sdi::{
    handle_get_constraint_profile, handle_get_icon_manifest, handle_inject_ff_id,
    handle_lint_source, handle_model_adapter, handle_prompt_compile, handle_put_constraint_profile,
    handle_sdi_clarification, handle_sdi_intent, handle_surgical_patch, handle_validate_icon,
};
use legacy_sessions::{
    extract_actor_id, handle_create_chat_session, handle_delete_chat_session, handle_get_boundary,
    handle_get_chat_session, handle_issue_path_token, handle_list_chat_sessions,
    handle_promote_scratch, handle_update_chat_session, handle_verify_path_token,
    parse_project_uuid,
};
use legacy_specs::{
    constant_time_eq, handle_context_slice, handle_create_project, handle_list_projects,
    handle_list_specs, handle_put_spec, handle_synthesize_specs,
    verify_anti_idor_and_resolve_registry,
};

impl DaemonAppState {
    pub fn new(registry: Arc<SpecRegistry>, host: String, port: u16) -> Self {
        Self {
            registry,
            host,
            port,
        }
    }
}

#[derive(Clone)]
pub struct DaemonAppState {
    pub registry: Arc<SpecRegistry>,
    pub host: String,
    pub port: u16,
}

fn registry_auth(registry: &SpecRegistry) -> Arc<DaemonAuthState> {
    static AUTH_BY_ROOT: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Arc<DaemonAuthState>>>,
    > = std::sync::OnceLock::new();
    let map_mutex =
        AUTH_BY_ROOT.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let root = registry
        .pool()
        .workspace_root()
        .canonicalize()
        .unwrap_or_else(|_| registry.pool().workspace_root().to_path_buf());
    let mut map = map_mutex.lock().expect("auth_by_root");
    if let Some(existing) = map.get(&root) {
        return Arc::clone(existing);
    }
    let created = DaemonAuthState::from_registry_root(&root);
    map.insert(root, Arc::clone(&created));
    created
}

pub(crate) fn state_auth(state: &DaemonAppState) -> Arc<DaemonAuthState> {
    registry_auth(&state.registry)
}

/// Health response returned by `GET /api/v1/health` (redacts `hmac_key_hex` unconditionally).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub daemon_nonce: String,
    #[serde(default)]
    pub schema_version: u32,
}

async fn handle_health(State(state): State<DaemonAppState>) -> Json<HealthResponse> {
    let auth = state_auth(&state);
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "cowok-design-harness-daemon".to_string(),
        host: state.host,
        port: state.port,
        pid: std::process::id(),
        daemon_nonce: auth.daemon_nonce.clone(),
        schema_version: CURRENT_SCHEMA_VERSION,
    })
}

/// Builds the canonical Axum Router combining the 28 legacy routes (`Bearer <project.token>`),
/// `/api/v1/auth/local-session`, and the scoped Dual-Ingress `/api/v1/projects/:id/flow/*` router.
pub fn build_router(state: DaemonAppState) -> Router {
    let _ = ensure_ac06_tables(&state.registry);
    let flow_router = build_flow_subrouter(state.clone());

    Router::new()
        .route("/api/v1/health", get(handle_health))
        .route(
            "/api/v1/auth/local-session",
            post(handle_issue_local_session),
        )
        .route(
            "/api/v1/projects",
            get(handle_list_projects).post(handle_create_project),
        )
        .route("/api/v1/projects/:id/specs", get(handle_list_specs))
        .route("/api/v1/projects/:id/specs/:spec_key", put(handle_put_spec))
        .route(
            "/api/v1/projects/:id/specs/synthesize",
            post(handle_synthesize_specs),
        )
        .route(
            "/api/v1/projects/:id/context-slice",
            post(handle_context_slice),
        )
        .route(
            "/api/v1/projects/:id/specs/slice",
            post(handle_context_slice),
        )
        .route(
            "/api/v1/projects/:id/prompt-compile",
            post(handle_prompt_compile),
        )
        .route(
            "/api/v1/projects/:id/prompt/compile",
            post(handle_prompt_compile),
        )
        .route(
            "/api/v1/projects/:id/model-adapter",
            post(handle_model_adapter),
        )
        .route(
            "/api/v1/projects/:id/model/adapt",
            post(handle_model_adapter),
        )
        .route("/api/v1/projects/:id/lint", post(handle_lint_source))
        .route(
            "/api/v1/projects/:id/constraint-profile",
            get(handle_get_constraint_profile).put(handle_put_constraint_profile),
        )
        .route(
            "/api/v1/projects/:id/icon-manifest",
            get(handle_get_icon_manifest),
        )
        .route(
            "/api/v1/projects/:id/validate-icon",
            post(handle_validate_icon),
        )
        .route(
            "/api/v1/projects/:id/inject-ff-id",
            post(handle_inject_ff_id),
        )
        .route(
            "/api/v1/projects/:id/surgical-patch",
            post(handle_surgical_patch),
        )
        .route("/api/v1/projects/:id/sdi/intent", post(handle_sdi_intent))
        .route(
            "/api/v1/projects/:id/sdi/clarification",
            post(handle_sdi_clarification),
        )
        .route(
            "/api/v1/projects/:id/proposals",
            get(handle_list_proposals).post(handle_create_proposal),
        )
        .route(
            "/api/v1/projects/:id/proposals/:proposal_id/validate",
            post(handle_validate_proposal),
        )
        .route(
            "/api/v1/projects/:id/proposals/:proposal_id/apply",
            post(handle_apply_proposal),
        )
        .route(
            "/api/v1/projects/:id/proposals/:proposal_id/reject",
            post(handle_reject_proposal),
        )
        .route(
            "/api/v1/projects/:id/snapshots",
            get(handle_list_snapshots).post(handle_create_snapshot),
        )
        .route(
            "/api/v1/projects/:id/snapshots/:snapshot_id/revert",
            post(handle_revert_snapshot),
        )
        .route(
            "/api/v1/projects/:id/snapshots/export",
            post(handle_export_snapshots),
        )
        .route(
            "/api/v1/projects/:id/snapshots/export/download",
            get(handle_download_export),
        )
        .route("/api/v1/projects/:id/snapshots/pr", post(handle_create_pr))
        .route("/api/v1/projects/:id/boundary", get(handle_get_boundary))
        .route("/api/v1/projects/:id/promote", post(handle_promote_scratch))
        .route(
            "/api/v1/projects/:id/sessions",
            get(handle_list_chat_sessions).post(handle_create_chat_session),
        )
        .route(
            "/api/v1/projects/:id/sessions/:session_id",
            get(handle_get_chat_session)
                .put(handle_update_chat_session)
                .delete(handle_delete_chat_session),
        )
        .route(
            "/api/v1/projects/:id/path-token",
            post(handle_issue_path_token),
        )
        .route(
            "/api/v1/projects/:id/verify-path-token",
            post(handle_verify_path_token),
        )
        .merge(flow_router)
        .with_state(state)
}
