// @story 04.4
// @implements Protected Legacy Router (Split from routes/mod.rs <= 250 LOC)
use axum::{
    middleware,
    routing::{get, patch, post, put},
    Router,
};

use crate::daemon_auth::legacy_project_auth_middleware;
use crate::routes::chat_cas_vault::chat_cas_vault_routes;
use crate::routes::fs_jail;
use crate::routes::github_ide_integrations::integration_routes;
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
use crate::routes::presentation::presentation_routes;
use crate::routes::project_trash::handle_restore_project;
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
        .route_layer(middleware::from_fn_with_state(
            state,
            legacy_project_auth_middleware,
        ))
}
