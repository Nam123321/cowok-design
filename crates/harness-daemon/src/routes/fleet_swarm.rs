// @story 10.2
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon /api/v1/fleet/leases/acquire, /api/v1/fleet/dispatch & /api/v1/fleet/jury/evaluate)
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use bot_fleet::design_jury::{
    enforce_builder_neq_scorer, evaluate_design_jury_session, ContrastMeasurement, DesignJuryError,
    DesignJurySessionInput, ElementBounds, JuryLensScore, JuryVerdictStatus, QcIssueInput,
    QcRunInput, QcViolationItem, ReviewFreezeBundle, HUMAN_ONLY_APPROVAL_POLICY,
};
use bot_fleet::swarm_leases::{
    canonicalize_and_check_self_disjoint, check_conflict_against_active_leases,
    is_safe_worker_pgid, reap_expired_leases_and_kill_pgid, validate_disjoint_write_sets,
    validate_safe_worker_id, CoordinationMailbox, OneJobContract, PgidKiller, RateLimitSemaphore,
    SwarmLeaseError, WorkerLeaseRecord, WorkerRole, COORDINATION_REL_DIR, DEFAULT_LEASE_TTL_MS,
    MAX_LEASE_TTL_MS, MIN_LEASE_TTL_MS,
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::SqliteActorError;
use uuid::Uuid;

use super::execution_reaper::{terminate_pgid_checked, ActiveWorkerRegistry};
use super::orchestrator::shared_write_actor_for_pool;
use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, FlowPrincipal, LegacyAuthKind,
    VerifiedLegacyIngress,
};

const MAX_FLEET_SWARM_BODY_BYTES: usize = 524_288;
const MAX_DISPATCH_TASKS: usize = 32;

fn project_semaphores() -> &'static Mutex<HashMap<String, RateLimitSemaphore>> {
    static MAP: OnceLock<Mutex<HashMap<String, RateLimitSemaphore>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns the shared adaptive `429` concurrency semaphore (`3 -> 1`) scoped to `(tenant_id, project_id)`.
pub fn shared_project_rate_semaphore(tenant_id: &str, project_id: &str) -> RateLimitSemaphore {
    let key = format!("{tenant_id}::{project_id}");
    let mut guard = project_semaphores()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    guard.entry(key).or_default().clone()
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

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn json_ok<T: Serialize>(data: T, request_id: String) -> Response {
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

fn json_err(
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

fn extract_principal_strict(
    headers: &HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    request_id: &str,
) -> Result<(FlowPrincipal, bool), Response> {
    let superadmin = is_valid_superadmin_bearer(headers);
    if let Some(Extension(principal)) = flow_principal {
        return Ok((principal, superadmin));
    }
    if let Some(Extension(legacy)) = legacy_ingress {
        if legacy.kind == LegacyAuthKind::Bearer && !superadmin {
            return Err(json_err(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Arbitrary bearer tokens are not permitted on fleet swarm endpoints",
                None,
                None,
                request_id.to_string(),
            ));
        }
        return Ok((legacy.principal, superadmin));
    }
    Err(json_err(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Missing verified authentication credentials",
        None,
        None,
        request_id.to_string(),
    ))
}

fn resolve_tenant_project_workspace(
    state: &DaemonAppState,
    principal: &FlowPrincipal,
    is_superadmin: bool,
    project_id: &str,
    request_id: &str,
) -> Result<(String, PathBuf), Response> {
    let pid = project_id.trim();
    if pid.is_empty() || pid.len() > 128 {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "projectId must be 1..=128 characters",
            Some("projectId"),
            None,
            request_id.to_string(),
        ));
    }
    let conn = state.registry.pool().connection().map_err(|e| {
        json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            format!("Database connection error: {e}"),
            None,
            None,
            request_id.to_string(),
        )
    })?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT tenant_id, workspace_root FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![pid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| {
            json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                format!("Failed to resolve project: {e}"),
                None,
                None,
                request_id.to_string(),
            )
        })?;
    let Some((project_tenant, workspace_root)) = row else {
        return Err(json_err(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found or inaccessible",
            Some("projectId"),
            None,
            request_id.to_string(),
        ));
    };
    if !is_superadmin && project_tenant != principal.tenant_id {
        return Err(json_err(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found or inaccessible",
            Some("projectId"),
            None,
            request_id.to_string(),
        ));
    }
    Ok((project_tenant, PathBuf::from(workspace_root)))
}

fn parse_bounded_json<T: for<'de> Deserialize<'de>>(
    body: &Bytes,
    request_id: &str,
) -> Result<T, Response> {
    if body.is_empty() {
        return Err(json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Request body must not be empty",
            None,
            None,
            request_id.to_string(),
        ));
    }
    if body.len() > MAX_FLEET_SWARM_BODY_BYTES {
        return Err(json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            format!(
                "Request body exceeds maximum {} bytes",
                MAX_FLEET_SWARM_BODY_BYTES
            ),
            None,
            None,
            request_id.to_string(),
        ));
    }
    serde_json::from_slice(body).map_err(|e| {
        json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            format!("Invalid JSON payload: {e}"),
            None,
            None,
            request_id.to_string(),
        )
    })
}

fn map_actor_err(err: SqliteActorError, request_id: &str) -> Response {
    match err {
        SqliteActorError::Backpressure {
            retry_after_secs,
            queue_capacity,
        } => json_err(
            StatusCode::TOO_MANY_REQUESTS,
            "STORAGE_BACKPRESSURE",
            "SQLite write actor queue saturated; retry shortly",
            None,
            Some(serde_json::json!({
                "retryAfterSecs": retry_after_secs,
                "queueCapacity": queue_capacity,
            })),
            request_id.to_string(),
        ),
        other => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            format!("Write actor failed: {other}"),
            None,
            None,
            request_id.to_string(),
        ),
    }
}

fn map_lease_error(err: SwarmLeaseError, request_id: &str) -> Response {
    match err {
        SwarmLeaseError::LeasePathConflict {
            requested,
            held_by,
            held_path,
        } => json_err(
            StatusCode::CONFLICT,
            "LEASE_PATH_CONFLICT",
            format!(
                "Requested path '{requested}' overlaps active lease '{held_path}' held by worker '{held_by}'"
            ),
            Some("ownedPaths"),
            Some(serde_json::json!({
                "requestedPath": requested,
                "conflictingWorkerId": held_by,
                "conflictingPath": held_path,
            })),
            request_id.to_string(),
        ),
        SwarmLeaseError::SymlinkViolation(msg) => json_err(
            StatusCode::BAD_REQUEST,
            "SYMLINK_VIOLATION",
            msg,
            Some("ownedPaths"),
            None,
            request_id.to_string(),
        ),
        SwarmLeaseError::PathTraversal(msg) => json_err(
            StatusCode::BAD_REQUEST,
            "PATH_TRAVERSAL",
            msg,
            Some("ownedPaths"),
            None,
            request_id.to_string(),
        ),
        SwarmLeaseError::InvalidPath(msg) => json_err(
            StatusCode::BAD_REQUEST,
            "INVALID_LEASE_PATH",
            msg,
            Some("ownedPaths"),
            None,
            request_id.to_string(),
        ),
        SwarmLeaseError::StaleFencingToken { expected, provided } => json_err(
            StatusCode::CONFLICT,
            "STALE_FENCING_TOKEN",
            format!("Stale fencing_token: expected {expected}, provided {provided}"),
            Some("fencingToken"),
            Some(serde_json::json!({ "expected": expected, "provided": provided })),
            request_id.to_string(),
        ),
        other => json_err(
            StatusCode::BAD_REQUEST,
            "INVALID_LEASE_REQUEST",
            other.to_string(),
            None,
            None,
            request_id.to_string(),
        ),
    }
}

fn default_lease_ttl_ms() -> i64 {
    DEFAULT_LEASE_TTL_MS
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcquireWorkerLeaseRequest {
    pub project_id: String,
    pub worker_id: String,
    pub worker_role: WorkerRole,
    pub owned_paths: Vec<String>,
    #[serde(default = "default_lease_ttl_ms")]
    pub ttl_ms: i64,
    #[serde(default)]
    pub worker_pgid: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireWorkerLeaseData {
    pub lease_id: String,
    pub project_id: String,
    pub worker_id: String,
    pub worker_role: WorkerRole,
    pub fencing_token: i64,
    pub owned_paths: Vec<String>,
    pub expires_at: i64,
    pub created_at: i64,
    pub reaped_expired_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SwarmWorkerTaskInput {
    pub worker_id: String,
    pub worker_role: WorkerRole,
    pub one_job: String,
    pub never_rules: Vec<String>,
    pub owned_paths: Vec<String>,
    #[serde(default = "default_lease_ttl_ms")]
    pub ttl_ms: i64,
    #[serde(default)]
    pub worker_pgid: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispatchFleetSwarmRequest {
    pub project_id: String,
    pub session_id: String,
    pub tasks: Vec<SwarmWorkerTaskInput>,
    #[serde(default)]
    pub upstream_http_status: Option<u16>,
    #[serde(default)]
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchedTaskAssignment {
    pub task_id: String,
    pub lease_id: String,
    pub worker_id: String,
    pub worker_role: WorkerRole,
    pub fencing_token: i64,
    pub owned_paths: Vec<String>,
    pub envelope_rel_path: String,
    pub dispatch_state: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchFleetSwarmData {
    pub dispatch_id: String,
    pub project_id: String,
    pub session_id: String,
    pub concurrency_limit: usize,
    pub clamped_on_429: bool,
    pub backoff_ms: u64,
    pub reaped_expired_count: usize,
    pub assignments: Vec<DispatchedTaskAssignment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluateDesignJuryRequest {
    pub project_id: String,
    pub screen_id: String,
    pub builder_worker_id: String,
    pub scorer_worker_id: String,
    pub scorer_role: WorkerRole,
    pub generation_fence: i64,
    #[serde(default)]
    pub expected_generation_fence: Option<i64>,
    pub freeze_snapshot_sha256: String,
    #[serde(default)]
    pub frozen_html: Option<String>,
    #[serde(default)]
    pub qc_run_id: Option<String>,
    #[serde(default)]
    pub reported_issues: Vec<QcIssueInput>,
    #[serde(default)]
    pub expected_defect_ids: Vec<String>,
    #[serde(default)]
    pub contrast_ratios: Vec<ContrastMeasurement>,
    #[serde(default)]
    pub bounding_boxes: Vec<ElementBounds>,
    #[serde(default)]
    pub brand_tokens_expected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateDesignJuryData {
    pub verdict_id: String,
    pub project_id: String,
    pub screen_id: String,
    pub builder_worker_id: String,
    pub scorer_worker_id: String,
    pub generation_fence: i64,
    pub freeze_snapshot_sha256: String,
    pub qc_run_id: Option<String>,
    pub e1_e4_catch_rate: f64,
    pub verdict: JuryVerdictStatus,
    pub lens_scores: Vec<JuryLensScore>,
    pub violations: Vec<QcViolationItem>,
    pub recommended_fixes: Vec<String>,
    pub escalated_to_ledger: bool,
    pub ledger_entry_id: Option<String>,
    pub approval_policy: Option<String>,
}

fn load_unreleased_leases_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: &str,
) -> Result<Vec<WorkerLeaseRecord>, rusqlite::Error> {
    let mut stmt = tx.prepare(
        "SELECT lease_id, project_id, worker_id, worker_role, worker_pgid, fencing_token,
                owned_paths_json, expires_at, released, created_at
         FROM worker_file_leases
         WHERE project_id = ?1 AND released = 0",
    )?;
    let rows = stmt.query_map(params![project_id], |r| {
        let role_str: String = r.get(3)?;
        let paths_json: String = r.get(6)?;
        let released_int: i64 = r.get(8)?;
        let owned_paths: Vec<String> = serde_json::from_str(&paths_json).unwrap_or_default();
        let worker_role = WorkerRole::parse_strict(&role_str).unwrap_or(WorkerRole::Builder);
        Ok(WorkerLeaseRecord {
            lease_id: r.get(0)?,
            project_id: r.get(1)?,
            worker_id: r.get(2)?,
            worker_role,
            worker_pgid: r.get(4)?,
            fencing_token: r.get(5)?,
            owned_paths,
            expires_at: r.get(7)?,
            released: released_int != 0,
            created_at: r.get(9)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Daemon-verified PGID killer that ONLY signals process groups actively registered
/// as daemon-spawned child workers in `ActiveWorkerRegistry` for this `project_id`,
/// verifying live process `start_time` via `execution_reaper::terminate_pgid_checked`.
///
/// Unowned external caller-supplied PGIDs are NEVER signalled; their expired leases are
/// safely released without touching arbitrary host process groups.
struct DaemonOwnedPgidKiller {
    active_workers: ActiveWorkerRegistry,
    project_id: String,
}

impl PgidKiller for DaemonOwnedPgidKiller {
    fn kill_pgid(&self, pgid: i32) -> Result<(), SwarmLeaseError> {
        if !is_safe_worker_pgid(pgid) {
            return Err(SwarmLeaseError::UnsafePgid(pgid));
        }
        let entries = self
            .active_workers
            .take_by_project_and_pgid(&self.project_id, pgid);
        if entries.is_empty() {
            return Ok(());
        }
        let mut first_err: Option<String> = None;
        for entry in entries {
            if let Err(err) = terminate_pgid_checked(entry.pid, entry.pgid, &entry.start_time) {
                self.active_workers.register(entry);
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
        if let Some(err) = first_err {
            return Err(SwarmLeaseError::Io(format!(
                "terminate_pgid_checked(-{pgid}) failed: {err}"
            )));
        }
        Ok(())
    }
}

fn reap_expired_leases_in_tx(
    tx: &rusqlite::Transaction<'_>,
    leases: &mut [WorkerLeaseRecord],
    now_ms: i64,
    active_workers: &ActiveWorkerRegistry,
    project_id: &str,
) -> Result<usize, rusqlite::Error> {
    let killer = DaemonOwnedPgidKiller {
        active_workers: active_workers.clone(),
        project_id: project_id.to_string(),
    };
    let outcomes = reap_expired_leases_and_kill_pgid(leases, now_ms, &killer);
    let mut reaped_count = 0usize;
    for item in &outcomes {
        if item.released {
            tx.execute(
                "UPDATE worker_file_leases SET released = 1 WHERE lease_id = ?1",
                params![item.lease_id],
            )?;
            reaped_count += 1;
        }
    }
    Ok(reaped_count)
}

fn next_fencing_token_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: &str,
) -> Result<i64, rusqlite::Error> {
    let current_max: i64 = tx.query_row(
        "SELECT COALESCE(MAX(fencing_token), 0) FROM worker_file_leases WHERE project_id = ?1",
        params![project_id],
        |r| r.get(0),
    )?;
    Ok(current_max.saturating_add(1))
}

/// `POST /api/v1/fleet/leases/acquire`
pub async fn handle_acquire_worker_lease(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, is_superadmin) =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(v) => v,
            Err(resp) => return resp,
        };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }

    let req: AcquireWorkerLeaseRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if !(MIN_LEASE_TTL_MS..=MAX_LEASE_TTL_MS).contains(&req.ttl_ms) {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            format!("ttlMs must be between {MIN_LEASE_TTL_MS} and {MAX_LEASE_TTL_MS}"),
            Some("ttlMs"),
            None,
            request_id,
        );
    }

    if let Some(pgid) = req.worker_pgid {
        if !is_safe_worker_pgid(pgid) {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                format!(
                    "workerPgid {pgid} is unsafe (must be > 1 and distinct from daemon process group)"
                ),
                Some("workerPgid"),
                None,
                request_id,
            );
        }
    }

    let worker_id = match validate_safe_worker_id(&req.worker_id, "workerId") {
        Ok(id) => id,
        Err(e) => return map_lease_error(e, &request_id),
    };

    let (_project_tenant, workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &req.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let canonical_paths = match canonicalize_and_check_self_disjoint(
        Some(&workspace_root),
        &worker_id,
        &req.owned_paths,
    ) {
        Ok(p) => p,
        Err(e) => return map_lease_error(e, &request_id),
    };

    let actor = match shared_write_actor_for_pool(state.registry.pool()) {
        Ok(a) => a,
        Err(e) => return map_actor_err(e, &request_id),
    };

    let project_id = req.project_id.trim().to_string();
    let worker_role = req.worker_role;
    let worker_pgid = req.worker_pgid;
    let ttl_ms = req.ttl_ms;
    let active_workers = state.active_workers.clone();

    let write_res = tokio::task::spawn_blocking(move || {
        actor.execute_immediate(
            move |tx| -> Result<Result<AcquireWorkerLeaseData, SwarmLeaseError>, rusqlite::Error> {
                let now_ms = Utc::now().timestamp_millis();
                let mut active = load_unreleased_leases_tx(tx, &project_id)?;
                let reaped_expired_count = reap_expired_leases_in_tx(
                    tx,
                    &mut active,
                    now_ms,
                    &active_workers,
                    &project_id,
                )?;

                if let Err(conflict) =
                    check_conflict_against_active_leases(&canonical_paths, &active, now_ms)
                {
                    return Ok(Err(conflict));
                }

                let fencing_token = next_fencing_token_tx(tx, &project_id)?;
                let lease_id = Uuid::new_v4().to_string();
                let expires_at = now_ms.saturating_add(ttl_ms);
                let owned_paths_json =
                    serde_json::to_string(&canonical_paths).unwrap_or_else(|_| "[]".to_string());

                tx.execute(
                    "INSERT INTO worker_file_leases (
                    lease_id, project_id, worker_id, worker_role, worker_pgid,
                    fencing_token, owned_paths_json, expires_at, released, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
                    params![
                        lease_id,
                        project_id,
                        worker_id,
                        worker_role.as_str(),
                        worker_pgid,
                        fencing_token,
                        owned_paths_json,
                        expires_at,
                        now_ms,
                    ],
                )?;

                Ok(Ok(AcquireWorkerLeaseData {
                    lease_id,
                    project_id,
                    worker_id,
                    worker_role,
                    fencing_token,
                    owned_paths: canonical_paths,
                    expires_at,
                    created_at: now_ms,
                    reaped_expired_count,
                }))
            },
        )
    })
    .await;

    match write_res {
        Ok(Ok(Ok(data))) => json_ok(data, request_id),
        Ok(Ok(Err(lease_err))) => map_lease_error(lease_err, &request_id),
        Ok(Err(actor_err)) => map_actor_err(actor_err, &request_id),
        Err(join_err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Lease acquire task failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

/// `POST /api/v1/fleet/dispatch`
pub async fn handle_dispatch_fleet_swarm(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, is_superadmin) =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(v) => v,
            Err(resp) => return resp,
        };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }

    let req: DispatchFleetSwarmRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if req.tasks.is_empty() || req.tasks.len() > MAX_DISPATCH_TASKS {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            format!("tasks must contain 1..={MAX_DISPATCH_TASKS} worker assignments"),
            Some("tasks"),
            None,
            request_id,
        );
    }
    let session_id = match validate_safe_worker_id(&req.session_id, "sessionId") {
        Ok(s) => s,
        Err(e) => return map_lease_error(e, &request_id),
    };

    for task in &req.tasks {
        if !(MIN_LEASE_TTL_MS..=MAX_LEASE_TTL_MS).contains(&task.ttl_ms) {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                format!("Each task ttlMs must be {MIN_LEASE_TTL_MS}..={MAX_LEASE_TTL_MS}"),
                Some("tasks"),
                None,
                request_id,
            );
        }
        if let Some(pgid) = task.worker_pgid {
            if !is_safe_worker_pgid(pgid) {
                return json_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_REQUEST",
                    format!(
                        "Task workerPgid {pgid} is unsafe (must be > 1 and distinct from daemon process group)"
                    ),
                    Some("tasks"),
                    None,
                    request_id,
                );
            }
        }
        if task.one_job.trim().is_empty()
            || task.never_rules.is_empty()
            || task.never_rules.iter().all(|r| r.trim().is_empty())
        {
            return json_err(
                StatusCode::BAD_REQUEST,
                "INVALID_CONTRACT",
                "Each task must specify non-empty oneJob and neverRules (ONE_JOB + NEVER)",
                Some("tasks"),
                None,
                request_id,
            );
        }
    }

    let (project_tenant, workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &req.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    // 1. Validate pairwise disjointness across the entire batch + canonicalize & reject symlinks/traversal.
    let raw_sets: Vec<(String, Vec<String>)> = req
        .tasks
        .iter()
        .map(|t| (t.worker_id.clone(), t.owned_paths.clone()))
        .collect();
    let canonical_sets = match validate_disjoint_write_sets(Some(&workspace_root), &raw_sets) {
        Ok(sets) => sets,
        Err(e) => return map_lease_error(e, &request_id),
    };

    // 2. Open and validate coordination mailbox BEFORE opening the SQLite transaction.
    let mailbox = match CoordinationMailbox::open(&workspace_root) {
        Ok(m) => m,
        Err(e) => return map_lease_error(e, &request_id),
    };

    // 3. Atomically reap expired leases, verify zero overlap with active DB leases, and acquire all batch leases.
    let actor = match shared_write_actor_for_pool(state.registry.pool()) {
        Ok(a) => a,
        Err(e) => return map_actor_err(e, &request_id),
    };

    struct PreparedTask {
        worker_id: String,
        worker_role: WorkerRole,
        one_job: String,
        never_rules: Vec<String>,
        owned_paths: Vec<String>,
        ttl_ms: i64,
        worker_pgid: Option<i32>,
    }

    type PreparedLeaseRecord = (PreparedTask, String, i64, i64, i64);
    type DispatchTxOutcome = Result<(usize, Vec<PreparedLeaseRecord>), SwarmLeaseError>;

    let upstream_http_status = req.upstream_http_status;
    let retry_after_ms = req.retry_after_ms;
    let prepared: Vec<PreparedTask> = req
        .tasks
        .into_iter()
        .zip(canonical_sets)
        .map(|(t, (worker_id, owned_paths))| PreparedTask {
            worker_id,
            worker_role: t.worker_role,
            one_job: t.one_job,
            never_rules: t.never_rules,
            owned_paths,
            ttl_ms: t.ttl_ms,
            worker_pgid: t.worker_pgid,
        })
        .collect();

    let project_id = req.project_id.trim().to_string();
    let project_id_for_tx = project_id.clone();
    let actor_for_tx = actor.clone();
    let active_workers = state.active_workers.clone();

    let acquired_res = tokio::task::spawn_blocking(move || {
        actor_for_tx.execute_immediate(move |tx| -> Result<DispatchTxOutcome, rusqlite::Error> {
            let now_ms = Utc::now().timestamp_millis();
            let mut active = load_unreleased_leases_tx(tx, &project_id_for_tx)?;
            let reaped_count = reap_expired_leases_in_tx(
                tx,
                &mut active,
                now_ms,
                &active_workers,
                &project_id_for_tx,
            )?;

            for task in &prepared {
                if let Err(conflict) =
                    check_conflict_against_active_leases(&task.owned_paths, &active, now_ms)
                {
                    return Ok(Err(conflict));
                }
            }

            let mut next_token = next_fencing_token_tx(tx, &project_id_for_tx)?;
            let mut issued = Vec::with_capacity(prepared.len());

            for task in prepared {
                let lease_id = Uuid::new_v4().to_string();
                let fencing_token = next_token;
                next_token = next_token.saturating_add(1);
                let expires_at = now_ms.saturating_add(task.ttl_ms);
                let owned_paths_json =
                    serde_json::to_string(&task.owned_paths).unwrap_or_else(|_| "[]".to_string());

                tx.execute(
                    "INSERT INTO worker_file_leases (
                        lease_id, project_id, worker_id, worker_role, worker_pgid,
                        fencing_token, owned_paths_json, expires_at, released, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
                    params![
                        lease_id,
                        project_id_for_tx,
                        task.worker_id,
                        task.worker_role.as_str(),
                        task.worker_pgid,
                        fencing_token,
                        owned_paths_json,
                        expires_at,
                        now_ms,
                    ],
                )?;

                issued.push((task, lease_id, fencing_token, expires_at, now_ms));
            }

            Ok(Ok((reaped_count, issued)))
        })
    })
    .await;

    let (reaped_expired_count, issued_leases) = match acquired_res {
        Ok(Ok(Ok(pair))) => pair,
        Ok(Ok(Err(lease_err))) => return map_lease_error(lease_err, &request_id),
        Ok(Err(actor_err)) => return map_actor_err(actor_err, &request_id),
        Err(join_err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Fleet dispatch task failed: {join_err}"),
                None,
                None,
                request_id,
            )
        }
    };

    // 4. Write `.cowok-design/coordination/` `ONE_JOB + NEVER` task envelopes outside SQLite tx.
    //    If any envelope write fails mid-batch, unlink written envelopes and compensate in SQLite.
    struct StagedAssignment {
        task_id: String,
        lease_id: String,
        worker_id: String,
        worker_role: WorkerRole,
        fencing_token: i64,
        owned_paths: Vec<String>,
        expires_at: i64,
    }

    let issued_lease_ids: Vec<String> = issued_leases
        .iter()
        .map(|(_, lease_id, _, _, _)| lease_id.clone())
        .collect();
    let mut written_envelope_paths: Vec<PathBuf> = Vec::with_capacity(issued_leases.len());
    let mut staged_assignments: Vec<StagedAssignment> = Vec::with_capacity(issued_leases.len());

    for (task, lease_id, fencing_token, expires_at, now_ms) in issued_leases {
        let task_id = format!("task-{}", Uuid::new_v4().simple());
        let staging_rel_dir = format!("{COORDINATION_REL_DIR}/staging/{lease_id}");
        let contract = OneJobContract {
            task_id: task_id.clone(),
            project_id: project_id.clone(),
            worker_id: task.worker_id.clone(),
            worker_role: task.worker_role,
            lease_id: lease_id.clone(),
            fencing_token,
            one_job: task.one_job,
            never_rules: task.never_rules,
            owned_paths: task.owned_paths.clone(),
            staging_rel_dir,
            expires_at,
            issued_at: now_ms,
        };
        match mailbox.write_task_envelope(&contract) {
            Ok(path) => written_envelope_paths.push(path),
            Err(e) => {
                for path in &written_envelope_paths {
                    let _ = std::fs::remove_file(path);
                }
                let rollback_actor = actor.clone();
                let rollback_ids = issued_lease_ids.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    rollback_actor.execute_immediate(move |tx| -> Result<(), rusqlite::Error> {
                        for lid in &rollback_ids {
                            tx.execute(
                                "UPDATE worker_file_leases SET released = 1 WHERE lease_id = ?1",
                                params![lid],
                            )?;
                        }
                        Ok(())
                    })
                })
                .await;
                return map_lease_error(e, &request_id);
            }
        }
        staged_assignments.push(StagedAssignment {
            task_id,
            lease_id,
            worker_id: task.worker_id,
            worker_role: task.worker_role,
            fencing_token,
            owned_paths: task.owned_paths,
            expires_at,
        });
    }

    // 5. Apply adaptive 429 semaphore mutations ONLY AFTER conflict checks, DB commit & envelope writes succeed.
    let sem = shared_project_rate_semaphore(&project_tenant, &project_id);
    let (clamped_on_429, backoff_ms, concurrency_limit) = if upstream_http_status == Some(429) {
        let transition = sem.on_http_429(retry_after_ms, Utc::now().timestamp_millis() as u64);
        (true, transition.backoff_ms, transition.current_limit)
    } else if matches!(upstream_http_status, None | Some(200..=299)) {
        let limit = sem.on_success();
        let snap = sem.snapshot();
        (snap.clamped, snap.last_backoff_ms, limit)
    } else {
        let snap = sem.snapshot();
        (snap.clamped, snap.last_backoff_ms, snap.current_limit)
    };

    let assignments: Vec<DispatchedTaskAssignment> = staged_assignments
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let dispatch_state = if idx < concurrency_limit {
                "DISPATCHED"
            } else {
                "QUEUED_BY_CLAMP"
            };
            DispatchedTaskAssignment {
                envelope_rel_path: format!("{COORDINATION_REL_DIR}/tasks/{}.json", item.task_id),
                task_id: item.task_id,
                lease_id: item.lease_id,
                worker_id: item.worker_id,
                worker_role: item.worker_role,
                fencing_token: item.fencing_token,
                owned_paths: item.owned_paths,
                dispatch_state: dispatch_state.to_string(),
                expires_at: item.expires_at,
            }
        })
        .collect();

    json_ok(
        DispatchFleetSwarmData {
            dispatch_id: Uuid::new_v4().to_string(),
            project_id,
            session_id,
            concurrency_limit,
            clamped_on_429,
            backoff_ms,
            reaped_expired_count,
            assignments,
        },
        request_id,
    )
}

#[derive(Debug)]
enum JuryPersistError {
    Domain(DesignJuryError),
}

/// `POST /api/v1/fleet/jury/evaluate`
pub async fn handle_evaluate_design_jury(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, is_superadmin) =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(v) => v,
            Err(resp) => return resp,
        };
    if let Err(resp) = require_editor_role(&principal) {
        return resp;
    }

    let req: EvaluateDesignJuryRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let builder_worker_id = match validate_safe_worker_id(&req.builder_worker_id, "builderWorkerId")
    {
        Ok(id) => id,
        Err(e) => {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                e.to_string(),
                Some("builderWorkerId"),
                None,
                request_id,
            );
        }
    };
    let scorer_worker_id = match validate_safe_worker_id(&req.scorer_worker_id, "scorerWorkerId") {
        Ok(id) => id,
        Err(e) => {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                e.to_string(),
                Some("scorerWorkerId"),
                None,
                request_id,
            );
        }
    };

    // Unconditional Builder != Scorer guard before any DB access (AC-02, AC-03).
    if let Err(err) =
        enforce_builder_neq_scorer(&builder_worker_id, &scorer_worker_id, req.scorer_role)
    {
        return map_jury_error(err, &request_id);
    }

    if req.screen_id.trim().is_empty() || req.screen_id.len() > 128 {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "screenId must be 1..=128 characters",
            Some("screenId"),
            None,
            request_id,
        );
    }
    if req.generation_fence < 1 {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "generationFence must be >= 1",
            Some("generationFence"),
            None,
            request_id,
        );
    }

    let (project_tenant, _workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &req.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let actor = match shared_write_actor_for_pool(state.registry.pool()) {
        Ok(a) => a,
        Err(e) => return map_actor_err(e, &request_id),
    };

    let project_id = req.project_id.trim().to_string();
    let write_res = tokio::task::spawn_blocking(move || {
        actor.execute_immediate(
            move |tx| -> Result<Result<EvaluateDesignJuryData, JuryPersistError>, rusqlite::Error> {
                let now_ms = Utc::now().timestamp_millis();

                // 0. Server-side DB anti-spoofing check: any worker with a BUILDER lease in this
                //    project is unconditionally forbidden from scoring or self-PASSing.
                let scorer_held_builder_lease: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM worker_file_leases WHERE project_id = ?1 AND LOWER(TRIM(worker_id)) = LOWER(TRIM(?2)) AND worker_role = 'BUILDER')",
                    params![project_id, scorer_worker_id],
                    |r| r.get::<_, i64>(0).map(|v| v != 0),
                )?;
                if scorer_held_builder_lease {
                    return Ok(Err(JuryPersistError::Domain(
                        DesignJuryError::BuilderSelfPassForbidden {
                            builder_worker_id: builder_worker_id.clone(),
                            scorer_worker_id: scorer_worker_id.clone(),
                        },
                    )));
                }

                // 1. Check latest persisted generation_fence CAS for this project.
                let latest_fence: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(generation_fence), 0) FROM fleet_jury_verdicts WHERE project_id = ?1",
                    params![project_id],
                    |r| r.get(0),
                )?;

                let expected_fence = match req.expected_generation_fence {
                    Some(explicit) => explicit,
                    None if req.generation_fence < latest_fence => latest_fence,
                    None => req.generation_fence,
                };
                if req.generation_fence != expected_fence || req.generation_fence < latest_fence {
                    return Ok(Err(JuryPersistError::Domain(
                        DesignJuryError::StaleGenerationFence {
                            expected: expected_fence.max(latest_fence),
                            provided: req.generation_fence,
                        },
                    )));
                }

                // 2. Collect prior violation fingerprint counts from prior verdicts & gate_failure_ledger.
                let mut prior_counts: BTreeMap<String, usize> = BTreeMap::new();
                {
                    let mut stmt = tx.prepare(
                        "SELECT violations_json FROM fleet_jury_verdicts WHERE project_id = ?1",
                    )?;
                    let rows = stmt.query_map(params![project_id], |r| r.get::<_, String>(0))?;
                    for row in rows {
                        let json_str = row?;
                        if let Ok(items) = serde_json::from_str::<Vec<QcViolationItem>>(&json_str) {
                            for item in items {
                                *prior_counts.entry(item.fingerprint_sha256).or_insert(0) += 1;
                            }
                        }
                    }
                }
                {
                    let mut stmt = tx.prepare(
                        "SELECT fingerprint_sha256, hit_count FROM gate_failure_ledger
                         WHERE tenant_id = ?1 AND project_id = ?2",
                    )?;
                    let rows = stmt.query_map(params![project_tenant, project_id], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                    })?;
                    for row in rows {
                        let (fp, hits) = row?;
                        let entry = prior_counts.entry(fp).or_insert(0);
                        *entry = (*entry).max(hits.max(0) as usize);
                    }
                }

                // 3. Execute independent E1..E4 + Tier-3 Single-Session Design Jury.
                let session_input = DesignJurySessionInput {
                    builder_worker_id,
                    scorer_worker_id,
                    scorer_role: req.scorer_role,
                    freeze_bundle: ReviewFreezeBundle {
                        project_id: project_id.clone(),
                        screen_id: req.screen_id.clone(),
                        generation_fence: req.generation_fence,
                        freeze_snapshot_sha256: req.freeze_snapshot_sha256.clone(),
                        frozen_html: req.frozen_html.clone(),
                        frozen_at: now_ms,
                    },
                    qc_input: QcRunInput {
                        qc_run_id: req.qc_run_id.clone(),
                        screen_id: req.screen_id.clone(),
                        html_snapshot: req.frozen_html.clone(),
                        reported_issues: req.reported_issues,
                        expected_defect_ids: req.expected_defect_ids,
                        contrast_ratios: req.contrast_ratios,
                        bounding_boxes: req.bounding_boxes,
                        brand_tokens_expected: req.brand_tokens_expected,
                    },
                };

                let outcome = match evaluate_design_jury_session(
                    &session_input,
                    expected_fence,
                    &prior_counts,
                ) {
                    Ok(o) => o,
                    Err(e) => return Ok(Err(JuryPersistError::Domain(e))),
                };

                // 4. Escalate recurring violations (>= 2 hits) atomically into V18a/V20 gate_failure_ledger (HUMAN_ONLY).
                let mut primary_ledger_id: Option<String> = None;
                for esc in &outcome.ledger_escalations {
                    let candidate_id = Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO gate_failure_ledger (
                            id, tenant_id, project_id, rule_id, fingerprint_sha256,
                            sample_message, hit_count, slpr_score, promoted_card_id, status, last_seen_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0.0, NULL, 'PROPOSED', ?8)
                         ON CONFLICT(tenant_id, project_id, fingerprint_sha256) DO UPDATE SET
                            hit_count = MAX(gate_failure_ledger.hit_count, excluded.hit_count),
                            sample_message = excluded.sample_message,
                            status = CASE WHEN gate_failure_ledger.status = 'ACCUMULATING' THEN 'PROPOSED' ELSE gate_failure_ledger.status END,
                            last_seen_at = excluded.last_seen_at",
                        params![
                            candidate_id,
                            project_tenant,
                            project_id,
                            esc.rule_id,
                            esc.fingerprint_sha256,
                            esc.sample_message,
                            esc.hit_count.max(1),
                            now_ms,
                        ],
                    )?;

                    let resolved_id: String = tx.query_row(
                        "SELECT id FROM gate_failure_ledger
                         WHERE tenant_id = ?1 AND project_id = ?2 AND fingerprint_sha256 = ?3",
                        params![project_tenant, project_id, esc.fingerprint_sha256],
                        |r| r.get(0),
                    )?;
                    if primary_ledger_id.is_none() {
                        primary_ledger_id = Some(resolved_id);
                    }
                }

                // 5. Persist V27 fleet_jury_verdicts row (DB CHECK enforces builder_worker_id <> scorer_worker_id).
                let verdict_id = Uuid::new_v4().to_string();
                let violations_json = serde_json::to_string(&outcome.report.violations)
                    .unwrap_or_else(|_| "[]".to_string());

                tx.execute(
                    "INSERT INTO fleet_jury_verdicts (
                        id, project_id, builder_worker_id, scorer_worker_id,
                        generation_fence, freeze_snapshot_sha256, qc_run_id,
                        e1_e4_catch_rate, verdict, violations_json, ledger_entry_id, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        verdict_id,
                        project_id,
                        outcome.report.builder_worker_id,
                        outcome.report.scorer_worker_id,
                        outcome.report.generation_fence,
                        outcome.report.freeze_snapshot_sha256,
                        outcome.report.qc_run_id,
                        outcome.report.e1_e4_catch_rate,
                        outcome.report.verdict.as_str(),
                        violations_json,
                        primary_ledger_id,
                        now_ms,
                    ],
                )?;

                let approval_policy = if outcome.report.escalated_to_ledger {
                    Some(HUMAN_ONLY_APPROVAL_POLICY.to_string())
                } else {
                    None
                };

                Ok(Ok(EvaluateDesignJuryData {
                    verdict_id,
                    project_id,
                    screen_id: outcome.report.screen_id,
                    builder_worker_id: outcome.report.builder_worker_id,
                    scorer_worker_id: outcome.report.scorer_worker_id,
                    generation_fence: outcome.report.generation_fence,
                    freeze_snapshot_sha256: outcome.report.freeze_snapshot_sha256,
                    qc_run_id: outcome.report.qc_run_id,
                    e1_e4_catch_rate: outcome.report.e1_e4_catch_rate,
                    verdict: outcome.report.verdict,
                    lens_scores: outcome.report.lens_scores,
                    violations: outcome.report.violations,
                    recommended_fixes: outcome.report.recommended_fixes,
                    escalated_to_ledger: outcome.report.escalated_to_ledger,
                    ledger_entry_id: primary_ledger_id,
                    approval_policy,
                }))
            },
        )
    })
    .await;

    match write_res {
        Ok(Ok(Ok(data))) => json_ok(data, request_id),
        Ok(Ok(Err(JuryPersistError::Domain(err)))) => map_jury_error(err, &request_id),
        Ok(Err(actor_err)) => map_actor_err(actor_err, &request_id),
        Err(join_err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Design Jury evaluation task failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

fn map_jury_error(err: DesignJuryError, request_id: &str) -> Response {
    match err {
        DesignJuryError::BuilderSelfPassForbidden {
            builder_worker_id,
            scorer_worker_id,
        } => json_err(
            StatusCode::FORBIDDEN,
            "BUILDER_SELF_PASS_FORBIDDEN",
            format!(
                "Builder '{builder_worker_id}' is strictly forbidden from self-scoring or issuing PASS via '{scorer_worker_id}'"
            ),
            Some("scorerWorkerId"),
            Some(serde_json::json!({
                "builderWorkerId": builder_worker_id,
                "scorerWorkerId": scorer_worker_id,
            })),
            request_id.to_string(),
        ),
        DesignJuryError::StaleGenerationFence { expected, provided } => json_err(
            StatusCode::CONFLICT,
            "STALE_GENERATION_FENCE",
            format!("Stale generation_fence: expected {expected}, provided {provided}"),
            Some("generationFence"),
            Some(serde_json::json!({
                "expected": expected,
                "provided": provided,
            })),
            request_id.to_string(),
        ),
        DesignJuryError::InvalidFreezeHash(msg) => json_err(
            StatusCode::BAD_REQUEST,
            "INVALID_FREEZE_SNAPSHOT_SHA256",
            msg,
            Some("freezeSnapshotSha256"),
            None,
            request_id.to_string(),
        ),
        DesignJuryError::InvalidJuryInput(msg) => json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            msg,
            None,
            None,
            request_id.to_string(),
        ),
    }
}

/// Mountable sub-router exposing Story 10.2 Parallel Swarm Dispatch, Disjoint Leases & Tier-3 Design Jury routes.
pub fn fleet_swarm_router() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/fleet/leases/acquire",
            post(handle_acquire_worker_lease),
        )
        .route("/api/v1/fleet/dispatch", post(handle_dispatch_fleet_swarm))
        .route(
            "/api/v1/fleet/jury/evaluate",
            post(handle_evaluate_design_jury),
        )
}

/// Alias matching sibling `build_*_router()` naming convention in `legacy_router.rs`.
pub fn build_fleet_swarm_router() -> Router<DaemonAppState> {
    fleet_swarm_router()
}
