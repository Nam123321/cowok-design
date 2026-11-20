// @story 06.1
// @implements AC-01, AC-02, AC-04, AC-05
use std::collections::HashMap;
use std::path::PathBuf;

use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use presentation_cms::{
    compute_sha256_hex, generate_slide_uid, is_valid_slide_uid, render_branded_slide,
    rewrite_slide_html_for_duplicate, validate_slide_protocol_structure, ProtocolValidationError,
    RenderSlideRequest, SlideLayoutKind,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use spec_registry::presentation_repo::{
    create_deck, duplicate_slide_record, list_decks, load_slide_by_uid, map_repo_error_code,
    reorder_slides, CreateDeckInput, NewSlideSeed, PresentationRepoError,
};
use uuid::Uuid;

use crate::daemon_auth::{require_editor_role, LegacyAuthKind, VerifiedLegacyIngress};
use crate::routes::presentation_storage::{
    check_and_reserve_scoped_idempotency, cleanup_staged_files, err_resp, finalize_idempotency,
    read_and_verify_slide_html, release_idempotency, resolve_constraint_tokens_css,
    stage_slide_html_file, ErrorDetails, IdemState,
};
pub use crate::routes::presentation_storage::{
    sweep_orphan_slide_artifacts, sweep_orphan_tmp_slides,
};
use crate::routes::DaemonAppState;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitialSlideInputBody {
    pub layout_kind: String,
    #[serde(default)]
    pub eyebrow_tag: String,
    #[serde(default)]
    pub narrative_arc: String,
    pub headline: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateDeckBody {
    pub title: String,
    #[serde(default = "default_theme_preset")]
    pub theme_preset: String,
    #[serde(default = "default_aspect_ratio")]
    pub aspect_ratio: String,
    #[serde(default = "default_deck_mode")]
    pub deck_mode: String,
    pub initial_slides: Vec<InitialSlideInputBody>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReorderSlidesBody {
    pub expected_revision: i64,
    pub ordered_slide_uids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicateSlideBody {
    pub expected_deck_revision: i64,
    #[serde(default)]
    pub insert_after_slide_uid: Option<String>,
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

fn map_protocol_err(err: &ProtocolValidationError, slide_uid: Option<String>) -> Response {
    match err {
        ProtocolValidationError::PayloadTooLarge { .. } => err_resp(
            StatusCode::PAYLOAD_TOO_LARGE,
            "E_PAYLOAD_TOO_LARGE",
            err.to_string(),
            false,
            None,
        ),
        ProtocolValidationError::DuplicateFfId(ff_id) => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_PROTOCOL_INVALID",
            err.to_string(),
            false,
            Some(ErrorDetails {
                expected_revision: None,
                actual_revision: None,
                slide_uid,
                duplicate_ff_id: Some(ff_id.clone()),
            }),
        ),
        _ => err_resp(
            StatusCode::UNPROCESSABLE_ENTITY,
            "E_DECK_PROTOCOL_INVALID",
            err.to_string(),
            false,
            None,
        ),
    }
}

pub(crate) fn map_repo_err(err: PresentationRepoError) -> Response {
    let (code_num, code, retryable, msg) = map_repo_error_code(&err);
    let status = StatusCode::from_u16(code_num).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let details = match err {
        PresentationRepoError::StaleRevision { expected, actual } => Some(ErrorDetails {
            expected_revision: Some(expected),
            actual_revision: Some(actual),
            slide_uid: None,
            duplicate_ff_id: None,
        }),
        _ => None,
    };
    err_resp(status, code, msg, retryable, details)
}

pub(crate) struct ScopedProject {
    pub(crate) project_id: String,
    pub(crate) tenant_id: String,
    pub(crate) workspace_root: PathBuf,
}

fn resolve_project_scope(
    state: &DaemonAppState,
    caller: &VerifiedLegacyIngress,
    project_id: &str,
) -> Result<ScopedProject, Response> {
    let conn = state.registry.pool().connection().map_err(|e| {
        err_resp(
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
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying project ownership: {e}"),
                false,
                None,
            )
        })?;

    let Some((pid, row_tenant, ws_raw)) = row else {
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested presentation deck was not found",
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
                let canon_ws = std::fs::canonicalize(&ws_root).ok();
                let verified_tok = caller.bearer_token.as_deref().is_some_and(|tok| {
                    canon_ws.as_ref().is_some_and(|root| {
                        crate::routes::legacy_specs::read_verified_project_token(root, tok)
                    })
                });
                if !verified_tok {
                    return Err(err_resp(
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
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested presentation deck was not found",
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

pub(crate) fn resolve_deck_scope(
    state: &DaemonAppState,
    caller: &VerifiedLegacyIngress,
    expected_project_id: Option<&str>,
    deck_id: &str,
) -> Result<ScopedProject, Response> {
    let conn = state.registry.pool().connection().map_err(|e| {
        err_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            format!("DB connection unavailable: {e}"),
            true,
            None,
        )
    })?;

    let deck_project: Option<String> = conn
        .query_row(
            "SELECT project_id FROM presentation_decks WHERE id = ?1",
            params![deck_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying deck ownership: {e}"),
                false,
                None,
            )
        })?;

    let Some(actual_project_id) = deck_project else {
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested presentation deck was not found",
            false,
            None,
        ));
    };

    if let Some(exp_pid) = expected_project_id {
        if exp_pid != actual_project_id {
            return Err(err_resp(
                StatusCode::NOT_FOUND,
                "E_DECK_NOT_FOUND",
                "Requested presentation deck was not found",
                false,
                None,
            ));
        }
    }

    resolve_project_scope(state, caller, &actual_project_id)
}

pub async fn handle_list_decks(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(project_id): AxumPath<String>,
) -> Response {
    let scoped = match resolve_project_scope(&state, &caller, &project_id) {
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

    let decks = match list_decks(&conn, &scoped.project_id, &scoped.tenant_id) {
        Ok(d) => d,
        Err(e) => return map_repo_err(e),
    };

    for deck in &decks {
        for slide in &deck.slides {
            if let Err(resp) = read_and_verify_slide_html(
                &scoped.workspace_root,
                &slide.html_path,
                &slide.content_sha256,
                &slide.slide_uid,
            ) {
                return resp;
            }
        }
    }

    (StatusCode::OK, Json(serde_json::json!({ "decks": decks }))).into_response()
}

pub async fn handle_create_deck(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(project_id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<CreateDeckBody>,
) -> Response {
    if let Err(resp) = require_editor_role(&caller.principal) {
        return resp;
    }
    if !matches!(body.aspect_ratio.as_str(), "16:9" | "16:10" | "Auto") {
        return err_resp(
            StatusCode::BAD_REQUEST,
            "E_ASPECT_UNSUPPORTED",
            format!("Unsupported aspect ratio '{}'", body.aspect_ratio),
            false,
            None,
        );
    }
    if !matches!(body.deck_mode.as_str(), "multi-page" | "one-page") {
        return err_resp(
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
        return err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "Unsupported themePreset",
            false,
            None,
        );
    }
    if body.title.trim().is_empty()
        || body.title.trim().len() > 200
        || body.initial_slides.is_empty()
        || body.initial_slides.len() > 32
    {
        return err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "Deck title (1..200 chars) and 1..32 initial slides are required",
            false,
            None,
        );
    }

    let scoped = match resolve_project_scope(&state, &caller, &project_id) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let mut scoped_payload =
        format!("POST:/api/projects/{}/decks\n", scoped.project_id).into_bytes();
    scoped_payload.extend_from_slice(&serde_json::to_vec(&body).unwrap_or_default());
    let idem_key = match check_and_reserve_scoped_idempotency(
        &state,
        &scoped.tenant_id,
        &caller.principal.user_id,
        &headers,
        "create_deck",
        &scoped.project_id,
        "_new",
        "_deck",
        &scoped_payload,
    ) {
        Ok(IdemState::Replay(status, json_body)) => {
            let st = StatusCode::from_u16(status).unwrap_or(StatusCode::CREATED);
            return (st, [("content-type", "application/json")], json_body).into_response();
        }
        Ok(IdemState::Proceed(k)) => Some(k),
        Ok(IdemState::None) => None,
        Err(resp) => return resp,
    };

    let deck_id = Uuid::new_v4().to_string();
    let conn_tokens = match state.registry.pool().connection() {
        Ok(c) => resolve_constraint_tokens_css(&c, &scoped.project_id, &scoped.workspace_root),
        Err(_) => String::new(),
    };

    let mut seeds = Vec::with_capacity(body.initial_slides.len());
    let mut staged_rel_paths = Vec::with_capacity(body.initial_slides.len());

    for (idx, item) in body.initial_slides.iter().enumerate() {
        let Some(layout_kind) = SlideLayoutKind::parse(&item.layout_kind) else {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return err_resp(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                format!("Unsupported slide layoutKind '{}'", item.layout_kind),
                false,
                None,
            );
        };
        if item.headline.trim().is_empty() {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return err_resp(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "Slide headline cannot be empty",
                false,
                None,
            );
        }

        let slide_uid = generate_slide_uid();
        let rendered = render_branded_slide(&RenderSlideRequest {
            slide_uid: Some(slide_uid.clone()),
            layout_kind,
            theme_preset: body.theme_preset.clone(),
            aspect_ratio: body.aspect_ratio.clone(),
            eyebrow_tag: item.eyebrow_tag.clone(),
            headline: item.headline.clone(),
            subtitle: item.narrative_arc.clone(),
            primary_tokens_css: conn_tokens.clone(),
            icon_manifest_version: "1".to_string(),
        });

        if let Err(err) = validate_slide_protocol_structure(&rendered.html) {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return map_protocol_err(&err, Some(rendered.slide_uid));
        }

        let rel_path = match stage_slide_html_file(
            &scoped.workspace_root,
            &deck_id,
            &rendered.slide_uid,
            &rendered.html,
        ) {
            Ok(p) => p,
            Err(resp) => {
                cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
                if let Some(ref k) = idem_key {
                    release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
                }
                return resp;
            }
        };
        staged_rel_paths.push(rel_path.clone());

        let slots = rendered
            .extracted_slots
            .into_iter()
            .map(|(ff_id, label, text)| (Uuid::new_v4().to_string(), ff_id, label, text))
            .collect();

        seeds.push(NewSlideSeed {
            id: Uuid::new_v4().to_string(),
            slide_uid: rendered.slide_uid,
            slide_index: idx as i64,
            layout_kind: item.layout_kind.clone(),
            html_path: rel_path,
            content_sha256: rendered.content_sha256,
            speaker_notes: String::new(),
            narrative_arc: item.narrative_arc.clone(),
            eyebrow_tag: item.eyebrow_tag.clone(),
            initial_slots: slots,
        });
    }

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let input = CreateDeckInput {
        id: deck_id.clone(),
        project_id: scoped.project_id.clone(),
        tenant_id: scoped.tenant_id.clone(),
        title: body.title.trim().to_string(),
        theme_preset: body.theme_preset.clone(),
        aspect_ratio: body.aspect_ratio.clone(),
        deck_mode: body.deck_mode.clone(),
        slides: seeds,
    };

    match create_deck(&mut conn, &input) {
        Ok(created) => {
            if let Some(ref k) = idem_key {
                if let Ok(json_str) = serde_json::to_string(&created) {
                    finalize_idempotency(
                        &state,
                        &scoped.tenant_id,
                        &caller.principal.user_id,
                        k,
                        201,
                        &json_str,
                    );
                }
            }
            (StatusCode::CREATED, Json(created)).into_response()
        }
        Err(err) => {
            cleanup_staged_files(&scoped.workspace_root, &staged_rel_paths);
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            map_repo_err(err)
        }
    }
}

pub async fn handle_reorder_slides_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath(deck_id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<ReorderSlidesBody>,
) -> Response {
    let header_pid = headers
        .get("x-cowok-project-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    reorder_slides_inner(state, caller, header_pid, deck_id, body).await
}

pub async fn handle_reorder_slides_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id)): AxumPath<(String, String)>,
    Json(body): Json<ReorderSlidesBody>,
) -> Response {
    reorder_slides_inner(state, caller, Some(project_id), deck_id, body).await
}

async fn reorder_slides_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    body: ReorderSlidesBody,
) -> Response {
    if let Err(resp) = require_editor_role(&caller.principal) {
        return resp;
    }
    if body.ordered_slide_uids.is_empty()
        || body
            .ordered_slide_uids
            .iter()
            .any(|uid| !is_valid_slide_uid(uid))
    {
        return err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "orderedSlideUids must contain valid SLD- ULIDs",
            false,
            None,
        );
    }

    let scoped = match resolve_deck_scope(&state, &caller, expected_project_id.as_deref(), &deck_id)
    {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let mut conn = match state.registry.pool().connection() {
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

    match reorder_slides(
        &mut conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        body.expected_revision,
        &body.ordered_slide_uids,
    ) {
        Ok(updated) => (StatusCode::OK, Json(updated)).into_response(),
        Err(err) => map_repo_err(err),
    }
}

pub async fn handle_duplicate_slide_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((deck_id, slide_uid)): AxumPath<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<DuplicateSlideBody>,
) -> Response {
    let header_pid = headers
        .get("x-cowok-project-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    duplicate_slide_inner(state, caller, header_pid, deck_id, slide_uid, headers, body).await
}

pub async fn handle_duplicate_slide_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    AxumPath((project_id, deck_id, slide_uid)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<DuplicateSlideBody>,
) -> Response {
    duplicate_slide_inner(
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

async fn duplicate_slide_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    expected_project_id: Option<String>,
    deck_id: String,
    slide_uid: String,
    headers: HeaderMap,
    body: DuplicateSlideBody,
) -> Response {
    if let Err(resp) = require_editor_role(&caller.principal) {
        return resp;
    }
    if !is_valid_slide_uid(&slide_uid) {
        return err_resp(
            StatusCode::BAD_REQUEST,
            "E_INVALID_SLIDE_UID",
            "Invalid slideUid format",
            false,
            None,
        );
    }

    if let Some(ref anchor_uid) = body.insert_after_slide_uid {
        if !is_valid_slide_uid(anchor_uid) {
            return err_resp(
                StatusCode::BAD_REQUEST,
                "E_INVALID_SLIDE_UID",
                "Invalid insertAfterSlideUid format",
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

    let mut scoped_payload = format!(
        "POST:/api/projects/{}/decks/{}/slides/{}/duplicate\n",
        scoped.project_id, deck_id, slide_uid
    )
    .into_bytes();
    scoped_payload.extend_from_slice(&serde_json::to_vec(&body).unwrap_or_default());
    let idem_key = match check_and_reserve_scoped_idempotency(
        &state,
        &scoped.tenant_id,
        &caller.principal.user_id,
        &headers,
        "duplicate_slide",
        &scoped.project_id,
        &deck_id,
        &slide_uid,
        &scoped_payload,
    ) {
        Ok(IdemState::Replay(status, json_body)) => {
            let st = StatusCode::from_u16(status).unwrap_or(StatusCode::CREATED);
            return (st, [("content-type", "application/json")], json_body).into_response();
        }
        Ok(IdemState::Proceed(k)) => Some(k),
        Ok(IdemState::None) => None,
        Err(resp) => return resp,
    };

    let mut conn = match state.registry.pool().connection() {
        Ok(c) => c,
        Err(e) => {
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("DB unavailable: {e}"),
                true,
                None,
            );
        }
    };

    let (actual_rev, source_slide, source_slots) = match load_slide_by_uid(
        &conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &slide_uid,
    ) {
        Ok(tup) => tup,
        Err(err) => {
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return map_repo_err(err);
        }
    };

    if actual_rev != body.expected_deck_revision {
        if let Some(ref k) = idem_key {
            release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
        }
        return err_resp(
            StatusCode::CONFLICT,
            "E_STALE_REVISION",
            format!(
                "Expected deck revision {}, actual revision {}",
                body.expected_deck_revision, actual_rev
            ),
            true,
            Some(ErrorDetails {
                expected_revision: Some(body.expected_deck_revision),
                actual_revision: Some(actual_rev),
                slide_uid: Some(slide_uid),
                duplicate_ff_id: None,
            }),
        );
    }

    let source_html = match read_and_verify_slide_html(
        &scoped.workspace_root,
        &source_slide.html_path,
        &source_slide.content_sha256,
        &source_slide.slide_uid,
    ) {
        Ok(h) => h,
        Err(resp) => {
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            return resp;
        }
    };

    let new_uid = generate_slide_uid();
    let dup_suffix = format!("dup{}", &new_uid[new_uid.len().saturating_sub(6)..]);
    let (rewritten_html, id_map) =
        match rewrite_slide_html_for_duplicate(&source_html, &new_uid, &dup_suffix) {
            Ok(tup) => tup,
            Err(err) => {
                if let Some(ref k) = idem_key {
                    release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
                }
                return map_protocol_err(&err, Some(new_uid));
            }
        };
    if let Err(err) = validate_slide_protocol_structure(&rewritten_html) {
        if let Some(ref k) = idem_key {
            release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
        }
        return map_protocol_err(&err, Some(new_uid));
    }

    let rel_path =
        match stage_slide_html_file(&scoped.workspace_root, &deck_id, &new_uid, &rewritten_html) {
            Ok(p) => p,
            Err(resp) => {
                if let Some(ref k) = idem_key {
                    release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
                }
                return resp;
            }
        };

    let new_sha = compute_sha256_hex(rewritten_html.as_bytes());
    let source_slot_map: HashMap<&str, (&str, &str, &str)> = source_slots
        .iter()
        .map(|(ff_id, label, kind, val)| {
            (
                ff_id.as_str(),
                (label.as_str(), kind.as_str(), val.as_str()),
            )
        })
        .collect();
    let copied_slots: Vec<(String, String, String, String, String)> = id_map
        .iter()
        .filter_map(|(old_ff, new_ff)| {
            source_slot_map
                .get(old_ff.as_str())
                .map(|(label, kind, val)| {
                    (
                        Uuid::new_v4().to_string(),
                        new_ff.clone(),
                        (*label).to_string(),
                        (*kind).to_string(),
                        (*val).to_string(),
                    )
                })
        })
        .collect();

    let new_seed = NewSlideSeed {
        id: Uuid::new_v4().to_string(),
        slide_uid: new_uid,
        slide_index: source_slide.slide_index + 1,
        layout_kind: source_slide.layout_kind.clone(),
        html_path: rel_path.clone(),
        content_sha256: new_sha,
        speaker_notes: source_slide.speaker_notes.clone(),
        narrative_arc: source_slide.narrative_arc.clone(),
        eyebrow_tag: source_slide.eyebrow_tag.clone(),
        initial_slots: vec![],
    };

    match duplicate_slide_record(
        &mut conn,
        &scoped.project_id,
        &scoped.tenant_id,
        &deck_id,
        &source_slide.slide_uid,
        body.insert_after_slide_uid.as_deref(),
        body.expected_deck_revision,
        &new_seed,
        &copied_slots,
    ) {
        Ok(res) => {
            if let Some(ref k) = idem_key {
                if let Ok(json_str) = serde_json::to_string(&res) {
                    finalize_idempotency(
                        &state,
                        &scoped.tenant_id,
                        &caller.principal.user_id,
                        k,
                        201,
                        &json_str,
                    );
                }
            }
            (StatusCode::CREATED, Json(res)).into_response()
        }
        Err(err) => {
            cleanup_staged_files(&scoped.workspace_root, &[rel_path]);
            if let Some(ref k) = idem_key {
                release_idempotency(&state, &scoped.tenant_id, &caller.principal.user_id, k);
            }
            map_repo_err(err)
        }
    }
}

/// Sweeps orphaned `.cowok-tmp-*` / `.tmp*` slide staging artifacts older than 60s on startup without following symlinks.
pub fn presentation_routes() -> Router<DaemonAppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/decks",
            get(handle_list_decks).post(handle_create_deck),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/reorder",
            put(handle_reorder_slides_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/reorder",
            put(handle_reorder_slides_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/duplicate",
            post(handle_duplicate_slide_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/duplicate",
            post(handle_duplicate_slide_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/slots",
            get(crate::routes::presentation_cms_routes::handle_list_slide_slots_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/slots",
            get(crate::routes::presentation_cms_routes::handle_list_slide_slots_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/slots/batch",
            post(crate::routes::presentation_cms_routes::handle_commit_slots_batch_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/slots/batch",
            post(crate::routes::presentation_cms_routes::handle_commit_slots_batch_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/apply-html",
            post(crate::routes::presentation_cms_routes::handle_apply_slide_html_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/apply-html",
            post(crate::routes::presentation_cms_routes::handle_apply_slide_html_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/copilot/dispatch",
            post(crate::routes::presentation_copilot::handle_dispatch_slide_copilot_direct),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/copilot",
            post(crate::routes::presentation_copilot::handle_dispatch_slide_copilot_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/copilot/dispatch",
            post(crate::routes::presentation_copilot::handle_dispatch_slide_copilot_project),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/copilot",
            post(crate::routes::presentation_copilot::handle_dispatch_slide_copilot_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/copilot/cancel",
            post(crate::routes::presentation_copilot::handle_cancel_slide_copilot_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/copilot/cancel",
            post(crate::routes::presentation_copilot::handle_cancel_slide_copilot_project),
        )
        .route(
            "/api/v1/decks/:deck_id/slides/:slide_uid/charts/validate",
            post(crate::routes::presentation_copilot::handle_validate_chart_spec_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/slides/:slide_uid/charts/validate",
            post(crate::routes::presentation_copilot::handle_validate_chart_spec_project),
        )
        .route(
            "/api/v1/decks/:deck_id/presenter/handshake",
            post(crate::routes::presentation_presenter::handle_presenter_handshake_direct),
        )
        .route(
            "/api/v1/projects/:id/decks/:deck_id/presenter/handshake",
            post(crate::routes::presentation_presenter::handle_presenter_handshake_project),
        )
}
