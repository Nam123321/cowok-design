// @story 01.3

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use domain::{ContextSliceRequest, Department, HarnessError, Portal};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
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
    pub target_scope: TargetScopeDto,
    pub ambiguity_score: f64,
    pub risk_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_variants: Option<Vec<ClarificationVariantDto>>,
    pub status: String,
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

    let target_scope = serde_json::from_str(&scope_json).unwrap_or(TargetScopeDto {
        node_ids: vec![],
        screen_keys: vec![],
        components: vec![],
    });
    let clarification_variants = variants_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<ClarificationVariantDto>>(raw).ok());
    let design_intent_v1 = design_intent_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<domain::DesignIntentV1>(raw).ok());

    Ok(SdiCompileResponse {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        tenant_id,
        project_id: Uuid::parse_str(&project_id_str).unwrap_or_else(|_| Uuid::nil()),
        raw_prompt,
        intent_type,
        target_scope,
        ambiguity_score,
        risk_score,
        clarification_variants,
        status,
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

pub(super) async fn handle_sdi_intent(
    State(state): State<DaemonAppState>,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SdiCompileRequest>,
) -> Result<(StatusCode, Json<SdiCompileResponse>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_with_caller(&state.registry, project_id, &headers, caller.as_deref())
            .map_err(map_error_to_response)?;

    let tenant_id = resolve_sdi_tenant_id(&state.registry, project_id, caller.as_deref());

    let slicer = spec_registry::context_slicer::ContextSlicer::new(&state.registry);
    let active_constraints = slicer
        .load_active_constraints(project_id)
        .unwrap_or_default();

    let lower = req.raw_prompt.to_lowercase();
    let has_vague = lower.contains("đẹp")
        || lower.contains("fix")
        || lower.contains("sửa")
        || lower.contains("change")
        || lower.contains("update")
        || lower.contains("tối ưu")
        || lower.contains("cải tiến")
        || lower.contains("giao diện");
    let has_specific = lower.contains("node")
        || lower.contains("screen")
        || lower.contains("button")
        || lower.contains("header")
        || lower.contains("table")
        || lower.contains("drawer")
        || lower.contains("input")
        || lower.contains("auth")
        || req.target_node_id.is_some();

    let ambiguity_score = if has_vague && !has_specific {
        0.55
    } else if has_specific {
        0.20
    } else {
        0.40
    };
    let risk_score = 0.25;
    let is_ambiguous = ambiguity_score >= 0.45;

    let variants = if is_ambiguous {
        Some(vec![
            ClarificationVariantDto {
                id: "var-1-minimal".to_string(),
                title: "Phương án 1: Cục bộ & Tối giản (Minimal)".to_string(),
                description: format!(
                    "Áp dụng thay đổi giới hạn ngay trên thành phần cho '{}'",
                    req.raw_prompt
                ),
                pros: vec!["Thực thi nhanh (<100ms)".to_string()],
                cons: vec!["Không mở rộng dài hạn".to_string()],
            },
            ClarificationVariantDto {
                id: "var-2-balanced".to_string(),
                title: "Phương án 2: Tiêu chuẩn Cân bằng (Balanced)".to_string(),
                description: format!(
                    "Điều chỉnh bố cục theo chuẩn DESIGN.md cho '{}'",
                    req.raw_prompt
                ),
                pros: vec!["Đồng bộ 100% token thiết kế".to_string()],
                cons: vec!["Cần kiểm tra lại các viewport".to_string()],
            },
            ClarificationVariantDto {
                id: "var-3-modular".to_string(),
                title: "Phương án 3: Mô-đun hóa Toàn diện (Modular)".to_string(),
                description: format!("Tách thành component con độc lập cho '{}'", req.raw_prompt),
                pros: vec!["Khả năng mở rộng tối đa".to_string()],
                cons: vec!["Tăng số lượng file mã nguồn".to_string()],
            },
        ])
    } else {
        None
    };

    let now = chrono::Utc::now().to_rfc3339();
    let intent_id = Uuid::new_v4();

    let mut key_principles = vec![
        format!("Raw Prompt: {}", req.raw_prompt),
        format!("Ambiguity: {:.2}", ambiguity_score),
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
        visual_style: "clean-modern".to_string(),
        color_palette: vec![
            "#3B82F6".to_string(),
            "#10B981".to_string(),
            "#F59E0B".to_string(),
        ],
        typography: vec!["Inter".to_string(), "sans-serif".to_string()],
        key_principles,
    };

    let target_scope = TargetScopeDto {
        node_ids: req.target_node_id.clone().into_iter().collect(),
        screen_keys: vec![],
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
        target_scope,
        ambiguity_score,
        risk_score,
        clarification_variants: variants,
        status,
        created_at: now.clone(),
        updated_at: now,
        design_intent_v1: Some(design_intent),
    };

    let resp_to_store = resp.clone();
    tokio::task::spawn_blocking(move || {
        let scope_json = serde_json::to_string(&resp_to_store.target_scope).unwrap_or_else(|_| "{}".to_string());
        let variants_json = resp_to_store
            .clarification_variants
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        let design_intent_json = resp_to_store
            .design_intent_v1
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());

        let mut conn = project_registry.pool().connection()?;
        retry_on_busy("insert_sdi_intent", || {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO sdi_intents (
                    id, tenant_id, project_id, raw_prompt, intent_type, target_scope_json,
                    ambiguity_score, risk_score, variants_json, selected_variant_id,
                    status, design_intent_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11, ?12, ?13)",
                params![
                    resp_to_store.id.to_string(),
                    resp_to_store.tenant_id,
                    resp_to_store.project_id.to_string(),
                    resp_to_store.raw_prompt,
                    resp_to_store.intent_type,
                    scope_json,
                    resp_to_store.ambiguity_score,
                    resp_to_store.risk_score,
                    variants_json,
                    resp_to_store.status,
                    design_intent_json,
                    resp_to_store.created_at,
                    resp_to_store.updated_at,
                ],
            )?;
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
                        created_at, updated_at
                 FROM sdi_intents
                 WHERE project_id = ?1 AND tenant_id = ?2
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| map_sqlite_error(e, "prepare_list_sdi_intents"))?;
        let rows = stmt
            .query_map(params![project_id.to_string(), tenant_id], map_sdi_intent_row)
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
                    created_at, updated_at
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
                        created_at, updated_at
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

        intent.status = "ready".to_string();
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
                     design_intent_json = COALESCE(?2, design_intent_json),
                     updated_at = ?3
                 WHERE id = ?4 AND project_id = ?5 AND tenant_id = ?6",
                params![
                    selected_variant_id,
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
