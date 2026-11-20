// @story 06.4
// @implements AC-01, AC-02, AC-04
use axum::{
    extract::{Extension, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon_auth::VerifiedLegacyIngress;
use crate::routes::presentation::resolve_deck_scope;
use crate::routes::presentation_storage::{err_resp, read_and_verify_slide_html};
use crate::routes::DaemonAppState;

const MAX_SNAPSHOT_TAG_LEN: usize = 64;
const MAX_PRESENTER_SLIDES: usize = 64;

fn default_transport_mode() -> String {
    "broadcast_channel".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresenterHandshakeBody {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub snapshot_tag: Option<String>,
    #[serde(default = "default_transport_mode")]
    pub transport_mode: String,
    #[serde(default)]
    pub active_slide_index: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenterSlideSnapshotDto {
    pub slide_uid: String,
    pub slide_index: i64,
    pub layout_kind: String,
    pub eyebrow_tag: String,
    pub narrative_arc: String,
    pub speaker_notes: String,
    pub content_sha256: String,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenterHandshakeResp {
    pub session_id: String,
    pub deck_id: String,
    pub project_id: String,
    pub deck_title: String,
    pub aspect_ratio: String,
    pub deck_mode: String,
    pub revision: i64,
    pub active_slide_index: i64,
    pub is_historical_read_only: bool,
    pub snapshot_tag: Option<String>,
    pub issued_at_ms: i64,
    pub slides: Vec<PresenterSlideSnapshotDto>,
}

fn is_valid_snapshot_tag(tag: &str) -> bool {
    if !tag.starts_with("SNAP-") || tag.len() < 6 || tag.len() > MAX_SNAPSHOT_TAG_LEN {
        return false;
    }
    tag[5..]
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn validate_handshake_body(body: &PresenterHandshakeBody) -> Result<(), Response> {
    if !matches!(
        body.transport_mode.as_str(),
        "broadcast_channel" | "tauri_ipc" | "inline_fallback"
    ) {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "Unsupported presenter transportMode",
            false,
            None,
        ));
    }
    if body.active_slide_index < 0 {
        return Err(err_resp(
            StatusCode::BAD_REQUEST,
            "VALIDATION_FAILED",
            "activeSlideIndex must be non-negative",
            false,
            None,
        ));
    }
    if let Some(ref tag) = body.snapshot_tag {
        if !is_valid_snapshot_tag(tag) {
            return Err(err_resp(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "Invalid snapshotTag format (expected SNAP-*)",
                false,
                None,
            ));
        }
    }
    Ok(())
}

fn require_presenter_viewer_role(caller: &VerifiedLegacyIngress) -> Result<(), Response> {
    if matches!(
        caller.principal.role.as_str(),
        "viewer" | "editor" | "admin"
    ) {
        return Ok(());
    }
    Err(err_resp(
        StatusCode::FORBIDDEN,
        "FORBIDDEN",
        "Insufficient role to open presenter session",
        false,
        None,
    ))
}

fn resolve_expected_project_id(
    route_project_id: Option<&str>,
    body_project_id: Option<&str>,
    headers: &HeaderMap,
) -> Result<Option<String>, Response> {
    let header_pid = headers
        .get("x-cowok-project-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let candidates: Vec<&str> = [
        route_project_id,
        body_project_id.map(str::trim).filter(|s| !s.is_empty()),
        header_pid,
    ]
    .into_iter()
    .flatten()
    .collect();

    if let Some((&first, rest)) = candidates.split_first() {
        if rest.iter().any(|&other| other != first) {
            return Err(err_resp(
                StatusCode::NOT_FOUND,
                "E_DECK_NOT_FOUND",
                "Requested presentation deck was not found",
                false,
                None,
            ));
        }
        return Ok(Some(first.to_string()));
    }
    Ok(None)
}

fn verify_snapshot_tag_ownership(
    conn: &Connection,
    project_id: &str,
    tenant_id: &str,
    snapshot_tag: &str,
) -> Result<(), Response> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM version_snapshots
             WHERE project_id = ?1 AND tenant_id = ?2 AND snapshot_tag = ?3
             LIMIT 1",
            params![project_id, tenant_id, snapshot_tag],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying snapshot scope: {e}"),
                false,
                None,
            )
        })?;

    if found.is_none() {
        return Err(err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested presentation deck was not found",
            false,
            None,
        ));
    }
    Ok(())
}

struct DeckHeaderRow {
    title: String,
    aspect_ratio: String,
    deck_mode: String,
    revision: i64,
}

struct SlideRowWithHtml {
    dto: PresenterSlideSnapshotDto,
    html_path: String,
}

fn load_deck_header(
    conn: &Connection,
    deck_id: &str,
    project_id: &str,
) -> Result<DeckHeaderRow, Response> {
    let row: Option<DeckHeaderRow> = conn
        .query_row(
            "SELECT title, aspect_ratio, deck_mode, revision
             FROM presentation_decks
             WHERE id = ?1 AND project_id = ?2",
            params![deck_id, project_id],
            |r| {
                Ok(DeckHeaderRow {
                    title: r.get(0)?,
                    aspect_ratio: r.get(1)?,
                    deck_mode: r.get(2)?,
                    revision: r.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying deck header: {e}"),
                false,
                None,
            )
        })?;

    row.ok_or_else(|| {
        err_resp(
            StatusCode::NOT_FOUND,
            "E_DECK_NOT_FOUND",
            "Requested presentation deck was not found",
            false,
            None,
        )
    })
}

fn load_deck_slides_for_presenter(
    conn: &Connection,
    deck_id: &str,
) -> Result<Vec<SlideRowWithHtml>, Response> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(slide_uid, ''), slide_index, layout_kind, html_path,
                    content_sha256, speaker_notes, narrative_arc, eyebrow_tag, revision
             FROM deck_slides
             WHERE deck_id = ?1
             ORDER BY slide_index ASC
             LIMIT ?2",
        )
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed preparing presenter slides query: {e}"),
                false,
                None,
            )
        })?;

    let rows = stmt
        .query_map(params![deck_id, MAX_PRESENTER_SLIDES as i64], |r| {
            Ok(SlideRowWithHtml {
                dto: PresenterSlideSnapshotDto {
                    slide_uid: r.get(0)?,
                    slide_index: r.get(1)?,
                    layout_kind: r.get(2)?,
                    eyebrow_tag: r.get(7)?,
                    narrative_arc: r.get(6)?,
                    speaker_notes: r.get(5)?,
                    content_sha256: r.get(4)?,
                    revision: r.get(8)?,
                },
                html_path: r.get(3)?,
            })
        })
        .map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed querying presenter slides: {e}"),
                false,
                None,
            )
        })?;

    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| {
            err_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("Failed collecting presenter slide row: {e}"),
                false,
                None,
            )
        })?);
    }
    Ok(out)
}

pub async fn handle_presenter_handshake_direct(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    headers: HeaderMap,
    AxumPath(deck_id): AxumPath<String>,
    Json(body): Json<PresenterHandshakeBody>,
) -> Response {
    handle_presenter_handshake_inner(state, caller, headers, None, deck_id, body).await
}

pub async fn handle_presenter_handshake_project(
    State(state): State<DaemonAppState>,
    Extension(caller): Extension<VerifiedLegacyIngress>,
    headers: HeaderMap,
    AxumPath((project_id, deck_id)): AxumPath<(String, String)>,
    Json(body): Json<PresenterHandshakeBody>,
) -> Response {
    handle_presenter_handshake_inner(state, caller, headers, Some(project_id), deck_id, body).await
}

async fn handle_presenter_handshake_inner(
    state: DaemonAppState,
    caller: VerifiedLegacyIngress,
    headers: HeaderMap,
    route_project_id: Option<String>,
    deck_id: String,
    body: PresenterHandshakeBody,
) -> Response {
    if let Err(resp) = require_presenter_viewer_role(&caller) {
        return resp;
    }
    if let Err(resp) = validate_handshake_body(&body) {
        return resp;
    }
    let expected_pid = match resolve_expected_project_id(
        route_project_id.as_deref(),
        body.project_id.as_deref(),
        &headers,
    ) {
        Ok(pid) => pid,
        Err(resp) => return resp,
    };

    let scoped = match resolve_deck_scope(&state, &caller, expected_pid.as_deref(), &deck_id) {
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

    if let Some(ref tag) = body.snapshot_tag {
        if let Err(resp) =
            verify_snapshot_tag_ownership(&conn, &scoped.project_id, &scoped.tenant_id, tag)
        {
            return resp;
        }
    }

    let header = match load_deck_header(&conn, &deck_id, &scoped.project_id) {
        Ok(h) => h,
        Err(resp) => return resp,
    };
    let slide_rows = match load_deck_slides_for_presenter(&conn, &deck_id) {
        Ok(rows) => rows,
        Err(resp) => return resp,
    };

    let mut slides = Vec::with_capacity(slide_rows.len());
    for row in slide_rows {
        if let Err(resp) = read_and_verify_slide_html(
            &scoped.workspace_root,
            &row.html_path,
            &row.dto.content_sha256,
            &row.dto.slide_uid,
        ) {
            return resp;
        }
        slides.push(row.dto);
    }

    let clamped_idx = if slides.is_empty() {
        0
    } else {
        body.active_slide_index.clamp(0, (slides.len() - 1) as i64)
    };

    (
        StatusCode::OK,
        Json(PresenterHandshakeResp {
            session_id: Uuid::new_v4().to_string(),
            deck_id,
            project_id: scoped.project_id,
            deck_title: header.title,
            aspect_ratio: header.aspect_ratio,
            deck_mode: header.deck_mode,
            revision: header.revision,
            active_slide_index: clamped_idx,
            is_historical_read_only: body.snapshot_tag.is_some(),
            snapshot_tag: body.snapshot_tag,
            issued_at_ms: Utc::now().timestamp_millis().max(0),
            slides,
        }),
    )
        .into_response()
}
