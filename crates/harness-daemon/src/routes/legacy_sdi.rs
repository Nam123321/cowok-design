// @story 01.3

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use domain::{ContextSliceRequest, Department, HarnessError, Portal};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::{map_sqlite_error, retry_on_busy};
use uuid::Uuid;

use super::*;
use crate::daemon_auth::VerifiedLegacyIngress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCompileRequest {
    pub system_instruction: Option<String>,
    pub project_constraints: Option<Vec<String>>,
    pub slice_request: Option<ContextSliceRequest>,
    pub task_instructions: String,
    pub optional_components: Option<Vec<String>>,
    pub token_ceiling: Option<usize>,
}

pub(super) async fn handle_prompt_compile(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<PromptCompileRequest>,
) -> Result<Json<domain::CompiledPromptPayload>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;

    let (project_registry, _canon_project_root, is_superadmin) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    // F-01: Validate token_ceiling does not exceed mandatory ceiling
    if let Some(c) = req.token_ceiling {
        if c > domain::TOKEN_BUDGET_CEILING {
            return Err(map_error_to_response(HarnessError::ValidationFailed {
                reason: format!(
                    "token_ceiling {} exceeds maximum allowed {}",
                    c,
                    domain::TOKEN_BUDGET_CEILING
                ),
            }));
        }
    }

    if let Some(ref slice_req) = req.slice_request {
        let is_privileged = matches!(
            slice_req.portal,
            Portal::SuperAdmin | Portal::TenantAdmin | Portal::DeveloperPortal
        ) || matches!(slice_req.department, Department::ExecutiveReport);

        if is_privileged && !is_superadmin {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "kind": "FORBIDDEN",
                    "message": "Privileged portal or department requires valid superadmin bearer authorization"
                })),
            )
                .into_response());
        }
    }

    let payload = tokio::task::spawn_blocking(move || {
        let slicer = spec_registry::ContextSlicer::new(&project_registry);

        let context_slice = match req.slice_request {
            Some(slice_req) => Some(slicer.slice_context(project_id, &slice_req)?),
            None => None,
        };

        // F-05: Fail-closed on SQLite errors loading project constraints
        let mut all_constraints = req.project_constraints.unwrap_or_default();
        let active = slicer.load_active_constraints(project_id)?;
        for c in active {
            all_constraints.push(format!("{}: {}", c.rule_code, c.config_json));
        }

        let input = domain::CompilationInput {
            project_id: id,
            system_instruction: req
                .system_instruction
                .unwrap_or_else(|| "You are an autonomous AI coding assistant.".into()),
            project_constraints: all_constraints,
            context_slice,
            task_instructions: req.task_instructions,
            optional_components: req.optional_components.unwrap_or_default(),
        };

        let ceiling = req.token_ceiling.unwrap_or(domain::TOKEN_BUDGET_CEILING);
        let compiler = domain::DeterministicPromptCompiler::new(ceiling);
        compiler.compile(input)
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok(Json(payload))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelAdapterRequestDto {
    pub requested_profile: domain::ModelProfileTier,
    pub raw_system_prompt: String,
    pub tools: Vec<domain::ModelToolDefinition>,
    #[serde(default)]
    pub disallowed_profiles: Vec<domain::ModelProfileTier>,
    #[serde(default)]
    pub syntax_error_reported: bool,
    #[serde(default)]
    pub max_budget_override: Option<usize>,
}

pub(super) async fn handle_model_adapter(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ModelAdapterRequestDto>,
) -> Result<Json<domain::AdaptationResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;

    let (project_registry, _canon_project_root, _is_superadmin) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        // F-01, F-07: Load active project constraints from SQLite WAL and merge disallowed profiles
        let slicer = spec_registry::ContextSlicer::new(&project_registry);
        let active_constraints = slicer.load_active_constraints(project_id)?;

        let disallowed_profiles =
            crate::extract_disallowed_profiles(&req.disallowed_profiles, &active_constraints);

        let adapt_req = domain::AdaptationRequest {
            project_id,
            requested_profile: req.requested_profile,
            raw_system_prompt: req.raw_system_prompt,
            tools: req.tools,
            disallowed_profiles,
            syntax_error_reported: req.syntax_error_reported,
            max_budget_override: req.max_budget_override,
        };

        let adapter = domain::ModelCapabilityAdapter::new();
        adapter.adapt(&adapt_req)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintSourceRequest {
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintSourceResponse {
    pub violations: Vec<constraint_engine::ConstraintViolation>,
    pub passed: bool,
}

pub(super) async fn handle_lint_source(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<LintSourceRequest>,
) -> Result<Json<LintSourceResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let constraints = constraint_engine::ConstraintEvaluator::load_constraints_for_project(
            &conn,
            &project_id.to_string(),
        )?;
        let (icon_manifest, typography_contract, component_registry) =
            constraint_engine::IconValidator::load_project_design_context(
                &conn,
                &canon_project_root,
                &project_id.to_string(),
            )?;
        let violations = constraint_engine::AstLinter::lint_source_with_context(
            &req.source,
            &constraints,
            Some(&icon_manifest),
            Some(&typography_contract),
            Some(&component_registry),
        )?;
        let passed = violations.is_empty();
        Ok::<_, HarnessError>(LintSourceResponse { violations, passed })
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

pub(super) async fn handle_get_constraint_profile(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<domain::ConstraintProfile>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let constraints = constraint_engine::ConstraintEvaluator::load_constraints_for_project(
            &conn,
            &project_id.to_string(),
        )?;
        let existing_profile_id: Option<String> = conn.query_row(
            "SELECT id FROM constraint_profiles WHERE project_id = ?1 ORDER BY updated_at DESC LIMIT 1",
            rusqlite::params![project_id.to_string()],
            |r| r.get(0),
        ).ok();
        let prof_id_str = existing_profile_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        let domain_profile = constraint_engine::ConstraintEvaluator::to_domain_profile(
            &prof_id_str,
            &project_id.to_string(),
            &constraints,
            Some("react"),
        );
        Ok::<_, HarnessError>(domain_profile)
    })
    .await
    .map_err(|e| map_error_to_response(HarnessError::ValidationFailed {
        reason: format!("BLOCKING_TASK_FAILED: {e}"),
    }))?
    .map_err(map_error_to_response)?;

    Ok(Json(resp))
}

pub(super) async fn handle_put_constraint_profile(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(profile): Json<domain::ConstraintProfile>,
) -> Result<Json<Vec<constraint_engine::ActiveConstraint>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let active = constraint_engine::ConstraintEvaluator::active_constraints_from_domain(&profile);
        let mut conn = project_registry.pool().connection()?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let prof_id_str = profile.id.to_string();
        let proj_id_str = project_id.to_string();

        let forbid_tailwind = if profile.forbidden_patterns.iter().any(|p| p.to_uppercase().contains("TAILWIND")) { 1 } else { 0 };
        let forbid_raw_hex = if profile.forbidden_patterns.iter().any(|p| p.to_uppercase().contains("HEX")) { 1 } else { 0 };

        retry_on_busy("handle_put_constraint_profile_tx", || {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO constraint_profiles (id, project_id, preset_profile, styling_policy, forbid_tailwind, forbid_raw_hex, tokens_css_path, icon_manifest_path, allowed_icon_set_json, typography_contract_json, updated_at)
                 VALUES (?1, ?2, 'custom', 'tokens_strict', ?3, ?4, 'tokens.css', ?5, '[]', '{}', ?6)
                 ON CONFLICT(project_id, preset_profile) DO UPDATE SET forbid_tailwind = ?3, forbid_raw_hex = ?4, updated_at = ?6",
                rusqlite::params![
                    prof_id_str,
                    proj_id_str,
                    forbid_tailwind,
                    forbid_raw_hex,
                    constraint_engine::FLAGSHIP_MANIFEST_SENTINEL,
                    now_ms
                ],
            )?;

            let effective_prof_id: String = tx.query_row(
                "SELECT id FROM constraint_profiles WHERE project_id = ?1 AND preset_profile = 'custom'",
                rusqlite::params![proj_id_str],
                |r| r.get(0),
            ).unwrap_or_else(|_| prof_id_str.clone());

            tx.execute(
                "DELETE FROM constraint_rules WHERE profile_id = ?1",
                rusqlite::params![effective_prof_id],
            )?;

            for c in &active {
                tx.execute(
                    "INSERT OR REPLACE INTO constraint_rules (id, profile_id, rule_code, severity, config_json, enabled, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                    rusqlite::params![
                        Uuid::new_v4().to_string(),
                        effective_prof_id,
                        c.rule_code,
                        c.severity,
                        c.config_json,
                        now_ms,
                    ],
                )?;
            }
            tx.commit()
        })?;
        Ok::<_, HarnessError>(active)
    })
    .await
    .map_err(|e| map_error_to_response(HarnessError::ValidationFailed {
        reason: format!("BLOCKING_TASK_FAILED: {e}"),
    }))?
    .map_err(map_error_to_response)?;

    Ok(Json(resp))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateIconRequest {
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateTypographyRequest {
    pub font_family: String,
    pub font_weight: u32,
    #[serde(default)]
    pub font_size: Option<String>,
    pub contrast_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateComponentRequest {
    pub component: String,
}

pub(super) async fn handle_get_icon_manifest(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<constraint_engine::IconManifest>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let (manifest, _, _) = constraint_engine::IconValidator::load_project_design_context(
            &conn,
            &project_root,
            &project_id.to_string(),
        )?;
        Ok::<_, HarnessError>(manifest)
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

pub(super) async fn handle_validate_icon(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ValidateIconRequest>,
) -> Result<Json<constraint_engine::IconValidationReport>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let (manifest, _, _) = constraint_engine::IconValidator::load_project_design_context(
            &conn,
            &project_root,
            &project_id.to_string(),
        )?;
        let report = constraint_engine::IconValidator::validate_icon(&manifest, &req.icon);
        Ok::<_, HarnessError>(report)
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

pub(super) async fn handle_validate_typography(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ValidateTypographyRequest>,
) -> Result<Json<constraint_engine::TypographyValidationReport>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let (_, typography_contract, _) =
            constraint_engine::IconValidator::load_project_design_context(
                &conn,
                &project_root,
                &project_id.to_string(),
            )?;
        let report = constraint_engine::IconValidator::validate_typography_report(
            &typography_contract,
            &req.font_family,
            req.font_weight,
            req.font_size.as_deref(),
            req.contrast_ratio,
        );
        Ok::<_, HarnessError>(report)
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

pub(super) async fn handle_validate_component(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ValidateComponentRequest>,
) -> Result<Json<constraint_engine::ComponentValidationReport>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let (_, _, component_registry) =
            constraint_engine::IconValidator::load_project_design_context(
                &conn,
                &project_root,
                &project_id.to_string(),
            )?;
        let report = constraint_engine::IconValidator::validate_component(
            &component_registry,
            &req.component,
        );
        Ok::<_, HarnessError>(report)
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

pub(super) async fn handle_get_design_contracts(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<domain::ProjectDesignContracts>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        constraint_engine::IconValidator::load_project_design_contracts(
            &conn,
            &project_root,
            &project_id.to_string(),
        )
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

pub(super) async fn handle_put_design_contracts(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(contracts): Json<domain::ProjectDesignContracts>,
) -> Result<Json<domain::ProjectDesignContracts>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let proj_id_str = project_id.to_string();
        constraint_engine::IconValidator::save_project_design_contracts(
            &mut conn,
            &project_root,
            &proj_id_str,
            &contracts,
        )?;
        constraint_engine::IconValidator::load_project_design_contracts(
            &conn,
            &project_root,
            &proj_id_str,
        )
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

pub(super) async fn handle_inject_ff_id(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<constraint_engine::AstInjectionRequest>,
) -> Result<Json<constraint_engine::AstInjectionResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        constraint_engine::SurgicalPatcher::inject_ff_ids(
            &req.source,
            req.component_name.as_deref(),
        )
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

pub(super) async fn handle_surgical_patch(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<constraint_engine::SurgicalPatchRequest>,
) -> Result<Json<constraint_engine::SurgicalPatchResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
        .map_err(map_error_to_response)?;

    let resp = tokio::task::spawn_blocking(move || {
        constraint_engine::SurgicalPatcher::patch_by_ff_id(&req.source, &req)
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdiCompileRequest {
    #[serde(alias = "raw_prompt")]
    pub raw_prompt: String,
    #[serde(default, alias = "target_node_id")]
    pub target_node_id: Option<String>,
    #[serde(default, alias = "source_anchor_comment_id")]
    pub source_anchor_comment_id: Option<String>,
    #[serde(default, alias = "execute_immediately")]
    pub execute_immediately: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetScopeDto {
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub screen_keys: Vec<String>,
    #[serde(default)]
    pub components: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationVariantDto {
    pub id: String,
    pub title: String,
    pub description: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GapQuestionOptionDto {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_thumbnail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GapQuestionDto {
    pub id: String,
    pub question: String,
    pub category: String,
    pub options: Vec<GapQuestionOptionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_option_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdiCompileResponse {
    pub id: Uuid,
    #[serde(alias = "tenantId")]
    pub tenant_id: String,
    #[serde(alias = "projectId")]
    pub project_id: Uuid,
    pub raw_prompt: String,
    pub intent_type: String,
    pub routing_mode: String,
    pub target_scope: TargetScopeDto,
    pub ambiguity_score: f64,
    pub risk_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_variants: Option<Vec<ClarificationVariantDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_questions: Option<Vec<GapQuestionDto>>,
    pub status: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_intent_v1: Option<domain::DesignIntentV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdiClarificationRequest {
    #[serde(alias = "intent_id")]
    pub intent_id: Uuid,
    #[serde(alias = "selected_variant_id")]
    pub selected_variant_id: String,
}

enum IdempotencyReservation {
    New,
    InProgress,
    PayloadMismatch,
    Completed(u16, String),
}

fn check_or_reserve_idempotency_tx(
    conn: &mut Connection,
    tenant_id: &str,
    actor_id: &str,
    idempotency_key: &str,
    request_sha: &str,
    now_ms: i64,
) -> Result<IdempotencyReservation, HarnessError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| map_sqlite_error(e, "begin_idempotency_tx1"))?;

    let existing: Option<(String, String, i64, Option<i64>, Option<String>, i64)> = tx
        .query_row(
            "SELECT request_sha256, status, lease_expires_at, response_status, response_body_json, expires_at
             FROM idempotency_records
             WHERE tenant_id = ?1 AND actor_id = ?2 AND idempotency_key = ?3",
            params![tenant_id, actor_id, idempotency_key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "query_idempotency_record"))?;

    if let Some((saved_sha, status, lease_exp, resp_st, resp_body, expires_at)) = existing {
        if saved_sha != request_sha {
            tx.rollback().ok();
            return Ok(IdempotencyReservation::PayloadMismatch);
        }
        if status == "COMPLETED" && now_ms < expires_at {
            tx.rollback().ok();
            return Ok(IdempotencyReservation::Completed(
                resp_st.unwrap_or(201) as u16,
                resp_body.unwrap_or_default(),
            ));
        }
        if status == "IN_PROGRESS" && now_ms < lease_exp {
            tx.rollback().ok();
            return Ok(IdempotencyReservation::InProgress);
        }
        // Lease expired or COMPLETED past 24h TTL - reset reservation cleanly with new 30s lease + 24h TTL
        let new_lease = now_ms + 30_000;
        let new_expires = now_ms + 86_400_000;
        tx.execute(
            "UPDATE idempotency_records
             SET status = 'IN_PROGRESS', lease_expires_at = ?1, response_status = NULL, response_body_json = NULL, expires_at = ?2
             WHERE tenant_id = ?3 AND actor_id = ?4 AND idempotency_key = ?5",
            params![new_lease, new_expires, tenant_id, actor_id, idempotency_key],
        )
        .map_err(|e| map_sqlite_error(e, "renew_idempotency_lease"))?;
        tx.commit()
            .map_err(|e| map_sqlite_error(e, "commit_idempotency_lease"))?;
        return Ok(IdempotencyReservation::New);
    }

    let lease_expires_at = now_ms + 30_000;
    let expires_at = now_ms + 86_400_000; // 24h
    tx.execute(
        "INSERT INTO idempotency_records (
            tenant_id, actor_id, idempotency_key, request_sha256, status, lease_expires_at,
            response_status, response_body_json, expires_at
         ) VALUES (?1, ?2, ?3, ?4, 'IN_PROGRESS', ?5, NULL, NULL, ?6)",
        params![tenant_id, actor_id, idempotency_key, request_sha, lease_expires_at, expires_at],
    )
    .map_err(|e| map_sqlite_error(e, "insert_idempotency_record"))?;
    tx.commit()
        .map_err(|e| map_sqlite_error(e, "commit_idempotency_tx1"))?;

    Ok(IdempotencyReservation::New)
}

fn resolve_sdi_tenant_id(
    registry: &spec_registry::SpecRegistry,
    project_id: Uuid,
    caller: Option<&VerifiedLegacyIngress>,
) -> String {
    match caller {
        Some(verified)
            if matches!(
                verified.kind,
                crate::daemon_auth::LegacyAuthKind::HmacV2
                    | crate::daemon_auth::LegacyAuthKind::SessionToken
            ) && !verified.principal.tenant_id.trim().is_empty() =>
        {
            verified.principal.tenant_id.trim().to_string()
        }
        _ => registry
            .pool()
            .connection()
            .ok()
            .and_then(|conn| {
                conn.query_row(
                    "SELECT tenant_id FROM projects WHERE id = ?1",
                    params![project_id.to_string()],
                    |r| r.get::<_, String>(0),
                )
                .ok()
            })
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "local-default".to_string()),
    }
}

fn map_sdi_intent_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SdiCompileResponse> {
    let id_str: String = row.get(0)?;
    let tenant_id: String = row.get(1)?;
    let project_id_str: String = row.get(2)?;
    let raw_prompt: String = row.get(3)?;
    let intent_type: String = row.get(4)?;
    let scope_json: String = row.get(5)?;
    let ambiguity_score: f64 = row.get(6)?;
    let risk_score: f64 = row.get(7)?;
    let variants_json: Option<String> = row.get(8)?;
    let status: String = row.get(9)?;
    let design_intent_json: Option<String> = row.get(10)?;
    let created_at: String = row.get(11)?;
    let updated_at: String = row.get(12)?;
    let routing_mode: String = row.get(13).unwrap_or_else(|_| "direct_edit".to_string());
    let gap_questions_json: Option<String> = row.get(14).unwrap_or(None);
    let revision: i64 = row.get(15).unwrap_or(1);

    let target_scope = serde_json::from_str(&scope_json).unwrap_or(TargetScopeDto {
        node_ids: vec![],
        screen_keys: vec![],
        components: vec![],
    });
    let clarification_variants = variants_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<ClarificationVariantDto>>(raw).ok());
    let gap_questions = gap_questions_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<GapQuestionDto>>(raw).ok());
    let design_intent_v1 = design_intent_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<domain::DesignIntentV1>(raw).ok());

    Ok(SdiCompileResponse {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        tenant_id,
        project_id: Uuid::parse_str(&project_id_str).unwrap_or_else(|_| Uuid::nil()),
        raw_prompt,
        intent_type,
        routing_mode,
        target_scope,
        ambiguity_score,
        risk_score,
        clarification_variants,
        gap_questions,
        status,
        revision,
        created_at,
        updated_at,
        design_intent_v1,
    })
}

fn intent_not_found(id: Uuid) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "INTENT_NOT_FOUND",
            "error": "INTENT_NOT_FOUND",
            "message": format!("SDI intent '{id}' was not found for this project/tenant"),
        })),
    )
        .into_response()
}

fn compute_sdi_risk_score(
    lower_prompt: &str,
    has_specific: bool,
    active_constraints: &[constraint_engine::ActiveConstraint],
) -> f64 {
    let mut risk: f64 = 0.15;
    if !has_specific {
        risk += 0.40;
    }
    if lower_prompt.contains("delete")
        || lower_prompt.contains("drop")
        || lower_prompt.contains("auth")
        || lower_prompt.contains("security")
        || lower_prompt.contains("migration")
        || lower_prompt.contains("schema")
        || lower_prompt.contains("remove")
    {
        risk += 0.15;
    }
    let blocker_count = active_constraints
        .iter()
        .filter(|c| format!("{:?}", c.severity).eq_ignore_ascii_case("blocker"))
        .count();
    risk += (blocker_count.min(3) as f64) * 0.05;
    (risk.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

fn project_not_found(id: Uuid) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "PROJECT_NOT_FOUND",
            "error": "PROJECT_NOT_FOUND",
            "message": format!("Project '{id}' was not found for this tenant"),
        })),
    )
        .into_response()
}

fn verify_sdi_project_tenant(
    registry: &spec_registry::SpecRegistry,
    project_id: Uuid,
    caller: Option<&VerifiedLegacyIngress>,
) -> Result<(), Response> {
    if let Some(ver) = caller {
        let conn = registry.pool().connection().map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: e.to_string(),
            })
        })?;
        let row: Option<String> = conn
            .query_row(
                "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "anti_idor_check")))?;
        match row {
            Some(ref pt)
                if ver.principal.role == "admin"
                    || pt.trim() == ver.principal.tenant_id.trim() =>
            {
                Ok(())
            }
            _ => Err(project_not_found(project_id)),
        }
    } else {
        Ok(())
    }
}

pub(super) async fn handle_sdi_intent(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SdiCompileRequest>,
) -> Result<(StatusCode, Json<SdiCompileResponse>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    verify_sdi_project_tenant(&state.registry, project_id, caller.as_deref())?;

    let (project_registry, canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let tenant_id = resolve_sdi_tenant_id(&state.registry, project_id, caller.as_deref());

    // Anti-IDOR: verify project belongs to tenant (404 on mismatch/not-found)
    let (project_tenant, project_name) = {
        let conn = project_registry.pool().connection().map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: e.to_string(),
            })
        })?;
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT tenant_id, name FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
                params![project_id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "query_project_tenant")))?;
        match row {
            Some(p) => p,
            None => return Err(project_not_found(project_id)),
        }
    };

    if let Some(ref ver) = caller {
        if !ver.principal.tenant_id.trim().is_empty()
            && ver.principal.role != "admin"
            && ver.principal.tenant_id.trim() != project_tenant.trim()
        {
            return Err(project_not_found(project_id));
        }
    }

    // Two-Phase Idempotency Reservation
    let actor_id = caller
        .as_ref()
        .map(|v| v.principal.user_id.clone())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "local-operator".to_string());

    let idempotency_key = headers
        .get("idempotency-key")
        .or_else(|| headers.get("x-idempotency-key"))
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let req_body_str = serde_json::to_string(&req).unwrap_or_default();
    let req_sha = {
        let mut hasher = Sha256::new();
        hasher.update(format!("{tenant_id}:{actor_id}:{project_id}:{req_body_str}").as_bytes());
        format!("{:x}", hasher.finalize())
    };

    if let Some(ref ikey) = idempotency_key {
        let mut conn = project_registry.pool().connection().map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: e.to_string(),
            })
        })?;
        let now_ms = Utc::now().timestamp_millis();
        let reservation = check_or_reserve_idempotency_tx(
            &mut conn,
            &tenant_id,
            &actor_id,
            ikey,
            &req_sha,
            now_ms,
        )
        .map_err(map_error_to_response)?;

        match reservation {
            IdempotencyReservation::PayloadMismatch => {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({
                        "code": "IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_PAYLOAD",
                        "error": "IDEMPOTENCY_KEY_REUSED_WITH_DIFFERENT_PAYLOAD",
                        "message": "Idempotency key has already been used with a different request payload",
                    })),
                )
                    .into_response());
            }
            IdempotencyReservation::InProgress => {
                return Err((
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "code": "IDEMPOTENCY_IN_PROGRESS",
                        "error": "IDEMPOTENCY_IN_PROGRESS",
                        "message": "A request with this idempotency key is currently in progress",
                    })),
                )
                    .into_response());
            }
            IdempotencyReservation::Completed(status_code, body_str) => {
                let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK);
                if let Ok(dto) = serde_json::from_str::<SdiCompileResponse>(&body_str) {
                    return Ok((status, Json(dto)));
                } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    if let Ok(dto) = serde_json::from_value::<SdiCompileResponse>(val) {
                        return Ok((status, Json(dto)));
                    }
                }
            }
            IdempotencyReservation::New => {}
        }
    }

    let slicer = spec_registry::context_slicer::ContextSlicer::new(&state.registry);
    let active_constraints = slicer
        .load_active_constraints(project_id)
        .unwrap_or_default();

    let (typography_fonts, visual_style, color_palette) = {
        let conn = project_registry.pool().connection().ok();
        let proj_str = project_id.to_string();
        let loaded_fonts = conn
            .as_ref()
            .and_then(|c| {
                constraint_engine::IconValidator::load_project_design_context(
                    c,
                    &canon_project_root,
                    &proj_str,
                )
                .ok()
            })
            .map(|(_, typo, _)| typo.allowed_font_families)
            .filter(|fonts| !fonts.is_empty())
            .unwrap_or_else(|| vec!["system-ui".to_string(), "sans-serif".to_string()]);
        let palette = vec![
            "var(--accent-primary)".to_string(),
            "var(--accent-success)".to_string(),
            "var(--accent-warning)".to_string(),
        ];
        (loaded_fonts, "design-token-governed".to_string(), palette)
    };

    // SQLite-grounded scoring: query flow_nodes, flow_edges, project_specs, constraint_profiles
    let (existing_nodes, existing_edges_count, _confirmed_specs, styling_policy) = {
        let conn = project_registry.pool().connection().map_err(|e| {
            map_error_to_response(HarnessError::ValidationFailed {
                reason: e.to_string(),
            })
        })?;
        let mut node_stmt = conn
            .prepare(
                "SELECT id, screen_key, title, node_type, route_path
                 FROM flow_nodes
                 WHERE tenant_id = ?1 AND project_id = ?2",
            )
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "query_flow_nodes")))?;
        let nodes = node_stmt
            .query_map(params![tenant_id, project_id.to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "map_flow_nodes")))?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let edge_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM flow_edges WHERE tenant_id = ?1 AND project_id = ?2",
                params![tenant_id, project_id.to_string()],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let mut spec_stmt = conn
            .prepare("SELECT spec_key FROM project_specs WHERE project_id = ?1 AND status = 'CONFIRMED'")
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "query_specs")))?;
        let specs = spec_stmt
            .query_map(params![project_id.to_string()], |r| r.get::<_, String>(0))
            .map_err(|e| map_error_to_response(map_sqlite_error(e, "map_specs")))?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let styling: String = conn
            .query_row(
                "SELECT styling_policy FROM constraint_profiles WHERE project_id = ?1",
                params![project_id.to_string()],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "CSS_MODULES".to_string());

        (nodes, edge_count, specs, styling)
    };

    let lower = req.raw_prompt.to_lowercase();
    let word_count = lower.split_whitespace().count();

    let matched_by_target_id = req
        .target_node_id
        .as_deref()
        .and_then(|tid| existing_nodes.iter().find(|(nid, ..)| nid == tid));

    let matched_by_content = existing_nodes.iter().find(|(_, sk, title, ..)| {
        lower.contains(&sk.to_lowercase()) || lower.contains(&title.to_lowercase())
    });

    let matched_node = matched_by_target_id.or(matched_by_content);
    let is_node_targeted = matched_node.is_some();

    let has_specific_element = lower.contains("button")
        || lower.contains("header")
        || lower.contains("modal")
        || lower.contains("table")
        || lower.contains("drawer")
        || lower.contains("input")
        || lower.contains("form")
        || lower.contains("card")
        || lower.contains("nav")
        || lower.contains("sidebar")
        || lower.contains("footer")
        || lower.contains("pill")
        || lower.contains("dialog")
        || lower.contains("badge");

    let is_vague = (lower.contains("đẹp")
        || lower.contains("fix")
        || lower.contains("sửa")
        || lower.contains("change")
        || lower.contains("update")
        || lower.contains("tối ưu")
        || lower.contains("cải tiến")
        || lower.contains("giao diện")
        || lower.contains("better"))
        && word_count <= 4
        && !is_node_targeted;

    let is_multi_flow = lower.contains("luồng")
        || lower.contains("flow")
        || lower.contains("multi-step")
        || lower.contains("quy trình")
        || lower.contains("journey")
        || lower.contains("checkout")
        || lower.contains("onboarding");

    let (ambiguity_score, routing_mode) = if is_vague {
        (0.65, "clarification_required")
    } else if is_node_targeted && has_specific_element {
        (0.15, "direct_edit")
    } else if is_node_targeted {
        (0.25, "direct_edit")
    } else if is_multi_flow && word_count >= 6 {
        (0.30, "full_plan")
    } else if is_multi_flow {
        (0.55, "clarification_required")
    } else if existing_nodes.is_empty() && word_count >= 6 {
        (0.20, "full_plan")
    } else if has_specific_element {
        (0.35, "direct_edit")
    } else if word_count <= 3 {
        (0.70, "clarification_required")
    } else {
        (0.50, "clarification_required")
    };

    let risk_score = compute_sdi_risk_score(&lower, has_specific_element, &active_constraints);
    let is_ambiguous = ambiguity_score >= 0.45;

    let target_label = matched_node
        .map(|(_, _, title, ..)| title.as_str())
        .unwrap_or(project_name.as_str());

    let (variants, gap_questions) = if is_ambiguous {
        let vars = vec![
            ClarificationVariantDto {
                id: "var-1-minimal".to_string(),
                title: format!("Phương án 1: Cục bộ & Tối giản (Minimal) — {target_label}"),
                description: format!(
                    "Áp dụng điều chỉnh cục bộ ngay trên màn hình '{}' cho yêu cầu '{}'",
                    target_label, req.raw_prompt
                ),
                pros: vec![
                    "Thực thi nhanh (<100ms)".to_string(),
                    "Phạm vi ảnh hưởng giới hạn, an toàn tuyệt đối".to_string(),
                ],
                cons: vec!["Không tạo cấu trúc mới cho các màn hình liên quan".to_string()],
                preview_payload: Some(serde_json::json!({
                    "mode": "minimal",
                    "target": target_label,
                    "scope": "local"
                })),
            },
            ClarificationVariantDto {
                id: "var-2-balanced".to_string(),
                title: format!("Phương án 2: Tiêu chuẩn Cân bằng (Balanced) — {target_label}"),
                description: format!(
                    "Chuẩn hóa thành phần theo DESIGN.md và chính sách {} cho '{}'",
                    styling_policy, req.raw_prompt
                ),
                pros: vec![
                    "Đồng bộ 100% token thiết kế và màu sắc hệ thống".to_string(),
                    "Tự động tương thích các viewport Desktop/Tablet/Mobile".to_string(),
                ],
                cons: vec!["Cần kiểm tra lại bố cục responsive đa khung nhìn".to_string()],
                preview_payload: Some(serde_json::json!({
                    "mode": "balanced",
                    "stylingPolicy": styling_policy,
                    "scope": "component"
                })),
            },
            ClarificationVariantDto {
                id: "var-3-modular".to_string(),
                title: format!("Phương án 3: Mô-đun hóa Toàn diện (Modular) — {target_label}"),
                description: format!(
                    "Tách thành component con độc lập với đầy đủ 4 Sub-States cho '{}'",
                    req.raw_prompt
                ),
                pros: vec![
                    "Khả năng mở rộng tối đa cho luồng nghiệp vụ".to_string(),
                    "Hỗ trợ 4 trạng thái (Default, Loading, Empty, Error)".to_string(),
                ],
                cons: vec!["Tăng số lượng component trong codebase dự án".to_string()],
                preview_payload: Some(serde_json::json!({
                    "mode": "modular",
                    "substates": ["DEFAULT", "LOADING", "EMPTY", "ERROR"],
                    "scope": "global"
                })),
            },
        ];

        let g_questions = vec![
            GapQuestionDto {
                id: "gap-q-layout".to_string(),
                question: "Bố cục hiển thị thành phần (Layout Presentation)".to_string(),
                category: "layout".to_string(),
                options: vec![
                    GapQuestionOptionDto {
                        id: "opt-drawer".to_string(),
                        label: "RightDrawer (Khuyến nghị)".to_string(),
                        description: Some("Ngăn kéo trượt từ cạnh phải, giữ ngữ cảnh màn hình chính".to_string()),
                        preview_thumbnail: Some("layout-drawer.svg".to_string()),
                    },
                    GapQuestionOptionDto {
                        id: "opt-modal".to_string(),
                        label: "Modal Popup trung tâm".to_string(),
                        description: Some("Hộp thoại đè giữa màn hình với nền mờ backdrop".to_string()),
                        preview_thumbnail: Some("layout-modal.svg".to_string()),
                    },
                    GapQuestionOptionDto {
                        id: "opt-inline".to_string(),
                        label: "Mở rộng nội dòng (Inline Accordion)".to_string(),
                        description: Some("Mở rộng trực tiếp tại vị trí dòng/thẻ đang chọn".to_string()),
                        preview_thumbnail: Some("layout-inline.svg".to_string()),
                    },
                ],
                default_option_id: Some("opt-drawer".to_string()),
            },
            GapQuestionDto {
                id: "gap-q-empty-state".to_string(),
                question: "Trạng thái khi chưa có dữ liệu (Empty State Behavior)".to_string(),
                category: "interaction".to_string(),
                options: vec![
                    GapQuestionOptionDto {
                        id: "opt-empty-cta".to_string(),
                        label: "Minh họa kèm nút Tạo Mới (CTA Button)".to_string(),
                        description: Some("Khuyến khích hành động ban đầu của người dùng".to_string()),
                        preview_thumbnail: Some("empty-cta.svg".to_string()),
                    },
                    GapQuestionOptionDto {
                        id: "opt-empty-ghost".to_string(),
                        label: "Khung giữ chỗ nhẹ (Ghost Skeleton)".to_string(),
                        description: Some("Hiển thị khung viền mờ tối giản".to_string()),
                        preview_thumbnail: Some("empty-ghost.svg".to_string()),
                    },
                ],
                default_option_id: Some("opt-empty-cta".to_string()),
            },
            GapQuestionDto {
                id: "gap-q-scope".to_string(),
                question: "Phạm vi kế thừa Shared AppShell".to_string(),
                category: "scope".to_string(),
                options: vec![
                    GapQuestionOptionDto {
                        id: "opt-scope-local".to_string(),
                        label: "Chỉ áp dụng trên màn hình này".to_string(),
                        description: Some("Không ảnh hưởng đến thanh điều hướng hay bố cục chung".to_string()),
                        preview_thumbnail: Some("scope-local.svg".to_string()),
                    },
                    GapQuestionOptionDto {
                        id: "opt-scope-shared".to_string(),
                        label: "Kế thừa Shared AppShell cho toàn bộ Portal Group".to_string(),
                        description: Some("Đồng bộ thanh Sidebar, Topbar và Footer của Portal".to_string()),
                        preview_thumbnail: Some("scope-shared.svg".to_string()),
                    },
                ],
                default_option_id: Some("opt-scope-local".to_string()),
            },
        ];

        (Some(vars), Some(g_questions))
    } else {
        (None, None)
    };

    // Enforce ambiguity_score >= 0.45 (409 CLARIFICATION_REQUIRED) before mutation
    if req.execute_immediately == Some(true) && is_ambiguous {
        if let Some(ref ikey) = idempotency_key {
            if let Ok(conn) = project_registry.pool().connection() {
                let _ = conn.execute(
                    "DELETE FROM idempotency_records WHERE tenant_id = ?1 AND actor_id = ?2 AND idempotency_key = ?3 AND status = 'IN_PROGRESS'",
                    params![&tenant_id, &actor_id, ikey],
                );
            }
        }
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "code": "CLARIFICATION_REQUIRED",
                "error": "CLARIFICATION_REQUIRED",
                "message": format!(
                    "Prompt ambiguity score ({:.2}) >= 0.45. Clarification is required before mutation.",
                    ambiguity_score
                ),
                "ambiguityScore": ambiguity_score,
                "routingMode": routing_mode,
                "clarificationVariants": variants,
                "gapQuestions": gap_questions,
            })),
        )
            .into_response());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let intent_id = Uuid::new_v4();

    let mut key_principles = vec![
        format!("Raw Prompt: {}", req.raw_prompt),
        format!("Ambiguity: {:.2}", ambiguity_score),
        format!("RoutingMode: {routing_mode}"),
        format!("ProjectNodesCount: {}", existing_nodes.len()),
        format!("ProjectEdgesCount: {existing_edges_count}"),
    ];
    for c in &active_constraints {
        key_principles.push(format!(
            "ActiveConstraint: {} ({:?})",
            c.rule_code, c.severity
        ));
    }

    let design_intent = domain::DesignIntentV1 {
        id: intent_id,
        project_id,
        visual_style,
        color_palette,
        typography: typography_fonts,
        key_principles,
    };

    let target_node_ids = if let Some((nid, ..)) = matched_node {
        vec![nid.clone()]
    } else {
        req.target_node_id.clone().into_iter().collect()
    };
    let target_screen_keys = matched_node
        .map(|(_, sk, ..)| vec![sk.clone()])
        .unwrap_or_default();
    let target_scope = TargetScopeDto {
        node_ids: target_node_ids,
        screen_keys: target_screen_keys,
        components: vec![],
    };

    let status = if is_ambiguous {
        "clarifying".to_string()
    } else {
        "ready".to_string()
    };

    let resp = SdiCompileResponse {
        id: intent_id,
        tenant_id: tenant_id.clone(),
        project_id,
        raw_prompt: req.raw_prompt,
        intent_type: "create_screen".to_string(),
        routing_mode: routing_mode.to_string(),
        target_scope,
        ambiguity_score,
        risk_score,
        clarification_variants: variants,
        gap_questions,
        status,
        revision: 1,
        created_at: now.clone(),
        updated_at: now,
        design_intent_v1: Some(design_intent),
    };

    let resp_to_store = resp.clone();
    let resp_json_for_tx2 = serde_json::to_string(&resp).unwrap_or_default();
    let ikey_for_tx2 = idempotency_key.clone();
    let tenant_for_tx2 = tenant_id.clone();
    let actor_for_tx2 = actor_id.clone();
    let reg_for_store = project_registry.clone();
    let commit_now_ms = chrono::Utc::now().timestamp_millis();
    tokio::task::spawn_blocking(move || {
        let scope_json =
            serde_json::to_string(&resp_to_store.target_scope).unwrap_or_else(|_| "{}".to_string());
        let variants_json = resp_to_store
            .clarification_variants
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        let gap_q_json = resp_to_store
            .gap_questions
            .as_ref()
            .and_then(|g| serde_json::to_string(g).ok());
        let design_intent_json = resp_to_store
            .design_intent_v1
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());

        let mut conn = reg_for_store.pool().connection()?;
        retry_on_busy("insert_sdi_intent", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO sdi_intents (
                    id, tenant_id, project_id, raw_prompt, intent_type, routing_mode, target_scope_json,
                    ambiguity_score, risk_score, variants_json, gap_questions_json, selected_variant_id,
                    status, design_intent_json, revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, 1, ?14, ?14)",
                params![
                    resp_to_store.id.to_string(),
                    resp_to_store.tenant_id,
                    resp_to_store.project_id.to_string(),
                    resp_to_store.raw_prompt,
                    resp_to_store.intent_type,
                    resp_to_store.routing_mode,
                    scope_json,
                    resp_to_store.ambiguity_score,
                    resp_to_store.risk_score,
                    variants_json,
                    gap_q_json,
                    resp_to_store.status,
                    design_intent_json,
                    resp_to_store.created_at,
                ],
            )?;
            if let Some(ref ikey) = ikey_for_tx2 {
                let updated = tx.execute(
                    "UPDATE idempotency_records
                     SET status = 'COMPLETED', response_status = ?1, response_body_json = ?2
                     WHERE tenant_id = ?3 AND actor_id = ?4 AND idempotency_key = ?5
                       AND status = 'IN_PROGRESS' AND lease_expires_at >= ?6",
                    params![201u16, resp_json_for_tx2, tenant_for_tx2, actor_for_tx2, ikey, commit_now_ms],
                )?;
                if updated == 0 {
                    return Err(rusqlite::Error::StatementChangedRows(0));
                }
            }
            tx.commit()
        })
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    Ok((StatusCode::CREATED, Json(resp)))
}

pub(super) async fn handle_list_sdi_intents(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<SdiCompileResponse>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    verify_sdi_project_tenant(&state.registry, project_id, caller.as_deref())?;

    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_sdi_tenant_id(&state.registry, project_id, caller.as_deref());

    let items = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tenant_id, project_id, raw_prompt, intent_type, target_scope_json,
                        ambiguity_score, risk_score, variants_json, status, design_intent_json,
                        created_at, updated_at, routing_mode, gap_questions_json, revision
                 FROM sdi_intents
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_list_sdi_intents"))?;
        let rows = stmt
            .query_map(
                params![project_id.to_string(), tenant_id],
                map_sdi_intent_row,
            )
            .map_err(|e| map_sqlite_error(e, "query_list_sdi_intents"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| map_sqlite_error(e, "row_list_sdi_intents"))?);
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

    Ok(Json(items))
}

pub(super) async fn handle_get_sdi_intent(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath((id, intent_id_raw)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<SdiCompileResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    verify_sdi_project_tenant(&state.registry, project_id, caller.as_deref())?;

    let intent_id = parse_project_uuid(&intent_id_raw).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_sdi_tenant_id(&state.registry, project_id, caller.as_deref());

    let found = tokio::task::spawn_blocking(move || {
        let conn = project_registry.pool().connection()?;
        conn.query_row(
            "SELECT id, tenant_id, project_id, raw_prompt, intent_type, target_scope_json,
                    ambiguity_score, risk_score, variants_json, status, design_intent_json,
                    created_at, updated_at, routing_mode, gap_questions_json, revision
             FROM sdi_intents
             WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
            params![intent_id.to_string(), project_id.to_string(), tenant_id],
            map_sdi_intent_row,
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "get_sdi_intent"))
    })
    .await
    .map_err(|e| {
        map_error_to_response(HarnessError::ValidationFailed {
            reason: format!("BLOCKING_TASK_FAILED: {e}"),
        })
    })?
    .map_err(map_error_to_response)?;

    match found {
        Some(item) => Ok(Json(item)),
        None => Err(intent_not_found(intent_id)),
    }
}

pub(super) async fn handle_sdi_clarification(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SdiClarificationRequest>,
) -> Result<Json<SdiCompileResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    verify_sdi_project_tenant(&state.registry, project_id, caller.as_deref())?;

    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;
    let tenant_id = resolve_sdi_tenant_id(&state.registry, project_id, caller.as_deref());

    let intent_id = req.intent_id;
    let selected_variant_id = req.selected_variant_id.trim().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = project_registry.pool().connection()?;
        let existing = conn
            .query_row(
                "SELECT id, tenant_id, project_id, raw_prompt, intent_type, target_scope_json,
                        ambiguity_score, risk_score, variants_json, status, design_intent_json,
                        created_at, updated_at, routing_mode, gap_questions_json, revision
                 FROM sdi_intents
                 WHERE id = ?1 AND project_id = ?2 AND tenant_id = ?3",
                params![intent_id.to_string(), project_id.to_string(), tenant_id],
                map_sdi_intent_row,
            )
            .optional()
            .map_err(|e| map_sqlite_error(e, "select_sdi_clarification"))?;

        let Some(mut intent) = existing else {
            return Ok::<_, HarnessError>(Err(intent_not_found(intent_id)));
        };

        let valid_variant = intent
            .clarification_variants
            .as_ref()
            .is_some_and(|list| list.iter().any(|v| v.id == selected_variant_id));
        if !valid_variant {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "code": "INVALID_VARIANT_ID",
                    "error": "INVALID_VARIANT_ID",
                    "message": format!("Variant '{selected_variant_id}' is not valid for intent '{intent_id}'"),
                })),
            )
                .into_response()));
        }

        let new_routing_mode = if selected_variant_id == "var-1-minimal" {
            "direct_edit".to_string()
        } else {
            "full_plan".to_string()
        };

        intent.status = "ready".to_string();
        intent.routing_mode = new_routing_mode.clone();
        intent.revision += 1;
        intent.updated_at = now.clone();
        if let Some(ref mut di) = intent.design_intent_v1 {
            let marker = format!("Selected Variant: {selected_variant_id}");
            if !di.key_principles.contains(&marker) {
                di.key_principles.push(marker);
            }
        }
        let updated_di_json = intent
            .design_intent_v1
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());

        retry_on_busy("update_sdi_clarification", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE sdi_intents
                 SET selected_variant_id = ?1,
                     status = 'ready',
                     routing_mode = ?2,
                     design_intent_json = COALESCE(?3, design_intent_json),
                     revision = revision + 1,
                     updated_at = ?4
                 WHERE id = ?5 AND project_id = ?6 AND tenant_id = ?7",
                params![
                    selected_variant_id,
                    new_routing_mode,
                    updated_di_json,
                    now,
                    intent_id.to_string(),
                    project_id.to_string(),
                    tenant_id,
                ],
            )?;
            tx.commit()
        })?;

        Ok(Ok(intent))
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
