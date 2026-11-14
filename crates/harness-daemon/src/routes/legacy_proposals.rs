// @story 01.3
use std::collections::HashSet;
use std::path::Path;

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use domain::{CanonicalSpecKey, HarnessError};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{map_sqlite_error, read_jailed_spec_bytes, retry_on_busy, SpecRegistry};
use uuid::Uuid;

use super::*;
use crate::daemon_auth::VerifiedLegacyIngress;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffItemDto {
    pub path: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheckDto {
    pub check_type: String,
    pub status: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProposalRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub intent_id: Option<Uuid>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub files: Vec<FileDiffItemDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeProposalDto {
    pub id: Uuid,
    #[serde(alias = "tenantId")]
    pub tenant_id: String,
    #[serde(alias = "projectId")]
    pub project_id: Uuid,
    #[serde(default)]
    pub intent_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub files: Vec<FileDiffItemDto>,
    pub validation_status: String,
    pub validation_checks: Vec<ValidationCheckDto>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyProposalRequest {
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotFileItemDto {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSnapshotRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version_tag: Option<String>,
    #[serde(default = "default_trigger")]
    pub trigger: String,
    pub files: Vec<SnapshotFileItemDto>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

fn default_trigger() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSnapshotDto {
    pub id: Uuid,
    #[serde(alias = "tenantId")]
    pub tenant_id: String,
    #[serde(alias = "projectId")]
    pub project_id: Uuid,
    pub version_tag: String,
    pub label: String,
    pub description: String,
    pub trigger: String,
    pub tree_sha256: String,
    pub files: Vec<SnapshotFileItemDto>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertSnapshotRequest {
    #[serde(default)]
    pub safety_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertResultDto {
    pub success: bool,
    pub reverted_to_snapshot_id: Uuid,
    pub safety_snapshot_id: Uuid,
    pub restored_files_count: usize,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportZipRequest {
    #[serde(default = "default_export_bool")]
    pub include_fixtures: bool,
    #[serde(default = "default_export_bool")]
    pub include_tokens: bool,
    #[serde(default = "default_export_bool")]
    pub include_icons: bool,
    #[serde(default = "default_export_format")]
    pub format: String,
}

fn default_export_bool() -> bool {
    true
}

fn default_export_format() -> String {
    "zip".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportZipResponseDto {
    pub file_name: String,
    pub byte_length: usize,
    pub sha256: String,
    pub files_included: Vec<String>,
    pub ready_to_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPRRequest {
    #[serde(alias = "branch_name")]
    pub branch_name: String,
    #[serde(alias = "pr_title")]
    pub pr_title: String,
    #[serde(default, alias = "pr_body")]
    pub pr_body: String,
    #[serde(default = "default_target_branch", alias = "target_branch")]
    pub target_branch: String,
}

fn default_target_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPRResponseDto {
    pub pr_url: String,
    pub pr_number: u64,
    pub branch_name: String,
    pub status: String,
    #[serde(default)]
    pub is_offline_mock: bool,
}

fn resolve_tenant_id(
    registry: &SpecRegistry,
    project_id: Uuid,
    caller: Option<&VerifiedLegacyIngress>,
) -> String {
    if let Some(verified) = caller {
        if matches!(
            verified.kind,
            crate::daemon_auth::LegacyAuthKind::HmacV2
                | crate::daemon_auth::LegacyAuthKind::SessionToken
        ) && !verified.principal.tenant_id.trim().is_empty()
        {
            return verified.principal.tenant_id.trim().to_string();
        }
    }
    if let Ok(conn) = registry.pool().connection() {
        if let Ok(t) = conn.query_row(
            "SELECT tenant_id FROM projects WHERE id = ?1",
            params![project_id.to_string()],
            |r| r.get::<_, String>(0),
        ) {
            if !t.trim().is_empty() {
                return t;
            }
        }
    }
    "local-default".to_string()
}

fn reconstruct_lintable_patch_source(raw_patch: &str) -> String {
    let looks_like_diff = raw_patch.starts_with("--- ")
        || raw_patch.contains("\n@@ ")
        || raw_patch.starts_with("@@ ")
        || raw_patch.lines().any(|line| {
            [
                "+const ",
                "+let ",
                "+var ",
                "+export ",
                "+import ",
                "+function ",
                "+interface ",
                "+type ",
                "+return ",
                "+<",
                "+//",
            ]
            .iter()
            .any(|pfx| line.starts_with(pfx))
        });
    if !looks_like_diff {
        return raw_patch.to_string();
    }
    raw_patch
        .lines()
        .filter_map(|line| {
            if line.starts_with("+++ ") || line.starts_with("--- ") || line.starts_with("@@ ") {
                None
            } else if let Some(added) = line.strip_prefix('+') {
                Some(added)
            } else if let Some(ctx) = line.strip_prefix(' ') {
                Some(ctx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_safe_relative_path(raw: &str) -> Result<String, HarnessError> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\0')
        || trimmed
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(HarnessError::PathTraversalDenied {
            path: raw.to_string(),
        });
    }
    Ok(trimmed)
}

fn run_proposal_validation_checks(
    registry: &SpecRegistry,
    canon_project_root: &Path,
    project_id: Uuid,
    files: &[FileDiffItemDto],
    timestamp: &str,
) -> Result<(String, Vec<ValidationCheckDto>), HarnessError> {
    if files.is_empty() {
        return Err(HarnessError::ValidationFailed {
            reason: "PROPOSAL_EMPTY: at least one file patch is required".to_string(),
        });
    }
    for f in files {
        validate_safe_relative_path(&f.path)?;
    }

    let conn = registry.pool().connection()?;
    let proj_str = project_id.to_string();
    let constraints =
        constraint_engine::ConstraintEvaluator::load_constraints_for_project(&conn, &proj_str)?;
    let (icon_manifest, typography_contract, component_registry) =
        constraint_engine::IconValidator::load_project_design_context(
            &conn,
            canon_project_root,
            &proj_str,
        )?;

    let mut ast_failures = Vec::new();
    for f in files {
        let lower_patch = f.patch.to_ascii_lowercase();
        if lower_patch.contains("eval(")
            || lower_patch.contains("dangerouslysetinnerhtml")
            || lower_patch.contains("__html")
        {
            ast_failures.push(format!(
                "{}: forbidden dynamic execution/HTML pattern",
                f.path
            ));
        }
        let lower_path = f.path.to_ascii_lowercase();
        let is_js_ts = lower_path.ends_with(".tsx")
            || lower_path.ends_with(".ts")
            || lower_path.ends_with(".jsx")
            || lower_path.ends_with(".js");
        if is_js_ts {
            let source_to_lint = reconstruct_lintable_patch_source(&f.patch);
            let violations = constraint_engine::AstLinter::lint_source_with_context(
                &source_to_lint,
                &constraints,
                Some(&icon_manifest),
                Some(&typography_contract),
                Some(&component_registry),
            )?;
            for v in violations {
                if matches!(
                    v.severity.to_ascii_uppercase().as_str(),
                    "BLOCKER" | "WARN" | "WARNING"
                ) {
                    ast_failures.push(format!("{}: [{}] {}", f.path, v.rule_code, v.message));
                }
            }
        } else if lower_path.ends_with(".json")
            && serde_json::from_str::<serde_json::Value>(&f.patch).is_err()
        {
            ast_failures.push(format!("{}: invalid JSON payload", f.path));
        }
    }

    let ast_passed = ast_failures.is_empty();
    let ast_msg = if ast_passed {
        format!(
            "AST and design contracts verified across {} file(s)",
            files.len()
        )
    } else {
        ast_failures.join("; ")
    };

    let has_package_json = canon_project_root.join("package.json").is_file();
    let ext_msg = if has_package_json {
        "External synchronous TypeScript/bundle runner skipped by daemon safety policy".to_string()
    } else {
        "No package.json workspace target in project root; skipped".to_string()
    };

    let checks = vec![
        ValidationCheckDto {
            check_type: "ast_lint".to_string(),
            status: if ast_passed { "passed" } else { "failed" }.to_string(),
            message: ast_msg,
            timestamp: timestamp.to_string(),
        },
        ValidationCheckDto {
            check_type: "typecheck".to_string(),
            status: "skipped".to_string(),
            message: ext_msg.clone(),
            timestamp: timestamp.to_string(),
        },
        ValidationCheckDto {
            check_type: "build".to_string(),
            status: "skipped".to_string(),
            message: ext_msg,
            timestamp: timestamp.to_string(),
        },
    ];

    let validation_status = if ast_passed { "passed" } else { "failed" }.to_string();
    Ok((validation_status, checks))
}

fn map_proposal_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChangeProposalDto> {
    let id_str: String = row.get(0)?;
    let project_id_str: String = row.get(1)?;
    let tenant_id: String = row.get(2)?;
    let intent_id_str: Option<String> = row.get(3)?;
    let title: String = row.get(4)?;
    let description: String = row.get(5)?;
    let files_json: String = row.get(6)?;
    let validation_status: String = row.get(7)?;
    let checks_json: String = row.get(8)?;
    let status: String = row.get(9)?;
    let created_at: String = row.get(10)?;
    let updated_at: String = row.get(11)?;

    Ok(ChangeProposalDto {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        tenant_id,
        project_id: Uuid::parse_str(&project_id_str).unwrap_or_else(|_| Uuid::nil()),
        intent_id: intent_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        title,
        description,
        files: serde_json::from_str(&files_json).unwrap_or_default(),
        validation_status,
        validation_checks: serde_json::from_str(&checks_json).unwrap_or_default(),
        status,
        created_at,
        updated_at,
    })
}

fn proposal_not_found(id: Uuid) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "PROPOSAL_NOT_FOUND",
            "error": "PROPOSAL_NOT_FOUND",
            "message": format!("ChangeProposal '{id}' was not found for this project/tenant"),
        })),
    )
        .into_response()
}

fn snapshot_not_found(id: Uuid) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "SNAPSHOT_NOT_FOUND",
            "error": "SNAPSHOT_NOT_FOUND",
            "message": format!("VersionSnapshot '{id}' was not found for this project/tenant"),
        })),
    )
        .into_response()
}

fn load_proposal_by_id(
    conn: &rusqlite::Connection,
    project_id: Uuid,
    proposal_id: Uuid,
    tenant_id: &str,
) -> Result<Option<ChangeProposalDto>, HarnessError> {
    conn.query_row(
        "SELECT id, project_id, tenant_id, intent_id, title, description, files_json,
                validation_status, validation_checks_json, status, created_at, updated_at
         FROM change_proposals
         WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
        params![proposal_id.to_string(), project_id.to_string(), tenant_id],
        map_proposal_row,
    )
    .optional()
    .map_err(|e| map_sqlite_error(e, "load_proposal_by_id"))
}

pub(super) async fn handle_list_proposals(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ChangeProposalDto>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let list = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, tenant_id, intent_id, title, description, files_json,
                        validation_status, validation_checks_json, status, created_at, updated_at
                 FROM change_proposals
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_list_proposals"))?;
        let rows = stmt
            .query_map(params![project_id.to_string(), tenant_id], map_proposal_row)
            .map_err(|e| map_sqlite_error(e, "query_list_proposals"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sqlite_error(e, "row_list_proposals"))?);
        }
        Ok::<_, HarnessError>(out)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(list))
}

pub(super) async fn handle_get_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ChangeProposalDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let found = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    match found {
        Some(p) => Ok(Json(p)),
        None => Err(proposal_not_found(proposal_id)),
    }
}

pub(super) async fn handle_create_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<CreateProposalRequest>,
) -> Result<(StatusCode, Json<ChangeProposalDto>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let base_registry = state.registry.clone();
    let now = chrono::Utc::now().to_rfc3339();
    let proposal_id = req.id.unwrap_or_else(Uuid::new_v4);

    let dto = tokio::task::spawn_blocking(move || {
        let (validation_status, checks) = run_proposal_validation_checks(
            &base_registry,
            &canon_project_root,
            project_id,
            &req.files,
            &now,
        )?;
        let status = if validation_status == "passed" {
            "ready".to_string()
        } else {
            "draft".to_string()
        };

        let files_json =
            serde_json::to_string(&req.files).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("PROPOSAL_SERIALIZE_ERROR: {e}"),
            })?;
        let checks_json =
            serde_json::to_string(&checks).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("PROPOSAL_CHECKS_SERIALIZE_ERROR: {e}"),
            })?;

        let mut conn = project_registry.pool().connection()?;
        let title_trimmed = req.title.trim().to_string();
        let spec_key = req
            .files
            .first()
            .map(|f| f.path.clone())
            .unwrap_or_else(|| "proposal".to_string());

        retry_on_busy("insert_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO change_proposals (
                    id, project_id, spec_key, diff_content, tenant_id, intent_id,
                    title, description, files_json, validation_status,
                    validation_checks_json, status, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    proposal_id.to_string(),
                    project_id.to_string(),
                    spec_key,
                    files_json,
                    tenant_id,
                    req.intent_id.map(|u| u.to_string()),
                    title_trimmed,
                    req.description,
                    files_json,
                    validation_status,
                    checks_json,
                    status,
                    now,
                    now,
                ],
            )?;
            tx.commit()
        })?;

        Ok::<_, HarnessError>(ChangeProposalDto {
            id: proposal_id,
            tenant_id,
            project_id,
            intent_id: req.intent_id,
            title: title_trimmed,
            description: req.description,
            files: req.files,
            validation_status,
            validation_checks: checks,
            status,
            created_at: now.clone(),
            updated_at: now,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok((StatusCode::CREATED, Json(dto)))
}

pub(super) async fn handle_validate_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ChangeProposalDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let base_registry = state.registry.clone();
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(mut proposal) =
            load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        if proposal.status == "rejected" || proposal.status == "applied" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": format!("Cannot validate ChangeProposal in terminal '{}' status.", proposal.status)
                })),
            )
                .into_response()));
        }

        let (validation_status, checks) = run_proposal_validation_checks(
            &base_registry,
            &canon_project_root,
            project_id,
            &proposal.files,
            &now,
        )?;
        let next_status = if validation_status == "passed" {
            "ready".to_string()
        } else {
            "draft".to_string()
        };
        let checks_json = serde_json::to_string(&checks).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("PROPOSAL_CHECKS_SERIALIZE_ERROR: {e}"),
            }
        })?;

        retry_on_busy("validate_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE change_proposals
                 SET status = ?1, validation_status = ?2, validation_checks_json = ?3, updated_at = ?4
                 WHERE id = ?5 AND project_id = ?6 AND tenant_id = ?7",
                params![
                    next_status,
                    validation_status,
                    checks_json,
                    now,
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id,
                ],
            )?;
            tx.commit()
        })?;

        proposal.status = next_status;
        proposal.validation_status = validation_status;
        proposal.validation_checks = checks;
        proposal.updated_at = now;
        Ok(Ok(proposal))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

pub(super) async fn handle_apply_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(_req): Json<ApplyProposalRequest>,
) -> Result<Json<ChangeProposalDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(mut proposal) =
            load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        if proposal.status == "rejected" || proposal.status == "applied" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": format!("Cannot apply ChangeProposal in '{}' status. Only 'ready' proposals can be applied.", proposal.status)
                })),
            )
                .into_response()));
        }

        let has_failed = proposal.validation_checks.iter().any(|c| c.status == "failed");
        if proposal.validation_status != "passed" || has_failed {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "PRECONDITION_FAILED",
                    "message": "Cannot apply ChangeProposal: AST/constraint validation checks must pass first."
                })),
            )
                .into_response()));
        }

        if proposal.status != "ready" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": format!("Cannot apply ChangeProposal in '{}' status. Only 'ready' proposals can be applied.", proposal.status)
                })),
            )
                .into_response()));
        }

        for f in &proposal.files {
            validate_safe_relative_path(&f.path)?;
        }

        retry_on_busy("apply_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for file in &proposal.files {
                let rel_path = validate_safe_relative_path(&file.path)
                    .map_err(|_| rusqlite::Error::InvalidParameterName(file.path.clone()))?;
                if file.status.eq_ignore_ascii_case("deleted") {
                    tx.execute(
                        "DELETE FROM project_workspace_files WHERE project_id = ?1 AND relative_path = ?2",
                        params![project_id.to_string(), rel_path],
                    )?;
                } else {
                    let sha = format!("{:x}", Sha256::digest(file.patch.as_bytes()));
                    let byte_size = file.patch.len() as i64;
                    tx.execute(
                        "INSERT INTO project_workspace_files (
                            id, project_id, relative_path, file_sha256, byte_size, content, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(project_id, relative_path) DO UPDATE SET
                            file_sha256 = excluded.file_sha256,
                            byte_size = excluded.byte_size,
                            content = excluded.content,
                            updated_at = excluded.updated_at",
                        params![
                            Uuid::new_v4().to_string(),
                            project_id.to_string(),
                            rel_path,
                            sha,
                            byte_size,
                            file.patch,
                            now,
                            now,
                        ],
                    )?;
                }
            }
            tx.execute(
                "UPDATE change_proposals SET status = 'applied', updated_at = ?1
                 WHERE id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![now, proposal_id.to_string(), project_id.to_string(), tenant_id],
            )?;
            tx.commit()
        })?;

        proposal.status = "applied".to_string();
        proposal.updated_at = now;
        Ok(Ok(proposal))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

pub(super) async fn handle_reject_proposal(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, proposal_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<ChangeProposalDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let proposal_id = parse_project_uuid(&proposal_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let Some(mut proposal) = load_proposal_by_id(&conn, project_id, proposal_id, &tenant_id)?
        else {
            return Ok::<_, HarnessError>(Err(proposal_not_found(proposal_id)));
        };

        if proposal.status == "applied" {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "INVALID_STATE",
                    "message": "Cannot reject an already applied ChangeProposal."
                })),
            )
                .into_response()));
        }

        retry_on_busy("reject_change_proposal", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE change_proposals SET status = 'rejected', updated_at = ?1
                 WHERE id = ?2 AND project_id = ?3 AND tenant_id = ?4",
                params![
                    now,
                    proposal_id.to_string(),
                    project_id.to_string(),
                    tenant_id
                ],
            )?;
            tx.commit()
        })?;

        proposal.status = "rejected".to_string();
        proposal.updated_at = now;
        Ok(Ok(proposal))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

fn map_snapshot_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VersionSnapshotDto> {
    let id_str: String = row.get(0)?;
    let proj_id_str: String = row.get(1)?;
    let tenant_id: String = row.get(2)?;
    let version_tag: String = row.get(3)?;
    let label: String = row.get(4)?;
    let description: String = row.get(5)?;
    let trigger: String = row.get(6)?;
    let tree_sha256: String = row.get(7)?;
    let files_json: String = row.get(8)?;
    let created_at: String = row.get(9)?;

    Ok(VersionSnapshotDto {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        tenant_id,
        project_id: Uuid::parse_str(&proj_id_str).unwrap_or_else(|_| Uuid::nil()),
        version_tag,
        label,
        description,
        trigger,
        tree_sha256,
        files: serde_json::from_str(&files_json).unwrap_or_default(),
        metadata: None,
        created_at,
    })
}

fn compute_files_tree_sha(files: &[SnapshotFileItemDto]) -> Result<String, HarnessError> {
    let mut sorted = files.to_vec();
    for f in &mut sorted {
        f.path = validate_safe_relative_path(&f.path)?;
    }
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let canonical = serde_json::to_string(&sorted).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("SNAPSHOT_SERIALIZE_ERROR: {e}"),
    })?;
    Ok(format!("{:x}", Sha256::digest(canonical.as_bytes())))
}

pub(super) async fn handle_get_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, snapshot_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<VersionSnapshotDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let snapshot_id = parse_project_uuid(&snapshot_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let found = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        conn.query_row(
            "SELECT id, project_id, tenant_id, version_tag, label, description,
                    trigger, tree_sha256, files_json, created_at
             FROM version_snapshots
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![snapshot_id.to_string(), project_id.to_string(), tenant_id],
            map_snapshot_row,
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "get_snapshot_by_id"))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    match found {
        Some(snap) => Ok(Json(snap)),
        None => Err(snapshot_not_found(snapshot_id)),
    }
}

pub(super) async fn handle_list_snapshots(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<VersionSnapshotDto>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());

    let list = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, tenant_id, version_tag, label, description,
                        trigger, tree_sha256, files_json, created_at
                 FROM version_snapshots
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY created_at ASC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_list_snapshots"))?;
        let rows = stmt
            .query_map(params![project_id.to_string(), tenant_id], map_snapshot_row)
            .map_err(|e| map_sqlite_error(e, "query_list_snapshots"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sqlite_error(e, "row_list_snapshots"))?);
        }
        Ok::<_, HarnessError>(out)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(list))
}

pub(super) async fn handle_create_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<(StatusCode, Json<VersionSnapshotDto>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();
    let snapshot_id = req.id.unwrap_or_else(Uuid::new_v4);

    let dto = tokio::task::spawn_blocking(move || {
        let tree_sha256 = compute_files_tree_sha(&req.files)?;
        let normalized_files: Vec<SnapshotFileItemDto> = req
            .files
            .into_iter()
            .map(|mut f| {
                f.path = validate_safe_relative_path(&f.path)?;
                let sha = format!("{:x}", Sha256::digest(f.content.as_bytes()));
                f.sha256 = Some(sha);
                f.size_bytes = Some(f.content.len());
                Ok(f)
            })
            .collect::<Result<Vec<_>, HarnessError>>()?;

        let files_json =
            serde_json::to_string(&normalized_files).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("SNAPSHOT_FILES_SERIALIZE_ERROR: {e}"),
            })?;

        let mut conn = project_registry.pool().connection()?;
        let existing_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM version_snapshots WHERE project_id = ?1",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| map_sqlite_error(e, "count_snapshots"))?;
        let version_tag = req
            .version_tag
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("v1.{}.0", existing_count + 1));

        retry_on_busy("insert_version_snapshot", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO version_snapshots (
                    id, project_id, snapshot_tag, manifest_sha256,
                    tenant_id, version_tag, label, description, trigger,
                    tree_sha256, files_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    snapshot_id.to_string(),
                    project_id.to_string(),
                    format!("{version_tag}-{}", &snapshot_id.to_string()[..8]),
                    tree_sha256,
                    tenant_id,
                    version_tag,
                    req.label,
                    req.description,
                    req.trigger,
                    tree_sha256,
                    files_json,
                    now,
                    now,
                ],
            )?;
            for f in &normalized_files {
                let sha = f
                    .sha256
                    .clone()
                    .unwrap_or_else(|| format!("{:x}", Sha256::digest(f.content.as_bytes())));
                tx.execute(
                    "INSERT INTO project_workspace_files (
                        id, project_id, relative_path, file_sha256, byte_size, content, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(project_id, relative_path) DO UPDATE SET
                        file_sha256 = excluded.file_sha256,
                        byte_size = excluded.byte_size,
                        content = excluded.content,
                        updated_at = excluded.updated_at",
                    params![
                        Uuid::new_v4().to_string(),
                        project_id.to_string(),
                        f.path,
                        sha,
                        f.content.len() as i64,
                        f.content,
                        now,
                        now,
                    ],
                )?;
            }
            tx.commit()
        })?;

        Ok::<_, HarnessError>(VersionSnapshotDto {
            id: snapshot_id,
            tenant_id,
            project_id,
            version_tag,
            label: req.label,
            description: req.description,
            trigger: req.trigger,
            tree_sha256,
            files: normalized_files,
            metadata: req.metadata,
            created_at: now,
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok((StatusCode::CREATED, Json(dto)))
}

fn load_workspace_files_from_db(
    conn: &rusqlite::Connection,
    project_id: Uuid,
) -> Result<Vec<SnapshotFileItemDto>, HarnessError> {
    let mut stmt = conn
        .prepare(
            "SELECT relative_path, content, file_sha256, byte_size
             FROM project_workspace_files
             WHERE project_id = ?1
             ORDER BY relative_path ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_load_workspace_files"))?;
    let rows = stmt
        .query_map(params![project_id.to_string()], |row| {
            Ok(SnapshotFileItemDto {
                path: row.get(0)?,
                content: row.get(1)?,
                sha256: row.get::<_, Option<String>>(2)?,
                size_bytes: row.get::<_, Option<i64>>(3)?.map(|n| n.max(0) as usize),
            })
        })
        .map_err(|e| map_sqlite_error(e, "query_load_workspace_files"))?;
    let mut files = Vec::new();
    for r in rows {
        files.push(r.map_err(|e| map_sqlite_error(e, "row_load_workspace_files"))?);
    }
    Ok(files)
}

pub(super) async fn handle_revert_snapshot(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, snapshot_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<RevertSnapshotRequest>,
) -> Result<Json<RevertResultDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let snapshot_id = parse_project_uuid(&snapshot_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let now = chrono::Utc::now().to_rfc3339();
    let safety_id = Uuid::new_v4();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let target: Option<VersionSnapshotDto> = conn
            .query_row(
                "SELECT id, project_id, tenant_id, version_tag, label, description,
                        trigger, tree_sha256, files_json, created_at
                 FROM version_snapshots
                 WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![snapshot_id.to_string(), project_id.to_string(), tenant_id],
                map_snapshot_row,
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "load_target_snapshot"))?;

        let Some(target) = target else {
            return Ok::<_, HarnessError>(Err(snapshot_not_found(snapshot_id)));
        };

        let mut safety_files = load_workspace_files_from_db(&conn, project_id)?;
        if safety_files.is_empty() {
            safety_files = target.files.clone();
        }
        let safety_sha = compute_files_tree_sha(&safety_files)?;
        let safety_files_json =
            serde_json::to_string(&safety_files).map_err(|e| HarnessError::ValidationFailed {
                reason: format!("SAFETY_FILES_SERIALIZE_ERROR: {e}"),
            })?;
        let safety_label = req
            .safety_label
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Pre-Revert Safety Backup".to_string());
        let safety_tag = format!("safety-backup-{}", &safety_id.to_string()[..8]);

        retry_on_busy("revert_version_snapshot", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO version_snapshots (
                    id, project_id, snapshot_tag, manifest_sha256,
                    tenant_id, version_tag, label, description, trigger,
                    tree_sha256, files_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pre_revert_safety', ?9, ?10, ?11, ?12)",
                params![
                    safety_id.to_string(),
                    project_id.to_string(),
                    safety_tag,
                    safety_sha,
                    tenant_id,
                    "safety-backup",
                    safety_label,
                    format!("Auto safety backup before reverting to {snapshot_id}"),
                    safety_sha,
                    safety_files_json,
                    now,
                    now,
                ],
            )?;
            tx.execute(
                "DELETE FROM project_workspace_files WHERE project_id = ?1",
                params![project_id.to_string()],
            )?;
            for f in &target.files {
                let rel_path = validate_safe_relative_path(&f.path)
                    .map_err(|_| rusqlite::Error::InvalidParameterName(f.path.clone()))?;
                let sha = format!("{:x}", Sha256::digest(f.content.as_bytes()));
                tx.execute(
                    "INSERT INTO project_workspace_files (
                        id, project_id, relative_path, file_sha256, byte_size, content, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        Uuid::new_v4().to_string(),
                        project_id.to_string(),
                        rel_path,
                        sha,
                        f.content.len() as i64,
                        f.content,
                        now,
                        now,
                    ],
                )?;
            }
            tx.commit()
        })?;

        Ok(Ok(RevertResultDto {
            success: true,
            reverted_to_snapshot_id: snapshot_id,
            safety_snapshot_id: safety_id,
            restored_files_count: target.files.len(),
            timestamp: now,
        }))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    result.map(Json)
}

fn collect_exportable_files(
    project_registry: &SpecRegistry,
    canon_project_root: &Path,
    project_id: Uuid,
    tenant_id: &str,
) -> Result<Vec<(String, Vec<u8>)>, HarnessError> {
    let conn = project_registry.pool().connection()?;
    let mut items = load_workspace_files_from_db(&conn, project_id)?;
    if items.is_empty() {
        let latest_json: Option<String> = conn
            .query_row(
                "SELECT files_json FROM version_snapshots
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY created_at DESC LIMIT 1",
                params![project_id.to_string(), tenant_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "load_latest_snapshot_for_export"))?;
        if let Some(json_str) = latest_json {
            items = serde_json::from_str(&json_str).unwrap_or_default();
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let rel = validate_safe_relative_path(&item.path)?;
        if seen.insert(rel.clone()) {
            out.push((rel, item.content.into_bytes()));
        }
    }

    for spec_key in CanonicalSpecKey::ALL {
        if project_registry.get_spec(project_id, spec_key)?.is_some() {
            if let Some(bytes) = read_jailed_spec_bytes(canon_project_root, spec_key)? {
                let rel = format!("product/specs/{}", spec_key.as_str());
                if seen.insert(rel.clone()) {
                    out.push((rel, bytes));
                }
            }
        }
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Builds a valid STORE-method PKZIP archive (Local File Headers + Central Directory + EOCD).
fn build_pkzip_archive(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for (name, data) in entries {
        let offset = out.len() as u32;
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(u16::MAX as usize) as u16;
        let crc = crc32_ieee(data);
        let size = data.len() as u32;

        // Local file header (30 bytes + filename)
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 filename flag
        out.extend_from_slice(&0u16.to_le_bytes()); // compression method: STORE
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(&name_bytes[..name_len as usize]);
        out.extend_from_slice(data);

        // Central directory header (46 bytes + filename)
        central_dir.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central_dir.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central_dir.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central_dir.extend_from_slice(&0x0800u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes()); // STORE
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&crc.to_le_bytes());
        central_dir.extend_from_slice(&size.to_le_bytes());
        central_dir.extend_from_slice(&size.to_le_bytes());
        central_dir.extend_from_slice(&name_len.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u32.to_le_bytes());
        central_dir.extend_from_slice(&offset.to_le_bytes());
        central_dir.extend_from_slice(&name_bytes[..name_len as usize]);
    }

    let cd_offset = out.len() as u32;
    let cd_size = central_dir.len() as u32;
    let count = entries.len().min(u16::MAX as usize) as u16;
    out.extend_from_slice(&central_dir);

    // End of Central Directory (22 bytes)
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

pub(super) async fn handle_export_snapshots(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ExportZipRequest>,
) -> Result<Json<ExportZipResponseDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    if !req.include_tokens || !req.include_icons {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "code": "AC03_RUNNABLE_BUNDLE_VIOLATION",
                "error": "AC03_RUNNABLE_BUNDLE_VIOLATION",
                "message": "Export bundle must retain design tokens and icon manifest to remain runnable (Story 05.3 AC-03).",
            })),
        )
            .into_response());
    }

    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let tenant_for_name = tenant_id.clone();

    let dto = tokio::task::spawn_blocking(move || {
        let entries = collect_exportable_files(
            &project_registry,
            &canon_project_root,
            project_id,
            &tenant_id,
        )?;
        if entries.is_empty() {
            return Ok::<_, HarnessError>(Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "code": "NO_EXPORTABLE_FILES",
                    "error": "NO_EXPORTABLE_FILES",
                    "message": "No persisted workspace files, snapshots, or specs available to export",
                })),
            )
                .into_response()));
        }

        let files_included: Vec<String> = entries.iter().map(|(p, _)| p.clone()).collect();
        let ready_to_run = files_included.iter().any(|p| p == "package.json");
        let zip_bytes = build_pkzip_archive(&entries);
        let sha256 = format!("{:x}", Sha256::digest(&zip_bytes));
        let format_ext = "zip";
        let date_str = chrono::Utc::now().format("%Y%m%d").to_string();
        let file_name = format!("{project_id}-{tenant_for_name}-bundle-{date_str}.{format_ext}");

        Ok(Ok(ExportZipResponseDto {
            file_name,
            byte_length: zip_bytes.len(),
            sha256,
            files_included,
            ready_to_run,
        }))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    dto.map(Json)
}

pub(super) async fn handle_download_export(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_tenant_id(&state.registry, project_id, caller.as_deref());
    let tenant_for_collect = tenant_id.clone();

    let entries = tokio::task::spawn_blocking(move || {
        collect_exportable_files(
            &project_registry,
            &canon_project_root,
            project_id,
            &tenant_for_collect,
        )
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    if entries.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "code": "NO_EXPORTABLE_FILES",
                "error": "NO_EXPORTABLE_FILES",
                "message": "No persisted workspace files, snapshots, or specs available to download",
            })),
        )
            .into_response());
    }

    let zip_bytes = build_pkzip_archive(&entries);
    let sha256 = format!("{:x}", Sha256::digest(&zip_bytes));
    let date_str = chrono::Utc::now().format("%Y%m%d").to_string();
    let file_name = format!("{project_id}-{tenant_id}-bundle-{date_str}.zip");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file_name}\""),
        )
        .header("x-cowok-bundle-sha256", sha256)
        .body(axum::body::Body::from(zip_bytes))
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to build zip response",
            )
                .into_response()
        })
}

pub(super) async fn handle_create_pr(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(_req): Json<GitHubPRRequest>,
) -> Result<Json<GitHubPRResponseDto>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "code": "GITHUB_INTEGRATION_NOT_CONFIGURED",
            "error": "GITHUB_INTEGRATION_NOT_CONFIGURED",
            "message": "Remote GitHub Pull Request creation requires a configured GitHub App / git remote integration; offline mock PR fabrication is disabled.",
        })),
    )
        .into_response())
}
