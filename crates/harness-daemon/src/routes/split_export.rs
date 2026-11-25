// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
use std::collections::BTreeMap;
use std::path::Path;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use domain::HarnessError;
use html_forge::export_bundler::{
    build_developer_handoff, build_vector_pdf, bundle_offline_html, DeveloperHandoffFile,
    DeveloperHandoffInput, ExportBundlerError, OfflineHtmlInput, VectorPdfInput, VectorPdfSlide,
    MAX_BUNDLE_BYTES,
};
use html_forge::pptx_ir::{PptxExportMode, PptxFidelityWarning};
use html_forge::pptx_layout::PptxExportInput;
use html_forge::pptx_ooxml::build_pptx_deck;
use rusqlite::{params, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::retry_on_busy;
use uuid::Uuid;

use super::legacy_specs::verify_anti_idor_with_caller;
use super::visual_qc::shared_cdp_evaluator;
use super::DaemonAppState;
use crate::daemon_auth::{is_valid_superadmin_bearer, LegacyAuthKind, VerifiedLegacyIngress};

/// Slightly above `MAX_BUNDLE_BYTES` so JSON payloads slightly exceeding 25 MiB reach
/// the bundler's typed `413 ASSET_SIZE_LIMIT_EXCEEDED` guard cleanly.
const MAX_EXPORT_HTTP_BODY_BYTES: usize = MAX_BUNDLE_BYTES + (2 * 1024 * 1024);
/// Metadata-only legacy PPTX probe limit (Story 07.2 compatibility).
const MAX_PPTX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const PPTX_PLUGIN_ID: &str = "pptx-deck-exporter-plugin";
const PPTX_INSTALL_ACTION: &str = "/plugins?install=pptx-deck-exporter-plugin";
const BUILTIN_PPTX_PLUGIN_ID: &str = "builtin-native-rust-pptx-exporter";
const BUILTIN_PPTX_INSTALL_ACTION: &str = "/plugins";

pub const V30_FALLBACK_DDL: &str =
    include_str!("../../../spec-registry/migrations/V30__story_07_2_split_export_audit.sql");

pub fn ensure_v30_tables(conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(V30_FALLBACK_DDL)?;
    // Upgrade legacy V30 databases created before `tenant_id` column was added
    let mut has_tenant_id = false;
    {
        let mut stmt = tx.prepare("PRAGMA table_info(export_bundle_runs)")?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in cols.flatten() {
            if col.eq_ignore_ascii_case("tenant_id") {
                has_tenant_id = true;
                break;
            }
        }
    }
    if !has_tenant_id {
        tx.execute(
            "ALTER TABLE export_bundle_runs ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default'",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_export_bundle_runs_tenant_project_created ON export_bundle_runs (tenant_id, project_id, created_at DESC)",
            [],
        )?;
    }
    tx.commit()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SplitExportErrorEnvelope {
    error: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    p0_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p1_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    qc_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actual_bytes: Option<usize>,
    request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcGateSummary {
    pub status: String,
    pub p0_count: usize,
    pub p1_count: usize,
    pub latest_run_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlBundleExportRequest {
    pub project_id: String,
    #[serde(default = "default_export_title")]
    pub title: String,
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub inline_css: Option<String>,
    #[serde(default)]
    pub cas_assets: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    pub enable_presenter: bool,
    #[serde(default)]
    pub override_qc_warning: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlBundleExportData {
    pub export_id: String,
    pub project_id: String,
    pub audience_mode: String,
    pub format: String,
    pub file_name: String,
    pub sha256: String,
    pub byte_size: usize,
    pub external_cdn_refs_count: usize,
    pub stripped_external_refs_count: usize,
    pub presenter_enabled: bool,
    pub slide_count: usize,
    pub svg_count: usize,
    pub broken_svg_count: usize,
    pub inline_assets_count: usize,
    pub broken_cas_assets_count: usize,
    pub csp_header: String,
    pub qc_gate_status: String,
    pub qc_override_acknowledged: bool,
    pub warnings: Vec<String>,
    pub html_content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPdfExportRequest {
    pub project_id: String,
    #[serde(default = "default_export_title")]
    pub title: String,
    #[serde(default)]
    pub slides: Vec<VectorPdfSlide>,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub override_qc_warning: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPdfExportData {
    pub export_id: String,
    pub project_id: String,
    pub audience_mode: String,
    pub format: String,
    pub file_name: String,
    pub pdf_base64: String,
    pub sha256: String,
    pub byte_size: usize,
    pub page_count: usize,
    pub width_pt: f64,
    pub height_pt: f64,
    pub engine: String,
    pub qc_gate_status: String,
    pub qc_override_acknowledged: bool,
    pub warnings: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxExportRequest {
    pub project_id: String,
    #[serde(default = "default_export_title")]
    pub title: String,
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub inline_css: Option<String>,
    #[serde(default)]
    pub cas_assets: BTreeMap<String, String>,
    #[serde(default)]
    pub export_mode: Option<PptxExportMode>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub slide_count: Option<usize>,
    #[serde(default)]
    pub override_qc_warning: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxExportResponse {
    pub ok: bool,
    pub plugin_installed: bool,
    pub plugin_id: String,
    pub install_action: String,
    pub message: String,
    pub data: PptxExportData,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxExportData {
    pub export_id: String,
    pub project_id: String,
    pub audience_mode: String,
    pub format: String,
    pub plugin_installed: bool,
    pub plugin_id: String,
    pub plugin_status: String,
    pub install_action: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_mode: Option<PptxExportMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pptx_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub byte_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_static_fallback: Option<bool>,
    #[serde(default)]
    pub fidelity_warnings: Vec<PptxFidelityWarning>,
    pub qc_gate_status: String,
    pub qc_override_acknowledged: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperHandoffRequest {
    pub project_id: String,
    #[serde(default = "default_export_title")]
    pub project_name: String,
    #[serde(default)]
    pub spec_markdown: Option<String>,
    #[serde(default)]
    pub design_tokens_css: Option<String>,
    #[serde(default)]
    pub design_tokens_json: Option<String>,
    #[serde(default)]
    pub screen_tsx: Option<String>,
    #[serde(default)]
    pub html_source: Option<String>,
    #[serde(default)]
    pub e2e_spec_ts: Option<String>,
    #[serde(default)]
    pub override_qc_warning: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperHandoffData {
    pub export_id: String,
    pub project_id: String,
    pub audience_mode: String,
    pub format: String,
    pub file_name: String,
    pub sha256: String,
    pub byte_size: usize,
    pub files: Vec<DeveloperHandoffFile>,
    pub qc_gate_status: String,
    pub qc_override_acknowledged: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryQuery {
    pub project_id: String,
    #[serde(default = "default_history_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryItem {
    pub id: String,
    pub project_id: String,
    pub audience_mode: String,
    pub format: String,
    pub file_name: String,
    pub artifact_sha256: String,
    pub byte_size: usize,
    pub external_cdn_refs_count: usize,
    pub qc_gate_status: String,
    pub qc_override_acknowledged: bool,
    pub plugin_status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryData {
    pub project_id: String,
    pub qc_gate: QcGateSummary,
    pub pptx_plugin_installed: bool,
    pub items: Vec<ExportHistoryItem>,
}

fn default_export_title() -> String {
    "Presentation".to_string()
}

fn default_true() -> bool {
    true
}

fn default_history_limit() -> usize {
    20
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

fn err_resp(status: StatusCode, code: &str, message: &str, request_id: String) -> Response {
    (
        status,
        Json(SplitExportErrorEnvelope {
            error: code.to_string(),
            message: message.to_string(),
            p0_count: None,
            p1_count: None,
            qc_run_id: None,
            limit_bytes: None,
            actual_bytes: None,
            request_id,
        }),
    )
        .into_response()
}

fn ok_resp<T: Serialize>(data: T, request_id: String) -> Response {
    (
        StatusCode::OK,
        Json(ApiSuccessEnvelope {
            ok: true,
            data,
            request_id,
        }),
    )
        .into_response()
}

fn require_caller<'a>(
    caller: Option<&'a VerifiedLegacyIngress>,
    headers: &HeaderMap,
    request_id: &str,
) -> Result<&'a VerifiedLegacyIngress, Response> {
    let verified = caller.ok_or_else(|| {
        err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Missing verified authentication credentials",
            request_id.to_string(),
        )
    })?;
    if verified.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
        return Err(err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Untrusted Bearer token rejected",
            request_id.to_string(),
        ));
    }
    Ok(verified)
}

fn require_export_role(verified: &VerifiedLegacyIngress, request_id: &str) -> Result<(), Response> {
    let allowed = matches!(
        verified.principal.role.trim().to_ascii_lowercase().as_str(),
        "editor" | "admin" | "owner"
    );
    if !allowed {
        return Err(err_resp(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Export action requires editor or admin role",
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn resolve_tenant_for_project(
    state: &DaemonAppState,
    project_id: &str,
    headers: &HeaderMap,
    verified: &VerifiedLegacyIngress,
    request_id: &str,
) -> Result<String, Response> {
    let trimmed = project_id.trim();
    let project_uuid = Uuid::parse_str(trimmed).map_err(|_| {
        err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "projectId must be a valid UUID",
            request_id.to_string(),
        )
    })?;
    match verify_anti_idor_with_caller(&state.registry, project_uuid, headers, Some(verified)) {
        Ok(_) => {
            let conn = state.registry.pool().connection().map_err(|_| {
                err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Storage connection unavailable",
                    request_id.to_string(),
                )
            })?;
            let tenant_id: String = conn
                .query_row(
                    "SELECT tenant_id FROM projects WHERE id = ?1 AND trash_deleted_at IS NULL",
                    params![trimmed],
                    |r| r.get(0),
                )
                .map_err(|_| {
                    err_resp(
                        StatusCode::NOT_FOUND,
                        "PROJECT_NOT_FOUND",
                        "Project not found",
                        request_id.to_string(),
                    )
                })?;
            if matches!(
                verified.kind,
                LegacyAuthKind::HmacV2 | LegacyAuthKind::SessionToken
            ) && verified.principal.tenant_id.trim() != tenant_id.trim()
            {
                return Err(err_resp(
                    StatusCode::FORBIDDEN,
                    "PROJECT_TENANT_MISMATCH",
                    "Cross-tenant project access forbidden",
                    request_id.to_string(),
                ));
            }
            Ok(tenant_id)
        }
        Err(HarnessError::ProjectNotFound { .. }) => Err(err_resp(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found",
            request_id.to_string(),
        )),
        Err(_) => Err(err_resp(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "Project access denied",
            request_id.to_string(),
        )),
    }
}

fn is_no_such_table_err(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(_, Some(msg)) => {
            msg.to_ascii_lowercase().contains("no such table")
        }
        _ => err
            .to_string()
            .to_ascii_lowercase()
            .contains("no such table"),
    }
}

/// Aggregates the latest `qc_gate_runs` record per `(target_kind, target_id)` so a passing scan
/// on one screen/slide cannot mask unresolved P0/P1 findings on another target.
/// Treats ONLY empty result sets or missing `qc_gate_runs` table as `UNSCANNED`; propagates
/// any other SQLite error fail-closed so callers return `503 STORAGE_BUSY`.
fn inspect_latest_qc_gate(
    conn: &rusqlite::Connection,
    project_id: &str,
) -> Result<QcGateSummary, rusqlite::Error> {
    let row_result = conn.query_row(
        "WITH latest_per_target AS (
             SELECT id, verdict, p0_count, p1_count, created_at, rowid,
                    ROW_NUMBER() OVER (
                        PARTITION BY target_kind, target_id
                        ORDER BY created_at DESC, rowid DESC
                    ) AS rn
             FROM qc_gate_runs
             WHERE project_id = ?1
         )
         SELECT
             COUNT(*) AS target_count,
             COALESCE(SUM(CASE WHEN p0_count > 0 THEN p0_count ELSE 0 END), 0) AS total_p0,
             COALESCE(SUM(CASE WHEN p1_count > 0 THEN p1_count ELSE 0 END), 0) AS total_p1,
             COALESCE(SUM(CASE WHEN verdict = 'FAIL' THEN 1 ELSE 0 END), 0) AS fail_count,
             COALESCE(SUM(CASE WHEN verdict = 'UNVERIFIED_NO_BROWSER' THEN 1 ELSE 0 END), 0) AS unverified_count,
             (SELECT id FROM latest_per_target WHERE rn = 1 ORDER BY created_at DESC, rowid DESC LIMIT 1) AS latest_run_id
         FROM latest_per_target
         WHERE rn = 1",
        params![project_id.trim()],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        },
    );

    match row_result {
        Ok((target_count, total_p0, total_p1, fail_count, unverified_count, latest_run_id)) => {
            if target_count <= 0 {
                return Ok(QcGateSummary {
                    status: "UNSCANNED".to_string(),
                    p0_count: 0,
                    p1_count: 0,
                    latest_run_id: None,
                });
            }
            let p0 = total_p0.max(0) as usize;
            let p1 = total_p1.max(0) as usize;
            let status = if p0 > 0 || fail_count > 0 {
                "FAIL"
            } else if unverified_count > 0 {
                "UNVERIFIED_NO_BROWSER"
            } else {
                "PASS"
            };
            Ok(QcGateSummary {
                status: status.to_string(),
                p0_count: p0,
                p1_count: p1,
                latest_run_id,
            })
        }
        Err(err) if is_no_such_table_err(&err) => Ok(QcGateSummary {
            status: "UNSCANNED".to_string(),
            p0_count: 0,
            p1_count: 0,
            latest_run_id: None,
        }),
        Err(err) => Err(err),
    }
}

/// Enforces the Pre-Export QC Gate: if any target's latest QC run has `p0_count > 0` and
/// `override_qc_warning` is false, returns `409 Conflict` with `{ error: "QC_GATE_P0_BLOCKED", p0Count, message }`.
/// Returns `503 STORAGE_BUSY` fail-closed on unexpected SQLite failures.
fn enforce_pre_export_qc_gate(
    conn: &rusqlite::Connection,
    project_id: &str,
    override_qc_warning: bool,
    request_id: &str,
) -> Result<(String, bool), Response> {
    let summary = inspect_latest_qc_gate(conn, project_id).map_err(|_| {
        err_resp(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_BUSY",
            "Unable to inspect QC gate status; storage temporarily unavailable",
            request_id.to_string(),
        )
    })?;
    if summary.p0_count > 0 && !override_qc_warning {
        return Err((
            StatusCode::CONFLICT,
            Json(SplitExportErrorEnvelope {
                error: "QC_GATE_P0_BLOCKED".to_string(),
                message: format!(
                    "Export paused because {} critical layout issue(s) must be reviewed first. Fix them or acknowledge the override to continue.",
                    summary.p0_count
                ),
                p0_count: Some(summary.p0_count),
                p1_count: Some(summary.p1_count),
                qc_run_id: summary.latest_run_id,
                limit_bytes: None,
                actual_bytes: None,
                request_id: request_id.to_string(),
            }),
        )
            .into_response());
    }
    let acknowledged = summary.p0_count > 0 && override_qc_warning;
    let status_label = if acknowledged {
        "FAIL_OVERRIDDEN".to_string()
    } else {
        summary.status
    };
    Ok((status_label, acknowledged))
}

/// Verifies a local workspace plugin directory exists without following symlinks and remains
/// strictly inside the canonical workspace root.
fn is_safe_local_plugin_dir(root: &Path, relative_segments: &[&str]) -> bool {
    let Ok(canon_root) = root.canonicalize() else {
        return false;
    };
    let mut current = root.to_path_buf();
    for seg in relative_segments {
        current = current.join(seg);
        let Ok(meta) = std::fs::symlink_metadata(&current) else {
            return false;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return false;
        }
    }
    let Ok(canon_dir) = current.canonicalize() else {
        return false;
    };
    if !canon_dir.starts_with(&canon_root) {
        return false;
    }
    for marker in ["manifest.json", "SKILL.md"] {
        let candidate = current.join(marker);
        if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
            if !meta.file_type().is_symlink() && meta.is_file() {
                if let Ok(canon_file) = candidate.canonicalize() {
                    if canon_file.starts_with(&canon_dir) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Checks whether the optional PPTX export plugin is installed and approved (`PASS`) for
/// `(tenant_id, project_id)`. A non-`PASS` database verdict (`QUARANTINED` / `REJECT`)
/// is authoritative and NEVER overridden by any local filesystem fallback.
fn is_pptx_plugin_installed(
    state: &DaemonAppState,
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
) -> bool {
    let verdicts_result: Result<Vec<String>, rusqlite::Error> = conn
        .prepare(
            "SELECT security_scan_verdict
             FROM user_skill_registry
             WHERE tenant_id = ?1
               AND (project_id = ?2 OR project_id IS NULL)
               AND skill_slug = ?3
             ORDER BY CASE WHEN project_id IS NOT NULL THEN 0 ELSE 1 END, created_at DESC",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map(
                params![tenant_id.trim(), project_id.trim(), PPTX_PLUGIN_ID],
                |r| r.get::<_, String>(0),
            )?;
            rows.collect()
        });

    match verdicts_result {
        Ok(verdicts) if !verdicts.is_empty() => {
            // Never override a non-PASS DB verdict (e.g. QUARANTINED or REJECT)
            if verdicts.iter().any(|v| v != "PASS") {
                return false;
            }
            return true;
        }
        Ok(_) => {
            // No matching DB record for this tenant+project; check safe non-symlink local plugin fallback
        }
        Err(err) if is_no_such_table_err(&err) => {
            // Registry table not provisioned yet; check safe non-symlink local plugin fallback
        }
        Err(_) => {
            // Fail-closed on unexpected DB errors
            return false;
        }
    }

    let root = state.registry.pool().workspace_root();
    is_safe_local_plugin_dir(root, &["plugins", PPTX_PLUGIN_ID])
        || is_safe_local_plugin_dir(root, &[".cowok-design", "plugins", PPTX_PLUGIN_ID])
}

struct ExportAuditRecord<'a> {
    id: &'a str,
    tenant_id: &'a str,
    project_id: &'a str,
    audience_mode: &'a str,
    format: &'a str,
    file_name: &'a str,
    artifact_sha256: &'a str,
    byte_size: usize,
    external_cdn_refs_count: usize,
    qc_gate_status: &'a str,
    qc_override_acknowledged: bool,
    plugin_status: &'a str,
    created_at: i64,
}

fn persist_export_audit_run(
    conn: &mut rusqlite::Connection,
    rec: &ExportAuditRecord<'_>,
) -> Result<(), HarnessError> {
    ensure_v30_tables(conn).map_err(|e| HarnessError::ValidationFailed {
        reason: format!("ensure V30 export_bundle_runs failed: {e}"),
    })?;
    retry_on_busy("insert_export_bundle_run_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO export_bundle_runs (
                id, tenant_id, project_id, audience_mode, format, file_name, artifact_sha256,
                byte_size, external_cdn_refs_count, qc_gate_status,
                qc_override_acknowledged, plugin_status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                rec.id,
                rec.tenant_id.trim(),
                rec.project_id.trim(),
                rec.audience_mode,
                rec.format,
                rec.file_name,
                rec.artifact_sha256,
                rec.byte_size as i64,
                rec.external_cdn_refs_count as i64,
                rec.qc_gate_status,
                if rec.qc_override_acknowledged {
                    1i64
                } else {
                    0i64
                },
                rec.plugin_status,
                rec.created_at,
            ],
        )?;
        tx.commit()?;
        Ok(())
    })
}

fn map_bundler_err(err: ExportBundlerError, request_id: String) -> Response {
    match err {
        ExportBundlerError::AssetSizeExceeded {
            actual_bytes,
            limit_bytes,
        } => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(SplitExportErrorEnvelope {
                error: "ASSET_SIZE_LIMIT_EXCEEDED".to_string(),
                message: format!(
                    "Export package exceeds the maximum allowed size ({limit_bytes} bytes)"
                ),
                p0_count: None,
                p1_count: None,
                qc_run_id: None,
                limit_bytes: Some(limit_bytes),
                actual_bytes: Some(actual_bytes),
                request_id,
            }),
        )
            .into_response(),
        ExportBundlerError::ExternalCdnLeak { count } => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "EXTERNAL_CDN_LEAK_BLOCKED",
            &format!(
                "Offline export blocked: {count} external network reference(s) remained after sanitization"
            ),
            request_id,
        ),
        ExportBundlerError::InvalidInput(msg) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &msg,
            request_id,
        ),
        ExportBundlerError::HtmlRewrite(msg) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "EXPORT_REWRITE_FAILED",
            &msg,
            request_id,
        ),
    }
}

async fn run_blocking_export<F>(request_id: String, task: F) -> Response
where
    F: FnOnce() -> Response + Send + 'static,
{
    match tokio::task::spawn_blocking(task).await {
        Ok(resp) => resp,
        Err(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "EXPORT_WORKER_FAILED",
            "Export worker thread failed unexpectedly",
            request_id,
        ),
    }
}

async fn handle_post_html_bundle(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_export_role(&verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_EXPORT_HTTP_BODY_BYTES {
        return map_bundler_err(
            ExportBundlerError::AssetSizeExceeded {
                actual_bytes: body.len(),
                limit_bytes: MAX_BUNDLE_BYTES,
            },
            request_id,
        );
    }
    let req: HtmlBundleExportRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(err) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid HTML bundle export payload: {err}"),
                request_id,
            );
        }
    };

    let worker_req_id = request_id.clone();
    run_blocking_export(request_id, move || {
        let tenant_id = match resolve_tenant_for_project(
            &state,
            &req.project_id,
            &headers,
            &verified,
            &worker_req_id,
        ) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

        let mut conn = match state.registry.pool().connection() {
            Ok(c) => c,
            Err(_) => {
                return err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Storage connection unavailable",
                    worker_req_id,
                );
            }
        };

        let (qc_gate_status, qc_override_acknowledged) = match enforce_pre_export_qc_gate(
            &conn,
            &req.project_id,
            req.override_qc_warning,
            &worker_req_id,
        ) {
            Ok(v) => v,
            Err(resp) => return resp,
        };

        let bundled = match bundle_offline_html(&OfflineHtmlInput {
            title: req.title.clone(),
            html: req.html.clone(),
            inline_css: req.inline_css.clone(),
            cas_assets: req.cas_assets.clone(),
            enable_presenter: req.enable_presenter,
            max_bytes: Some(MAX_BUNDLE_BYTES),
        }) {
            Ok(b) => b,
            Err(e) => return map_bundler_err(e, worker_req_id),
        };

        let export_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp_millis();

        if let Err(err) = persist_export_audit_run(
            &mut conn,
            &ExportAuditRecord {
                id: &export_id,
                tenant_id: &tenant_id,
                project_id: &req.project_id,
                audience_mode: "SHARE_PRESENT",
                format: "OFFLINE_HTML",
                file_name: &bundled.file_name,
                artifact_sha256: &bundled.sha256,
                byte_size: bundled.byte_size,
                external_cdn_refs_count: bundled.external_cdn_refs_count,
                qc_gate_status: &qc_gate_status,
                qc_override_acknowledged,
                plugin_status: "NOT_APPLICABLE",
                created_at,
            },
        ) {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "EXPORT_AUDIT_PERSIST_FAILED",
                &format!("Failed to persist export receipt: {err}"),
                worker_req_id,
            );
        }

        ok_resp(
            HtmlBundleExportData {
                export_id,
                project_id: req.project_id,
                audience_mode: "SHARE_PRESENT".to_string(),
                format: "OFFLINE_HTML".to_string(),
                file_name: bundled.file_name,
                sha256: bundled.sha256,
                byte_size: bundled.byte_size,
                external_cdn_refs_count: bundled.external_cdn_refs_count,
                stripped_external_refs_count: bundled.stripped_external_refs_count,
                presenter_enabled: bundled.presenter_enabled,
                slide_count: bundled.slide_count,
                svg_count: bundled.svg_count,
                broken_svg_count: bundled.broken_svg_count,
                inline_assets_count: bundled.inline_assets_count,
                broken_cas_assets_count: bundled.broken_cas_assets_count,
                csp_header: bundled.csp_header,
                qc_gate_status,
                qc_override_acknowledged,
                warnings: bundled.warnings,
                html_content: bundled.html_content,
                created_at,
            },
            worker_req_id,
        )
    })
    .await
}

async fn handle_post_vector_pdf(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_export_role(&verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_EXPORT_HTTP_BODY_BYTES {
        return map_bundler_err(
            ExportBundlerError::AssetSizeExceeded {
                actual_bytes: body.len(),
                limit_bytes: MAX_BUNDLE_BYTES,
            },
            request_id,
        );
    }
    let req: VectorPdfExportRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(err) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid Vector PDF export payload: {err}"),
                request_id,
            );
        }
    };

    let worker_req_id = request_id.clone();
    run_blocking_export(request_id, move || {
        let tenant_id = match resolve_tenant_for_project(
            &state,
            &req.project_id,
            &headers,
            &verified,
            &worker_req_id,
        ) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

        let mut conn = match state.registry.pool().connection() {
            Ok(c) => c,
            Err(_) => {
                return err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Storage connection unavailable",
                    worker_req_id,
                );
            }
        };

        let (qc_gate_status, qc_override_acknowledged) = match enforce_pre_export_qc_gate(
            &conn,
            &req.project_id,
            req.override_qc_warning,
            &worker_req_id,
        ) {
            Ok(v) => v,
            Err(resp) => return resp,
        };

        let pdf = match build_vector_pdf(&VectorPdfInput {
            title: req.title.clone(),
            slides: req.slides.clone(),
            html: req.html.clone(),
            max_bytes: Some(MAX_BUNDLE_BYTES),
        }) {
            Ok(v) => v,
            Err(e) => return map_bundler_err(e, worker_req_id),
        };

        let export_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp_millis();

        if let Err(err) = persist_export_audit_run(
            &mut conn,
            &ExportAuditRecord {
                id: &export_id,
                tenant_id: &tenant_id,
                project_id: &req.project_id,
                audience_mode: "SHARE_PRESENT",
                format: "VECTOR_PDF",
                file_name: &pdf.file_name,
                artifact_sha256: &pdf.sha256,
                byte_size: pdf.byte_size,
                external_cdn_refs_count: 0,
                qc_gate_status: &qc_gate_status,
                qc_override_acknowledged,
                plugin_status: "NOT_APPLICABLE",
                created_at,
            },
        ) {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "EXPORT_AUDIT_PERSIST_FAILED",
                &format!("Failed to persist Vector PDF export receipt: {err}"),
                worker_req_id,
            );
        }

        ok_resp(
            VectorPdfExportData {
                export_id,
                project_id: req.project_id,
                audience_mode: "SHARE_PRESENT".to_string(),
                format: "VECTOR_PDF".to_string(),
                file_name: pdf.file_name,
                pdf_base64: pdf.pdf_base64,
                sha256: pdf.sha256,
                byte_size: pdf.byte_size,
                page_count: pdf.page_count,
                width_pt: pdf.width_pt,
                height_pt: pdf.height_pt,
                engine: pdf.engine,
                qc_gate_status,
                qc_override_acknowledged,
                warnings: pdf.warnings,
                created_at,
            },
            worker_req_id,
        )
    })
    .await
}

async fn handle_post_pptx(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_export_role(&verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_EXPORT_HTTP_BODY_BYTES {
        return map_bundler_err(
            ExportBundlerError::AssetSizeExceeded {
                actual_bytes: body.len(),
                limit_bytes: MAX_BUNDLE_BYTES,
            },
            request_id,
        );
    }
    let req: PptxExportRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(err) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid PPTX export payload: {err}"),
                request_id,
            );
        }
    };

    let is_legacy_plugin_probe = req.html.is_none()
        && req.inline_css.is_none()
        && req.cas_assets.is_empty()
        && req.export_mode.is_none()
        && req.locale.is_none();

    if (is_legacy_plugin_probe && body.len() > MAX_PPTX_HTTP_BODY_BYTES) || req.title.len() > 65_536
    {
        return map_bundler_err(
            ExportBundlerError::AssetSizeExceeded {
                actual_bytes: body.len(),
                limit_bytes: MAX_PPTX_HTTP_BODY_BYTES,
            },
            request_id,
        );
    }

    let worker_req_id = request_id.clone();
    run_blocking_export(request_id, move || {
        let tenant_id = match resolve_tenant_for_project(
            &state,
            &req.project_id,
            &headers,
            &verified,
            &worker_req_id,
        ) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

        let mut conn = match state.registry.pool().connection() {
            Ok(c) => c,
            Err(_) => {
                return err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Storage connection unavailable",
                    worker_req_id,
                );
            }
        };

        let (qc_gate_status, qc_override_acknowledged) = match enforce_pre_export_qc_gate(
            &conn,
            &req.project_id,
            req.override_qc_warning,
            &worker_req_id,
        ) {
            Ok(v) => v,
            Err(resp) => return resp,
        };

        let export_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp_millis();

        // Story 07.2 backward-compatible metadata-only plugin probe path
        if is_legacy_plugin_probe
            && !is_pptx_plugin_installed(&state, &conn, &tenant_id, &req.project_id)
        {
            let message = "PowerPoint (.pptx) export is provided by an optional extension. Install it from the Extension Center to continue without needing LibreOffice.".to_string();
            let _ = persist_export_audit_run(
                &mut conn,
                &ExportAuditRecord {
                    id: &export_id,
                    tenant_id: &tenant_id,
                    project_id: &req.project_id,
                    audience_mode: "SHARE_PRESENT",
                    format: "PLUGIN_PPTX",
                    file_name: "plugin-not-installed.pptx",
                    artifact_sha256: "",
                    byte_size: 0,
                    external_cdn_refs_count: 0,
                    qc_gate_status: &qc_gate_status,
                    qc_override_acknowledged,
                    plugin_status: "NOT_INSTALLED",
                    created_at,
                },
            );
            return (
                StatusCode::OK,
                Json(PptxExportResponse {
                    ok: true,
                    plugin_installed: false,
                    plugin_id: PPTX_PLUGIN_ID.to_string(),
                    install_action: PPTX_INSTALL_ACTION.to_string(),
                    message: message.clone(),
                    data: PptxExportData {
                        export_id,
                        project_id: req.project_id,
                        audience_mode: "SHARE_PRESENT".to_string(),
                        format: "PLUGIN_PPTX".to_string(),
                        plugin_installed: false,
                        plugin_id: PPTX_PLUGIN_ID.to_string(),
                        plugin_status: "NOT_INSTALLED".to_string(),
                        install_action: PPTX_INSTALL_ACTION.to_string(),
                        message,
                        export_mode: None,
                        pptx_base64: None,
                        file_name: None,
                        sha256: None,
                        byte_size: 0,
                        slide_count: None,
                        engine: None,
                        used_static_fallback: None,
                        fidelity_warnings: Vec::new(),
                        qc_gate_status,
                        qc_override_acknowledged,
                        created_at,
                    },
                    request_id: worker_req_id,
                }),
            )
                .into_response();
        }

        // Story 07.3 Built-in Native Rust OOXML `.pptx` Exporter (ACTIVE default)
        let export_mode = req.export_mode.unwrap_or(PptxExportMode::Editable);
        let cdp_pool = shared_cdp_evaluator();
        let pptx_bundle = match build_pptx_deck(
            &PptxExportInput {
                title: req.title.clone(),
                html: req.html.clone().unwrap_or_default(),
                inline_css: req.inline_css.clone(),
                cas_assets: req.cas_assets.clone(),
                export_mode,
                locale: req.locale.clone(),
                slide_count_hint: req.slide_count,
                cdp_snapshot: None,
                max_bytes: Some(MAX_BUNDLE_BYTES),
            },
            Some(cdp_pool.as_ref()),
        ) {
            Ok(b) => b,
            Err(err) => return map_bundler_err(err, worker_req_id),
        };

        if let Err(err) = persist_export_audit_run(
            &mut conn,
            &ExportAuditRecord {
                id: &export_id,
                tenant_id: &tenant_id,
                project_id: &req.project_id,
                audience_mode: "SHARE_PRESENT",
                format: "PLUGIN_PPTX",
                file_name: &pptx_bundle.file_name,
                artifact_sha256: &pptx_bundle.sha256,
                byte_size: pptx_bundle.byte_size,
                external_cdn_refs_count: 0,
                qc_gate_status: &qc_gate_status,
                qc_override_acknowledged,
                plugin_status: "BUILTIN_ACTIVE",
                created_at,
            },
        ) {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "EXPORT_AUDIT_PERSIST_FAILED",
                &format!("Failed to persist PPTX export receipt: {err}"),
                worker_req_id,
            );
        }

        let message = format!(
            "Built PowerPoint deck ({} slide(s)) with built-in native Rust exporter.",
            pptx_bundle.slide_count
        );
        (
            StatusCode::OK,
            Json(PptxExportResponse {
                ok: true,
                plugin_installed: true,
                plugin_id: BUILTIN_PPTX_PLUGIN_ID.to_string(),
                install_action: BUILTIN_PPTX_INSTALL_ACTION.to_string(),
                message: message.clone(),
                data: PptxExportData {
                    export_id,
                    project_id: req.project_id,
                    audience_mode: "SHARE_PRESENT".to_string(),
                    format: "PLUGIN_PPTX".to_string(),
                    plugin_installed: true,
                    plugin_id: BUILTIN_PPTX_PLUGIN_ID.to_string(),
                    plugin_status: "BUILTIN_ACTIVE".to_string(),
                    install_action: BUILTIN_PPTX_INSTALL_ACTION.to_string(),
                    message,
                    export_mode: Some(pptx_bundle.export_mode),
                    pptx_base64: Some(pptx_bundle.pptx_base64),
                    file_name: Some(pptx_bundle.file_name),
                    sha256: Some(pptx_bundle.sha256),
                    byte_size: pptx_bundle.byte_size,
                    slide_count: Some(pptx_bundle.slide_count),
                    engine: Some(pptx_bundle.engine),
                    used_static_fallback: Some(pptx_bundle.used_static_fallback),
                    fidelity_warnings: pptx_bundle.fidelity_warnings,
                    qc_gate_status,
                    qc_override_acknowledged,
                    created_at,
                },
                request_id: worker_req_id,
            }),
        )
            .into_response()
    })
    .await
}

async fn handle_post_developer_handoff(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v.clone(),
        Err(resp) => return resp,
    };
    if let Err(resp) = require_export_role(&verified, &request_id) {
        return resp;
    }
    if body.len() > MAX_EXPORT_HTTP_BODY_BYTES {
        return map_bundler_err(
            ExportBundlerError::AssetSizeExceeded {
                actual_bytes: body.len(),
                limit_bytes: MAX_BUNDLE_BYTES,
            },
            request_id,
        );
    }
    let req: DeveloperHandoffRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(err) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid Developer Handoff payload: {err}"),
                request_id,
            );
        }
    };

    let worker_req_id = request_id.clone();
    run_blocking_export(request_id, move || {
        let tenant_id = match resolve_tenant_for_project(
            &state,
            &req.project_id,
            &headers,
            &verified,
            &worker_req_id,
        ) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

        let mut conn = match state.registry.pool().connection() {
            Ok(c) => c,
            Err(_) => {
                return err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Storage connection unavailable",
                    worker_req_id,
                );
            }
        };

        let (qc_gate_status, qc_override_acknowledged) = match enforce_pre_export_qc_gate(
            &conn,
            &req.project_id,
            req.override_qc_warning,
            &worker_req_id,
        ) {
            Ok(v) => v,
            Err(resp) => return resp,
        };

        let handoff = match build_developer_handoff(&DeveloperHandoffInput {
            project_name: req.project_name.clone(),
            spec_markdown: req.spec_markdown.clone(),
            design_tokens_css: req.design_tokens_css.clone(),
            design_tokens_json: req.design_tokens_json.clone(),
            screen_tsx: req.screen_tsx.clone(),
            html_source: req.html_source.clone(),
            e2e_spec_ts: req.e2e_spec_ts.clone(),
        }) {
            Ok(b) => b,
            Err(e) => return map_bundler_err(e, worker_req_id),
        };

        let export_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp_millis();

        if let Err(err) = persist_export_audit_run(
            &mut conn,
            &ExportAuditRecord {
                id: &export_id,
                tenant_id: &tenant_id,
                project_id: &req.project_id,
                audience_mode: "DEVELOPER_HANDOFF",
                format: "DEV_HANDOFF_BUNDLE",
                file_name: &handoff.file_name,
                artifact_sha256: &handoff.sha256,
                byte_size: handoff.byte_size,
                external_cdn_refs_count: 0,
                qc_gate_status: &qc_gate_status,
                qc_override_acknowledged,
                plugin_status: "NOT_APPLICABLE",
                created_at,
            },
        ) {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "EXPORT_AUDIT_PERSIST_FAILED",
                &format!("Failed to persist Developer Handoff receipt: {err}"),
                worker_req_id,
            );
        }

        ok_resp(
            DeveloperHandoffData {
                export_id,
                project_id: req.project_id,
                audience_mode: "DEVELOPER_HANDOFF".to_string(),
                format: "DEV_HANDOFF_BUNDLE".to_string(),
                file_name: handoff.file_name,
                sha256: handoff.sha256,
                byte_size: handoff.byte_size,
                files: handoff.files,
                qc_gate_status,
                qc_override_acknowledged,
                created_at,
            },
            worker_req_id,
        )
    })
    .await
}

async fn handle_get_export_history(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    Query(query): Query<ExportHistoryQuery>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v.clone(),
        Err(resp) => return resp,
    };

    let worker_req_id = request_id.clone();
    run_blocking_export(request_id, move || {
        let tenant_id = match resolve_tenant_for_project(
            &state,
            &query.project_id,
            &headers,
            &verified,
            &worker_req_id,
        ) {
            Ok(t) => t,
            Err(resp) => return resp,
        };

        let mut conn = match state.registry.pool().connection() {
            Ok(c) => c,
            Err(_) => {
                return err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Storage connection unavailable",
                    worker_req_id,
                );
            }
        };
        if let Err(err) = ensure_v30_tables(&mut conn) {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                &format!("Failed to initialize export_bundle_runs: {err}"),
                worker_req_id,
            );
        }

        let qc_gate = match inspect_latest_qc_gate(&conn, &query.project_id) {
            Ok(summary) => summary,
            Err(_) => {
                return err_resp(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "STORAGE_BUSY",
                    "Unable to inspect QC gate status; storage temporarily unavailable",
                    worker_req_id,
                );
            }
        };
        // Story 07.3: Built-in Native Rust OOXML PPTX Exporter is permanently active by default
        let pptx_plugin_installed = true;
        let limit = query.limit.clamp(1, 100) as i64;

        let mut stmt = match conn.prepare(
            "SELECT id, project_id, audience_mode, format, file_name, artifact_sha256,
                    byte_size, external_cdn_refs_count, qc_gate_status,
                    qc_override_acknowledged, plugin_status, created_at
             FROM export_bundle_runs
             WHERE tenant_id = ?1 AND project_id = ?2
             ORDER BY created_at DESC
             LIMIT ?3",
        ) {
            Ok(s) => s,
            Err(err) => {
                return err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    &format!("Failed to query export history: {err}"),
                    worker_req_id,
                );
            }
        };

        let items: Vec<ExportHistoryItem> = stmt
            .query_map(
                params![tenant_id.trim(), query.project_id.trim(), limit],
                |row| {
                    let byte_size: i64 = row.get(6)?;
                    let ext_cdn: i64 = row.get(7)?;
                    let override_ack: i64 = row.get(9)?;
                    Ok(ExportHistoryItem {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        audience_mode: row.get(2)?,
                        format: row.get(3)?,
                        file_name: row.get(4)?,
                        artifact_sha256: row.get(5)?,
                        byte_size: byte_size.max(0) as usize,
                        external_cdn_refs_count: ext_cdn.max(0) as usize,
                        qc_gate_status: row.get(8)?,
                        qc_override_acknowledged: override_ack == 1,
                        plugin_status: row.get(10)?,
                        created_at: row.get(11)?,
                    })
                },
            )
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default();

        ok_resp(
            ExportHistoryData {
                project_id: query.project_id,
                qc_gate,
                pptx_plugin_installed,
                items,
            },
            worker_req_id,
        )
    })
    .await
}

pub fn split_export_router() -> Router<DaemonAppState> {
    Router::new()
        .route("/api/v1/export/html-bundle", post(handle_post_html_bundle))
        .route("/api/v1/export/vector-pdf", post(handle_post_vector_pdf))
        .route("/api/v1/export/pptx", post(handle_post_pptx))
        .route(
            "/api/v1/export/developer-handoff",
            post(handle_post_developer_handoff),
        )
        .route("/api/v1/export/history", get(handle_get_export_history))
        .layer(DefaultBodyLimit::max(MAX_EXPORT_HTTP_BODY_BYTES))
}
