// @story 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (End-to-End Multi-Slide Deck Generation, CSV/TSV/PDF -> ChartSpecV12 Normalizer & Slide Copilot Rewrite)
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bot_fleet::{ByokGatewayRouter, DecisionCascadeClient, PostExecutionNormalizer};
use chrono::Utc;
use domain::decision::{CascadeStage, DecisionCandidate, DecisionRouteInput};
use presentation_cms::deck_generator::assemble_one_page_stageshow_html;
use presentation_cms::{
    compute_sha256_hex, count_words, enforce_slide_word_budget_and_split, generate_stageshow_deck,
    is_valid_ff_id, is_valid_slide_uid, normalize_chart_source, patch_slide_slots_lol_html,
    synthesize_slide_rewrite, ChartKind2D, ChartNormalizeOptions, ChartNormalizerError,
    ChartSourceFormat, CmsSlotKind, DraftSlideInput, GenerateDeckSpecInput, GeneratedSlideArtifact,
    NormalizedChartKind, NormalizedChartOutput, SlideLayoutKind, SlideRewriteInput, SlotPatchInput,
    StageshowFramePreset, MAX_PDF_BYTES, MAX_POINTS, MAX_SERIES,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use spec_registry::presentation_repo::{
    list_decks, load_slide_slots_snapshot, PresentationDeckRecord,
};
use spec_registry::retry_on_busy;
use uuid::Uuid;

use crate::daemon_auth::{require_editor_role, LegacyAuthKind, VerifiedLegacyIngress};
use crate::routes::chat_cas_storage::redact_secrets;
use crate::routes::fs_jail;
use crate::routes::presentation::{map_repo_err, resolve_deck_scope, ScopedProject};
use crate::routes::presentation_copilot::{
    has_active_copilot_lease, synthesize_copilot_chart_json,
};
use crate::routes::presentation_storage::{
    cleanup_staged_files, read_and_verify_slide_html, resolve_constraint_tokens_css,
    stage_slide_html_file, stage_tx_owned_slide_html_file,
};
use crate::routes::DaemonAppState;

const MAX_DECK_BRIEF_CHARS: usize = 12_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeckGenErrorInner {
    code: String,
    message: String,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    guidance: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeckGenErrorEnvelope {
    error: DeckGenErrorInner,
}

fn deck_err_resp(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
    retryable: bool,
    guidance: Option<String>,
) -> Response {
    (
        status,
        Json(DeckGenErrorEnvelope {
            error: DeckGenErrorInner {
                code: code.to_string(),
                message: message.into(),
                retryable,
                guidance,
            },
        }),
    )
        .into_response()
}

fn map_normalizer_error(err: ChartNormalizerError) -> Response {
    let status = match err {
        ChartNormalizerError::PdfTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    let guidance = err.non_tech_guidance().map(ToString::to_string);
    deck_err_resp(status, err.code(), err.to_string(), false, guidance)
}

fn resolve_scoped_project(
    state: &DaemonAppState,
    caller: &VerifiedLegacyIngress,
    project_id: &str,
) -> Result<ScopedProject, Response> {
    let conn = state.registry.pool().connection().map_err(|e| {
        deck_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("DB connection unavailable: {e}"),
            true,
            None,
        )
    })?;
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT id, tenant_id, workspace_root
             FROM projects
             WHERE id = ?1 AND trash_deleted_at IS NULL",
            params![project_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| {
            deck_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying project ownership: {e}"),
                false,
                None,
            )
        })?;
    let Some((pid, row_tenant, ws_raw)) = row else {
        return Err(deck_err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested project was not found",
            false,
            None,
        ));
    };

    let trimmed_ws = ws_raw.trim();
    let ws_root = if trimmed_ws.is_empty() || trimmed_ws == "." {
        state.registry.pool().workspace_root().to_path_buf()
    } else {
        PathBuf::from(trimmed_ws)
    };

    let authorized = match caller.kind {
        LegacyAuthKind::Bearer => {
            if caller.principal.role == "admin" {
                true
            } else if caller.principal.tenant_id.trim() != row_tenant.trim() {
                false
            } else {
                let canon_ws = fs::canonicalize(&ws_root).ok();
                let verified_tok = caller.bearer_token.as_deref().is_some_and(|tok| {
                    canon_ws.as_ref().is_some_and(|root| {
                        crate::routes::legacy_specs::read_verified_project_token(root, tok)
                    })
                });
                if !verified_tok {
                    return Err(deck_err_resp(
                        StatusCode::FORBIDDEN,
                        "TENANT_ISOLATION_BREACH",
                        "Invalid Bearer token for project scope",
                        false,
                        None,
                    ));
                }
                true
            }
        }
        LegacyAuthKind::SessionToken | LegacyAuthKind::HmacV2 => {
            caller.principal.role == "admin"
                || caller.principal.tenant_id.trim() == row_tenant.trim()
        }
    };

    if !authorized {
        return Err(deck_err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested project was not found",
            false,
            None,
        ));
    }

    Ok(ScopedProject {
        project_id: pid,
        tenant_id: row_tenant,
        workspace_root: ws_root,
    })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartSourceInputBody {
    #[serde(default)]
    pub cas_sha256: Option<String>,
    #[serde(default)]
    pub inline_content: Option<String>,
    #[serde(default)]
    pub inline_base64: Option<String>,
    #[serde(default)]
    pub source_format: Option<ChartSourceFormat>,
    #[serde(default)]
    pub title: Option<String>,
    pub chart_kind: NormalizedChartKind,
    #[serde(default = "default_max_series")]
    pub max_series: usize,
    #[serde(default = "default_max_points")]
    pub max_points: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NormalizeChartSourceRequestBody {
    pub project_id: String,
    #[serde(default)]
    pub cas_sha256: Option<String>,
    #[serde(default)]
    pub inline_content: Option<String>,
    #[serde(default)]
    pub inline_base64: Option<String>,
    #[serde(default)]
    pub source_format: Option<ChartSourceFormat>,
    #[serde(default)]
    pub title: Option<String>,
    pub chart_kind: NormalizedChartKind,
    #[serde(default = "default_max_series")]
    pub max_series: usize,
    #[serde(default = "default_max_points")]
    pub max_points: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateDeckRequestBody {
    pub project_id: String,
    pub title: String,
    pub brief: String,
    #[serde(default)]
    pub frame_preset: StageshowFramePreset,
    #[serde(default = "default_theme_preset")]
    pub theme_preset: String,
    #[serde(default = "default_aspect_ratio")]
    pub aspect_ratio: String,
    #[serde(default = "default_deck_mode")]
    pub deck_mode: String,
    #[serde(default)]
    pub chart_source: Option<ChartSourceInputBody>,
    #[serde(default = "default_true")]
    pub use_byok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedSlideSummaryDto {
    pub slide_uid: String,
    pub slide_index: usize,
    pub headline: String,
    pub word_count: usize,
    pub is_continuation: bool,
    pub frame_preset: StageshowFramePreset,
    pub has_chart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateDeckResponseBody {
    pub deck: PresentationDeckRecord,
    pub frame_preset: StageshowFramePreset,
    pub auto_split_count: usize,
    pub max_words_per_slide: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_chart: Option<NormalizedChartOutput>,
    pub generation_engine: String,
    pub slides_summary: Vec<GeneratedSlideSummaryDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopilotRewriteSlideRequestBody {
    pub project_id: String,
    pub deck_id: String,
    pub slide_uid: String,
    pub target_ff_id: String,
    pub prompt: String,
    pub expected_revision: i64,
    #[serde(default)]
    pub frame_preset: StageshowFramePreset,
    #[serde(default)]
    pub requested_chart_kind: Option<ChartKind2D>,
    #[serde(default = "default_true")]
    pub auto_split_on_overflow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationSlideDto {
    pub slide_uid: String,
    pub slide_index: i64,
    pub headline: String,
    pub word_count: usize,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotRewriteSlideResponseBody {
    pub deck_id: String,
    pub slide_uid: String,
    pub target_ff_id: String,
    pub new_revision: i64,
    pub content_sha256: String,
    pub rewritten_text: String,
    pub word_count: usize,
    pub was_condensed_or_split: bool,
    pub continuation_slides: Vec<ContinuationSlideDto>,
    pub generation_engine: String,
}

fn default_max_series() -> usize {
    MAX_SERIES
}
fn default_max_points() -> usize {
    MAX_POINTS
}
fn default_theme_preset() -> String {
    "night".to_string()
}
fn default_aspect_ratio() -> String {
    "16:9".to_string()
}
fn default_deck_mode() -> String {
    "multi-page".to_string()
}
fn default_true() -> bool {
    true
}

fn load_cas_blob_bytes(
    state: &DaemonAppState,
    tenant_id: &str,
    cas_sha256: &str,
) -> Result<(Vec<u8>, ChartSourceFormat), Response> {
    let sha_lower = cas_sha256.trim().to_ascii_lowercase();
    if sha_lower.len() != 64 || !sha_lower.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(deck_err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_CAS_SHA256",
            "casSha256 must be a valid 64-character hex SHA-256 digest",
            false,
            None,
        ));
    }

    let conn = state.registry.pool().connection().map_err(|e| {
        deck_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("DB unavailable: {e}"),
            true,
            None,
        )
    })?;
    let row: Option<(String, i64)> = conn
        .query_row(
            "SELECT file_ext, byte_size FROM cas_blobs WHERE sha256 = ?1 AND tenant_id = ?2",
            params![sha_lower, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| {
            deck_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying CAS blob: {e}"),
                false,
                None,
            )
        })?;
    let Some((file_ext, byte_size)) = row else {
        return Err(deck_err_resp(
            StatusCode::NOT_FOUND,
            "E_CAS_BLOB_NOT_FOUND",
            "Referenced CAS attachment was not found in this tenant workspace",
            false,
            None,
        ));
    };
    let Some(format) = ChartSourceFormat::from_ext(&file_ext) else {
        return Err(deck_err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_UNSUPPORTED_CHART_FORMAT",
            format!("Unsupported CAS file extension '{file_ext}'; expected csv, tsv, or pdf"),
            false,
            Some("Upload a .csv, .tsv, or text-based .pdf file.".to_string()),
        ));
    };
    if byte_size > MAX_PDF_BYTES as i64 {
        return Err(deck_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "E_PDF_TOO_LARGE",
            format!("CAS blob size {byte_size} exceeds maximum {MAX_PDF_BYTES} bytes"),
            false,
            None,
        ));
    }

    let cas_dir = state
        .registry
        .pool()
        .workspace_root()
        .join(".cowok-design/cas");
    let dir_fd = fs_jail::open_dir_abs_nofollow(&cas_dir).map_err(|_| {
        deck_err_resp(
            StatusCode::NOT_FOUND,
            "E_CAS_BLOB_MISSING",
            "CAS storage directory is missing or blocked by FS jail",
            false,
            None,
        )
    })?;
    let c_name = CString::new(sha_lower.as_bytes()).map_err(|_| {
        deck_err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_CAS_SHA256",
            "Invalid CAS blob filename bytes",
            false,
            None,
        )
    })?;
    let raw_fd = unsafe {
        libc::openat(
            dir_fd.as_raw_fd(),
            c_name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if raw_fd < 0 {
        let io_err = std::io::Error::last_os_error();
        return match io_err.raw_os_error() {
            Some(code) if code == libc::ELOOP => Err(deck_err_resp(
                StatusCode::FORBIDDEN,
                "E_FS_JAIL_VIOLATION",
                "Refused symlinked CAS blob",
                false,
                None,
            )),
            Some(code) if code == libc::ENOENT => Err(deck_err_resp(
                StatusCode::NOT_FOUND,
                "E_CAS_BLOB_MISSING",
                "CAS blob content is missing on disk",
                false,
                None,
            )),
            _ => Err(deck_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "E_CAS_READ_FAILED",
                "Unable to open CAS blob",
                true,
                None,
            )),
        };
    }
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(owned_fd.as_raw_fd(), &mut stat_buf) } != 0
        || (stat_buf.st_mode & libc::S_IFMT) != libc::S_IFREG
    {
        return Err(deck_err_resp(
            StatusCode::FORBIDDEN,
            "E_FS_JAIL_VIOLATION",
            "Refused non-regular or symlinked CAS blob",
            false,
            None,
        ));
    }
    if stat_buf.st_size < 0 || (stat_buf.st_size as usize) > MAX_PDF_BYTES {
        return Err(deck_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "E_PDF_TOO_LARGE",
            format!("CAS blob exceeds {MAX_PDF_BYTES} byte limit"),
            false,
            None,
        ));
    }

    let file = File::from(owned_fd);
    let mut buf = Vec::with_capacity((stat_buf.st_size as usize).min(MAX_PDF_BYTES));
    file.take((MAX_PDF_BYTES as u64) + 1)
        .read_to_end(&mut buf)
        .map_err(|_| {
            deck_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "E_CAS_READ_FAILED",
                "Failed reading CAS blob bytes",
                true,
                None,
            )
        })?;
    if buf.len() > MAX_PDF_BYTES {
        return Err(deck_err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "E_PDF_TOO_LARGE",
            format!("CAS blob exceeds {MAX_PDF_BYTES} byte limit"),
            false,
            None,
        ));
    }
    let actual_sha = compute_sha256_hex(&buf);
    if actual_sha != sha_lower {
        return Err(deck_err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_CAS_INTEGRITY_MISMATCH",
            "CAS blob SHA-256 verification failed",
            false,
            None,
        ));
    }

    Ok((buf, format))
}

fn resolve_and_normalize_chart(
    state: &DaemonAppState,
    tenant_id: &str,
    src: &ChartSourceInputBody,
) -> Result<NormalizedChartOutput, Response> {
    if src.max_series == 0 || src.max_series > MAX_SERIES {
        return Err(deck_err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_CHART_MAX_SERIES_EXCEEDED",
            format!("maxSeries must be between 1 and {MAX_SERIES}"),
            false,
            None,
        ));
    }
    if src.max_points == 0 || src.max_points > MAX_POINTS {
        return Err(deck_err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_CHART_MAX_POINTS_EXCEEDED",
            format!("maxPoints must be between 1 and {MAX_POINTS}"),
            false,
            None,
        ));
    }

    let (bytes, inferred_fmt) = if let Some(ref sha) = src.cas_sha256 {
        load_cas_blob_bytes(state, tenant_id, sha)?
    } else if let Some(ref b64) = src.inline_base64 {
        let decoded = BASE64_STANDARD.decode(b64.trim()).map_err(|_| {
            deck_err_resp(
                StatusCode::BAD_REQUEST,
                "E_INVALID_BASE64",
                "inlineBase64 must be valid base64 bytes",
                false,
                None,
            )
        })?;
        let fmt = src.source_format.unwrap_or_else(|| {
            if decoded.starts_with(b"%PDF-") {
                ChartSourceFormat::Pdf
            } else if decoded.contains(&b'\t') {
                ChartSourceFormat::Tsv
            } else {
                ChartSourceFormat::Csv
            }
        });
        (decoded, fmt)
    } else if let Some(ref text) = src.inline_content {
        let fmt = src.source_format.unwrap_or_else(|| {
            if text.contains('\t') && !text.contains(',') {
                ChartSourceFormat::Tsv
            } else {
                ChartSourceFormat::Csv
            }
        });
        (text.as_bytes().to_vec(), fmt)
    } else {
        return Err(deck_err_resp(
            StatusCode::BAD_REQUEST,
            "E_CHART_SOURCE_EMPTY",
            "Provide casSha256, inlineContent, or inlineBase64",
            false,
            Some("Drop a .csv, .tsv, or .pdf file or paste rows into the data box.".to_string()),
        ));
    };

    let format = src.source_format.unwrap_or(inferred_fmt);
    let opts = ChartNormalizeOptions {
        title: src.title.clone(),
        chart_kind: src.chart_kind,
        max_series: src.max_series,
        max_points: src.max_points,
    };
    normalize_chart_source(&bytes, format, &opts).map_err(map_normalizer_error)
}

async fn route_deck_decision_cascade(
    tenant_id: &str,
    project_id: &str,
    prompt: &str,
    context: &str,
    artifact_sha256: &str,
    use_byok: bool,
) -> Option<String> {
    let client = DecisionCascadeClient::new();
    let req = DecisionRouteInput {
        project_id: project_id.to_string(),
        stage: CascadeStage::Decision1,
        prompt: redact_secrets(prompt),
        context: redact_secrets(context),
        untrusted_artifact_sha256: artifact_sha256.to_string(),
        attempt_number: 1,
        prior_verdicts: Vec::new(),
        stale_file_sha256: Some(artifact_sha256.to_string()),
        current_file_sha256: Some(artifact_sha256.to_string()),
        candidates: vec![
            DecisionCandidate {
                id: "stageshow-deterministic".to_string(),
                label: "stageshow-deterministic".to_string(),
                text: "Deterministic stageshow multi-slide synthesis with 30-word budget"
                    .to_string(),
                de_score_bps: if use_byok { 7_000 } else { 9_500 },
            },
            DecisionCandidate {
                id: "bot-fleet-byok".to_string(),
                label: "bot-fleet-byok".to_string(),
                text: "BYOK LLM stageshow slide generation with failover".to_string(),
                de_score_bps: if use_byok { 9_000 } else { 3_000 },
            },
        ],
        rubric_pair: None,
        prefer_external_tier: false,
    };
    client
        .route(tenant_id, &req)
        .await
        .ok()
        .map(|v| v.verdict_label)
}

async fn try_generate_slides_via_byok(
    state: &DaemonAppState,
    tenant_id: &str,
    title: &str,
    brief: &str,
    frame_preset: StageshowFramePreset,
) -> Option<serde_json::Value> {
    let st = state.clone();
    let tid = tenant_id.to_string();
    let (creds, _) = tokio::task::spawn_blocking(move || {
        crate::routes::bot_fleet::load_tenant_byok_credentials(&st, &tid)
    })
    .await
    .ok()?;
    if creds.is_empty() {
        return None;
    }

    let safe_title = redact_secrets(title);
    let safe_brief = redact_secrets(brief);
    let prompt = format!(
        "Generate a stageshow multi-slide deck JSON for title='{safe_title}', frame='{}', brief='{safe_brief}'. \
         Return ONLY valid JSON: {{\"slides\":[{{\"eyebrow\":\"...\",\"headline\":\"<=8 words\",\"bullets\":[\"3-7 words\"],\"speakerNotes\":\"...\",\"includeChart\":false}}]}}. \
         Enforce <= 30 words per slide.",
        frame_preset.as_str()
    );
    let router = ByokGatewayRouter::new(crate::routes::bot_fleet::shared_cooldown_tracker());
    let res = router
        .execute_with_failover(tenant_id, &prompt, None, None, &creds, 15_000)
        .await
        .ok()?;
    let norm = PostExecutionNormalizer::normalize(&res.raw_content);
    norm.extracted_json
}

async fn try_rewrite_slot_via_byok(
    state: &DaemonAppState,
    tenant_id: &str,
    prompt: &str,
    current_text: &str,
    slot_kind: &str,
    frame_preset: StageshowFramePreset,
) -> Option<String> {
    let st = state.clone();
    let tid = tenant_id.to_string();
    let (creds, _) = tokio::task::spawn_blocking(move || {
        crate::routes::bot_fleet::load_tenant_byok_credentials(&st, &tid)
    })
    .await
    .ok()?;
    if creds.is_empty() {
        return None;
    }
    let safe_prompt = redact_secrets(prompt);
    let safe_current = redact_secrets(current_text);
    let full_prompt = format!(
        "Rewrite slide slot (kind='{slot_kind}', frame='{}') strictly obeying <=30 words and 3-7 words per bullet. \
         Current copy: '{safe_current}'. Directive: '{safe_prompt}'. Output only the rewritten slide copy.",
        frame_preset.as_str()
    );
    let router = ByokGatewayRouter::new(crate::routes::bot_fleet::shared_cooldown_tracker());
    let res = router
        .execute_with_failover(tenant_id, &full_prompt, None, None, &creds, 12_000)
        .await
        .ok()?;
    let norm = PostExecutionNormalizer::normalize(&res.raw_content);
    let trimmed = norm.normalized_content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `POST /api/v1/decks/normalize-chart-source`
pub async fn handle_normalize_chart_source(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    Json(body): Json<NormalizeChartSourceRequestBody>,
) -> Response {
    if let Err(resp) = require_editor_role(&caller.principal) {
        return resp;
    }
    let scoped = match resolve_scoped_project(&state, &caller, &body.project_id) {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let input = ChartSourceInputBody {
        cas_sha256: body.cas_sha256,
        inline_content: body.inline_content,
        inline_base64: body.inline_base64,
        source_format: body.source_format,
        title: body.title,
        chart_kind: body.chart_kind,
        max_series: body.max_series,
        max_points: body.max_points,
    };
    match resolve_and_normalize_chart(&state, &scoped.tenant_id, &input) {
        Ok(out) => (StatusCode::OK, Json(out)).into_response(),
        Err(resp) => resp,
    }
}

/// `POST /api/v1/decks/generate`
pub async fn handle_generate_deck(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    Json(body): Json<GenerateDeckRequestBody>,
) -> Response {
    if let Err(resp) = require_editor_role(&caller.principal) {
        return resp;
    }
    if body.brief.trim().is_empty() || body.brief.chars().count() > MAX_DECK_BRIEF_CHARS {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            format!("Deck brief must be between 1 and {MAX_DECK_BRIEF_CHARS} characters"),
            false,
            None,
        );
    }
    if !matches!(body.aspect_ratio.as_str(), "16:9" | "16:10" | "Auto") {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "E_ASPECT_UNSUPPORTED",
            format!("Unsupported aspect ratio '{}'", body.aspect_ratio),
            false,
            None,
        );
    }
    if !matches!(body.deck_mode.as_str(), "multi-page" | "one-page") {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "Unsupported deckMode",
            false,
            None,
        );
    }
    if !matches!(
        body.theme_preset.as_str(),
        "warm" | "dim" | "night" | "low-focus"
    ) {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "Unsupported themePreset",
            false,
            None,
        );
    }

    let scoped = match resolve_scoped_project(&state, &caller, &body.project_id) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let normalized_chart = match body.chart_source {
        Some(ref src) => match resolve_and_normalize_chart(&state, &scoped.tenant_id, src) {
            Ok(out) => Some(out),
            Err(resp) => return resp,
        },
        None => None,
    };

    let brief_digest = compute_sha256_hex(format!("{}\n{}", body.title, body.brief).as_bytes());
    let cascade_verdict = route_deck_decision_cascade(
        &scoped.tenant_id,
        &scoped.project_id,
        &body.title,
        &body.brief,
        &brief_digest,
        body.use_byok,
    )
    .await;

    let llm_slides_json =
        if body.use_byok && cascade_verdict.as_deref() != Some("ESCALATE_HUMAN_ADVISOR") {
            try_generate_slides_via_byok(
                &state,
                &scoped.tenant_id,
                &body.title,
                &body.brief,
                body.frame_preset,
            )
            .await
        } else {
            None
        };
    let generation_engine = if llm_slides_json.is_some() {
        "bot-fleet-byok".to_string()
    } else {
        "stageshow-deterministic".to_string()
    };

    let primary_tokens_css = match state.registry.pool().connection() {
        Ok(c) => resolve_constraint_tokens_css(&c, &scoped.project_id, &scoped.workspace_root),
        Err(_) => String::new(),
    };

    let generated = match generate_stageshow_deck(&GenerateDeckSpecInput {
        title: body.title.clone(),
        brief: body.brief.clone(),
        frame_preset: body.frame_preset,
        theme_preset: body.theme_preset.clone(),
        aspect_ratio: body.aspect_ratio.clone(),
        primary_tokens_css: primary_tokens_css.clone(),
        normalized_chart: normalized_chart.clone(),
        llm_slides_json,
    }) {
        Ok(g) => g,
        Err(e) => {
            return deck_err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "E_DECK_GENERATION_FAILED",
                e.to_string(),
                false,
                None,
            )
        }
    };

    let deck_id = Uuid::new_v4().to_string();
    let is_one_page = body.deck_mode == "one-page";
    let mut staged_rel_paths: Vec<String> = Vec::with_capacity(generated.slides.len());
    let mut staged_slides: Vec<(&GeneratedSlideArtifact, String, String)> =
        Vec::with_capacity(generated.slides.len());

    if is_one_page {
        let one_page_html = match assemble_one_page_stageshow_html(
            &generated.slides,
            generated.frame_preset,
            &generated.theme_preset,
            &generated.aspect_ratio,
            &primary_tokens_css,
        ) {
            Ok(h) => h,
            Err(e) => {
                return deck_err_resp(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "E_DECK_GENERATION_FAILED",
                    e.to_string(),
                    false,
                    None,
                )
            }
        };
        let one_page_sha = compute_sha256_hex(one_page_html.as_bytes());
        let tx_uuid = Uuid::new_v4().to_string();
        let lead_uid = generated
            .slides
            .first()
            .map(|s| s.slide_uid.as_str())
            .unwrap_or("");
        let rel_path = match stage_tx_owned_slide_html_file(
            &scoped.workspace_root,
            &deck_id,
            lead_uid,
            true,
            1,
            &tx_uuid,
            &one_page_html,
        ) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
        staged_rel_paths.push(rel_path.clone());
        for slide in &generated.slides {
            staged_slides.push((slide, rel_path.clone(), one_page_sha.clone()));
        }
    } else {
        for slide in &generated.slides {
            let rel_path = match stage_slide_html_file(
                &scoped.workspace_root,
                &deck_id,
                &slide.slide_uid,
                &slide.html,
            ) {
                Ok(p) => p,
                Err(resp) => {
                    cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
                    return resp;
                }
            };
            staged_rel_paths.push(rel_path.clone());
            staged_slides.push((slide, rel_path, slide.content_sha256.clone()));
        }
    }

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            return deck_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let now_ms = Utc::now().timestamp_millis();
    let slides_count = staged_slides.len() as i64;
    let commit_res = retry_on_busy("deck_generate_v15_tx", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO presentation_decks
             (id, project_id, title, theme_preset, aspect_ratio, deck_protocol_version, deck_mode, slides_count, revision, last_exported_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, 1, NULL, ?8)",
            params![
                deck_id,
                scoped.project_id,
                generated.title,
                generated.theme_preset,
                generated.aspect_ratio,
                body.deck_mode,
                slides_count,
                now_ms
            ],
        )?;

        for (slide, rel_path, content_sha) in &staged_slides {
            let slide_row_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO deck_slides
                 (id, deck_id, slide_uid, slide_index, layout_kind, html_path, content_sha256, speaker_notes, narrative_arc, eyebrow_tag, revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
                params![
                    slide_row_id,
                    deck_id,
                    slide.slide_uid,
                    slide.slide_index as i64,
                    slide.layout_kind.as_str(),
                    rel_path,
                    content_sha,
                    slide.speaker_notes,
                    slide.frame_preset.as_str(),
                    slide.eyebrow_tag,
                    now_ms
                ],
            )?;
            for slot in &slide.slots {
                let slot_row_id = Uuid::new_v4().to_string();
                let charts_val = slot.charts_json.as_deref().unwrap_or("[]");
                tx.execute(
                    "INSERT INTO wording_slots
                     (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}', 'copilot', 1, ?8)",
                    params![
                        slot_row_id,
                        slide_row_id,
                        slot.ff_id,
                        slot.slot_label,
                        slot.slot_kind,
                        slot.text_value,
                        charts_val,
                        now_ms
                    ],
                )?;
            }
        }
        tx.commit()
    });

    if let Err(e) = commit_res {
        cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
        return deck_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_DECK_COMMIT_FAILED",
            format!("Atomic V15 deck persistence failed: {e}"),
            true,
            None,
        );
    }

    let created_deck = match list_decks(&conn, &scoped.project_id, &scoped.tenant_id) {
        Ok(decks) => match decks.into_iter().find(|d| d.id == deck_id) {
            Some(d) => d,
            None => {
                return deck_err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "E_DECK_NOT_FOUND",
                    "Generated deck missing immediately after commit",
                    false,
                    None,
                )
            }
        },
        Err(e) => return map_repo_err(e),
    };

    let _ = state.sse_tx.send(scoped.project_id.clone());

    let slides_summary = generated
        .slides
        .iter()
        .map(|s| GeneratedSlideSummaryDto {
            slide_uid: s.slide_uid.clone(),
            slide_index: s.slide_index,
            headline: s.headline.clone(),
            word_count: s.word_count,
            is_continuation: s.is_continuation,
            frame_preset: s.frame_preset,
            has_chart: s.charts_json.is_some(),
        })
        .collect();

    (
        StatusCode::CREATED,
        Json(GenerateDeckResponseBody {
            deck: created_deck,
            frame_preset: generated.frame_preset,
            auto_split_count: generated.auto_split_count,
            max_words_per_slide: generated.max_words_per_slide,
            normalized_chart,
            generation_engine,
            slides_summary,
        }),
    )
        .into_response()
}

/// `POST /api/v1/slides/copilot/rewrite`
pub async fn handle_copilot_rewrite_slide(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    Json(body): Json<CopilotRewriteSlideRequestBody>,
) -> Response {
    if let Err(resp) = require_editor_role(&caller.principal) {
        return resp;
    }
    if !is_valid_slide_uid(&body.slide_uid) {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_SLIDE_UID",
            "Invalid slideUid format",
            false,
            None,
        );
    }
    if !is_valid_ff_id(&body.target_ff_id) {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_FF_ID",
            "Invalid targetFfId format",
            false,
            None,
        );
    }
    if body.prompt.trim().is_empty() || body.prompt.len() > 8_192 {
        return deck_err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "Prompt must be between 1 and 8192 characters",
            false,
            None,
        );
    }

    let scoped = match resolve_deck_scope(&state, &caller, Some(&body.project_id), &body.deck_id) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let (snap, primary_tokens_css, deck_theme, deck_aspect, initial_max_idx, lease_held) = {
        let conn = match state.registry.pool().connection() {
            Ok(c) => c,
            Err(e) => {
                return deck_err_resp(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    format!("DB unavailable: {e}"),
                    true,
                    None,
                )
            }
        };
        let now_check_ms = Utc::now().timestamp_millis();
        let lease_held = has_active_copilot_lease(
            &conn,
            &scoped.tenant_id,
            &scoped.project_id,
            &body.deck_id,
            &body.slide_uid,
            now_check_ms,
        );
        let snap = match load_slide_slots_snapshot(
            &conn,
            &scoped.project_id,
            &scoped.tenant_id,
            &body.deck_id,
            &body.slide_uid,
        ) {
            Ok(s) => s,
            Err(e) => return map_repo_err(e),
        };
        let (theme, aspect): (String, String) = conn
            .query_row(
                "SELECT theme_preset, aspect_ratio FROM presentation_decks WHERE id = ?1",
                params![body.deck_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or_else(|_| ("night".to_string(), "16:9".to_string()));
        let max_idx: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(slide_index), -1) FROM deck_slides WHERE deck_id = ?1",
                params![body.deck_id],
                |r| r.get(0),
            )
            .unwrap_or(-1);
        let tokens =
            resolve_constraint_tokens_css(&conn, &scoped.project_id, &scoped.workspace_root);
        (snap, tokens, theme, aspect, max_idx, lease_held)
    };

    if lease_held {
        return deck_err_resp(
            StatusCode::CONFLICT,
            "E_COPILOT_LEASE_HELD",
            "Slide holds an active Slide Copilot execution lease; wait or cancel before rewriting",
            true,
            None,
        );
    }

    let is_one_page = snap.deck_mode == "one-page";
    let current_rev = if is_one_page {
        snap.deck_revision
    } else {
        snap.slide.revision
    };
    if current_rev != body.expected_revision {
        return deck_err_resp(
            StatusCode::CONFLICT,
            "E_STALE_REVISION",
            format!(
                "Stale revision: expected {}, actual {}",
                body.expected_revision, current_rev
            ),
            false,
            None,
        );
    }

    let current_html = match read_and_verify_slide_html(
        &scoped.workspace_root,
        &snap.slide.html_path,
        &snap.slide.content_sha256,
        &snap.slide.slide_uid,
    ) {
        Ok(h) => h,
        Err(resp) => return resp,
    };

    let existing_slot = snap.slots.iter().find(|s| s.ff_id == body.target_ff_id);
    let is_chart_rewrite = body.requested_chart_kind.is_some()
        || existing_slot.is_some_and(|s| s.slot_kind == "chart_json");
    let slot_kind_str = if is_chart_rewrite {
        "chart_json".to_string()
    } else {
        existing_slot
            .map(|s| s.slot_kind.clone())
            .unwrap_or_else(|| "text".to_string())
    };
    let current_text = existing_slot
        .map(|s| s.text_value.clone())
        .unwrap_or_default();

    let cascade_verdict = route_deck_decision_cascade(
        &scoped.tenant_id,
        &scoped.project_id,
        &body.prompt,
        &current_text,
        &snap.slide.content_sha256,
        true,
    )
    .await;

    let llm_output = if cascade_verdict.as_deref() != Some("ESCALATE_HUMAN_ADVISOR") {
        try_rewrite_slot_via_byok(
            &state,
            &scoped.tenant_id,
            &body.prompt,
            &current_text,
            &slot_kind_str,
            body.frame_preset,
        )
        .await
    } else {
        None
    };
    let generation_engine = if llm_output.is_some() {
        "bot-fleet-byok".to_string()
    } else {
        "stageshow-deterministic".to_string()
    };

    let existing_other_slots_words: usize = snap
        .slots
        .iter()
        .filter(|s| s.ff_id != body.target_ff_id && s.slot_kind != "chart_json")
        .map(|s| count_words(&s.text_value))
        .sum();

    let (primary_text, primary_word_count, overflow_bullets, was_condensed_or_split, charts_json) =
        if is_chart_rewrite {
            let synthesized_chart = match synthesize_copilot_chart_json(
                &body.prompt,
                body.requested_chart_kind,
                existing_slot.map(|s| s.charts_json.as_str()),
                &snap.slots,
                llm_output.as_deref(),
            ) {
                Ok(json) => json,
                Err((code, msg)) => {
                    return deck_err_resp(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        code,
                        msg,
                        false,
                        Some(
                            "Provide CSV/TSV rows, explicit numbers in the prompt, or select a slide containing numeric KPI slots."
                                .to_string(),
                        ),
                    );
                }
            };
            let title_text = if current_text.trim().is_empty() {
                "Updated Chart".to_string()
            } else {
                current_text.clone()
            };
            let wc = count_words(&title_text).min(presentation_cms::STAGESHOW_MAX_WORDS_PER_SLIDE);
            (title_text, wc, Vec::new(), false, Some(synthesized_chart))
        } else {
            let rewritten = synthesize_slide_rewrite(&SlideRewriteInput {
                prompt: body.prompt.clone(),
                current_text,
                slot_kind: slot_kind_str.clone(),
                target_ff_id: body.target_ff_id.clone(),
                frame_preset: body.frame_preset,
                llm_output,
                existing_other_slots_words,
            });
            (
                rewritten.primary_text,
                rewritten.word_count,
                rewritten.overflow_bullets,
                rewritten.was_condensed_or_split,
                None,
            )
        };

    let cms_kind = if is_chart_rewrite {
        CmsSlotKind::ChartJson
    } else {
        CmsSlotKind::from_str_checked(&slot_kind_str).unwrap_or(CmsSlotKind::Text)
    };
    let scope_uid = if is_one_page {
        Some(body.slide_uid.as_str())
    } else {
        None
    };
    let patch_out = match patch_slide_slots_lol_html(
        &current_html,
        scope_uid,
        &[SlotPatchInput {
            ff_id: body.target_ff_id.clone(),
            slot_kind: cms_kind,
            text_value: primary_text.clone(),
            bullet_items: None,
            charts_json: charts_json.clone(),
            safe_styles_json: None,
        }],
    ) {
        Ok(o) => o,
        Err(e) => {
            return deck_err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "E_SLIDE_PATCH_FAILED",
                e.to_string(),
                false,
                None,
            )
        }
    };

    // Prepare optional atomic continuation slides if rewritten content exceeded 30 words,
    // computing exact continuation slide_index BEFORE rendering HTML so `ff-s{N}-*` matches `slide_index`.
    let mut continuation_artifacts: Vec<GeneratedSlideArtifact> = Vec::new();
    if body.auto_split_on_overflow && !overflow_bullets.is_empty() && !is_one_page {
        let next_continuation_start_idx = (initial_max_idx + 1).max(0) as usize;
        let cont_draft = DraftSlideInput {
            layout_kind: SlideLayoutKind::parse(&snap.slide.layout_kind)
                .unwrap_or(SlideLayoutKind::ProblemSolution),
            eyebrow_tag: snap.slide.eyebrow_tag.clone(),
            headline: "Continued Key Takeaways".to_string(),
            bullets: overflow_bullets,
            speaker_notes: "Auto-split continuation slide preserving <=30 word stageshow budget."
                .to_string(),
            charts_json: None,
            chart_title: None,
        };
        match enforce_slide_word_budget_and_split(
            &cont_draft,
            next_continuation_start_idx,
            body.frame_preset,
            &deck_theme,
            &deck_aspect,
            &primary_tokens_css,
        ) {
            Ok((emitted, _)) => continuation_artifacts = emitted,
            Err(e) => {
                return deck_err_resp(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "E_SLIDE_SPLIT_FAILED",
                    e.to_string(),
                    false,
                    None,
                );
            }
        }
    }

    let tx_uuid = Uuid::new_v4().to_string();
    let next_rev = current_rev + 1;
    let mut staged_rel_paths: Vec<String> = Vec::with_capacity(1 + continuation_artifacts.len());

    let primary_staged_path = match stage_tx_owned_slide_html_file(
        &scoped.workspace_root,
        &body.deck_id,
        &body.slide_uid,
        is_one_page,
        next_rev,
        &tx_uuid,
        &patch_out.patched_html,
    ) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    staged_rel_paths.push(primary_staged_path.clone());

    let mut staged_continuations: Vec<(&GeneratedSlideArtifact, String)> =
        Vec::with_capacity(continuation_artifacts.len());
    for cont in &continuation_artifacts {
        let p = match stage_slide_html_file(
            &scoped.workspace_root,
            &body.deck_id,
            &cont.slide_uid,
            &cont.html,
        ) {
            Ok(path) => path,
            Err(resp) => {
                cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
                return resp;
            }
        };
        staged_rel_paths.push(p.clone());
        staged_continuations.push((cont, p));
    }

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            return deck_err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let now_ms = Utc::now().timestamp_millis();
    let mut stale_conflict = false;
    let mut lease_conflict = false;
    let mut inserted_continuations: Vec<ContinuationSlideDto> = Vec::new();
    let slot_label = existing_slot
        .map(|s| s.slot_label.clone())
        .unwrap_or_else(|| "Rewritten Slot".to_string());
    let persisted_charts_val = charts_json
        .as_deref()
        .or_else(|| existing_slot.map(|s| s.charts_json.as_str()))
        .unwrap_or("[]");

    let commit_res = retry_on_busy("copilot_rewrite_v15_tx", || {
        stale_conflict = false;
        lease_conflict = false;
        inserted_continuations.clear();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if has_active_copilot_lease(
            &tx,
            &scoped.tenant_id,
            &scoped.project_id,
            &body.deck_id,
            &body.slide_uid,
            now_ms,
        ) {
            lease_conflict = true;
            return Ok(());
        }
        let actual_rev: i64 = if is_one_page {
            tx.query_row(
                "SELECT revision FROM presentation_decks WHERE id = ?1",
                params![body.deck_id],
                |r| r.get(0),
            )?
        } else {
            tx.query_row(
                "SELECT revision FROM deck_slides WHERE id = ?1",
                params![snap.slide.id],
                |r| r.get(0),
            )?
        };
        if actual_rev != body.expected_revision {
            stale_conflict = true;
            return Ok(());
        }

        if !staged_continuations.is_empty() {
            let actual_max_idx: i64 = tx.query_row(
                "SELECT COALESCE(MAX(slide_index), -1) FROM deck_slides WHERE deck_id = ?1",
                params![body.deck_id],
                |r| r.get(0),
            )?;
            if actual_max_idx != initial_max_idx {
                stale_conflict = true;
                return Ok(());
            }
        }

        if is_one_page {
            tx.execute(
                "UPDATE deck_slides SET html_path = ?1, content_sha256 = ?2, revision = ?3, updated_at = ?4 WHERE deck_id = ?5",
                params![
                    primary_staged_path,
                    patch_out.content_sha256,
                    next_rev,
                    now_ms,
                    body.deck_id
                ],
            )?;
            tx.execute(
                "UPDATE presentation_decks SET revision = ?1, updated_at = ?2 WHERE id = ?3",
                params![next_rev, now_ms, body.deck_id],
            )?;
        } else {
            tx.execute(
                "UPDATE deck_slides SET html_path = ?1, content_sha256 = ?2, revision = ?3, updated_at = ?4 WHERE id = ?5",
                params![
                    primary_staged_path,
                    patch_out.content_sha256,
                    next_rev,
                    now_ms,
                    snap.slide.id
                ],
            )?;
        }

        let updated_slot = tx.execute(
            "UPDATE wording_slots
             SET text_value = ?1, slot_kind = ?2, charts_json = ?3, updated_by = 'copilot', ast_revision = ast_revision + 1, updated_at = ?4
             WHERE slide_id = ?5 AND ff_id = ?6",
            params![
                primary_text,
                cms_kind.as_str(),
                persisted_charts_val,
                now_ms,
                snap.slide.id,
                body.target_ff_id
            ],
        )?;
        if updated_slot == 0 {
            tx.execute(
                "INSERT INTO wording_slots
                 (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}', 'copilot', 1, ?8)",
                params![
                    Uuid::new_v4().to_string(),
                    snap.slide.id,
                    body.target_ff_id,
                    slot_label,
                    cms_kind.as_str(),
                    primary_text,
                    persisted_charts_val,
                    now_ms
                ],
            )?;
        }

        if !staged_continuations.is_empty() {
            for (cont, rel_path) in &staged_continuations {
                let next_idx = cont.slide_index as i64;
                let cont_row_id = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO deck_slides
                     (id, deck_id, slide_uid, slide_index, layout_kind, html_path, content_sha256, speaker_notes, narrative_arc, eyebrow_tag, revision, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
                    params![
                        cont_row_id,
                        body.deck_id,
                        cont.slide_uid,
                        next_idx,
                        cont.layout_kind.as_str(),
                        rel_path,
                        cont.content_sha256,
                        cont.speaker_notes,
                        cont.frame_preset.as_str(),
                        cont.eyebrow_tag,
                        now_ms
                    ],
                )?;
                for slot in &cont.slots {
                    tx.execute(
                        "INSERT INTO wording_slots
                         (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}', 'copilot', 1, ?8)",
                        params![
                            Uuid::new_v4().to_string(),
                            cont_row_id,
                            slot.ff_id,
                            slot.slot_label,
                            slot.slot_kind,
                            slot.text_value,
                            slot.charts_json.as_deref().unwrap_or("[]"),
                            now_ms
                        ],
                    )?;
                }
                inserted_continuations.push(ContinuationSlideDto {
                    slide_uid: cont.slide_uid.clone(),
                    slide_index: next_idx,
                    headline: cont.headline.clone(),
                    word_count: cont.word_count,
                    content_sha256: cont.content_sha256.clone(),
                });
            }
            tx.execute(
                "UPDATE presentation_decks
                 SET slides_count = slides_count + ?1, revision = revision + 1, updated_at = ?2
                 WHERE id = ?3",
                params![staged_continuations.len() as i64, now_ms, body.deck_id],
            )?;
        }

        tx.commit()
    });

    if lease_conflict {
        cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
        return deck_err_resp(
            StatusCode::CONFLICT,
            "E_COPILOT_LEASE_HELD",
            "Slide holds an active Slide Copilot execution lease; wait or cancel before rewriting",
            true,
            None,
        );
    }

    if stale_conflict {
        cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
        return deck_err_resp(
            StatusCode::CONFLICT,
            "E_STALE_REVISION",
            "Slide revision changed concurrently; reload before retrying rewrite",
            true,
            None,
        );
    }

    if let Err(e) = commit_res {
        cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
        return deck_err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "E_COPILOT_REWRITE_FAILED",
            format!("Atomic slide rewrite commit failed: {e}"),
            true,
            None,
        );
    }

    let _ = state.sse_tx.send(scoped.project_id);

    (
        StatusCode::OK,
        Json(CopilotRewriteSlideResponseBody {
            deck_id: body.deck_id,
            slide_uid: body.slide_uid,
            target_ff_id: body.target_ff_id,
            new_revision: next_rev,
            content_sha256: patch_out.content_sha256,
            rewritten_text: primary_text,
            word_count: primary_word_count,
            was_condensed_or_split: was_condensed_or_split || !inserted_continuations.is_empty(),
            continuation_slides: inserted_continuations,
            generation_engine,
        }),
    )
        .into_response()
}

pub fn build_deck_generation_router() -> Router<DaemonAppState> {
    let data_upload_routes = Router::new()
        .route("/api/v1/decks/generate", post(handle_generate_deck))
        .route(
            "/api/v1/decks/normalize-chart-source",
            post(handle_normalize_chart_source),
        )
        .layer(axum::extract::DefaultBodyLimit::max(7_500_000));
    Router::new().merge(data_upload_routes).route(
        "/api/v1/slides/copilot/rewrite",
        post(handle_copilot_rewrite_slide),
    )
}
