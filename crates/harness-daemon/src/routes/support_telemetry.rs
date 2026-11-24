// @story 14.1
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon HTTP routes for /api/v1/support/tickets, /api/v1/telemetry/events, /api/v1/telemetry/preferences)
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Extension, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, FlowPrincipal, LegacyAuthKind,
    VerifiedLegacyIngress,
};
use crate::routes::DaemonAppState;
use crate::support_edge::{
    create_support_ticket, enqueue_telemetry_event, get_telemetry_preferences,
    list_support_tickets, read_support_ticket_attachment, set_telemetry_preferences,
    CreateSupportTicketInput, EnqueueTelemetryInput, LocalSupportTicketDto, SupportEdgeError,
};

const MAX_SUPPORT_TICKET_BODY_BYTES: usize = 8 * 1024 * 1024;
const SUPPORT_TICKET_ROUTE_BODY_LIMIT: usize = MAX_SUPPORT_TICKET_BODY_BYTES + 64 * 1024;
const MAX_TELEMETRY_EVENT_BODY_BYTES: usize = 64 * 1024;
const MAX_TELEMETRY_PREFS_BODY_BYTES: usize = 4 * 1024;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportTicketListPayload {
    tickets: Vec<LocalSupportTicketDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSupportTicketsQuery {
    #[serde(default = "default_tickets_limit")]
    pub limit: usize,
}

fn default_tickets_limit() -> usize {
    50
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTelemetryPreferencesBody {
    pub telemetry_enabled: bool,
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

fn json_ok<T: Serialize>(status: StatusCode, data: T, request_id: String) -> Response {
    (
        status,
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
    field: Option<&str>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.into(),
                field: field.map(str::to_string),
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
                "Legacy bearer authentication is not permitted on support/telemetry endpoints",
                None,
                request_id.to_string(),
            ));
        }
        return Ok(legacy.principal);
    }
    Err(json_err(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        request_id.to_string(),
    ))
}

fn map_support_edge_error(err: SupportEdgeError, request_id: String) -> Response {
    match err {
        SupportEdgeError::Validation {
            code,
            field,
            message,
        } => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            message,
            field,
            request_id,
        ),
        SupportEdgeError::PayloadTooLarge { field, message } => json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            message,
            field,
            request_id,
        ),
        SupportEdgeError::Storage { context, .. } => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            format!("Local support/telemetry storage operation failed ({context})"),
            None,
            request_id,
        ),
    }
}

/// `POST /api/v1/support/tickets`
pub async fn handle_create_support_ticket(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
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
    if body.len() > MAX_SUPPORT_TICKET_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Support ticket payload exceeds 8 MiB limit",
            Some("body"),
            request_id,
        );
    }
    let parsed: CreateSupportTicketInput = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid /support/tickets JSON payload: {e}"),
                Some("body"),
                request_id,
            );
        }
    };

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || create_support_ticket(&pool, &parsed)).await;
    match res {
        Ok(Ok(ticket)) => json_ok(StatusCode::CREATED, ticket, request_id),
        Ok(Err(err)) => map_support_edge_error(err, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Support ticket worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/support/tickets`
pub async fn handle_list_support_tickets(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<ListSupportTicketsQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    if let Err(resp) =
        extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id)
    {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let limit = query.limit.clamp(1, 200);
    let res = tokio::task::spawn_blocking(move || list_support_tickets(&pool, limit)).await;
    match res {
        Ok(Ok(tickets)) => json_ok(
            StatusCode::OK,
            SupportTicketListPayload { tickets },
            request_id,
        ),
        Ok(Err(err)) => map_support_edge_error(err, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Support ticket listing worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/support/tickets/:id/attachment`
pub async fn handle_get_support_ticket_attachment(
    State(state): State<DaemonAppState>,
    Path(ticket_id): Path<String>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    if let Err(resp) =
        extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id)
    {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let res =
        tokio::task::spawn_blocking(move || read_support_ticket_attachment(&pool, &ticket_id))
            .await;
    match res {
        Ok(Ok(Some((bytes, content_type)))) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::CACHE_CONTROL, "private, max-age=3600"),
            ],
            bytes,
        )
            .into_response(),
        Ok(Ok(None)) => json_err(
            StatusCode::NOT_FOUND,
            "ATTACHMENT_NOT_FOUND",
            "Support ticket attachment not found",
            None,
            request_id,
        ),
        Ok(Err(err)) => map_support_edge_error(err, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Support ticket attachment reader failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// `POST /api/v1/telemetry/events`
pub async fn handle_ingest_telemetry_event(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
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
    if body.len() > MAX_TELEMETRY_EVENT_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Telemetry event payload exceeds 64 KiB limit",
            Some("body"),
            request_id,
        );
    }
    let parsed: EnqueueTelemetryInput = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid /telemetry/events JSON payload: {e}"),
                Some("body"),
                request_id,
            );
        }
    };

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || enqueue_telemetry_event(&pool, &parsed)).await;
    match res {
        Ok(Ok(outcome)) => json_ok(StatusCode::OK, outcome, request_id),
        Ok(Err(err)) => map_support_edge_error(err, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Telemetry event worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/telemetry/preferences`
pub async fn handle_get_telemetry_preferences(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    if let Err(resp) =
        extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id)
    {
        return resp;
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || get_telemetry_preferences(&pool)).await;
    match res {
        Ok(Ok(prefs)) => json_ok(StatusCode::OK, prefs, request_id),
        Ok(Err(err)) => map_support_edge_error(err, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Telemetry preference read worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// `PUT /api/v1/telemetry/preferences`
pub async fn handle_put_telemetry_preferences(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
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
    if body.len() > MAX_TELEMETRY_PREFS_BODY_BYTES {
        return json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Telemetry preferences payload exceeds 4 KiB limit",
            Some("body"),
            request_id,
        );
    }
    let parsed: UpdateTelemetryPreferencesBody = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid /telemetry/preferences JSON payload: {e}"),
                Some("body"),
                request_id,
            );
        }
    };

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        set_telemetry_preferences(&pool, parsed.telemetry_enabled)
    })
    .await;
    match res {
        Ok(Ok(prefs)) => json_ok(StatusCode::OK, prefs, request_id),
        Ok(Err(err)) => map_support_edge_error(err, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Telemetry preference update worker failed unexpectedly",
            None,
            request_id,
        ),
    }
}

/// Standalone Axum subrouter for Story 14.1 Support & Telemetry Client Edge routes.
pub fn router() -> Router<DaemonAppState> {
    let support_tickets_route = Router::new()
        .route(
            "/api/v1/support/tickets",
            get(handle_list_support_tickets).post(handle_create_support_ticket),
        )
        .layer(DefaultBodyLimit::max(SUPPORT_TICKET_ROUTE_BODY_LIMIT));

    Router::new()
        .merge(support_tickets_route)
        .route(
            "/api/v1/support/tickets/:id/attachment",
            get(handle_get_support_ticket_attachment),
        )
        .route(
            "/api/v1/telemetry/events",
            post(handle_ingest_telemetry_event),
        )
        .route(
            "/api/v1/telemetry/preferences",
            get(handle_get_telemetry_preferences).put(handle_put_telemetry_preferences),
        )
}

/// Alias matching `build_rea_bridge_router()` / `build_decision_router()` naming convention.
pub fn build_support_telemetry_router() -> Router<DaemonAppState> {
    router()
}
