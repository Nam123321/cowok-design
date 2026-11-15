// @story 04.4
// @implements Flow Transition & Deploy Handlers (Split from flow_routes.rs <= 250 LOC)
use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use flow_graph::{
    CreateFlowTransitionInput, DeleteFlowTransitionResult, DeployFlowDraftInput,
    FlowGraphSnapshotDto, FlowTransitionDto, UpdateFlowTransitionInput,
};

use crate::daemon_auth::FlowPrincipal;
use crate::flow_routes::{check_uuid, run_blocking, DeleteTransitionQuery};
use crate::routes::DaemonAppState;

pub(crate) async fn handle_list_transitions(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Vec<FlowTransitionDto>>, Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out =
        run_blocking(move || flow_graph::list_transitions(reg.pool(), &principal.tenant_id, &pid))
            .await?;
    Ok(Json(out))
}

pub(crate) async fn handle_create_transition(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<CreateFlowTransitionInput>,
) -> Result<(StatusCode, Json<FlowTransitionDto>), Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::create_transition(reg.pool(), &principal.tenant_id, &pid, &input)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(out)))
}

pub(crate) async fn handle_update_transition(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, transition_id)): AxumPath<(String, String)>,
    Json(input): Json<UpdateFlowTransitionInput>,
) -> Result<Json<FlowTransitionDto>, Response> {
    let pid = check_uuid(&id, "project")?;
    let tid = check_uuid(&transition_id, "transition")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::update_transition(reg.pool(), &principal.tenant_id, &pid, &tid, &input)
    })
    .await?;
    Ok(Json(out))
}

pub(crate) async fn handle_delete_transition(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, transition_id)): AxumPath<(String, String)>,
    Query(q): Query<DeleteTransitionQuery>,
) -> Result<Json<DeleteFlowTransitionResult>, Response> {
    let pid = check_uuid(&id, "project")?;
    let tid = check_uuid(&transition_id, "transition")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::delete_transition(
            reg.pool(),
            &principal.tenant_id,
            &pid,
            &tid,
            q.expected_version,
        )
    })
    .await?;
    Ok(Json(out))
}

pub(crate) async fn handle_deploy_draft(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<DeployFlowDraftInput>,
) -> Result<Json<FlowGraphSnapshotDto>, Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::deploy_draft(reg.pool(), &principal.tenant_id, &pid, &input)
    })
    .await?;
    Ok(Json(out))
}
