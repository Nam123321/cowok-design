// @story 04.2
// @implements AC-03 AC-06
use std::fs;
use std::path::{Path, PathBuf};

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use chrono::Utc;
use domain::{HarnessError, SafeJsInteger};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::{map_sqlite_error, retry_on_busy, SpecRegistry};
use uuid::Uuid;

use super::fs_jail;
use super::legacy_specs::{verify_anti_idor_with_caller_allow_trashed, ProjectDto};
use super::{map_error_to_response, parse_project_uuid};
use crate::daemon_auth::VerifiedLegacyIngress;
use crate::lockfile::validate_and_canonicalize_workspace_root;
use crate::routes::DaemonAppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreProjectResponse {
    pub restored: bool,
    pub project: ProjectDto,
    pub renamed_slug: Option<String>,
}

struct TrashedProjectRow {
    slug: String,
    name: String,
    product_brief: String,
    workspace_root: String,
    inherit_str: Option<String>,
    created_at: i64,
    project_code: Option<String>,
    provenance: String,
    trash_deleted_at: Option<i64>,
}

pub(crate) fn provision_project_workspace_root(
    registry: &SpecRegistry,
    project_id: Uuid,
    slug: &str,
    name: &str,
    workspace_root: Option<&str>,
    managed_slug: Option<&str>,
) -> Result<(PathBuf, &'static str, &'static str), HarnessError> {
    if slug.trim().is_empty() || name.trim().is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "PROJECT_METADATA_EMPTY: slug and name must not be empty".to_string(),
        });
    }
    let canon_reg = validate_and_canonicalize_workspace_root(registry.pool().workspace_root())?;
    let managed_slug_trimmed = managed_slug.map(str::trim).filter(|s| !s.is_empty());
    if let Some(ms) = managed_slug_trimmed {
        fs_jail::validate_managed_slug(ms)?;
    }
    if let Some(ws_str) = workspace_root.map(str::trim).filter(|s| !s.is_empty()) {
        let anchor = fs_jail::inspect_path_jail(Path::new(ws_str))?;
        fs_jail::ensure_writable_target_not_home(&anchor.canonical_path)?;
        if anchor.canonical_path != canon_reg {
            let prov = fs_jail::provision_dir_zero_toctou(&anchor.canonical_path)?;
            let is_managed = managed_slug_trimmed.is_some_and(|ms| {
                fs_jail::default_managed_root()
                    .ok()
                    .and_then(|mr| {
                        fs::canonicalize(&mr)
                            .or(Ok::<_, std::io::Error>(mr))
                            .ok()
                            .map(|cm| cm.join(ms))
                    })
                    .is_some_and(|exp| prov.canonical_path == exp)
            });
            let provenance = if is_managed {
                "managed"
            } else {
                "folder-backed"
            };
            return Ok((prov.canonical_path, provenance, "folder_backed"));
        }
    }
    if let Some(ms) = managed_slug_trimmed {
        let target = fs_jail::default_managed_root()?.join(ms);
        fs_jail::ensure_writable_target_not_home(&target)?;
        let anchor = fs_jail::provision_dir_zero_toctou(&target)?;
        return Ok((anchor.canonical_path, "managed", "folder_backed"));
    }
    provision_scratch_dir(&canon_reg, project_id)
}

fn provision_scratch_dir(
    canon_reg: &Path,
    project_id: Uuid,
) -> Result<(PathBuf, &'static str, &'static str), HarnessError> {
    let root_fd = fs_jail::open_dir_abs_nofollow(canon_reg)?;
    let cowok_fd =
        fs_jail::ensure_child_dir_fd(&root_fd, std::ffi::OsStr::new(".cowok-design"), 0o700)?;
    let projs_fd =
        fs_jail::ensure_child_dir_fd(&cowok_fd, std::ffi::OsStr::new("projects"), 0o700)?;
    let id_str = project_id.to_string();
    let _proj_fd = fs_jail::ensure_child_dir_fd(&projs_fd, std::ffi::OsStr::new(&id_str), 0o700)?;
    let isolated_dir = canon_reg
        .join(".cowok-design")
        .join("projects")
        .join(id_str);
    let canon = validate_and_canonicalize_workspace_root(&isolated_dir)?;
    Ok((canon, "scratch", "managed_scratch"))
}

fn load_trashed_project(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
) -> Result<TrashedProjectRow, HarnessError> {
    tx.query_row(
        "SELECT slug, name, product_brief, workspace_root, inherit_brand_from_project_id,
                created_at, project_code, provenance, trash_deleted_at
         FROM projects WHERE id = ?1",
        params![project_id.to_string()],
        |r| {
            Ok(TrashedProjectRow {
                slug: r.get(0)?,
                name: r.get(1)?,
                product_brief: r.get(2)?,
                workspace_root: r.get(3)?,
                inherit_str: r.get(4)?,
                created_at: r.get(5)?,
                project_code: r.get(6)?,
                provenance: r
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "scratch".to_string()),
                trash_deleted_at: r.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "query_restore_project"))?
    .ok_or_else(|| HarnessError::ProjectNotFound {
        project_id: project_id.to_string(),
    })
}

fn ensure_no_active_workspace_conflict(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    workspace_root: &str,
) -> Result<(), HarnessError> {
    let ws_conflict: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM projects
             WHERE workspace_root = ?1 AND trash_deleted_at IS NULL AND id != ?2",
            params![workspace_root, project_id.to_string()],
            |r| r.get(0),
        )
        .map_err(|e| map_sqlite_error(e, "check_restore_ws_conflict"))?;
    if ws_conflict > 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!(
                "WORKSPACE_ROOT_CONFLICT: Another active project claims workspace '{workspace_root}'"
            ),
        });
    }
    Ok(())
}

fn resolve_unique_restore_slug(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    orig_slug: &str,
    now_ms: i64,
) -> Result<(String, Option<String>), HarnessError> {
    let is_taken = |candidate: &str| -> Result<bool, HarnessError> {
        let cnt: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM projects
                 WHERE slug = ?1 AND trash_deleted_at IS NULL AND id != ?2",
                params![candidate, project_id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| map_sqlite_error(e, "check_restore_slug_conflict"))?;
        Ok(cnt > 0)
    };
    if !is_taken(orig_slug)? {
        return Ok((orig_slug.to_string(), None));
    }
    let short_ts = (now_ms / 1000).rem_euclid(100_000);
    for attempt in 0..100 {
        let candidate = if attempt == 0 {
            format!("{orig_slug}-restored-{short_ts}")
        } else {
            format!("{orig_slug}-restored-{short_ts}-{attempt}")
        };
        if !is_taken(&candidate)? {
            return Ok((candidate.clone(), Some(candidate)));
        }
    }
    Err(HarnessError::ValidationFailed {
        reason: format!("RESTORE_SLUG_EXHAUSTED: Unable to allocate unique slug for '{orig_slug}'"),
    })
}

fn restore_project_in_tx(
    tx: &rusqlite::Transaction<'_>,
    project_id: Uuid,
    now_ms: i64,
) -> Result<RestoreProjectResponse, HarnessError> {
    let row = load_trashed_project(tx, project_id)?;
    if row.trash_deleted_at.is_none() {
        return Err(HarnessError::ValidationFailed {
            reason: "PROJECT_NOT_TRASHED: Project is not in the Trash Journal".to_string(),
        });
    }
    let boundary_type: Option<String> = tx
        .query_row(
            "SELECT boundary_type FROM project_boundaries WHERE project_id = ?1",
            params![project_id.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "load_restore_boundary_type"))?;
    let expected_scratch_suffix = format!("/.cowok-design/projects/{project_id}");
    let is_verified_managed_scratch = boundary_type.as_deref() == Some("managed_scratch")
        && row.workspace_root.ends_with(&expected_scratch_suffix);
    if !is_verified_managed_scratch && !row.workspace_root.trim().is_empty() {
        let anchor = fs_jail::inspect_path_jail(Path::new(&row.workspace_root))?;
        fs_jail::ensure_writable_target_not_home(&anchor.canonical_path)?;
    }
    ensure_no_active_workspace_conflict(tx, project_id, &row.workspace_root)?;
    let (next_slug, renamed_slug) = resolve_unique_restore_slug(tx, project_id, &row.slug, now_ms)?;
    tx.execute(
        "UPDATE projects
         SET slug = ?1, trash_deleted_at = NULL, status = 'ACTIVE', updated_at = ?2
         WHERE id = ?3",
        params![next_slug, now_ms, project_id.to_string()],
    )
    .map_err(|e| map_sqlite_error(e, "update_restore_project"))?;
    let ws_opt = (!row.workspace_root.trim().is_empty()).then_some(row.workspace_root);
    Ok(RestoreProjectResponse {
        restored: true,
        project: ProjectDto {
            id: project_id,
            slug: next_slug,
            name: row.name,
            product_brief: row.product_brief,
            workspace_root: ws_opt,
            inherit_brand_from_project_id: row
                .inherit_str
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok()),
            created_at: SafeJsInteger::new(row.created_at).unwrap_or(SafeJsInteger::ZERO),
            updated_at: SafeJsInteger::new(now_ms).unwrap_or(SafeJsInteger::ZERO),
            project_code: row.project_code,
            provenance: Some(row.provenance),
            trash_deleted_at: None,
        },
        renamed_slug,
    })
}

pub(super) async fn handle_restore_project(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id_raw): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<RestoreProjectResponse>, Response> {
    let project_id = parse_project_uuid(&id_raw).map_err(map_error_to_response)?;
    verify_anti_idor_with_caller_allow_trashed(
        &state.registry,
        project_id,
        &headers,
        caller.as_deref(),
    )
    .map_err(map_error_to_response)?;
    let registry = state.registry.clone();
    let res = tokio::task::spawn_blocking(move || {
        let now_ms = Utc::now().timestamp_millis();
        let mut conn = registry.pool().connection()?;
        let mut domain_err: Option<HarnessError> = None;
        let out = retry_on_busy("restore_project_tx", || {
            domain_err = None;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            match restore_project_in_tx(&tx, project_id, now_ms) {
                Ok(restored) => {
                    tx.commit()?;
                    Ok(Some(restored))
                }
                Err(err) => {
                    domain_err = Some(err);
                    Ok(None)
                }
            }
        })?;
        if let Some(err) = domain_err {
            return Err(err);
        }
        out.ok_or_else(|| HarnessError::ValidationFailed {
            reason: "RESTORE_TX_ABORTED".to_string(),
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
