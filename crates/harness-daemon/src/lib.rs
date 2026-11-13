// @story 01.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10
#![allow(clippy::result_large_err)]
//! Harness Daemon entrypoint, loopback Axum control plane, `.cowok-design/daemon.lock`
//! dynamic port discovery, and 9-Spec Synthesis Pipeline for `cowok-design`.

pub mod daemon_auth;
pub mod flow_routes;
pub mod lockfile;
pub mod routes;
pub mod synthesis;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use domain::{CanonicalSpecKey, HarnessError};
use spec_registry::{retry_on_busy, SpecRegistry, SqlitePool, SqliteWalConfig};
use uuid::Uuid;

pub use crate::lockfile::{
    bind_loopback_with_lockfile, read_daemon_lock, validate_and_canonicalize_workspace_root,
    validate_loopback_host, DaemonConfig, DaemonLockInfo, DEFAULT_DAEMON_PORT,
};
pub use crate::routes::{
    build_router, create_project_isolated, ensure_ac06_tables, map_error_to_response,
    ChatSessionDto, CreateChatSessionRequest, CreateProjectRequest, DaemonAppState,
    GitHubPRRequest, GitHubPRResponseDto, HealthResponse, HttpErrorBody, IssuePathTokenRequest,
    LintSourceRequest, LintSourceResponse, ModelAdapterRequestDto, PathTokenPayload,
    PathTokenResponse, ProjectBoundaryDto, ProjectDto, ProjectSpecsListResponse,
    PromoteScratchRequest, PromptCompileRequest, PutSpecRequest, SpecItemWithDriftDto,
    UpdateChatSessionRequest, ValidateComponentRequest, ValidateIconRequest,
    ValidateTypographyRequest, VerifyPathTokenRequest,
};
pub use crate::synthesis::{
    count_provenance_markers, ensure_project_token, resolve_project_scoped_registry,
    synthesize_nine_specs, validate_seed_repo_path, SynthesizeSpecsRequest,
    SynthesizeSpecsResponse, SynthesizedSpecItemDto, SPARSE_BRIEF_MIN_CHARS,
};

/// High-assurance loopback Axum Daemon Server for `cowok-design`.
pub struct DaemonServer {
    listener: std::net::TcpListener,
    lock_info: DaemonLockInfo,
    lock_path: PathBuf,
    router: Router,
}

impl DaemonServer {
    /// Binds on `127.0.0.1` (falling back to a dynamic loopback port on collision),
    /// writes `.cowok-design/daemon.lock` atomically, and builds the Axum router.
    pub fn bind_loopback(
        config: &DaemonConfig,
        registry: Arc<SpecRegistry>,
    ) -> Result<Self, HarnessError> {
        let (listener, lock_info, lock_path) = bind_loopback_with_lockfile(config)?;
        let auth = crate::daemon_auth::DaemonAuthState::from_lock_info(&lock_info)?;
        let state =
            DaemonAppState::with_auth(registry, lock_info.host.clone(), lock_info.port, auth);
        let router = build_router(state);
        Ok(Self {
            listener,
            lock_info,
            lock_path,
            router,
        })
    }

    pub fn lock_info(&self) -> &DaemonLockInfo {
        &self.lock_info
    }

    pub fn lock_path(&self) -> &PathBuf {
        &self.lock_path
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn into_parts(self) -> (std::net::TcpListener, DaemonLockInfo, PathBuf, Router) {
        (self.listener, self.lock_info, self.lock_path, self.router)
    }

    /// Serves the bound loopback listener using Axum's async server loop.
    pub async fn serve(self) -> Result<(), HarnessError> {
        let tokio_listener = tokio::net::TcpListener::from_std(self.listener).map_err(|e| {
            HarnessError::ValidationFailed {
                reason: format!("DAEMON_TOKIO_LISTENER_FAILED: {e}"),
            }
        })?;
        axum::serve(tokio_listener, self.router)
            .await
            .map_err(|e| HarnessError::ValidationFailed {
                reason: format!("DAEMON_SERVE_FAILED: {e}"),
            })
    }
}

/// Typed Client SDK (`DaemonHttpClient`) for invoking the `cowok-design` harness daemon
/// without raw unvalidated HTTP calls.
#[derive(Clone)]
pub struct DaemonHttpClient {
    state: DaemonAppState,
}

impl DaemonHttpClient {
    pub fn new(registry: Arc<SpecRegistry>, host: String, port: u16) -> Self {
        Self {
            state: DaemonAppState::new(registry, host, port),
        }
    }

    pub fn health(&self) -> HealthResponse {
        HealthResponse {
            status: "ok".to_string(),
            service: "cowok-design-harness-daemon".to_string(),
            host: self.state.host.clone(),
            port: self.state.port,
            pid: std::process::id(),
            daemon_nonce: String::new(),
            schema_version: crate::lockfile::CURRENT_SCHEMA_VERSION,
        }
    }

    pub fn create_project(&self, req: &CreateProjectRequest) -> Result<ProjectDto, HarnessError> {
        create_project_isolated(&self.state.registry, req)
    }

    pub fn synthesize_specs(
        &self,
        project_id: Uuid,
        req: &SynthesizeSpecsRequest,
    ) -> Result<SynthesizeSpecsResponse, HarnessError> {
        synthesize_nine_specs(&self.state.registry, project_id, req)
    }

    pub fn get_spec(
        &self,
        project_id: Uuid,
        spec_key: CanonicalSpecKey,
    ) -> Result<Option<spec_registry::SpecRecord>, HarnessError> {
        let (project_registry, _) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        project_registry.get_spec(project_id, spec_key)
    }

    pub fn slice_context(
        &self,
        project_id: Uuid,
        req: &domain::ContextSliceRequest,
    ) -> Result<domain::AuthorizedContextSlice, HarnessError> {
        let (project_registry, _) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let slicer = spec_registry::ContextSlicer::new(&project_registry);
        slicer.slice_context(project_id, req)
    }

    pub fn compile_prompt(
        &self,
        project_id: Uuid,
        req: &PromptCompileRequest,
    ) -> Result<domain::CompiledPromptPayload, HarnessError> {
        if let Some(c) = req.token_ceiling {
            if c > domain::TOKEN_BUDGET_CEILING {
                return Err(HarnessError::ValidationFailed {
                    reason: format!(
                        "TOKEN_CEILING_EXCEEDED: Requested ceiling {} exceeds maximum {}",
                        c,
                        domain::TOKEN_BUDGET_CEILING
                    ),
                });
            }
        }

        let (project_registry, _) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let slicer = spec_registry::ContextSlicer::new(&project_registry);

        let context_slice = match &req.slice_request {
            Some(slice_req) => Some(slicer.slice_context(project_id, slice_req)?),
            None => None,
        };

        let mut all_constraints = req.project_constraints.clone().unwrap_or_default();
        let active = slicer.load_active_constraints(project_id)?;
        for c in active {
            all_constraints.push(format!("{}: {}", c.rule_code, c.config_json));
        }

        let input = domain::CompilationInput {
            project_id: project_id.to_string(),
            system_instruction: req
                .system_instruction
                .clone()
                .unwrap_or_else(|| "You are an autonomous AI coding assistant.".into()),
            project_constraints: all_constraints,
            context_slice,
            task_instructions: req.task_instructions.clone(),
            optional_components: req.optional_components.clone().unwrap_or_default(),
        };

        let ceiling = req.token_ceiling.unwrap_or(domain::TOKEN_BUDGET_CEILING);
        let compiler = domain::DeterministicPromptCompiler::new(ceiling);
        compiler.compile(input)
    }

    /// Adapts model capabilities across 3 profiles (Profile A, Profile B, Profile C)
    pub fn adapt_model_capabilities(
        &self,
        project_id: Uuid,
        req: &ModelAdapterRequestDto,
    ) -> Result<domain::AdaptationResponse, HarnessError> {
        let (project_registry, _) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let slicer = spec_registry::ContextSlicer::new(&project_registry);
        let active_constraints = slicer.load_active_constraints(project_id)?;
        let disallowed_profiles =
            extract_disallowed_profiles(&req.disallowed_profiles, &active_constraints);

        let adapt_req = domain::AdaptationRequest {
            project_id,
            requested_profile: req.requested_profile,
            raw_system_prompt: req.raw_system_prompt.clone(),
            tools: req.tools.clone(),
            disallowed_profiles,
            syntax_error_reported: req.syntax_error_reported,
            max_budget_override: req.max_budget_override,
        };
        let adapter = domain::ModelCapabilityAdapter::new();
        adapter.adapt(&adapt_req)
    }

    /// Runs AST and lexical linting on JSX/HTML source under the project's active constraints
    /// and persisted icon/typography/component design contracts.
    pub fn lint_source(
        &self,
        project_id: Uuid,
        source: &str,
    ) -> Result<LintSourceResponse, HarnessError> {
        let (project_registry, canon_project_root) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
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
            source,
            &constraints,
            Some(&icon_manifest),
            Some(&typography_contract),
            Some(&component_registry),
        )?;
        let passed = violations.is_empty();
        Ok(LintSourceResponse { violations, passed })
    }

    /// Retrieves the domain ConstraintProfile for a project.
    pub fn get_constraint_profile(
        &self,
        project_id: Uuid,
    ) -> Result<domain::ConstraintProfile, HarnessError> {
        let (project_registry, _) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
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
        Ok(domain_profile)
    }

    /// Sets/updates constraints from a domain ConstraintProfile for a project.
    pub fn set_constraint_profile(
        &self,
        project_id: Uuid,
        profile: &domain::ConstraintProfile,
    ) -> Result<Vec<constraint_engine::ActiveConstraint>, HarnessError> {
        let (project_registry, _) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let active =
            constraint_engine::ConstraintEvaluator::active_constraints_from_domain(profile);
        let mut conn = project_registry.pool().connection()?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let prof_id_str = profile.id.to_string();
        let proj_id_str = project_id.to_string();

        let forbid_tailwind = if profile
            .forbidden_patterns
            .iter()
            .any(|p| p.to_uppercase().contains("TAILWIND"))
        {
            1
        } else {
            0
        };
        let forbid_raw_hex = if profile
            .forbidden_patterns
            .iter()
            .any(|p| p.to_uppercase().contains("HEX"))
        {
            1
        } else {
            0
        };

        retry_on_busy("set_constraint_profile_tx", || {
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
                "DELETE FROM constraint_rules WHERE profile_id = ?1 AND UPPER(rule_code) != 'ENFORCE_COMPONENT_REGISTRY'",
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
        Ok(active)
    }

    /// Retrieves the fail-closed IconManifest for a project (or flagship when set to builtin://flagship).
    pub fn get_icon_manifest(
        &self,
        project_id: Uuid,
    ) -> Result<constraint_engine::IconManifest, HarnessError> {
        let (project_registry, project_root) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let conn = project_registry.pool().connection()?;
        let (manifest, _, _) = constraint_engine::IconValidator::load_project_design_context(
            &conn,
            &project_root,
            &project_id.to_string(),
        )?;
        Ok(manifest)
    }

    /// Validates an icon name against project's icon manifest or flagship default.
    pub fn validate_icon(
        &self,
        project_id: Uuid,
        icon_name: &str,
    ) -> Result<constraint_engine::IconValidationReport, HarnessError> {
        let manifest = self.get_icon_manifest(project_id)?;
        Ok(constraint_engine::IconValidator::validate_icon(
            &manifest, icon_name,
        ))
    }

    /// Validates typography properties against the project's persisted TypographyContract.
    pub fn validate_typography(
        &self,
        project_id: Uuid,
        req: &ValidateTypographyRequest,
    ) -> Result<constraint_engine::TypographyValidationReport, HarnessError> {
        let (project_registry, project_root) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let conn = project_registry.pool().connection()?;
        let (_, typography_contract, _) =
            constraint_engine::IconValidator::load_project_design_context(
                &conn,
                &project_root,
                &project_id.to_string(),
            )?;
        Ok(
            constraint_engine::IconValidator::validate_typography_report(
                &typography_contract,
                &req.font_family,
                req.font_weight,
                req.font_size.as_deref(),
                req.contrast_ratio,
            ),
        )
    }

    /// Validates a JSX component identifier against the project's `07_COMPONENT_REGISTRY`.
    pub fn validate_component(
        &self,
        project_id: Uuid,
        component_name: &str,
    ) -> Result<constraint_engine::ComponentValidationReport, HarnessError> {
        let (project_registry, project_root) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let conn = project_registry.pool().connection()?;
        let (_, _, component_registry) =
            constraint_engine::IconValidator::load_project_design_context(
                &conn,
                &project_root,
                &project_id.to_string(),
            )?;
        Ok(constraint_engine::IconValidator::validate_component(
            &component_registry,
            component_name,
        ))
    }

    /// Retrieves persisted design contracts for a project.
    pub fn get_design_contracts(
        &self,
        project_id: Uuid,
    ) -> Result<domain::ProjectDesignContracts, HarnessError> {
        let (project_registry, project_root) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let conn = project_registry.pool().connection()?;
        constraint_engine::IconValidator::load_project_design_contracts(
            &conn,
            &project_root,
            &project_id.to_string(),
        )
    }

    /// Validates and persists updated design contracts for a project.
    pub fn set_design_contracts(
        &self,
        project_id: Uuid,
        contracts: &domain::ProjectDesignContracts,
    ) -> Result<domain::ProjectDesignContracts, HarnessError> {
        let (project_registry, project_root) =
            resolve_project_scoped_registry(&self.state.registry, project_id)?;
        let mut conn = project_registry.pool().connection()?;
        let proj_id_str = project_id.to_string();
        constraint_engine::IconValidator::save_project_design_contracts(
            &mut conn,
            &project_root,
            &proj_id_str,
            contracts,
        )?;
        constraint_engine::IconValidator::load_project_design_contracts(
            &conn,
            &project_root,
            &proj_id_str,
        )
    }

    /// Injects deterministic `data-ff-id` attributes into JSX elements.
    pub fn inject_ff_id(
        &self,
        _project_id: Uuid,
        source: &str,
        component_name: Option<&str>,
    ) -> Result<constraint_engine::AstInjectionResponse, HarnessError> {
        constraint_engine::SurgicalPatcher::inject_ff_ids(source, component_name)
    }

    /// Surgically patches text, metric, or attributes in <1ms without mutating surrounding AST.
    pub fn surgical_patch(
        &self,
        _project_id: Uuid,
        req: &constraint_engine::SurgicalPatchRequest,
    ) -> Result<constraint_engine::SurgicalPatchResponse, HarnessError> {
        constraint_engine::SurgicalPatcher::patch_by_ff_id(&req.source, req)
    }
}

/// Extracts disallowed model profiles from project constraints and base requested disallow list (F-01, F-07).
pub fn extract_disallowed_profiles(
    base: &[domain::ModelProfileTier],
    constraints: &[constraint_engine::ActiveConstraint],
) -> Vec<domain::ModelProfileTier> {
    let mut disallowed = base.to_vec();
    for c in constraints {
        let code = c.rule_code.to_uppercase();
        // 1. Direct rule_code matching
        if (code.contains("FORBID_FRONTIER")
            || code.contains("DISALLOW_FRONTIER")
            || code.contains("FORBID_PROFILE_A"))
            && !disallowed.contains(&domain::ModelProfileTier::ProfileA_Frontier)
        {
            disallowed.push(domain::ModelProfileTier::ProfileA_Frontier);
        }
        if (code.contains("FORBID_FAST_CODING")
            || code.contains("DISALLOW_FAST_CODING")
            || code.contains("FORBID_PROFILE_B"))
            && !disallowed.contains(&domain::ModelProfileTier::ProfileB_FastCoding)
        {
            disallowed.push(domain::ModelProfileTier::ProfileB_FastCoding);
        }
        if (code.contains("FORBID_COMPACT")
            || code.contains("DISALLOW_COMPACT")
            || code.contains("FORBID_PROFILE_C"))
            && !disallowed.contains(&domain::ModelProfileTier::ProfileC_Compact)
        {
            disallowed.push(domain::ModelProfileTier::ProfileC_Compact);
        }

        // 2. Structured JSON config matching
        if let Ok(json_cfg) = serde_json::from_str::<serde_json::Value>(&c.config_json) {
            // Check explicit forbid flags
            if (json_cfg
                .get("forbid_frontier")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || json_cfg
                    .get("forbid_profile_a")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                || json_cfg
                    .get("disallow_frontier")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false))
                && !disallowed.contains(&domain::ModelProfileTier::ProfileA_Frontier)
            {
                disallowed.push(domain::ModelProfileTier::ProfileA_Frontier);
            }
            if (json_cfg
                .get("forbid_fast_coding")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || json_cfg
                    .get("forbid_profile_b")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                || json_cfg
                    .get("disallow_fast_coding")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false))
                && !disallowed.contains(&domain::ModelProfileTier::ProfileB_FastCoding)
            {
                disallowed.push(domain::ModelProfileTier::ProfileB_FastCoding);
            }
            if (json_cfg
                .get("forbid_compact")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || json_cfg
                    .get("forbid_profile_c")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                || json_cfg
                    .get("disallow_compact")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false))
                && !disallowed.contains(&domain::ModelProfileTier::ProfileC_Compact)
            {
                disallowed.push(domain::ModelProfileTier::ProfileC_Compact);
            }

            // Check forbidden/disallowed profile lists
            for list_key in &[
                "disallowed_profiles",
                "forbidden_profiles",
                "disallowed",
                "forbidden",
            ] {
                if let Some(arr) = json_cfg.get(list_key).and_then(|v| v.as_array()) {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            let s_lower = s.to_ascii_lowercase();
                            if (s_lower == "frontier_reasoning"
                                || s_lower == "profilea_frontier"
                                || s_lower == "profile_a"
                                || s_lower == "frontier")
                                && !disallowed
                                    .contains(&domain::ModelProfileTier::ProfileA_Frontier)
                            {
                                disallowed.push(domain::ModelProfileTier::ProfileA_Frontier);
                            }
                            if (s_lower == "fast_coding"
                                || s_lower == "profileb_fastcoding"
                                || s_lower == "profile_b")
                                && !disallowed
                                    .contains(&domain::ModelProfileTier::ProfileB_FastCoding)
                            {
                                disallowed.push(domain::ModelProfileTier::ProfileB_FastCoding);
                            }
                            if (s_lower == "compact_motion"
                                || s_lower == "profilec_compact"
                                || s_lower == "profile_c"
                                || s_lower == "compact")
                                && !disallowed.contains(&domain::ModelProfileTier::ProfileC_Compact)
                            {
                                disallowed.push(domain::ModelProfileTier::ProfileC_Compact);
                            }
                        }
                    }
                }
            }
        }
    }
    disallowed
}

/// Reconciles live filesystem watchers for all active projects under `registry`, draining
/// pending `notify` events so external markdown edits/deletions stay synced with SQLite WAL.
pub fn reconcile_project_watchers(
    registry: &Arc<SpecRegistry>,
    watchers: &mut std::collections::HashMap<uuid::Uuid, spec_registry::SpecWatcher>,
) {
    let Ok(conn) = registry.pool().connection() else {
        return;
    };
    let Ok(mut stmt) = conn.prepare("SELECT id FROM projects WHERE trash_deleted_at IS NULL")
    else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
        return;
    };
    let mut active_ids = std::collections::HashSet::new();
    for id_str in rows.flatten() {
        if let Ok(pid) = uuid::Uuid::parse_str(&id_str) {
            active_ids.insert(pid);
            if !watchers.contains_key(&pid) {
                if let Ok((proj_reg, proj_root)) =
                    crate::synthesis::resolve_project_scoped_registry(registry, pid)
                {
                    if proj_root.join("product").join("specs").is_dir() {
                        if let Ok(w) = spec_registry::SpecWatcher::start(proj_reg, pid) {
                            watchers.insert(pid, w);
                        }
                    }
                }
            }
        }
    }
    watchers.retain(|pid, _| active_ids.contains(pid));
    for watcher in watchers.values() {
        let _ = watcher.drain_events();
    }
}

/// Opens the embedded SQLite WAL registry inside `config.workspace_root`, binds the loopback
/// Axum server, atomically writes `.cowok-design/daemon.lock`, and serves incoming HTTP requests.
pub async fn serve_daemon_with_config(config: DaemonConfig) -> Result<(), HarnessError> {
    let canonical_root = validate_and_canonicalize_workspace_root(&config.workspace_root)?;
    let db_path = canonical_root.join(".cowok-design/cowok-design.sqlite3");
    let pool = SqlitePool::open_wal(SqliteWalConfig::new(&db_path, &canonical_root))?;
    let registry = Arc::new(SpecRegistry::new(pool));
    let server = DaemonServer::bind_loopback(&config, Arc::clone(&registry))?;

    let watcher_registry = Arc::clone(&registry);
    let watcher_handle = tokio::spawn(async move {
        let mut watchers = std::collections::HashMap::new();
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            ticker.tick().await;
            let reg = Arc::clone(&watcher_registry);
            let updated = tokio::task::spawn_blocking(move || {
                reconcile_project_watchers(&reg, &mut watchers);
                watchers
            })
            .await;
            match updated {
                Ok(w) => watchers = w,
                Err(_) => break,
            }
        }
    });

    let serve_result = server.serve().await;
    watcher_handle.abort();
    serve_result
}

/// Starts and serves the `cowok-design` loopback Axum harness daemon on the target workspace.
pub fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = match std::env::var("UXUI_OS_WORKSPACE_ROOT") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => std::env::current_dir()?,
    };
    let config = DaemonConfig::loopback_default(workspace_root);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(serve_daemon_with_config(config))?;
    Ok(())
}
