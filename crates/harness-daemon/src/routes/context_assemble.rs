// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::VerifiedLegacyIngress;
use axum::{
    body::Bytes,
    extract::{Extension, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use context_compiler::{
    assemble_and_record, list_context_telemetry_scoped, ContextApiErrorDto,
    ContextAssembleHttpRequest, ContextAssembleSuccessDto, ContextAssemblyCommand,
    ContextCompilerError, ContextErrorDetail, ContextTelemetryPageDto, ContextTelemetryQueryDto,
    ModelProfile, PortalId, DEFAULT_MAX_IMAGE_TOKENS_PER_TURN,
};
use domain::HarnessError;
use std::collections::HashSet;
use uuid::Uuid;

const MAX_CONTEXT_BODY_BYTES: usize = 262_144;
const MAX_DOM_OUTLINE_NODES: usize = 128;

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s.trim()).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    field: Option<String>,
    retryable: bool,
    request_id: String,
) -> Response {
    let dto = ContextApiErrorDto {
        ok: false,
        error: ContextErrorDetail {
            code: code.to_string(),
            message: message.to_string(),
            field,
            retryable,
        },
        request_id,
    };
    let mut resp = (status, Json(dto)).into_response();
    if code == "TELEMETRY_WRITE_BUSY" {
        resp.headers_mut()
            .insert("Retry-After", HeaderValue::from_static("1"));
    }
    resp
}

fn compiler_error_response(err: ContextCompilerError, request_id: String) -> Response {
    let (code_num, code, msg_key, field, retryable) = err.descriptor();
    let status = StatusCode::from_u16(code_num).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    error_response(
        status,
        code,
        msg_key,
        field.map(str::to_string),
        retryable,
        request_id,
    )
}

fn idor_error_response(err: HarnessError, request_id: String) -> Response {
    match err {
        HarnessError::ProjectNotFound { .. } | HarnessError::BoundaryViolation { .. } => {
            error_response(
                StatusCode::FORBIDDEN,
                "PROJECT_TENANT_MISMATCH",
                "contextMeter.errors.projectTenantMismatch",
                None,
                false,
                request_id,
            )
        }
        _ => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEMETRY_STORAGE_UNAVAILABLE",
            "contextMeter.errors.telemetryStorageUnavailable",
            None,
            true,
            request_id,
        ),
    }
}

fn validate_idempotency_key(headers: &HeaderMap) -> Result<String, &'static str> {
    let raw = headers
        .get("idempotency-key")
        .or_else(|| headers.get("x-idempotency-key"))
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("INVALID_IDEMPOTENCY_KEY")?;
    let parsed = Uuid::parse_str(raw).map_err(|_| "INVALID_IDEMPOTENCY_KEY")?;
    if parsed.get_version() != Some(uuid::Version::Random) {
        return Err("INVALID_IDEMPOTENCY_KEY");
    }
    Ok(parsed.to_string())
}

fn enforce_portal_role_guard(
    headers: &HeaderMap,
    requested_portal: &mut Option<PortalId>,
    is_superadmin: bool,
    request_id: &str,
) -> Result<(), Response> {
    if let Some(raw_val) = headers.get("x-cowok-portal") {
        let Ok(raw_hdr) = raw_val.to_str() else {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "CROSS_PORTAL_LEAK_BLOCKED",
                "contextMeter.errors.crossPortalLeak",
                Some("portalId".to_string()),
                false,
                request_id.to_string(),
            ));
        };
        let Ok(hp) = serde_json::from_str::<PortalId>(&format!("\"{}\"", raw_hdr.trim())) else {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "CROSS_PORTAL_LEAK_BLOCKED",
                "contextMeter.errors.crossPortalLeak",
                Some("portalId".to_string()),
                false,
                request_id.to_string(),
            ));
        };
        if hp == PortalId::SuperAdmin && !is_superadmin {
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "CROSS_PORTAL_LEAK_BLOCKED",
                "contextMeter.errors.crossPortalLeak",
                Some("portalId".to_string()),
                false,
                request_id.to_string(),
            ));
        }
        match *requested_portal {
            Some(rp) if rp != hp => {
                return Err(error_response(
                    StatusCode::FORBIDDEN,
                    "CROSS_PORTAL_LEAK_BLOCKED",
                    "contextMeter.errors.crossPortalLeak",
                    Some("portalId".to_string()),
                    false,
                    request_id.to_string(),
                ));
            }
            None => *requested_portal = Some(hp),
            _ => {}
        }
    }
    if matches!(*requested_portal, Some(PortalId::SuperAdmin)) && !is_superadmin {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "CROSS_PORTAL_LEAK_BLOCKED",
            "contextMeter.errors.crossPortalLeak",
            Some("portalId".to_string()),
            false,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn storage_or_busy_response(msg: &str, request_id: &str) -> Response {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("busy") || lower.contains("locked") {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEMETRY_WRITE_BUSY",
            "contextMeter.errors.telemetryWriteBusy",
            None,
            true,
            request_id.to_string(),
        )
    } else {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEMETRY_STORAGE_UNAVAILABLE",
            "contextMeter.errors.telemetryStorageUnavailable",
            None,
            true,
            request_id.to_string(),
        )
    }
}

fn resolve_authorized_project_tenant(
    state: &DaemonAppState,
    project_id: Uuid,
    request_id: &str,
) -> Result<String, Response> {
    let conn = state
        .registry
        .pool()
        .connection()
        .map_err(|err| storage_or_busy_response(&err.to_string(), request_id))?;
    match conn.query_row(
        "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
        rusqlite::params![project_id.to_string()],
        |r| r.get::<_, String>(0),
    ) {
        Ok(tenant_id) => Ok(tenant_id),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(error_response(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "contextMeter.errors.projectTenantMismatch",
            None,
            false,
            request_id.to_string(),
        )),
        Err(err) => Err(storage_or_busy_response(&err.to_string(), request_id)),
    }
}

fn validate_unique_sources(req: &ContextAssembleHttpRequest) -> Result<(), (String, String)> {
    if let Some(ref nodes) = req.dom_outline_nodes {
        if nodes.len() > MAX_DOM_OUTLINE_NODES
            || nodes
                .iter()
                .any(|n| n.ff_id.trim().is_empty() || n.tag.trim().is_empty())
        {
            return Err((
                "contextMeter.errors.invalidRequest".to_string(),
                "domOutlineNodes".to_string(),
            ));
        }
    }
    let Some(ref ids) = req.source_ids else {
        return Ok(());
    };
    if ids.is_empty() || ids.len() > 100 {
        return Err((
            "contextMeter.errors.invalidSources".to_string(),
            "sourceIds".to_string(),
        ));
    }
    let mut seen = HashSet::with_capacity(ids.len());
    for id in ids {
        if !seen.insert(*id) {
            return Err((
                "contextMeter.errors.duplicateSourceId".to_string(),
                "sourceIds".to_string(),
            ));
        }
    }
    Ok(())
}

fn parse_assemble_body_or_error(
    body: &[u8],
    request_id: &str,
) -> Result<ContextAssembleHttpRequest, Response> {
    if body.len() > MAX_CONTEXT_BODY_BYTES {
        return Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "contextMeter.errors.invalidRequest",
            None,
            false,
            request_id.to_string(),
        ));
    }
    let req: ContextAssembleHttpRequest = serde_json::from_slice(body).map_err(|_| {
        error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "contextMeter.errors.invalidRequest",
            Some("body".to_string()),
            false,
            request_id.to_string(),
        )
    })?;
    if let Err((msg, field)) = validate_unique_sources(&req) {
        return Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &msg,
            Some(field),
            false,
            request_id.to_string(),
        ));
    }
    Ok(req)
}

fn build_context_command(
    req: ContextAssembleHttpRequest,
    tenant_id: String,
    idem_key: String,
) -> ContextAssemblyCommand {
    ContextAssemblyCommand {
        project_id: req.project_id,
        tenant_id,
        idempotency_key: Some(idem_key),
        portal_id: req.portal_id,
        horizon: req.horizon,
        model_profile: req.model_profile,
        intent: req.intent,
        requested_image_tokens: req.requested_image_tokens,
        max_image_tokens_per_turn: DEFAULT_MAX_IMAGE_TOKENS_PER_TURN,
        source_ids: req.source_ids,
        inspect_only: req.inspect_only.unwrap_or(false),
        brand_tokens_prefix: String::new(),
        active_skill_prefix: String::new(),
        extra_constraints: Vec::new(),
        dom_outline_nodes: req.dom_outline_nodes.unwrap_or_default(),
        optional_blocks: Vec::new(),
    }
}

pub(crate) async fn handle_context_assemble(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let Ok(idem_key) = validate_idempotency_key(&headers) else {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_IDEMPOTENCY_KEY",
            "contextMeter.errors.invalidIdempotencyKey",
            Some("Idempotency-Key".to_string()),
            false,
            request_id,
        );
    };
    let req = match parse_assemble_body_or_error(&body, &request_id) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let (scoped_registry, _, is_superadmin) = match verify_anti_idor_with_caller(
        &state.registry,
        req.project_id,
        &headers,
        caller.as_deref(),
    ) {
        Ok(tuple) => tuple,
        Err(err) => return idor_error_response(err, request_id),
    };
    let mut portal_check = Some(req.portal_id);
    if let Err(resp) =
        enforce_portal_role_guard(&headers, &mut portal_check, is_superadmin, &request_id)
    {
        return resp;
    }
    let tenant_id = match resolve_authorized_project_tenant(&state, req.project_id, &request_id) {
        Ok(tid) => tid,
        Err(resp) => return resp,
    };
    let cmd = build_context_command(req, tenant_id, idem_key);
    let out =
        tokio::task::spawn_blocking(move || assemble_and_record(&scoped_registry, &cmd)).await;
    match out {
        Ok(Ok(bundle)) => (
            StatusCode::OK,
            Json(ContextAssembleSuccessDto {
                ok: true,
                data: bundle.data,
                request_id,
            }),
        )
            .into_response(),
        Ok(Err(err)) => compiler_error_response(err, request_id),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEMETRY_STORAGE_UNAVAILABLE",
            "contextMeter.errors.telemetryStorageUnavailable",
            None,
            true,
            request_id,
        ),
    }
}

fn percent_decode(raw: &str) -> Result<String, ()> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(());
                }
                let hi = (bytes[i + 1] as char).to_digit(16).ok_or(())?;
                let lo = (bytes[i + 2] as char).to_digit(16).ok_or(())?;
                out.push(((hi << 4) | lo) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn parse_telemetry_query(raw_query: Option<&str>) -> Result<ContextTelemetryQueryDto, String> {
    let mut project_id: Option<Uuid> = None;
    let mut portal_id: Option<PortalId> = None;
    let mut model_profile: Option<ModelProfile> = None;
    let mut cursor: Option<String> = None;
    let mut limit: Option<usize> = None;
    let query_str = raw_query.unwrap_or("");
    for pair in query_str.split('&').filter(|p| !p.is_empty()) {
        let (raw_k, raw_v) = pair.split_once('=').unwrap_or((pair, ""));
        let k = percent_decode(raw_k).map_err(|_| raw_k.to_string())?;
        let v = percent_decode(raw_v).map_err(|_| k.clone())?;
        match k.as_str() {
            "projectId" => {
                project_id = Some(Uuid::parse_str(v.trim()).map_err(|_| "projectId".to_string())?);
            }
            "portalId" => {
                portal_id = Some(
                    serde_json::from_str(&format!("\"{v}\""))
                        .map_err(|_| "portalId".to_string())?,
                );
            }
            "modelProfile" => {
                model_profile = Some(
                    serde_json::from_str(&format!("\"{v}\""))
                        .map_err(|_| "modelProfile".to_string())?,
                );
            }
            "cursor" => cursor = Some(v),
            "limit" => {
                let parsed = v.parse::<usize>().map_err(|_| "limit".to_string())?;
                if parsed == 0 || parsed > 100 {
                    return Err("limit".to_string());
                }
                limit = Some(parsed);
            }
            unknown => return Err(unknown.to_string()),
        }
    }
    let Some(pid) = project_id else {
        return Err("projectId".to_string());
    };
    Ok(ContextTelemetryQueryDto {
        project_id: pid,
        portal_id,
        model_profile,
        cursor,
        limit,
    })
}

pub(crate) async fn handle_get_context_telemetry(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let request_id = extract_request_id(&headers);
    let mut query = match parse_telemetry_query(raw_query.as_deref()) {
        Ok(q) => q,
        Err(field) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "contextMeter.errors.invalidRequest",
                Some(field),
                false,
                request_id,
            );
        }
    };
    let (scoped_registry, _, is_superadmin) = match verify_anti_idor_with_caller(
        &state.registry,
        query.project_id,
        &headers,
        caller.as_deref(),
    ) {
        Ok(tuple) => tuple,
        Err(err) => return idor_error_response(err, request_id),
    };
    if let Err(resp) =
        enforce_portal_role_guard(&headers, &mut query.portal_id, is_superadmin, &request_id)
    {
        return resp;
    }
    let tenant_id = match resolve_authorized_project_tenant(&state, query.project_id, &request_id) {
        Ok(tid) => tid,
        Err(resp) => return resp,
    };
    let out = tokio::task::spawn_blocking(move || {
        list_context_telemetry_scoped(scoped_registry.pool(), &tenant_id, &query, is_superadmin)
    })
    .await;
    match out {
        Ok(Ok(data)) => (
            StatusCode::OK,
            Json(ContextTelemetryPageDto {
                ok: true,
                data,
                request_id,
            }),
        )
            .into_response(),
        Ok(Err(err)) => compiler_error_response(err, request_id),
        Err(_) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "TELEMETRY_STORAGE_UNAVAILABLE",
            "contextMeter.errors.telemetryStorageUnavailable",
            None,
            true,
            request_id,
        ),
    }
}
