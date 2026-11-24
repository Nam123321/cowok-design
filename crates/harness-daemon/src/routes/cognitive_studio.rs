// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use axum::{
    body::Bytes,
    extract::{Extension, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use domain::HarnessError;
use html_forge::{
    compute_sha256_hex, default_popular_deck_catalog, default_popular_fe_catalog,
    extract_fe_data_contract, generate_default_brand_proposals, is_valid_theme_mode_id,
    render_fe_data_contract_ts, render_tokenized_brand_preview, scan_sketch_components,
    BrandPreviewBundle, BrandProposalV1, FeDataContractDraft, HtmlForgeError,
    PopularComponentAuditItem, SketchPageInput, MAX_THEME_MODES,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::locks::is_valid_ssot_portal;
use uuid::Uuid;

use super::fs_jail;
use super::legacy_specs::verify_anti_idor_with_caller;
use super::DaemonAppState;
use crate::daemon_auth::{is_valid_superadmin_bearer, LegacyAuthKind, VerifiedLegacyIngress};

const MAX_BODY_BYTES: usize = 512 * 1024;
const ALLOWED_ENTRY_MODES: &[&str] = &[
    "GREENFIELD",
    "ADD_FEATURE",
    "REVAMP",
    "MICRO_POLISH",
    "DECK_FROM_APP",
];

const CANONICAL_HIDDEN_SPECS: &[(&str, &str)] = &[
    ("01_PRODUCT_BRIEF", "01 · Product Brief & Goals"),
    ("02_BRAND_IDENTITY", "02 · Brand Identity & Theme Modes"),
    ("03_UX_FLOW", "03 · UX Flow & Navigation"),
    ("04_DESIGN_TOKENS", "04 · Design Tokens & Contrast"),
    ("05_SCREEN_SPECS", "05 · Screen Specifications"),
    ("06_DATA_CONTRACT", "06 · FE Data Contract"),
    ("07_COMPONENT_REGISTRY", "07 · Component Registry"),
    ("08_INTERACTION_STATES", "08 · Interaction & Sub-States"),
    ("09_QA_ACCEPTANCE", "09 · Acceptance & Heritage"),
];

pub const V25_FALLBACK_DDL: &str = include_str!(
    "../../../spec-registry/migrations/V25__story_05_6_brand_proposals_and_heritage_packs.sql"
);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDetail {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateBrandProposalsRequest {
    project_id: String,
    #[serde(default)]
    reference_apps: Vec<String>,
    theme_modes: Vec<String>,
    #[serde(default = "default_locale")]
    locale: String,
    #[serde(default)]
    active_mode_id: Option<String>,
    #[serde(default)]
    selected_proposal_id: Option<String>,
    #[serde(default)]
    sketch_pages: Vec<SketchPageInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandProposalsResponseData {
    pub project_id: String,
    pub requested_modes: Vec<String>,
    pub active_mode_id: String,
    pub selected_proposal_id: String,
    pub proposals: Vec<BrandProposalV1>,
    pub preview_bundle: BrandPreviewBundle,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScanComponentsRequest {
    project_id: String,
    #[serde(default = "default_portal_id")]
    portal_id: String,
    #[serde(default = "default_surface")]
    surface: String,
    #[serde(default)]
    sketch_htmls: Vec<String>,
    #[serde(default)]
    selected_kinds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanComponentsResponseData {
    pub project_id: String,
    pub portal_id: String,
    pub extracted_kinds: Vec<String>,
    pub audit_items: Vec<PopularComponentAuditItem>,
    pub detected_count: usize,
    pub suggested_count: usize,
    pub not_needed_count: usize,
    pub synced_to_registry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeDataContractResponseData {
    pub draft: FeDataContractDraft,
    pub draft_ts_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevealSpecsFolderRequest {
    pub project_id: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealSpecsFolderResponseData {
    pub opened: bool,
    pub relative_path: String,
    pub specs_dir: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenSpecDocumentDto {
    pub spec_key: String,
    pub title: String,
    pub revision: i64,
    pub content_sha256: String,
    pub content_markdown: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PutHiddenSpecRequest {
    project_id: String,
    spec_key: String,
    content_markdown: String,
    expected_revision: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateHeritagePackRequest {
    source_project_id: String,
    name: String,
    #[serde(default = "default_entry_mode")]
    entry_mode: String,
    #[serde(default = "default_json_object")]
    brand_identity: serde_json::Value,
    #[serde(default = "default_json_object")]
    component_registry: serde_json::Value,
    #[serde(default = "default_json_object")]
    presentation_theme: serde_json::Value,
    theme_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeritageDesignPackDto {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub source_project_id: String,
    pub entry_mode: String,
    pub brand_identity: serde_json::Value,
    pub component_registry: serde_json::Value,
    pub presentation_theme: serde_json::Value,
    pub theme_modes: Vec<String>,
    pub created_at: i64,
}

fn default_locale() -> String {
    "vi".to_string()
}

fn default_portal_id() -> String {
    "SalesApp".to_string()
}

fn default_surface() -> String {
    "fe".to_string()
}

fn default_entry_mode() -> String {
    "GREENFIELD".to_string()
}

fn default_json_object() -> serde_json::Value {
    serde_json::json!({})
}

fn normalize_json_object(val: serde_json::Value) -> serde_json::Value {
    if val.is_object() {
        val
    } else {
        serde_json::json!({})
    }
}

fn parse_json_object_or_empty(raw: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(raw)
        .map(normalize_json_object)
        .unwrap_or_else(|_| serde_json::json!({}))
}

/// Ensures V25 tables exist idempotently on SQLite connections.
pub fn ensure_v25_tables(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(V25_FALLBACK_DDL)
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

fn err_resp(
    status: StatusCode,
    code: &str,
    message: &str,
    field: Option<&str>,
    details: Option<serde_json::Value>,
    request_id: String,
) -> Response {
    (
        status,
        Json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
                field: field.map(str::to_string),
                request_id,
                details,
            },
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

fn enforce_max_body_bytes(body: &Bytes, request_id: &str) -> Result<(), Response> {
    if body.len() > MAX_BODY_BYTES {
        return Err(err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Request body exceeds limit",
            Some("body"),
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn map_html_forge_error(err: HtmlForgeError, default_field: &str, request_id: String) -> Response {
    match err {
        HtmlForgeError::MxssBlocked(detail) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSS_SECURITY_BLOCKED",
            &format!("Unsafe HTML construct blocked: {detail}"),
            Some("sketchPages"),
            None,
            request_id,
        ),
        HtmlForgeError::CssTokenBlocked(detail) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSS_SECURITY_BLOCKED",
            &format!("Unsafe CSS token blocked: {detail}"),
            Some("brandTokens"),
            None,
            request_id,
        ),
        HtmlForgeError::PayloadTooLarge { actual, max } => err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            &format!("HTML payload size {actual} exceeds {max} bytes"),
            Some("sketchPages"),
            None,
            request_id,
        ),
        HtmlForgeError::SlotTextTooLong { actual, max } => err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            &format!("Slot text length {actual} exceeds {max}"),
            Some("sketchPages"),
            None,
            request_id,
        ),
        other => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            &other.to_string(),
            Some(default_field),
            None,
            request_id,
        ),
    }
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
            None,
            None,
            request_id.to_string(),
        )
    })?;
    if verified.kind == LegacyAuthKind::Bearer && !is_valid_superadmin_bearer(headers) {
        return Err(err_resp(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Untrusted Bearer token rejected; use session token, HMAC v2, or verified superadmin Bearer",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(verified)
}

fn require_write_role(verified: &VerifiedLegacyIngress, request_id: &str) -> Result<(), Response> {
    let ok = matches!(
        verified.principal.role.trim().to_ascii_lowercase().as_str(),
        "editor" | "admin" | "owner"
    );
    if !ok {
        return Err(err_resp(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Write operation requires editor or admin role",
            None,
            None,
            request_id.to_string(),
        ));
    }
    Ok(())
}

fn percent_decode_query_component(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push(((h << 4) | l) as u8);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query_map(raw_query: Option<&str>) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Some(q) = raw_query else {
        return map;
    };
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(
                percent_decode_query_component(k),
                percent_decode_query_component(v),
            );
        }
    }
    map
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
            Some("projectId"),
            None,
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
                    None,
                    None,
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
                        Some("projectId"),
                        None,
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
                    Some("projectId"),
                    None,
                    request_id.to_string(),
                ));
            }
            Ok(tenant_id)
        }
        Err(HarnessError::ProjectNotFound { .. }) => Err(err_resp(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "Project not found",
            Some("projectId"),
            None,
            request_id.to_string(),
        )),
        Err(_) => Err(err_resp(
            StatusCode::FORBIDDEN,
            "PROJECT_TENANT_MISMATCH",
            "Project access denied",
            Some("projectId"),
            None,
            request_id.to_string(),
        )),
    }
}

fn default_representative_sketch_pages() -> Vec<SketchPageInput> {
    vec![
        SketchPageInput {
            screen_id: "sketch-hero".into(),
            title: "Hero Overview".into(),
            page_role: "hero".into(),
            html: "<!DOCTYPE html><html><head></head><body><main data-ff-id=\"stage-root\"><section class=\"card\" data-component-kind=\"kpi-metric\"><h1 data-ff-id=\"ff-hero-title\">Workspace Overview</h1><p data-ff-id=\"ff-hero-sub\">Visual-first flow architecture</p><span data-ff-id=\"ff-kpi-mrr\">$24,800</span></section></main></body></html>".into(),
        },
        SketchPageInput {
            screen_id: "sketch-list".into(),
            title: "List & Detail".into(),
            page_role: "list_detail".into(),
            html: "<!DOCTYPE html><html><head></head><body><main><section class=\"card\"><table data-ff-id=\"ff-records-table\" data-component-kind=\"data-table\"><thead><tr><th>Record</th><th>Status</th></tr></thead><tbody><tr><td>Deal Alpha</td><td>Active</td></tr></tbody></table></section></main></body></html>".into(),
        },
        SketchPageInput {
            screen_id: "sketch-form".into(),
            title: "Action Form".into(),
            page_role: "action_form".into(),
            html: "<!DOCTYPE html><html><head></head><body><main><section class=\"card\"><form data-ff-id=\"ff-action-form\"><input name=\"title\" placeholder=\"New item\" /><button type=\"submit\" data-ff-id=\"ff-submit-btn\">Confirm</button></form></section></main></body></html>".into(),
        },
    ]
}

fn load_project_sketch_pages(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
) -> Vec<SketchPageInput> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT screen_id, html_snapshot
         FROM screen_artifact_patches
         WHERE tenant_id = ?1 AND project_id = ?2
         GROUP BY screen_id
         HAVING ast_revision = MAX(ast_revision)
         ORDER BY created_at DESC
         LIMIT 32",
    ) else {
        return default_representative_sketch_pages();
    };
    let rows = stmt
        .query_map(params![tenant_id, project_id], |r| {
            let sid: String = r.get(0)?;
            let html: String = r.get(1)?;
            Ok(SketchPageInput {
                title: sid.clone(),
                screen_id: sid,
                page_role: String::new(),
                html,
            })
        })
        .ok()
        .map(|it| {
            it.flatten()
                .filter(|p| !p.html.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if rows.is_empty() {
        default_representative_sketch_pages()
    } else {
        rows
    }
}

fn extract_body_inner(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    if let Some(open_idx) = lower.find("<body") {
        if let Some(gt_rel) = lower[open_idx..].find('>') {
            let start = open_idx + gt_rel + 1;
            if let Some(close_rel) = lower[start..].find("</body>") {
                return &html[start..start + close_rel];
            }
            return &html[start..];
        }
    }
    html
}

fn default_representative_fe_contract_html() -> String {
    let mut combined = String::from("<!DOCTYPE html><html><head></head><body>");
    for page in default_representative_sketch_pages() {
        combined.push_str(extract_body_inner(&page.html));
        combined.push('\n');
    }
    combined.push_str("</body></html>");
    combined
}

fn validate_jailed_abs_dir(dir: &Path) -> Result<(), HarnessError> {
    let raw = dir.to_string_lossy();
    if raw.trim().is_empty() || raw.contains('\0') || !dir.is_absolute() {
        return Err(HarnessError::PathTraversalDenied {
            path: raw.into_owned(),
        });
    }
    if dir.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(HarnessError::PathTraversalDenied {
            path: raw.into_owned(),
        });
    }
    Ok(())
}

fn upsert_hidden_spec_in_tx(
    tx: &rusqlite::Transaction<'_>,
    tenant_id: &str,
    project_id: &str,
    spec_key: &str,
    content_markdown: &str,
    now_ms: i64,
) -> Result<i64, rusqlite::Error> {
    let current_rev: i64 = tx
        .query_row(
            "SELECT revision FROM hidden_spec_documents
             WHERE tenant_id = ?1 AND project_id = ?2 AND spec_key = ?3",
            params![tenant_id, project_id, spec_key],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    let next_rev = current_rev + 1;
    let sha = compute_sha256_hex(content_markdown.as_bytes());
    tx.execute(
        "INSERT INTO hidden_spec_documents (
            id, tenant_id, project_id, spec_key, revision, content_sha256, content_markdown, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(tenant_id, project_id, spec_key)
        DO UPDATE SET revision = excluded.revision, content_sha256 = excluded.content_sha256,
                      content_markdown = excluded.content_markdown, updated_at = excluded.updated_at",
        params![
            Uuid::new_v4().to_string(),
            tenant_id,
            project_id,
            spec_key,
            next_rev,
            sha,
            content_markdown,
            now_ms
        ],
    )?;
    Ok(next_rev)
}

fn ensure_project_specs_dir_fd(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
) -> Option<(PathBuf, std::os::fd::OwnedFd)> {
    let workspace_root: Option<String> = conn
        .query_row(
            "SELECT workspace_root FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    let ws_str = workspace_root.filter(|s| !s.trim().is_empty())?;
    let ws_path = PathBuf::from(ws_str.trim());
    if validate_jailed_abs_dir(&ws_path).is_err() {
        return None;
    }
    let ws_fd = fs_jail::open_dir_abs_nofollow(&ws_path).ok()?;
    let cowok_fd = fs_jail::ensure_child_dir_fd(&ws_fd, OsStr::new(".cowok-design"), 0o700).ok()?;
    let specs_fd = fs_jail::ensure_child_dir_fd(&cowok_fd, OsStr::new("specs"), 0o700).ok()?;
    let specs_dir = ws_path.join(".cowok-design").join("specs");
    Some((specs_dir, specs_fd))
}

fn mirror_hidden_spec_to_workspace(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
    spec_key: &str,
    markdown: &str,
) {
    if spec_title_for_key(spec_key).is_none() {
        return;
    }
    let Some((_, specs_fd)) = ensure_project_specs_dir_fd(conn, tenant_id, project_id) else {
        return;
    };
    let file_name = format!("{spec_key}.md");
    let _ = fs_jail::write_file_in_dir_fd(
        &specs_fd,
        OsStr::new(&file_name),
        markdown.as_bytes(),
        0o600,
    );
}

fn mirror_fe_contract_to_workspace(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    project_id: &str,
    draft_ts_source: &str,
) -> Option<String> {
    let (_, specs_fd) = ensure_project_specs_dir_fd(conn, tenant_id, project_id)?;
    fs_jail::write_file_in_dir_fd(
        &specs_fd,
        OsStr::new("fe-data-contract.draft.ts"),
        draft_ts_source.as_bytes(),
        0o600,
    )
    .ok()?;
    Some(".cowok-design/specs/fe-data-contract.draft.ts".to_string())
}

fn render_brand_identity_markdown(
    selected: &BrandProposalV1,
    requested_modes: &[String],
    active_mode: &str,
) -> String {
    format!(
        "# 02 · Brand Identity & Theme Modes\n\n\
         - **Proposal ID:** `{}`\n\
         - **Brand Name:** {}\n\
         - **Active Mode:** `{}`\n\
         - **Theme Modes:** {}\n\
         - **Inspiration Refs:** {}\n\
         - **Font Pairing:** heading=`{}`, body=`{}`, mono=`{}`\n\
         - **Rationale:** {}\n",
        selected.id,
        selected.brand_name,
        active_mode,
        requested_modes.join(", "),
        selected.inspiration_refs.join(", "),
        selected.font_pairing.heading,
        selected.font_pairing.body,
        selected.font_pairing.mono,
        selected.rationale
    )
}

fn render_design_tokens_markdown(selected: &BrandProposalV1, active_mode: &str) -> String {
    let mut out = format!(
        "# 04 · Design Tokens & Contrast\n\n- **Proposal ID:** `{}`\n- **Active Mode:** `{}`\n",
        selected.id, active_mode
    );
    for mode in &selected.theme_modes {
        out.push_str(&format!(
            "\n## Mode `{}` (WCAG AA: {})\n",
            mode.mode_id, mode.wcag_aa_verified
        ));
        for (k, v) in &mode.tokens {
            out.push_str(&format!("- `{k}`: `{v}`\n"));
        }
    }
    out
}

fn render_component_registry_markdown(
    portal_id: &str,
    items: &[PopularComponentAuditItem],
) -> String {
    let mut out = format!(
        "# 07 · Component Registry\n\n- **Portal:** `{portal_id}`\n\n## Selected Components\n"
    );
    for item in items.iter().filter(|i| i.selected) {
        let reason = item
            .reason
            .as_deref()
            .unwrap_or("Extracted from Phase 1 Sketch");
        out.push_str(&format!("- `{}`: {}\n", item.component_kind, reason));
    }
    out
}

/// `POST /api/v1/cognitive/brand-proposals`
pub async fn handle_post_brand_proposals(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(verified, &request_id) {
        return resp;
    }
    if let Err(resp) = enforce_max_body_bytes(&body, &request_id) {
        return resp;
    }
    let req: GenerateBrandProposalsRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid brand proposals payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };
    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.project_id,
        &headers,
        verified,
        &request_id,
    ) {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    let proposals = match generate_default_brand_proposals(
        &req.reference_apps,
        &req.theme_modes,
        &req.locale,
    ) {
        Ok(p) => p,
        Err(e) => return map_html_forge_error(e, "themeModes", request_id),
    };

    let active_mode = req
        .active_mode_id
        .as_deref()
        .unwrap_or(&req.theme_modes[0])
        .to_string();
    let selected = req
        .selected_proposal_id
        .as_deref()
        .and_then(|id| proposals.iter().find(|p| p.id == id))
        .unwrap_or(&proposals[0]);

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    let _ = ensure_v25_tables(&conn);
    let pages = if req.sketch_pages.is_empty() {
        load_project_sketch_pages(&conn, &tenant_id, &req.project_id)
    } else {
        req.sketch_pages.clone()
    };
    let preview_bundle = match render_tokenized_brand_preview(&pages, selected, &active_mode) {
        Ok(b) => b,
        Err(e) => return map_html_forge_error(e, "activeModeId", request_id),
    };

    let now_ms = Utc::now().timestamp_millis();
    let modes_json = serde_json::to_string(&req.theme_modes).unwrap_or_else(|_| "[]".into());
    let brand_md = render_brand_identity_markdown(selected, &req.theme_modes, &active_mode);
    let tokens_md = render_design_tokens_markdown(selected, &active_mode);
    let brand_committed = if let Ok(tx) =
        conn.transaction_with_behavior(TransactionBehavior::Immediate)
    {
        for prop in &proposals {
            let prop_json = serde_json::to_string(prop).unwrap_or_else(|_| "{}".into());
            let is_sel = if prop.id == selected.id { 1 } else { 0 };
            let _ = tx.execute(
                    "INSERT INTO brand_proposal_snapshots (
                        id, tenant_id, project_id, proposal_id, proposal_json, theme_modes_json, is_selected, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        Uuid::new_v4().to_string(),
                        tenant_id,
                        req.project_id,
                        prop.id,
                        prop_json,
                        modes_json,
                        is_sel,
                        now_ms
                    ],
                );
        }
        let _ = upsert_hidden_spec_in_tx(
            &tx,
            &tenant_id,
            &req.project_id,
            "02_BRAND_IDENTITY",
            &brand_md,
            now_ms,
        );
        let _ = upsert_hidden_spec_in_tx(
            &tx,
            &tenant_id,
            &req.project_id,
            "04_DESIGN_TOKENS",
            &tokens_md,
            now_ms,
        );
        tx.commit().is_ok()
    } else {
        false
    };
    if brand_committed {
        mirror_hidden_spec_to_workspace(
            &conn,
            &tenant_id,
            &req.project_id,
            "02_BRAND_IDENTITY",
            &brand_md,
        );
        mirror_hidden_spec_to_workspace(
            &conn,
            &tenant_id,
            &req.project_id,
            "04_DESIGN_TOKENS",
            &tokens_md,
        );
    }

    ok_resp(
        BrandProposalsResponseData {
            project_id: req.project_id,
            requested_modes: req.theme_modes,
            active_mode_id: active_mode,
            selected_proposal_id: selected.id.clone(),
            proposals,
            preview_bundle,
        },
        request_id,
    )
}

/// `POST /api/v1/cognitive/scan-components`
pub async fn handle_post_scan_components(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(verified, &request_id) {
        return resp;
    }
    if let Err(resp) = enforce_max_body_bytes(&body, &request_id) {
        return resp;
    }
    let req: ScanComponentsRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid scan payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };
    if !is_valid_ssot_portal(&req.portal_id) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Invalid portalId",
            Some("portalId"),
            None,
            request_id,
        );
    }
    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.project_id,
        &headers,
        verified,
        &request_id,
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
                None,
                None,
                request_id,
            );
        }
    };
    let _ = ensure_v25_tables(&conn);
    let html_sources: Vec<String> = if req.sketch_htmls.is_empty() {
        load_project_sketch_pages(&conn, &tenant_id, &req.project_id)
            .into_iter()
            .map(|p| p.html)
            .collect()
    } else {
        req.sketch_htmls.clone()
    };
    let refs: Vec<&str> = html_sources.iter().map(String::as_str).collect();
    let catalog = if req.surface.eq_ignore_ascii_case("deck") {
        default_popular_deck_catalog()
    } else {
        default_popular_fe_catalog()
    };
    let mut scan = scan_sketch_components(&refs, &catalog);
    if let Some(selected_kinds) = &req.selected_kinds {
        for item in &mut scan.audit_items {
            item.selected = selected_kinds.contains(&item.component_kind);
        }
    }

    let now_ms = Utc::now().timestamp_millis();
    let mut synced_count = 0usize;
    let selected_slugs: HashSet<String> = scan
        .audit_items
        .iter()
        .filter(|i| i.selected)
        .map(|i| format!("studio-{}", i.component_kind))
        .collect();
    let registry_md = render_component_registry_markdown(&req.portal_id, &scan.audit_items);
    let scan_committed = if let Ok(tx) =
        conn.transaction_with_behavior(TransactionBehavior::Immediate)
    {
        let existing_slugs: Vec<String> = tx
            .prepare(
                "SELECT pattern_slug FROM component_patterns
                     WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3
                       AND pattern_slug LIKE 'studio-%'",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(params![tenant_id, req.project_id, req.portal_id], |r| {
                    r.get::<_, String>(0)
                })
                .ok()
                .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        for old_slug in existing_slugs {
            if !selected_slugs.contains(&old_slug) {
                let _ = tx.execute(
                        "DELETE FROM component_patterns
                         WHERE tenant_id = ?1 AND project_id = ?2 AND portal_id = ?3 AND pattern_slug = ?4",
                        params![tenant_id, req.project_id, req.portal_id, old_slug],
                    );
            }
        }
        for item in scan.audit_items.iter().filter(|i| i.selected) {
            let slug = format!("studio-{}", item.component_kind);
            let sig = compute_sha256_hex(item.component_kind.as_bytes());
            let doc = item
                .reason
                .as_deref()
                .unwrap_or("Extracted from Phase 1 Sketch");
            if tx
                    .execute(
                        "INSERT INTO component_patterns (
                            id, tenant_id, project_id, portal_id, pattern_slug, component_name,
                            source_path, ast_signature_sha256, dedup_marker_sha256, source_revision,
                            last_sync_source, tokens_snapshot_json, doc_markdown, drift_status,
                            quarantined_diff_json, revision, created_at, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'sketch-phase-1', ?7, ?7, 1, 'manual', '{}', ?8, 'synced', '{}', 1, ?9, ?9)
                        ON CONFLICT(tenant_id, project_id, portal_id, pattern_slug)
                        DO UPDATE SET doc_markdown = excluded.doc_markdown, revision = revision + 1, updated_at = excluded.updated_at",
                        params![
                            Uuid::new_v4().to_string(),
                            tenant_id,
                            req.project_id,
                            req.portal_id,
                            slug,
                            item.component_kind,
                            sig,
                            doc,
                            now_ms
                        ],
                    )
                    .is_ok()
                {
                    synced_count += 1;
                }
        }
        let _ = upsert_hidden_spec_in_tx(
            &tx,
            &tenant_id,
            &req.project_id,
            "07_COMPONENT_REGISTRY",
            &registry_md,
            now_ms,
        );
        tx.commit().is_ok()
    } else {
        false
    };
    if scan_committed {
        mirror_hidden_spec_to_workspace(
            &conn,
            &tenant_id,
            &req.project_id,
            "07_COMPONENT_REGISTRY",
            &registry_md,
        );
    }

    ok_resp(
        ScanComponentsResponseData {
            project_id: req.project_id,
            portal_id: req.portal_id,
            extracted_kinds: scan.extracted_kinds,
            audit_items: scan.audit_items,
            detected_count: scan.detected_count,
            suggested_count: scan.suggested_count,
            not_needed_count: scan.not_needed_count,
            synced_to_registry_count: synced_count,
        },
        request_id,
    )
}

/// `GET /api/v1/cognitive/fe-data-contract?projectId=...&screenId=...`
pub async fn handle_get_fe_data_contract(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let qp = parse_query_map(raw_query.as_deref());
    let project_id = qp.get("projectId").cloned().unwrap_or_default();
    let screen_id = qp
        .get("screenId")
        .cloned()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let tenant_id =
        match resolve_tenant_for_project(&state, &project_id, &headers, verified, &request_id) {
            Ok(t) => t,
            Err(resp) => return resp,
        };
    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    let html_opt: Option<String> = conn
        .query_row(
            "SELECT html_snapshot FROM screen_artifact_patches
             WHERE tenant_id = ?1 AND project_id = ?2 AND screen_id = ?3
             ORDER BY ast_revision DESC LIMIT 1",
            params![tenant_id, project_id, screen_id],
            |r| r.get(0),
        )
        .optional()
        .unwrap_or(None);
    let html = html_opt
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(default_representative_fe_contract_html);
    let draft = extract_fe_data_contract(&project_id, &screen_id, &html);
    let draft_ts_source = render_fe_data_contract_ts(&draft);
    let disk_path = if verified.principal.role != "viewer" {
        mirror_fe_contract_to_workspace(&conn, &tenant_id, &project_id, &draft_ts_source)
    } else {
        Some(".cowok-design/specs/fe-data-contract.draft.ts".to_string())
    };
    ok_resp(
        FeDataContractResponseData {
            draft,
            draft_ts_source,
            disk_path,
        },
        request_id,
    )
}

const MAX_REVEAL_BODY_BYTES: usize = 4_096;

fn is_safe_spec_filename(raw: &str) -> bool {
    !raw.is_empty()
        && raw.len() <= 96
        && !raw.starts_with('.')
        && !raw.starts_with('-')
        && !raw.contains('/')
        && !raw.contains('\\')
        && !raw.contains("..")
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn spawn_os_reveal(specs_dir: &Path, target_file: Option<&Path>) -> bool {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("/usr/bin/open");
        if let Some(file_path) = target_file {
            cmd.arg("-R").arg(file_path);
        } else {
            cmd.arg(specs_dir);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }
    #[cfg(target_os = "linux")]
    {
        let _ = target_file;
        std::process::Command::new("xdg-open")
            .arg(specs_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("explorer");
        if let Some(file_path) = target_file {
            cmd.arg(format!("/select,{}", file_path.display()));
        } else {
            cmd.arg(specs_dir);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }
}

/// `POST /api/v1/cognitive/specs/reveal`
pub async fn handle_post_reveal_specs_folder(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if verified.principal.role == "viewer" {
        return err_resp(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Editor or admin role required",
            None,
            None,
            request_id,
        );
    }
    if body.len() > MAX_REVEAL_BODY_BYTES {
        return err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PAYLOAD_TOO_LARGE",
            "Reveal request payload exceeds 4096 bytes",
            None,
            None,
            request_id,
        );
    }
    let req: RevealSpecsFolderRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(_) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "Invalid reveal specs folder JSON payload",
                None,
                None,
                request_id,
            );
        }
    };
    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.project_id,
        &headers,
        verified,
        &request_id,
    ) {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    if let Some(ref fname) = req.file_name {
        if !is_safe_spec_filename(fname.trim()) {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                "Invalid spec file name",
                Some("fileName"),
                None,
                request_id,
            );
        }
    }
    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    let Some((specs_dir, specs_fd)) =
        ensure_project_specs_dir_fd(&conn, &tenant_id, &req.project_id)
    else {
        return err_resp(
            StatusCode::NOT_FOUND,
            "FS_JAIL_ERROR",
            "Project specs folder is not available on local disk",
            Some("projectId"),
            None,
            request_id,
        );
    };
    let safe_name = req
        .file_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let target_file = safe_name.and_then(|name| {
        if !fs_jail::check_child_non_symlink_in_dir_fd(&specs_fd, OsStr::new(name)) {
            return None;
        }
        let candidate = specs_dir.join(name);
        std::fs::symlink_metadata(&candidate)
            .ok()
            .filter(|m| m.file_type().is_file())
            .map(|_| candidate)
    });
    let relative_path = match safe_name {
        Some(name) => format!(".cowok-design/specs/{name}"),
        None => ".cowok-design/specs".to_string(),
    };
    let headless_env = std::env::var("COWOK_HEADLESS_TEST")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        || std::env::var("CI")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let (opened, mode) = if req.dry_run || headless_env {
        (false, "dry_run".to_string())
    } else if spawn_os_reveal(&specs_dir, target_file.as_deref()) {
        (true, "os_file_manager".to_string())
    } else {
        (false, "unavailable".to_string())
    };
    ok_resp(
        RevealSpecsFolderResponseData {
            opened,
            relative_path,
            specs_dir: specs_dir.display().to_string(),
            mode,
        },
        request_id,
    )
}

fn spec_title_for_key(key: &str) -> Option<&'static str> {
    CANONICAL_HIDDEN_SPECS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, title)| *title)
}

/// `GET /api/v1/cognitive/hidden-specs?projectId=...`
pub async fn handle_get_hidden_specs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let qp = parse_query_map(raw_query.as_deref());
    let project_id = qp.get("projectId").cloned().unwrap_or_default();
    let tenant_id =
        match resolve_tenant_for_project(&state, &project_id, &headers, verified, &request_id) {
            Ok(t) => t,
            Err(resp) => return resp,
        };
    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    let _ = ensure_v25_tables(&conn);

    let mut stored: BTreeMap<String, (i64, String, String, i64)> = BTreeMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT spec_key, revision, content_sha256, content_markdown, updated_at
         FROM hidden_spec_documents
         WHERE tenant_id = ?1 AND project_id = ?2",
    ) {
        if let Ok(rows) = stmt.query_map(params![tenant_id, project_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        }) {
            for row in rows.flatten() {
                stored.insert(row.0, (row.1, row.2, row.3, row.4));
            }
        }
    }

    let specs: Vec<HiddenSpecDocumentDto> = CANONICAL_HIDDEN_SPECS
        .iter()
        .map(|(key, title)| {
            if let Some((rev, sha, md, ts)) = stored.get(*key) {
                HiddenSpecDocumentDto {
                    spec_key: (*key).to_string(),
                    title: (*title).to_string(),
                    revision: *rev,
                    content_sha256: sha.clone(),
                    content_markdown: md.clone(),
                    updated_at: *ts,
                }
            } else {
                let default_md = format!("# {title}\n\nAuto-synced hidden specification.\n");
                HiddenSpecDocumentDto {
                    spec_key: (*key).to_string(),
                    title: (*title).to_string(),
                    revision: 0,
                    content_sha256: compute_sha256_hex(default_md.as_bytes()),
                    content_markdown: default_md,
                    updated_at: 0,
                }
            }
        })
        .collect();

    ok_resp(
        serde_json::json!({ "projectId": project_id, "specs": specs }),
        request_id,
    )
}

/// `PUT /api/v1/cognitive/hidden-specs` — strict OCC (`expectedRevision`) returning `409 STALE_MAPPING`.
pub async fn handle_put_hidden_spec(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(verified, &request_id) {
        return resp;
    }
    if let Err(resp) = enforce_max_body_bytes(&body, &request_id) {
        return resp;
    }
    let req: PutHiddenSpecRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid hidden spec payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };
    let Some(title) = spec_title_for_key(&req.spec_key) else {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "specKey must be a canonical 01..09 specification key",
            Some("specKey"),
            None,
            request_id,
        );
    };
    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.project_id,
        &headers,
        verified,
        &request_id,
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
                None,
                None,
                request_id,
            );
        }
    };
    let _ = ensure_v25_tables(&conn);

    let now_ms = Utc::now().timestamp_millis();
    let sha = compute_sha256_hex(req.content_markdown.as_bytes());
    let tx = match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(t) => t,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Unable to acquire SQLite write transaction",
                None,
                None,
                request_id,
            );
        }
    };
    let current_rev: i64 = tx
        .query_row(
            "SELECT revision FROM hidden_spec_documents
             WHERE tenant_id = ?1 AND project_id = ?2 AND spec_key = ?3",
            params![tenant_id, req.project_id, req.spec_key],
            |r| r.get(0),
        )
        .optional()
        .unwrap_or(None)
        .unwrap_or(0);

    if current_rev != req.expected_revision {
        return err_resp(
            StatusCode::CONFLICT,
            "STALE_MAPPING",
            &format!(
                "Concurrent edit conflict on {}: expected revision {}, found {}",
                req.spec_key, req.expected_revision, current_rev
            ),
            Some("expectedRevision"),
            Some(serde_json::json!({
                "specKey": req.spec_key,
                "expectedRevision": req.expected_revision,
                "currentRevision": current_rev,
            })),
            request_id,
        );
    }

    let next_rev = current_rev + 1;
    let exec_res = tx.execute(
        "INSERT INTO hidden_spec_documents (
            id, tenant_id, project_id, spec_key, revision, content_sha256, content_markdown, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(tenant_id, project_id, spec_key)
        DO UPDATE SET revision = excluded.revision, content_sha256 = excluded.content_sha256,
                      content_markdown = excluded.content_markdown, updated_at = excluded.updated_at",
        params![
            Uuid::new_v4().to_string(),
            tenant_id,
            req.project_id,
            req.spec_key,
            next_rev,
            sha,
            req.content_markdown,
            now_ms
        ],
    );
    if exec_res.is_err() || tx.commit().is_err() {
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Failed to persist hidden spec update",
            None,
            None,
            request_id,
        );
    }

    mirror_hidden_spec_to_workspace(
        &conn,
        &tenant_id,
        &req.project_id,
        &req.spec_key,
        &req.content_markdown,
    );

    ok_resp(
        HiddenSpecDocumentDto {
            spec_key: req.spec_key,
            title: title.to_string(),
            revision: next_rev,
            content_sha256: sha,
            content_markdown: req.content_markdown,
            updated_at: now_ms,
        },
        request_id,
    )
}

fn resolve_heritage_packs_base_dir() -> Result<PathBuf, HarnessError> {
    let dir = if let Ok(override_dir) = std::env::var("COWOK_HERITAGE_PACKS_ROOT") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            PathBuf::from(trimmed)
        } else {
            let home = std::env::var("HOME").map_err(|_| HarnessError::BoundaryViolation {
                reason: "HOME environment variable is unset".to_string(),
            })?;
            PathBuf::from(home)
        }
    } else {
        let home = std::env::var("HOME").map_err(|_| HarnessError::BoundaryViolation {
            reason: "HOME environment variable is unset".to_string(),
        })?;
        PathBuf::from(home)
    };
    validate_jailed_abs_dir(&dir)?;
    Ok(dir)
}

fn open_or_provision_heritage_base_fd(
    base_dir: &Path,
) -> Result<std::os::fd::OwnedFd, HarnessError> {
    validate_jailed_abs_dir(base_dir)?;
    match fs_jail::open_dir_abs_nofollow(base_dir) {
        Ok(fd) => Ok(fd),
        Err(HarnessError::ValidationFailed { ref reason })
            if reason.starts_with("DIR_NOT_FOUND:") =>
        {
            fs_jail::provision_dir_zero_toctou(base_dir)?;
            fs_jail::open_dir_abs_nofollow(base_dir)
        }
        Err(err) => Err(err),
    }
}

fn write_jailed_heritage_pack_file(pack: &HeritageDesignPackDto) -> Result<String, HarnessError> {
    let base_dir = resolve_heritage_packs_base_dir()?;
    let base_fd = open_or_provision_heritage_base_fd(&base_dir)?;
    let cowok_fd = fs_jail::ensure_child_dir_fd(&base_fd, OsStr::new(".cowok-design"), 0o700)?;
    let packs_fd = fs_jail::ensure_child_dir_fd(&cowok_fd, OsStr::new("heritage-packs"), 0o700)?;
    let file_name = format!("{}.json", pack.id);
    let payload = serde_json::to_vec_pretty(pack).unwrap_or_default();
    fs_jail::write_file_in_dir_fd(&packs_fd, OsStr::new(&file_name), &payload, 0o600)?;
    Ok(base_dir
        .join(".cowok-design")
        .join("heritage-packs")
        .join(file_name)
        .display()
        .to_string())
}

fn unlink_jailed_heritage_pack_file(pack_id: &str) {
    let Ok(base_dir) = resolve_heritage_packs_base_dir() else {
        return;
    };
    let Ok(base_fd) = fs_jail::open_dir_abs_nofollow(&base_dir) else {
        return;
    };
    let Ok(Some(cowok_fd)) =
        fs_jail::open_child_dir_fd_nofollow(&base_fd, OsStr::new(".cowok-design"))
    else {
        return;
    };
    let Ok(Some(packs_fd)) =
        fs_jail::open_child_dir_fd_nofollow(&cowok_fd, OsStr::new("heritage-packs"))
    else {
        return;
    };
    let file_name = format!("{pack_id}.json");
    let _ = fs_jail::remove_rel_file_in_dir_fd(&packs_fd, Path::new(&file_name));
}

fn load_jailed_heritage_pack_files_for_tenant(tenant_id: &str) -> Vec<HeritageDesignPackDto> {
    let Ok(base_dir) = resolve_heritage_packs_base_dir() else {
        return Vec::new();
    };
    let Ok(base_fd) = fs_jail::open_dir_abs_nofollow(&base_dir) else {
        return Vec::new();
    };
    let Ok(Some(cowok_fd)) =
        fs_jail::open_child_dir_fd_nofollow(&base_fd, OsStr::new(".cowok-design"))
    else {
        return Vec::new();
    };
    let Ok(Some(packs_fd)) =
        fs_jail::open_child_dir_fd_nofollow(&cowok_fd, OsStr::new("heritage-packs"))
    else {
        return Vec::new();
    };
    let Ok(entries) = fs_jail::read_dir_fd_nofollow(&packs_fd, "heritage-packs") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        if !entry.is_file || entry.is_symlink {
            continue;
        }
        let name_str = entry.name.to_string_lossy();
        if !name_str.ends_with(".json") {
            continue;
        }
        let Ok(bytes) = fs_jail::read_file_in_dir_fd(&packs_fd, &entry.name, MAX_BODY_BYTES) else {
            continue;
        };
        let Ok(mut pack) = serde_json::from_slice::<HeritageDesignPackDto>(&bytes) else {
            continue;
        };
        if pack.tenant_id.trim() != tenant_id {
            continue;
        }
        if !ALLOWED_ENTRY_MODES.contains(&pack.entry_mode.as_str())
            || pack.theme_modes.is_empty()
            || pack.theme_modes.len() > MAX_THEME_MODES
            || !pack.theme_modes.iter().all(|m| is_valid_theme_mode_id(m))
        {
            continue;
        }
        pack.brand_identity = normalize_json_object(pack.brand_identity);
        pack.component_registry = normalize_json_object(pack.component_registry);
        pack.presentation_theme = normalize_json_object(pack.presentation_theme);
        out.push(pack);
    }
    out
}

/// `GET /api/v1/cognitive/heritage-packs`
pub async fn handle_get_heritage_packs(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    let _ = ensure_v25_tables(&conn);
    let tenant_id = verified.principal.tenant_id.trim().to_string();
    let mut packs = Vec::new();
    let mut seen_ids = HashSet::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, tenant_id, name, source_project_id, entry_mode, brand_identity_json,
                component_registry_json, presentation_theme_json, theme_modes_json, created_at
         FROM heritage_design_packs
         WHERE tenant_id = ?1
         ORDER BY created_at DESC
         LIMIT 50",
    ) {
        if let Ok(rows) = stmt.query_map(params![tenant_id], |r| {
            let brand_s: String = r.get(5)?;
            let comp_s: String = r.get(6)?;
            let pres_s: String = r.get(7)?;
            let modes_s: String = r.get(8)?;
            Ok(HeritageDesignPackDto {
                id: r.get(0)?,
                tenant_id: r.get(1)?,
                name: r.get(2)?,
                source_project_id: r.get(3)?,
                entry_mode: r.get(4)?,
                brand_identity: parse_json_object_or_empty(&brand_s),
                component_registry: parse_json_object_or_empty(&comp_s),
                presentation_theme: parse_json_object_or_empty(&pres_s),
                theme_modes: serde_json::from_str(&modes_s).unwrap_or_default(),
                created_at: r.get(9)?,
            })
        }) {
            for pack in rows.flatten() {
                if seen_ids.insert(pack.id.clone()) {
                    packs.push(pack);
                }
            }
        }
    }
    for disk_pack in load_jailed_heritage_pack_files_for_tenant(&tenant_id) {
        if seen_ids.insert(disk_pack.id.clone()) {
            packs.push(disk_pack);
        }
    }
    packs.sort_by_key(|a| std::cmp::Reverse(a.created_at));
    packs.truncate(50);
    ok_resp(serde_json::json!({ "packs": packs }), request_id)
}

/// `POST /api/v1/cognitive/heritage-packs`
pub async fn handle_post_heritage_pack(
    State(state): State<DaemonAppState>,
    headers: HeaderMap,
    caller: Option<Extension<VerifiedLegacyIngress>>,
    body: Bytes,
) -> Response {
    let request_id = extract_request_id(&headers);
    let verified = match require_caller(caller.as_ref().map(|e| &e.0), &headers, &request_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_write_role(verified, &request_id) {
        return resp;
    }
    if let Err(resp) = enforce_max_body_bytes(&body, &request_id) {
        return resp;
    }
    let req: CreateHeritagePackRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("Invalid heritage pack payload: {e}"),
                Some("body"),
                None,
                request_id,
            );
        }
    };
    let name = req.name.trim();
    if name.is_empty() || name.len() > 80 {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "Pack name must be 1..=80 characters",
            Some("name"),
            None,
            request_id,
        );
    }
    if !ALLOWED_ENTRY_MODES.contains(&req.entry_mode.as_str()) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "entryMode must be GREENFIELD|ADD_FEATURE|REVAMP|MICRO_POLISH|DECK_FROM_APP",
            Some("entryMode"),
            None,
            request_id,
        );
    }
    if req.theme_modes.is_empty()
        || req.theme_modes.len() > MAX_THEME_MODES
        || !req.theme_modes.iter().all(|m| is_valid_theme_mode_id(m))
    {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_REQUEST",
            "themeModes must contain 1..=6 valid mode IDs",
            Some("themeModes"),
            None,
            request_id,
        );
    }
    for (field_name, val) in [
        ("brandIdentity", &req.brand_identity),
        ("componentRegistry", &req.component_registry),
        ("presentationTheme", &req.presentation_theme),
    ] {
        if !val.is_object() {
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_REQUEST",
                &format!("{field_name} must be a JSON object"),
                Some(field_name),
                None,
                request_id,
            );
        }
    }
    let tenant_id = match resolve_tenant_for_project(
        &state,
        &req.source_project_id,
        &headers,
        verified,
        &request_id,
    ) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let pack = HeritageDesignPackDto {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.clone(),
        name: name.to_string(),
        source_project_id: req.source_project_id,
        entry_mode: req.entry_mode,
        brand_identity: req.brand_identity,
        component_registry: req.component_registry,
        presentation_theme: req.presentation_theme,
        theme_modes: req.theme_modes,
        created_at: Utc::now().timestamp_millis(),
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Storage connection unavailable",
                None,
                None,
                request_id,
            );
        }
    };
    let _ = ensure_v25_tables(&conn);
    let tx = match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(t) => t,
        Err(_) => {
            return err_resp(
                StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_BUSY",
                "Unable to open SQLite write transaction",
                None,
                None,
                request_id,
            );
        }
    };
    let ins = tx.execute(
        "INSERT INTO heritage_design_packs (
            id, tenant_id, name, source_project_id, entry_mode, brand_identity_json,
            component_registry_json, presentation_theme_json, theme_modes_json, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            pack.id,
            pack.tenant_id,
            pack.name,
            pack.source_project_id,
            pack.entry_mode,
            serde_json::to_string(&pack.brand_identity).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&pack.component_registry).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&pack.presentation_theme).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&pack.theme_modes).unwrap_or_else(|_| "[]".into()),
            pack.created_at
        ],
    );
    if ins.is_err() {
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Failed to stage heritage design pack",
            None,
            None,
            request_id,
        );
    }

    // Write jailed 0o600 file BEFORE committing SQLite transaction so jail failures never leave phantom rows
    let file_path = match write_jailed_heritage_pack_file(&pack) {
        Ok(p) => p,
        Err(e) => {
            drop(tx);
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "FS_JAIL_ERROR",
                &format!("Failed to persist jailed heritage pack file: {e}"),
                None,
                None,
                request_id,
            );
        }
    };

    if tx.commit().is_err() {
        unlink_jailed_heritage_pack_file(&pack.id);
        return err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Failed to commit heritage design pack",
            None,
            None,
            request_id,
        );
    }

    ok_resp(
        serde_json::json!({ "pack": pack, "filePath": file_path }),
        request_id,
    )
}

pub fn build_cognitive_studio_router() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/cognitive/brand-proposals",
            post(handle_post_brand_proposals),
        )
        .route(
            "/api/v1/cognitive/scan-components",
            post(handle_post_scan_components),
        )
        .route(
            "/api/v1/cognitive/fe-data-contract",
            get(handle_get_fe_data_contract),
        )
        .route(
            "/api/v1/cognitive/specs/reveal",
            post(handle_post_reveal_specs_folder),
        )
        .route(
            "/api/v1/cognitive/hidden-specs",
            get(handle_get_hidden_specs).put(handle_put_hidden_spec),
        )
        .route(
            "/api/v1/cognitive/heritage-packs",
            get(handle_get_heritage_packs).post(handle_post_heritage_pack),
        )
}
