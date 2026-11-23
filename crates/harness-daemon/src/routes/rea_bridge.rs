// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon /api/v1/rea/ingest, /api/v1/rea/search, /api/v1/rea/export-proof)
use axum::{
    body::Bytes,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rea_bridge::{
    ImporterError, ReaBridgeError, ReaBridgeService, ReaExportProofRequest, ReaIngestRequest,
    ReaSearchRequest, ReaSearchTier, ReaStoreError, SourceFormat,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, FlowPrincipal, LegacyAuthKind,
    VerifiedLegacyIngress,
};
use crate::routes::DaemonAppState;

const MAX_REA_INGEST_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_REA_EXPORT_BODY_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
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

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn ok_resp<T: Serialize>(data: T, request_id: String) -> Response {
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

fn err_resp(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    field: Option<&str>,
    details: Option<serde_json::Value>,
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
                details,
            },
        }),
    )
        .into_response()
}

/// Strictly authenticates caller without permitting arbitrary legacy Bearer tokens.
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
            return Err(err_resp(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Legacy bearer authentication is not permitted on /rea bridge endpoints",
                None,
                None,
                request_id.to_string(),
            ));
        }
        return Ok(legacy.principal);
    }
    Err(err_resp(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        None,
        request_id.to_string(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpReaIngestRequest {
    pub slug: String,
    pub source_url: String,
    pub source_format: SourceFormat,
    #[serde(default)]
    pub license_spdx: String,
    #[serde(default)]
    pub summary_hint: Option<String>,
    #[serde(default)]
    pub payload_text: Option<String>,
    #[serde(default)]
    pub payload_base64: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpReaSearchQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_tier")]
    pub tier: String,
}

fn default_limit() -> usize {
    20
}

fn default_tier() -> String {
    "1a".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HttpReaExportProofRequest {
    pub reference_id: String,
    #[serde(default)]
    pub roundtrip_checksum: Option<String>,
}

fn map_bridge_error(err: ReaBridgeError, request_id: String) -> Response {
    match err {
        ReaBridgeError::InvalidSlug(msg) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            msg,
            Some("slug"),
            None,
            request_id,
        ),
        ReaBridgeError::Ssrf(e) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SSRF_URL_BLOCKED",
            e.to_string(),
            Some("sourceUrl"),
            None,
            request_id,
        ),
        ReaBridgeError::FirewallRejected {
            quarantine_id,
            license_spdx,
            policy_rule,
            reason,
        } => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "FIREWALL_REJECTED",
            reason,
            Some("licenseSpdx"),
            Some(serde_json::json!({
                "verdict": "REJECT",
                "quarantineId": quarantine_id,
                "licenseSpdx": license_spdx,
                "policyRule": policy_rule,
            })),
            request_id,
        ),
        ReaBridgeError::Importer(
            e @ (ImporterError::ZipSizeLimitExceeded { .. }
            | ImporterError::ZipRatioExceeded { .. }
            | ImporterError::ZipTooManyEntries { .. }),
        ) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ZIP_BOMB_REJECTED",
            e.to_string(),
            Some("payloadBase64"),
            None,
            request_id,
        ),
        ReaBridgeError::Importer(
            e @ (ImporterError::ZipPathTraversal(_) | ImporterError::ZipSymlinkRejected(_)),
        ) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PATH_JAIL_VIOLATION",
            e.to_string(),
            Some("payloadBase64"),
            None,
            request_id,
        ),
        ReaBridgeError::Importer(e) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_PAYLOAD",
            e.to_string(),
            Some("payloadText"),
            None,
            request_id,
        ),
        ReaBridgeError::Store(ReaStoreError::NotFound(id)) => err_resp(
            StatusCode::NOT_FOUND,
            "REFERENCE_NOT_FOUND",
            format!("Reference '{id}' not found for tenant"),
            Some("referenceId"),
            None,
            request_id,
        ),
        ReaBridgeError::Store(ReaStoreError::SymlinkRejected(msg)) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PATH_JAIL_VIOLATION",
            msg,
            None,
            None,
            request_id,
        ),
        ReaBridgeError::Store(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            "Two-Store cache operation failed",
            None,
            None,
            request_id,
        ),
    }
}

/// `POST /api/v1/rea/ingest`
pub async fn handle_rea_ingest(
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
    if body.len() > MAX_REA_INGEST_BODY_BYTES {
        return err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Ingest payload exceeds 2 MiB limit",
            Some("body"),
            None,
            request_id,
        );
    }
    let parsed: HttpReaIngestRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid /rea/ingest JSON payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };

    let base_dir = state.registry.pool().workspace_root().to_path_buf();
    let tenant_id = principal.tenant_id.clone();
    let res = tokio::task::spawn_blocking(move || {
        let svc = ReaBridgeService::open(&base_dir)?;
        svc.ingest(
            &tenant_id,
            ReaIngestRequest {
                slug: parsed.slug,
                source_url: parsed.source_url,
                source_format: parsed.source_format,
                license_spdx: parsed.license_spdx,
                summary_hint: parsed.summary_hint,
                payload_text: parsed.payload_text,
                payload_base64: parsed.payload_base64,
            },
        )
    })
    .await;

    match res {
        Ok(Ok(outcome)) => ok_resp(outcome, request_id),
        Ok(Err(err)) => map_bridge_error(err, request_id),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "REA ingest worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

/// `GET /api/v1/rea/search?q=&limit=&tier=1a|1b`
pub async fn handle_rea_search(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<HttpReaSearchQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let principal =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(p) => p,
            Err(resp) => return resp,
        };

    let tier = match query.tier.trim().to_ascii_lowercase().as_str() {
        "" | "1a" => ReaSearchTier::Tier1a,
        "1b" => ReaSearchTier::Tier1b,
        _ => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "tier must be '1a' or '1b'",
                Some("tier"),
                None,
                request_id,
            );
        }
    };

    let base_dir = state.registry.pool().workspace_root().to_path_buf();
    let tenant_id = principal.tenant_id.clone();
    let search_req = ReaSearchRequest {
        q: query.q,
        limit: query.limit.clamp(1, 100),
        tier,
    };

    let res = tokio::task::spawn_blocking(move || {
        let svc = ReaBridgeService::open(&base_dir)?;
        svc.search(&tenant_id, search_req)
    })
    .await;

    match res {
        Ok(Ok(result)) => ok_resp(result, request_id),
        Ok(Err(err)) => map_bridge_error(err, request_id),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "REA search worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

/// `POST /api/v1/rea/export-proof`
pub async fn handle_rea_export_proof(
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
    if body.len() > MAX_REA_EXPORT_BODY_BYTES {
        return err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Export proof payload exceeds 32 KiB limit",
            Some("body"),
            None,
            request_id,
        );
    }
    let parsed: HttpReaExportProofRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!("Invalid /rea/export-proof JSON payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };

    let base_dir = state.registry.pool().workspace_root().to_path_buf();
    let tenant_id = principal.tenant_id.clone();
    let res = tokio::task::spawn_blocking(move || {
        let svc = ReaBridgeService::open(&base_dir)?;
        svc.export_proof(
            &tenant_id,
            ReaExportProofRequest {
                reference_id: parsed.reference_id,
                roundtrip_checksum: parsed.roundtrip_checksum,
            },
        )
    })
    .await;

    match res {
        Ok(Ok(proof)) => ok_resp(proof, request_id),
        Ok(Err(err)) => map_bridge_error(err, request_id),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "REA proof exporter worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpReaCatalogQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_catalog_limit")]
    pub limit: usize,
}

fn default_catalog_limit() -> usize {
    5
}

/// `GET /api/v1/rea/catalog?q=&limit=5`
pub async fn handle_rea_catalog(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<HttpReaCatalogQuery>,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    if let Err(resp) =
        extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id)
    {
        return resp;
    }

    let base_dir = state.registry.pool().workspace_root().to_path_buf();
    let q = query.q;
    let limit = query.limit.clamp(1, 20);
    let res = tokio::task::spawn_blocking(move || {
        let svc = ReaBridgeService::open(&base_dir)?;
        svc.search_catalog(&q, limit)
    })
    .await;

    match res {
        Ok(Ok(catalog)) => ok_resp(catalog, request_id),
        Ok(Err(err)) => map_bridge_error(err, request_id),
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "REA catalog worker failed unexpectedly",
            None,
            None,
            request_id,
        ),
    }
}

/// Standalone Axum subrouter for Story 09.1 Two-Store `/rea` Bridge routes.
pub fn router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/rea/ingest", post(handle_rea_ingest))
        .route("/api/v1/rea/search", get(handle_rea_search))
        .route("/api/v1/rea/catalog", get(handle_rea_catalog))
        .route("/api/v1/rea/export-proof", post(handle_rea_export_proof))
}

/// Alias matching `build_de_runtime_router()` / `build_bot_fleet_router()` naming convention.
pub fn build_rea_bridge_router() -> Router<DaemonAppState> {
    router()
}
