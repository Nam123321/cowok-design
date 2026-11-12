// @story 01.3
use std::fs;
use std::path::{Path, PathBuf};

use axum::{
    extract::{Path as AxumPath, State},
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
use crate::lockfile::validate_and_canonicalize_workspace_root;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuePathTokenRequest {
    pub relative_path: String,
    pub principal_id: Option<String>,
    pub ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathTokenResponse {
    pub token: String,
    pub expires_at: i64,
    pub normalized_relative_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VerifyPathTokenRequest {
    pub token: String,
    pub expected_relative_path: String,
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

fn copy_dir_preserving_git(src: &Path, dst: &Path) -> Result<(), HarnessError> {
    if !src.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(src).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("READ_DIR_FAILED [{}]: {e}", src.display()),
    })? {
        let entry = entry.map_err(|e| HarnessError::ValidationFailed {
            reason: format!("DIR_ENTRY_FAILED: {e}"),
        })?;
        let file_type = entry
            .file_type()
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("FILE_TYPE_FAILED: {e}"),
            })?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        if name_str.ends_with(".lock") || name_str.ends_with(".sock") {
            continue;
        }

        if name_str == ".git" && dst.join(".git").exists() {
            continue;
        }

        let dst_path = dst.join(&file_name);
        if file_type.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("CREATE_DIR_FAILED [{}]: {e}", dst_path.display()),
            })?;
            copy_dir_preserving_git(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|e| HarnessError::ValidationFailed {
                reason: format!(
                    "COPY_FILE_FAILED [{} -> {}]: {e}",
                    src_path.display(),
                    dst_path.display()
                ),
            })?;
        }
    }
    Ok(())
}

pub(super) async fn handle_get_boundary(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
) -> Result<Json<ProjectBoundaryDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, ws_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
                config_dir: ".opendesign".to_string(),
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
        "/Applications/",
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
    // Disallow promoting directly to user's home root
    if let Ok(home) = std::env::var("HOME") {
        let home_norm = home.trim_end_matches('/');
        if norm == home_norm {
            return true;
        }
    }
    false
}

pub(super) async fn handle_promote_scratch(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<PromoteScratchRequest>,
) -> Result<Json<ProjectBoundaryDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, current_ws, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
                "SYSTEM_PATH_DENIED: Target directory '{}' is a protected system directory",
                target_dir_str
            ),
        }));
    }

    let _ = ensure_ac06_tables(&state.registry);
    let mut conn = state
        .registry
        .pool()
        .connection()
        .map_err(map_error_to_response)?;

    let current_boundary_type: Option<String> = conn
        .query_row(
            "SELECT boundary_type FROM project_boundaries WHERE project_id = ?1",
            params![project_id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("QUERY_BOUNDARY_FAILED: {e}"),
            })
        })?;

    if let Some(ref bt) = current_boundary_type {
        if bt == "folder_backed" {
            return Err(map_error_to_response(HarnessError::ValidationFailed {
                reason: "PROJECT_ALREADY_FOLDER_BACKED: Project is already folder-backed and cannot be promoted again".to_string(),
            }));
        }
    }

    fs::create_dir_all(&target_path).map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("FAILED_TO_CREATE_TARGET_DIRECTORY: {e}"),
        })
    })?;
    let target_canon =
        validate_and_canonicalize_workspace_root(&target_path).map_err(map_error_to_response)?;

    if target_canon == current_ws {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "TARGET_DIRECTORY_IDENTICAL: Target directory cannot be identical to current workspace".to_string(),
        }));
    }

    let now_ms = Utc::now().timestamp_millis();

    // AC-09 / F-03: Phase 1 (Intent Phase) - Record PROMOTING status before modifying filesystem
    retry_on_busy("record_promote_intent_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE projects SET status = 'PROMOTING', updated_at = ?1 WHERE id = ?2",
            params![now_ms, project_id.to_string()],
        )?;
        tx.commit()
    })
    .map_err(map_error_to_response)?;

    // Phase 2 (Filesystem Phase) - Copy directory preserving git
    if let Err(e) = copy_dir_preserving_git(&current_ws, &target_canon) {
        // Rollback intent
        let _ = conn.execute(
            "UPDATE projects SET status = 'ACTIVE', updated_at = ?1 WHERE id = ?2",
            params![now_ms, project_id.to_string()],
        );
        return Err(map_error_to_response(e));
    }

    let opendesign_dir = target_canon.join(".opendesign");
    let _ = fs::create_dir_all(&opendesign_dir);
    let opendesign_meta = serde_json::json!({
        "id": project_id,
        "boundary_type": "folder_backed",
        "promoted_at": now_ms,
        "promoted_from": current_ws.display().to_string(),
    });
    let _ = fs::write(
        opendesign_dir.join("project.json"),
        serde_json::to_string_pretty(&opendesign_meta).unwrap_or_default(),
    );

    let is_git = target_canon.join(".git").exists();
    let target_canon_str = target_canon.display().to_string();

    // Phase 3 (Commit Phase) - Update workspace_root, reset status to ACTIVE, update boundary
    retry_on_busy("promote_project_boundary_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE projects SET workspace_root = ?1, status = 'ACTIVE', updated_at = ?2 WHERE id = ?3",
            params![target_canon_str, now_ms, project_id.to_string()],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO project_boundaries (
                project_id, boundary_type, workspace_path, config_dir, is_git_repository,
                promoted_at, created_at, updated_at
            ) VALUES (?1, 'folder_backed', ?2, '.opendesign', ?3, ?4, ?5, ?4)",
            params![
                project_id.to_string(),
                target_canon_str,
                if is_git { 1 } else { 0 },
                now_ms,
                now_ms,
            ],
        )?;
        tx.commit()
    })
    .map_err(map_error_to_response)?;

    // Phase 4 (Cleanup Phase) - Clean up old managed scratch workspace
    if current_boundary_type.as_deref() == Some("managed_scratch")
        && current_ws.exists()
        && current_ws != target_canon
    {
        let _ = fs::remove_dir_all(&current_ws);
    }

    Ok(Json(ProjectBoundaryDto {
        project_id,
        boundary_type: "folder_backed".to_string(),
        workspace_path: target_canon_str,
        config_dir: ".opendesign".to_string(),
        is_git_repository: is_git,
        promoted_at: Some(now_ms),
        created_at: now_ms,
        updated_at: now_ms,
    }))
}

pub(super) async fn handle_list_chat_sessions(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
) -> Result<Json<Vec<ChatSessionDto>>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<CreateChatSessionRequest>,
) -> Result<(StatusCode, Json<ChatSessionDto>), Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    headers: HeaderMap,
    AxumPath((project_id_raw, session_id_raw)): AxumPath<(String, String)>,
) -> Result<Json<ChatSessionDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    headers: HeaderMap,
    AxumPath((project_id_raw, session_id_raw)): AxumPath<(String, String)>,
    Json(req): Json<UpdateChatSessionRequest>,
) -> Result<Json<ChatSessionDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    headers: HeaderMap,
    AxumPath((project_id_raw, session_id_raw)): AxumPath<(String, String)>,
) -> Result<StatusCode, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<IssuePathTokenRequest>,
) -> Result<Json<PathTokenResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, ws_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    let norm_path =
        validate_and_normalize_relative_path(&req.relative_path).map_err(map_error_to_response)?;

    let principal = req
        .principal_id
        .unwrap_or_else(|| extract_actor_id(&headers));
    let secret = crate::synthesis::ensure_project_token(&ws_root).map_err(map_error_to_response)?;

    let ttl = req.ttl_seconds.unwrap_or(3600).max(1);
    let exp = Utc::now().timestamp() + ttl;
    let nonce = Uuid::new_v4().to_string();

    let payload = PathTokenPayload {
        v: 1,
        project_id,
        normalized_relative_path: norm_path.clone(),
        principal_id: principal,
        exp,
        nonce,
    };

    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("SERIALIZE_PAYLOAD_FAILED: {e}"),
        })
    })?;
    let payload_hex = payload_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let mac = hmac_sha256(secret.as_bytes(), payload_hex.as_bytes());
    let mac_hex = mac.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let token = format!("{payload_hex}.{mac_hex}");

    Ok(Json(PathTokenResponse {
        token,
        expires_at: exp,
        normalized_relative_path: norm_path,
    }))
}

pub(super) async fn handle_verify_path_token(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(req): Json<VerifyPathTokenRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (_, ws_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    let norm_expected = validate_and_normalize_relative_path(&req.expected_relative_path)
        .map_err(map_error_to_response)?;

    let secret = crate::synthesis::ensure_project_token(&ws_root).map_err(map_error_to_response)?;

    let parts: Vec<&str> = req.token.split('.').collect();
    if parts.len() != 2 {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_PATH_TOKEN: Malformed token structure".to_string(),
        }));
    }

    let payload_hex = parts[0];
    let mac_hex = parts[1];

    let expected_mac = hmac_sha256(secret.as_bytes(), payload_hex.as_bytes());
    let expected_mac_hex = expected_mac
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    if !constant_time_eq(mac_hex.as_bytes(), expected_mac_hex.as_bytes()) {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_PATH_TOKEN: Signature verification failed".to_string(),
        }));
    }

    let payload_bytes = decode_hex(payload_hex).ok_or_else(|| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_PATH_TOKEN: Payload hex decode failed".to_string(),
        })
    })?;

    let payload: PathTokenPayload = serde_json::from_slice(&payload_bytes).map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("INVALID_PATH_TOKEN: Invalid payload json: {e}"),
        })
    })?;

    if payload.project_id != project_id {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_PATH_TOKEN: Token project mismatch".to_string(),
        }));
    }

    let now_sec = Utc::now().timestamp();
    if payload.exp < now_sec {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "INVALID_PATH_TOKEN: Token expired".to_string(),
        }));
    }

    if payload.normalized_relative_path != norm_expected {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: format!(
                "INVALID_PATH_TOKEN: Path mismatch (expected '{}', got '{}')",
                norm_expected, payload.normalized_relative_path
            ),
        }));
    }

    if let Some(ref p) = req.principal_id {
        if payload.principal_id != *p {
            return Err(map_error_to_response(HarnessError::BoundaryViolation {
                reason: "UNAUTHORIZED: Principal ID mismatch".to_string(),
            }));
        }
    }

    Ok(Json(serde_json::json!({
        "valid": true,
        "normalized_relative_path": norm_expected,
        "project_id": project_id,
        "principal_id": payload.principal_id,
    })))
}
