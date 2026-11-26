// @story 04.4
// @implements Protected Legacy Router (Split from routes/mod.rs <= 250 LOC)
use axum::{
    middleware,
    routing::{get, patch, post, put},
    Router,
};

use crate::daemon_auth::legacy_project_auth_middleware;
use crate::routes::bot_fleet::build_bot_fleet_router;
use crate::routes::chat_cas_vault::chat_cas_vault_routes;
use crate::routes::cognitive_studio::build_cognitive_studio_router;
use crate::routes::context_assemble::{handle_context_assemble, handle_get_context_telemetry};
use crate::routes::de_runtime::build_de_runtime_router;
use crate::routes::decision::build_decision_router;
use crate::routes::deck_generation::build_deck_generation_router;
use crate::routes::design_tournament::design_tournament_router;
use crate::routes::eval_certification::eval_certification_router;
use crate::routes::fleet_swarm::fleet_swarm_router;
use crate::routes::flow_synthesize::build_flow_synthesize_router;
use crate::routes::fs_jail;
use crate::routes::github_ide_integrations::integration_routes;
use crate::routes::learn_studio_corrections::build_learn_studio_corrections_router;
use crate::routes::legacy_proposals::{
    handle_apply_proposal, handle_create_pr, handle_create_proposal, handle_create_snapshot,
    handle_decide_tradeoff, handle_diff_proposal, handle_download_export, handle_export_snapshots,
    handle_fork_turn_snapshot, handle_get_proposal, handle_get_snapshot, handle_list_proposals,
    handle_list_snapshots, handle_list_turn_snapshots, handle_open_ide_handoff,
    handle_reject_proposal, handle_revert_snapshot, handle_validate_proposal,
};
use crate::routes::legacy_sdi::{
    handle_get_constraint_profile, handle_get_design_contracts, handle_get_icon_manifest,
    handle_get_sdi_intent, handle_inject_ff_id, handle_lint_source, handle_list_sdi_intents,
    handle_model_adapter, handle_prompt_compile, handle_put_constraint_profile,
    handle_put_design_contracts, handle_sdi_clarification, handle_sdi_intent,
    handle_surgical_patch, handle_validate_component, handle_validate_icon,
    handle_validate_typography,
};
use crate::routes::legacy_sessions::{
    handle_create_chat_session, handle_delete_chat_session, handle_get_boundary,
    handle_get_chat_session, handle_issue_path_token, handle_list_chat_sessions,
    handle_promote_scratch, handle_update_chat_session, handle_verify_path_token,
};
use crate::routes::legacy_specs::{
    handle_context_slice, handle_create_project, handle_delete_project, handle_delete_spec,
    handle_list_projects, handle_list_specs, handle_put_spec, handle_synthesize_specs,
    handle_update_project,
};
use crate::routes::mcp_gateway::mcp_gateway_router;
use crate::routes::models_registry::build_models_registry_router;
use crate::routes::orchestrator::build_orchestrator_router;
use crate::routes::presentation::presentation_routes;
use crate::routes::project_trash::handle_restore_project;
use crate::routes::rea_bridge::build_rea_bridge_router;
use crate::routes::sketch_polish::{
    handle_apply_sketch_patch, handle_get_sdi_clarify, handle_get_sketch_patch_history,
    handle_sdi_clarify,
};
use crate::routes::skills_catalog::build_skills_catalog_router;
use crate::routes::split_export::split_export_router;
use crate::routes::ssot_registry::{
    handle_export_ssot, handle_get_ssot_components, handle_mutate_appshell_lock,
    handle_route_skills,
};
use crate::routes::support_telemetry::build_support_telemetry_router;
use crate::routes::visual_qc::visual_qc_router;
use crate::routes::DaemonAppState;

fn legacy_project_spec_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects",
            get(handle_list_projects).post(handle_create_project),
        )
        .route(
            "/api/v1/projects/:id",
            patch(handle_update_project).delete(handle_delete_project),
        )
        .route("/api/v1/projects/:id/restore", post(handle_restore_project))
        .route("/api/v1/projects/:id/specs", get(handle_list_specs))
        .route(
            "/api/v1/projects/:id/specs/:spec_key",
            put(handle_put_spec).delete(handle_delete_spec),
        )
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
        .route("/api/v1/context/assemble", post(handle_context_assemble))
        .route(
            "/api/v1/context/telemetry",
            get(handle_get_context_telemetry),
        )
        .route("/api/v1/ssot/components", get(handle_get_ssot_components))
        .route(
            "/api/v1/ssot/appshell/lock",
            post(handle_mutate_appshell_lock),
        )
        .route("/api/v1/ssot/export", post(handle_export_ssot))
        .route("/api/v1/skills/route", post(handle_route_skills))
        .route(
            "/api/v1/sdi/clarify",
            get(handle_get_sdi_clarify).post(handle_sdi_clarify),
        )
        .route(
            "/api/v1/patch",
            get(handle_get_sketch_patch_history).post(handle_apply_sketch_patch),
        )
}

fn legacy_sdi_proposal_routes() -> Router<DaemonAppState> {
    Router::new()
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
            "/api/v1/projects/:id/validate-typography",
            post(handle_validate_typography),
        )
        .route(
            "/api/v1/projects/:id/validate-component",
            post(handle_validate_component),
        )
        .route(
            "/api/v1/projects/:id/design-contracts",
            get(handle_get_design_contracts).put(handle_put_design_contracts),
        )
        .route(
            "/api/v1/projects/:id/inject-ff-id",
            post(handle_inject_ff_id),
        )
        .route(
            "/api/v1/projects/:id/surgical-patch",
            post(handle_surgical_patch),
        )
        .route(
            "/api/v1/projects/:id/sdi/intent",
            get(handle_list_sdi_intents).post(handle_sdi_intent),
        )
        .route(
            "/api/v1/projects/:id/sdi/intent/:intent_id",
            get(handle_get_sdi_intent),
        )
        .route(
            "/api/v1/projects/:id/sdi/clarification",
            post(handle_sdi_clarification),
        )
        .route(
            "/api/v1/projects/:id/proposals",
            get(handle_list_proposals).post(handle_create_proposal),
        )
        .route(
            "/api/v1/projects/:id/proposals/:proposal_id",
            get(handle_get_proposal),
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
            "/api/v1/projects/:id/proposals/:proposal_id/diff",
            post(handle_diff_proposal),
        )
        .route(
            "/api/v1/projects/:id/proposals/:proposal_id/tradeoffs/:tradeoff_id/decide",
            post(handle_decide_tradeoff),
        )
}

fn legacy_snapshot_session_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/snapshots",
            get(handle_list_snapshots).post(handle_create_snapshot),
        )
        .route(
            "/api/v1/projects/:id/turn-snapshots",
            get(handle_list_turn_snapshots),
        )
        .route(
            "/api/v1/projects/:id/snapshots/:snapshot_id",
            get(handle_get_snapshot),
        )
        .route(
            "/api/v1/projects/:id/snapshots/:snapshot_id/revert",
            post(handle_revert_snapshot),
        )
        .route(
            "/api/v1/projects/:id/snapshots/:turn_snapshot_id/fork",
            post(handle_fork_turn_snapshot),
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
        .route(
            "/api/v1/projects/:id/handoff/open-ide",
            post(handle_open_ide_handoff),
        )
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
        .route("/api/v1/fs/browse", get(fs_jail::handle_fs_browse))
        .route(
            "/api/v1/fs/pick-native",
            post(fs_jail::handle_fs_pick_native),
        )
        .route(
            "/api/v1/projects/:id/path-token",
            post(handle_issue_path_token),
        )
        .route(
            "/api/v1/projects/:id/verify-path-token",
            post(handle_verify_path_token),
        )
}

pub(crate) fn build_protected_legacy_router(state: DaemonAppState) -> Router<DaemonAppState> {
    Router::new()
        .merge(legacy_project_spec_routes())
        .merge(legacy_sdi_proposal_routes())
        .merge(legacy_snapshot_session_routes())
        .merge(chat_cas_vault_routes())
        .merge(integration_routes())
        .merge(presentation_routes())
        .merge(build_de_runtime_router())
        .merge(build_bot_fleet_router())
        .merge(build_rea_bridge_router())
        .merge(build_decision_router())
        .merge(build_flow_synthesize_router())
        .merge(build_deck_generation_router())
        .merge(build_orchestrator_router())
        .merge(build_support_telemetry_router())
        .merge(build_skills_catalog_router())
        .merge(build_models_registry_router())
        .merge(build_learn_studio_corrections_router())
        .merge(build_cognitive_studio_router())
        .merge(visual_qc_router())
        .merge(split_export_router())
        .merge(fleet_swarm_router())
        .merge(design_tournament_router())
        .merge(eval_certification_router())
        .merge(mcp_gateway_router())
        .route_layer(middleware::from_fn_with_state(
            state,
            legacy_project_auth_middleware,
        ))
}

/// Response header attached whenever a standalone `/api/*` request is rewritten to `/api/v1/*`.
pub const STANDALONE_API_ALIAS_HEADER: &str = "x-cowok-api-alias";

/// Rewrites `/api/<tail>` to `/api/v1/<tail>` when `<tail>` does not already target `v1`.
/// Returns `None` for `/api/v1/*`, bare `/api`, or non-`/api/*` paths.
pub fn normalize_standalone_api_alias_path(path: &str) -> Option<String> {
    let tail = path.strip_prefix("/api/")?;
    if tail.is_empty() || tail == "v1" || tail.starts_with("v1/") {
        return None;
    }
    Some(format!("/api/v1/{tail}"))
}

async fn handle_standalone_api_not_found(req: axum::extract::Request) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let path = req.uri().path().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    (
        StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({
            "error": {
                "code": "NOT_FOUND",
                "message": format!("API endpoint '{path}' was not found"),
                "requestId": request_id,
            }
        })),
    )
        .into_response()
}

fn has_unsafe_path_tokens(path: &str) -> bool {
    if path.contains("..") || path.contains('\\') || path.contains('\0') {
        return true;
    }
    let lower = path.to_ascii_lowercase();
    lower.contains("%2e%2e")
        || lower.contains("%2e.")
        || lower.contains(".%2e")
        || lower.contains("%5c")
        || lower.contains("%00")
}

fn handle_standalone_bad_request(path: &str) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let request_id = uuid::Uuid::new_v4().to_string();
    (
        StatusCode::BAD_REQUEST,
        axum::Json(serde_json::json!({
            "error": {
                "code": "BAD_REQUEST",
                "message": format!("Unsafe path token rejected for '{path}'"),
                "requestId": request_id,
            }
        })),
    )
        .into_response()
}

async fn standalone_parity_middleware(
    axum::extract::State(spa): axum::extract::State<crate::static_spa::StaticSpaHandler>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{HeaderName, HeaderValue, Uri};

    let original_path = req.uri().path().to_string();

    if has_unsafe_path_tokens(&original_path) {
        return handle_standalone_bad_request(&original_path);
    }

    if let Some(rewritten_path) = normalize_standalone_api_alias_path(&original_path) {
        let path_and_query = match req.uri().query() {
            Some(q) => format!("{rewritten_path}?{q}"),
            None => rewritten_path,
        };
        let mut parts = req.uri().clone().into_parts();
        if let Ok(pq) = path_and_query.parse() {
            parts.path_and_query = Some(pq);
            if let Ok(new_uri) = Uri::from_parts(parts) {
                *req.uri_mut() = new_uri;
            }
        }
        let mut resp = next.run(req).await;
        resp.headers_mut().insert(
            HeaderName::from_static(STANDALONE_API_ALIAS_HEADER),
            HeaderValue::from_static("v1"),
        );
        return resp;
    }

    let collapsed_lower = original_path.trim_start_matches('/').to_ascii_lowercase();
    if collapsed_lower == "api" || collapsed_lower.starts_with("api/") {
        return next.run(req).await;
    }

    spa.handle_request(req.method(), &original_path)
}

/// Wraps a canonical `build_router(state)` instance with TDR-002 Phase 2 standalone parity:
/// 1. Enforces `cors_and_loopback_middleware` as the OUTERMOST layer covering SPA requests,
///    aliased `/api/*` routes, and JSON `404` fallback responses.
/// 2. Rejects unsafe path tokens (`..`, `\`, `\0`) with JSON `400 BAD_REQUEST`.
/// 3. Rewrites `/api/<tail>` (non-`v1`) to `/api/v1/<tail>` preserving method, query, headers,
///    and body while attaching `x-cowok-api-alias: v1`.
/// 4. Returns structured JSON `404` for unknown `/api/*` (and `//api/*`, `/API/*`) paths so
///    they never fall through to SPA HTML.
/// 5. Serves the embedded React 19 Workbench SPA / `dist/` for non-`/api/*` routes.
pub fn wrap_with_standalone_parity(
    api_router: Router,
    state: DaemonAppState,
    spa: crate::static_spa::StaticSpaHandler,
) -> Router {
    let api_with_json_404 = api_router.fallback(handle_standalone_api_not_found);
    Router::new()
        .fallback_service(api_with_json_404)
        .layer(middleware::from_fn_with_state(
            spa,
            standalone_parity_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state,
            crate::daemon_auth::cors_and_loopback_middleware,
        ))
}
