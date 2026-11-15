// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
use axum::{
    extract::{DefaultBodyLimit, Extension, Path as AxumPath, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use flow_graph::{
    BatchUpdatePositionsInput, BatchUpdatePositionsResult, CreateFlowNodeInput,
    DeleteFlowNodeResult, FlowGraphSnapshotDto, FlowNodeDto, FlowServiceError, UpdateFlowNodeInput,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::daemon_auth::{flow_dual_ingress_auth_middleware, FlowPrincipal};
use crate::flow_transition_handlers::{
    handle_create_transition, handle_delete_transition, handle_deploy_draft,
    handle_list_transitions, handle_update_transition,
};
use crate::routes::doctor_vault::{
    handle_delete_vault_key, handle_get_doctor_cli_matrix, handle_list_plugins,
    handle_list_vault_keys, handle_probe_cli_tool, handle_update_vault_settings,
    handle_upsert_vault_key, handle_verify_provider_key,
};
use crate::routes::execution_coordinator::{
    handle_cancel_node_job, handle_enqueue_node_job, handle_list_node_chat_sessions,
    handle_pause_node_job, handle_resume_node_job, handle_steer_node_job,
};
use crate::routes::execution_events::{handle_post_job_event, handle_stream_job_events};
use crate::routes::execution_sessions::handle_mark_node_session_read;
use crate::routes::legacy_workbench_v6::{
    handle_create_comment, handle_create_fixture, handle_delete_mcp_server,
    handle_list_agent_settings, handle_list_comments, handle_list_fixtures,
    handle_list_mcp_servers, handle_update_comment, handle_update_fixture, handle_update_viewport,
    handle_upsert_agent_setting, handle_upsert_mcp_server,
};

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

fn flow_graph_routes() -> Router<DaemonAppState> {
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
}

fn flow_execution_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/flow/jobs/events",
            post(handle_post_job_event),
        )
        .route(
            "/api/v1/projects/:id/flow/jobs/stream",
            get(handle_stream_job_events),
        )
        .route(
            "/api/v1/projects/:id/flow/jobs/enqueue",
            post(handle_enqueue_node_job),
        )
        .route(
            "/api/v1/projects/:id/flow/jobs/steer",
            post(handle_steer_node_job),
        )
        .route(
            "/api/v1/projects/:id/flow/jobs/pause",
            post(handle_pause_node_job),
        )
        .route(
            "/api/v1/projects/:id/flow/jobs/resume",
            post(handle_resume_node_job),
        )
        .route(
            "/api/v1/projects/:id/flow/jobs/cancel",
            post(handle_cancel_node_job),
        )
        .route(
            "/api/v1/projects/:id/flow/node-sessions",
            get(handle_list_node_chat_sessions),
        )
        .route(
            "/api/v1/projects/:id/flow/node-sessions/read",
            post(handle_mark_node_session_read),
        )
}

fn flow_workbench_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/flow/nodes/:node_id/comments",
            get(handle_list_comments).post(handle_create_comment),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes/:node_id/comments/:comment_id",
            patch(handle_update_comment),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes/:node_id/fixtures",
            get(handle_list_fixtures).post(handle_create_fixture),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes/:node_id/fixtures/:fixture_id",
            patch(handle_update_fixture),
        )
        .route(
            "/api/v1/projects/:id/flow/nodes/:node_id/viewport",
            patch(handle_update_viewport),
        )
        .route(
            "/api/v1/agent-settings",
            get(handle_list_agent_settings).put(handle_upsert_agent_setting),
        )
        .route(
            "/api/v1/mcp-servers",
            get(handle_list_mcp_servers).post(handle_upsert_mcp_server),
        )
        .route(
            "/api/v1/mcp-servers/:server_id",
            delete(handle_delete_mcp_server),
        )
        .route(
            "/api/v1/doctor/verify-key",
            post(handle_verify_provider_key),
        )
        .route(
            "/api/v1/doctor/cli-matrix",
            get(handle_get_doctor_cli_matrix),
        )
        .route("/api/v1/doctor/clis", get(handle_get_doctor_cli_matrix))
        .route("/api/v1/doctor/probe-cli", post(handle_probe_cli_tool))
        .route(
            "/api/v1/vault/keys",
            get(handle_list_vault_keys).post(handle_upsert_vault_key),
        )
        .route(
            "/api/v1/vault/keys/:key_id",
            delete(handle_delete_vault_key),
        )
        .route("/api/v1/settings/vault", put(handle_update_vault_settings))
        .route("/api/v1/plugins", get(handle_list_plugins))
        .route("/api/v1/plugins/registry", get(handle_list_plugins))
}

pub fn build_flow_subrouter(state: DaemonAppState) -> Router<DaemonAppState> {
    Router::new()
        .merge(flow_graph_routes())
        .merge(flow_execution_routes())
        .merge(flow_workbench_routes())
        .layer(DefaultBodyLimit::max(1_048_576))
        .route_layer(middleware::from_fn_with_state(
            state,
            flow_dual_ingress_auth_middleware,
        ))
}

pub(crate) fn check_uuid(raw: &str, label: &str) -> Result<String, Response> {
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

pub(crate) async fn run_blocking<T, F>(op: F) -> Result<T, Response>
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
    Ok(Json(
        run_blocking(move || flow_graph::get_snapshot(reg.pool(), &principal.tenant_id, &pid))
            .await?,
    ))
}

async fn handle_list_nodes(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Vec<FlowNodeDto>>, Response> {
    let pid = check_uuid(&id, "project")?;
    let reg = state.registry.clone();
    Ok(Json(
        run_blocking(move || flow_graph::list_nodes(reg.pool(), &principal.tenant_id, &pid))
            .await?,
    ))
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
    Ok(Json(
        run_blocking(move || {
            flow_graph::batch_update_positions(reg.pool(), &principal.tenant_id, &pid, &input)
        })
        .await?,
    ))
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
    Ok(Json(
        run_blocking(move || {
            flow_graph::update_node(reg.pool(), &principal.tenant_id, &pid, &nid, &input)
        })
        .await?,
    ))
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
