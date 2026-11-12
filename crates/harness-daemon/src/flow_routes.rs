// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use axum::{
    extract::{DefaultBodyLimit, Extension, Path as AxumPath, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use flow_graph::{
    BatchUpdatePositionsInput, BatchUpdatePositionsResult, CreateFlowNodeInput,
    CreateFlowTransitionInput, DeleteFlowNodeResult, DeleteFlowTransitionResult,
    DeployFlowDraftInput, FlowGraphSnapshotDto, FlowNodeDto, FlowServiceError, FlowTransitionDto,
    UpdateFlowNodeInput, UpdateFlowTransitionInput,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::daemon_auth::{flow_dual_ingress_auth_middleware, FlowPrincipal};
use crate::routes::{DaemonAppState, HttpErrorBody};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNodeQuery {
    pub expected_version: Option<i64>,
    pub expected_graph_version: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTransitionQuery {
    pub expected_version: Option<i64>,
}

pub fn build_flow_subrouter(state: DaemonAppState) -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/flow/snapshot",
            get(handle_get_snapshot),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes",
            get(handle_list_nodes).post(handle_create_node),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes/positions",
            patch(handle_batch_positions),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes/:node_id",
            patch(handle_update_node).delete(handle_delete_node),
        )
        .route(
            "/api/v1/projects/:id/flow/transitions",
            get(handle_list_transitions).post(handle_create_transition),
        )
        .route(
            "/api/v1/projects/:id/flow/transitions/:transition_id",
            patch(handle_update_transition).delete(handle_delete_transition),
        )
        .route(
            "/api/v1/projects/:id/flow/deploy-draft",
            post(handle_deploy_draft),
        )
        .layer(DefaultBodyLimit::max(1_048_576))
        .route_layer(middleware::from_fn_with_state(
            state,
            flow_dual_ingress_auth_middleware,
        ))
}

fn check_uuid(raw: &str, label: &str) -> Result<String, Response> {
    Uuid::parse_str(raw.trim())
        .map(|u| u.to_string())
        .map_err(|_| {
            flow_err_to_resp(FlowServiceError::new(
                400,
                "VALIDATION_ERROR",
                format!("Invalid {label} UUID"),
            ))
        })
}

fn flow_err_to_resp(err: FlowServiceError) -> Response {
    let status = StatusCode::from_u16(err.status).unwrap_or(StatusCode::BAD_REQUEST);
    let body = HttpErrorBody {
        code: err.code.to_string(),
        kind: Some(err.code.to_string()),
        message: err.message,
        details: serde_json::Value::Null,
        request_id: Uuid::new_v4().to_string(),
    };
    (status, Json(body)).into_response()
}

async fn run_blocking<T, F>(op: F) -> Result<T, Response>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, FlowServiceError> + Send + 'static,
{
    tokio::task::spawn_blocking(op)
        .await
        .map_err(|e| {
            flow_err_to_resp(FlowServiceError::new(
                500,
                "INTERNAL_ERROR",
                format!("Join error: {e}"),
            ))
        })?
        .map_err(flow_err_to_resp)
}

async fn handle_get_snapshot(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<FlowGraphSnapshotDto>, Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out =
        run_blocking(move || flow_graph::get_snapshot(reg.pool(), &principal.tenant_id, &pid))
            .await?;
    Ok(Json(out))
}

async fn handle_list_nodes(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Vec<FlowNodeDto>>, Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || flow_graph::list_nodes(reg.pool(), &principal.tenant_id, &pid))
        .await?;
    Ok(Json(out))
}

async fn handle_create_node(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<CreateFlowNodeInput>,
) -> Result<(StatusCode, Json<FlowNodeDto>), Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::create_node(reg.pool(), &principal.tenant_id, &pid, &input)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(out)))
}

async fn handle_batch_positions(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<BatchUpdatePositionsInput>,
) -> Result<Json<BatchUpdatePositionsResult>, Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::batch_update_positions(reg.pool(), &principal.tenant_id, &pid, &input)
    })
    .await?;
    Ok(Json(out))
}

async fn handle_update_node(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
    Json(input): Json<UpdateFlowNodeInput>,
) -> Result<Json<FlowNodeDto>, Response> {
    let pid = check_uuid(&id, "project")?;
    let nid = check_uuid(&node_id, "node")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::update_node(reg.pool(), &principal.tenant_id, &pid, &nid, &input)
    })
    .await?;
    Ok(Json(out))
}

async fn handle_delete_node(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath((id, node_id)): AxumPath<(String, String)>,
    Query(q): Query<DeleteNodeQuery>,
) -> Result<Json<DeleteFlowNodeResult>, Response> {
    let pid = check_uuid(&id, "project")?;
    let nid = check_uuid(&node_id, "node")?;
    let reg = state.registry.clone();
    let out = run_blocking(move || {
        flow_graph::delete_node(
            reg.pool(),
            &principal.tenant_id,
            &pid,
            &nid,
            q.expected_version,
            q.expected_graph_version,
        )
    })
    .await?;
    Ok(Json(out))
}

async fn handle_list_transitions(
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

async fn handle_create_transition(
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

async fn handle_update_transition(
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

async fn handle_delete_transition(
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

async fn handle_deploy_draft(
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
