// @story 01.3
use std::fs;
use std::path::{Path, PathBuf};

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use domain::HarnessError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{retry_on_busy, SpecRegistry};
use uuid::Uuid;

use super::*;
use crate::daemon_auth::VerifiedLegacyIngress;

pub(super) fn parse_project_uuid(raw: &str) -> Result<Uuid, HarnessError> {
    let trimmed = raw.trim();
    Uuid::parse_str(trimmed).map_err(|_| HarnessError::ValidationFailed {
        reason: format!("INVALID_PROJECT_UUID: '{trimmed}' is not a valid RFC-4122 UUID"),
    })
}

pub(super) fn extract_actor_id(headers: &HeaderMap) -> String {
    headers
        .get("x-cowok-actor-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("http-anonymous")
        .to_string()
}

pub fn map_error_to_response(err: HarnessError) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let (status, code) = match &err {
        HarnessError::StaleFile { .. } => (StatusCode::CONFLICT, "STALE_FILE"),
        HarnessError::InvalidSpecKey { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, "INVALID_SPEC_KEY")
        }
        HarnessError::ProjectNotFound { .. } => (StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND"),
        HarnessError::PathTraversalDenied { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, "PATH_TRAVERSAL_DENIED")
        }
        HarnessError::SqliteBusy { .. } => (StatusCode::SERVICE_UNAVAILABLE, "DATABASE_BUSY"),
        HarnessError::UnsafeInteger { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "UNSAFE_INTEGER"),
        HarnessError::BoundaryViolation { ref reason } => {
            if reason.starts_with("UNAUTHORIZED") {
                (StatusCode::UNAUTHORIZED, "UNAUTHORIZED")
            } else if reason.starts_with("FS_JAIL_VIOLATION") {
                (StatusCode::FORBIDDEN, "FS_JAIL_VIOLATION")
            } else {
                (StatusCode::FORBIDDEN, "TENANT_ISOLATION_BREACH")
            }
        }
        HarnessError::PromptBudgetExceeded { .. } => {
            (StatusCode::PAYLOAD_TOO_LARGE, "PROMPT_BUDGET_EXCEEDED")
        }
        HarnessError::InvalidPromptPayload { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, "INVALID_PROMPT_PAYLOAD")
        }
        HarnessError::ModelAdaptationFailed { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, "MODEL_ADAPTATION_FAILED")
        }
        HarnessError::ValidationFailed { reason } => {
            if reason.contains("UNIQUE_CONSTRAINT") {
                (StatusCode::CONFLICT, "UNIQUE_CONSTRAINT")
            } else if reason.contains("WORKSPACE_ROOT_CONFLICT") {
                (StatusCode::CONFLICT, "WORKSPACE_ROOT_CONFLICT")
            } else if reason.contains("REFERENTIAL_INTEGRITY") {
                (StatusCode::UNPROCESSABLE_ENTITY, "REFERENTIAL_INTEGRITY")
            } else if reason.contains("PROJECT_SCOPE_VIOLATION") {
                (StatusCode::FORBIDDEN, "PROJECT_SCOPE_VIOLATION")
            } else if reason.contains("BRIEF_EMPTY") {
                (StatusCode::UNPROCESSABLE_ENTITY, "BRIEF_EMPTY")
            } else if reason.contains("SESSION_NOT_FOUND") {
                (StatusCode::NOT_FOUND, "SESSION_NOT_FOUND")
            } else if reason.contains("PROJECT_ALREADY_FOLDER_BACKED") {
                (StatusCode::CONFLICT, "PROJECT_ALREADY_FOLDER_BACKED")
            } else if reason.contains("PROMOTE_CONFLICT") {
                (StatusCode::CONFLICT, "PROMOTE_CONFLICT")
            } else if reason.contains("INVALID_PATH_TOKEN") {
                (StatusCode::UNPROCESSABLE_ENTITY, "INVALID_PATH_TOKEN")
            } else if reason.contains("SESSION_TITLE_EMPTY") {
                (StatusCode::UNPROCESSABLE_ENTITY, "SESSION_TITLE_EMPTY")
            } else {
                (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_FAILED")
            }
        }
    };

    let body = HttpErrorBody {
        code: code.to_string(),
        kind: Some(code.to_string()),
        message: err.to_string(),
        details: serde_json::to_value(&err).unwrap_or(serde_json::Value::Null),
        request_id,
    };

    (status, Json(body)).into_response()
}

// ============================================================================
// AC-06: Hybrid Project Boundary & 1 Project : N Chat Sessions & Path Token
// ============================================================================

pub fn ensure_ac06_tables(registry: &SpecRegistry) -> Result<(), HarnessError> {
    // Schema is centrally managed in V2 migration in crates/spec-registry (AC-10 / F-04).
    // Verify pool connection is operational:
    let _conn = registry.pool().connection()?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectBoundaryDto {
    pub project_id: Uuid,
    pub boundary_type: String,
    pub workspace_path: String,
    pub config_dir: String,
    pub is_git_repository: bool,
    pub promoted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromoteScratchRequest {
    pub target_directory: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub path_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSessionDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChatSessionRequest {
    pub id: Option<Uuid>,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChatSessionRequest {
    pub title: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssuePathTokenRequest {
    #[serde(default)]
    pub relative_path: Option<String>,
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default)]
    pub principal_id: Option<String>,
    #[serde(default)]
    pub ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathTokenResponse {
    pub token: String,
    #[serde(default)]
    pub path_token: String,
    pub expires_at: i64,
    pub normalized_relative_path: String,
    #[serde(default)]
    pub canonical_path: String,
    #[serde(default)]
    pub daemon_nonce: String,
    #[serde(default)]
    pub dev_ino: String,
    #[serde(default)]
    pub ancestor_dev_ino: String,
    #[serde(default)]
    pub target_exists: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VerifyPathTokenRequest {
    pub token: String,
    #[serde(default)]
    pub expected_relative_path: Option<String>,
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default)]
    pub target_directory: Option<String>,
    #[serde(default)]
    pub principal_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathTokenPayload {
    pub v: u32,
    pub project_id: Uuid,
    pub normalized_relative_path: String,
    pub principal_id: String,
    pub exp: i64,
    pub nonce: String,
    #[serde(default)]
    pub daemon_nonce: String,
    #[serde(default)]
    pub canonical_path: String,
    #[serde(default)]
    pub ancestor_dev_ino: String,
    #[serde(default)]
    pub target_exists: bool,
}

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut k_ipad = [0u8; 64];
    let mut k_opad = [0u8; 64];
    for i in 0..64 {
        k_ipad[i] = k[i] ^ 0x36;
        k_opad[i] = k[i] ^ 0x5c;
    }

    let mut inner = Sha256::new();
    inner.update(k_ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(k_opad);
    outer.update(inner_hash);
    let outer_hash = outer.finalize();

    let mut result = [0u8; 32];
    result.copy_from_slice(&outer_hash);
    result
}

pub fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

pub fn validate_and_normalize_relative_path(path_str: &str) -> Result<String, HarnessError> {
    if path_str.contains('\0') {
        return Err(HarnessError::PathTraversalDenied {
            path: path_str.to_string(),
        });
    }
    let lower = path_str.to_lowercase();
    if lower.contains("%2e%2e") || lower.contains("%2f") || lower.contains("%5c") {
        return Err(HarnessError::PathTraversalDenied {
            path: path_str.to_string(),
        });
    }
    if path_str.starts_with('/')
        || path_str.starts_with('\\')
        || (path_str.len() >= 2 && path_str.chars().nth(1) == Some(':'))
    {
        return Err(HarnessError::PathTraversalDenied {
            path: path_str.to_string(),
        });
    }

    let p = Path::new(path_str);
    let mut components = Vec::new();
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(c) => {
                let s = c
                    .to_str()
                    .ok_or_else(|| HarnessError::PathTraversalDenied {
                        path: path_str.to_string(),
                    })?;
                if s == ".." {
                    return Err(HarnessError::PathTraversalDenied {
                        path: path_str.to_string(),
                    });
                }
                components.push(s);
            }
            std::path::Component::CurDir => {}
            _ => {
                return Err(HarnessError::PathTraversalDenied {
                    path: path_str.to_string(),
                });
            }
        }
    }

    if components.is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "PATH_EMPTY: relative_path cannot be empty".to_string(),
        });
    }

    Ok(components.join("/"))
}

fn copy_dir_preserving_git_fds(
    src_label: &Path,
    src_fd: &std::os::fd::OwnedFd,
    dst_fd: &std::os::fd::OwnedFd,
    rel_prefix: &Path,
    copied_files: &mut Vec<PathBuf>,
) -> Result<(), HarnessError> {
    let entries = super::fs_jail::read_dir_fd_nofollow(src_fd, &src_label.display().to_string())?;
    for entry in entries {
        let name_str = entry.name.to_string_lossy();
        if name_str.ends_with(".lock")
            || name_str.ends_with(".sock")
            || name_str.starts_with(".cowok-tmp-")
            || entry.is_symlink
        {
            continue;
        }
        let child_label = src_label.join(&entry.name);
        let child_rel = rel_prefix.join(&entry.name);

        if name_str == ".git"
            && super::fs_jail::open_child_dir_fd_nofollow(dst_fd, &entry.name)?.is_some()
        {
            continue;
        }

        if entry.is_dir {
            let child_src_fd = super::fs_jail::open_child_dir_fd_nofollow(src_fd, &entry.name)?
                .ok_or_else(|| HarnessError::ValidationFailed {
                    reason: format!("SRC_DIR_MISSING [{}]", child_label.display()),
                })?;
            let child_dst_fd = super::fs_jail::ensure_child_dir_fd(dst_fd, &entry.name, 0o700)?;
            copy_dir_preserving_git_fds(
                &child_label,
                &child_src_fd,
                &child_dst_fd,
                &child_rel,
                copied_files,
            )?;
        } else if entry.is_file {
            super::fs_jail::copy_file_between_dir_fds(src_fd, dst_fd, &entry.name, 0o600)?;
            copied_files.push(child_rel);
        }
    }
    Ok(())
}

fn copy_dir_preserving_git(src: &Path, dst: &Path) -> Result<Vec<PathBuf>, HarnessError> {
    if !src.exists() {
        return Ok(Vec::new());
    }
    let src_fd = super::fs_jail::open_dir_abs_nofollow(src)?;
    let dst_fd = super::fs_jail::open_dir_abs_nofollow(dst)?;
    let mut copied_files = Vec::new();
    if let Err(e) =
        copy_dir_preserving_git_fds(src, &src_fd, &dst_fd, Path::new(""), &mut copied_files)
    {
        for rel in copied_files {
            let _ = super::fs_jail::remove_rel_file_in_dir_fd(&dst_fd, &rel);
        }
        return Err(e);
    }
    Ok(copied_files)
}

pub(super) async fn handle_get_boundary(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
) -> Result<Json<ProjectBoundaryDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, ws_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let _ = ensure_ac06_tables(&state.registry);
    let conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    let boundary_opt: Option<ProjectBoundaryDto> = conn
        .query_row(
            "SELECT project_id, boundary_type, workspace_path, config_dir, is_git_repository,
                    promoted_at, created_at, updated_at
             FROM project_boundaries WHERE project_id = ?1",
            params![project_id.to_string()],
            |row| {
                let pid_str: String = row.get(0)?;
                let b_type: String = row.get(1)?;
                let ws: String = row.get(2)?;
                let cfg: String = row.get(3)?;
                let is_git: i64 = row.get(4)?;
                let prom_at: Option<i64> = row.get(5)?;
                let cr_at: i64 = row.get(6)?;
                let up_at: i64 = row.get(7)?;
                Ok(ProjectBoundaryDto {
                    project_id: Uuid::parse_str(&pid_str).unwrap_or(project_id),
                    boundary_type: b_type,
                    workspace_path: ws,
                    config_dir: cfg,
                    is_git_repository: is_git != 0,
                    promoted_at: prom_at,
                    created_at: cr_at,
                    updated_at: up_at,
                })
            },
        )
        .optional()
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("QUERY_BOUNDARY_FAILED: {e}"),
            })
        })?;

    let boundary = match boundary_opt {
        Some(b) => b,
        None => {
            let is_git = ws_root.join(".git").exists();
            let now = Utc::now().timestamp_millis();
            ProjectBoundaryDto {
                project_id,
                boundary_type: "folder_backed".to_string(),
                workspace_path: ws_root.display().to_string(),
                config_dir: ".cowok-design".to_string(),
                is_git_repository: is_git,
                promoted_at: None,
                created_at: now,
                updated_at: now,
            }
        }
    };

    Ok(Json(boundary))
}

fn is_system_or_root_path(p: &Path) -> bool {
    let s = p.to_string_lossy();
    let norm = s.trim_end_matches('/');
    if norm.is_empty() || norm == "/" || norm == "~" {
        return true;
    }
    let exact_system = [
        "/",
        "/etc",
        "/usr",
        "/bin",
        "/sbin",
        "/var",
        "/private",
        "/System",
        "/Library",
        "/Applications",
        "/Volumes",
        "/boot",
        "/dev",
        "/proc",
        "/sys",
        "C:\\",
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\Program Files (x86)",
    ];
    for d in exact_system {
        if norm.eq_ignore_ascii_case(d) {
            return true;
        }
    }
    // Subtree blocks for critical system binaries & OS kernel dirs
    let system_prefixes = [
        "/etc/",
        "/bin/",
        "/sbin/",
        "/System/",
        "/Library/",
        "/Applications/",
        "/Volumes/",
        "/dev/",
        "/proc/",
        "/sys/",
        "C:\\Windows\\",
        "C:\\Program Files\\",
        "C:\\Program Files (x86)\\",
    ];
    for pfx in system_prefixes {
        if norm.to_lowercase().starts_with(&pfx.to_lowercase()) {
            return true;
        }
    }
    // Block /usr/ except /usr/local/
    if norm.starts_with("/usr/") && !norm.starts_with("/usr/local/") {
        return true;
    }
    // Disallow promoting directly to user's home root or sensitive home subdirectories
    if let Ok(home) = std::env::var("HOME") {
        let home_norm = home.trim_end_matches('/');
        let norm_lower = norm.to_lowercase();
        let home_lower = home_norm.to_lowercase();
        if norm_lower == home_lower {
            return true;
        }
        for sub in ["/library", "/.ssh", "/.gnupg", "/.aws", "/.kube"] {
            let blocked = format!("{home_lower}{sub}");
            if norm_lower == blocked || norm_lower.starts_with(&format!("{blocked}/")) {
                return true;
            }
        }
    }
    false
}

pub(super) async fn handle_promote_scratch(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<PromoteScratchRequest>,
) -> Result<Json<ProjectBoundaryDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, current_ws, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let target_dir_str = req.target_directory.trim();
    if target_dir_str.is_empty() {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "TARGET_DIRECTORY_EMPTY: target_directory must not be empty".to_string(),
        }));
    }
    if target_dir_str.contains("..") || target_dir_str.contains('\0') {
        return Err(map_error_to_response(HarnessError::PathTraversalDenied {
            path: target_dir_str.to_string(),
        }));
    }

    let target_path = PathBuf::from(target_dir_str);
    if is_system_or_root_path(&target_path) {
        return Err(map_error_to_response(HarnessError::BoundaryViolation {
            reason: format!(
                "FS_JAIL_VIOLATION:SYSTEM_PATH_DENIED: Target directory '{}' is a protected system directory",
                target_dir_str
            ),
        }));
    }
    let anchor = super::fs_jail::inspect_path_jail(&target_path).map_err(map_error_to_response)?;
    super::fs_jail::ensure_writable_target_not_home(&anchor.canonical_path)
        .map_err(map_error_to_response)?;

    let tok = req
        .path_token
        .as_deref()
        .or(req.token.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            map_error_to_response(HarnessError::BoundaryViolation {
                reason: "FS_JAIL_VIOLATION: Promote target requires a signed PathToken".to_string(),
            })
        })?;
    let secret =
        crate::synthesis::ensure_project_token(&current_ws).map_err(map_error_to_response)?;
    let payload = super::fs_jail::decode_and_verify_path_token(
        secret.as_bytes(),
        tok,
        project_id,
        &state.auth.daemon_nonce,
        true,
    )
    .map_err(map_error_to_response)?;
    if payload.canonical_path != anchor.canonical_path.display().to_string()
        || payload.ancestor_dev_ino.is_empty()
        || payload.ancestor_dev_ino != anchor.ancestor_dev_ino
    {
        return Err(map_error_to_response(HarnessError::BoundaryViolation {
            reason: "FS_JAIL_VIOLATION: Promote target failed PathToken inode/canonical check"
                .to_string(),
        }));
    }

    let _ = ensure_ac06_tables(&state.registry);
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    let provisioned = super::fs_jail::provision_dir_zero_toctou_checked(
        &target_path,
        Some(&payload.canonical_path),
        Some(&payload.ancestor_dev_ino),
    )
    .map_err(map_error_to_response)?;
    let target_canon = provisioned.canonical_path;
    let target_canon_str = target_canon.display().to_string();

    if target_canon == current_ws || target_canon.starts_with(&current_ws) {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "TARGET_DIRECTORY_IDENTICAL: Target directory cannot be identical to or nested inside current workspace".to_string(),
        }));
    }

    let rollback_promote_intent = |conn_ref: &mut rusqlite::Connection, ts: i64| {
        if let Err(rb_err) = retry_on_busy("rollback_promote_intent_tx", || {
            let tx = conn_ref.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE projects SET status = 'ACTIVE', updated_at = ?1 WHERE id = ?2 AND status = 'PROMOTING'",
                params![ts, project_id.to_string()],
            )?;
            tx.commit()
        }) {
            eprintln!("[harness-daemon] CRITICAL: Failed to rollback PROMOTING status for {project_id}: {rb_err}");
        }
    };

    let now_ms = Utc::now().timestamp_millis();

    // AC-09 / F-01: Phase 1 (Atomic Intent Phase) - Validate boundary, active conflicts & CAS PROMOTING lock inside BEGIN IMMEDIATE
    let mut precheck_boundary_type: Option<String> = None;
    let mut phase1_domain_err: Option<HarnessError> = None;
    retry_on_busy("record_promote_intent_tx", || {
        precheck_boundary_type = None;
        phase1_domain_err = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let bt: Option<String> = tx
            .query_row(
                "SELECT boundary_type FROM project_boundaries WHERE project_id = ?1",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if bt.as_deref() == Some("folder_backed") {
            phase1_domain_err = Some(HarnessError::ValidationFailed {
                reason: "PROJECT_ALREADY_FOLDER_BACKED: Project is already folder-backed and cannot be promoted again".to_string(),
            });
            return Ok(());
        }
        let ws_conflicts: i64 = tx.query_row(
            "SELECT COUNT(*) FROM projects WHERE workspace_root = ?1 AND trash_deleted_at IS NULL AND id != ?2",
            params![target_canon_str, project_id.to_string()],
            |r| r.get(0),
        )?;
        if ws_conflicts > 0 {
            phase1_domain_err = Some(HarnessError::ValidationFailed {
                reason: "WORKSPACE_ROOT_CONFLICT: Target directory is already bound to another active project".to_string(),
            });
            return Ok(());
        }
        let changed = tx.execute(
            "UPDATE projects SET status = 'PROMOTING', updated_at = ?1 WHERE id = ?2 AND status = 'ACTIVE' AND trash_deleted_at IS NULL",
            params![now_ms, project_id.to_string()],
        )?;
        if changed == 0 {
            phase1_domain_err = Some(HarnessError::ValidationFailed {
                reason: "PROMOTE_CONFLICT: Project is already promoting, trashed, or not ACTIVE".to_string(),
            });
            return Ok(());
        }
        precheck_boundary_type = bt;
        tx.commit()
    })
    .map_err(map_error_to_response)?;
    if let Some(err) = phase1_domain_err {
        return Err(map_error_to_response(err));
    }

    let rollback_copied_files = |files: &[PathBuf], remove_project_json: bool| {
        if let Ok(target_fd) = super::fs_jail::open_dir_abs_nofollow(&target_canon) {
            for rel in files {
                let _ = super::fs_jail::remove_rel_file_in_dir_fd(&target_fd, rel);
            }
            if remove_project_json {
                let _ = super::fs_jail::remove_rel_file_in_dir_fd(
                    &target_fd,
                    Path::new(".cowok-design/project.json"),
                );
            }
        }
    };

    // Phase 2 (Filesystem Phase) - Copy directory preserving git using FD-anchored openat/O_NOFOLLOW
    let copied_files = match copy_dir_preserving_git(&current_ws, &target_canon) {
        Ok(files) => files,
        Err(e) => {
            rollback_promote_intent(&mut conn, now_ms);
            return Err(map_error_to_response(e));
        }
    };

    let target_fd = match super::fs_jail::open_dir_abs_nofollow(&target_canon) {
        Ok(fd) => fd,
        Err(e) => {
            rollback_copied_files(&copied_files, false);
            rollback_promote_intent(&mut conn, now_ms);
            return Err(map_error_to_response(e));
        }
    };
    let cowok_fd = match super::fs_jail::ensure_child_dir_fd(
        &target_fd,
        std::ffi::OsStr::new(".cowok-design"),
        0o700,
    ) {
        Ok(fd) => fd,
        Err(e) => {
            rollback_copied_files(&copied_files, false);
            rollback_promote_intent(&mut conn, now_ms);
            return Err(map_error_to_response(e));
        }
    };
    let boundary_meta = serde_json::json!({
        "id": project_id,
        "boundary_type": "folder_backed",
        "promoted_at": now_ms,
        "promoted_from": current_ws.display().to_string(),
    });
    let bytes = serde_json::to_vec_pretty(&boundary_meta).unwrap_or_default();
    if let Err(e) = super::fs_jail::write_file_in_dir_fd(
        &cowok_fd,
        std::ffi::OsStr::new("project.json"),
        &bytes,
        0o600,
    ) {
        rollback_copied_files(&copied_files, false);
        rollback_promote_intent(&mut conn, now_ms);
        return Err(map_error_to_response(e));
    }

    let is_git = match super::fs_jail::open_child_dir_fd_nofollow(
        &target_fd,
        std::ffi::OsStr::new(".git"),
    ) {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            rollback_copied_files(&copied_files, true);
            rollback_promote_intent(&mut conn, now_ms);
            return Err(map_error_to_response(e));
        }
    };

    // Phase 3 (Commit Phase) - Update workspace_root, provenance, reset status to ACTIVE, update boundary
    if let Err(e) = retry_on_busy("promote_project_boundary_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE projects SET workspace_root = ?1, provenance = 'folder-backed', status = 'ACTIVE', updated_at = ?2 WHERE id = ?3",
            params![target_canon_str, now_ms, project_id.to_string()],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO project_boundaries (
                project_id, boundary_type, workspace_path, config_dir, is_git_repository,
                promoted_at, created_at, updated_at
            ) VALUES (?1, 'folder_backed', ?2, '.cowok-design', ?3, ?4, ?5, ?4)",
            params![
                project_id.to_string(),
                target_canon_str,
                if is_git { 1 } else { 0 },
                now_ms,
                now_ms,
            ],
        )?;
        tx.commit()
    }) {
        rollback_copied_files(&copied_files, true);
        rollback_promote_intent(&mut conn, now_ms);
        return Err(map_error_to_response(e));
    }

    // Phase 4 (Cleanup Phase) - Clean up old managed scratch workspace
    if precheck_boundary_type.as_deref() == Some("managed_scratch")
        && fs::symlink_metadata(&current_ws)
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        && current_ws != target_canon
    {
        if let Err(rm_err) = fs::remove_dir_all(&current_ws) {
            eprintln!(
                "[harness-daemon] WARN: Failed to remove old managed_scratch dir '{}' for project {project_id}: {rm_err}",
                current_ws.display()
            );
        }
    }

    Ok(Json(ProjectBoundaryDto {
        project_id,
        boundary_type: "folder_backed".to_string(),
        workspace_path: target_canon_str,
        config_dir: ".cowok-design".to_string(),
        is_git_repository: is_git,
        promoted_at: Some(now_ms),
        created_at: now_ms,
        updated_at: now_ms,
    }))
}

pub(super) async fn handle_list_chat_sessions(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
) -> Result<Json<Vec<ChatSessionDto>>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let _ = ensure_ac06_tables(&state.registry);
    let conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, title, created_at, updated_at
             FROM chat_sessions WHERE project_id = ?1 ORDER BY updated_at DESC",
        )
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("PREPARE_SESSIONS_FAILED: {e}"),
            })
        })?;

    let sessions = stmt
        .query_map(params![project_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let pid_str: String = row.get(1)?;
            let title: String = row.get(2)?;
            let cr: i64 = row.get(3)?;
            let up: i64 = row.get(4)?;
            Ok(ChatSessionDto {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                project_id: Uuid::parse_str(&pid_str).unwrap_or(project_id),
                title,
                created_at: cr,
                updated_at: up,
            })
        })
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("QUERY_SESSIONS_FAILED: {e}"),
            })
        })?
        .filter_map(Result::ok)
        .collect();

    Ok(Json(sessions))
}

pub(super) async fn handle_create_chat_session(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<CreateChatSessionRequest>,
) -> Result<(StatusCode, Json<ChatSessionDto>), Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "SESSION_TITLE_EMPTY: Chat session title must not be empty".to_string(),
        }));
    }

    let _ = ensure_ac06_tables(&state.registry);
    let session_id = req.id.unwrap_or_else(Uuid::new_v4);
    let now_ms = Utc::now().timestamp_millis();
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    retry_on_busy("create_chat_session_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO chat_sessions (id, project_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id.to_string(),
                project_id.to_string(),
                title,
                now_ms,
                now_ms
            ],
        )?;
        tx.commit()
    })
    .map_err(map_error_to_response)?;

    Ok((
        StatusCode::CREATED,
        Json(ChatSessionDto {
            id: session_id,
            project_id,
            title: req.title.trim().to_string(),
            created_at: now_ms,
            updated_at: now_ms,
        }),
    ))
}

pub(super) async fn handle_get_chat_session(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath((project_id_raw, session_id_raw)): AxumPath<(String, String)>,
) -> Result<Json<ChatSessionDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let session_id = Uuid::parse_str(&session_id_raw).map_err(|_| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_SESSION_ID: Invalid UUID".to_string(),
        })
    })?;

    let _ = ensure_ac06_tables(&state.registry);
    let conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    let session = conn
        .query_row(
            "SELECT id, project_id, title, created_at, updated_at
             FROM chat_sessions WHERE id = ?1 AND project_id = ?2",
            params![session_id.to_string(), project_id.to_string()],
            |row| {
                let id_str: String = row.get(0)?;
                let pid_str: String = row.get(1)?;
                let title: String = row.get(2)?;
                let cr: i64 = row.get(3)?;
                let up: i64 = row.get(4)?;
                Ok(ChatSessionDto {
                    id: Uuid::parse_str(&id_str).unwrap_or(session_id),
                    project_id: Uuid::parse_str(&pid_str).unwrap_or(project_id),
                    title,
                    created_at: cr,
                    updated_at: up,
                })
            },
        )
        .optional()
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("QUERY_SESSION_FAILED: {e}"),
            })
        })?
        .ok_or_else(|| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: "SESSION_NOT_FOUND: Chat session does not exist".to_string(),
            })
        })?;

    Ok(Json(session))
}

pub(super) async fn handle_update_chat_session(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath((project_id_raw, session_id_raw)): AxumPath<(String, String)>,
    Json(req): Json<UpdateChatSessionRequest>,
) -> Result<Json<ChatSessionDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let session_id = Uuid::parse_str(&session_id_raw).map_err(|_| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_SESSION_ID: Invalid UUID".to_string(),
        })
    })?;

    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "SESSION_TITLE_EMPTY: Title must not be empty".to_string(),
        }));
    }

    let _ = ensure_ac06_tables(&state.registry);
    let now_ms = Utc::now().timestamp_millis();
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    let rows_affected = retry_on_busy("update_chat_session_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count = tx.execute(
            "UPDATE chat_sessions SET title = ?1, updated_at = ?2 WHERE id = ?3 AND project_id = ?4",
            params![title, now_ms, session_id.to_string(), project_id.to_string()],
        )?;
        tx.commit()?;
        Ok(count)
    })
    .map_err(map_error_to_response)?;

    if rows_affected == 0 {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "SESSION_NOT_FOUND: Session not found for update".to_string(),
        }));
    }

    let created_at: i64 = conn
        .query_row(
            "SELECT created_at FROM chat_sessions WHERE id = ?1",
            params![session_id.to_string()],
            |r| r.get(0),
        )
        .unwrap_or(now_ms);

    Ok(Json(ChatSessionDto {
        id: session_id,
        project_id,
        title,
        created_at,
        updated_at: now_ms,
    }))
}

pub(super) async fn handle_delete_chat_session(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath((project_id_raw, session_id_raw)): AxumPath<(String, String)>,
) -> Result<StatusCode, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let session_id = Uuid::parse_str(&session_id_raw).map_err(|_| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_SESSION_ID: Invalid UUID".to_string(),
        })
    })?;

    let _ = ensure_ac06_tables(&state.registry);
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    let rows_affected = retry_on_busy("delete_chat_session_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count = tx.execute(
            "DELETE FROM chat_sessions WHERE id = ?1 AND project_id = ?2",
            params![session_id.to_string(), project_id.to_string()],
        )?;
        tx.commit()?;
        Ok(count)
    })
    .map_err(map_error_to_response)?;

    if rows_affected == 0 {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "SESSION_NOT_FOUND: Session not found for deletion".to_string(),
        }));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn handle_issue_path_token(
    state: State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    project_id_raw: AxumPath<String>,
    req: Json<IssuePathTokenRequest>,
) -> Result<Json<PathTokenResponse>, Response> {
    super::fs_jail::handle_issue_path_token_v2(state, caller, headers, project_id_raw, req).await
}

pub(super) async fn handle_verify_path_token(
    state: State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    headers: HeaderMap,
    project_id_raw: AxumPath<String>,
    req: Json<VerifyPathTokenRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    super::fs_jail::handle_verify_path_token_v2(state, caller, headers, project_id_raw, req).await
}
