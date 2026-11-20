// @story 06.2
// @implements AC-02, AC-04, AC-05
use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use presentation_cms::{
    extract_slide_uids_in_html, inspect_slide_html_convergence_scoped, is_valid_ff_id,
    is_valid_slide_uid, patch_slide_slots_lol_html, sanitize_slide_html_deep_scoped,
    CmsPatcherError, CmsSlotKind, SlotPatchInput,
};
use serde::{Deserialize, Serialize};
use spec_registry::presentation_repo::{
    apply_custom_slide_html_commit, commit_slide_slots_batch, list_deck_slide_uids,
    load_slide_slots_snapshot, SlotUpsertInput, WordingSlotV12,
};
use uuid::Uuid;

use crate::daemon_auth::VerifiedLegacyIngress;
use crate::routes::presentation::{map_repo_err, resolve_deck_scope};
use crate::routes::presentation_storage::{
    check_and_reserve_idempotency, cleanup_staged_files, err_resp, finalize_idempotency,
    read_and_verify_slide_html, release_idempotency, stage_tx_owned_slide_html_file, IdemState,
};
use crate::routes::DaemonAppState;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchSlotItemBody {
    pub ff_id: String,
    pub slot_kind: String,
    pub text_value: String,
    #[serde(default)]
    pub bullet_items: Option<Vec<String>>,
    #[serde(default)]
    pub charts_json: Option<String>,
    #[serde(default)]
    pub safe_styles_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchSlotsBatchBody {
    pub expected_revision: i64,
    #[serde(default)]
    pub speaker_notes: Option<String>,
    #[serde(default)]
    pub patches: Vec<PatchSlotItemBody>,
    #[serde(default)]
    pub one_page_scope_uid: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplySlideHtmlBody {
    pub expected_revision: i64,
    pub raw_html: String,
    #[serde(default = "default_resync_true")]
    pub resync_structured_slots: bool,
}

fn default_resync_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlideSlotsSnapshotResp {
    deck_id: String,
    deck_mode: String,
    slide_uid: String,
    layout_kind: String,
    revision: i64,
    content_sha256: String,
    speaker_notes: String,
    sanitized_html: String,
    custom_html_override: bool,
    unmapped_slot_ids: Vec<String>,
    slots: Vec<WordingSlotV12>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PatchSlotsBatchResp {
    slide_uid: String,
    new_revision: i64,
    content_sha256: String,
    patched_slots_count: usize,
    execution_time_micros: u64,
    slots: Vec<WordingSlotV12>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplySlideHtmlResp {
    slide_uid: String,
    new_revision: i64,
    content_sha256: String,
    sanitized_html: String,
    converged_slots_count: usize,
    custom_html_override: bool,
    unmapped_slot_ids: Vec<String>,
    slots: Vec<WordingSlotV12>,
}

fn require_editor_role(caller: &VerifiedLegacyIngress) -> Result<(), Response> {
    if matches!(caller.principal.role.as_str(), "editor" | "admin") {
        return Ok(());
    }
    Err(err_resp(
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
        "Insufficient role to mutate presentation slide slots",
        false,
        None,
    ))
}

fn extract_expected_project_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-cowok-project-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn map_cms_patcher_err(err: &CmsPatcherError) -> Response {
    match err {
        CmsPatcherError::PayloadTooLarge { .. } | CmsPatcherError::SlotTextTooLong { .. } => {
            err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "E_PAYLOAD_TOO_LARGE",
                err.to_string(),
                false,
                None,
            )
        }
        CmsPatcherError::MxssBlocked(_) | CmsPatcherError::CssTokenBlocked(_) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_MXSS_BLOCKED",
            err.to_string(),
            false,
            None,
        ),
        CmsPatcherError::InvalidFfId(_)
        | CmsPatcherError::InvalidSlideUid(_)
        | CmsPatcherError::InvalidSlotValue(_) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            err.to_string(),
            false,
            None,
        ),
        CmsPatcherError::ChartValidationFailed { code, message } => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            code,
            message.clone(),
            true,
            None,
        ),
        CmsPatcherError::RewriterFailure(_) => err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            err.to_string(),
            true,
            None,
        ),
    }
}

fn replay_response(status_u16: u16, body_json: String) -> Response {
    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::OK);
    let parsed: serde_json::Value =
        serde_json::from_str(&body_json).unwrap_or_else(|_| serde_json::json!({}));
    (status, Json(parsed)).into_response()
}

pub async fn handle_list_slide_slots_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
) -> Response {
    handle_list_slide_slots_inner(state, caller, Some(project_id), deck_id, slide_uid).await
}

fn require_expected_project_from_headers(headers: &HeaderMap) -> Result<String, Response> {
    extract_expected_project_from_headers(headers).ok_or_else(|| {
        err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested presentation deck was not found",
            false,
            None,
        )
    })
}

pub async fn handle_list_slide_slots_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    headers: HeaderMap,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
) -> Response {
    let header_pid = match require_expected_project_from_headers(&headers) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };
    handle_list_slide_slots_inner(state, caller, Some(header_pid), deck_id, slide_uid).await
}

async fn handle_list_slide_slots_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
) -> Response {
    if !is_valid_slide_uid(&slide_uid) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid slideUid format",
            false,
            None,
        );
    }

    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            )
        }
    };

    let snap = match load_slide_slots_snapshot(
        &conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
    ) {
        Ok(s) => s,
        Err(err) => return map_repo_err(err),
    };

    let html = match read_and_verify_slide_html(
        &scoped.workspace_root,
        &snap.slide.html_path,
        &snap.slide.content_sha256,
        &snap.slide.slide_uid,
    ) {
        Ok(h) => h,
        Err(resp) => return resp,
    };

    let committed_ids: Vec<String> = snap.slots.iter().map(|s| s.ff_id.clone()).collect();
    let scope_uid = if snap.deck_mode == "one-page" {
        Some(slide_uid.as_str())
    } else {
        None
    };
    let (custom_html_override, unmapped_slot_ids) =
        inspect_slide_html_convergence_scoped(&html, &committed_ids, scope_uid);
    let effective_rev = if snap.deck_mode == "one-page" {
        snap.deck_revision
    } else {
        snap.slide.revision
    };

    (
        StatusCode::OK,
        Json(SlideSlotsSnapshotResp {
            deck_id: snap.deck_id,
            deck_mode: snap.deck_mode,
            slide_uid: snap.slide.slide_uid,
            layout_kind: snap.slide.layout_kind,
            revision: effective_rev,
            content_sha256: snap.slide.content_sha256,
            speaker_notes: snap.slide.speaker_notes,
            sanitized_html: html,
            custom_html_override,
            unmapped_slot_ids,
            slots: snap.slots,
        }),
    )
        .into_response()
}

pub async fn handle_commit_slots_batch_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<PatchSlotsBatchBody>,
) -> Response {
    handle_commit_slots_batch_inner(
        state,
        caller,
        Some(project_id),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

pub async fn handle_commit_slots_batch_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<PatchSlotsBatchBody>,
) -> Response {
    let header_pid = match require_expected_project_from_headers(&headers) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };
    handle_commit_slots_batch_inner(
        state,
        caller,
        Some(header_pid),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

async fn handle_commit_slots_batch_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
    headers: HeaderMap,
    body: PatchSlotsBatchBody,
) -> Response {
    if let Err(resp) = require_editor_role(&caller) {
        return resp;
    }
    if !is_valid_slide_uid(&slide_uid) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid slideUid format",
            false,
            None,
        );
    }
    if body.expected_revision < 0 {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "expectedRevision must be non-negative",
            false,
            None,
        );
    }
    if body.patches.is_empty() && body.speaker_notes.is_none() {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "At least one slot patch or speakerNotes update is required",
            false,
            None,
        );
    }
    if body.patches.len() > 64 {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "patches exceeds maximum batch size of 64",
            false,
            None,
        );
    }
    if let Some(ref notes) = body.speaker_notes {
        if notes.chars().count() > 8_000 {
            return err_resp(
                StatusCode::PAYLOAD_TOO_LARGE,
                "E_PAYLOAD_TOO_LARGE",
                "speakerNotes exceeds 8000 characters",
                false,
                None,
            );
        }
    }

    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let body_canon = serde_json::to_string(&body).unwrap_or_default();
    let fingerprint = format!(
        "POST\n/slides/slots/batch\n{}\n{}\n{}\n{}",
        scoped.project_id, deck_id, slide_uid, body_canon
    );
    let idem_key = match check_and_reserve_idempotency(
        &state,
        &scoped.tenant_id,
        &caller.principal.user_id,
        &headers,
        fingerprint.as_bytes(),
    ) {
        Ok(IdemState::None) => None,
        Ok(IdemState::Proceed(k)) => Some(k),
        Ok(IdemState::Replay(status, resp_json)) => return replay_response(status, resp_json),
        Err(resp) => return resp,
    };

    let release_idem = |st: &DaemonAppState| {
        if let Some(ref k) = idem_key {
            release_idempotency(st, &scoped.tenant_id, &caller.principal.user_id, k);
        }
    };

    let mut patch_inputs = Vec::with_capacity(body.patches.len());
    for item in &body.patches {
        if !is_valid_ff_id(&item.ff_id) {
            release_idem(&state);
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                format!("Invalid ffId '{}'", item.ff_id),
                false,
                None,
            );
        }
        let kind = match CmsSlotKind::from_str_checked(&item.slot_kind) {
            Ok(k) => k,
            Err(e) => {
                release_idem(&state);
                return map_cms_patcher_err(&e);
            }
        };
        patch_inputs.push(SlotPatchInput {
            ff_id: item.ff_id.clone(),
            slot_kind: kind,
            text_value: item.text_value.clone(),
            bullet_items: item.bullet_items.clone(),
            charts_json: item.charts_json.clone(),
            safe_styles_json: item.safe_styles_json.clone(),
        });
    }

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            release_idem(&state);
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let snap = match load_slide_slots_snapshot(
        &conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
    ) {
        Ok(s) => s,
        Err(err) => {
            release_idem(&state);
            return map_repo_err(err);
        }
    };

    if crate::routes::presentation_copilot::has_active_copilot_lease(
        &conn,
        &scoped.tenant_id,
        &scoped.project_id,
        &deck_id,
        &slide_uid,
        chrono::Utc::now().timestamp_millis(),
    ) {
        release_idem(&state);
        return err_resp(
            StatusCode::CONFLICT,
            "E_COPILOT_LEASE_HELD",
            "Slide CMS is temporarily locked while Slide Copilot holds an active lease",
            true,
            Some(crate::routes::presentation_storage::ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid),
                duplicate_ff_id: None,
            }),
        );
    }

    let current_html = match read_and_verify_slide_html(
        &scoped.workspace_root,
        &snap.slide.html_path,
        &snap.slide.content_sha256,
        &snap.slide.slide_uid,
    ) {
        Ok(h) => h,
        Err(resp) => {
            release_idem(&state);
            return resp;
        }
    };

    let is_one_page = snap.deck_mode == "one-page";
    let effective_rev = if is_one_page {
        snap.deck_revision
    } else {
        snap.slide.revision
    };
    if effective_rev != body.expected_revision {
        release_idem(&state);
        return err_resp(
            StatusCode::CONFLICT,
            "E_STALE_REVISION",
            format!(
                "Expected deck revision {}, actual revision {}",
                body.expected_revision, effective_rev
            ),
            true,
            Some(crate::routes::presentation_storage::ErrorDetails {
                expected_revision: Some(body.expected_revision),
                actual_revision: Some(effective_rev),
                slide_uid: Some(slide_uid),
                duplicate_ff_id: None,
            }),
        );
    }

    if let Some(ref req_scope) = body.one_page_scope_uid {
        if req_scope != &slide_uid {
            release_idem(&state);
            return err_resp(
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                "onePageScopeUid must match path slideUid",
                false,
                None,
            );
        }
    }
    let scope_uid = if is_one_page {
        Some(slide_uid.as_str())
    } else {
        None
    };

    let committed_ids: Vec<String> = snap.slots.iter().map(|s| s.ff_id.clone()).collect();
    let (is_override, _) =
        inspect_slide_html_convergence_scoped(&current_html, &committed_ids, scope_uid);
    if is_override && !patch_inputs.is_empty() {
        release_idem(&state);
        return err_resp(
            StatusCode::CONFLICT,
            "E_CUSTOM_HTML_OVERRIDE_LOCK",
            "Structured slot editing is locked while custom HTML override is active; re-sync first",
            false,
            None,
        );
    }

    let patch_out = match patch_slide_slots_lol_html(&current_html, scope_uid, &patch_inputs) {
        Ok(out) => out,
        Err(err) => {
            release_idem(&state);
            return map_cms_patcher_err(&err);
        }
    };

    // Fail-closed: reject non-matching ff_id patches so phantom wording_slots rows can never cause self-lock
    if !patch_inputs.is_empty() && patch_out.patched_slots_count < patch_inputs.len() {
        release_idem(&state);
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            format!(
                "Patch targeted {} slot(s) but only {} matched in slide HTML",
                patch_inputs.len(),
                patch_out.patched_slots_count
            ),
            false,
            None,
        );
    }

    let tx_uuid = Uuid::new_v4().to_string();
    let Some(next_rev) = body.expected_revision.checked_add(1) else {
        release_idem(&state);
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "expectedRevision overflow",
            false,
            None,
        );
    };
    let staged_rel_path = if patch_inputs.is_empty() {
        // speakerNotes-only update: keep existing verified artifact path & sha256 untouched
        snap.slide.html_path.clone()
    } else {
        match stage_tx_owned_slide_html_file(
            &scoped.workspace_root,
            &deck_id,
            &slide_uid,
            is_one_page,
            next_rev,
            &tx_uuid,
            &patch_out.patched_html,
        ) {
            Ok(p) => p,
            Err(resp) => {
                release_idem(&state);
                return resp;
            }
        }
    };

    let existing_by_ff: std::collections::HashMap<&str, &WordingSlotV12> =
        snap.slots.iter().map(|s| (s.ff_id.as_str(), s)).collect();
    let slot_upserts: Vec<SlotUpsertInput> = patch_inputs
        .iter()
        .map(|p| {
            let existing = existing_by_ff.get(p.ff_id.as_str()).copied();
            SlotUpsertInput {
                ff_id: p.ff_id.clone(),
                slot_label: existing.map(|s| s.slot_label.clone()).unwrap_or_default(),
                slot_kind: p.slot_kind.as_str().to_string(),
                text_value: p.text_value.clone(),
                charts_json: p.charts_json.clone().unwrap_or_else(|| {
                    existing
                        .map(|s| s.charts_json.clone())
                        .unwrap_or_else(|| "[]".to_string())
                }),
                safe_styles_json: p.safe_styles_json.clone().unwrap_or_else(|| {
                    existing
                        .map(|s| s.safe_styles_json.clone())
                        .unwrap_or_else(|| "{}".to_string())
                }),
                updated_by: "human".to_string(),
            }
        })
        .collect();

    let html_artifact_update = if patch_inputs.is_empty() {
        None
    } else {
        Some((staged_rel_path.as_str(), patch_out.content_sha256.as_str()))
    };

    match commit_slide_slots_batch(
        &mut conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
        body.expected_revision,
        Some(&snap.slide.html_path),
        html_artifact_update,
        body.speaker_notes.as_deref(),
        &slot_upserts,
        false,
    ) {
        Ok(committed) => {
            // Superseded immutable *.r{rev}.{tx_uuid}.html artifacts are retained for concurrent readers
            // and reclaimed asynchronously by the grace-period sweeper (`sweep_orphan_slide_artifacts`).
            let _ = &committed.previous_html_path;
            let resp = PatchSlotsBatchResp {
                slide_uid,
                new_revision: committed.new_revision,
                content_sha256: patch_out.content_sha256,
                patched_slots_count: patch_out.patched_slots_count,
                execution_time_micros: patch_out.execution_time_micros,
                slots: committed.slots,
            };
            if let Some(ref k) = idem_key {
                if let Ok(json_str) = serde_json::to_string(&resp) {
                    finalize_idempotency(
                        &state,
                        &scoped.tenant_id,
                        &caller.principal.user_id,
                        k,
                        200,
                        &json_str,
                    );
                }
            }
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(err) => {
            // Rollback deletes ONLY this transaction's tx_uuid-owned file (never touches concurrent winner)
            if !patch_inputs.is_empty() {
                cleanup_staged_files(&scoped.workspace_root, &[staged_rel_path]);
            }
            release_idem(&state);
            map_repo_err(err)
        }
    }
}

pub async fn handle_apply_slide_html_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<ApplySlideHtmlBody>,
) -> Response {
    handle_apply_slide_html_inner(
        state,
        caller,
        Some(project_id),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

pub async fn handle_apply_slide_html_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<ApplySlideHtmlBody>,
) -> Response {
    let header_pid = match require_expected_project_from_headers(&headers) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };
    handle_apply_slide_html_inner(
        state,
        caller,
        Some(header_pid),
        deck_id,
        slide_uid,
        headers,
        body,
    )
    .await
}

async fn handle_apply_slide_html_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
    headers: HeaderMap,
    body: ApplySlideHtmlBody,
) -> Response {
    if let Err(resp) = require_editor_role(&caller) {
        return resp;
    }
    if !is_valid_slide_uid(&slide_uid) {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid slideUid format",
            false,
            None,
        );
    }
    if body.expected_revision < 0
        || (!body.resync_structured_slots && body.raw_html.trim().is_empty())
    {
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "expectedRevision >= 0 and non-empty rawHtml are required",
            false,
            None,
        );
    }

    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let body_canon = serde_json::to_string(&body).unwrap_or_default();
    let fingerprint = format!(
        "POST\n/slides/apply-html\n{}\n{}\n{}\n{}",
        scoped.project_id, deck_id, slide_uid, body_canon
    );
    let idem_key = match check_and_reserve_idempotency(
        &state,
        &scoped.tenant_id,
        &caller.principal.user_id,
        &headers,
        fingerprint.as_bytes(),
    ) {
        Ok(IdemState::None) => None,
        Ok(IdemState::Proceed(k)) => Some(k),
        Ok(IdemState::Replay(status, resp_json)) => return replay_response(status, resp_json),
        Err(resp) => return resp,
    };

    let release_idem = |st: &DaemonAppState| {
        if let Some(ref k) = idem_key {
            release_idempotency(st, &scoped.tenant_id, &caller.principal.user_id, k);
        }
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            release_idem(&state);
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let snap = match load_slide_slots_snapshot(
        &conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
    ) {
        Ok(s) => s,
        Err(err) => {
            release_idem(&state);
            return map_repo_err(err);
        }
    };

    if crate::routes::presentation_copilot::has_active_copilot_lease(
        &conn,
        &scoped.tenant_id,
        &scoped.project_id,
        &deck_id,
        &slide_uid,
        chrono::Utc::now().timestamp_millis(),
    ) {
        release_idem(&state);
        return err_resp(
            StatusCode::CONFLICT,
            "E_COPILOT_LEASE_HELD",
            "Slide CMS is temporarily locked while Slide Copilot holds an active lease",
            true,
            Some(crate::routes::presentation_storage::ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid: Some(slide_uid),
                duplicate_ff_id: None,
            }),
        );
    }

    // Fail-closed read verification on existing committed artifact before mutating
    let current_disk_html = match read_and_verify_slide_html(
        &scoped.workspace_root,
        &snap.slide.html_path,
        &snap.slide.content_sha256,
        &snap.slide.slide_uid,
    ) {
        Ok(h) => h,
        Err(resp) => {
            release_idem(&state);
            return resp;
        }
    };

    let is_one_page = snap.deck_mode == "one-page";
    let scope_uid = if is_one_page {
        Some(slide_uid.as_str())
    } else {
        None
    };
    let effective_raw_html = if body.raw_html.trim().is_empty() && body.resync_structured_slots {
        current_disk_html.as_str()
    } else {
        body.raw_html.as_str()
    };
    // When resync_structured_slots is true AND rawHtml is empty (explicit post-reload Resync),
    // adopt current DOM slots as the new SSOT (expected_ff_ids = empty -> clears override cleanly).
    let expected_ff_ids: Vec<String> =
        if body.raw_html.trim().is_empty() && body.resync_structured_slots {
            Vec::new()
        } else {
            snap.slots.iter().map(|s| s.ff_id.clone()).collect()
        };
    let sanitized = match sanitize_slide_html_deep_scoped(
        effective_raw_html,
        &expected_ff_ids,
        body.resync_structured_slots,
        scope_uid,
    ) {
        Ok(s) => s,
        Err(err) => {
            release_idem(&state);
            return map_cms_patcher_err(&err);
        }
    };

    let effective_rev = if is_one_page {
        snap.deck_revision
    } else {
        snap.slide.revision
    };
    if effective_rev != body.expected_revision {
        release_idem(&state);
        return err_resp(
            StatusCode::CONFLICT,
            "E_STALE_REVISION",
            format!(
                "Expected deck revision {}, actual revision {}",
                body.expected_revision, effective_rev
            ),
            true,
            Some(crate::routes::presentation_storage::ErrorDetails {
                expected_revision: Some(body.expected_revision),
                actual_revision: Some(effective_rev),
                slide_uid: Some(slide_uid),
                duplicate_ff_id: None,
            }),
        );
    }

    // Codex P1 #2 / Subagent 2 F-04: In one-page mode, reject custom HTML that drops sibling slide sections
    if is_one_page {
        let deck_slide_uids = match list_deck_slide_uids(&conn, &deck_id) {
            Ok(set) => set,
            Err(err) => {
                release_idem(&state);
                return map_repo_err(err);
            }
        };
        let present_uids = extract_slide_uids_in_html(&sanitized.sanitized_html);
        for req_uid in &deck_slide_uids {
            if !present_uids.contains(req_uid) {
                release_idem(&state);
                return err_resp(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "VALIDATION_ERROR",
                    format!(
                        "One-page deck HTML must preserve all sibling slide sections (missing data-slide-uid='{req_uid}')"
                    ),
                    false,
                    None,
                );
            }
        }
    }

    let tx_uuid = Uuid::new_v4().to_string();
    let Some(next_rev) = body.expected_revision.checked_add(1) else {
        release_idem(&state);
        return err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "expectedRevision overflow",
            false,
            None,
        );
    };
    let staged_rel_path = match stage_tx_owned_slide_html_file(
        &scoped.workspace_root,
        &deck_id,
        &slide_uid,
        is_one_page,
        next_rev,
        &tx_uuid,
        &sanitized.sanitized_html,
    ) {
        Ok(p) => p,
        Err(resp) => {
            release_idem(&state);
            return resp;
        }
    };

    let extracted_upserts: Vec<SlotUpsertInput> = sanitized
        .extracted_slots
        .iter()
        .map(|s| SlotUpsertInput {
            ff_id: s.ff_id.clone(),
            slot_label: s.slot_label.clone(),
            slot_kind: s.slot_kind.as_str().to_string(),
            text_value: s.text_value.clone(),
            charts_json: s.charts_json.clone(),
            safe_styles_json: s.safe_styles_json.clone(),
            updated_by: "apply_html".to_string(),
        })
        .collect();

    match apply_custom_slide_html_commit(
        &mut conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
        body.expected_revision,
        Some(&snap.slide.html_path),
        &staged_rel_path,
        &sanitized.content_sha256,
        body.resync_structured_slots,
        &extracted_upserts,
    ) {
        Ok(committed) => {
            let _ = &committed.previous_html_path;
            let resp = ApplySlideHtmlResp {
                slide_uid,
                new_revision: committed.new_revision,
                content_sha256: sanitized.content_sha256,
                sanitized_html: sanitized.sanitized_html,
                converged_slots_count: sanitized.converged_slots_count,
                custom_html_override: sanitized.custom_html_override,
                unmapped_slot_ids: sanitized.unmapped_slot_ids,
                slots: committed.slots,
            };
            if let Some(ref k) = idem_key {
                if let Ok(json_str) = serde_json::to_string(&resp) {
                    finalize_idempotency(
                        &state,
                        &scoped.tenant_id,
                        &caller.principal.user_id,
                        k,
                        200,
                        &json_str,
                    );
                }
            }
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(err) => {
            cleanup_staged_files(&scoped.workspace_root, &[staged_rel_path]);
            release_idem(&state);
            map_repo_err(err)
        }
    }
}
