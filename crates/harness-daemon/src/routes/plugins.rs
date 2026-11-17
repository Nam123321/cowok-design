// @story 08.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use chrono::Utc;
use futures_util::stream::Stream;
use plugin_host::{
    load_verified_staged_bundle_from_dir, manifest_to_skill_plugin, validate_safe_segment,
    validate_skill_frontmatter, ActorKind as DomainActorKind, CardDecisionAction, CardStatus,
    ConsumedNonceRecord, HumanSessionAuthority, KnowledgeCard, LearnCardCategory, LearnError,
    LearnPipeline, ManagedPlugin, MintTokenInput, PerRunStager, PluginLifecycleError, PluginState,
    ProposeCardInput, SkillManifest, SkillRepackager, TriangleLinter, VideoIntakeProcessor,
    MAX_LEARN_PROMPT_TOKENS,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spec_registry::plugin_store::{
    clear_auth_required_schedules_for_tenant, commit_learn_card_decision, find_plugin_for_tenant,
    get_cron_schedule_scoped, get_plugin_scoped, insert_learn_card_tx, list_consumed_nonces_scoped,
    list_learn_cards_scoped, list_plugins_for_tenant, update_plugin_cas, upsert_cron_schedule,
    upsert_plugin_record, verify_project_scope, AuditReceiptInput, CasFailure, ConsumedNonceRow,
    KnowledgeCardRow, PluginCronScheduleRow, SkillPluginRow,
};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::daemon_auth::{
    require_human_workbench_actor, FlowPrincipal, HttpActorKind, VerifiedActorContext,
};
use crate::routes::execution_coordinator_sql::{err_resp, run_blocking_resp};
use crate::routes::legacy_workbench_v6::validate_ssrf_safe_url;
use crate::routes::plugins_cron::{
    run_plugin_sync_cycle, snapshot_from_plugin_row, validate_cron_expression,
};
use crate::routes::{DaemonAppState, HttpErrorBody};
use crate::vault::VaultStore;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPluginsQuery {
    pub enabled: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummaryDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub plugin_slug: String,
    pub version: String,
    pub lifecycle_state: String,
    pub enabled: bool,
    pub source_type: String,
    pub source_repo_url: String,
    pub upstream_commit_sha: Option<String>,
    pub cron_schedule: Option<String>,
    pub revision: u64,
    pub breaking_change_isolated: bool,
    pub staging_version: Option<String>,
    pub staging_diff_reasons: Vec<String>,
    pub quarantined: bool,
    pub quarantine_reason: Option<String>,
    pub manifest_json: serde_json::Value,
    pub updated_at: String,
}

fn row_to_summary_dto(row: SkillPluginRow) -> PluginSummaryDto {
    let parsed_manifest = serde_json::from_str::<serde_json::Value>(&row.manifest_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    let diff_reasons = row
        .staging_diff_reasons_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default();
    let enabled = matches!(
        row.lifecycle_state.to_ascii_uppercase().as_str(),
        "ACTIVE" | "ENABLED"
    );
    let updated_at = chrono::DateTime::from_timestamp_millis(row.updated_at)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();
    PluginSummaryDto {
        id: row.id,
        tenant_id: row.tenant_id,
        project_id: row.project_id,
        plugin_slug: row.plugin_slug,
        version: row.version,
        lifecycle_state: row.lifecycle_state,
        enabled,
        source_type: row.source_type,
        source_repo_url: row.source_repo_url,
        upstream_commit_sha: row.upstream_commit_sha,
        cron_schedule: row.cron_schedule,
        revision: row.revision,
        breaking_change_isolated: row.breaking_change_isolated,
        staging_version: row.staging_version,
        staging_diff_reasons: diff_reasons,
        quarantined: row.quarantined,
        quarantine_reason: row.quarantine_reason,
        manifest_json: parsed_manifest,
        updated_at,
    }
}

fn corr_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn map_harness_to_resp(err: domain::HarnessError) -> Response {
    match err {
        domain::HarnessError::ProjectNotFound { project_id } => err_resp(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            format!("Project {project_id} not found for tenant"),
        ),
        domain::HarnessError::SqliteBusy { details } => {
            err_resp(StatusCode::SERVICE_UNAVAILABLE, "SQLITE_BUSY", details)
        }
        domain::HarnessError::BoundaryViolation { reason } => {
            err_resp(StatusCode::CONFLICT, "BOUNDARY_VIOLATION", reason)
        }
        domain::HarnessError::PathTraversalDenied { path } => {
            err_resp(StatusCode::BAD_REQUEST, "PATH_TRAVERSAL_DENIED", path)
        }
        other => err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            other.to_string(),
        ),
    }
}

fn map_cas_failure(f: CasFailure, entity: &str) -> Response {
    match f {
        CasFailure::NotFound => err_resp(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            format!("{entity} not found for tenant/project"),
        ),
        CasFailure::RevisionConflict { current_revision } => (
            StatusCode::CONFLICT,
            Json(HttpErrorBody {
                code: "CAS_REVISION_MISMATCH".to_string(),
                kind: Some("CAS_REVISION_MISMATCH".to_string()),
                message: format!(
                    "Revision conflict on {entity}: current_revision is {current_revision}"
                ),
                details: serde_json::json!({ "currentRevision": current_revision }),
                request_id: Uuid::new_v4().to_string(),
            }),
        )
            .into_response(),
    }
}

fn map_lifecycle_err(err: PluginLifecycleError) -> Response {
    match err {
        PluginLifecycleError::RevisionConflict { expected, actual } => (
            StatusCode::CONFLICT,
            Json(HttpErrorBody {
                code: "CAS_REVISION_MISMATCH".to_string(),
                kind: Some("CAS_REVISION_MISMATCH".to_string()),
                message: format!("Expected revision {expected}, got {actual}"),
                details: serde_json::json!({ "expected": expected, "currentRevision": actual }),
                request_id: Uuid::new_v4().to_string(),
            }),
        )
            .into_response(),
        PluginLifecycleError::QuarantinedReactivationForbidden { reason } => {
            err_resp(StatusCode::FORBIDDEN, "PLUGIN_QUARANTINED", reason)
        }
        other => err_resp(
            StatusCode::BAD_REQUEST,
            "LIFECYCLE_ERROR",
            other.to_string(),
        ),
    }
}

fn to_domain_actor_kind(kind: HttpActorKind) -> DomainActorKind {
    match kind {
        HttpActorKind::HumanWorkbenchSession => DomainActorKind::HumanWorkbenchSession,
        HttpActorKind::AgentHmac => DomainActorKind::AgentHmac,
        HttpActorKind::McpClient => DomainActorKind::McpClient,
        HttpActorKind::SuperadminCli => DomainActorKind::SuperadminCli,
    }
}

fn actor_kind_label(kind: HttpActorKind) -> &'static str {
    match kind {
        HttpActorKind::HumanWorkbenchSession => "HUMAN_WORKBENCH_SESSION",
        HttpActorKind::AgentHmac => "AGENT_HMAC",
        HttpActorKind::McpClient => "MCP_CLIENT",
        HttpActorKind::SuperadminCli => "SUPERADMIN_CLI",
    }
}

fn parse_card_category(raw: &str) -> Result<LearnCardCategory, Response> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "RULE" => Ok(LearnCardCategory::Rule),
        "ANTI_PATTERN" => Ok(LearnCardCategory::AntiPattern),
        "PATTERN" => Ok(LearnCardCategory::Pattern),
        "LESSON" => Ok(LearnCardCategory::Lesson),
        other => Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            format!("Invalid LEARN card category '{other}'"),
        )),
    }
}

fn xml_escape_attr(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn card_category_str(cat: LearnCardCategory) -> &'static str {
    match cat {
        LearnCardCategory::Rule => "RULE",
        LearnCardCategory::AntiPattern => "ANTI_PATTERN",
        LearnCardCategory::Pattern => "PATTERN",
        LearnCardCategory::Lesson => "LESSON",
        LearnCardCategory::Flow => "FLOW",
    }
}

fn card_status_str(status: CardStatus) -> &'static str {
    match status {
        CardStatus::Proposed => "PROPOSED",
        CardStatus::Accepted => "ACCEPTED",
        CardStatus::Rejected => "REJECTED",
    }
}

fn row_to_domain_card(row: &KnowledgeCardRow) -> Result<KnowledgeCard, Response> {
    let category = parse_card_category(&row.category)?;
    let status = match row.status.trim().to_ascii_uppercase().as_str() {
        "PROPOSED" => CardStatus::Proposed,
        "ACCEPTED" => CardStatus::Accepted,
        "REJECTED" => CardStatus::Rejected,
        other => {
            return Err(err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                format!("Invalid persisted card status '{other}'"),
            ))
        }
    };
    Ok(KnowledgeCard {
        id: row.id.clone(),
        tenant_id: row.tenant_id.clone(),
        project_id: row.project_id.clone(),
        idempotency_key: row.idempotency_key.clone(),
        category,
        title: row.title.clone(),
        content: row.content.clone(),
        content_sha256: row.content_sha256.clone(),
        source_evidence: row.source_evidence.clone(),
        status,
        revision: row.revision,
        reviewed_by: row.reviewed_by.clone(),
        decision_reason: row.decision_reason.clone(),
        created_at: row.created_at,
        accepted_at: row.accepted_at,
    })
}

fn build_learn_authorities(
    vault: &VaultStore,
) -> Result<(HumanSessionAuthority, Vec<HumanSessionAuthority>), Response> {
    let ring = vault.ensure_or_load_learn_signing_ring().map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "VAULT_ERROR",
            format!("Failed to load LEARN HMAC ring from Vault 0o600: {e}"),
        )
    })?;
    let active = HumanSessionAuthority::from_key_with_kid(&ring.active_kid, ring.active_key)
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_ERROR",
                e.to_string(),
            )
        })?;
    let mut retired = Vec::with_capacity(ring.retired.len());
    for (kid, key) in &ring.retired {
        if let Ok(auth) = HumanSessionAuthority::from_key_with_kid(kid, *key) {
            retired.push(auth);
        }
    }
    Ok((active, retired))
}

fn build_hydrated_pipeline(
    pool: &spec_registry::SqlitePool,
    workspace_root: &Path,
    tenant_id: &str,
    project_id: &str,
    now_s: i64,
) -> Result<(LearnPipeline, HumanSessionAuthority), Response> {
    let vault = VaultStore::from_workspace_root(workspace_root);
    let (active_auth, retired_auths) = build_learn_authorities(&vault)?;
    let (card_rows, nonce_rows) = {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let cards =
            list_learn_cards_scoped(&conn, tenant_id, project_id).map_err(map_harness_to_resp)?;
        let nonces = list_consumed_nonces_scoped(&conn, tenant_id, project_id, now_s)
            .map_err(map_harness_to_resp)?;
        (cards, nonces)
    };
    let domain_cards: Vec<KnowledgeCard> = card_rows
        .iter()
        .map(row_to_domain_card)
        .collect::<Result<Vec<_>, _>>()?;
    let domain_nonces: Vec<ConsumedNonceRecord> = nonce_rows
        .into_iter()
        .map(|n| ConsumedNonceRecord {
            tenant_id: n.tenant_id,
            project_id: n.project_id,
            nonce: n.nonce,
            expires_at_s: n.expires_at_s,
        })
        .collect();
    let mut pipeline =
        LearnPipeline::from_persisted_state(&active_auth, domain_cards, domain_nonces, now_s)
            .map_err(|e| {
                err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "LEARN_ERROR",
                    e.to_string(),
                )
            })?;
    for r in &retired_auths {
        pipeline.register_authority(r);
    }
    Ok((pipeline, active_auth))
}

fn ensure_writable_actor(actor: &VerifiedActorContext) -> Result<(), Response> {
    match actor.kind {
        HttpActorKind::HumanWorkbenchSession | HttpActorKind::SuperadminCli => Ok(()),
        HttpActorKind::AgentHmac | HttpActorKind::McpClient => Err(err_resp(
            StatusCode::FORBIDDEN,
            "HUMAN_APPROVAL_REQUIRED",
            "Plugin install requires HumanWorkbenchSession or SuperadminCli",
        )),
    }
}

pub async fn handle_list_plugins(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Query(query): Query<ListPluginsQuery>,
) -> Result<Json<Vec<PluginSummaryDto>>, Response> {
    let pool = state.registry.pool().clone();
    let only_enabled = query.enabled.as_deref() == Some("true");
    let pid_opt = query
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let tenant_id = principal.tenant_id;

    let rows = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let items = list_plugins_for_tenant(&conn, &tenant_id, pid_opt.as_deref())
            .map_err(map_harness_to_resp)?;
        let mut dtos: Vec<PluginSummaryDto> = items.into_iter().map(row_to_summary_dto).collect();
        if only_enabled {
            dtos.retain(|d| d.enabled);
        }
        Ok(dtos)
    })
    .await?;

    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverOrInstallPluginRequest {
    pub project_id: String,
    pub plugin_slug: String,
    pub source_type: Option<String>,
    pub source_repo_url: String,
    pub manifest: SkillManifest,
    pub skill_md: Option<String>,
}

pub async fn handle_discover_plugin(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    headers: HeaderMap,
    Json(req): Json<DiscoverOrInstallPluginRequest>,
) -> Result<Json<PluginSummaryDto>, Response> {
    persist_discovered_or_installed(state, principal, actor, headers, req, false).await
}

pub async fn handle_install_plugin(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    headers: HeaderMap,
    Json(req): Json<DiscoverOrInstallPluginRequest>,
) -> Result<Json<PluginSummaryDto>, Response> {
    ensure_writable_actor(&actor)?;
    persist_discovered_or_installed(state, principal, actor, headers, req, true).await
}

async fn persist_discovered_or_installed(
    state: DaemonAppState,
    principal: FlowPrincipal,
    actor: VerifiedActorContext,
    headers: HeaderMap,
    req: DiscoverOrInstallPluginRequest,
    promote_to_installed: bool,
) -> Result<Json<PluginSummaryDto>, Response> {
    let source_type = req
        .source_type
        .as_deref()
        .unwrap_or("local")
        .trim()
        .to_ascii_lowercase();
    if !matches!(
        source_type.as_str(),
        "builtin" | "local" | "github" | "upload"
    ) {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid source_type",
        ));
    }
    if source_type == "github" {
        validate_ssrf_safe_url(&req.source_repo_url, false)
            .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "SSRF_BLOCKED", reason))?;
    }
    if let Some(ref md) = req.skill_md {
        validate_skill_frontmatter(md).map_err(|e| {
            err_resp(
                StatusCode::BAD_REQUEST,
                "FRONTMATTER_INVALID",
                e.to_string(),
            )
        })?;
    }

    let id = format!("plug-{}", Uuid::new_v4());
    let mut managed = ManagedPlugin::with_scope(
        &principal.tenant_id,
        &req.project_id,
        &id,
        &req.plugin_slug,
        req.manifest,
    )
    .map_err(map_lifecycle_err)?;
    if promote_to_installed {
        let rev = managed.revision();
        managed
            .transition_to_checked(PluginState::Validated, rev)
            .map_err(map_lifecycle_err)?;
        let rev2 = managed.revision();
        managed
            .transition_to_checked(PluginState::Installed, rev2)
            .map_err(map_lifecycle_err)?;
    }

    let _compat = managed.to_domain_plugin();
    let manifest_json = serde_json::to_string(managed.manifest()).map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SERDE_ERROR",
            e.to_string(),
        )
    })?;
    let row = SkillPluginRow {
        id: managed.id().to_string(),
        tenant_id: managed.tenant_id().to_string(),
        project_id: managed.project_id().to_string(),
        plugin_slug: managed.plugin_slug().to_string(),
        version: managed.version().to_string(),
        lifecycle_state: managed.state().as_wire_str().to_string(),
        source_type: source_type.clone(),
        upstream_repo_url: if source_type == "github" {
            Some(req.source_repo_url.clone())
        } else {
            None
        },
        source_repo_url: req.source_repo_url,
        upstream_commit_sha: None,
        cron_schedule: None,
        skill_md_path: "SKILL.md".to_string(),
        plugin_json_path: "plugin.json".to_string(),
        manifest_json,
        last_synced_at: None,
        revision: managed.revision(),
        staging_version: None,
        staging_manifest_json: None,
        staging_diff_reasons_json: None,
        staging_bundle_dir: None,
        staging_package_sha256: None,
        breaking_change_isolated: false,
        quarantined: false,
        quarantine_reason: None,
        updated_at: Utc::now().timestamp_millis(),
    };
    let pool = state.registry.pool().clone();
    let sse_tx = state.sse_tx.clone();
    let cid = corr_id(&headers);
    let pid_copy = row.project_id.clone();
    let saved = run_blocking_resp(move || {
        {
            let conn = pool.connection().map_err(|e| {
                err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string())
            })?;
            ensure_project_owned_by_tenant(&conn, &row.tenant_id, &row.project_id)?;
        }
        let audit = AuditReceiptInput {
            tenant_id: &row.tenant_id,
            project_id: &row.project_id,
            correlation_id: &cid,
            gate_name: if promote_to_installed {
                "plugin_install"
            } else {
                "plugin_discover"
            },
            passed: true,
            checker_id: actor_kind_label(actor.kind),
            target_id: &row.id,
            action: if promote_to_installed {
                "INSTALL"
            } else {
                "DISCOVER"
            },
            revision: row.revision,
            fencing_token: 0,
            reason_code: "OK",
        };
        upsert_plugin_record(&pool, &row, Some(&audit)).map_err(map_harness_to_resp)
    })
    .await?;
    let _ = sse_tx.send(pid_copy);
    Ok(Json(row_to_summary_dto(saved)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionPluginRequest {
    pub project_id: String,
    pub target_state: String,
    pub expected_revision: u64,
    pub reason: Option<String>,
}

pub async fn handle_transition_plugin(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath(plugin_id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<TransitionPluginRequest>,
) -> Result<Json<PluginSummaryDto>, Response> {
    let target = PluginState::parse_compat(&req.target_state)
        .map_err(|e| err_resp(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", e.to_string()))?;
    if target == PluginState::Active {
        require_human_workbench_actor(&actor)?;
    }
    let pool = state.registry.pool().clone();
    let sse_tx = state.sse_tx.clone();
    let cid = corr_id(&headers);
    let pid = req.project_id.clone();
    let updated = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let current = get_plugin_scoped(&conn, &principal.tenant_id, &req.project_id, &plugin_id)
            .map_err(map_harness_to_resp)?
            .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "Plugin not found"))?;
        let mut managed = snapshot_from_plugin_row(&current)
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "LIFECYCLE_ERROR", e))?;
        managed
            .transition_to_checked(target, req.expected_revision)
            .map_err(map_lifecycle_err)?;
        let mut next = current.clone();
        next.lifecycle_state = managed.state().as_wire_str().to_string();
        next.revision = managed.revision();
        next.breaking_change_isolated = managed.breaking_change_isolated();
        if !managed.breaking_change_isolated() {
            next.staging_version = None;
            next.staging_manifest_json = None;
            next.staging_diff_reasons_json = None;
            next.staging_bundle_dir = None;
            next.staging_package_sha256 = None;
        }
        next.updated_at = Utc::now().timestamp_millis();
        let audit = AuditReceiptInput {
            tenant_id: &next.tenant_id,
            project_id: &next.project_id,
            correlation_id: &cid,
            gate_name: "plugin_transition",
            passed: true,
            checker_id: actor_kind_label(actor.kind),
            target_id: &next.id,
            action: next.lifecycle_state.as_str(),
            revision: next.revision,
            fencing_token: 0,
            reason_code: req.reason.as_deref().unwrap_or("TRANSITION"),
        };
        update_plugin_cas(&pool, req.expected_revision, &next, Some(&audit))
            .map_err(map_harness_to_resp)?
            .map_err(|f| map_cas_failure(f, "SkillPlugin"))
    })
    .await?;
    let _ = sse_tx.send(pid);
    Ok(Json(row_to_summary_dto(updated)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagingDecisionRequest {
    pub project_id: String,
    pub expected_revision: u64,
    pub reason: String,
}

pub async fn handle_promote_staging(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath(plugin_id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<StagingDecisionRequest>,
) -> Result<Json<PluginSummaryDto>, Response> {
    require_human_workbench_actor(&actor)?;
    if req.reason.trim().len() < 4 {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Approval reason must be at least 4 characters",
        ));
    }
    let pool = state.registry.pool().clone();
    let sse_tx = state.sse_tx.clone();
    let cid = corr_id(&headers);
    let pid = req.project_id.clone();
    let updated = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let current = get_plugin_scoped(&conn, &principal.tenant_id, &req.project_id, &plugin_id)
            .map_err(map_harness_to_resp)?
            .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "Plugin not found"))?;
        let mut managed = snapshot_from_plugin_row(&current)
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "LIFECYCLE_ERROR", e))?;
        let staged_dir = current
            .staging_bundle_dir
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| {
                err_resp(
                    StatusCode::BAD_REQUEST,
                    "NO_STAGED_BUNDLE_DIR",
                    "Staged breaking-change bundle directory missing on disk",
                )
            })?;
        managed
            .promote_verified_staging_dir(
                &staged_dir,
                current.staging_package_sha256.as_deref(),
                req.expected_revision,
            )
            .map_err(map_lifecycle_err)?;
        let mut next = current.clone();
        next.version = managed.version().to_string();
        next.manifest_json = serde_json::to_string(managed.manifest()).map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SERDE_ERROR",
                e.to_string(),
            )
        })?;
        next.revision = managed.revision();
        next.breaking_change_isolated = false;
        next.staging_version = None;
        next.staging_manifest_json = None;
        next.staging_diff_reasons_json = None;
        next.staging_bundle_dir = None;
        next.staging_package_sha256 = None;
        next.updated_at = Utc::now().timestamp_millis();
        let audit = AuditReceiptInput {
            tenant_id: &next.tenant_id,
            project_id: &next.project_id,
            correlation_id: &cid,
            gate_name: "plugin_staging_promote",
            passed: true,
            checker_id: &principal.user_id,
            target_id: &next.id,
            action: "PROMOTE",
            revision: next.revision,
            fencing_token: 0,
            reason_code: req.reason.trim(),
        };
        update_plugin_cas(&pool, req.expected_revision, &next, Some(&audit))
            .map_err(map_harness_to_resp)?
            .map_err(|f| map_cas_failure(f, "SkillPlugin"))
    })
    .await?;
    let _ = sse_tx.send(pid);
    Ok(Json(row_to_summary_dto(updated)))
}

pub async fn handle_discard_staging(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath(plugin_id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<StagingDecisionRequest>,
) -> Result<Json<PluginSummaryDto>, Response> {
    require_human_workbench_actor(&actor)?;
    if req.reason.trim().len() < 4 {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Discard reason must be at least 4 characters",
        ));
    }
    let pool = state.registry.pool().clone();
    let sse_tx = state.sse_tx.clone();
    let cid = corr_id(&headers);
    let pid = req.project_id.clone();
    let updated = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let current = get_plugin_scoped(&conn, &principal.tenant_id, &req.project_id, &plugin_id)
            .map_err(map_harness_to_resp)?
            .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "Plugin not found"))?;
        let mut managed = snapshot_from_plugin_row(&current)
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "LIFECYCLE_ERROR", e))?;
        managed
            .discard_staging(req.expected_revision)
            .map_err(map_lifecycle_err)?;
        let mut next = current.clone();
        next.revision = managed.revision();
        next.breaking_change_isolated = false;
        next.staging_version = None;
        next.staging_manifest_json = None;
        next.staging_diff_reasons_json = None;
        next.staging_bundle_dir = None;
        next.staging_package_sha256 = None;
        next.updated_at = Utc::now().timestamp_millis();
        let audit = AuditReceiptInput {
            tenant_id: &next.tenant_id,
            project_id: &next.project_id,
            correlation_id: &cid,
            gate_name: "plugin_staging_discard",
            passed: true,
            checker_id: &principal.user_id,
            target_id: &next.id,
            action: "DISCARD",
            revision: next.revision,
            fencing_token: 0,
            reason_code: req.reason.trim(),
        };
        update_plugin_cas(&pool, req.expected_revision, &next, Some(&audit))
            .map_err(map_harness_to_resp)?
            .map_err(|f| map_cas_failure(f, "SkillPlugin"))
    })
    .await?;
    let _ = sse_tx.send(pid);
    Ok(Json(row_to_summary_dto(updated)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepackageBundleRequest {
    pub project_id: String,
    pub source_dir: String,
    pub skill_name: String,
    pub description: String,
    #[serde(default)]
    pub capabilities_required: Vec<String>,
    pub upstream_url: Option<String>,
}

pub async fn handle_repackage_bundle(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Json(req): Json<RepackageBundleRequest>,
) -> Result<Json<plugin_host::RepackageReceipt>, Response> {
    if let Some(ref url) = req.upstream_url {
        validate_ssrf_safe_url(url, false)
            .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "SSRF_BLOCKED", reason))?;
    }
    validate_safe_segment("skill_name", &req.skill_name)
        .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", reason))?;
    let pool = state.registry.pool().clone();
    let ws_root = pool.workspace_root().to_path_buf();
    let src_rel = req.source_dir.clone();
    let receipt = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        verify_project_scope(&conn, &principal.tenant_id, &req.project_id)
            .map_err(map_harness_to_resp)?;
        let canon_ws = std::fs::canonicalize(&ws_root)
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "FS_ERROR", e.to_string()))?;
        let candidate_src = Path::new(&src_rel);
        if candidate_src.is_absolute() || src_rel.contains("..") {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "PATH_TRAVERSAL_DENIED",
                "sourceDir must be a relative path inside workspace",
            ));
        }
        let abs_src = std::fs::canonicalize(canon_ws.join(candidate_src))
            .map_err(|e| err_resp(StatusCode::BAD_REQUEST, "NOT_FOUND", e.to_string()))?;
        if !abs_src.starts_with(&canon_ws) {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "PATH_TRAVERSAL_DENIED",
                "sourceDir escaped workspace root",
            ));
        }
        let dest_root = canon_ws.join(".cowok-design").join("repackaged-skills");
        std::fs::create_dir_all(&dest_root)
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "FS_ERROR", e.to_string()))?;
        SkillRepackager::repackage_directory(
            &abs_src,
            &dest_root,
            &req.skill_name,
            &req.description,
            req.capabilities_required,
            req.upstream_url.as_deref(),
        )
        .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "REPACKAGE_FAILED", reason))
    })
    .await?;
    Ok(Json(receipt))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryStubRequest {
    pub bundle_dir: String,
}

pub async fn handle_repackage_discovery_stub(
    State(state): State<DaemonAppState>,
    Json(req): Json<DiscoveryStubRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let ws_root = state.registry.pool().workspace_root().to_path_buf();
    let out = run_blocking_resp(move || {
        let canon_ws = std::fs::canonicalize(&ws_root)
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "FS_ERROR", e.to_string()))?;
        let rel = Path::new(&req.bundle_dir);
        if rel.is_absolute() || req.bundle_dir.contains("..") {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "PATH_TRAVERSAL_DENIED",
                "bundleDir must stay inside workspace",
            ));
        }
        let abs = std::fs::canonicalize(canon_ws.join(rel))
            .map_err(|e| err_resp(StatusCode::BAD_REQUEST, "NOT_FOUND", e.to_string()))?;
        if !abs.starts_with(&canon_ws) {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "PATH_TRAVERSAL_DENIED",
                "bundleDir escaped workspace",
            ));
        }
        let bundle = load_verified_staged_bundle_from_dir(&abs, None)
            .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "BUNDLE_INVALID", reason))?;
        let compat = manifest_to_skill_plugin(&bundle.manifest);
        let descriptor_path = abs.join("discovery-stub.json");
        let bytes = std::fs::read(&descriptor_path)
            .map_err(|e| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", e.to_string()))?;
        if bytes.len() > plugin_host::MAX_DISCOVERY_STUB_BYTES {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "STUB_OVERSIZED",
                "discovery-stub.json exceeds 1,536B limit",
            ));
        }
        let stub_json: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| err_resp(StatusCode::BAD_REQUEST, "INVALID_JSON", e.to_string()))?;
        Ok(serde_json::json!({
            "discoveryStub": stub_json,
            "skillPlugin": compat,
        }))
    })
    .await?;
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoIntakeRequest {
    pub skill_name: String,
    pub storyboard_markdown: String,
    pub expected_duration_secs: Option<f64>,
}

pub async fn handle_repackage_video_intake(
    Json(req): Json<VideoIntakeRequest>,
) -> Result<Json<plugin_host::VideoStoryboardReceipt>, Response> {
    let receipt = VideoIntakeProcessor::parse_storyboard_text_strict(
        &req.skill_name,
        &req.storyboard_markdown,
    )
    .map_err(|e| err_resp(StatusCode::BAD_REQUEST, "VIDEO_INTAKE_ERROR", e.to_string()))?;
    if let Some(expected_max) = req.expected_duration_secs {
        if receipt.duration_s > expected_max {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "VIDEO_INTAKE_DURATION_EXCEEDED",
                format!(
                    "Parsed storyboard duration {:.2}s exceeds expected maximum {:.2}s",
                    receipt.duration_s, expected_max
                ),
            ));
        }
    }
    Ok(Json(receipt))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriangleLintRequest {
    pub content: String,
}

pub async fn handle_triangle_lint(
    Json(req): Json<TriangleLintRequest>,
) -> Json<plugin_host::TriangleLintReport> {
    Json(TriangleLinter::lint(&req.content))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilePromptBundleRequest {
    pub run_id: String,
    #[serde(default)]
    pub selected_skill_names: Vec<String>,
    pub max_learn_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledPromptSnapshotDto {
    pub run_id: String,
    pub staging_dir: String,
    pub stable_prompt_hash: String,
    pub learn_snapshot_sha256: String,
    pub learn_card_ids: Vec<String>,
    pub learn_total_tokens: usize,
    pub fenced_prompt_xml: String,
}

pub fn ensure_project_owned_by_tenant(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
) -> Result<(), Response> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            rusqlite::params![project_id, tenant_id],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => err_resp(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Project not found in tenant scope",
            ),
            other => err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                other.to_string(),
            ),
        })?;
    if exists.is_none() {
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Project not found in tenant scope",
        ));
    }
    Ok(())
}

/// Deterministic skill catalog staging (`.cowok-design/.cowok-skills/<run_id>/`) and fenced LEARN prompt compiler (`<= 1,500` tokens).
pub fn compile_skill_prompt_bundle_sync(
    pool: &spec_registry::SqlitePool,
    tenant_id: &str,
    project_id: &str,
    req: &CompilePromptBundleRequest,
) -> Result<CompiledPromptSnapshotDto, Response> {
    validate_safe_segment("run_id", &req.run_id)
        .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", reason))?;
    {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        ensure_project_owned_by_tenant(&conn, tenant_id, project_id)?;
    }
    let ws_root = pool.workspace_root().to_path_buf();
    let max_tokens = req
        .max_learn_tokens
        .unwrap_or(MAX_LEARN_PROMPT_TOKENS)
        .min(MAX_LEARN_PROMPT_TOKENS);
    let selected_set: BTreeSet<String> = req.selected_skill_names.iter().cloned().collect();
    let now_s = Utc::now().timestamp();
    let (pipeline, _) = build_hydrated_pipeline(pool, &ws_root, tenant_id, project_id, now_s)?;
    let active_rows = {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        list_plugins_for_tenant(&conn, tenant_id, Some(project_id)).map_err(map_harness_to_resp)?
    };
    let mut manifests: Vec<SkillManifest> = Vec::new();
    for r in active_rows {
        if r.lifecycle_state != "ACTIVE" {
            continue;
        }
        if !selected_set.is_empty()
            && !selected_set.contains(&r.plugin_slug)
            && !selected_set.contains(&r.id)
        {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<SkillManifest>(&r.manifest_json) {
            manifests.push(m);
        }
    }
    let skill_refs: Vec<(&SkillManifest, Option<&str>)> =
        manifests.iter().map(|m| (m, None)).collect();
    let stage_base = ws_root.join(".cowok-design").join(".cowok-skills");
    std::fs::create_dir_all(&stage_base)
        .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "FS_ERROR", e.to_string()))?;
    let stager = PerRunStager::new(&stage_base);
    let stage_receipt = stager
        .stage_run_with_sources(&req.run_id, &skill_refs)
        .map_err(|e| err_resp(StatusCode::CONFLICT, "STAGE_RUN_FAILED", e.to_string()))?;

    let learn_pack = pipeline.render_prompt_fenced_cards(tenant_id, project_id, max_tokens);
    let catalog_xml = manifests
        .iter()
        .map(|m| {
            format!(
                "<SKILL name=\"{}\" version=\"{}\" />",
                xml_escape_attr(&m.name),
                xml_escape_attr(&m.version)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let fenced_prompt_xml = format!(
        "<SKILL_CATALOG_DATA>\n{catalog_xml}\n</SKILL_CATALOG_DATA>\n{}",
        learn_pack.fenced_xml
    );
    let mut sha_hasher = Sha256::new();
    sha_hasher.update(stage_receipt.stable_prompt_hash.as_bytes());
    sha_hasher.update(b"|");
    sha_hasher.update(fenced_prompt_xml.as_bytes());
    let learn_snapshot_sha256 = format!("{:x}", sha_hasher.finalize());

    Ok(CompiledPromptSnapshotDto {
        run_id: req.run_id.clone(),
        staging_dir: stage_receipt.staged_dir,
        stable_prompt_hash: stage_receipt.stable_prompt_hash,
        learn_snapshot_sha256,
        learn_card_ids: learn_pack.card_ids,
        learn_total_tokens: learn_pack.total_tokens,
        fenced_prompt_xml,
    })
}

pub async fn handle_compile_skill_prompt_bundle(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(project_id): AxumPath<String>,
    Json(req): Json<CompilePromptBundleRequest>,
) -> Result<Json<CompiledPromptSnapshotDto>, Response> {
    let pool = state.registry.pool().clone();
    let dto = run_blocking_resp(move || {
        compile_skill_prompt_bundle_sync(&pool, &principal.tenant_id, &project_id, &req)
    })
    .await?;

    Ok(Json(dto))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCardDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub idempotency_key: Option<String>,
    pub category: String,
    pub title: String,
    pub content: String,
    pub content_sha256: String,
    pub source_evidence: String,
    pub status: String,
    pub revision: u64,
    pub reviewed_by: Option<String>,
    pub decision_reason: Option<String>,
    pub created_at: i64,
    pub accepted_at: Option<i64>,
}

fn card_row_to_dto(r: KnowledgeCardRow) -> KnowledgeCardDto {
    KnowledgeCardDto {
        id: r.id,
        tenant_id: r.tenant_id,
        project_id: r.project_id,
        idempotency_key: r.idempotency_key,
        category: r.category,
        title: r.title,
        content: r.content,
        content_sha256: r.content_sha256,
        source_evidence: r.source_evidence,
        status: r.status,
        revision: r.revision,
        reviewed_by: r.reviewed_by,
        decision_reason: r.decision_reason,
        created_at: r.created_at,
        accepted_at: r.accepted_at,
    }
}

pub async fn handle_list_learn_cards(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Json<Vec<KnowledgeCardDto>>, Response> {
    let pool = state.registry.pool().clone();
    let cards = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        ensure_project_owned_by_tenant(&conn, &principal.tenant_id, &project_id)?;
        let rows = list_learn_cards_scoped(&conn, &principal.tenant_id, &project_id)
            .map_err(map_harness_to_resp)?;
        Ok(rows.into_iter().map(card_row_to_dto).collect())
    })
    .await?;
    Ok(Json(cards))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposeLearnCardRequest {
    pub idempotency_key: Option<String>,
    pub category: String,
    pub title: String,
    pub content: String,
    pub source_evidence: String,
}

pub async fn handle_propose_learn_card(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<ProposeLearnCardRequest>,
) -> Result<Json<KnowledgeCardDto>, Response> {
    let category = parse_card_category(&req.category)?;
    let pool = state.registry.pool().clone();
    let ws_root = pool.workspace_root().to_path_buf();
    let sse_tx = state.sse_tx.clone();
    let cid = corr_id(&headers);
    let pid_copy = project_id.clone();

    let saved = run_blocking_resp(move || {
        {
            let conn = pool.connection().map_err(|e| {
                err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string())
            })?;
            ensure_project_owned_by_tenant(&conn, &principal.tenant_id, &project_id)?;
        }
        let now_s = Utc::now().timestamp();
        let (mut pipeline, _) =
            build_hydrated_pipeline(&pool, &ws_root, &principal.tenant_id, &project_id, now_s)?;
        let replay_id_opt = req.idempotency_key.as_deref().and_then(|key| {
            pipeline
                .list_cards_for_scope(&principal.tenant_id, &project_id)
                .into_iter()
                .find(|c| c.idempotency_key.as_deref() == Some(key))
                .map(|c| c.id.clone())
        });
        let generated_id = replay_id_opt.unwrap_or_else(|| format!("card-{}", Uuid::new_v4()));
        let propose_res = pipeline.propose_card_scoped(&ProposeCardInput {
            tenant_id: &principal.tenant_id,
            project_id: &project_id,
            id: &generated_id,
            idempotency_key: req.idempotency_key.as_deref(),
            category,
            title: &req.title,
            content: &req.content,
            source_evidence: &req.source_evidence,
            created_at: now_s,
        });
        let card = match propose_res {
            Ok(c) => c.clone(),
            Err(LearnError::IdempotentReplay(existing_id)) => {
                let conn = pool.connection().map_err(|e| {
                    err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string())
                })?;
                return list_learn_cards_scoped(&conn, &principal.tenant_id, &project_id)
                    .map_err(map_harness_to_resp)?
                    .into_iter()
                    .find(|c| c.id == existing_id)
                    .ok_or_else(|| {
                        err_resp(
                            StatusCode::CONFLICT,
                            "LEARN_ERROR",
                            "Idempotent card missing",
                        )
                    });
            }
            Err(LearnError::PromptInjectionBlocked(msg)) => {
                return Err(err_resp(
                    StatusCode::BAD_REQUEST,
                    "PROMPT_INJECTION_BLOCKED",
                    msg,
                ));
            }
            Err(other) => {
                return Err(err_resp(
                    StatusCode::CONFLICT,
                    "LEARN_ERROR",
                    other.to_string(),
                ));
            }
        };
        let row = KnowledgeCardRow {
            id: card.id.clone(),
            tenant_id: card.tenant_id.clone(),
            project_id: card.project_id.clone(),
            idempotency_key: card.idempotency_key.clone(),
            category: card_category_str(card.category).to_string(),
            title: card.title.clone(),
            content: card.content.clone(),
            content_sha256: card.content_sha256.clone(),
            source_evidence: card.source_evidence.clone(),
            status: card_status_str(card.status).to_string(),
            revision: card.revision,
            reviewed_by: card.reviewed_by.clone(),
            decision_reason: card.decision_reason.clone(),
            created_at: card.created_at,
            accepted_at: card.accepted_at,
        };
        let audit = AuditReceiptInput {
            tenant_id: &row.tenant_id,
            project_id: &row.project_id,
            correlation_id: &cid,
            gate_name: "learn_card_propose",
            passed: true,
            checker_id: actor_kind_label(actor.kind),
            target_id: &row.id,
            action: "PROPOSE",
            revision: row.revision,
            fencing_token: 0,
            reason_code: "PROPOSED",
        };
        insert_learn_card_tx(&pool, &row, Some(&audit)).map_err(map_harness_to_resp)
    })
    .await?;

    let _ = sse_tx.send(pid_copy);
    Ok(Json(card_row_to_dto(saved)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnCardDecisionRequest {
    pub reason: String,
    pub expected_revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanOnlyDecisionReceiptDto {
    pub card: KnowledgeCardDto,
    pub kid: String,
    pub actor_kind: HttpActorKind,
    pub evidence_hash: String,
}

struct LearnDecisionTarget {
    project_id: String,
    card_id: String,
    correlation_id: String,
}

pub async fn handle_approve_learn_card(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath((project_id, card_id)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<LearnCardDecisionRequest>,
) -> Result<Json<HumanOnlyDecisionReceiptDto>, Response> {
    let target = LearnDecisionTarget {
        project_id,
        card_id,
        correlation_id: corr_id(&headers),
    };
    decide_learn_card(
        state,
        principal,
        actor,
        target,
        req,
        CardDecisionAction::Accept,
    )
    .await
}

pub async fn handle_reject_learn_card(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath((project_id, card_id)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<LearnCardDecisionRequest>,
) -> Result<Json<HumanOnlyDecisionReceiptDto>, Response> {
    let target = LearnDecisionTarget {
        project_id,
        card_id,
        correlation_id: corr_id(&headers),
    };
    decide_learn_card(
        state,
        principal,
        actor,
        target,
        req,
        CardDecisionAction::Reject,
    )
    .await
}

async fn decide_learn_card(
    state: DaemonAppState,
    principal: FlowPrincipal,
    actor: VerifiedActorContext,
    target: LearnDecisionTarget,
    req: LearnCardDecisionRequest,
    action: CardDecisionAction,
) -> Result<Json<HumanOnlyDecisionReceiptDto>, Response> {
    require_human_workbench_actor(&actor)?;
    if req.reason.trim().len() < 4 {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Decision reason must be at least 4 characters",
        ));
    }
    let pool = state.registry.pool().clone();
    let ws_root = pool.workspace_root().to_path_buf();
    let sse_tx = state.sse_tx.clone();
    let project_id = target.project_id;
    let card_id = target.card_id;
    let cid = target.correlation_id;
    let pid_copy = project_id.clone();

    let receipt = run_blocking_resp(move || {
        let now_s = Utc::now().timestamp().max(1);
        let (mut pipeline, authority) = build_hydrated_pipeline(
            &pool,
            &ws_root,
            &principal.tenant_id,
            &project_id,
            now_s,
        )?;
        let existing_card = pipeline
            .list_cards_for_scope(&principal.tenant_id, &project_id)
            .into_iter()
            .find(|c| c.id == card_id)
            .cloned()
            .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "LEARN card not found"))?;
        let nonce = format!("n-{}", Uuid::new_v4().simple());
        let token = authority
            .mint_token(&MintTokenInput {
                actor_kind: to_domain_actor_kind(actor.kind),
                actor_id: &principal.user_id,
                tenant_id: &principal.tenant_id,
                project_id: &project_id,
                card_id: &existing_card.id,
                content_sha256: &existing_card.content_sha256,
                action,
                expected_revision: req.expected_revision,
                reason: req.reason.trim(),
                now_s,
                ttl_s: 300,
                nonce: &nonce,
            })
            .map_err(|e| err_resp(StatusCode::FORBIDDEN, "HUMAN_APPROVAL_REQUIRED", e.to_string()))?;

        let decision_res = match action {
            CardDecisionAction::Accept => pipeline.accept_card_verified(
                &principal.tenant_id,
                &project_id,
                &card_id,
                &token,
                req.expected_revision,
                now_s,
            ),
            CardDecisionAction::Reject => pipeline.reject_card_verified(
                &principal.tenant_id,
                &project_id,
                &card_id,
                &token,
                req.expected_revision,
                now_s,
            ),
        };
        decision_res.map_err(|e| match e {
            LearnError::RevisionConflict { actual, .. } => {
                map_cas_failure(CasFailure::RevisionConflict { current_revision: actual }, "KnowledgeCard")
            }
            LearnError::CardNotFound(_) => map_cas_failure(CasFailure::NotFound, "KnowledgeCard"),
            other => err_resp(StatusCode::CONFLICT, "LEARN_DECISION_FAILED", other.to_string()),
        })?;

        let updated_domain = pipeline
            .list_cards_for_scope(&principal.tenant_id, &project_id)
            .into_iter()
            .find(|c| c.id == card_id)
            .cloned()
            .ok_or_else(|| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "LEARN_ERROR", "Missing card"))?;
        let next_row = KnowledgeCardRow {
            id: updated_domain.id.clone(),
            tenant_id: updated_domain.tenant_id.clone(),
            project_id: updated_domain.project_id.clone(),
            idempotency_key: updated_domain.idempotency_key.clone(),
            category: card_category_str(updated_domain.category).to_string(),
            title: updated_domain.title.clone(),
            content: updated_domain.content.clone(),
            content_sha256: updated_domain.content_sha256.clone(),
            source_evidence: updated_domain.source_evidence.clone(),
            status: card_status_str(updated_domain.status).to_string(),
            revision: updated_domain.revision,
            reviewed_by: updated_domain.reviewed_by.clone(),
            decision_reason: updated_domain.decision_reason.clone(),
            created_at: updated_domain.created_at,
            accepted_at: updated_domain.accepted_at,
        };
        let nonce_row = ConsumedNonceRow {
            tenant_id: principal.tenant_id.clone(),
            project_id: project_id.clone(),
            nonce: token.nonce().to_string(),
            expires_at_s: token.expires_at_s(),
            consumed_at: now_s,
        };
        let audit = AuditReceiptInput {
            tenant_id: &next_row.tenant_id,
            project_id: &next_row.project_id,
            correlation_id: &cid,
            gate_name: if action == CardDecisionAction::Accept {
                "learn_card_approve"
            } else {
                "learn_card_reject"
            },
            passed: true,
            checker_id: &principal.user_id,
            target_id: &next_row.id,
            action: next_row.status.as_str(),
            revision: next_row.revision,
            fencing_token: 0,
            reason_code: req.reason.trim(),
        };
        let persisted = commit_learn_card_decision(
            &pool,
            req.expected_revision,
            &next_row,
            &nonce_row,
            Some(&audit),
        )
        .map_err(map_harness_to_resp)?
        .map_err(|f| map_cas_failure(f, "KnowledgeCard"))?;

        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let evidence_hash: String = conn
            .query_row(
                "SELECT evidence_hash FROM gate_reports WHERE project_id = ?1 AND gate_name = ?2 ORDER BY timestamp DESC, id DESC LIMIT 1",
                rusqlite::params![&next_row.project_id, audit.gate_name],
                |r| r.get(0),
            )
            .unwrap_or_default();

        Ok(HumanOnlyDecisionReceiptDto {
            card: card_row_to_dto(persisted),
            kid: token.kid().to_string(),
            actor_kind: actor.kind,
            evidence_hash,
        })
    })
    .await?;

    let _ = sse_tx.send(pid_copy);
    Ok(Json(receipt))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleQuery {
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CronScheduleStatusDto {
    pub id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub plugin_id: String,
    pub cron_expression: String,
    pub upstream_repo_url: String,
    pub last_commit_sha: Option<String>,
    pub sync_status: String,
    pub consecutive_failures: i32,
    pub next_retry_at: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub updated_at: String,
}

fn cron_row_to_dto(r: PluginCronScheduleRow) -> CronScheduleStatusDto {
    CronScheduleStatusDto {
        id: r.id,
        tenant_id: r.tenant_id,
        project_id: r.project_id,
        plugin_id: r.plugin_id,
        cron_expression: r.cron_expression,
        upstream_repo_url: r.upstream_repo_url,
        last_commit_sha: r.last_commit_sha,
        sync_status: r.sync_status,
        consecutive_failures: r.consecutive_failures,
        next_retry_at: r.next_retry_at,
        last_error_code: r.last_error_code,
        last_error_message: r.last_error_message,
        updated_at: chrono::DateTime::from_timestamp_millis(r.updated_at)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
    }
}

pub async fn handle_get_plugin_schedule(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(plugin_id): AxumPath<String>,
    Query(q): Query<ScheduleQuery>,
) -> Result<Json<CronScheduleStatusDto>, Response> {
    let pool = state.registry.pool().clone();
    let row = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let plugin = match q.project_id.as_deref() {
            Some(pid) => get_plugin_scoped(&conn, &principal.tenant_id, pid, &plugin_id)
                .map_err(map_harness_to_resp)?,
            None => find_plugin_for_tenant(&conn, &principal.tenant_id, &plugin_id)
                .map_err(map_harness_to_resp)?,
        }
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "Plugin not found"))?;
        get_cron_schedule_scoped(&conn, &principal.tenant_id, &plugin.project_id, &plugin.id)
            .map_err(map_harness_to_resp)?
            .ok_or_else(|| {
                err_resp(
                    StatusCode::NOT_FOUND,
                    "NOT_FOUND",
                    "Cron schedule not configured",
                )
            })
    })
    .await?;
    Ok(Json(cron_row_to_dto(row)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCronScheduleRequest {
    pub project_id: String,
    pub cron_expression: String,
    pub upstream_repo_url: String,
}

pub async fn handle_upsert_plugin_schedule(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath(plugin_id): AxumPath<String>,
    headers: HeaderMap,
    Json(req): Json<UpsertCronScheduleRequest>,
) -> Result<Json<CronScheduleStatusDto>, Response> {
    require_human_workbench_actor(&actor)?;
    let valid_expr = validate_cron_expression(&req.cron_expression)
        .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "INVALID_CRON", reason))?;
    let safe_url = validate_ssrf_safe_url(&req.upstream_repo_url, false)
        .map_err(|reason| err_resp(StatusCode::BAD_REQUEST, "SSRF_BLOCKED", reason))?;
    let pool = state.registry.pool().clone();
    let cid = corr_id(&headers);
    let row = run_blocking_resp(move || {
        {
            let conn = pool.connection().map_err(|e| {
                err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string())
            })?;
            get_plugin_scoped(&conn, &principal.tenant_id, &req.project_id, &plugin_id)
                .map_err(map_harness_to_resp)?
                .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "Plugin not found"))?;
        }
        let audit = AuditReceiptInput {
            tenant_id: &principal.tenant_id,
            project_id: &req.project_id,
            correlation_id: &cid,
            gate_name: "plugin_schedule_upsert",
            passed: true,
            checker_id: &principal.user_id,
            target_id: &plugin_id,
            action: "UPSERT_SCHEDULE",
            revision: 1,
            fencing_token: 0,
            reason_code: &valid_expr,
        };
        upsert_cron_schedule(
            &pool,
            &principal.tenant_id,
            &req.project_id,
            &plugin_id,
            &valid_expr,
            &safe_url,
            Some(&audit),
        )
        .map_err(map_harness_to_resp)
    })
    .await?;
    Ok(Json(cron_row_to_dto(row)))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerSyncRequest {
    pub project_id: Option<String>,
    pub refresh_auth: Option<bool>,
}

pub async fn handle_trigger_plugin_sync(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    Extension(actor): Extension<VerifiedActorContext>,
    AxumPath(plugin_id): AxumPath<String>,
    body_opt: Option<Json<TriggerSyncRequest>>,
) -> Result<Json<CronScheduleStatusDto>, Response> {
    require_human_workbench_actor(&actor)?;
    let req = body_opt.map(|Json(v)| v).unwrap_or_default();
    let pool = state.registry.pool().clone();
    let ws_root = pool.workspace_root().to_path_buf();
    let tenant_id = principal.tenant_id.clone();
    let plugin_id_lookup = plugin_id.clone();
    let proj_id = run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        let plugin = match req.project_id.as_deref() {
            Some(pid) => get_plugin_scoped(&conn, &tenant_id, pid, &plugin_id_lookup)
                .map_err(map_harness_to_resp)?,
            None => find_plugin_for_tenant(&conn, &tenant_id, &plugin_id_lookup)
                .map_err(map_harness_to_resp)?,
        }
        .ok_or_else(|| err_resp(StatusCode::NOT_FOUND, "NOT_FOUND", "Plugin not found"))?;
        if req.refresh_auth.unwrap_or(false) {
            let _ = clear_auth_required_schedules_for_tenant(
                &pool,
                &tenant_id,
                Utc::now().timestamp_millis(),
            );
        }
        Ok(plugin.project_id)
    })
    .await?;

    let row = run_plugin_sync_cycle(
        state.registry.pool(),
        &ws_root,
        &principal.tenant_id,
        &proj_id,
        &plugin_id,
        true,
    )
    .await
    .map_err(|reason| {
        if reason.contains("IN_FLIGHT") || reason.contains("BUSY") {
            err_resp(StatusCode::CONFLICT, "SYNC_IN_FLIGHT", reason)
        } else {
            err_resp(StatusCode::BAD_GATEWAY, "CRON_SYNC_FAILED", reason)
        }
    })?;
    let _ = state.sse_tx.send(proj_id);
    Ok(Json(cron_row_to_dto(row)))
}

struct PluginSseState {
    project_id: String,
    rx: tokio::sync::broadcast::Receiver<String>,
    sent_initial: bool,
}

/// Streams project-scoped plugin, staging, LEARN, and cron sync notifications over Server-Sent Events.
pub async fn handle_stream_plugin_events(
    State(state): State<DaemonAppState>,
    Extension(principal): Extension<FlowPrincipal>,
    AxumPath(project_id): AxumPath<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, Response> {
    let pool = state.registry.pool().clone();
    let tid = principal.tenant_id.clone();
    let pid_c = project_id.clone();
    run_blocking_resp(move || {
        let conn = pool
            .connection()
            .map_err(|e| err_resp(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", e.to_string()))?;
        ensure_project_owned_by_tenant(&conn, &tid, &pid_c)
    })
    .await?;

    let init = PluginSseState {
        project_id,
        rx: state.sse_tx.subscribe(),
        sent_initial: false,
    };
    let stream = futures_util::stream::unfold(init, |mut st| async move {
        if !st.sent_initial {
            st.sent_initial = true;
            let payload = serde_json::json!({
                "projectId": st.project_id,
                "kind": "connected",
                "timestamp": Utc::now().to_rfc3339(),
            })
            .to_string();
            let ev = Event::default()
                .event("plugin_ecosystem_update")
                .data(payload);
            return Some((Ok(ev), st));
        }
        loop {
            match st.rx.recv().await {
                Ok(msg_pid) if msg_pid == st.project_id => {
                    let payload = serde_json::json!({
                        "projectId": st.project_id,
                        "kind": "refresh",
                        "timestamp": Utc::now().to_rfc3339(),
                    })
                    .to_string();
                    let ev = Event::default()
                        .event("plugin_ecosystem_update")
                        .data(payload);
                    return Some((Ok(ev), st));
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(_)) => {
                    let payload = serde_json::json!({
                        "projectId": st.project_id,
                        "kind": "refresh",
                        "timestamp": Utc::now().to_rfc3339(),
                    })
                    .to_string();
                    let ev = Event::default()
                        .event("plugin_ecosystem_update")
                        .data(payload);
                    return Some((Ok(ev), st));
                }
                Err(RecvError::Closed) => return None,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
