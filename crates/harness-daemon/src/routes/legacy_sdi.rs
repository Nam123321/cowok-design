// @story 01.3

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use domain::{ContextSliceRequest, Department, HarnessError, Portal};
use serde::{Deserialize, Serialize};
use spec_registry::retry_on_busy;
use uuid::Uuid;

use super::*;

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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<PromptCompileRequest>,
) -> Result<Json<domain::CompiledPromptPayload>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;

    let (project_registry, _canon_project_root, is_superadmin) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ModelAdapterRequestDto>,
) -> Result<Json<domain::AdaptationResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;

    let (project_registry, _canon_project_root, _is_superadmin) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<LintSourceRequest>,
) -> Result<Json<LintSourceResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, canon_project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<domain::ConstraintProfile>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(profile): Json<domain::ConstraintProfile>,
) -> Result<Json<Vec<constraint_engine::ActiveConstraint>>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, _canon_project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<constraint_engine::IconManifest>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ValidateIconRequest>,
) -> Result<Json<constraint_engine::IconValidationReport>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ValidateTypographyRequest>,
) -> Result<Json<constraint_engine::TypographyValidationReport>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ValidateComponentRequest>,
) -> Result<Json<constraint_engine::ComponentValidationReport>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Json<domain::ProjectDesignContracts>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(contracts): Json<domain::ProjectDesignContracts>,
) -> Result<Json<domain::ProjectDesignContracts>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (project_registry, project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<constraint_engine::AstInjectionRequest>,
) -> Result<Json<constraint_engine::AstInjectionResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<constraint_engine::SurgicalPatchRequest>,
) -> Result<Json<constraint_engine::SurgicalPatchResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let _ = verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
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
    pub clarification_variants: Option<Vec<ClarificationVariantDto>>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
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

pub(super) async fn handle_sdi_intent(
    State(state): State<DaemonAppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SdiCompileRequest>,
) -> Result<(StatusCode, Json<SdiCompileResponse>), Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (_project_registry, _canon_project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    let tenant_id = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default-tenant".to_string());

    let slicer = spec_registry::context_slicer::ContextSlicer::new(&state.registry);
    let active_constraints = slicer
        .load_active_constraints(project_id)
        .unwrap_or_default();

    let lower = req.raw_prompt.to_lowercase();
    let has_vague = lower.contains("đẹp")
        || lower.contains("fix")
        || lower.contains("sửa")
        || lower.contains("change")
        || lower.contains("update");
    let has_specific = lower.contains("node")
        || lower.contains("screen")
        || lower.contains("button")
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

    let resp = SdiCompileResponse {
        id: intent_id,
        tenant_id,
        project_id,
        raw_prompt: req.raw_prompt,
        intent_type: "create_screen".to_string(),
        target_scope,
        ambiguity_score,
        risk_score,
        clarification_variants: variants,
        status: if is_ambiguous {
            "clarifying".to_string()
        } else {
            "ready".to_string()
        },
        created_at: now.clone(),
        updated_at: now,
        design_intent_v1: Some(design_intent),
    };

    Ok((StatusCode::CREATED, Json(resp)))
}

pub(super) async fn handle_sdi_clarification(
    State(state): State<DaemonAppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<SdiClarificationRequest>,
) -> Result<Json<SdiCompileResponse>, Response> {
    let project_id = parse_project_uuid(&id).map_err(map_error_to_response)?;
    let (_project_registry, _canon_project_root, _) =
        verify_anti_idor_and_resolve_registry(&state.registry, project_id, &headers)
            .map_err(map_error_to_response)?;

    let valid_variants = ["var-1-minimal", "var-2-balanced", "var-3-modular"];
    if !valid_variants.contains(&req.selected_variant_id.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "INVALID_VARIANT_ID",
                "message": format!("Variant '{}' is not recognized", req.selected_variant_id),
            })),
        )
            .into_response());
    }

    let tenant_id = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default-tenant".to_string());

    let target_scope = TargetScopeDto {
        node_ids: vec![],
        screen_keys: vec![],
        components: vec![],
    };

    let now = chrono::Utc::now().to_rfc3339();
    let resp = SdiCompileResponse {
        id: req.intent_id,
        tenant_id,
        project_id,
        raw_prompt: "Clarified intent".to_string(),
        intent_type: "create_screen".to_string(),
        target_scope,
        ambiguity_score: 0.20,
        risk_score: 0.25,
        clarification_variants: None,
        status: "ready".to_string(),
        created_at: now.clone(),
        updated_at: now,
        design_intent_v1: Some(domain::DesignIntentV1 {
            id: req.intent_id,
            project_id,
            visual_style: "clean-modern".to_string(),
            color_palette: vec!["#3B82F6".to_string(), "#10B981".to_string()],
            typography: vec!["Inter".to_string()],
            key_principles: vec![format!("Selected Variant: {}", req.selected_variant_id)],
        }),
    };

    Ok(Json(resp))
}
