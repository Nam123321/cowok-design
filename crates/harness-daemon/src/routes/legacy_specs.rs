// @story 01.3
use std::fs;
use std::path::PathBuf;

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectHttpRequest {
    pub id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub product_brief: String,
    pub workspace_root: Option<String>,
    pub inherit_brand_from_project_id: Option<Uuid>,
    #[serde(default)]
    pub managed_slug: Option<String>,
    #[serde(default)]
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListProjectsQuery {
    #[serde(default)]
    pub include_trashed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDto {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub product_brief: String,
    #[serde(default)]
    pub workspace_root: Option<String>,
    pub inherit_brand_from_project_id: Option<Uuid>,
    pub created_at: SafeJsInteger,
    pub updated_at: SafeJsInteger,
    #[serde(default)]
    pub project_code: Option<String>,
    #[serde(default)]
    pub provenance: Option<String>,
    #[serde(default)]
    pub trash_deleted_at: Option<SafeJsInteger>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub product_brief: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteProjectResponse {
    pub deleted: bool,
    pub id: Uuid,
    pub project_id: Uuid,
    pub trash_deleted_at: SafeJsInteger,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeleteSpecQuery {
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteSpecResponse {
    pub deleted: bool,
    pub spec_key: CanonicalSpecKey,
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

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDto> {
    let id_str: String = row.get(0)?;
    let slug: String = row.get(1)?;
    let name: String = row.get(2)?;
    let brief: String = row.get(3)?;
    let workspace_root: String = row.get(4)?;
    let inherit_str: Option<String> = row.get(5)?;
    let cr: i64 = row.get(6)?;
    let up: i64 = row.get(7)?;
    let project_code: Option<String> = row.get(8)?;
    let provenance: Option<String> = row.get(9)?;
    let trash_ms: Option<i64> = row.get(10)?;
    Ok(ProjectDto {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        slug,
        name,
        product_brief: brief,
        workspace_root: (!workspace_root.trim().is_empty()).then_some(workspace_root),
        inherit_brand_from_project_id: inherit_str.and_then(|s| Uuid::parse_str(&s).ok()),
        created_at: SafeJsInteger::new(cr).unwrap_or(SafeJsInteger::ZERO),
        updated_at: SafeJsInteger::new(up).unwrap_or(SafeJsInteger::ZERO),
        project_code,
        provenance: provenance.or_else(|| Some("scratch".to_string())),
        trash_deleted_at: trash_ms.and_then(|ms| SafeJsInteger::new(ms).ok()),
    })
}

fn list_projects_blocking(
    registry: &SpecRegistry,
    filter_tenant: &str,
    is_superadmin: bool,
    include_trashed: bool,
) -> Result<Vec<ProjectDto>, HarnessError> {
    let conn = registry.pool().connection()?;
    let sql = match (is_superadmin, include_trashed) {
        (true, true) => {
            "SELECT id, slug, name, product_brief, workspace_root, inherit_brand_from_project_id,
                    created_at, updated_at, project_code, provenance, trash_deleted_at
             FROM projects ORDER BY updated_at DESC"
        }
        (true, false) => {
            "SELECT id, slug, name, product_brief, workspace_root, inherit_brand_from_project_id,
                    created_at, updated_at, project_code, provenance, trash_deleted_at
             FROM projects WHERE trash_deleted_at IS NULL ORDER BY updated_at DESC"
        }
        (false, true) => {
            "SELECT id, slug, name, product_brief, workspace_root, inherit_brand_from_project_id,
                    created_at, updated_at, project_code, provenance, trash_deleted_at
             FROM projects WHERE tenant_id = ?1 ORDER BY updated_at DESC"
        }
        (false, false) => {
            "SELECT id, slug, name, product_brief, workspace_root, inherit_brand_from_project_id,
                    created_at, updated_at, project_code, provenance, trash_deleted_at
             FROM projects WHERE trash_deleted_at IS NULL AND tenant_id = ?1 ORDER BY updated_at DESC"
        }
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| map_sqlite_error(e, "prepare_list_projects"))?;
    let rows = if is_superadmin {
        stmt.query_map([], map_project_row)
    } else {
        stmt.query_map(params![filter_tenant], map_project_row)
    }
    .map_err(|e| map_sqlite_error(e, "query_list_projects"))?;
    rows.map(|r| r.map_err(|e| map_sqlite_error(e, "row_list_projects")))
        .collect()
}

pub(super) async fn handle_list_projects(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<Vec<ProjectDto>>, Response> {
    let registry = state.registry.clone();
    let include_trashed = query.include_trashed.unwrap_or(false);
    let (filter_tenant, is_superadmin) = match caller {
        Some(Extension(ref verified)) => {
            let trimmed = verified.principal.tenant_id.trim();
            let tenant = if trimmed.is_empty() {
                "local-default"
            } else {
                trimmed
            };
            (tenant.to_string(), verified.principal.role == "admin")
        }
        None => ("local-default".to_string(), false),
    };
    let projects = tokio::task::spawn_blocking(move || {
        list_projects_blocking(&registry, &filter_tenant, is_superadmin, include_trashed)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;
    Ok(Json(projects))
}

pub(super) async fn handle_create_project(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    Json(req): Json<CreateProjectHttpRequest>,
) -> Result<(StatusCode, Json<ProjectDto>), Response> {
    let registry = state.registry.clone();
    let tenant_id = caller
        .map(|Extension(v)| v.principal.tenant_id)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "local-default".to_string());
    let core_req = CreateProjectRequest {
        id: req.id,
        slug: req.slug,
        name: req.name,
        product_brief: req.product_brief,
        workspace_root: req.workspace_root,
        inherit_brand_from_project_id: req.inherit_brand_from_project_id,
    };
    let managed_slug = req.managed_slug;
    let provenance_override = req.provenance;
    let created = tokio::task::spawn_blocking(move || {
        create_project_isolated_with_options(
            &registry,
            &core_req,
            &tenant_id,
            managed_slug.as_deref(),
            provenance_override.as_deref(),
        )
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok((StatusCode::CREATED, Json(created)))
}

type ProjectUpdateRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    i64,
    Option<String>,
    Option<String>,
);

fn update_project_blocking(
    registry: &SpecRegistry,
    project_id: Uuid,
    req: &UpdateProjectRequest,
) -> Result<ProjectDto, HarnessError> {
    let now_ms = Utc::now().timestamp_millis();
    let mut conn = registry.pool().connection()?;
    let row: Option<ProjectUpdateRow> = conn
        .query_row(
            "SELECT slug, name, product_brief, workspace_root, inherit_brand_from_project_id,
                    created_at, project_code, provenance
             FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![project_id.to_string()],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "load_project_for_update"))?;
    let Some((
        slug,
        cur_name,
        cur_brief,
        workspace_root,
        inherit_str,
        created_at_ms,
        project_code,
        provenance,
    )) = row
    else {
        return Err(HarnessError::ProjectNotFound {
            project_id: project_id.to_string(),
        });
    };
    let next_name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or(cur_name);
    let next_brief = req
        .product_brief
        .as_deref()
        .map(str::trim)
        .map(ToOwned::to_owned)
        .unwrap_or(cur_brief);
    retry_on_busy("update_project_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE projects SET name = ?1, product_brief = ?2, updated_at = ?3 WHERE id = ?4 AND trash_deleted_at IS NULL",
            params![next_name, next_brief, now_ms, project_id.to_string()],
        )?;
        tx.commit()
    })?;
    Ok(ProjectDto {
        id: project_id,
        slug,
        name: next_name,
        product_brief: next_brief,
        workspace_root: (!workspace_root.trim().is_empty()).then_some(workspace_root),
        inherit_brand_from_project_id: inherit_str.and_then(|s| Uuid::parse_str(&s).ok()),
        created_at: SafeJsInteger::new(created_at_ms).unwrap_or(SafeJsInteger::ZERO),
        updated_at: SafeJsInteger::new(now_ms).unwrap_or(SafeJsInteger::ZERO),
        project_code,
        provenance: provenance.or_else(|| Some("scratch".to_string())),
        trash_deleted_at: None,
    })
}

pub(super) async fn handle_update_project(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    AxumPath(id_raw): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectDto>, Response> {
    let project_id = parse_project_uuid(&id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;
    let registry = state.registry.clone();
    let updated =
        tokio::task::spawn_blocking(move || update_project_blocking(&registry, project_id, &req))
            .await
            .map_err(|e| {
                map_error_to_response(HarnessError::ValidationFailed {
                    reason: format!("BLOCKING_TASK_FAILED: {e}"),
                })
            })?
            .map_err(map_error_to_response)?;
    Ok(Json(updated))
}

fn soft_delete_project_blocking(
    registry: &SpecRegistry,
    project_id: Uuid,
) -> Result<(bool, i64), HarnessError> {
    let now_ms = Utc::now().timestamp_millis();
    let mut conn = registry.pool().connection()?;
    retry_on_busy("soft_delete_project_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE projects SET trash_deleted_at = ?1, updated_at = ?1, status = 'DELETED'
             WHERE id = ?2 AND trash_deleted_at IS NULL",
            params![now_ms, project_id.to_string()],
        )?;
        let effective_ts: i64 = if changed > 0 {
            now_ms
        } else {
            tx.query_row(
                "SELECT COALESCE(trash_deleted_at, ?1) FROM projects WHERE id = ?2",
                params![now_ms, project_id.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(now_ms)
        };
        tx.commit()?;
        Ok::<(bool, i64), rusqlite::Error>((changed > 0, effective_ts))
    })
}

pub(super) async fn handle_delete_project(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    AxumPath(id_raw): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<DeleteProjectResponse>, Response> {
    let project_id = parse_project_uuid(&id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller_allow_trashed(
        &state.registry,
        project_id,
        &headers,
        caller.as_deref(),
    )
    .map_err(map_error_to_response)?;
    let registry = state.registry.clone();
    let (deleted, trash_deleted_at) =
        tokio::task::spawn_blocking(move || soft_delete_project_blocking(&registry, project_id))
            .await
            .map_err(|e| {
                map_error_to_response(HarnessError::ValidationFailed {
                    reason: format!("BLOCKING_TASK_FAILED: {e}"),
                })
            })?
            .map_err(map_error_to_response)?;
    let safe_trash = SafeJsInteger::new(trash_deleted_at).unwrap_or(SafeJsInteger::ZERO);
    Ok(Json(DeleteProjectResponse {
        deleted,
        id: project_id,
        project_id,
        trash_deleted_at: safe_trash,
        retention_days: 30,
    }))
}

/// Creates a project and binds it to an isolated project workspace root (`<daemon_root>/.cowok-design/projects/<project_id>`
/// when `workspace_root` is omitted or shares a multi-project root) inside a single `BEGIN IMMEDIATE`
/// SQLite transaction with a `UNIQUE` index on `projects(workspace_root)` (`F-04`) so concurrent
/// requests can never bind two projects to the same workspace directory.
pub fn create_project_isolated(
    base_registry: &SpecRegistry,
    req: &CreateProjectRequest,
) -> Result<ProjectDto, HarnessError> {
    create_project_isolated_with_tenant(base_registry, req, "local-default")
}

pub fn create_project_isolated_with_tenant(
    base_registry: &SpecRegistry,
    req: &CreateProjectRequest,
    tenant_id: &str,
) -> Result<ProjectDto, HarnessError> {
    create_project_isolated_with_options(base_registry, req, tenant_id, None, None)
}

struct InsertProjectRowParams<'a> {
    project_id: Uuid,
    clean_tenant: &'a str,
    req: &'a CreateProjectRequest,
    iso_str: &'a str,
    project_code: &'a str,
    provenance_str: &'a str,
    now_ms: i64,
}

fn insert_project_row_tx(
    conn: &mut rusqlite::Connection,
    p: InsertProjectRowParams<'_>,
) -> Result<(), HarnessError> {
    retry_on_busy("create_project_isolated_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO projects (
                id, tenant_id, slug, name, workspace_root, product_brief,
                inherit_brand_from_project_id, project_code, provenance, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'ACTIVE', ?10, ?11)",
            params![
                p.project_id.to_string(),
                p.clean_tenant,
                p.req.slug.trim(),
                p.req.name.trim(),
                p.iso_str,
                p.req.product_brief.trim(),
                p.req.inherit_brand_from_project_id.map(|u| u.to_string()),
                p.project_code,
                p.provenance_str,
                p.now_ms,
                p.now_ms,
            ],
        )?;
        tx.commit()
    })
}

fn write_project_boundary_and_session(
    base_registry: &SpecRegistry,
    conn: &mut rusqlite::Connection,
    project_id: Uuid,
    isolated_ws: &std::path::Path,
    req: &CreateProjectRequest,
    boundary_type: &str,
    now_ms: i64,
) -> Result<(), HarnessError> {
    let _ = ensure_ac06_tables(base_registry);
    let iso_str = isolated_ws.display().to_string();
    let ws_fd = super::fs_jail::open_dir_abs_nofollow(isolated_ws)?;
    let is_git =
        super::fs_jail::open_child_dir_fd_nofollow(&ws_fd, std::ffi::OsStr::new(".git"))?.is_some();
    let cowok_fd =
        super::fs_jail::ensure_child_dir_fd(&ws_fd, std::ffi::OsStr::new(".cowok-design"), 0o700)?;
    let manifest = serde_json::json!({
        "id": project_id, "slug": req.slug.trim(), "name": req.name.trim(),
        "boundary_type": boundary_type, "created_at": now_ms,
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap_or_default();
    super::fs_jail::write_file_in_dir_fd(
        &cowok_fd,
        std::ffi::OsStr::new("project.json"),
        &manifest_bytes,
        0o600,
    )?;
    retry_on_busy("record_project_boundary_and_default_session", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT OR REPLACE INTO project_boundaries (
                project_id, boundary_type, workspace_path, config_dir, is_git_repository,
                promoted_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, '.cowok-design', ?4, NULL, ?5, ?6)",
            params![
                project_id.to_string(),
                boundary_type,
                iso_str,
                if is_git { 1 } else { 0 },
                now_ms,
                now_ms
            ],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO chat_sessions (id, project_id, title, created_at, updated_at)
             VALUES (?1, ?2, 'Main Session', ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                project_id.to_string(),
                now_ms,
                now_ms
            ],
        )?;
        tx.commit()
    })?;
    Ok(())
}

pub(crate) fn create_project_isolated_with_options(
    base_registry: &SpecRegistry,
    req: &CreateProjectRequest,
    tenant_id: &str,
    managed_slug: Option<&str>,
    provenance_override: Option<&str>,
) -> Result<ProjectDto, HarnessError> {
    let project_id = req.id.unwrap_or_else(Uuid::new_v4);
    let (isolated_ws, resolved_provenance, boundary_type) =
        super::project_trash::provision_project_workspace_root(
            base_registry,
            project_id,
            &req.slug,
            &req.name,
            req.workspace_root.as_deref(),
            managed_slug,
        )?;
    if let Some(ov) = provenance_override.map(str::trim).filter(|s| !s.is_empty()) {
        let norm = ov.replace('_', "-");
        if (resolved_provenance == "scratch" && norm != "scratch")
            || (resolved_provenance != "scratch" && norm == "scratch")
        {
            return Err(HarnessError::ValidationFailed {
                reason: format!(
                    "PROVENANCE_MISMATCH: Client provenance '{norm}' conflicts with resolved workspace boundary '{resolved_provenance}'"
                ),
            });
        }
    }
    let provenance_str = resolved_provenance.to_string();
    let project_code = format!(
        "CWK-{}",
        project_id.simple().to_string()[..6].to_uppercase()
    );
    let iso_str = isolated_ws.display().to_string();
    let clean_tenant = if tenant_id.trim().is_empty() {
        "local-default"
    } else {
        tenant_id.trim()
    };
    let now_ms = Utc::now().timestamp_millis();
    let safe_now = SafeJsInteger::new(now_ms)?;
    let mut conn = base_registry.pool().connection()?;
    insert_project_row_tx(
        &mut conn,
        InsertProjectRowParams {
            project_id,
            clean_tenant,
            req,
            iso_str: &iso_str,
            project_code: &project_code,
            provenance_str: &provenance_str,
            now_ms,
        },
    )?;
    if let Err(err) = crate::synthesis::ensure_project_token(&isolated_ws).and_then(|_| {
        write_project_boundary_and_session(
            base_registry,
            &mut conn,
            project_id,
            &isolated_ws,
            req,
            boundary_type,
            now_ms,
        )
    }) {
        let _ = conn.execute(
            "DELETE FROM projects WHERE id = ?1",
            params![project_id.to_string()],
        );
        return Err(err);
    }
    Ok(ProjectDto {
        id: project_id,
        slug: req.slug.trim().to_string(),
        name: req.name.trim().to_string(),
        product_brief: req.product_brief.trim().to_string(),
        workspace_root: Some(iso_str),
        inherit_brand_from_project_id: req.inherit_brand_from_project_id,
        created_at: safe_now,
        updated_at: safe_now,
        project_code: Some(project_code),
        provenance: Some(provenance_str),
        trash_deleted_at: None,
    })
}

pub(super) async fn handle_list_specs(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
) -> Result<Json<ProjectSpecsListResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let (project_registry, project_workspace_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
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
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath((project_id_raw, spec_key_raw)): AxumPath<(String, String)>,
    Json(req): Json<PutSpecRequest>,
) -> Result<Json<SpecItemWithDriftDto>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let spec_key = CanonicalSpecKey::parse(&spec_key_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
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

pub(super) async fn handle_delete_spec(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath((project_id_raw, spec_key_raw)): AxumPath<(String, String)>,
    Query(query): Query<DeleteSpecQuery>,
) -> Result<Json<DeleteSpecResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let spec_key = CanonicalSpecKey::parse(&spec_key_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let actor_id = extract_actor_id(&headers);
    let expected_sha = query.expected_sha256.or_else(|| {
        headers
            .get("if-match")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches('"').trim().to_string())
            .filter(|s| !s.is_empty())
    });

    let _outcome = tokio::task::spawn_blocking(move || {
        project_registry.delete_spec_checked(
            project_id,
            spec_key,
            expected_sha.as_deref(),
            &actor_id,
        )
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(DeleteSpecResponse {
        deleted: true,
        spec_key,
    }))
}

pub(super) async fn handle_synthesize_specs(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    headers: HeaderMap,
    AxumPath(project_id_raw): AxumPath<String>,
    Json(mut req): Json<SynthesizeSpecsRequest>,
) -> Result<Json<SynthesizeSpecsResponse>, Response> {
    let project_id = parse_project_uuid(&project_id_raw).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
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

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub(crate) fn verify_anti_idor_with_caller(
    base_registry: &SpecRegistry,
    project_id: Uuid,
    headers: &HeaderMap,
    caller: Option<&crate::daemon_auth::VerifiedLegacyIngress>,
) -> Result<(SpecRegistry, PathBuf, bool), HarnessError> {
    verify_anti_idor_with_caller_inner(base_registry, project_id, headers, caller, false)
}

pub(crate) fn verify_anti_idor_with_caller_allow_trashed(
    base_registry: &SpecRegistry,
    project_id: Uuid,
    headers: &HeaderMap,
    caller: Option<&crate::daemon_auth::VerifiedLegacyIngress>,
) -> Result<(SpecRegistry, PathBuf, bool), HarnessError> {
    verify_anti_idor_with_caller_inner(base_registry, project_id, headers, caller, true)
}

fn verify_anti_idor_with_caller_inner(
    base_registry: &SpecRegistry,
    project_id: Uuid,
    headers: &HeaderMap,
    caller: Option<&crate::daemon_auth::VerifiedLegacyIngress>,
    allow_trashed: bool,
) -> Result<(SpecRegistry, PathBuf, bool), HarnessError> {
    let conn = base_registry.pool().connection()?;
    let meta_sql = if allow_trashed {
        "SELECT tenant_id, workspace_root FROM projects WHERE id = ?1"
    } else {
        "SELECT tenant_id, workspace_root FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL"
    };
    let (project_tenant, project_workspace_root): (String, String) = conn
        .query_row(meta_sql, params![project_id.to_string()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(|e| map_sqlite_error(e, "anti_idor_project_tenant"))?
        .ok_or(HarnessError::ProjectNotFound {
            project_id: project_id.to_string(),
        })?;

    // Check caller auth & tenant BEFORE any workspace dir or project.token disk writes
    let mut is_superadmin = false;
    let mut bearer_token: Option<&str> = None;
    if let Some(verified) = caller {
        match verified.kind {
            crate::daemon_auth::LegacyAuthKind::HmacV2
            | crate::daemon_auth::LegacyAuthKind::SessionToken => {
                is_superadmin = verified.principal.role == "admin";
                if !is_superadmin && verified.principal.tenant_id.trim() != project_tenant.trim() {
                    return Err(HarnessError::BoundaryViolation {
                        reason: format!(
                            "TENANT_ISOLATION_BREACH: Tenant '{}' cannot access project {project_id} owned by '{}'",
                            verified.principal.tenant_id, project_tenant
                        ),
                    });
                }
            }
            crate::daemon_auth::LegacyAuthKind::Bearer => {
                let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
                bearer_token = match auth_header {
                    Some(h) if h.starts_with("Bearer ") => Some(h["Bearer ".len()..].trim()),
                    _ => {
                        return Err(HarnessError::BoundaryViolation {
                            reason: format!(
                                "UNAUTHORIZED: Accessing project {project_id} requires Bearer authorization, session token, or verified loopback HMAC"
                            ),
                        });
                    }
                };
            }
        }
    } else {
        let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
        bearer_token = match auth_header {
            Some(h) if h.starts_with("Bearer ") => Some(h["Bearer ".len()..].trim()),
            _ => {
                return Err(HarnessError::BoundaryViolation {
                    reason: format!(
                        "UNAUTHORIZED: Accessing project {project_id} requires Bearer authorization, session token, or verified loopback HMAC"
                    ),
                });
            }
        };
    }

    if let Some(token) = bearer_token {
        is_superadmin = match std::env::var("COWOK_SUPERADMIN_SECRET") {
            Ok(ref secret) if !secret.trim().is_empty() => {
                constant_time_eq(token.as_bytes(), secret.trim().as_bytes())
            }
            _ => false,
        };
    }

    // Validate X-Cowok-Workspace-Root & non-superadmin Bearer token BEFORE resolve_project_scoped_registry creates any directories or project.token on disk.
    let daemon_canon_root = crate::lockfile::validate_and_canonicalize_workspace_root(
        base_registry.pool().workspace_root(),
    )?;
    let raw_proj_path = if project_workspace_root == "." || project_workspace_root.trim().is_empty()
    {
        daemon_canon_root.clone()
    } else {
        PathBuf::from(&project_workspace_root)
    };
    let existing_canon_root = if fs::symlink_metadata(&raw_proj_path).is_ok() {
        Some(crate::lockfile::validate_and_canonicalize_workspace_root(
            &raw_proj_path,
        )?)
    } else {
        None
    };

    // Anti-IDOR Check 1: If caller declares X-Cowok-Workspace-Root, fail-closed on symlinks/non-canonical/mismatched paths BEFORE disk writes
    let prevalidated_hdr_canon = if let Some(hdr_val) = headers.get("x-cowok-workspace-root") {
        let hdr_str = hdr_val
            .to_str()
            .map_err(|_| HarnessError::ValidationFailed {
                reason: "PROJECT_SCOPE_VIOLATION: invalid X-Cowok-Workspace-Root header"
                    .to_string(),
            })?;
        let hdr_path = PathBuf::from(hdr_str);
        if let Ok(meta) = fs::symlink_metadata(&hdr_path) {
            if meta.file_type().is_symlink() {
                return Err(HarnessError::ValidationFailed {
                    reason: "PROJECT_SCOPE_VIOLATION: symlink X-Cowok-Workspace-Root rejected"
                        .to_string(),
                });
            }
        }
        let canon_hdr = hdr_path
            .canonicalize()
            .map_err(|_| HarnessError::ValidationFailed {
                reason: "PROJECT_SCOPE_VIOLATION: invalid or non-canonical X-Cowok-Workspace-Root"
                    .to_string(),
            })?;
        match existing_canon_root.as_ref() {
            Some(existing_canon) if canon_hdr == *existing_canon => {}
            _ => {
                return Err(HarnessError::ValidationFailed {
                    reason: "PROJECT_SCOPE_VIOLATION: Invalid X-Cowok-Workspace-Root provided"
                        .to_string(),
                });
            }
        }
        Some(canon_hdr)
    } else {
        None
    };

    // Non-superadmin Bearer callers MUST present a matching token against an existing non-symlink .cowok-design/project.token BEFORE any directory, SQLite DB, or project.token creation.
    if let Some(token) = bearer_token {
        if !is_superadmin {
            let is_project_scoped = existing_canon_root
                .as_ref()
                .map(|existing_root| read_verified_project_token(existing_root, token))
                .unwrap_or(false);
            if !is_project_scoped {
                return Err(HarnessError::BoundaryViolation {
                    reason: format!(
                        "TENANT_ISOLATION_BREACH: Invalid Bearer token for project {project_id}"
                    ),
                });
            }
        }
    }

    let (project_registry, canon_project_root) = if allow_trashed {
        crate::synthesis::resolve_project_scoped_registry_including_trashed(
            base_registry,
            project_id,
        )?
    } else {
        resolve_project_scoped_registry(base_registry, project_id)?
    };

    if let Some(canon_hdr) = prevalidated_hdr_canon {
        if canon_hdr != canon_project_root {
            return Err(HarnessError::ValidationFailed {
                reason: "PROJECT_SCOPE_VIOLATION: Invalid X-Cowok-Workspace-Root provided"
                    .to_string(),
            });
        }
    }

    Ok((project_registry, canon_project_root, is_superadmin))
}

fn read_verified_project_token(existing_canon_root: &std::path::Path, candidate: &str) -> bool {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::io::Read;
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

        let Ok(ws_fd) = super::fs_jail::open_dir_abs_nofollow(existing_canon_root) else {
            return false;
        };
        let Ok(Some(dot_fd)) = super::fs_jail::open_child_dir_fd_nofollow(
            &ws_fd,
            std::ffi::OsStr::new(".cowok-design"),
        ) else {
            return false;
        };
        let Ok(c_name) = CString::new("project.token") else {
            return false;
        };
        let fd = unsafe {
            libc::openat(
                dot_fd.as_raw_fd(),
                c_name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return false;
        }
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut file = fs::File::from(owned);
        let Ok(meta) = file.metadata() else {
            return false;
        };
        if !meta.is_file() {
            return false;
        }
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            return false;
        }
        constant_time_eq(buf.trim().as_bytes(), candidate.as_bytes())
    }
    #[cfg(not(unix))]
    {
        let dot_dir = existing_canon_root.join(".cowok-design");
        let Ok(dot_meta) = fs::symlink_metadata(&dot_dir) else {
            return false;
        };
        if dot_meta.file_type().is_symlink() || !dot_meta.is_dir() {
            return false;
        }
        let tp = dot_dir.join("project.token");
        let Ok(tp_meta) = fs::symlink_metadata(&tp) else {
            return false;
        };
        if tp_meta.file_type().is_symlink() || !tp_meta.is_file() {
            return false;
        }
        fs::read_to_string(&tp)
            .map(|s| constant_time_eq(s.trim().as_bytes(), candidate.as_bytes()))
            .unwrap_or(false)
    }
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
    caller: Option<Extension<crate::daemon_auth::VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ContextSliceRequest>,
) -> Result<Json<AuthorizedContextSlice>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;

    let (project_registry, _canon_project_root, has_bearer) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
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
