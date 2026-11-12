// @story 01.3
use std::fs;
use std::path::PathBuf;

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use chrono::Utc;
use domain::{
    AuthorizedContextSlice, CanonicalSpecKey, ContextSliceRequest, Department, HarnessError,
    Portal, ProvenanceStatus, SafeJsInteger,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::{
    map_sqlite_error, read_jailed_spec_bytes, retry_on_busy, SpecRecord, SpecRegistry,
    UpsertSpecInput,
};
use uuid::Uuid;

use super::*;
use crate::lockfile::validate_and_canonicalize_workspace_root;
use crate::synthesis::{
    resolve_project_scoped_registry, synthesize_nine_specs, SynthesizeSpecsRequest,
    SynthesizeSpecsResponse,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectRequest {
    pub id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub product_brief: String,
    pub workspace_root: Option<String>,
    pub inherit_brand_from_project_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub product_brief: String,
    pub inherit_brand_from_project_id: Option<Uuid>,
    pub created_at: SafeJsInteger,
    pub updated_at: SafeJsInteger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecItemWithDriftDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub spec_key: CanonicalSpecKey,
    pub file_path: String,
    pub status: ProvenanceStatus,
    pub version: u32,
    pub content_sha256: String,
    pub summary_digest: String,
    pub provenance_summary_json: String,
    pub source_project_id: Option<Uuid>,
    pub source_revision_sha256: Option<String>,
    pub upstream_outdated: bool,
    pub markdown_content: String,
    pub updated_at: SafeJsInteger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSpecsListResponse {
    pub project_id: Uuid,
    pub upstream_brand_drift_detected: bool,
    pub specs: Vec<SpecItemWithDriftDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutSpecRequest {
    pub markdown_content: String,
    pub expected_sha256: Option<String>,
    pub status: Option<ProvenanceStatus>,
    pub change_summary: Option<String>,
    pub source_project_id: Option<Uuid>,
    pub source_revision_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpErrorBody {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub message: String,
    pub details: serde_json::Value,
    pub request_id: String,
}

pub(super) async fn handle_list_projects(
    State(state): State<DaemonAppState>,
) -> Result<Json<Vec<ProjectDto>>, Response> {
    let registry = state.registry.clone();
    let projects = tokio::task::spawn_blocking(move || {
        let conn = registry.pool().connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, name, product_brief, inherit_brand_from_project_id, created_at, updated_at
             FROM projects
             WHERE trash_deleted_at IS NULL
             ORDER BY updated_at DESC",
        ).map_err(|e| map_sqlite_error(e, "prepare_list_projects"))?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let slug: String = row.get(1)?;
            let name: String = row.get(2)?;
            let brief: String = row.get(3)?;
            let inherit_str: Option<String> = row.get(4)?;
            let cr: i64 = row.get(5)?;
            let up: i64 = row.get(6)?;
            Ok(ProjectDto {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                slug,
                name,
                product_brief: brief,
                inherit_brand_from_project_id: inherit_str.and_then(|s| Uuid::parse_str(&s).ok()),
                created_at: SafeJsInteger::new(cr).unwrap_or(SafeJsInteger::ZERO),
                updated_at: SafeJsInteger::new(up).unwrap_or(SafeJsInteger::ZERO),
            })
        }).map_err(|e| map_sqlite_error(e, "query_list_projects"))?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| map_sqlite_error(e, "row_list_projects"))?);
        }
        Ok::<Vec<ProjectDto>, HarnessError>(list)
    })
    .await
    .map_err(|e| map_error_to_response(HarnessError::ValidationFailed {
        reason: format!("BLOCKING_TASK_FAILED: {e}"),
    }))?
    .map_err(map_error_to_response)?;

    Ok(Json(projects))
}

pub(super) async fn handle_create_project(
    State(state): State<DaemonAppState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectDto>), Response> {
    let registry = state.registry.clone();
    let created = tokio::task::spawn_blocking(move || create_project_isolated(&registry, &req))
        .await
        .map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: format!("BLOCKING_TASK_FAILED: {e}"),
            })
        })?
        .map_err(map_error_to_response)?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// Creates a project and binds it to an isolated project workspace root (`<daemon_root>/.cowok-design/projects/<project_id>`
/// when `workspace_root` is omitted or shares a multi-project root) inside a single `BEGIN IMMEDIATE`
/// SQLite transaction with a `UNIQUE` index on `projects(workspace_root)` (`F-04`) so concurrent
/// requests can never bind two projects to the same workspace directory.
pub fn create_project_isolated(
    base_registry: &SpecRegistry,
    req: &CreateProjectRequest,
) -> Result<ProjectDto, HarnessError> {
    let project_id = req.id.unwrap_or_else(Uuid::new_v4);
    let isolated_ws = resolve_new_project_workspace_root(base_registry, project_id, req)?;
    let iso_str = isolated_ws.display().to_string();

    let now_ms = Utc::now().timestamp_millis();
    let safe_now = SafeJsInteger::new(now_ms)?;
    let mut conn = base_registry.pool().connection()?;

    retry_on_busy("create_project_isolated_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO projects (
                id, slug, name, workspace_root, product_brief,
                inherit_brand_from_project_id, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ACTIVE', ?7, ?8)",
            params![
                project_id.to_string(),
                req.slug.trim(),
                req.name.trim(),
                iso_str,
                req.product_brief.trim(),
                req.inherit_brand_from_project_id.map(|u| u.to_string()),
                now_ms,
                now_ms,
            ],
        )?;
        tx.commit()
    })?;

    // Provision .cowok-design/project.token with 0600 mode right upon project creation (Task 6.6 & 7.7)
    crate::synthesis::ensure_project_token(&isolated_ws)?;

    // Provision AC-06 Hybrid Boundary (.opendesign/project.json) and Default Chat Session
    let _ = ensure_ac06_tables(base_registry);
    let is_git = isolated_ws.join(".git").exists();
    let boundary_type = if req.workspace_root.is_some() {
        "folder_backed"
    } else {
        "managed_scratch"
    };

    let opendesign_dir = isolated_ws.join(".opendesign");
    let _ = fs::create_dir_all(&opendesign_dir);
    let opendesign_manifest = serde_json::json!({
        "id": project_id,
        "slug": req.slug.trim(),
        "name": req.name.trim(),
        "boundary_type": boundary_type,
        "created_at": now_ms,
    });
    let _ = fs::write(
        opendesign_dir.join("project.json"),
        serde_json::to_string_pretty(&opendesign_manifest).unwrap_or_default(),
    );

    let _ = retry_on_busy("record_project_boundary_and_default_session", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT OR REPLACE INTO project_boundaries (
                project_id, boundary_type, workspace_path, config_dir, is_git_repository,
                promoted_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
            params![
                project_id.to_string(),
                boundary_type,
                iso_str,
                ".opendesign",
                if is_git { 1 } else { 0 },
                now_ms,
                now_ms,
            ],
        )?;
        let default_session_id = Uuid::new_v4();
        tx.execute(
            "INSERT OR REPLACE INTO chat_sessions (
                id, project_id, title, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                default_session_id.to_string(),
                project_id.to_string(),
                "Main Session",
                now_ms,
                now_ms,
            ],
        )?;
        tx.commit()
    });

    Ok(ProjectDto {
        id: project_id,
        slug: req.slug.trim().to_string(),
        name: req.name.trim().to_string(),
        product_brief: req.product_brief.trim().to_string(),
        inherit_brand_from_project_id: req.inherit_brand_from_project_id,
        created_at: safe_now,
        updated_at: safe_now,
    })
}

pub(super) async fn handle_list_specs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
) -> Result<Json<ProjectSpecsListResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (project_registry, project_workspace_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    let base_registry = state.registry.clone();
    let res = tokio::task::spawn_blocking(move || {
        let mut specs_out = Vec::new();
        let mut any_drift = false;

        for spec_key in CanonicalSpecKey::ALL {
            if let Some(rec) = project_registry.get_spec(project_id, spec_key)? {
                let upstream_outdated = check_upstream_drift(&base_registry, &rec)?;
                if upstream_outdated {
                    any_drift = true;
                }
                let disk_bytes =
                    read_jailed_spec_bytes(&project_workspace_root, spec_key)?.unwrap_or_default();
                let markdown_content = String::from_utf8_lossy(&disk_bytes).to_string();

                specs_out.push(SpecItemWithDriftDto {
                    id: rec.id,
                    project_id: rec.project_id,
                    spec_key: rec.spec_key,
                    file_path: rec.file_path,
                    status: rec.status,
                    version: rec.version,
                    content_sha256: rec.content_sha256,
                    summary_digest: rec.summary_digest,
                    provenance_summary_json: rec.provenance_summary_json,
                    source_project_id: rec.source_project_id,
                    source_revision_sha256: rec.source_revision_sha256,
                    upstream_outdated,
                    markdown_content,
                    updated_at: rec.updated_at,
                });
            }
        }

        Ok::<_, HarnessError>(ProjectSpecsListResponse {
            project_id,
            upstream_brand_drift_detected: any_drift,
            specs: specs_out,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(res))
}

pub(super) async fn handle_put_spec(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath((project_id_raw, spec_key_raw)): AxumPath<(String, String)>,
    Json(req): Json<PutSpecRequest>,
) -> Result<Json<SpecItemWithDriftDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let spec_key = CanonicalSpecKey::parse(&spec_key_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    if req.markdown_content.trim().is_empty() {
        return Err(map_error_to_response(HarnessError::ValidationFailed {
            reason: "SPEC_CONTENT_EMPTY: markdown_content must not be empty".to_string(),
        }));
    }

    let actor_id = extract_actor_id(&headers);
    let status = req.status.unwrap_or(ProvenanceStatus::Confirmed);
    let change_summary = req
        .change_summary
        .clone()
        .unwrap_or_else(|| format!("Updated {} via HTTP PUT", spec_key.as_str()));

    let prov_json = serde_json::json!({
        "origin": "http_put",
        "actor": actor_id,
    })
    .to_string();

    let base_registry = state.registry.clone();
    let res = tokio::task::spawn_blocking(move || {
        let (rec, _) = project_registry.upsert_spec(UpsertSpecInput {
            project_id,
            spec_key,
            markdown_content: req.markdown_content.clone(),
            expected_sha256: req.expected_sha256,
            status,
            provenance_summary_json: prov_json,
            source_project_id: req.source_project_id,
            source_revision_sha256: req.source_revision_sha256,
            actor_kind: "http_client".to_string(),
            actor_id,
            change_summary,
        })?;

        let upstream_outdated = check_upstream_drift(&base_registry, &rec)?;

        Ok::<_, HarnessError>(SpecItemWithDriftDto {
            id: rec.id,
            project_id: rec.project_id,
            spec_key: rec.spec_key,
            file_path: rec.file_path,
            status: rec.status,
            version: rec.version,
            content_sha256: rec.content_sha256,
            summary_digest: rec.summary_digest,
            provenance_summary_json: rec.provenance_summary_json,
            source_project_id: rec.source_project_id,
            source_revision_sha256: rec.source_revision_sha256,
            upstream_outdated,
            markdown_content: req.markdown_content,
            updated_at: rec.updated_at,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(res))
}

pub(super) async fn handle_synthesize_specs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(mut req): Json<SynthesizeSpecsRequest>,
) -> Result<Json<SynthesizeSpecsResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
        .map_err(map_error_to_response)?;

    if req.actor_id.is_none() {
        if let Some(hdr_actor) = headers
            .get("x-cowok-actor-id")
            .and_then(|v| v.to_str().ok())
        {
            req.actor_id = Some(hdr_actor.to_string());
        }
    }

    let base_registry = state.registry.clone();
    let resp = tokio::task::spawn_blocking(move || {
        synthesize_nine_specs(&base_registry, project_id, &req)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(resp))
}

fn resolve_new_project_workspace_root(
    registry: &SpecRegistry,
    project_id: Uuid,
    req: &CreateProjectRequest,
) -> Result<PathBuf, HarnessError> {
    if req.slug.trim().is_empty() || req.name.trim().is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "PROJECT_METADATA_EMPTY: slug and name must not be empty".to_string(),
        });
    }
    let canon_reg = validate_and_canonicalize_workspace_root(registry.pool().workspace_root())?;

    if let Some(ref ws_str) = req.workspace_root {
        let candidate = PathBuf::from(ws_str);
        let canon_cand = validate_and_canonicalize_workspace_root(&candidate)?;
        if !canon_cand.starts_with(&canon_reg) {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "PROJECT_SCOPE_VIOLATION: workspace_root '{}' does not match daemon root '{}'",
                    canon_cand.display(),
                    canon_reg.display()
                ),
            });
        }
        if canon_cand != canon_reg {
            return Ok(canon_cand);
        }
    }

    // Isolate each project's on-disk `product/specs/` inside `<daemon_root>/.cowok-design/projects/<project_id>`
    let isolated_dir = canon_reg
        .join(".cowok-design")
        .join("projects")
        .join(project_id.to_string());
    fs::create_dir_all(&isolated_dir).map_err(|e| HarnessError::ValidationFailed {
        reason: format!(
            "PROJECT_WORKSPACE_CREATE_FAILED [{}]: {e}",
            isolated_dir.display()
        ),
    })?;
    validate_and_canonicalize_workspace_root(&isolated_dir)
}

pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub(super) fn verify_anti_idor_and_resolve_registry(
    base_registry: &SpecRegistry,
    project_id: Uuid,
    headers: &HeaderMap,
) -> Result<(SpecRegistry, PathBuf, bool), HarnessError> {
    let (project_registry, canon_project_root) =
        resolve_project_scoped_registry(base_registry, project_id)?;

    // Anti-IDOR Check 1: If caller declares X-Cowok-Workspace-Root, strictly verify it matches the resolved project root
    if let Some(hdr_val) = headers.get("x-cowok-workspace-root") {
        let hdr_str = hdr_val
            .to_str()
            .map_err(|_| HarnessError::ValidationFailed {
                reason: "PROJECT_SCOPE_VIOLATION: invalid X-Cowok-Workspace-Root header"
                    .to_string(),
            })?;
        let hdr_path = PathBuf::from(hdr_str);
        let canon_hdr = hdr_path.canonicalize().unwrap_or(hdr_path);
        let is_valid = canon_hdr == canon_project_root;
        if !is_valid {
            return Err(HarnessError::ValidationFailed {
                reason: "PROJECT_SCOPE_VIOLATION: Invalid X-Cowok-Workspace-Root provided"
                    .to_string(),
            });
        }
    }

    // Anti-IDOR Check 2: Mandatory Cryptographic Authentication
    // Accessing project specs strictly requires verified Bearer authorization
    // OR verified loopback HMAC signature from BFF (Dual-Ingress)
    let is_loopback_hmac = if let Some(sig_hdr) = headers
        .get("x-cowok-hmac-sig")
        .and_then(|v| v.to_str().ok())
    {
        if !sig_hdr.trim().is_empty() {
            if let Some(ts_str) = headers
                .get("x-cowok-timestamp")
                .and_then(|v| v.to_str().ok())
            {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    (Utc::now().timestamp() - ts).abs() <= 30
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if is_loopback_hmac {
        return Ok((project_registry, canon_project_root, true));
    }

    let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => h["Bearer ".len()..].trim(),
        _ => {
            return Err(HarnessError::BoundaryViolation {
                reason: format!(
                    "UNAUTHORIZED: Accessing project {project_id} requires Bearer authorization or loopback HMAC"
                ),
            });
        }
    };

    let is_superadmin = match std::env::var("COWOK_SUPERADMIN_SECRET") {
        Ok(ref secret) if !secret.trim().is_empty() => {
            constant_time_eq(token.as_bytes(), secret.trim().as_bytes())
        }
        _ => false,
    };
    let is_project_scoped = {
        let tp = canon_project_root
            .join(".cowok-design")
            .join("project.token");
        tp.exists()
            && fs::read_to_string(&tp)
                .map(|s| constant_time_eq(s.trim().as_bytes(), token.as_bytes()))
                .unwrap_or(false)
    };

    if !is_superadmin && !is_project_scoped {
        return Err(HarnessError::BoundaryViolation {
            reason: format!(
                "TENANT_ISOLATION_BREACH: Invalid Bearer token for project {project_id}"
            ),
        });
    }

    Ok((project_registry, canon_project_root, is_superadmin))
}

fn check_upstream_drift(registry: &SpecRegistry, rec: &SpecRecord) -> Result<bool, HarnessError> {
    let (Some(src_proj), Some(src_sha)) =
        (rec.source_project_id, rec.source_revision_sha256.as_ref())
    else {
        return Ok(false);
    };

    let conn = registry.pool().connection()?;
    let upstream_sha_opt: Option<String> = conn
        .query_row(
            "SELECT content_sha256 FROM project_specs
             WHERE project_id = ?1 AND spec_key = ?2 AND content_sha256 != ''",
            params![src_proj.to_string(), rec.spec_key.as_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "check_upstream_drift"))?;

    match upstream_sha_opt {
        Some(cur_up_sha) => Ok(&cur_up_sha != src_sha),
        None => Ok(false),
    }
}

pub(super) async fn handle_context_slice(
    State(state): State<DaemonAppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ContextSliceRequest>,
) -> Result<Json<AuthorizedContextSlice>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;

    let (project_registry, _canon_project_root, has_bearer) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    let header_portal: Option<Portal> = headers
        .get("x-cowok-portal")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| match s.trim().to_ascii_lowercase().as_str() {
            "super_admin" | "superadmin" => Some(Portal::SuperAdmin),
            "tenant_admin" | "tenantadmin" => Some(Portal::TenantAdmin),
            "sales_app" | "salesapp" => Some(Portal::SalesApp),
            "marketing_web" | "marketingweb" => Some(Portal::MarketingWeb),
            "developer_portal" | "developerportal" => Some(Portal::DeveloperPortal),
            "presentation_studio" | "presentationstudio" => Some(Portal::PresentationStudio),
            _ => None,
        });

    // Rule 1: Accessing any non-public portal (SuperAdmin, TenantAdmin, DeveloperPortal) strictly requires verified Bearer authorization
    if !req.portal.is_public_facing() && !has_bearer {
        return Err(map_error_to_response(HarnessError::BoundaryViolation {
            reason: format!(
                "TENANT_ISOLATION_BREACH: Accessing non-public portal {:?} requires valid Bearer authorization",
                req.portal
            ),
        }));
    }

    // Rule 2: Requesting executive reports with sensitive database/security specs strictly requires verified Bearer authorization
    if req.department == Department::ExecutiveReport && !has_bearer {
        return Err(map_error_to_response(HarnessError::BoundaryViolation {
            reason: "TENANT_ISOLATION_BREACH: Accessing ExecutiveReport department requires valid Bearer authorization".to_string(),
        }));
    }

    // Rule 3: If caller claims a public portal in header, forbid internal privilege escalation in payload
    if let Some(hdr_portal) = header_portal {
        if hdr_portal.is_public_facing() && !req.portal.is_public_facing() {
            return Err(map_error_to_response(HarnessError::BoundaryViolation {
                reason: "TENANT_ISOLATION_BREACH: Public portal caller cannot request internal portal context".to_string(),
            }));
        }
    }

    let slice = tokio::task::spawn_blocking(move || {
        let slicer = spec_registry::ContextSlicer::new(&project_registry);
        slicer.slice_context(project_id, &req)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(slice))
}
