// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (4-Channel Distribution Ops, Plugin Catalog Lifecycle, Deterministic A/B Experiments, Global Skill Quarantine, Offline Ed25519 Licenses & Cowok.ai Federation Routes)
use std::collections::{BTreeMap, HashMap, HashSet};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spec_registry::{retry_on_busy, SqlitePool};
use uuid::Uuid;

use crate::routes::cloud_admin_auth::{
    ensure_v29_tables, require_admin_role, scrub_json_server_pii, scrub_server_pii,
    verify_admin_session_from_headers, AdminSessionClaims, CloudAdminRole, CloudAuthError,
};
use crate::routes::distribution_ops_crypto::{
    assign_ab_bucket, default_features_for_tier, is_valid_ascii_identifier,
    is_valid_lifecycle_status, load_or_create_workspace_signing_key,
    resolve_effective_plugin_states, select_ab_variant, sign_manifest, sign_offline_license,
    validate_experiment_security_guard, verify_manifest_fail_closed, verify_offline_license,
    verifying_key_base64, AbVariantWeight, CatalogPluginRecord, EffectivePluginState,
    LocalPluginOverride, OfflineLicensePayload, SignedManifestEnvelope, SkillAdvisoryRecord,
    VerifiedOfflineLicense, DEFAULT_MANIFEST_TTL_SECS,
};
use crate::routes::DaemonAppState;

const ENSURE_V32_SQL: &str = include_str!(
    "../../../spec-registry/migrations/V32__story_14_3_distribution_ops_catalog_ab.sql"
);

const PUBLIC_MANIFEST_ACTOR: &str = "cowok-release-ops";
const FORBIDDEN_MUTABLE_REFS: [&str; 8] = [
    "main", "master", "head", "develop", "dev", "latest", "canary", "nightly",
];

const VALID_CHANNELS: [&str; 4] = ["cli", "desktop", "plugin", "docker"];
const VALID_SOURCE_KINDS: [&str; 2] = ["DEFAULT_BUILTIN", "RECOMMENDED_GITHUB"];
const VALID_TARGET_SURFACES: [&str; 3] =
    ["ONBOARDING_DEFAULTS", "PLUGIN_RANKING", "UX_COPY_LAYOUT"];
const VALID_EXPERIMENT_STATUSES: [&str; 5] = ["DRAFT", "RUNNING", "PAUSED", "KILLED", "COMPLETED"];
const VALID_SEVERITIES: [&str; 3] = ["CRITICAL", "HIGH", "MEDIUM"];
const VALID_ADVISORY_ACTIONS: [&str; 3] = ["QUARANTINE", "WARN", "REVOKE_QUARANTINE"];
const VALID_LICENSE_TIERS: [&str; 4] = ["FREE", "PRO", "TEAM", "ENTERPRISE"];
const ALL_ADMIN_ROLES: [CloudAdminRole; 4] = [
    CloudAdminRole::SuperAdmin,
    CloudAdminRole::SupportAgent,
    CloudAdminRole::SkillCurator,
    CloudAdminRole::AnalystReadonly,
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

fn extract_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn json_ok<T: Serialize>(status: StatusCode, data: T, request_id: String) -> Response {
    (
        status,
        Json(ApiSuccessEnvelope {
            ok: true,
            data,
            request_id,
        }),
    )
        .into_response()
}

fn json_err(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    field: Option<&str>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.into(),
                field: field.map(str::to_string),
                request_id,
            },
        }),
    )
        .into_response()
}

fn map_auth_err(err: CloudAuthError, request_id: String) -> Response {
    json_err(err.status, err.code, err.message, None, request_id)
}

fn ensure_catalog_async_lifecycle_constraint(conn: &Connection) -> Result<(), String> {
    let table_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cloud_plugin_catalog_entries'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("inspect cloud_plugin_catalog_entries schema failed: {e}"))?;
    let Some(ddl) = table_sql else {
        return Ok(());
    };
    if ddl.contains("PROCESSING") && ddl.contains("PENDING_APPROVAL") {
        return Ok(());
    }
    conn.execute_batch(
        "SAVEPOINT v32_upgrade_catalog_lifecycle;
         CREATE TABLE cloud_plugin_catalog_entries__v32_upgrade (
             plugin_id TEXT PRIMARY KEY NOT NULL,
             source_kind TEXT NOT NULL CHECK (source_kind IN ('DEFAULT_BUILTIN', 'RECOMMENDED_GITHUB')),
             lifecycle_status TEXT NOT NULL CHECK (lifecycle_status IN ('ACTIVE', 'INACTIVE', 'DEPRECATED', 'QUARANTINED', 'PROCESSING', 'PENDING_APPROVAL')),
             repo_url TEXT,
             pinned_commit_sha TEXT,
             min_app_version TEXT NOT NULL DEFAULT '0.1.0',
             default_enabled INTEGER NOT NULL DEFAULT 1 CHECK (default_enabled IN (0, 1)),
             default_config_json TEXT NOT NULL DEFAULT '{}',
             title_i18n_json TEXT NOT NULL DEFAULT '{}',
             summary_i18n_json TEXT NOT NULL DEFAULT '{}',
             manifest_seq BIGINT NOT NULL DEFAULT 1,
             updated_by TEXT NOT NULL,
             updated_at BIGINT NOT NULL
         );
         INSERT INTO cloud_plugin_catalog_entries__v32_upgrade (
             plugin_id, source_kind, lifecycle_status, repo_url, pinned_commit_sha,
             min_app_version, default_enabled, default_config_json, title_i18n_json,
             summary_i18n_json, manifest_seq, updated_by, updated_at
         )
         SELECT
             plugin_id, source_kind, lifecycle_status, repo_url, pinned_commit_sha,
             min_app_version, default_enabled, default_config_json, title_i18n_json,
             summary_i18n_json, manifest_seq, updated_by, updated_at
         FROM cloud_plugin_catalog_entries;
         DROP TABLE cloud_plugin_catalog_entries;
         ALTER TABLE cloud_plugin_catalog_entries__v32_upgrade RENAME TO cloud_plugin_catalog_entries;
         RELEASE SAVEPOINT v32_upgrade_catalog_lifecycle;",
    )
    .map_err(|e| {
        let _ = conn.execute_batch(
            "ROLLBACK TO SAVEPOINT v32_upgrade_catalog_lifecycle;
             RELEASE SAVEPOINT v32_upgrade_catalog_lifecycle;",
        );
        format!("upgrade cloud_plugin_catalog_entries lifecycle CHECK failed: {e}")
    })
}

/// Ensures V29 + V32 tables exist and seeds the 3 clean built-in plugins if catalog is empty.
pub fn ensure_v32_tables_and_seed(conn: &Connection) -> Result<(), String> {
    ensure_v29_tables(conn)?;
    conn.execute_batch(ENSURE_V32_SQL)
        .map_err(|e| format!("ensure V32 distribution ops tables failed: {e}"))?;
    ensure_catalog_async_lifecycle_constraint(conn)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cloud_plugin_catalog_entries",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("count plugin catalog failed: {e}"))?;
    if count == 0 {
        let now_ms = Utc::now().timestamp_millis();
        let seeds: [(&str, &str, &str, &str); 3] = [
            (
                "html-forge-core",
                r#"{"en":"HTML Forge Core","vi":"Lõi Dựng HTML Forge"}"#,
                r#"{"en":"Zero-bloat semantic HTML & design-token renderer","vi":"Trình kết xuất HTML ngữ nghĩa & token thiết kế tốc độ cao"}"#,
                r#"{"strictTokens":true}"#,
            ),
            (
                "visual-qc-17rule",
                r#"{"en":"Visual QC 17-Rule Engine","vi":"Kiểm định Thị giác 17 Quy tắc"}"#,
                r#"{"en":"Automated contrast, layout, and responsive visual audit","vi":"Kiểm tra tự động tương phản, bố cục và hiển thị đa màn hình"}"#,
                r#"{"autoFix":false}"#,
            ),
            (
                "mcp-open-gateway",
                r#"{"en":"MCP Open Gateway","vi":"Cổng Kết nối MCP Mở"}"#,
                r#"{"en":"Standardized MCP connector for IDE & AI agent hosts","vi":"Cầu nối MCP chuẩn hóa cho IDE và các tác nhân AI"}"#,
                r#"{"readOnlyDefault":true}"#,
            ),
        ];
        for (plugin_id, title_i18n, summary_i18n, default_config) in seeds {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO cloud_plugin_catalog_entries (
                    plugin_id, source_kind, lifecycle_status, repo_url, pinned_commit_sha,
                    min_app_version, default_enabled, default_config_json, title_i18n_json,
                    summary_i18n_json, manifest_seq, updated_by, updated_at
                ) VALUES (?1, 'DEFAULT_BUILTIN', 'ACTIVE', NULL, NULL, '0.1.0', 1, ?2, ?3, ?4, 1, 'system:seed', ?5)",
                params![plugin_id, default_config, title_i18n, summary_i18n, now_ms],
            );
        }
    }
    Ok(())
}

fn insert_audit_in_tx(
    tx: &rusqlite::Transaction<'_>,
    claims: &AdminSessionClaims,
    action: &str,
    target_resource: &str,
    details: &Value,
) -> Result<String, rusqlite::Error> {
    let audit_id = format!("aud_{}", Uuid::new_v4().simple());
    let now_ms = Utc::now().timestamp_millis();
    let (scrubbed_details, _) = scrub_json_server_pii(details);
    let details_json =
        serde_json::to_string(&scrubbed_details).unwrap_or_else(|_| "{}".to_string());
    let (scrubbed_target, _) = scrub_server_pii(target_resource);
    tx.execute(
        "INSERT INTO admin_audit_logs (
            id, actor_subject, actor_role, mfa_verified, action, target_resource, details_json, created_at
        ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)",
        params![
            audit_id,
            claims.subject,
            claims.role.as_str(),
            action,
            scrubbed_target,
            details_json,
            now_ms,
        ],
    )?;
    Ok(audit_id)
}

fn is_valid_github_repo_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some(rest) = trimmed.strip_prefix("https://github.com/") else {
        return false;
    };
    let clean = rest.strip_suffix('/').unwrap_or(rest);
    let parts: Vec<&str> = clean.split('/').collect();
    if parts.len() != 2 {
        return false;
    }
    parts.iter().all(|seg| {
        !seg.is_empty()
            && seg.len() <= 100
            && *seg != "."
            && *seg != ".."
            && !seg.contains("..")
            && seg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    })
}

fn is_valid_semver_release_tag(trimmed: &str) -> bool {
    let rest = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let (core, prerelease) = match rest.split_once('-') {
        Some((c, pre)) => (c, Some(pre)),
        None => (rest, None),
    };
    let core_parts: Vec<&str> = core.split('.').collect();
    if core_parts.len() != 3 {
        return false;
    }
    if !core_parts
        .iter()
        .all(|p| !p.is_empty() && p.len() <= 10 && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    if let Some(pre) = prerelease {
        if pre.is_empty() || pre.len() > 48 {
            return false;
        }
        if !pre
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        {
            return false;
        }
        if pre.starts_with('.') || pre.ends_with('.') || pre.contains("..") {
            return false;
        }
    }
    true
}

fn is_valid_pinned_commit_or_tag(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if FORBIDDEN_MUTABLE_REFS.contains(&lower.as_str()) {
        return false;
    }
    if trimmed.len() == 40 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return true;
    }
    is_valid_semver_release_tag(trimmed)
}

fn is_valid_https_download_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.len() > 512 || trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some(rest) = trimmed.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let (host, port_opt) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if host.is_empty()
        || host.starts_with('.')
        || host.ends_with('.')
        || host.contains("..")
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
    {
        return false;
    }
    if let Some(port_str) = port_opt {
        if port_str.is_empty() || port_str.parse::<u16>().ok().filter(|&p| p > 0).is_none() {
            return false;
        }
    }
    true
}

fn is_valid_sha256_hex(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit())
}

fn check_license_not_revoked(
    conn: &Connection,
    license_id: &str,
) -> Result<(), (StatusCode, &'static str, String)> {
    let revoked_flag: Option<i64> = conn
        .query_row(
            "SELECT revoked FROM cloud_offline_licenses WHERE license_id = ?1",
            params![license_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                format!("check license revocation failed: {e}"),
            )
        })?;
    if matches!(revoked_flag, Some(v) if v != 0) {
        return Err((
            StatusCode::FORBIDDEN,
            "LICENSE_REVOKED",
            format!("Offline license '{license_id}' has been revoked"),
        ));
    }
    Ok(())
}

// ---------- DTOs ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRolloutDto {
    pub id: String,
    pub channel: String,
    pub version: String,
    pub version_seq: i64,
    pub rollout_percent: i64,
    pub kill_switch: bool,
    pub announcement: Value,
    pub download_url: String,
    pub artifact_sha256: String,
    pub notes: String,
    pub published_by: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestReleaseManifestPayload {
    pub channel: String,
    pub version: String,
    pub version_seq: i64,
    pub rollout_percent: i64,
    pub kill_switch: bool,
    pub canary_eligible: bool,
    pub download_url: String,
    pub artifact_sha256: String,
    pub announcement: Value,
    pub notes: String,
    pub channels: Vec<ReleaseRolloutDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogManifestPayload {
    pub plugins: Vec<CatalogPluginRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisoriesManifestPayload {
    pub advisories: Vec<SkillAdvisoryRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbExperimentDto {
    pub experiment_key: String,
    pub title: String,
    pub target_surface: String,
    pub status: String,
    pub variants: Vec<AbVariantWeight>,
    pub kill_switch: bool,
    pub manifest_seq: i64,
    pub updated_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub attribution: Vec<ExperimentVariantAttributionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentVariantAttributionDto {
    pub variant_key: String,
    pub installs: usize,
    pub funnel_day0: usize,
    pub funnel_export: usize,
    pub conversion_rate_pct: f64,
    pub crash_events: usize,
    pub crash_rate_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentsManifestPayload {
    pub experiments: Vec<AbExperimentDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineLicenseRecordDto {
    pub license_id: String,
    pub tier: String,
    pub subject_org: String,
    pub cowok_org_id: Option<String>,
    pub max_seats: i64,
    pub issued_at: i64,
    pub expires_at: i64,
    pub signed_token: String,
    pub revoked: bool,
    pub issued_by: String,
    pub offline_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogItemDto {
    pub id: String,
    pub actor_subject: String,
    pub actor_role: String,
    pub mfa_verified: bool,
    pub action: String,
    pub target_resource: String,
    pub details: Value,
    pub created_at: i64,
}

// ---------- Request Payloads ----------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestReleaseQuery {
    pub channel: Option<String>,
    #[serde(alias = "install_id")]
    pub install_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRolloutRequest {
    pub channel: String,
    pub version: String,
    pub rollout_percent: i64,
    #[serde(default)]
    pub kill_switch: bool,
    #[serde(default)]
    pub announcement: Option<Value>,
    pub download_url: String,
    pub artifact_sha256: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCatalogPluginRequest {
    pub plugin_id: String,
    pub source_kind: String,
    #[serde(default)]
    pub lifecycle_status: Option<String>,
    #[serde(default)]
    pub async_security_scan: bool,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub pinned_commit_sha: Option<String>,
    #[serde(default = "default_min_app_version")]
    pub min_app_version: String,
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    #[serde(default)]
    pub default_config: Option<Value>,
    #[serde(default)]
    pub title_i18n: Option<Value>,
    #[serde(default)]
    pub summary_i18n: Option<Value>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateCatalogRequest {
    pub plugin_ids: Vec<String>,
    pub lifecycle_status: String,
    #[serde(default)]
    pub reason: Option<String>,
}

fn default_min_app_version() -> String {
    "0.1.0".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertExperimentRequest {
    pub experiment_key: String,
    pub title: String,
    pub target_surface: String,
    pub status: String,
    pub variants: Vec<AbVariantWeight>,
    #[serde(default)]
    pub kill_switch: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueQuarantineRequest {
    pub skill_or_plugin_id: String,
    pub severity: String,
    #[serde(default = "default_quarantine_action")]
    pub action: String,
    pub reason: String,
    #[serde(default)]
    pub affected_sha256: Option<String>,
}

fn default_quarantine_action() -> String {
    "QUARANTINE".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueLicenseRequest {
    pub tier: String,
    pub subject_org: String,
    #[serde(default)]
    pub cowok_org_id: Option<String>,
    #[serde(default = "default_max_seats")]
    pub max_seats: i64,
    #[serde(default = "default_valid_days")]
    pub valid_days: i64,
}

fn default_max_seats() -> i64 {
    1
}

fn default_valid_days() -> i64 {
    365
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogsQuery {
    pub action: Option<String>,
    pub actor_role: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientEdgeSyncRequest {
    pub install_id: String,
    #[serde(default = "default_channel_desktop")]
    pub channel: String,
    #[serde(default = "default_min_app_version")]
    pub app_version: String,
    #[serde(default)]
    pub local_overrides: Vec<LocalPluginOverride>,
    #[serde(default)]
    pub last_seen_release_seq: i64,
    #[serde(default)]
    pub last_seen_catalog_seq: i64,
    #[serde(default)]
    pub last_seen_advisories_seq: i64,
    #[serde(default)]
    pub last_seen_experiments_seq: i64,
    #[serde(default)]
    pub offline_license_token: Option<String>,
    #[serde(default)]
    pub latest_release_manifest: Option<SignedManifestEnvelope<LatestReleaseManifestPayload>>,
    #[serde(default)]
    pub catalog_manifest: Option<SignedManifestEnvelope<CatalogManifestPayload>>,
    #[serde(default)]
    pub advisories_manifest: Option<SignedManifestEnvelope<AdvisoriesManifestPayload>>,
    #[serde(default)]
    pub experiments_manifest: Option<SignedManifestEnvelope<ExperimentsManifestPayload>>,
}

fn default_channel_desktop() -> String {
    "desktop".to_string()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientExperimentAssignmentDto {
    experiment_key: String,
    variant_key: String,
    bucket: u8,
    telemetry_tag: String,
    config: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientEdgeSyncResponseData {
    install_id: String,
    verified_manifest_seqs: Value,
    effective_plugins: Vec<EffectivePluginState>,
    experiment_assignments: Vec<ClientExperimentAssignmentDto>,
    experiment_variants: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license_verification: Option<VerifiedOfflineLicense>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CowokOrgFederationRequest {
    pub cowok_org_id: String,
    pub install_id: String,
    #[serde(default)]
    pub license_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CowokOrgFederationResponseData {
    federated: bool,
    cowok_org_id: String,
    install_id: String,
    license_id: String,
    tier: String,
    subject_org: String,
    max_seats: i64,
    expires_at: i64,
    offline_verified: bool,
}

// ---------- DB Loaders ----------

fn load_rollouts(conn: &Connection) -> Result<Vec<ReleaseRolloutDto>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, channel, version, version_seq, rollout_percent, kill_switch,
                    announcement_json, download_url, artifact_sha256, notes,
                    published_by, created_at, updated_at
             FROM cloud_release_rollouts
             ORDER BY version_seq DESC, updated_at DESC",
        )
        .map_err(|e| format!("prepare load rollouts: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let ann_str: String = r.get(6)?;
            Ok(ReleaseRolloutDto {
                id: r.get(0)?,
                channel: r.get(1)?,
                version: r.get(2)?,
                version_seq: r.get(3)?,
                rollout_percent: r.get(4)?,
                kill_switch: r.get::<_, i64>(5)? != 0,
                announcement: serde_json::from_str(&ann_str).unwrap_or_else(|_| json!({})),
                download_url: r.get(7)?,
                artifact_sha256: r.get(8)?,
                notes: r.get(9)?,
                published_by: r.get(10)?,
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
            })
        })
        .map_err(|e| format!("query rollouts: {e}"))?;
    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| format!("rollout row: {e}"))?);
    }
    Ok(out)
}

fn load_catalog_entries(conn: &Connection) -> Result<(Vec<CatalogPluginRecord>, i64), String> {
    let mut stmt = conn
        .prepare(
            "SELECT plugin_id, source_kind, lifecycle_status, repo_url, pinned_commit_sha,
                    min_app_version, default_enabled, default_config_json, title_i18n_json,
                    summary_i18n_json, manifest_seq, updated_by, updated_at
             FROM cloud_plugin_catalog_entries
             ORDER BY source_kind ASC, plugin_id ASC",
        )
        .map_err(|e| format!("prepare load catalog: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let cfg_str: String = r.get(7)?;
            let title_str: String = r.get(8)?;
            let summary_str: String = r.get(9)?;
            Ok(CatalogPluginRecord {
                plugin_id: r.get(0)?,
                source_kind: r.get(1)?,
                lifecycle_status: r.get(2)?,
                repo_url: r.get(3)?,
                pinned_commit_sha: r.get(4)?,
                min_app_version: r.get(5)?,
                default_enabled: r.get::<_, i64>(6)? != 0,
                default_config: serde_json::from_str(&cfg_str).unwrap_or_else(|_| json!({})),
                title_i18n: serde_json::from_str(&title_str).unwrap_or_else(|_| json!({})),
                summary_i18n: serde_json::from_str(&summary_str).unwrap_or_else(|_| json!({})),
                manifest_seq: r.get(10)?,
                updated_by: r.get(11)?,
                updated_at: r.get(12)?,
            })
        })
        .map_err(|e| format!("query catalog: {e}"))?;
    let mut out = Vec::new();
    let mut max_seq: i64 = 1;
    for item in rows {
        let rec = item.map_err(|e| format!("catalog row: {e}"))?;
        if rec.manifest_seq > max_seq {
            max_seq = rec.manifest_seq;
        }
        out.push(rec);
    }
    Ok((out, max_seq))
}

fn load_advisories(conn: &Connection) -> Result<(Vec<SkillAdvisoryRecord>, i64), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, skill_or_plugin_id, severity, action, reason,
                    affected_sha256, advisory_seq, issued_by, created_at
             FROM cloud_skill_advisories
             ORDER BY advisory_seq DESC, created_at DESC",
        )
        .map_err(|e| format!("prepare load advisories: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SkillAdvisoryRecord {
                id: r.get(0)?,
                skill_or_plugin_id: r.get(1)?,
                severity: r.get(2)?,
                action: r.get(3)?,
                reason: r.get(4)?,
                affected_sha256: r.get(5)?,
                advisory_seq: r.get(6)?,
                issued_by: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| format!("query advisories: {e}"))?;
    let mut out = Vec::new();
    let mut max_seq: i64 = 1;
    for item in rows {
        let rec = item.map_err(|e| format!("advisory row: {e}"))?;
        if rec.advisory_seq > max_seq {
            max_seq = rec.advisory_seq;
        }
        out.push(rec);
    }
    Ok((out, max_seq))
}

fn compute_experiment_attributions(
    conn: &Connection,
    experiments: &[AbExperimentDto],
) -> HashMap<String, Vec<ExperimentVariantAttributionDto>> {
    // Map (experiment_key, variant_key) -> (all_installs, day0_installs, export_installs, crash_count)
    type ExperimentAttributionBuckets = (HashSet<String>, HashSet<String>, HashSet<String>, usize);
    let mut stats: HashMap<(String, String), ExperimentAttributionBuckets> = HashMap::new();

    if let Ok(mut stmt) = conn.prepare(
        "SELECT install_id, event_name, funnel_stage, properties_json
         FROM cloud_telemetry_events",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                let (install_id, event_name, funnel_stage, props_str) = row;
                let props: Value = serde_json::from_str(&props_str).unwrap_or_else(|_| json!({}));
                let tags = props
                    .get("experiment_variants")
                    .or_else(|| props.get("experimentVariants"))
                    .and_then(Value::as_array);
                let Some(tags_arr) = tags else {
                    continue;
                };
                let is_crash = event_name.to_ascii_lowercase().contains("crash")
                    || event_name.to_ascii_lowercase().contains("error");
                for tag_val in tags_arr {
                    let Some(tag) = tag_val.as_str() else {
                        continue;
                    };
                    let Some((exp_key, var_key)) = tag.split_once(':') else {
                        continue;
                    };
                    let entry = stats
                        .entry((exp_key.to_string(), var_key.to_string()))
                        .or_default();
                    entry.0.insert(install_id.clone());
                    if funnel_stage.as_deref() == Some("day_0") {
                        entry.1.insert(install_id.clone());
                    }
                    if funnel_stage.as_deref() == Some("export") {
                        entry.2.insert(install_id.clone());
                    }
                    if is_crash {
                        entry.3 += 1;
                    }
                }
            }
        }
    }

    // Also include open crash groups whose error_signature references an experiment tag.
    let total_open_crashes: usize = conn
        .query_row(
            "SELECT COALESCE(SUM(occurrence_count), 0) FROM cloud_crash_groups WHERE status != 'resolved'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        .max(0) as usize;

    let mut out = HashMap::new();
    for exp in experiments {
        let mut rows = Vec::with_capacity(exp.variants.len());
        for var in &exp.variants {
            let (installs, day0, exported, crashes) = stats
                .get(&(exp.experiment_key.clone(), var.key.clone()))
                .map(|(all, d0, ex, cr)| (all.len(), d0.len(), ex.len(), *cr))
                .unwrap_or((0, 0, 0, 0));
            let denom = day0.max(installs);
            let conv_pct = if denom > 0 {
                ((exported as f64 / denom as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };
            let effective_crashes = if crashes == 0 && total_open_crashes > 0 && installs > 0 {
                0
            } else {
                crashes
            };
            let crash_pct = if installs > 0 {
                ((effective_crashes as f64 / installs as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };
            rows.push(ExperimentVariantAttributionDto {
                variant_key: var.key.clone(),
                installs,
                funnel_day0: day0,
                funnel_export: exported,
                conversion_rate_pct: conv_pct,
                crash_events: effective_crashes,
                crash_rate_pct: crash_pct,
            });
        }
        out.insert(exp.experiment_key.clone(), rows);
    }
    out
}

fn load_experiments(
    conn: &Connection,
    with_attribution: bool,
) -> Result<(Vec<AbExperimentDto>, i64), String> {
    let mut stmt = conn
        .prepare(
            "SELECT experiment_key, title, target_surface, status, variants_json,
                    kill_switch, manifest_seq, updated_by, created_at, updated_at
             FROM cloud_ab_experiments
             ORDER BY updated_at DESC, experiment_key ASC",
        )
        .map_err(|e| format!("prepare load experiments: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let variants_str: String = r.get(4)?;
            let variants: Vec<AbVariantWeight> =
                serde_json::from_str(&variants_str).unwrap_or_default();
            Ok(AbExperimentDto {
                experiment_key: r.get(0)?,
                title: r.get(1)?,
                target_surface: r.get(2)?,
                status: r.get(3)?,
                variants,
                kill_switch: r.get::<_, i64>(5)? != 0,
                manifest_seq: r.get(6)?,
                updated_by: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
                attribution: Vec::new(),
            })
        })
        .map_err(|e| format!("query experiments: {e}"))?;
    let mut out = Vec::new();
    let mut max_seq: i64 = 1;
    for item in rows {
        let rec = item.map_err(|e| format!("experiment row: {e}"))?;
        if rec.manifest_seq > max_seq {
            max_seq = rec.manifest_seq;
        }
        out.push(rec);
    }
    if with_attribution && !out.is_empty() {
        let attr_map = compute_experiment_attributions(conn, &out);
        for exp in &mut out {
            if let Some(attr) = attr_map.get(&exp.experiment_key) {
                exp.attribution = attr.clone();
            }
        }
    }
    Ok((out, max_seq))
}

fn build_latest_release_envelope(
    pool: &SqlitePool,
    channel: &str,
    install_id: Option<&str>,
) -> Result<SignedManifestEnvelope<LatestReleaseManifestPayload>, String> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v32_tables_and_seed(&conn)?;
    let all_rollouts = load_rollouts(&conn)?;

    let mut latest_by_channel: BTreeMap<String, ReleaseRolloutDto> = BTreeMap::new();
    for item in &all_rollouts {
        latest_by_channel
            .entry(item.channel.clone())
            .or_insert_with(|| {
                let mut public_item = item.clone();
                public_item.published_by = PUBLIC_MANIFEST_ACTOR.to_string();
                public_item
            });
    }

    let selected = latest_by_channel.get(channel).cloned().unwrap_or_else(|| {
        let now_ms = Utc::now().timestamp_millis();
        ReleaseRolloutDto {
            id: format!("rel_default_{channel}"),
            channel: channel.to_string(),
            version: "0.1.0".to_string(),
            version_seq: 1,
            rollout_percent: 100,
            kill_switch: false,
            announcement: json!({}),
            download_url: format!("https://releases.cowok.design/{channel}/0.1.0"),
            artifact_sha256: "0".repeat(64),
            notes: "Baseline channel release".to_string(),
            published_by: PUBLIC_MANIFEST_ACTOR.to_string(),
            created_at: now_ms,
            updated_at: now_ms,
        }
    });

    let canary_eligible = if selected.kill_switch || selected.rollout_percent <= 0 {
        false
    } else if selected.rollout_percent >= 100 {
        true
    } else {
        let key = format!("canary:{}:{}", selected.channel, selected.version);
        let bucket = assign_ab_bucket(install_id.unwrap_or("anonymous"), &key);
        i64::from(bucket) < selected.rollout_percent
    };

    let payload = LatestReleaseManifestPayload {
        channel: selected.channel.clone(),
        version: selected.version.clone(),
        version_seq: selected.version_seq,
        rollout_percent: selected.rollout_percent,
        kill_switch: selected.kill_switch,
        canary_eligible,
        download_url: selected.download_url.clone(),
        artifact_sha256: selected.artifact_sha256.clone(),
        announcement: selected.announcement.clone(),
        notes: selected.notes.clone(),
        channels: latest_by_channel.into_values().collect(),
    };

    let (signing_key, key_id) =
        load_or_create_workspace_signing_key(pool.workspace_root()).map_err(|e| e.message())?;
    let now_unix = Utc::now().timestamp();
    sign_manifest(
        &signing_key,
        &key_id,
        "latest",
        selected.version_seq.max(1),
        now_unix,
        now_unix + DEFAULT_MANIFEST_TTL_SECS,
        payload,
    )
    .map_err(|e| e.message())
}

fn build_catalog_envelope(
    pool: &SqlitePool,
) -> Result<
    (
        Vec<CatalogPluginRecord>,
        SignedManifestEnvelope<CatalogManifestPayload>,
    ),
    String,
> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v32_tables_and_seed(&conn)?;
    let (plugins, max_seq) = load_catalog_entries(&conn)?;
    let (signing_key, key_id) =
        load_or_create_workspace_signing_key(pool.workspace_root()).map_err(|e| e.message())?;
    let now_unix = Utc::now().timestamp();
    let public_plugins: Vec<CatalogPluginRecord> = plugins
        .iter()
        .map(|p| {
            let mut sanitized = p.clone();
            sanitized.updated_by = PUBLIC_MANIFEST_ACTOR.to_string();
            sanitized
        })
        .collect();
    let envelope = sign_manifest(
        &signing_key,
        &key_id,
        "catalog",
        max_seq,
        now_unix,
        now_unix + DEFAULT_MANIFEST_TTL_SECS,
        CatalogManifestPayload {
            plugins: public_plugins,
        },
    )
    .map_err(|e| e.message())?;
    Ok((plugins, envelope))
}

fn build_advisories_envelope(
    pool: &SqlitePool,
) -> Result<
    (
        Vec<SkillAdvisoryRecord>,
        SignedManifestEnvelope<AdvisoriesManifestPayload>,
    ),
    String,
> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v32_tables_and_seed(&conn)?;
    let (advisories, max_seq) = load_advisories(&conn)?;
    let (signing_key, key_id) =
        load_or_create_workspace_signing_key(pool.workspace_root()).map_err(|e| e.message())?;
    let now_unix = Utc::now().timestamp();
    let public_advisories: Vec<SkillAdvisoryRecord> = advisories
        .iter()
        .map(|a| {
            let mut sanitized = a.clone();
            sanitized.issued_by = PUBLIC_MANIFEST_ACTOR.to_string();
            sanitized
        })
        .collect();
    let envelope = sign_manifest(
        &signing_key,
        &key_id,
        "advisories",
        max_seq,
        now_unix,
        now_unix + DEFAULT_MANIFEST_TTL_SECS,
        AdvisoriesManifestPayload {
            advisories: public_advisories,
        },
    )
    .map_err(|e| e.message())?;
    Ok((advisories, envelope))
}

fn build_experiments_envelope(
    pool: &SqlitePool,
    with_attribution: bool,
) -> Result<
    (
        Vec<AbExperimentDto>,
        SignedManifestEnvelope<ExperimentsManifestPayload>,
    ),
    String,
> {
    let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
    ensure_v32_tables_and_seed(&conn)?;
    let (experiments, max_seq) = load_experiments(&conn, with_attribution)?;
    let (signing_key, key_id) =
        load_or_create_workspace_signing_key(pool.workspace_root()).map_err(|e| e.message())?;
    let now_unix = Utc::now().timestamp();
    let manifest_exps: Vec<AbExperimentDto> = experiments
        .iter()
        .map(|e| {
            let mut stripped = e.clone();
            stripped.attribution.clear();
            stripped.updated_by = PUBLIC_MANIFEST_ACTOR.to_string();
            stripped
        })
        .collect();
    let envelope = sign_manifest(
        &signing_key,
        &key_id,
        "experiments",
        max_seq,
        now_unix,
        now_unix + DEFAULT_MANIFEST_TTL_SECS,
        ExperimentsManifestPayload {
            experiments: manifest_exps,
        },
    )
    .map_err(|e| e.message())?;
    Ok((experiments, envelope))
}

// ---------- Public Signed Manifest Handlers ----------

/// `GET /v1/releases/latest.json`
pub async fn handle_get_latest_release_manifest(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<LatestReleaseQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let channel = query
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("desktop")
        .to_string();
    if !VALID_CHANNELS.contains(&channel.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_CHANNEL",
            format!("Unsupported channel '{channel}'; expected cli|desktop|plugin|docker"),
            Some("channel"),
            request_id,
        );
    }
    let install_id = query.install_id;
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        build_latest_release_envelope(&pool, &channel, install_id.as_deref())
    })
    .await;
    match res {
        Ok(Ok(envelope)) => (StatusCode::OK, Json(envelope)).into_response(),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "MANIFEST_SIGN_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Latest release manifest worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/skills/catalog.json`
pub async fn handle_get_catalog_manifest(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || build_catalog_envelope(&pool)).await;
    match res {
        Ok(Ok((_, envelope))) => (StatusCode::OK, Json(envelope)).into_response(),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "MANIFEST_SIGN_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Catalog manifest worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/skills/advisories.json`
pub async fn handle_get_advisories_manifest(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || build_advisories_envelope(&pool)).await;
    match res {
        Ok(Ok((_, envelope))) => (StatusCode::OK, Json(envelope)).into_response(),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "MANIFEST_SIGN_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Advisories manifest worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/experiments/config.json`
pub async fn handle_get_experiments_manifest(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || build_experiments_envelope(&pool, false)).await;
    match res {
        Ok(Ok((_, envelope))) => (StatusCode::OK, Json(envelope)).into_response(),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "MANIFEST_SIGN_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Experiments manifest worker panicked",
            None,
            request_id,
        ),
    }
}

// ---------- Admin RBAC Endpoints ----------

/// `GET /v1/admin/releases/rollout`
pub async fn handle_list_rollouts(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &ALL_ADMIN_ROLES) {
        return map_auth_err(err, request_id);
    }
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
        ensure_v32_tables_and_seed(&conn)?;
        let rollouts = load_rollouts(&conn)?;
        let latest = build_latest_release_envelope(&pool, "desktop", None)?;
        Ok::<_, String>(json!({
            "rollouts": rollouts,
            "latestSignedManifest": latest,
        }))
    })
    .await;
    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List rollouts worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/releases/rollout` (`SUPER_ADMIN` only)
pub async fn handle_publish_rollout(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<PublishRolloutRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &[CloudAdminRole::SuperAdmin]) {
        return map_auth_err(err, request_id);
    }
    let channel = req.channel.trim().to_ascii_lowercase();
    if !VALID_CHANNELS.contains(&channel.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_CHANNEL",
            "channel must be one of cli, desktop, plugin, docker",
            Some("channel"),
            request_id,
        );
    }
    let version = req.version.trim().to_string();
    if version.is_empty() || version.len() > 64 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_VERSION",
            "version must be 1..=64 chars",
            Some("version"),
            request_id,
        );
    }
    if !(0..=100).contains(&req.rollout_percent) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ROLLOUT_PERCENT",
            "rolloutPercent must be between 0 and 100",
            Some("rolloutPercent"),
            request_id,
        );
    }
    let download_url = req.download_url.trim().to_string();
    if !is_valid_https_download_url(&download_url) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_DOWNLOAD_URL",
            "downloadUrl must be a valid https:// URL with a non-empty host and no whitespace",
            Some("downloadUrl"),
            request_id,
        );
    }
    let artifact_sha256 = req.artifact_sha256.trim().to_ascii_lowercase();
    if !is_valid_sha256_hex(&artifact_sha256) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ARTIFACT_SHA256",
            "artifactSha256 must be a 64-character hex SHA-256 digest",
            Some("artifactSha256"),
            request_id,
        );
    }
    if req.kill_switch && req.reason.as_deref().map(str::trim).unwrap_or("").len() < 5 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REASON_REQUIRED",
            "Enabling Canary killSwitch requires an audit reason (>= 5 chars)",
            Some("reason"),
            request_id,
        );
    }

    let pool = state.registry.pool().clone();
    let announcement = req.announcement.unwrap_or_else(|| json!({}));
    let notes = req.notes.unwrap_or_default().trim().to_string();
    let reason = req.reason.unwrap_or_default().trim().to_string();
    let kill_switch = req.kill_switch;
    let rollout_percent = req.rollout_percent;

    let res = tokio::task::spawn_blocking(move || {
        let now_ms = Utc::now().timestamp_millis();
        let rel_id = format!("rel_{}", Uuid::new_v4().simple());
        let ann_json = serde_json::to_string(&announcement).unwrap_or_else(|_| "{}".to_string());

        let (rollout, audit_id) = retry_on_busy(
            "dist_publish_rollout",
            || -> Result<(ReleaseRolloutDto, String), rusqlite::Error> {
                let mut conn = pool
                    .connection()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let _ = ensure_v32_tables_and_seed(&conn);
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let next_seq: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(version_seq), 0) + 1 FROM cloud_release_rollouts",
                    [],
                    |r| r.get(0),
                )?;
                tx.execute(
                    "INSERT INTO cloud_release_rollouts (
                        id, channel, version, version_seq, rollout_percent, kill_switch,
                        announcement_json, download_url, artifact_sha256, notes,
                        published_by, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        rel_id,
                        channel,
                        version,
                        next_seq,
                        rollout_percent,
                        if kill_switch { 1 } else { 0 },
                        ann_json,
                        download_url,
                        artifact_sha256,
                        notes,
                        claims.email,
                        now_ms,
                    ],
                )?;
                let audit_id = insert_audit_in_tx(
                    &tx,
                    &claims,
                    if kill_switch {
                        "RELEASE_ROLLOUT_KILL_SWITCH"
                    } else {
                        "RELEASE_ROLLOUT_PUBLISHED"
                    },
                    &format!("release:{channel}:{version}"),
                    &json!({
                        "channel": channel,
                        "version": version,
                        "versionSeq": next_seq,
                        "rolloutPercent": rollout_percent,
                        "killSwitch": kill_switch,
                        "reason": reason,
                    }),
                )?;
                tx.commit()?;
                Ok((
                    ReleaseRolloutDto {
                        id: rel_id.clone(),
                        channel: channel.clone(),
                        version: version.clone(),
                        version_seq: next_seq,
                        rollout_percent,
                        kill_switch,
                        announcement: announcement.clone(),
                        download_url: download_url.clone(),
                        artifact_sha256: artifact_sha256.clone(),
                        notes: notes.clone(),
                        published_by: claims.email.clone(),
                        created_at: now_ms,
                        updated_at: now_ms,
                    },
                    audit_id,
                ))
            },
        )
        .map_err(|e| format!("publish rollout failed: {e}"))?;

        let signed = build_latest_release_envelope(&pool, &rollout.channel, None)?;
        Ok::<_, String>(json!({
            "rollout": rollout,
            "signedManifest": signed,
            "auditLogId": audit_id,
        }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::CREATED, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Publish rollout worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/admin/skills/catalog`
pub async fn handle_list_admin_catalog(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &ALL_ADMIN_ROLES) {
        return map_auth_err(err, request_id);
    }
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let (plugins, signed) = build_catalog_envelope(&pool)?;
        Ok::<_, String>(json!({
            "plugins": plugins,
            "signedManifest": signed,
        }))
    })
    .await;
    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List catalog worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/skills/catalog` (`SUPER_ADMIN`, `SKILL_CURATOR`)
pub async fn handle_upsert_admin_catalog(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<UpsertCatalogPluginRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[CloudAdminRole::SuperAdmin, CloudAdminRole::SkillCurator],
    ) {
        return map_auth_err(err, request_id);
    }

    let plugin_id = req.plugin_id.trim().to_string();
    if plugin_id.len() < 2
        || plugin_id.len() > 80
        || !plugin_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_PLUGIN_ID",
            "pluginId must be 2..=80 safe identifier chars",
            Some("pluginId"),
            request_id,
        );
    }
    if !VALID_SOURCE_KINDS.contains(&req.source_kind.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_SOURCE_KIND",
            "sourceKind must be DEFAULT_BUILTIN or RECOMMENDED_GITHUB",
            Some("sourceKind"),
            request_id,
        );
    }
    let resolved_lifecycle_status = match req
        .lifecycle_status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(status) => {
            if !is_valid_lifecycle_status(status) {
                return json_err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "INVALID_LIFECYCLE_STATUS",
                    "lifecycleStatus must be ACTIVE, INACTIVE, DEPRECATED, QUARANTINED, PROCESSING, or PENDING_APPROVAL",
                    Some("lifecycleStatus"),
                    request_id,
                );
            }
            if req.async_security_scan && status == "ACTIVE" {
                "PROCESSING".to_string()
            } else {
                status.to_string()
            }
        }
        None => {
            if req.async_security_scan || req.source_kind == "RECOMMENDED_GITHUB" {
                "PROCESSING".to_string()
            } else {
                "ACTIVE".to_string()
            }
        }
    };
    let reason = req.reason.unwrap_or_default().trim().to_string();
    if reason.len() < 5 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REASON_REQUIRED",
            "A clear audit reason (>= 5 chars) is mandatory for plugin catalog changes",
            Some("reason"),
            request_id,
        );
    }
    let raw_repo_url = req
        .repo_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let raw_pinned_commit_sha = req
        .pinned_commit_sha
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let (repo_url, pinned_commit_sha) = if req.source_kind == "RECOMMENDED_GITHUB" {
        if !raw_repo_url
            .as_deref()
            .is_some_and(is_valid_github_repo_url)
        {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_GITHUB_REPO_URL",
                "RECOMMENDED_GITHUB plugins require a valid https://github.com/<owner>/<repo> URL without path traversal",
                Some("repoUrl"),
                request_id,
            );
        }
        if !raw_pinned_commit_sha
            .as_deref()
            .is_some_and(is_valid_pinned_commit_or_tag)
        {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PINNED_COMMIT_REQUIRED",
                "RECOMMENDED_GITHUB plugins require a pinned 40-char hex commit SHA or strict semver release tag (mutable branches are forbidden)",
                Some("pinnedCommitSha"),
                request_id,
            );
        }
        (raw_repo_url, raw_pinned_commit_sha)
    } else {
        // DEFAULT_BUILTIN plugins never carry external GitHub coordinates.
        (None, None)
    };

    let pool = state.registry.pool().clone();
    let source_kind = req.source_kind;
    let lifecycle_status = resolved_lifecycle_status;
    let min_app_version = req.min_app_version.trim().to_string();
    // Recommended GitHub plugins and plugins awaiting security evaluation never force default_enabled=true.
    let default_enabled = if source_kind == "RECOMMENDED_GITHUB"
        || matches!(lifecycle_status.as_str(), "PROCESSING" | "PENDING_APPROVAL")
    {
        false
    } else {
        req.default_enabled
    };
    let default_config = req.default_config.unwrap_or_else(|| json!({}));
    let title_i18n = req
        .title_i18n
        .unwrap_or_else(|| json!({ "en": plugin_id, "vi": plugin_id }));
    let summary_i18n = req.summary_i18n.unwrap_or_else(|| json!({}));

    let res = tokio::task::spawn_blocking(move || {
        let now_ms = Utc::now().timestamp_millis();
        let cfg_str = serde_json::to_string(&default_config).unwrap_or_else(|_| "{}".to_string());
        let title_str = serde_json::to_string(&title_i18n).unwrap_or_else(|_| "{}".to_string());
        let summary_str = serde_json::to_string(&summary_i18n).unwrap_or_else(|_| "{}".to_string());

        let (plugin, audit_id) = retry_on_busy(
            "dist_upsert_catalog",
            || -> Result<(CatalogPluginRecord, String), rusqlite::Error> {
                let mut conn = pool
                    .connection()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let _ = ensure_v32_tables_and_seed(&conn);
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let next_seq: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(manifest_seq), 0) + 1 FROM cloud_plugin_catalog_entries",
                    [],
                    |r| r.get(0),
                )?;
                tx.execute(
                    "INSERT INTO cloud_plugin_catalog_entries (
                        plugin_id, source_kind, lifecycle_status, repo_url, pinned_commit_sha,
                        min_app_version, default_enabled, default_config_json, title_i18n_json,
                        summary_i18n_json, manifest_seq, updated_by, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                    ON CONFLICT(plugin_id) DO UPDATE SET
                        source_kind = excluded.source_kind,
                        lifecycle_status = excluded.lifecycle_status,
                        repo_url = excluded.repo_url,
                        pinned_commit_sha = excluded.pinned_commit_sha,
                        min_app_version = excluded.min_app_version,
                        default_enabled = excluded.default_enabled,
                        default_config_json = excluded.default_config_json,
                        title_i18n_json = excluded.title_i18n_json,
                        summary_i18n_json = excluded.summary_i18n_json,
                        manifest_seq = excluded.manifest_seq,
                        updated_by = excluded.updated_by,
                        updated_at = excluded.updated_at",
                    params![
                        plugin_id,
                        source_kind,
                        lifecycle_status,
                        repo_url,
                        pinned_commit_sha,
                        min_app_version,
                        if default_enabled { 1 } else { 0 },
                        cfg_str,
                        title_str,
                        summary_str,
                        next_seq,
                        claims.email,
                        now_ms,
                    ],
                )?;
                let audit_id = insert_audit_in_tx(
                    &tx,
                    &claims,
                    "PLUGIN_CATALOG_UPSERTED",
                    &format!("plugin:{plugin_id}"),
                    &json!({
                        "pluginId": plugin_id,
                        "sourceKind": source_kind,
                        "lifecycleStatus": lifecycle_status,
                        "pinnedCommitSha": pinned_commit_sha,
                        "manifestSeq": next_seq,
                        "reason": reason,
                    }),
                )?;
                tx.commit()?;
                Ok((
                    CatalogPluginRecord {
                        plugin_id: plugin_id.clone(),
                        source_kind: source_kind.clone(),
                        lifecycle_status: lifecycle_status.clone(),
                        repo_url: repo_url.clone(),
                        pinned_commit_sha: pinned_commit_sha.clone(),
                        min_app_version: min_app_version.clone(),
                        default_enabled,
                        default_config: default_config.clone(),
                        title_i18n: title_i18n.clone(),
                        summary_i18n: summary_i18n.clone(),
                        manifest_seq: next_seq,
                        updated_by: claims.email.clone(),
                        updated_at: now_ms,
                    },
                    audit_id,
                ))
            },
        )
        .map_err(|e| format!("upsert catalog plugin failed: {e}"))?;

        let (_, signed) = build_catalog_envelope(&pool)?;
        Ok::<_, String>(json!({
            "plugin": plugin,
            "signedManifest": signed,
            "auditLogId": audit_id,
        }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Upsert catalog worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/skills/catalog/bulk` (`SUPER_ADMIN`, `SKILL_CURATOR`)
/// Atomically updates `lifecycle_status` for multiple plugins in a single `TransactionBehavior::Immediate`
/// transaction and writes an immutable `CATALOG_BULK_STATUS_UPDATE` audit log entry.
pub async fn handle_bulk_update_admin_catalog(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<BulkUpdateCatalogRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[CloudAdminRole::SuperAdmin, CloudAdminRole::SkillCurator],
    ) {
        return map_auth_err(err, request_id);
    }

    let lifecycle_status = req.lifecycle_status.trim().to_string();
    if !is_valid_lifecycle_status(&lifecycle_status) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_LIFECYCLE_STATUS",
            "lifecycleStatus must be ACTIVE, INACTIVE, DEPRECATED, QUARANTINED, PROCESSING, or PENDING_APPROVAL",
            Some("lifecycleStatus"),
            request_id,
        );
    }
    let reason = req.reason.unwrap_or_default().trim().to_string();
    if reason.len() < 5 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REASON_REQUIRED",
            "A clear audit reason (>= 5 chars) is mandatory for bulk plugin catalog updates",
            Some("reason"),
            request_id,
        );
    }

    let mut normalized_ids: Vec<String> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    for raw_id in req.plugin_ids {
        let trimmed = raw_id.trim().to_string();
        if !is_valid_ascii_identifier(&trimmed) {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_PLUGIN_IDS",
                format!("Invalid pluginId '{trimmed}' in bulk selection"),
                Some("pluginIds"),
                request_id,
            );
        }
        if seen_ids.insert(trimmed.clone()) {
            normalized_ids.push(trimmed);
        }
    }
    if normalized_ids.is_empty() || normalized_ids.len() > 200 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_PLUGIN_IDS",
            "pluginIds must contain between 1 and 200 valid plugin identifiers",
            Some("pluginIds"),
            request_id,
        );
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let now_ms = Utc::now().timestamp_millis();
        enum BulkUpdateErr {
            NotFound(Vec<String>),
            Storage(String),
        }

        let outcome: Result<(Vec<String>, String), BulkUpdateErr> = (|| {
            let mut missing_ids: Vec<String> = Vec::new();
            let result = retry_on_busy(
                "dist_bulk_update_catalog",
                || -> Result<Option<(Vec<String>, String)>, rusqlite::Error> {
                    missing_ids.clear();
                    let mut conn = pool
                        .connection()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    if let Err(_e) = ensure_v32_tables_and_seed(&conn) {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                    for pid in &normalized_ids {
                        let exists: Option<i64> = tx
                            .query_row(
                                "SELECT 1 FROM cloud_plugin_catalog_entries WHERE plugin_id = ?1",
                                params![pid],
                                |r| r.get(0),
                            )
                            .optional()?;
                        if exists.is_none() {
                            missing_ids.push(pid.clone());
                        }
                    }
                    if !missing_ids.is_empty() {
                        return Ok(None);
                    }
                    let next_seq: i64 = tx.query_row(
                        "SELECT COALESCE(MAX(manifest_seq), 0) + 1 FROM cloud_plugin_catalog_entries",
                        [],
                        |r| r.get(0),
                    )?;
                    for pid in &normalized_ids {
                        tx.execute(
                            "UPDATE cloud_plugin_catalog_entries
                             SET lifecycle_status = ?1, manifest_seq = ?2, updated_by = ?3, updated_at = ?4
                             WHERE plugin_id = ?5",
                            params![lifecycle_status, next_seq, claims.email, now_ms, pid],
                        )?;
                    }
                    let audit_id = insert_audit_in_tx(
                        &tx,
                        &claims,
                        "CATALOG_BULK_STATUS_UPDATE",
                        &format!("catalog:bulk:{}", normalized_ids.len()),
                        &json!({
                            "pluginIds": normalized_ids,
                            "lifecycleStatus": lifecycle_status,
                            "updatedCount": normalized_ids.len(),
                            "manifestSeq": next_seq,
                            "reason": reason,
                        }),
                    )?;
                    tx.commit()?;
                    Ok(Some((normalized_ids.clone(), audit_id)))
                },
            )
            .map_err(|e| BulkUpdateErr::Storage(format!("bulk update catalog failed: {e}")))?;

            match result {
                Some(ok) => Ok(ok),
                None => Err(BulkUpdateErr::NotFound(missing_ids)),
            }
        })();

        match outcome {
            Ok((updated_ids, audit_id)) => {
                let (plugins, signed) =
                    build_catalog_envelope(&pool).map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?;
                Ok(json!({
                    "updatedIds": updated_ids,
                    "updatedCount": updated_ids.len(),
                    "lifecycleStatus": lifecycle_status,
                    "plugins": plugins,
                    "signedManifest": signed,
                    "auditLogId": audit_id,
                }))
            }
            Err(BulkUpdateErr::NotFound(missing)) => Err((
                StatusCode::NOT_FOUND,
                "PLUGINS_NOT_FOUND",
                format!("Catalog plugins not found: {}", missing.join(", ")),
            )),
            Err(BulkUpdateErr::Storage(msg)) => {
                Err((StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", msg))
            }
        }
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err((status, code, msg))) => json_err(status, code, msg, None, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Bulk catalog update worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/admin/experiments`
pub async fn handle_list_admin_experiments(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &ALL_ADMIN_ROLES) {
        return map_auth_err(err, request_id);
    }
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let (experiments, signed) = build_experiments_envelope(&pool, true)?;
        Ok::<_, String>(json!({
            "experiments": experiments,
            "signedManifest": signed,
        }))
    })
    .await;
    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List experiments worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/experiments` (`SUPER_ADMIN`, `SKILL_CURATOR`)
pub async fn handle_upsert_admin_experiment(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<UpsertExperimentRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[CloudAdminRole::SuperAdmin, CloudAdminRole::SkillCurator],
    ) {
        return map_auth_err(err, request_id);
    }

    let exp_key = req.experiment_key.trim().to_string();
    let title = req.title.trim().to_string();
    let reason = req.reason.unwrap_or_default().trim().to_string();
    if !is_valid_ascii_identifier(&exp_key) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_EXPERIMENT_KEY",
            "experimentKey must match ^[a-zA-Z0-9._-]{2,80}$ (no ':' allowed)",
            Some("experimentKey"),
            request_id,
        );
    }
    if reason.len() < 5 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REASON_REQUIRED",
            "A clear audit reason (>= 5 chars) is mandatory for A/B experiment mutations",
            Some("reason"),
            request_id,
        );
    }
    if title.len() < 3 || title.len() > 160 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_TITLE",
            "title must be 3..=160 chars",
            Some("title"),
            request_id,
        );
    }
    if !VALID_TARGET_SURFACES.contains(&req.target_surface.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_TARGET_SURFACE",
            "targetSurface must be ONBOARDING_DEFAULTS, PLUGIN_RANKING, or UX_COPY_LAYOUT",
            Some("targetSurface"),
            request_id,
        );
    }
    if !VALID_EXPERIMENT_STATUSES.contains(&req.status.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_STATUS",
            "status must be DRAFT, RUNNING, PAUSED, KILLED, or COMPLETED",
            Some("status"),
            request_id,
        );
    }
    if req.variants.len() < 2 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_VARIANTS",
            "At least 2 variants are required for an A/B experiment",
            Some("variants"),
            request_id,
        );
    }
    let mut normalized_variants = Vec::with_capacity(req.variants.len());
    let mut seen_variant_keys = HashSet::with_capacity(req.variants.len());
    for variant in req.variants {
        let trimmed_key = variant.key.trim().to_string();
        if !is_valid_ascii_identifier(&trimmed_key) {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_VARIANT_KEY",
                "Each variant.key must match ^[a-zA-Z0-9._-]{2,80}$ (no ':' allowed)",
                Some("variants"),
                request_id,
            );
        }
        if !seen_variant_keys.insert(trimmed_key.clone()) {
            return json_err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "DUPLICATE_VARIANT_KEY",
                format!("Duplicate variant key '{trimmed_key}' is not allowed"),
                Some("variants"),
                request_id,
            );
        }
        normalized_variants.push(AbVariantWeight {
            key: trimmed_key,
            weight: variant.weight,
            config: variant.config,
        });
    }
    let total_weight: u32 = normalized_variants
        .iter()
        .map(|v| u32::from(v.weight))
        .sum();
    if total_weight != 100 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_VARIANT_WEIGHTS",
            format!("Variant weights must sum to 100 (got {total_weight})"),
            Some("variants"),
            request_id,
        );
    }

    let variants_val = serde_json::to_value(&normalized_variants).unwrap_or_else(|_| json!([]));
    if let Err(sec_err) = validate_experiment_security_guard(&exp_key, &title, &variants_val) {
        return json_err(
            StatusCode::BAD_REQUEST,
            sec_err.code(),
            sec_err.message(),
            Some("variants"),
            request_id,
        );
    }

    let status = if req.kill_switch {
        "KILLED".to_string()
    } else {
        req.status
    };
    let kill_switch = req.kill_switch || status == "KILLED";
    let target_surface = req.target_surface;
    let variants = normalized_variants;
    let pool = state.registry.pool().clone();

    let res = tokio::task::spawn_blocking(move || {
        let now_ms = Utc::now().timestamp_millis();
        let variants_json = serde_json::to_string(&variants).unwrap_or_else(|_| "[]".to_string());

        let (experiment, audit_id) = retry_on_busy(
            "dist_upsert_experiment",
            || -> Result<(AbExperimentDto, String), rusqlite::Error> {
                let mut conn = pool
                    .connection()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let _ = ensure_v32_tables_and_seed(&conn);
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let next_seq: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(manifest_seq), 0) + 1 FROM cloud_ab_experiments",
                    [],
                    |r| r.get(0),
                )?;
                let existing_created: Option<i64> = tx
                    .query_row(
                        "SELECT created_at FROM cloud_ab_experiments WHERE experiment_key = ?1",
                        params![exp_key],
                        |r| r.get(0),
                    )
                    .optional()?;
                let created_at = existing_created.unwrap_or(now_ms);
                tx.execute(
                    "INSERT INTO cloud_ab_experiments (
                        experiment_key, title, target_surface, status, variants_json,
                        kill_switch, manifest_seq, updated_by, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    ON CONFLICT(experiment_key) DO UPDATE SET
                        title = excluded.title,
                        target_surface = excluded.target_surface,
                        status = excluded.status,
                        variants_json = excluded.variants_json,
                        kill_switch = excluded.kill_switch,
                        manifest_seq = excluded.manifest_seq,
                        updated_by = excluded.updated_by,
                        updated_at = excluded.updated_at",
                    params![
                        exp_key,
                        title,
                        target_surface,
                        status,
                        variants_json,
                        if kill_switch { 1 } else { 0 },
                        next_seq,
                        claims.email,
                        created_at,
                        now_ms,
                    ],
                )?;
                let audit_id = insert_audit_in_tx(
                    &tx,
                    &claims,
                    if kill_switch {
                        "AB_EXPERIMENT_KILLED"
                    } else {
                        "AB_EXPERIMENT_UPSERTED"
                    },
                    &format!("experiment:{exp_key}"),
                    &json!({
                        "experimentKey": exp_key,
                        "status": status,
                        "killSwitch": kill_switch,
                        "manifestSeq": next_seq,
                        "reason": reason,
                    }),
                )?;
                tx.commit()?;
                Ok((
                    AbExperimentDto {
                        experiment_key: exp_key.clone(),
                        title: title.clone(),
                        target_surface: target_surface.clone(),
                        status: status.clone(),
                        variants: variants.clone(),
                        kill_switch,
                        manifest_seq: next_seq,
                        updated_by: claims.email.clone(),
                        created_at,
                        updated_at: now_ms,
                        attribution: Vec::new(),
                    },
                    audit_id,
                ))
            },
        )
        .map_err(|e| format!("upsert experiment failed: {e}"))?;

        let (_, signed) = build_experiments_envelope(&pool, false)?;
        Ok::<_, String>(json!({
            "experiment": experiment,
            "signedManifest": signed,
            "auditLogId": audit_id,
        }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Upsert experiment worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/admin/skills/quarantine-global`
pub async fn handle_list_quarantines(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &ALL_ADMIN_ROLES) {
        return map_auth_err(err, request_id);
    }
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let (advisories, signed) = build_advisories_envelope(&pool)?;
        Ok::<_, String>(json!({
            "advisories": advisories,
            "signedManifest": signed,
        }))
    })
    .await;
    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List advisories worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/skills/quarantine-global` (`SUPER_ADMIN`, `SKILL_CURATOR`)
pub async fn handle_issue_global_quarantine(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<IssueQuarantineRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(
        &claims,
        &[CloudAdminRole::SuperAdmin, CloudAdminRole::SkillCurator],
    ) {
        return map_auth_err(err, request_id);
    }

    let skill_id = req.skill_or_plugin_id.trim().to_string();
    let reason = req.reason.trim().to_string();
    if skill_id.is_empty() || skill_id.len() > 128 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_SKILL_ID",
            "skillOrPluginId is required (1..=128 chars)",
            Some("skillOrPluginId"),
            request_id,
        );
    }
    if reason.len() < 5 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "REASON_REQUIRED",
            "A clear audit reason (>= 5 chars) is mandatory for Global Quarantine actions",
            Some("reason"),
            request_id,
        );
    }
    if !VALID_SEVERITIES.contains(&req.severity.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_SEVERITY",
            "severity must be CRITICAL, HIGH, or MEDIUM",
            Some("severity"),
            request_id,
        );
    }
    if !VALID_ADVISORY_ACTIONS.contains(&req.action.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ACTION",
            "action must be QUARANTINE, WARN, or REVOKE_QUARANTINE",
            Some("action"),
            request_id,
        );
    }

    let severity = req.severity;
    let action = req.action;
    let affected_sha256 = req
        .affected_sha256
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let pool = state.registry.pool().clone();

    let res = tokio::task::spawn_blocking(move || {
        let now_ms = Utc::now().timestamp_millis();
        let adv_id = format!("adv_{}", Uuid::new_v4().simple());

        let (advisory, audit_id) = retry_on_busy(
            "dist_issue_quarantine",
            || -> Result<(SkillAdvisoryRecord, String), rusqlite::Error> {
                let mut conn = pool.connection().map_err(|_| rusqlite::Error::InvalidQuery)?;
                let _ = ensure_v32_tables_and_seed(&conn);
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let next_seq: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(advisory_seq), 0) + 1 FROM cloud_skill_advisories",
                    [],
                    |r| r.get(0),
                )?;
                tx.execute(
                    "INSERT INTO cloud_skill_advisories (
                        id, skill_or_plugin_id, severity, action, reason,
                        affected_sha256, advisory_seq, issued_by, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        adv_id,
                        skill_id,
                        severity,
                        action,
                        reason,
                        affected_sha256,
                        next_seq,
                        claims.email,
                        now_ms,
                    ],
                )?;

                if action == "QUARANTINE" {
                    let next_cat_seq: i64 = tx.query_row(
                        "SELECT COALESCE(MAX(manifest_seq), 0) + 1 FROM cloud_plugin_catalog_entries",
                        [],
                        |r| r.get(0),
                    )?;
                    tx.execute(
                        "UPDATE cloud_plugin_catalog_entries
                         SET lifecycle_status = 'QUARANTINED', manifest_seq = ?1, updated_by = ?2, updated_at = ?3
                         WHERE plugin_id = ?4",
                        params![next_cat_seq, claims.email, now_ms, skill_id],
                    )?;
                } else if action == "REVOKE_QUARANTINE" {
                    let next_cat_seq: i64 = tx.query_row(
                        "SELECT COALESCE(MAX(manifest_seq), 0) + 1 FROM cloud_plugin_catalog_entries",
                        [],
                        |r| r.get(0),
                    )?;
                    tx.execute(
                        "UPDATE cloud_plugin_catalog_entries
                         SET lifecycle_status = 'ACTIVE', manifest_seq = ?1, updated_by = ?2, updated_at = ?3
                         WHERE plugin_id = ?4 AND lifecycle_status = 'QUARANTINED'",
                        params![next_cat_seq, claims.email, now_ms, skill_id],
                    )?;
                }

                let audit_id = insert_audit_in_tx(
                    &tx,
                    &claims,
                    "GLOBAL_SKILL_QUARANTINE_ISSUED",
                    &format!("skill:{skill_id}"),
                    &json!({
                        "advisoryId": adv_id,
                        "skillOrPluginId": skill_id,
                        "severity": severity,
                        "action": action,
                        "reason": reason,
                        "advisorySeq": next_seq,
                    }),
                )?;
                tx.commit()?;
                Ok((
                    SkillAdvisoryRecord {
                        id: adv_id.clone(),
                        skill_or_plugin_id: skill_id.clone(),
                        severity: severity.clone(),
                        action: action.clone(),
                        reason: reason.clone(),
                        affected_sha256: affected_sha256.clone(),
                        advisory_seq: next_seq,
                        issued_by: claims.email.clone(),
                        created_at: now_ms,
                    },
                    audit_id,
                ))
            },
        )
        .map_err(|e| format!("issue quarantine advisory failed: {e}"))?;

        let (_, signed) = build_advisories_envelope(&pool)?;
        Ok::<_, String>(json!({
            "advisory": advisory,
            "signedManifest": signed,
            "auditLogId": audit_id,
        }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::CREATED, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Quarantine worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/admin/licenses/issue` (`SUPER_ADMIN` only — protects signed offline license tokens)
pub async fn handle_list_licenses(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &[CloudAdminRole::SuperAdmin]) {
        return map_auth_err(err, request_id);
    }
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
        ensure_v32_tables_and_seed(&conn)?;
        let (signing_key, key_id) =
            load_or_create_workspace_signing_key(pool.workspace_root()).map_err(|e| e.message())?;
        let vk = signing_key.verifying_key();
        let now_unix = Utc::now().timestamp();

        let mut stmt = conn
            .prepare(
                "SELECT license_id, tier, subject_org, cowok_org_id, max_seats,
                        issued_at, expires_at, signed_token, revoked, issued_by
                 FROM cloud_offline_licenses
                 ORDER BY issued_at DESC
                 LIMIT 200",
            )
            .map_err(|e| format!("prepare list licenses: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                let token: String = r.get(7)?;
                let revoked = r.get::<_, i64>(8)? != 0;
                let verified = !revoked && verify_offline_license(&token, &vk, now_unix).is_ok();
                Ok(OfflineLicenseRecordDto {
                    license_id: r.get(0)?,
                    tier: r.get(1)?,
                    subject_org: r.get(2)?,
                    cowok_org_id: r.get(3)?,
                    max_seats: r.get(4)?,
                    issued_at: r.get(5)?,
                    expires_at: r.get(6)?,
                    signed_token: token,
                    revoked,
                    issued_by: r.get(9)?,
                    offline_verified: verified,
                })
            })
            .map_err(|e| format!("query list licenses: {e}"))?;
        let mut licenses = Vec::new();
        for item in rows {
            licenses.push(item.map_err(|e| format!("license row: {e}"))?);
        }
        Ok::<_, String>(json!({
            "licenses": licenses,
            "keyId": key_id,
            "publicKeyBase64": verifying_key_base64(&vk),
        }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "List licenses worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/admin/licenses/issue` (`SUPER_ADMIN` only)
pub async fn handle_issue_license(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<IssueLicenseRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &[CloudAdminRole::SuperAdmin]) {
        return map_auth_err(err, request_id);
    }

    let tier = req.tier.trim().to_ascii_uppercase();
    if !VALID_LICENSE_TIERS.contains(&tier.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_LICENSE_TIER",
            "tier must be FREE, PRO, TEAM, or ENTERPRISE",
            Some("tier"),
            request_id,
        );
    }
    let subject_org = req.subject_org.trim().to_string();
    if subject_org.len() < 2 || subject_org.len() > 160 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_SUBJECT_ORG",
            "subjectOrg must be 2..=160 chars",
            Some("subjectOrg"),
            request_id,
        );
    }
    if req.max_seats < 1 || req.max_seats > 100_000 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_MAX_SEATS",
            "maxSeats must be between 1 and 100000",
            Some("maxSeats"),
            request_id,
        );
    }
    if !(1..=3650).contains(&req.valid_days) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_VALID_DAYS",
            "validDays must be between 1 and 3650",
            Some("validDays"),
            request_id,
        );
    }

    let cowok_org_id = req
        .cowok_org_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let max_seats = req.max_seats;
    let valid_days = req.valid_days;
    let pool = state.registry.pool().clone();

    let res = tokio::task::spawn_blocking(move || {
        let (signing_key, key_id) =
            load_or_create_workspace_signing_key(pool.workspace_root()).map_err(|e| e.message())?;
        let now_unix = Utc::now().timestamp();
        let expires_at = now_unix + valid_days * 86_400;
        let license_id = format!("lic_{}", Uuid::new_v4().simple());

        let payload = OfflineLicensePayload {
            license_id: license_id.clone(),
            tier: tier.clone(),
            subject_org: subject_org.clone(),
            cowok_org_id: cowok_org_id.clone(),
            max_seats,
            features: default_features_for_tier(&tier),
            issued_at: now_unix,
            expires_at,
        };
        let signed_token =
            sign_offline_license(&signing_key, &key_id, &payload).map_err(|e| e.message())?;
        let verified =
            verify_offline_license(&signed_token, &signing_key.verifying_key(), now_unix)
                .map_err(|e| e.message())?;

        let audit_id = retry_on_busy(
            "dist_issue_license",
            || -> Result<String, rusqlite::Error> {
                let mut conn = pool
                    .connection()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let _ = ensure_v32_tables_and_seed(&conn);
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "INSERT INTO cloud_offline_licenses (
                        license_id, tier, subject_org, cowok_org_id, max_seats,
                        issued_at, expires_at, signed_token, revoked, issued_by
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
                    params![
                        license_id,
                        tier,
                        subject_org,
                        cowok_org_id,
                        max_seats,
                        now_unix,
                        expires_at,
                        signed_token,
                        claims.email,
                    ],
                )?;
                let aid = insert_audit_in_tx(
                    &tx,
                    &claims,
                    "OFFLINE_LICENSE_ISSUED",
                    &format!("license:{license_id}"),
                    &json!({
                        "licenseId": license_id,
                        "tier": tier,
                        "subjectOrg": subject_org,
                        "cowokOrgId": cowok_org_id,
                        "maxSeats": max_seats,
                        "expiresAt": expires_at,
                    }),
                )?;
                tx.commit()?;
                Ok(aid)
            },
        )
        .map_err(|e| format!("persist offline license failed: {e}"))?;

        let record = OfflineLicenseRecordDto {
            license_id,
            tier,
            subject_org,
            cowok_org_id,
            max_seats,
            issued_at: now_unix,
            expires_at,
            signed_token,
            revoked: false,
            issued_by: claims.email,
            offline_verified: verified.offline_verified,
        };
        Ok::<_, String>(json!({
            "license": record,
            "verification": verified,
            "auditLogId": audit_id,
        }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::CREATED, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Issue license worker panicked",
            None,
            request_id,
        ),
    }
}

/// `GET /v1/admin/audit-logs` (and `/v1/admin/distribution/audit-logs`) — strictly `SUPER_ADMIN` only
pub async fn handle_list_distribution_audit_logs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Query(query): Query<AuditLogsQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let claims = match verify_admin_session_from_headers(&headers) {
        Ok(c) => c,
        Err(err) => return map_auth_err(err, request_id),
    };
    if let Err(err) = require_admin_role(&claims, &[CloudAdminRole::SuperAdmin]) {
        return map_auth_err(err, request_id);
    }
    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let conn = pool.connection().map_err(|e| format!("db conn: {e}"))?;
        ensure_v32_tables_and_seed(&conn)?;
        let limit = query.limit.unwrap_or(100).clamp(1, 200) as i64;
        let action_filter = query
            .action
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "all")
            .map(str::to_string);
        let role_filter = query
            .actor_role
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "all")
            .map(str::to_string);

        let mut sql = String::from(
            "SELECT id, actor_subject, actor_role, mfa_verified, action, target_resource, details_json, created_at
             FROM admin_audit_logs
             WHERE 1 = 1",
        );
        let mut bind_values: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(af) = action_filter {
            sql.push_str(" AND INSTR(action, ?) > 0");
            bind_values.push(rusqlite::types::Value::Text(af));
        }
        if let Some(rf) = role_filter {
            sql.push_str(" AND actor_role = ?");
            bind_values.push(rusqlite::types::Value::Text(rf));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        bind_values.push(rusqlite::types::Value::Integer(limit));

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("prepare audit logs: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(bind_values), |r| {
                let details_str: String = r.get(6)?;
                Ok(AuditLogItemDto {
                    id: r.get(0)?,
                    actor_subject: r.get(1)?,
                    actor_role: r.get(2)?,
                    mfa_verified: r.get::<_, i64>(3)? == 1,
                    action: r.get(4)?,
                    target_resource: r.get(5)?,
                    details: serde_json::from_str(&details_str).unwrap_or_else(|_| json!({})),
                    created_at: r.get(7)?,
                })
            })
            .map_err(|e| format!("query audit logs: {e}"))?;
        let mut logs = Vec::new();
        for item in rows {
            logs.push(item.map_err(|e| format!("audit log row: {e}"))?);
        }
        Ok::<_, String>(json!({ "logs": logs }))
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err(msg)) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE_ERROR",
            msg,
            None,
            request_id,
        ),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Audit logs worker panicked",
            None,
            request_id,
        ),
    }
}

// ---------- Client Edge & Cowok.ai Federation ----------

/// `POST /v1/client/edge/sync`
pub async fn handle_client_edge_sync(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<ClientEdgeSyncRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let install_id = req.install_id.trim().to_string();
    if install_id.is_empty() || install_id.len() > 128 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_INSTALL_ID",
            "installId is required (1..=128 chars)",
            Some("installId"),
            request_id,
        );
    }

    let channel = req.channel.trim().to_ascii_lowercase();
    if !VALID_CHANNELS.contains(&channel.as_str()) {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_CHANNEL",
            "channel must be one of cli, desktop, plugin, docker",
            Some("channel"),
            request_id,
        );
    }

    let pool = state.registry.pool().clone();
    let res = tokio::task::spawn_blocking(move || {
        let now_unix = Utc::now().timestamp();
        let (signing_key, _) = load_or_create_workspace_signing_key(pool.workspace_root())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.code(), e.message()))?;
        let trusted_vk = signing_key.verifying_key();

        let latest_env = match req.latest_release_manifest {
            Some(env) => env,
            None => build_latest_release_envelope(&pool, &channel, Some(&install_id))
                .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?,
        };
        if let Err(err) = verify_manifest_fail_closed(
            &latest_env,
            "latest",
            &trusted_vk,
            req.last_seen_release_seq,
            now_unix,
        ) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, err.code(), err.message()));
        }

        let catalog_env = match req.catalog_manifest {
            Some(env) => env,
            None => {
                build_catalog_envelope(&pool)
                    .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?
                    .1
            }
        };
        if let Err(err) = verify_manifest_fail_closed(
            &catalog_env,
            "catalog",
            &trusted_vk,
            req.last_seen_catalog_seq,
            now_unix,
        ) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, err.code(), err.message()));
        }

        let advisories_env = match req.advisories_manifest {
            Some(env) => env,
            None => {
                build_advisories_envelope(&pool)
                    .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?
                    .1
            }
        };
        if let Err(err) = verify_manifest_fail_closed(
            &advisories_env,
            "advisories",
            &trusted_vk,
            req.last_seen_advisories_seq,
            now_unix,
        ) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, err.code(), err.message()));
        }

        let experiments_env = match req.experiments_manifest {
            Some(env) => env,
            None => {
                build_experiments_envelope(&pool, false)
                    .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?
                    .1
            }
        };
        if let Err(err) = verify_manifest_fail_closed(
            &experiments_env,
            "experiments",
            &trusted_vk,
            req.last_seen_experiments_seq,
            now_unix,
        ) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, err.code(), err.message()));
        }

        let effective_plugins = resolve_effective_plugin_states(
            &catalog_env.payload.plugins,
            &advisories_env.payload.advisories,
            &req.local_overrides,
        );

        let mut experiment_assignments = Vec::new();
        let mut experiment_variants = Vec::new();
        for exp in &experiments_env.payload.experiments {
            if exp.status != "RUNNING" || exp.kill_switch {
                continue;
            }
            let variants_val = serde_json::to_value(&exp.variants).unwrap_or_else(|_| json!([]));
            if validate_experiment_security_guard(&exp.experiment_key, &exp.title, &variants_val)
                .is_err()
            {
                continue;
            }
            if let Some((chosen, bucket)) =
                select_ab_variant(&install_id, &exp.experiment_key, &exp.variants)
            {
                let tag = format!("{}:{}", exp.experiment_key, chosen.key);
                experiment_variants.push(tag.clone());
                experiment_assignments.push(ClientExperimentAssignmentDto {
                    experiment_key: exp.experiment_key.clone(),
                    variant_key: chosen.key.clone(),
                    bucket,
                    telemetry_tag: tag,
                    config: chosen.config.clone(),
                });
            }
        }

        let license_verification = if let Some(ref tok) = req.offline_license_token {
            let verified = match verify_offline_license(tok, &trusted_vk, now_unix) {
                Ok(v) => v,
                Err(err) => {
                    return Err((StatusCode::UNPROCESSABLE_ENTITY, err.code(), err.message()));
                }
            };
            let conn = pool.connection().map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "STORAGE_ERROR",
                    format!("db conn: {e}"),
                )
            })?;
            ensure_v32_tables_and_seed(&conn)
                .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?;
            check_license_not_revoked(&conn, &verified.payload.license_id)?;
            Some(verified)
        } else {
            None
        };

        Ok(ClientEdgeSyncResponseData {
            install_id,
            verified_manifest_seqs: json!({
                "latest": latest_env.manifest_seq,
                "catalog": catalog_env.manifest_seq,
                "advisories": advisories_env.manifest_seq,
                "experiments": experiments_env.manifest_seq,
                "channel": channel,
                "appVersion": req.app_version,
            }),
            effective_plugins,
            experiment_assignments,
            experiment_variants,
            license_verification,
        })
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err((status, code, msg))) => json_err(status, code, msg, None, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Client edge sync worker panicked",
            None,
            request_id,
        ),
    }
}

/// `POST /v1/federation/cowok-org`
pub async fn handle_cowok_org_federation(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    Json(req): Json<CowokOrgFederationRequest>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let cowok_org_id = req.cowok_org_id.trim().to_string();
    let install_id = req.install_id.trim().to_string();
    if cowok_org_id.len() < 2 || cowok_org_id.len() > 128 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_COWOK_ORG_ID",
            "cowokOrgId is required (2..=128 chars)",
            Some("cowokOrgId"),
            request_id,
        );
    }
    if install_id.is_empty() || install_id.len() > 128 {
        return json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_INSTALL_ID",
            "installId is required (1..=128 chars)",
            Some("installId"),
            request_id,
        );
    }

    let pool = state.registry.pool().clone();
    let token_str = match req
        .license_token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(tok) => tok.to_string(),
        None => {
            return json_err(
                StatusCode::UNAUTHORIZED,
                "LICENSE_TOKEN_REQUIRED",
                "Cryptographic Ed25519 licenseToken bound to cowokOrgId is required",
                Some("licenseToken"),
                request_id,
            );
        }
    };
    let res = tokio::task::spawn_blocking(move || {
        let now_unix = Utc::now().timestamp();
        let (signing_key, _) = load_or_create_workspace_signing_key(pool.workspace_root())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.code(), e.message()))?;
        let trusted_vk = signing_key.verifying_key();

        let verified = verify_offline_license(&token_str, &trusted_vk, now_unix)
            .map_err(|e| (StatusCode::UNAUTHORIZED, e.code(), e.message()))?;
        if verified.payload.cowok_org_id.as_deref() != Some(cowok_org_id.as_str()) {
            return Err((
                StatusCode::FORBIDDEN,
                "COWOK_ORG_MISMATCH",
                "Offline license token is not bound to the requested cowokOrgId".to_string(),
            ));
        }

        let conn = pool.connection().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "STORAGE_ERROR",
                format!("db conn: {e}"),
            )
        })?;
        ensure_v32_tables_and_seed(&conn)
            .map_err(|m| (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE_ERROR", m))?;
        check_license_not_revoked(&conn, &verified.payload.license_id)?;

        Ok(CowokOrgFederationResponseData {
            federated: true,
            cowok_org_id,
            install_id,
            license_id: verified.payload.license_id,
            tier: verified.payload.tier,
            subject_org: verified.payload.subject_org,
            max_seats: verified.payload.max_seats,
            expires_at: verified.payload.expires_at,
            offline_verified: true,
        })
    })
    .await;

    match res {
        Ok(Ok(data)) => json_ok(StatusCode::OK, data, request_id),
        Ok(Err((status, code, msg))) => json_err(status, code, msg, None, request_id),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Cowok.ai federation worker panicked",
            None,
            request_id,
        ),
    }
}

/// Core Distribution Ops routes without `/v1/admin/audit-logs` (for collision-free mounting alongside `cloud_admin_router` if `cloud_admin_router` already owns `/v1/admin/audit-logs`).
pub fn distribution_ops_mount_router() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/v1/releases/latest.json",
            get(handle_get_latest_release_manifest),
        )
        .route(
            "/v1/skills/advisories.json",
            get(handle_get_advisories_manifest),
        )
        .route("/v1/skills/catalog.json", get(handle_get_catalog_manifest))
        .route(
            "/v1/experiments/config.json",
            get(handle_get_experiments_manifest),
        )
        .route(
            "/v1/admin/releases/rollout",
            get(handle_list_rollouts).post(handle_publish_rollout),
        )
        .route(
            "/v1/admin/skills/catalog",
            get(handle_list_admin_catalog).post(handle_upsert_admin_catalog),
        )
        .route(
            "/v1/admin/skills/catalog/bulk",
            post(handle_bulk_update_admin_catalog),
        )
        .route(
            "/v1/admin/experiments",
            get(handle_list_admin_experiments).post(handle_upsert_admin_experiment),
        )
        .route(
            "/v1/admin/skills/quarantine-global",
            get(handle_list_quarantines).post(handle_issue_global_quarantine),
        )
        .route(
            "/v1/admin/licenses/issue",
            get(handle_list_licenses).post(handle_issue_license),
        )
        .route(
            "/v1/admin/distribution/audit-logs",
            get(handle_list_distribution_audit_logs),
        )
        .route("/v1/client/edge/sync", post(handle_client_edge_sync))
        .route(
            "/v1/federation/cowok-org",
            post(handle_cowok_org_federation),
        )
}

/// Complete Story 14.3 Distribution Ops router including `/v1/admin/audit-logs`.
pub fn distribution_ops_router() -> Router<DaemonAppState> {
    distribution_ops_mount_router().route(
        "/v1/admin/audit-logs",
        get(handle_list_distribution_audit_logs),
    )
}
