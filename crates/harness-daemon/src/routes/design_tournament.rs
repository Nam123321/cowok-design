// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Daemon /api/v1/tournament/start, /api/v1/tournament/select, /api/v1/tournament/runs, /api/v1/crystallize & /api/v1/advisor/evolve)
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use axum::{
    body::Bytes,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bot_fleet::design_tournament::{
    assert_advisor_target_allowed, assert_staging_human_only_policy, is_safe_tournament_id,
    merge_cross_variant_borrow, resolve_safe_artifact_path, run_advisor_gepa_loop,
    run_design_tournament_in_workspace, sha256_hex, AdvisorEvolveInput, AdvisorEvolveOutput,
    BlindRubricSummary, CreativeMemorySignal, DesignAxis, GeneratedVariant,
    StudioCorrectionCluster, TournamentError, TournamentStartConfig, VariantBranchOverride,
    BLIND_OPTION_LABELS, MAX_TOURNAMENT_BUDGET_MS, STUDIO_CORRECTION_CLUSTER_THRESHOLD,
};
use chrono::Utc;
use html_forge::reconcile::{reconcile_ff_ids, DEFAULT_RECONCILE_MIN_CONFIDENCE};
use html_forge::{sanitize_html_with_profile, HtmlForgeError, SanitizeProfile};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::SqliteActorError;
use uuid::Uuid;

use super::orchestrator::shared_write_actor_for_pool;
use super::DaemonAppState;
use crate::daemon_auth::{
    is_valid_superadmin_bearer, require_editor_role, FlowPrincipal, LegacyAuthKind,
    VerifiedLegacyIngress,
};

const MAX_TOURNAMENT_BODY_BYTES: usize = 1_048_576;
const V31_TOURNAMENT_DDL: &str =
    include_str!("../../../spec-registry/migrations/V31__story_10_3_design_tournament.sql");

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

fn ensure_v31_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(V31_TOURNAMENT_DDL)
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
                "Arbitrary bearer tokens are not permitted on design tournament endpoints",
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
    if body.len() > MAX_TOURNAMENT_BODY_BYTES {
        return Err(json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            format!("Request body exceeds maximum {MAX_TOURNAMENT_BODY_BYTES} bytes"),
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

fn map_tournament_err(err: TournamentError, request_id: &str) -> Response {
    match err {
        TournamentError::SealedEvalTamper {
            requested_path,
            reason,
        } => json_err(
            StatusCode::FORBIDDEN,
            "SEALED_EVAL_TAMPER_ERROR",
            format!("Blocked advisor access to '{requested_path}': {reason}"),
            Some("targetSkillPath"),
            Some(serde_json::json!({ "requestedPath": requested_path })),
            request_id.to_string(),
        ),
        TournamentError::HumanApprovalRequired { target, reason } => json_err(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            format!("Target '{target}' requires Staging + HUMAN_ONLY approval: {reason}"),
            Some("promoteDirectly"),
            Some(serde_json::json!({ "target": target, "approvalPolicy": "HUMAN_ONLY" })),
            request_id.to_string(),
        ),
        TournamentError::StaleGenerationEpoch { expected, provided } => json_err(
            StatusCode::CONFLICT,
            "STALE_GENERATION_EPOCH",
            format!("Stale generationEpoch: current is {expected}, received {provided}"),
            Some("generationEpoch"),
            Some(serde_json::json!({ "expected": expected, "provided": provided })),
            request_id.to_string(),
        ),
        TournamentError::BorrowZoneNotFound(msg) => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "BORROW_ZONE_NOT_FOUND",
            msg,
            Some("borrowSelectorOrInstruction"),
            None,
            request_id.to_string(),
        ),
        TournamentError::InsufficientVariants {
            generated,
            required,
        } => json_err(
            StatusCode::GATEWAY_TIMEOUT,
            "INSUFFICIENT_TOURNAMENT_VARIANTS",
            format!(
                "Only {generated} variants completed within budget; minimum required is {required}"
            ),
            None,
            Some(serde_json::json!({ "generated": generated, "required": required })),
            request_id.to_string(),
        ),
        TournamentError::LeaseConflict(msg) => json_err(
            StatusCode::CONFLICT,
            "TOURNAMENT_LEASE_CONFLICT",
            msg,
            None,
            None,
            request_id.to_string(),
        ),
        TournamentError::InvalidInput(msg) => json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            msg,
            None,
            None,
            request_id.to_string(),
        ),
    }
}

fn map_html_forge_err(err: HtmlForgeError, field: Option<&str>, request_id: &str) -> Response {
    match err {
        HtmlForgeError::MxssBlocked(detail) => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSS_SECURITY_BLOCKED",
            format!("Unsafe HTML construct blocked: {detail}"),
            field,
            Some(serde_json::json!({ "reason": detail })),
            request_id.to_string(),
        ),
        HtmlForgeError::CssTokenBlocked(detail) => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSS_SECURITY_BLOCKED",
            format!("Unsafe CSS token blocked: {detail}"),
            field,
            Some(serde_json::json!({ "reason": detail })),
            request_id.to_string(),
        ),
        HtmlForgeError::PayloadTooLarge { actual, max } => json_err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            format!("HTML payload {actual} bytes exceeds maximum {max} bytes"),
            field,
            Some(serde_json::json!({ "actualBytes": actual, "maxBytes": max })),
            request_id.to_string(),
        ),
        other => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "HTML_VALIDATION_ERROR",
            format!("HTML validation failed: {other}"),
            field,
            None,
            request_id.to_string(),
        ),
    }
}

#[derive(Debug)]
enum ArtifactIoError {
    SecurityBlocked(String),
    NotFound(String),
    Storage(String),
}

fn map_artifact_io_err(err: ArtifactIoError, request_id: &str) -> Response {
    match err {
        ArtifactIoError::SecurityBlocked(msg) => json_err(
            StatusCode::FORBIDDEN,
            "SEALED_EVAL_TAMPER_ERROR",
            msg,
            None,
            None,
            request_id.to_string(),
        ),
        ArtifactIoError::NotFound(msg) => json_err(
            StatusCode::NOT_FOUND,
            "VARIANT_ARTIFACT_NOT_FOUND",
            msg,
            None,
            None,
            request_id.to_string(),
        ),
        ArtifactIoError::Storage(msg) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            None,
            request_id.to_string(),
        ),
    }
}

fn is_symlink_io_error(err: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        err.raw_os_error() == Some(libc::ELOOP)
    }
    #[cfg(not(unix))]
    {
        let _ = err;
        false
    }
}

fn resolve_artifact_path_strict(
    workspace_root: &Path,
    segments: &[&str],
) -> Result<PathBuf, ArtifactIoError> {
    resolve_safe_artifact_path(workspace_root, segments).map_err(|e| {
        let msg = e.to_string();
        if msg.to_ascii_lowercase().contains("symlink") {
            ArtifactIoError::SecurityBlocked(msg)
        } else {
            ArtifactIoError::Storage(msg)
        }
    })
}

fn read_file_nofollow(path: &Path) -> Result<String, ArtifactIoError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(ArtifactIoError::SecurityBlocked(format!(
                    "Refused symlink artifact '{}'",
                    path.display()
                )));
            }
            if !meta.is_file() {
                return Err(ArtifactIoError::NotFound(format!(
                    "Artifact '{}' is not a regular file",
                    path.display()
                )));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ArtifactIoError::NotFound(format!(
                "Artifact '{}' not found",
                path.display()
            )));
        }
        Err(e) => {
            return Err(ArtifactIoError::Storage(format!(
                "Failed to inspect artifact '{}': {e}",
                path.display()
            )));
        }
    }

    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = opts.open(path).map_err(|e| {
        if is_symlink_io_error(&e) {
            ArtifactIoError::SecurityBlocked(format!(
                "Refused symlink read (O_NOFOLLOW) at '{}'",
                path.display()
            ))
        } else if e.kind() == std::io::ErrorKind::NotFound {
            ArtifactIoError::NotFound(format!("Artifact '{}' not found", path.display()))
        } else {
            ArtifactIoError::Storage(format!("Failed to open artifact '{}': {e}", path.display()))
        }
    })?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).map_err(|e| {
        ArtifactIoError::Storage(format!("Failed to read artifact '{}': {e}", path.display()))
    })?;
    Ok(buf)
}

fn atomic_write_nofollow(
    workspace_root: &Path,
    dir_segments: &[&str],
    file_name: &str,
    bytes: &[u8],
) -> Result<PathBuf, ArtifactIoError> {
    let dir = resolve_artifact_path_strict(workspace_root, dir_segments)?;
    fs::create_dir_all(&dir).map_err(|e| {
        ArtifactIoError::Storage(format!(
            "Failed to create directory '{}': {e}",
            dir.display()
        ))
    })?;
    // Re-validate every directory component after create_dir_all to catch symlink races.
    let dir = resolve_artifact_path_strict(workspace_root, dir_segments)?;
    if let Ok(meta) = fs::symlink_metadata(&dir) {
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(ArtifactIoError::SecurityBlocked(format!(
                "Refused non-directory or symlink parent '{}'",
                dir.display()
            )));
        }
    }

    let mut target_segments: Vec<&str> = Vec::with_capacity(dir_segments.len() + 1);
    target_segments.extend_from_slice(dir_segments);
    target_segments.push(file_name);
    let target_path = resolve_artifact_path_strict(workspace_root, &target_segments)?;

    let tmp_name = format!(".tmp-{}.part", Uuid::new_v4().simple());
    let tmp_path = dir.join(&tmp_name);
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut tmp_file = opts.open(&tmp_path).map_err(|e| {
        if is_symlink_io_error(&e) {
            ArtifactIoError::SecurityBlocked(format!(
                "Refused symlink temp write (O_NOFOLLOW) at '{}'",
                tmp_path.display()
            ))
        } else {
            ArtifactIoError::Storage(format!(
                "Failed to create temp file '{}': {e}",
                tmp_path.display()
            ))
        }
    })?;

    if let Err(e) = tmp_file.write_all(bytes).and_then(|_| tmp_file.sync_all()) {
        let _ = fs::remove_file(&tmp_path);
        return Err(ArtifactIoError::Storage(format!(
            "Failed writing temp artifact '{}': {e}",
            tmp_path.display()
        )));
    }
    drop(tmp_file);

    if let Ok(meta) = fs::symlink_metadata(&target_path) {
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(&tmp_path);
            return Err(ArtifactIoError::SecurityBlocked(format!(
                "Refused overwriting symlink target '{}'",
                target_path.display()
            )));
        }
    }

    if let Err(e) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(ArtifactIoError::Storage(format!(
            "Failed atomically committing '{}': {e}",
            target_path.display()
        )));
    }
    Ok(target_path)
}

fn persist_variant_html(
    workspace_root: &Path,
    run_id: &str,
    variant_id: &str,
    html: &str,
) -> Result<PathBuf, ArtifactIoError> {
    let file_name = format!("{variant_id}.html");
    atomic_write_nofollow(
        workspace_root,
        &[".cowok-design", "tournaments", run_id],
        &file_name,
        html.as_bytes(),
    )
}

fn stage_epoch_merged_html(
    workspace_root: &Path,
    run_id: &str,
    variant_id: &str,
    epoch: i64,
    html: &str,
) -> Result<(PathBuf, PathBuf), ArtifactIoError> {
    let staged_name = format!(".staged-{}.merged.html", Uuid::new_v4());
    let final_name = format!("{variant_id}.epoch-{epoch}.merged.html");
    let staged_path = atomic_write_nofollow(
        workspace_root,
        &[".cowok-design", "tournaments", run_id],
        &staged_name,
        html.as_bytes(),
    )?;
    let final_path = resolve_artifact_path_strict(
        workspace_root,
        &[".cowok-design", "tournaments", run_id, &final_name],
    )?;
    Ok((staged_path, final_path))
}

fn promote_staged_epoch_merged_html(staged_path: &Path, final_path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(final_path) {
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(staged_path);
            return;
        }
    }
    if fs::rename(staged_path, final_path).is_err() {
        let _ = fs::remove_file(staged_path);
    }
}

fn read_variant_html(
    workspace_root: &Path,
    run_id: &str,
    variant_id: &str,
) -> Result<String, ArtifactIoError> {
    let file_name = format!("{variant_id}.html");
    let path = resolve_artifact_path_strict(
        workspace_root,
        &[".cowok-design", "tournaments", run_id, &file_name],
    )?;
    read_file_nofollow(&path)
}

fn read_epoch_merged_html(
    workspace_root: &Path,
    run_id: &str,
    variant_id: &str,
    epoch: i64,
) -> Result<String, ArtifactIoError> {
    let file_name = format!("{variant_id}.epoch-{epoch}.merged.html");
    let path = resolve_artifact_path_strict(
        workspace_root,
        &[".cowok-design", "tournaments", run_id, &file_name],
    )?;
    read_file_nofollow(&path)
}

fn archive_losing_variant_file(workspace_root: &Path, run_id: &str, variant_id: &str) {
    let file_name = format!("{variant_id}.html");
    let Ok(src) = resolve_artifact_path_strict(
        workspace_root,
        &[".cowok-design", "tournaments", run_id, &file_name],
    ) else {
        return;
    };
    let Ok(html) = read_file_nofollow(&src) else {
        return;
    };
    let _ = atomic_write_nofollow(
        workspace_root,
        &["variants", "_archive", run_id],
        &file_name,
        html.as_bytes(),
    );
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartTournamentRequest {
    pub project_id: String,
    pub prompt: String,
    #[serde(default)]
    pub axes: Vec<DesignAxis>,
    #[serde(default = "default_budget_ms")]
    pub budget_ms: u64,
    #[serde(default)]
    pub branch_overrides: Vec<VariantBranchOverride>,
    #[serde(default)]
    pub rubric_criteria: Vec<String>,
}

fn default_budget_ms() -> u64 {
    MAX_TOURNAMENT_BUDGET_MS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTournamentData {
    pub run_id: String,
    pub project_id: String,
    pub prompt: String,
    pub axes: Vec<DesignAxis>,
    pub generation_epoch: i64,
    pub budget_ms: u64,
    pub elapsed_ms: u64,
    pub degraded_min_variants: bool,
    pub timed_out_axes: Vec<DesignAxis>,
    pub low_diversity_warning: bool,
    pub variants: Vec<GeneratedVariant>,
    pub rubric_summary: BlindRubricSummary,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectTournamentVariantRequest {
    pub project_id: String,
    pub run_id: String,
    pub winning_variant_id: String,
    pub generation_epoch: i64,
    #[serde(default)]
    pub borrow_from_variant_id: Option<String>,
    #[serde(default)]
    pub borrow_selector_or_instruction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectTournamentVariantData {
    pub run_id: String,
    pub project_id: String,
    pub winning_variant_id: String,
    pub generation_epoch: i64,
    pub merged_html: String,
    pub merged_sha256: String,
    pub borrowed_from_variant_id: Option<String>,
    pub borrowed_ff_id: Option<String>,
    pub extracted_css_rules_count: usize,
    pub reconciled_total_ff_ids: usize,
    pub duplicate_ff_ids_after_merge: usize,
    pub archived_variant_ids: Vec<String>,
    pub memory_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTournamentRunsQuery {
    pub project_id: String,
    #[serde(default = "default_runs_limit")]
    pub limit: usize,
}

fn default_runs_limit() -> usize {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedVariantRow {
    pub id: String,
    pub display_label: String,
    pub axis_profile: String,
    pub artifact_sha256: String,
    pub ast_similarity: f64,
    pub clone_distance_passed: bool,
    pub rubric_verdict: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedTournamentRun {
    pub id: String,
    pub project_id: String,
    pub axes: Vec<String>,
    pub winning_variant_id: Option<String>,
    pub borrowed_from_variant_id: Option<String>,
    pub borrowed_ff_id: Option<String>,
    pub generation_epoch: i64,
    pub created_at: i64,
    pub variants: Vec<PersistedVariantRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTournamentRunsData {
    pub project_id: String,
    pub runs: Vec<PersistedTournamentRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrystallizePackageKind {
    #[serde(rename = "STARTER_TEMPLATE")]
    StarterTemplate,
    #[serde(rename = "BRAND_KIT")]
    BrandKit,
    #[serde(rename = "SKILL_PLUGIN")]
    SkillPlugin,
}

impl CrystallizePackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StarterTemplate => "STARTER_TEMPLATE",
            Self::BrandKit => "BRAND_KIT",
            Self::SkillPlugin => "SKILL_PLUGIN",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrystallizeRequest {
    pub project_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub variant_id: Option<String>,
    pub package_kind: CrystallizePackageKind,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub html_snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystallizeData {
    pub package_id: String,
    pub project_id: String,
    pub package_kind: CrystallizePackageKind,
    pub name: String,
    pub artifact_rel_path: String,
    pub artifact_sha256: String,
    pub memory_event_id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvisorEvolveRequest {
    pub project_id: String,
    pub target_skill_path: String,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub promote_directly: bool,
    #[serde(default = "default_target_channel")]
    pub target_channel: String,
    #[serde(default = "default_min_hits")]
    pub min_studio_correction_hits: i64,
}

fn default_target_channel() -> String {
    "DESIGN_CRAFT_STAGING".to_string()
}

fn default_min_hits() -> i64 {
    STUDIO_CORRECTION_CLUSTER_THRESHOLD
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorEvolveResponseData {
    #[serde(flatten)]
    pub result: AdvisorEvolveOutput,
    pub memory_event_id: String,
    pub created_at: i64,
}

enum StartTournamentPrepError {
    Tournament(TournamentError),
    HtmlForge {
        err: HtmlForgeError,
        field: &'static str,
    },
    ArtifactIo(ArtifactIoError),
}

enum SelectPrepError {
    Tournament(TournamentError),
    HtmlForge(HtmlForgeError),
    ArtifactIo(ArtifactIoError),
    Storage(String),
}

struct PreparedSelectWork {
    merged_html: String,
    merged_sha256: String,
    borrowed_ff_id: Option<String>,
    extracted_css_rules_count: usize,
    reconciled_total_ff_ids: usize,
    duplicate_ff_ids_after_merge: usize,
    staged_epoch_path: PathBuf,
    final_epoch_path: PathBuf,
}

/// `POST /api/v1/tournament/start`
pub async fn handle_start_tournament(
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

    let req: StartTournamentRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let (_tenant, workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &req.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let ws_for_run = workspace_root.clone();
    let prep_res = tokio::task::spawn_blocking(move || -> Result<bot_fleet::design_tournament::TournamentRunOutput, StartTournamentPrepError> {
        let mut sanitized_overrides = Vec::with_capacity(req.branch_overrides.len());
        for ov in req.branch_overrides {
            let initial_html = match ov.initial_html {
                Some(raw) if !raw.trim().is_empty() => {
                    let (clean, _) = sanitize_html_with_profile(&raw, SanitizeProfile::AppPreview)
                        .map_err(|err| StartTournamentPrepError::HtmlForge {
                            err,
                            field: "branchOverrides",
                        })?;
                    Some(clean)
                }
                other => other,
            };
            let mut reroll_htmls = Vec::with_capacity(ov.reroll_htmls.len());
            for raw in ov.reroll_htmls {
                if raw.trim().is_empty() {
                    reroll_htmls.push(raw);
                } else {
                    let (clean, _) = sanitize_html_with_profile(&raw, SanitizeProfile::AppPreview)
                        .map_err(|err| StartTournamentPrepError::HtmlForge {
                            err,
                            field: "branchOverrides",
                        })?;
                    reroll_htmls.push(clean);
                }
            }
            sanitized_overrides.push(VariantBranchOverride {
                axis: ov.axis,
                initial_html,
                reroll_htmls,
                simulated_latency_ms: ov.simulated_latency_ms,
            });
        }

        let mut run_out = run_design_tournament_in_workspace(
            Some(&ws_for_run),
            &TournamentStartConfig {
                project_id: req.project_id.trim().to_string(),
                prompt: req.prompt,
                axes: req.axes,
                budget_ms: req.budget_ms,
                branch_overrides: sanitized_overrides,
                rubric_criteria: req.rubric_criteria,
            },
        )
        .map_err(StartTournamentPrepError::Tournament)?;

        for v in &mut run_out.variants {
            let (clean_html, clean_sha) =
                sanitize_html_with_profile(&v.html, SanitizeProfile::AppPreview).map_err(|err| {
                    StartTournamentPrepError::HtmlForge {
                        err,
                        field: "variants",
                    }
                })?;
            v.html = clean_html;
            v.artifact_sha256 = clean_sha;
            persist_variant_html(&ws_for_run, &run_out.run_id, &v.id, &v.html)
                .map_err(StartTournamentPrepError::ArtifactIo)?;
        }

        Ok(run_out)
    })
    .await;

    let run_out = match prep_res {
        Ok(Ok(v)) => v,
        Ok(Err(StartTournamentPrepError::Tournament(e))) => {
            return map_tournament_err(e, &request_id)
        }
        Ok(Err(StartTournamentPrepError::HtmlForge { err, field })) => {
            return map_html_forge_err(err, Some(field), &request_id)
        }
        Ok(Err(StartTournamentPrepError::ArtifactIo(e))) => {
            return map_artifact_io_err(e, &request_id)
        }
        Err(join_err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Tournament generation task failed: {join_err}"),
                None,
                None,
                request_id,
            )
        }
    };

    let actor = match shared_write_actor_for_pool(state.registry.pool()) {
        Ok(a) => a,
        Err(e) => return map_actor_err(e, &request_id),
    };

    let run_to_store = run_out.clone();
    let write_res = tokio::task::spawn_blocking(move || {
        actor.execute_immediate(move |tx| -> Result<(), rusqlite::Error> {
            ensure_v31_tables(tx)?;
            let axes_strs: Vec<&str> = run_to_store.axes.iter().map(|a| a.as_str()).collect();
            let axes_json = serde_json::to_string(&axes_strs).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "INSERT INTO tournament_runs (
                    id, project_id, axes_json, winning_variant_id,
                    borrowed_from_variant_id, borrowed_ff_id, generation_epoch, created_at
                ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, ?4, ?5)",
                params![
                    run_to_store.run_id,
                    run_to_store.project_id,
                    axes_json,
                    run_to_store.generation_epoch,
                    run_to_store.created_at,
                ],
            )?;

            for v in &run_to_store.variants {
                tx.execute(
                    "INSERT INTO tournament_variants (
                        id, run_id, axis_profile, artifact_sha256, ast_similarity,
                        clone_distance_passed, rubric_verdict, archived
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                    params![
                        v.id,
                        run_to_store.run_id,
                        v.axis_profile,
                        v.artifact_sha256,
                        v.ast_similarity,
                        if v.clone_distance_passed {
                            1_i64
                        } else {
                            0_i64
                        },
                        v.rubric_verdict,
                    ],
                )?;
            }
            Ok(())
        })
    })
    .await;

    match write_res {
        Ok(Ok(())) => json_ok(
            StartTournamentData {
                run_id: run_out.run_id,
                project_id: run_out.project_id,
                prompt: run_out.prompt,
                axes: run_out.axes,
                generation_epoch: run_out.generation_epoch,
                budget_ms: run_out.budget_ms,
                elapsed_ms: run_out.elapsed_ms,
                degraded_min_variants: run_out.degraded_min_variants,
                timed_out_axes: run_out.timed_out_axes,
                low_diversity_warning: run_out.low_diversity_warning,
                variants: run_out.variants,
                rubric_summary: run_out.rubric_summary,
                created_at: run_out.created_at,
            },
            request_id,
        ),
        Ok(Err(e)) => map_actor_err(e, &request_id),
        Err(join_err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Tournament start persistence failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

/// `POST /api/v1/tournament/select`
pub async fn handle_select_tournament_variant(
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

    let req: SelectTournamentVariantRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if req.generation_epoch < 1 {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "generationEpoch must be >= 1",
            Some("generationEpoch"),
            None,
            request_id,
        );
    }
    if !is_safe_tournament_id(&req.run_id) || !is_safe_tournament_id(&req.winning_variant_id) {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "runId and winningVariantId must be valid identifiers",
            None,
            None,
            request_id,
        );
    }
    if let Some(ref borrow_id) = req.borrow_from_variant_id {
        if !is_safe_tournament_id(borrow_id) {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "borrowFromVariantId must be a valid identifier",
                Some("borrowFromVariantId"),
                None,
                request_id,
            );
        }
    }

    let (_tenant, workspace_root) = match resolve_tenant_project_workspace(
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
    let run_id = req.run_id.trim().to_string();
    let winning_variant_id = req.winning_variant_id.trim().to_string();
    let generation_epoch = req.generation_epoch;
    let borrow_from_variant_id = req
        .borrow_from_variant_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != winning_variant_id)
        .map(str::to_string);
    let borrow_instruction = req
        .borrow_selector_or_instruction
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // Phase 1 (OUTSIDE actor.execute_immediate, offloaded to spawn_blocking):
    // Read immutable base variant HTML, perform cross-variant borrow merge, reconcile data-ff-ids,
    // sanitize final merged HTML via html_forge::sanitize_html_with_profile(AppPreview), and
    // atomically persist epoch-scoped merged artifact without overwriting immutable base <variant_id>.html.
    let pool_for_prep = state.registry.pool().clone();
    let ws_for_prep = workspace_root.clone();
    let project_id_prep = project_id.clone();
    let run_id_prep = run_id.clone();
    let winner_id_prep = winning_variant_id.clone();
    let borrow_id_prep = borrow_from_variant_id.clone();
    let borrow_instr_prep = borrow_instruction.clone();

    let prep_join = tokio::task::spawn_blocking(move || -> Result<PreparedSelectWork, SelectPrepError> {
        let conn = pool_for_prep
            .connection()
            .map_err(|e| SelectPrepError::Storage(format!("Database connection error: {e}")))?;
        ensure_v31_tables(&conn)
            .map_err(|e| SelectPrepError::Storage(format!("Failed to initialize V31 tables: {e}")))?;

        let run_row: Option<(i64, Option<String>)> = conn
            .query_row(
                "SELECT generation_epoch, winning_variant_id FROM tournament_runs WHERE id = ?1 AND project_id = ?2",
                params![run_id_prep, project_id_prep],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| SelectPrepError::Storage(format!("Failed to load tournament run: {e}")))?;

        let Some((current_epoch, existing_winner)) = run_row else {
            return Err(SelectPrepError::Tournament(TournamentError::InvalidInput(
                format!("Tournament run '{run_id_prep}' not found in project"),
            )));
        };

        if generation_epoch < current_epoch
            || (existing_winner.is_some() && generation_epoch <= current_epoch)
        {
            let expected = if existing_winner.is_some() {
                current_epoch.saturating_add(1)
            } else {
                current_epoch
            };
            return Err(SelectPrepError::Tournament(
                TournamentError::StaleGenerationEpoch {
                    expected,
                    provided: generation_epoch,
                },
            ));
        }

        let mut stmt = conn
            .prepare("SELECT id FROM tournament_variants WHERE run_id = ?1 ORDER BY rowid ASC")
            .map_err(|e| SelectPrepError::Storage(format!("Failed to query variants: {e}")))?;
        let all_variant_ids: Vec<String> = stmt
            .query_map(params![run_id_prep], |r| r.get(0))
            .map_err(|e| SelectPrepError::Storage(format!("Failed to decode variants: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| SelectPrepError::Storage(format!("Failed to collect variants: {e}")))?;
        drop(stmt);
        drop(conn);

        if !all_variant_ids.contains(&winner_id_prep) {
            return Err(SelectPrepError::Tournament(TournamentError::InvalidInput(
                format!("Winning variant '{winner_id_prep}' does not belong to run '{run_id_prep}'"),
            )));
        }
        if let Some(ref donor_id) = borrow_id_prep {
            if !all_variant_ids.contains(donor_id) {
                return Err(SelectPrepError::Tournament(TournamentError::InvalidInput(
                    format!("Donor variant '{donor_id}' does not belong to run '{run_id_prep}'"),
                )));
            }
        }

        let winner_html = read_variant_html(&ws_for_prep, &run_id_prep, &winner_id_prep)
            .map_err(SelectPrepError::ArtifactIo)?;

        let (
            candidate_html,
            borrowed_ff_id,
            extracted_css_rules_count,
            reconciled_total_ff_ids,
            duplicate_ff_ids_after_merge,
        ) = if let Some(ref donor_id) = borrow_id_prep {
            let donor_html = read_variant_html(&ws_for_prep, &run_id_prep, donor_id)
                .map_err(SelectPrepError::ArtifactIo)?;
            let instruction = borrow_instr_prep.as_deref().unwrap_or("section");
            let borrow_res = merge_cross_variant_borrow(&winner_html, &donor_html, instruction)
                .map_err(SelectPrepError::Tournament)?;
            let reconciled = reconcile_ff_ids(
                &winner_html,
                &borrow_res.merged_html,
                DEFAULT_RECONCILE_MIN_CONFIDENCE,
            )
            .map_err(|e| SelectPrepError::Storage(format!("reconcile_ff_ids failed: {e}")))?;
            (
                reconciled.reconciled_html,
                Some(borrow_res.borrowed_ff_id),
                borrow_res.extracted_css_rules.len(),
                reconciled.total_ff_ids,
                borrow_res.duplicate_ff_ids_after_merge,
            )
        } else {
            let reconciled =
                reconcile_ff_ids(&winner_html, &winner_html, DEFAULT_RECONCILE_MIN_CONFIDENCE)
                    .map_err(|e| SelectPrepError::Storage(format!("reconcile_ff_ids failed: {e}")))?;
            (reconciled.reconciled_html, None, 0, reconciled.total_ff_ids, 0)
        };

        // Mandatory sanitization of final merged HTML via html_forge::sanitize_html_with_profile(AppPreview).
        let (sanitized_html, sanitized_sha256) =
            sanitize_html_with_profile(&candidate_html, SanitizeProfile::AppPreview)
                .map_err(SelectPrepError::HtmlForge)?;

        // Stage merged output atomically to a request-unique temp file; only promote to
        // {variant_id}.epoch-{epoch}.merged.html AFTER SQLite commit succeeds (prevents concurrent clobber/delete).
        let (staged_epoch_path, final_epoch_path) = stage_epoch_merged_html(
            &ws_for_prep,
            &run_id_prep,
            &winner_id_prep,
            generation_epoch,
            &sanitized_html,
        )
        .map_err(SelectPrepError::ArtifactIo)?;

        Ok(PreparedSelectWork {
            merged_html: sanitized_html,
            merged_sha256: sanitized_sha256,
            borrowed_ff_id,
            extracted_css_rules_count,
            reconciled_total_ff_ids,
            duplicate_ff_ids_after_merge,
            staged_epoch_path,
            final_epoch_path,
        })
    })
    .await;

    let prepared = match prep_join {
        Ok(Ok(p)) => p,
        Ok(Err(SelectPrepError::Tournament(e))) => return map_tournament_err(e, &request_id),
        Ok(Err(SelectPrepError::HtmlForge(e))) => {
            return map_html_forge_err(e, Some("mergedHtml"), &request_id)
        }
        Ok(Err(SelectPrepError::ArtifactIo(e))) => return map_artifact_io_err(e, &request_id),
        Ok(Err(SelectPrepError::Storage(msg))) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                msg,
                None,
                None,
                request_id,
            )
        }
        Err(join_err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Tournament select prep task failed: {join_err}"),
                None,
                None,
                request_id,
            )
        }
    };

    // Phase 2: Pure SQLite BEGIN IMMEDIATE transaction with strict generation_epoch fencing (ZERO disk I/O).
    let staged_epoch_path = prepared.staged_epoch_path;
    let final_epoch_path = prepared.final_epoch_path;
    let merged_html = prepared.merged_html;
    let merged_sha256 = prepared.merged_sha256;
    let borrowed_ff_id = prepared.borrowed_ff_id;
    let extracted_css_rules_count = prepared.extracted_css_rules_count;
    let reconciled_total_ff_ids = prepared.reconciled_total_ff_ids;
    let duplicate_ff_ids_after_merge = prepared.duplicate_ff_ids_after_merge;

    let run_id_for_archive = run_id.clone();
    let write_res = tokio::task::spawn_blocking(move || {
        actor.execute_immediate(
            move |tx| -> Result<Result<SelectTournamentVariantData, TournamentError>, rusqlite::Error> {
                ensure_v31_tables(tx)?;

                let stored_row: Option<(i64, Option<String>)> = tx
                    .query_row(
                        "SELECT generation_epoch, winning_variant_id FROM tournament_runs WHERE id = ?1 AND project_id = ?2",
                        params![run_id, project_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((current_epoch, existing_winner)) = stored_row else {
                    return Ok(Err(TournamentError::InvalidInput(format!(
                        "Tournament run '{run_id}' not found in project"
                    ))));
                };

                if generation_epoch < current_epoch
                    || (existing_winner.is_some() && generation_epoch <= current_epoch)
                {
                    let expected = if existing_winner.is_some() {
                        current_epoch.saturating_add(1)
                    } else {
                        current_epoch
                    };
                    return Ok(Err(TournamentError::StaleGenerationEpoch {
                        expected,
                        provided: generation_epoch,
                    }));
                }

                let mut stmt = tx.prepare(
                    "SELECT id FROM tournament_variants WHERE run_id = ?1 ORDER BY rowid ASC",
                )?;
                let all_variant_ids: Vec<String> = stmt
                    .query_map(params![run_id], |r| r.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                drop(stmt);

                if !all_variant_ids.contains(&winning_variant_id) {
                    return Ok(Err(TournamentError::InvalidInput(format!(
                        "Winning variant '{winning_variant_id}' does not belong to run '{run_id}'"
                    ))));
                }
                if let Some(ref donor_id) = borrow_from_variant_id {
                    if !all_variant_ids.contains(donor_id) {
                        return Ok(Err(TournamentError::InvalidInput(format!(
                            "Donor variant '{donor_id}' does not belong to run '{run_id}'"
                        ))));
                    }
                }

                tx.execute(
                    "UPDATE tournament_runs
                     SET winning_variant_id = ?1,
                         borrowed_from_variant_id = ?2,
                         borrowed_ff_id = ?3,
                         generation_epoch = ?4
                     WHERE id = ?5 AND project_id = ?6",
                    params![
                        winning_variant_id,
                        borrow_from_variant_id,
                        borrowed_ff_id,
                        generation_epoch,
                        run_id,
                        project_id,
                    ],
                )?;

                let mut archived_variant_ids = Vec::new();
                for vid in &all_variant_ids {
                    let is_loser = vid != &winning_variant_id;
                    tx.execute(
                        "UPDATE tournament_variants SET archived = ?1 WHERE id = ?2 AND run_id = ?3",
                        params![if is_loser { 1_i64 } else { 0_i64 }, vid, run_id],
                    )?;
                    if is_loser {
                        archived_variant_ids.push(vid.clone());
                    }
                }

                let now_ms = Utc::now().timestamp_millis();
                let mut memory_event_ids = Vec::new();

                let win_event_id = Uuid::new_v4().to_string();
                let win_payload = serde_json::json!({
                    "runId": run_id,
                    "winningVariantId": winning_variant_id,
                    "generationEpoch": generation_epoch,
                    "mergedSha256": merged_sha256,
                    "archivedCount": archived_variant_ids.len(),
                })
                .to_string();
                tx.execute(
                    "INSERT INTO creative_memory_events (id, project_id, event_kind, payload_json, regression_passed, created_at)
                     VALUES (?1, ?2, 'TOURNAMENT_WIN', ?3, 1, ?4)",
                    params![win_event_id, project_id, win_payload, now_ms],
                )?;
                memory_event_ids.push(win_event_id);

                if let Some(ref donor_id) = borrow_from_variant_id {
                    let borrow_event_id = Uuid::new_v4().to_string();
                    let borrow_payload = serde_json::json!({
                        "runId": run_id,
                        "winningVariantId": winning_variant_id,
                        "borrowedFromVariantId": donor_id,
                        "borrowedFfId": borrowed_ff_id,
                        "extractedCssRulesCount": extracted_css_rules_count,
                    })
                    .to_string();
                    tx.execute(
                        "INSERT INTO creative_memory_events (id, project_id, event_kind, payload_json, regression_passed, created_at)
                         VALUES (?1, ?2, 'TOURNAMENT_BORROW', ?3, 1, ?4)",
                        params![borrow_event_id, project_id, borrow_payload, now_ms],
                    )?;
                    memory_event_ids.push(borrow_event_id);
                }

                Ok(Ok(SelectTournamentVariantData {
                    run_id,
                    project_id,
                    winning_variant_id,
                    generation_epoch,
                    merged_html,
                    merged_sha256,
                    borrowed_from_variant_id: borrow_from_variant_id,
                    borrowed_ff_id,
                    extracted_css_rules_count,
                    reconciled_total_ff_ids,
                    duplicate_ff_ids_after_merge,
                    archived_variant_ids,
                    memory_event_ids,
                }))
            },
        )
    })
    .await;

    // Phase 3: Archive losing variants ONLY AFTER SQLite commit succeeds; on failure clean up staged epoch file.
    match write_res {
        Ok(Ok(Ok(data))) => {
            let ws_archive = workspace_root.clone();
            let losers = data.archived_variant_ids.clone();
            let _ = tokio::task::spawn_blocking(move || {
                promote_staged_epoch_merged_html(&staged_epoch_path, &final_epoch_path);
                for vid in &losers {
                    archive_losing_variant_file(&ws_archive, &run_id_for_archive, vid);
                }
            })
            .await;
            json_ok(data, request_id)
        }
        Ok(Ok(Err(t_err))) => {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = fs::remove_file(&staged_epoch_path);
            })
            .await;
            map_tournament_err(t_err, &request_id)
        }
        Ok(Err(actor_err)) => {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = fs::remove_file(&staged_epoch_path);
            })
            .await;
            map_actor_err(actor_err, &request_id)
        }
        Err(join_err) => {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = fs::remove_file(&staged_epoch_path);
            })
            .await;
            json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Tournament select task failed: {join_err}"),
                None,
                None,
                request_id,
            )
        }
    }
}

/// `GET /api/v1/tournament/runs?projectId=...`
pub async fn handle_list_tournament_runs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    flow_principal: Option<Extension<FlowPrincipal>>,
    legacy_ingress: Option<Extension<VerifiedLegacyIngress>>,
    Query(query): Query<ListTournamentRunsQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let (principal, is_superadmin) =
        match extract_principal_strict(&headers, flow_principal, legacy_ingress, &request_id) {
            Ok(v) => v,
            Err(resp) => return resp,
        };

    let (_tenant, _workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &query.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let limit = query.limit.clamp(1, 50) as i64;
    let project_id = query.project_id.trim().to_string();
    let pool = state.registry.pool().clone();
    let project_id_q = project_id.clone();

    let list_join = tokio::task::spawn_blocking(move || -> Result<Vec<PersistedTournamentRun>, String> {
        let conn = pool
            .connection()
            .map_err(|e| format!("Database connection error: {e}"))?;
        ensure_v31_tables(&conn)
            .map_err(|e| format!("Failed to initialize V31 tables: {e}"))?;

        type TournamentRunRow = (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            i64,
        );
        let run_rows: Vec<TournamentRunRow> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, axes_json, winning_variant_id, borrowed_from_variant_id, borrowed_ff_id, generation_epoch, created_at
                     FROM tournament_runs
                     WHERE project_id = ?1
                     ORDER BY created_at DESC
                     LIMIT ?2",
                )
                .map_err(|e| format!("Failed to prepare tournament runs query: {e}"))?;
            let rows = stmt
                .query_map(params![project_id_q, limit], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                })
                .map_err(|e| format!("Failed to list tournament runs: {e}"))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to decode tournament runs: {e}"))?
        };

        let mut runs = Vec::with_capacity(run_rows.len());
        for (
            run_id,
            axes_json,
            winning_variant_id,
            borrowed_from_variant_id,
            borrowed_ff_id,
            generation_epoch,
            created_at,
        ) in run_rows
        {
            let axes: Vec<String> = serde_json::from_str(&axes_json).unwrap_or_default();
            let mut vstmt = conn
                .prepare(
                    "SELECT id, axis_profile, artifact_sha256, ast_similarity, clone_distance_passed, rubric_verdict, archived
                     FROM tournament_variants
                     WHERE run_id = ?1
                     ORDER BY rowid ASC",
                )
                .map_err(|e| format!("Failed to query tournament variants: {e}"))?;
            let mut variants: Vec<PersistedVariantRow> = vstmt
                .query_map(params![run_id], |r| {
                    let passed_i: i64 = r.get(4)?;
                    let archived_i: i64 = r.get(6)?;
                    Ok(PersistedVariantRow {
                        id: r.get(0)?,
                        display_label: String::new(),
                        axis_profile: r.get(1)?,
                        artifact_sha256: r.get(2)?,
                        ast_similarity: r.get(3)?,
                        clone_distance_passed: passed_i != 0,
                        rubric_verdict: r.get(5)?,
                        archived: archived_i != 0,
                    })
                })
                .and_then(|iter| iter.collect())
                .map_err(|e| format!("Failed to decode tournament variants: {e}"))?;
            for (idx, v) in variants.iter_mut().enumerate() {
                v.display_label = BLIND_OPTION_LABELS
                    .get(idx)
                    .copied()
                    .unwrap_or("Option")
                    .to_string();
            }
            runs.push(PersistedTournamentRun {
                id: run_id,
                project_id: project_id_q.clone(),
                axes,
                winning_variant_id,
                borrowed_from_variant_id,
                borrowed_ff_id,
                generation_epoch,
                created_at,
                variants,
            });
        }
        Ok(runs)
    })
    .await;

    match list_join {
        Ok(Ok(runs)) => json_ok(ListTournamentRunsData { project_id, runs }, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            None,
            request_id,
        ),
        Err(join_err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("List tournament runs task failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

enum CrystallizePrepError {
    NotFound {
        code: &'static str,
        msg: String,
        field: Option<&'static str>,
    },
    HtmlForge(HtmlForgeError),
    ArtifactIo(ArtifactIoError),
    Storage(String),
}

struct PreparedCrystallize {
    artifact_rel_path: String,
    artifact_sha256: String,
}

/// `POST /api/v1/crystallize`
pub async fn handle_crystallize(
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

    let req: CrystallizeRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let name = req.name.trim();
    if name.is_empty() || name.len() > 160 {
        return json_err(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "name must be 1..=160 characters",
            Some("name"),
            None,
            request_id,
        );
    }

    let req_run_id = req
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let req_variant_id = req
        .variant_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    if let Some(ref rid) = req_run_id {
        if !is_safe_tournament_id(rid) {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "runId must be a valid single-segment identifier",
                Some("runId"),
                None,
                request_id,
            );
        }
    }
    if let Some(ref vid) = req_variant_id {
        if !is_safe_tournament_id(vid) {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "variantId must be a valid single-segment identifier",
                Some("variantId"),
                None,
                request_id,
            );
        }
        if req_run_id.is_none() {
            return json_err(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "runId is required when variantId is supplied",
                Some("runId"),
                None,
                request_id,
            );
        }
    }

    let (_tenant, workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &req.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let package_id = format!("cryst-{}", Uuid::new_v4().simple());
    let created_at = Utc::now().timestamp_millis();
    let project_id = req.project_id.trim().to_string();
    let name_owned = name.to_string();
    let package_kind = req.package_kind;
    let description = req.description.clone().unwrap_or_default();
    let html_snapshot = req.html_snapshot.clone();

    let pool_for_prep = state.registry.pool().clone();
    let pkg_id_prep = package_id.clone();
    let proj_id_prep = project_id.clone();
    let name_prep = name_owned.clone();

    let prep_join = tokio::task::spawn_blocking(move || -> Result<PreparedCrystallize, CrystallizePrepError> {
        let mut verified_run_epoch_and_winner: Option<(i64, Option<String>)> = None;

        // Cross-project IDOR guard: verify run_id belongs to req.project_id and variant_id belongs to run_id.
        if let Some(ref rid) = req_run_id {
            let conn = pool_for_prep
                .connection()
                .map_err(|e| CrystallizePrepError::Storage(format!("Database connection error: {e}")))?;
            ensure_v31_tables(&conn).map_err(|e| {
                CrystallizePrepError::Storage(format!("Failed to initialize V31 tables: {e}"))
            })?;
            let run_row: Option<(i64, Option<String>)> = conn
                .query_row(
                    "SELECT generation_epoch, winning_variant_id FROM tournament_runs WHERE id = ?1 AND project_id = ?2",
                    params![rid, proj_id_prep],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(|e| CrystallizePrepError::Storage(format!("Failed to verify tournament run: {e}")))?;
            let Some((epoch, winner_opt)) = run_row else {
                return Err(CrystallizePrepError::NotFound {
                    code: "TOURNAMENT_RUN_NOT_FOUND",
                    msg: format!("Tournament run '{rid}' not found in project"),
                    field: Some("runId"),
                });
            };
            verified_run_epoch_and_winner = Some((epoch, winner_opt));

            if let Some(ref vid) = req_variant_id {
                let variant_exists: Option<i64> = conn
                    .query_row(
                        "SELECT 1 FROM tournament_variants WHERE id = ?1 AND run_id = ?2",
                        params![vid, rid],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| {
                        CrystallizePrepError::Storage(format!(
                            "Failed to verify tournament variant: {e}"
                        ))
                    })?;
                if variant_exists.is_none() {
                    return Err(CrystallizePrepError::NotFound {
                        code: "TOURNAMENT_VARIANT_NOT_FOUND",
                        msg: format!("Tournament variant '{vid}' not found in run '{rid}'"),
                        field: Some("variantId"),
                    });
                }
            }
        }

        let resolved_html = if let Some(raw_html) = html_snapshot
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            let (clean, _) = sanitize_html_with_profile(raw_html, SanitizeProfile::AppPreview)
                .map_err(CrystallizePrepError::HtmlForge)?;
            clean
        } else if let (Some(rid), Some(vid)) = (req_run_id.as_deref(), req_variant_id.as_deref()) {
            // Prefer latest committed epoch merged HTML if this variant is the committed winner,
            // falling back to immutable base `<variant_id>.html`; NEVER swallow missing/unreadable files!
            let from_epoch = match verified_run_epoch_and_winner {
                Some((epoch, Some(ref winner_id))) if winner_id == vid => {
                    read_epoch_merged_html(&workspace_root, rid, vid, epoch).ok()
                }
                _ => None,
            };
            match from_epoch {
                Some(h) => h,
                None => read_variant_html(&workspace_root, rid, vid)
                    .map_err(CrystallizePrepError::ArtifactIo)?,
            }
        } else {
            String::new()
        };

        let manifest = serde_json::json!({
            "packageId": pkg_id_prep,
            "projectId": proj_id_prep,
            "packageKind": package_kind.as_str(),
            "name": name_prep,
            "description": description,
            "runId": req_run_id,
            "variantId": req_variant_id,
            "htmlSha256": sha256_hex(resolved_html.as_bytes()),
            "createdAt": created_at,
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| CrystallizePrepError::Storage(format!("Manifest serialization error: {e}")))?;
        let artifact_sha256 = sha256_hex(&manifest_bytes);
        let file_name = format!("{pkg_id_prep}.json");
        let artifact_rel_path = format!(".cowok-design/crystallized/{file_name}");

        atomic_write_nofollow(
            &workspace_root,
            &[".cowok-design", "crystallized"],
            &file_name,
            &manifest_bytes,
        )
        .map_err(CrystallizePrepError::ArtifactIo)?;

        Ok(PreparedCrystallize {
            artifact_rel_path,
            artifact_sha256,
        })
    })
    .await;

    let prepared = match prep_join {
        Ok(Ok(p)) => p,
        Ok(Err(CrystallizePrepError::NotFound { code, msg, field })) => {
            return json_err(StatusCode::NOT_FOUND, code, msg, field, None, request_id)
        }
        Ok(Err(CrystallizePrepError::HtmlForge(e))) => {
            return map_html_forge_err(e, Some("htmlSnapshot"), &request_id)
        }
        Ok(Err(CrystallizePrepError::ArtifactIo(e))) => return map_artifact_io_err(e, &request_id),
        Ok(Err(CrystallizePrepError::Storage(msg))) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                msg,
                None,
                None,
                request_id,
            )
        }
        Err(join_err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Crystallize preparation task failed: {join_err}"),
                None,
                None,
                request_id,
            )
        }
    };

    let artifact_rel_path = prepared.artifact_rel_path;
    let artifact_sha256 = prepared.artifact_sha256;

    let actor = match shared_write_actor_for_pool(state.registry.pool()) {
        Ok(a) => a,
        Err(e) => return map_actor_err(e, &request_id),
    };

    let memory_event_id = Uuid::new_v4().to_string();
    let ev_id_copy = memory_event_id.clone();
    let proj_id_for_sql = project_id.clone();
    let payload_str = serde_json::json!({
        "packageId": package_id,
        "packageKind": package_kind.as_str(),
        "name": name_owned,
        "artifactRelPath": artifact_rel_path,
        "artifactSha256": artifact_sha256,
    })
    .to_string();

    let write_res = tokio::task::spawn_blocking(move || {
        actor.execute_immediate(move |tx| -> Result<(), rusqlite::Error> {
            ensure_v31_tables(tx)?;
            tx.execute(
                "INSERT INTO creative_memory_events (id, project_id, event_kind, payload_json, regression_passed, created_at)
                 VALUES (?1, ?2, 'CRYSTALLIZE', ?3, 1, ?4)",
                params![ev_id_copy, proj_id_for_sql, payload_str, created_at],
            )?;
            Ok(())
        })
    })
    .await;

    match write_res {
        Ok(Ok(())) => json_ok(
            CrystallizeData {
                package_id,
                project_id,
                package_kind,
                name: name_owned,
                artifact_rel_path,
                artifact_sha256,
                memory_event_id,
                created_at,
            },
            request_id,
        ),
        Ok(Err(e)) => map_actor_err(e, &request_id),
        Err(join_err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Crystallize persistence failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

fn table_exists(conn: &Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table_name],
        |_| Ok(()),
    )
    .optional()
    .ok()
    .flatten()
    .is_some()
}

enum AdvisorPrepError {
    Tournament(TournamentError),
    ArtifactIo(ArtifactIoError),
    Storage(String),
}

/// `POST /api/v1/advisor/evolve`
pub async fn handle_advisor_evolve(
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

    let req: AdvisorEvolveRequest = match parse_bounded_json(&body, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let (_tenant, workspace_root) = match resolve_tenant_project_workspace(
        &state,
        &principal,
        is_superadmin,
        &req.project_id,
        &request_id,
    ) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let project_id = req.project_id.trim().to_string();
    let min_hits = req
        .min_studio_correction_hits
        .max(STUDIO_CORRECTION_CLUSTER_THRESHOLD);
    let pool_for_prep = state.registry.pool().clone();
    let project_id_prep = project_id.clone();

    let prep_join =
        tokio::task::spawn_blocking(move || -> Result<AdvisorEvolveOutput, AdvisorPrepError> {
            // Kernel Guard pre-check before touching SQLite or workspace files.
            assert_advisor_target_allowed(Some(&workspace_root), &req.target_skill_path)
                .map_err(AdvisorPrepError::Tournament)?;
            for rp in &req.reference_paths {
                assert_advisor_target_allowed(Some(&workspace_root), rp)
                    .map_err(AdvisorPrepError::Tournament)?;
            }
            assert_staging_human_only_policy(
                &req.target_skill_path,
                req.promote_directly,
                &req.target_channel,
            )
            .map_err(AdvisorPrepError::Tournament)?;

            let (memory_signals, studio_correction_clusters) = {
                let conn = pool_for_prep.connection().map_err(|e| {
                    AdvisorPrepError::Storage(format!("Database connection error: {e}"))
                })?;
                ensure_v31_tables(&conn).map_err(|e| {
                    AdvisorPrepError::Storage(format!("Failed to initialize V31 tables: {e}"))
                })?;

                let mut mem_signals = Vec::new();
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT id, event_kind, payload_json
                 FROM creative_memory_events
                 WHERE project_id = ?1
                 ORDER BY created_at DESC
                 LIMIT 50",
                ) {
                    if let Ok(rows) = stmt.query_map(params![project_id_prep], |r| {
                        Ok(CreativeMemorySignal {
                            event_id: r.get(0)?,
                            event_kind: r.get(1)?,
                            summary: r.get(2)?,
                        })
                    }) {
                        for row in rows.flatten() {
                            mem_signals.push(row);
                        }
                    }
                }

                let mut clusters = Vec::new();
                if table_exists(&conn, "gate_failure_ledger") {
                    if let Ok(mut stmt) = conn.prepare(
                        "SELECT id, rule_id, sample_message, hit_count
                     FROM gate_failure_ledger
                     WHERE project_id = ?1 AND hit_count >= ?2
                     ORDER BY hit_count DESC, last_seen_at DESC
                     LIMIT 50",
                    ) {
                        if let Ok(rows) = stmt.query_map(params![project_id_prep, min_hits], |r| {
                            Ok(StudioCorrectionCluster {
                                ledger_id: r.get(0)?,
                                rule_id: r.get(1)?,
                                sample_message: r.get(2)?,
                                hit_count: r.get(3)?,
                            })
                        }) {
                            for row in rows.flatten() {
                                clusters.push(row);
                            }
                        }
                    }
                }
                (mem_signals, clusters)
            };

            let evolve_out = run_advisor_gepa_loop(
                Some(&workspace_root),
                &AdvisorEvolveInput {
                    project_id: project_id_prep,
                    target_skill_path: req.target_skill_path,
                    reference_paths: req.reference_paths,
                    promote_directly: req.promote_directly,
                    target_channel: req.target_channel,
                    memory_signals,
                    studio_correction_clusters,
                },
            )
            .map_err(AdvisorPrepError::Tournament)?;

            // Persist staging draft markdown atomically with O_NOFOLLOW strictly inside
            // `.cowok-design/staging/design-craft/<patch_id>/SKILL.draft.md` and propagate
            // ANY error BEFORE recording ADVISOR_PATCH in SQLite.
            atomic_write_nofollow(
                &workspace_root,
                &[
                    ".cowok-design",
                    "staging",
                    "design-craft",
                    &evolve_out.patch_id,
                ],
                "SKILL.draft.md",
                evolve_out.draft_patch_markdown.as_bytes(),
            )
            .map_err(AdvisorPrepError::ArtifactIo)?;

            Ok(evolve_out)
        })
        .await;

    let evolve_out = match prep_join {
        Ok(Ok(v)) => v,
        Ok(Err(AdvisorPrepError::Tournament(e))) => return map_tournament_err(e, &request_id),
        Ok(Err(AdvisorPrepError::ArtifactIo(e))) => return map_artifact_io_err(e, &request_id),
        Ok(Err(AdvisorPrepError::Storage(msg))) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                msg,
                None,
                None,
                request_id,
            )
        }
        Err(join_err) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Advisor evolve task failed: {join_err}"),
                None,
                None,
                request_id,
            )
        }
    };

    let actor = match shared_write_actor_for_pool(state.registry.pool()) {
        Ok(a) => a,
        Err(e) => return map_actor_err(e, &request_id),
    };

    let memory_event_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();
    let ev_id_copy = memory_event_id.clone();
    let payload_str = serde_json::json!({
        "patchId": evolve_out.patch_id,
        "targetSkillPath": evolve_out.target_skill_path,
        "stagingStatus": evolve_out.staging_status,
        "approvalPolicy": evolve_out.approval_policy,
        "draftPatchRelPath": evolve_out.draft_patch_rel_path,
        "signalsConsumed": evolve_out.signals_consumed,
        "studioCorrectionsConsumed": evolve_out.studio_corrections_consumed,
        "sealedEvalUntouched": evolve_out.sealed_eval_untouched,
    })
    .to_string();
    let reg_passed = if evolve_out.regression_passed {
        1_i64
    } else {
        0_i64
    };

    let write_res = tokio::task::spawn_blocking(move || {
        actor.execute_immediate(move |tx| -> Result<(), rusqlite::Error> {
            ensure_v31_tables(tx)?;
            tx.execute(
                "INSERT INTO creative_memory_events (id, project_id, event_kind, payload_json, regression_passed, created_at)
                 VALUES (?1, ?2, 'ADVISOR_PATCH', ?3, ?4, ?5)",
                params![ev_id_copy, project_id, payload_str, reg_passed, created_at],
            )?;
            Ok(())
        })
    })
    .await;

    match write_res {
        Ok(Ok(())) => json_ok(
            AdvisorEvolveResponseData {
                result: evolve_out,
                memory_event_id,
                created_at,
            },
            request_id,
        ),
        Ok(Err(e)) => map_actor_err(e, &request_id),
        Err(join_err) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("Advisor evolve persistence failed: {join_err}"),
            None,
            None,
            request_id,
        ),
    }
}

/// Mountable sub-router exposing Story 10.3 Anti-Slop Design Tournament, `/crystallize` & `advisor` 6-Step Loop routes.
pub fn design_tournament_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/tournament/start", post(handle_start_tournament))
        .route(
            "/api/v1/tournament/select",
            post(handle_select_tournament_variant),
        )
        .route("/api/v1/tournament/runs", get(handle_list_tournament_runs))
        .route("/api/v1/crystallize", post(handle_crystallize))
        .route("/api/v1/advisor/evolve", post(handle_advisor_evolve))
}

/// Alias matching sibling `build_*_router()` naming convention in `legacy_router.rs`.
pub fn build_design_tournament_router() -> Router<DaemonAppState> {
    design_tournament_router()
}
