// @story 06.1
// @implements AC-02, AC-05
use std::collections::HashSet;
use std::sync::atomic::{AtomicU16, Ordering};

use chrono::Utc;
use domain::HarnessError;
use rusqlite::{ffi, params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;
use uuid::Uuid;

use crate::errors::{map_sqlite_error, retry_on_busy};

const CROCKFORD_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const MAX_ULID_COLLISION_RETRIES: usize = 5;
static ULID_SEQ_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Generates a canonical 30-char slide UID matching `^SLD-[0-9A-HJKMNP-TV-Z]{26}$`
/// (48-bit Unix ms timestamp + 80-bit random/monotonic entropy encoded in Crockford Base32).
pub fn generate_slide_uid() -> String {
    let ts_ms = (Utc::now().timestamp_millis().max(0)) as u64;
    let uuid_bytes = *Uuid::new_v4().as_bytes();
    let seq = ULID_SEQ_COUNTER.fetch_add(1, Ordering::Relaxed);

    let mut raw = [0u8; 16];
    let ts_be = ts_ms.to_be_bytes();
    raw[0..6].copy_from_slice(&ts_be[2..8]);
    raw[6..16].copy_from_slice(&uuid_bytes[6..16]);
    raw[14] ^= (seq >> 8) as u8;
    raw[15] ^= (seq & 0xFF) as u8;

    encode_ulid_prefixed(&raw)
}

pub fn is_valid_slide_uid(value: &str) -> bool {
    value.len() == 30
        && value
            .strip_prefix("SLD-")
            .is_some_and(|rest| rest.bytes().all(|b| CROCKFORD_ALPHABET.contains(&b)))
}

fn encode_ulid_prefixed(bytes: &[u8; 16]) -> String {
    let val = u128::from_be_bytes(*bytes);
    let mut chars = [0u8; 26];
    for (idx, slot) in chars.iter_mut().enumerate() {
        let shift = (25 - idx) * 5;
        let digit = ((val >> shift) & 0x1F) as usize;
        *slot = CROCKFORD_ALPHABET[digit];
    }
    let suffix = std::str::from_utf8(&chars).unwrap_or("00000000000000000000000000");
    format!("SLD-{suffix}")
}

fn is_unique_constraint_err(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(ffi_err, _)
            if ffi_err.extended_code == ffi::SQLITE_CONSTRAINT_UNIQUE
                || ffi_err.extended_code == ffi::SQLITE_CONSTRAINT_PRIMARYKEY
    )
}

/// Deterministic Rust Crockford Base32 ULID backfill with finite collision retry (<= 5).
pub fn backfill_missing_slide_uids(tx: &Connection) -> Result<usize, HarnessError> {
    let mut stmt = tx
        .prepare(
            "SELECT id FROM deck_slides WHERE slide_uid IS NULL ORDER BY updated_at ASC, id ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_backfill_slide_uids"))?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "query_backfill_slide_uids"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "collect_backfill_slide_uids"))?;
    drop(stmt);

    for slide_id in &ids {
        assign_unique_slide_uid_with_retry(
            tx,
            slide_id,
            MAX_ULID_COLLISION_RETRIES,
            generate_slide_uid,
        )?;
    }

    let remaining_nulls: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM deck_slides WHERE slide_uid IS NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|e| map_sqlite_error(e, "verify_backfill_slide_uids_zero_nulls"))?;
    if remaining_nulls != 0 {
        return Err(HarnessError::ValidationFailed {
            reason: format!("V12_BACKFILL_INVARIANT_FAILED: {remaining_nulls} deck_slides still have NULL slide_uid"),
        });
    }
    Ok(ids.len())
}

pub fn assign_unique_slide_uid_with_retry<F>(
    conn: &Connection,
    slide_id: &str,
    max_retries: usize,
    mut uid_gen: F,
) -> Result<String, HarnessError>
where
    F: FnMut() -> String,
{
    for _ in 0..max_retries.max(1) {
        let candidate = uid_gen();
        match conn.execute(
            "UPDATE deck_slides SET slide_uid = ?1 WHERE id = ?2 AND slide_uid IS NULL",
            params![candidate, slide_id],
        ) {
            Ok(_) => return Ok(candidate),
            Err(err) if is_unique_constraint_err(&err) => continue,
            Err(err) => return Err(map_sqlite_error(err, "update_backfill_slide_uid")),
        }
    }
    Err(HarnessError::ValidationFailed {
        reason: "E_ULID_ENTROPY_EXHAUSTED: exceeded max ULID collision retries".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(export, export_to = "../../../packages/shared-types/src/generated/")
)]
pub struct WordingSlotV12 {
    pub id: String,
    pub slide_id: String,
    pub ff_id: String,
    pub slot_label: String,
    pub slot_kind: String,
    pub text_value: String,
    pub charts_json: String,
    pub safe_styles_json: String,
    pub updated_by: String,
    pub ast_revision: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        rename = "DeckSlideV12",
        export,
        export_to = "../../../packages/shared-types/src/generated/"
    )
)]
pub struct DeckSlideRecord {
    pub id: String,
    pub deck_id: String,
    pub slide_uid: String,
    pub slide_index: i64,
    pub layout_kind: String,
    pub html_path: String,
    pub content_sha256: String,
    pub speaker_notes: String,
    pub narrative_arc: String,
    pub eyebrow_tag: String,
    pub revision: i64,
    pub updated_at: i64,
}

pub type DeckSlideV12 = DeckSlideRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(
    feature = "ts",
    ts(
        rename = "PresentationDeckV12",
        export,
        export_to = "../../../packages/shared-types/src/generated/"
    )
)]
pub struct PresentationDeckRecord {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub theme_preset: String,
    pub aspect_ratio: String,
    pub deck_protocol_version: i64,
    pub deck_mode: String,
    pub slides_count: i64,
    pub revision: i64,
    pub last_exported_at: Option<i64>,
    pub updated_at: i64,
    pub slides: Vec<DeckSlideRecord>,
}

pub type PresentationDeckV12 = PresentationDeckRecord;

#[derive(Debug, Clone)]
pub struct NewSlideSeed {
    pub id: String,
    pub slide_uid: String,
    pub slide_index: i64,
    pub layout_kind: String,
    pub html_path: String,
    pub content_sha256: String,
    pub speaker_notes: String,
    pub narrative_arc: String,
    pub eyebrow_tag: String,
    pub initial_slots: Vec<(String, String, String, String)>,
}

#[derive(Debug, Clone)]
pub struct CreateDeckInput {
    pub id: String,
    pub project_id: String,
    pub tenant_id: String,
    pub title: String,
    pub theme_preset: String,
    pub aspect_ratio: String,
    pub deck_mode: String,
    pub slides: Vec<NewSlideSeed>,
}

#[derive(Debug, Clone)]
pub enum PresentationRepoError {
    NotFound(String),
    StaleRevision { expected: i64, actual: i64 },
    SlideIndexConflict(String),
    CopilotLeaseHeld(String),
    UlidExhausted(String),
    ForeignKeyInvalid(String),
    SqliteBusy(String),
    Internal(HarnessError),
}

impl From<HarnessError> for PresentationRepoError {
    fn from(value: HarnessError) -> Self {
        Self::Internal(value)
    }
}

fn verify_project_ownership(
    conn: &Connection,
    project_id: &str,
    tenant_id: &str,
) -> Result<bool, HarnessError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
            params![project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "verify_project_ownership"))?;
    Ok(exists.is_some())
}

fn load_slides_for_deck(
    conn: &Connection,
    deck_id: &str,
) -> Result<Vec<DeckSlideRecord>, HarnessError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, deck_id, COALESCE(slide_uid, ''), slide_index, layout_kind,
                    html_path, content_sha256, speaker_notes, narrative_arc, eyebrow_tag,
                    revision, updated_at
             FROM deck_slides
             WHERE deck_id = ?1
             ORDER BY slide_index ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_load_slides_for_deck"))?;
    let rows = stmt
        .query_map(params![deck_id], |r| {
            Ok(DeckSlideRecord {
                id: r.get(0)?,
                deck_id: r.get(1)?,
                slide_uid: r.get(2)?,
                slide_index: r.get(3)?,
                layout_kind: r.get(4)?,
                html_path: r.get(5)?,
                content_sha256: r.get(6)?,
                speaker_notes: r.get(7)?,
                narrative_arc: r.get(8)?,
                eyebrow_tag: r.get(9)?,
                revision: r.get(10)?,
                updated_at: r.get(11)?,
            })
        })
        .map_err(|e| map_sqlite_error(e, "query_load_slides_for_deck"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| map_sqlite_error(e, "collect_load_slides_for_deck"))?);
    }
    Ok(out)
}

pub fn list_decks(
    conn: &Connection,
    project_id: &str,
    tenant_id: &str,
) -> Result<Vec<PresentationDeckRecord>, PresentationRepoError> {
    if !verify_project_ownership(conn, project_id, tenant_id)? {
        return Err(PresentationRepoError::NotFound(
            "E_PROJECT_NOT_FOUND".to_string(),
        ));
    }
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.project_id, d.title, d.theme_preset, d.aspect_ratio,
                    d.deck_protocol_version, d.deck_mode, d.slides_count, d.revision,
                    d.last_exported_at, d.updated_at
             FROM presentation_decks d
             JOIN projects p ON p.id = d.project_id
             WHERE d.project_id = ?1 AND p.tenant_id = ?2 AND p.trash_deleted_at IS NULL
             ORDER BY d.updated_at DESC, d.id ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_list_decks"))?;
    let rows = stmt
        .query_map(params![project_id, tenant_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, Option<i64>>(9)?,
                r.get::<_, i64>(10)?,
            ))
        })
        .map_err(|e| map_sqlite_error(e, "query_list_decks"))?;

    let mut decks = Vec::new();
    for row in rows {
        let (
            id,
            proj_id,
            title,
            theme_preset,
            aspect_ratio,
            deck_protocol_version,
            deck_mode,
            slides_count,
            revision,
            last_exported_at,
            updated_at,
        ) = row.map_err(|e| map_sqlite_error(e, "collect_list_decks"))?;
        let slides = load_slides_for_deck(conn, &id)?;
        decks.push(PresentationDeckRecord {
            id,
            project_id: proj_id,
            title,
            theme_preset,
            aspect_ratio,
            deck_protocol_version,
            deck_mode,
            slides_count,
            revision,
            last_exported_at,
            updated_at,
            slides,
        });
    }
    Ok(decks)
}

pub fn create_deck(
    conn: &mut Connection,
    input: &CreateDeckInput,
) -> Result<PresentationDeckRecord, PresentationRepoError> {
    let now = Utc::now().timestamp_millis();
    retry_on_busy("presentation_create_deck", || {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let owner_ok: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM projects WHERE id = ?1 AND tenant_id = ?2 AND trash_deleted_at IS NULL",
                params![input.project_id, input.tenant_id],
                |r| r.get(0),
            )
            .optional()?;
        if owner_ok.is_none() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        let slides_count = input.slides.len() as i64;
        tx.execute(
            "INSERT INTO presentation_decks
             (id, project_id, title, theme_preset, aspect_ratio, deck_protocol_version, deck_mode, slides_count, revision, last_exported_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, 1, NULL, ?8)",
            params![
                input.id,
                input.project_id,
                input.title,
                input.theme_preset,
                input.aspect_ratio,
                input.deck_mode,
                slides_count,
                now
            ],
        )?;

        for slide in &input.slides {
            tx.execute(
                "INSERT INTO deck_slides
                 (id, deck_id, slide_uid, slide_index, layout_kind, html_path, content_sha256, speaker_notes, narrative_arc, eyebrow_tag, revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
                params![
                    slide.id,
                    input.id,
                    slide.slide_uid,
                    slide.slide_index,
                    slide.layout_kind,
                    slide.html_path,
                    slide.content_sha256,
                    slide.speaker_notes,
                    slide.narrative_arc,
                    slide.eyebrow_tag,
                    now
                ],
            )?;
            for (slot_id, ff_id, label, value) in &slide.initial_slots {
                let inferred_kind = if ff_id.ends_with("-headline") {
                    "heading"
                } else if ff_id.starts_with("ff-kpi-") || ff_id == "ff-hero-card" {
                    "metric"
                } else {
                    "text"
                };
                tx.execute(
                    "INSERT INTO wording_slots
                     (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, '[]', '{}', 'human', 1, ?7)",
                    params![slot_id, slide.id, ff_id, label, inferred_kind, value, now],
                )?;
            }
        }
        tx.commit()
    })
    .map_err(|e| translate_harness_to_repo_err(e, "create_deck"))?;

    let decks = list_decks(conn, &input.project_id, &input.tenant_id)?;
    decks
        .into_iter()
        .find(|d| d.id == input.id)
        .ok_or_else(|| PresentationRepoError::NotFound("E_DECK_NOT_FOUND".to_string()))
}

pub fn reorder_slides(
    conn: &mut Connection,
    project_id: &str,
    tenant_id: &str,
    deck_id: &str,
    expected_revision: i64,
    ordered_slide_uids: &[String],
) -> Result<PresentationDeckRecord, PresentationRepoError> {
    let now = Utc::now().timestamp_millis();
    let mut seen_uids = HashSet::with_capacity(ordered_slide_uids.len());
    for uid in ordered_slide_uids {
        if !seen_uids.insert(uid.as_str()) {
            return Err(PresentationRepoError::SlideIndexConflict(format!(
                "duplicate slideUid '{uid}' in orderedSlideUids"
            )));
        }
    }

    let mut domain_err: Option<PresentationRepoError> = None;
    let mut committed_deck: Option<PresentationDeckRecord> = None;
    retry_on_busy("reorder_slides_tx", || {
        domain_err = None;
        committed_deck = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let deck_meta = tx
            .query_row(
                "SELECT d.title, d.theme_preset, d.aspect_ratio, d.deck_protocol_version, d.deck_mode, d.slides_count, d.revision, d.last_exported_at
                 FROM presentation_decks d
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1 AND d.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL",
                params![deck_id, project_id, tenant_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, Option<i64>>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((title, theme_preset, aspect_ratio, deck_protocol_version, deck_mode, slides_count, actual_rev, last_exported_at)) = deck_meta else {
            domain_err = Some(PresentationRepoError::NotFound("E_DECK_NOT_FOUND".to_string()));
            return Ok(());
        };
        if actual_rev != expected_revision {
            domain_err = Some(PresentationRepoError::StaleRevision { expected: expected_revision, actual: actual_rev });
            return Ok(());
        }

        let existing_slides = match load_slides_for_deck(&tx, deck_id) {
            Ok(s) => s,
            Err(e) => {
                domain_err = Some(PresentationRepoError::Internal(e));
                return Ok(());
            }
        };
        if existing_slides.len() != ordered_slide_uids.len() {
            domain_err = Some(PresentationRepoError::SlideIndexConflict(
                "orderedSlideUids count does not match deck slides".to_string(),
            ));
            return Ok(());
        }

        // Two-step positive high-offset swap (1_000_000 + idx) satisfies CHECK(slide_index >= 0) and UNIQUE(deck_id, slide_index)
        for (idx, uid) in ordered_slide_uids.iter().enumerate() {
            let temp_idx = 1_000_000 + (idx as i64);
            let updated = tx.execute(
                "UPDATE deck_slides SET slide_index = ?1, updated_at = ?2 WHERE deck_id = ?3 AND slide_uid = ?4",
                params![temp_idx, now, deck_id, uid],
            )?;
            if updated != 1 {
                domain_err = Some(PresentationRepoError::NotFound(format!("SLIDE_NOT_FOUND:{uid}")));
                return Ok(());
            }
        }
        for (idx, uid) in ordered_slide_uids.iter().enumerate() {
            tx.execute(
                "UPDATE deck_slides SET slide_index = ?1, revision = revision + 1, updated_at = ?2 WHERE deck_id = ?3 AND slide_uid = ?4",
                params![idx as i64, now, deck_id, uid],
            )?;
        }

        let deck_updated = tx.execute(
            "UPDATE presentation_decks SET revision = revision + 1, updated_at = ?1 WHERE id = ?2 AND project_id = ?3 AND revision = ?4",
            params![now, deck_id, project_id, expected_revision],
        )?;
        if deck_updated != 1 {
            let latest_rev: i64 = tx.query_row(
                "SELECT revision FROM presentation_decks WHERE id = ?1 AND project_id = ?2",
                params![deck_id, project_id],
                |r| r.get(0),
            )?;
            domain_err = Some(PresentationRepoError::StaleRevision { expected: expected_revision, actual: latest_rev });
            return Ok(());
        }

        let slides = match load_slides_for_deck(&tx, deck_id) {
            Ok(s) => s,
            Err(e) => {
                domain_err = Some(PresentationRepoError::Internal(e));
                return Ok(());
            }
        };
        tx.commit()?;
        committed_deck = Some(PresentationDeckRecord {
            id: deck_id.to_string(),
            project_id: project_id.to_string(),
            title,
            theme_preset,
            aspect_ratio,
            deck_protocol_version,
            deck_mode,
            slides_count,
            revision: expected_revision + 1,
            last_exported_at,
            updated_at: now,
            slides,
        });
        Ok(())
    })
    .map_err(|e| translate_harness_to_repo_err(e, "reorder_slides"))?;

    if let Some(err) = domain_err {
        return Err(err);
    }
    committed_deck.ok_or_else(|| PresentationRepoError::NotFound("E_DECK_NOT_FOUND".to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateSlideResult {
    pub deck_revision: i64,
    pub slide: DeckSlideRecord,
}

#[allow(clippy::too_many_arguments)]
pub fn duplicate_slide_record(
    conn: &mut Connection,
    project_id: &str,
    tenant_id: &str,
    deck_id: &str,
    source_slide_uid: &str,
    insert_after_slide_uid: Option<&str>,
    expected_deck_revision: i64,
    new_slide: &NewSlideSeed,
    copied_slots: &[(String, String, String, String, String)],
) -> Result<DuplicateSlideResult, PresentationRepoError> {
    let now = Utc::now().timestamp_millis();
    let mut domain_err: Option<PresentationRepoError> = None;
    let mut committed: Option<DuplicateSlideResult> = None;
    retry_on_busy("duplicate_slide_record_tx", || {
        domain_err = None;
        committed = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let deck_row: Option<(i64,)> = tx
            .query_row(
                "SELECT d.revision
                 FROM presentation_decks d
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1 AND d.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL",
                params![deck_id, project_id, tenant_id],
                |r| Ok((r.get(0)?,)),
            )
            .optional()?;
        let Some((actual_rev,)) = deck_row else {
            domain_err = Some(PresentationRepoError::NotFound("E_DECK_NOT_FOUND".to_string()));
            return Ok(());
        };
        if actual_rev != expected_deck_revision {
            domain_err = Some(PresentationRepoError::StaleRevision {
                expected: expected_deck_revision,
                actual: actual_rev,
            });
            return Ok(());
        }

        let source_exists: Option<i64> = tx
            .query_row(
                "SELECT slide_index FROM deck_slides WHERE deck_id = ?1 AND slide_uid = ?2",
                params![deck_id, source_slide_uid],
                |r| r.get(0),
            )
            .optional()?;
        let Some(source_idx) = source_exists else {
            domain_err = Some(PresentationRepoError::NotFound("E_SLIDE_NOT_FOUND".to_string()));
            return Ok(());
        };

        let insert_after_idx = if let Some(anchor_uid) = insert_after_slide_uid {
            let anchor_idx: Option<i64> = tx
                .query_row(
                    "SELECT slide_index FROM deck_slides WHERE deck_id = ?1 AND slide_uid = ?2",
                    params![deck_id, anchor_uid],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(resolved_anchor) = anchor_idx else {
                domain_err = Some(PresentationRepoError::NotFound("E_SLIDE_NOT_FOUND".to_string()));
                return Ok(());
            };
            resolved_anchor
        } else {
            source_idx
        };

        // Shift slides after insert_after_idx using positive temp offset to satisfy CHECK(slide_index >= 0) and UNIQUE(deck_id, slide_index)
        let mut after_stmt = tx.prepare(
            "SELECT id, slide_index FROM deck_slides WHERE deck_id = ?1 AND slide_index > ?2 ORDER BY slide_index DESC",
        )?;
        let after_rows: Vec<(String, i64)> = after_stmt
            .query_map(params![deck_id, insert_after_idx], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(after_stmt);

        for (sid, sidx) in &after_rows {
            tx.execute(
                "UPDATE deck_slides SET slide_index = ?1 WHERE id = ?2",
                params![1_000_000 + sidx + 1, sid],
            )?;
        }
        for (sid, sidx) in &after_rows {
            tx.execute(
                "UPDATE deck_slides SET slide_index = ?1, updated_at = ?2 WHERE id = ?3",
                params![sidx + 1, now, sid],
            )?;
        }

        let new_idx = insert_after_idx + 1;
        tx.execute(
            "INSERT INTO deck_slides
             (id, deck_id, slide_uid, slide_index, layout_kind, html_path, content_sha256, speaker_notes, narrative_arc, eyebrow_tag, revision, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
            params![
                new_slide.id,
                deck_id,
                new_slide.slide_uid,
                new_idx,
                new_slide.layout_kind,
                new_slide.html_path,
                new_slide.content_sha256,
                new_slide.speaker_notes,
                new_slide.narrative_arc,
                new_slide.eyebrow_tag,
                now
            ],
        )?;

        for (slot_id, ff_id, label, slot_kind, text_val) in copied_slots {
            tx.execute(
                "INSERT INTO wording_slots
                 (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, '[]', '{}', 'human', 1, ?7)",
                params![slot_id, new_slide.id, ff_id, label, slot_kind, text_val, now],
            )?;
        }

        let deck_updated = tx.execute(
            "UPDATE presentation_decks SET slides_count = slides_count + 1, revision = revision + 1, updated_at = ?1 WHERE id = ?2 AND project_id = ?3 AND revision = ?4",
            params![now, deck_id, project_id, expected_deck_revision],
        )?;
        if deck_updated != 1 {
            let latest_rev: i64 = tx.query_row(
                "SELECT revision FROM presentation_decks WHERE id = ?1 AND project_id = ?2",
                params![deck_id, project_id],
                |r| r.get(0),
            )?;
            domain_err = Some(PresentationRepoError::StaleRevision {
                expected: expected_deck_revision,
                actual: latest_rev,
            });
            return Ok(());
        }

        let slide_opt = match load_slides_for_deck(&tx, deck_id) {
            Ok(list) => list.into_iter().find(|s| s.id == new_slide.id),
            Err(e) => {
                domain_err = Some(PresentationRepoError::Internal(e));
                return Ok(());
            }
        };
        let Some(slide) = slide_opt else {
            domain_err = Some(PresentationRepoError::NotFound("E_SLIDE_NOT_FOUND".to_string()));
            return Ok(());
        };
        tx.commit()?;
        committed = Some(DuplicateSlideResult {
            deck_revision: expected_deck_revision + 1,
            slide,
        });
        Ok(())
    })
    .map_err(|e| translate_harness_to_repo_err(e, "duplicate_slide_record"))?;

    if let Some(err) = domain_err {
        return Err(err);
    }
    committed.ok_or_else(|| PresentationRepoError::NotFound("E_SLIDE_NOT_FOUND".to_string()))
}

pub type LoadedSlideTuple = (i64, DeckSlideRecord, Vec<(String, String, String, String)>);

pub fn load_slide_by_uid(
    conn: &Connection,
    project_id: &str,
    tenant_id: &str,
    deck_id: &str,
    slide_uid: &str,
) -> Result<LoadedSlideTuple, PresentationRepoError> {
    let deck_rev: Option<i64> = conn
        .query_row(
            "SELECT d.revision
             FROM presentation_decks d
             JOIN projects p ON p.id = d.project_id
             WHERE d.id = ?1 AND d.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL",
            params![deck_id, project_id, tenant_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "select_slide_deck_owner"))?;
    let Some(rev) = deck_rev else {
        return Err(PresentationRepoError::NotFound(
            "E_DECK_NOT_FOUND".to_string(),
        ));
    };

    let slides = load_slides_for_deck(conn, deck_id)?;
    let Some(slide) = slides.into_iter().find(|s| s.slide_uid == slide_uid) else {
        return Err(PresentationRepoError::NotFound(
            "E_SLIDE_NOT_FOUND".to_string(),
        ));
    };

    let mut slot_stmt = conn
        .prepare(
            "SELECT ff_id, slot_label, slot_kind, text_value
             FROM wording_slots WHERE slide_id = ?1 ORDER BY id ASC",
        )
        .map_err(|e| map_sqlite_error(e, "prepare_load_slide_slots"))?;
    let slots = slot_stmt
        .query_map(params![slide.id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| map_sqlite_error(e, "query_load_slide_slots"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "collect_load_slide_slots"))?;
    Ok((rev, slide, slots))
}

#[derive(Debug, Clone)]
pub struct SlideSlotsSnapshot {
    pub deck_id: String,
    pub deck_mode: String,
    pub deck_revision: i64,
    pub slide: DeckSlideRecord,
    pub slots: Vec<WordingSlotV12>,
}

#[derive(Debug, Clone)]
pub struct SlotUpsertInput {
    pub ff_id: String,
    pub slot_label: String,
    pub slot_kind: String,
    pub text_value: String,
    pub charts_json: String,
    pub safe_styles_json: String,
    pub updated_by: String,
}

#[derive(Debug, Clone)]
pub struct CommitSlideMutationResult {
    pub new_revision: i64,
    pub previous_html_path: String,
    pub slots: Vec<WordingSlotV12>,
}

fn load_wording_slots_v12(
    conn: &Connection,
    slide_id: &str,
) -> Result<Vec<WordingSlotV12>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at
         FROM wording_slots
         WHERE slide_id = ?1
         ORDER BY ff_id ASC, id ASC",
    )?;
    let rows = stmt
        .query_map(params![slide_id], |r| {
            Ok(WordingSlotV12 {
                id: r.get(0)?,
                slide_id: r.get(1)?,
                ff_id: r.get(2)?,
                slot_label: r.get(3)?,
                slot_kind: r.get(4)?,
                text_value: r.get(5)?,
                charts_json: r.get(6)?,
                safe_styles_json: r.get(7)?,
                updated_by: r.get(8)?,
                ast_revision: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn load_slide_slots_snapshot(
    conn: &Connection,
    project_id: &str,
    tenant_id: &str,
    deck_id: &str,
    slide_uid: &str,
) -> Result<SlideSlotsSnapshot, PresentationRepoError> {
    let deck_row: Option<(String, i64)> = conn
        .query_row(
            "SELECT d.deck_mode, d.revision
             FROM presentation_decks d
             JOIN projects p ON p.id = d.project_id
             WHERE d.id = ?1 AND d.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL",
            params![deck_id, project_id, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| map_sqlite_error(e, "select_slide_slots_deck_owner"))?;
    let Some((deck_mode, deck_revision)) = deck_row else {
        return Err(PresentationRepoError::NotFound(
            "E_DECK_NOT_FOUND".to_string(),
        ));
    };

    let slides = load_slides_for_deck(conn, deck_id)?;
    let Some(slide) = slides.into_iter().find(|s| s.slide_uid == slide_uid) else {
        return Err(PresentationRepoError::NotFound(
            "E_SLIDE_NOT_FOUND".to_string(),
        ));
    };

    let slots = load_wording_slots_v12(conn, &slide.id)
        .map_err(|e| map_sqlite_error(e, "load_wording_slots_v12"))?;

    Ok(SlideSlotsSnapshot {
        deck_id: deck_id.to_string(),
        deck_mode,
        deck_revision,
        slide,
        slots,
    })
}

fn upsert_slots_in_tx(
    tx: &rusqlite::Transaction<'_>,
    slide_id: &str,
    now_ms: i64,
    slot_upserts: &[SlotUpsertInput],
) -> Result<(), rusqlite::Error> {
    for slot in slot_upserts {
        let new_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO wording_slots
             (id, slide_id, ff_id, slot_label, slot_kind, text_value, charts_json, safe_styles_json, updated_by, ast_revision, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10)
             ON CONFLICT(slide_id, ff_id) DO UPDATE SET
               slot_label = CASE WHEN excluded.slot_label != '' THEN excluded.slot_label ELSE wording_slots.slot_label END,
               slot_kind = excluded.slot_kind,
               text_value = excluded.text_value,
               charts_json = excluded.charts_json,
               safe_styles_json = excluded.safe_styles_json,
               updated_by = excluded.updated_by,
               ast_revision = wording_slots.ast_revision + 1,
               updated_at = excluded.updated_at",
            params![
                new_id,
                slide_id,
                slot.ff_id,
                slot.slot_label,
                slot.slot_kind,
                slot.text_value,
                slot.charts_json,
                slot.safe_styles_json,
                slot.updated_by,
                now_ms,
            ],
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit_slide_slots_batch(
    conn: &mut Connection,
    project_id: &str,
    tenant_id: &str,
    deck_id: &str,
    slide_uid: &str,
    expected_revision: i64,
    expected_html_path: Option<&str>,
    html_artifact_update: Option<(&str, &str)>,
    speaker_notes: Option<&str>,
    slot_upserts: &[SlotUpsertInput],
    prune_unlisted_slots: bool,
) -> Result<CommitSlideMutationResult, PresentationRepoError> {
    let mut domain_err: Option<PresentationRepoError> = None;
    let mut committed: Option<CommitSlideMutationResult> = None;

    retry_on_busy("commit_slide_slots_batch", || {
        domain_err = None;
        committed = None;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let deck_row: Option<(String, i64)> = tx
            .query_row(
                "SELECT d.deck_mode, d.revision
                 FROM presentation_decks d
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1 AND d.project_id = ?2 AND p.tenant_id = ?3 AND p.trash_deleted_at IS NULL",
                params![deck_id, project_id, tenant_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((deck_mode, deck_rev)) = deck_row else {
            domain_err = Some(PresentationRepoError::NotFound("E_DECK_NOT_FOUND".to_string()));
            return Ok(());
        };

        let slide_row: Option<(String, String, i64)> = tx
            .query_row(
                "SELECT id, html_path, revision
                 FROM deck_slides
                 WHERE deck_id = ?1 AND slide_uid = ?2",
                params![deck_id, slide_uid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((slide_id, previous_html_path, slide_rev)) = slide_row else {
            domain_err = Some(PresentationRepoError::NotFound("E_SLIDE_NOT_FOUND".to_string()));
            return Ok(());
        };

        let active_rev = if deck_mode == "one-page" { deck_rev } else { slide_rev };
        if let Some(exp_path) = expected_html_path {
            if previous_html_path != exp_path {
                domain_err = Some(PresentationRepoError::StaleRevision {
                    expected: expected_revision,
                    actual: active_rev,
                });
                return Ok(());
            }
        }

        let now_ms = Utc::now().timestamp_millis();
        let lease_held: Option<i64> = if deck_mode == "one-page" {
            let deck_owner_prefix = format!("slide-copilot:{deck_id}:%");
            tx.query_row(
                "SELECT 1 FROM node_execution_jobs
                 WHERE tenant_id = ?1 AND project_id = ?2 AND lease_owner LIKE ?3
                   AND status = 'RUNNING' AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?4)
                 LIMIT 1",
                params![tenant_id, project_id, deck_owner_prefix, now_ms],
                |r| r.get(0),
            )
            .optional()?
        } else {
            let slide_owner = format!("slide-copilot:{deck_id}:{slide_uid}");
            tx.query_row(
                "SELECT 1 FROM node_execution_jobs
                 WHERE tenant_id = ?1 AND project_id = ?2 AND lease_owner = ?3
                   AND status = 'RUNNING' AND (stale_after_timestamp IS NULL OR stale_after_timestamp >= ?4)
                 LIMIT 1",
                params![tenant_id, project_id, slide_owner, now_ms],
                |r| r.get(0),
            )
            .optional()?
        };
        if lease_held.is_some() {
            domain_err = Some(PresentationRepoError::CopilotLeaseHeld(
                "Slide CMS is temporarily locked while Slide Copilot holds an active lease"
                    .to_string(),
            ));
            return Ok(());
        }

        let new_revision: i64;

        if deck_mode == "one-page" {
            if deck_rev != expected_revision {
                domain_err = Some(PresentationRepoError::StaleRevision {
                    expected: expected_revision,
                    actual: deck_rev,
                });
                return Ok(());
            }
            let updated_deck = tx.execute(
                "UPDATE presentation_decks
                 SET revision = revision + 1, updated_at = ?1
                 WHERE id = ?2 AND revision = ?3",
                params![now_ms, deck_id, expected_revision],
            )?;
            if updated_deck != 1 {
                domain_err = Some(PresentationRepoError::StaleRevision {
                    expected: expected_revision,
                    actual: deck_rev,
                });
                return Ok(());
            }
            new_revision = expected_revision + 1;
            if let Some((new_html_path, new_content_sha256)) = html_artifact_update {
                tx.execute(
                    "UPDATE deck_slides
                     SET html_path = ?1, content_sha256 = ?2, revision = ?3, updated_at = ?4
                     WHERE deck_id = ?5",
                    params![new_html_path, new_content_sha256, new_revision, now_ms, deck_id],
                )?;
            } else {
                tx.execute(
                    "UPDATE deck_slides SET revision = ?1, updated_at = ?2 WHERE deck_id = ?3",
                    params![new_revision, now_ms, deck_id],
                )?;
            }
            if let Some(notes) = speaker_notes {
                tx.execute(
                    "UPDATE deck_slides SET speaker_notes = ?1, updated_at = ?2 WHERE id = ?3",
                    params![notes, now_ms, slide_id],
                )?;
            }
        } else {
            if slide_rev != expected_revision {
                domain_err = Some(PresentationRepoError::StaleRevision {
                    expected: expected_revision,
                    actual: slide_rev,
                });
                return Ok(());
            }
            let updated_slide = if let Some((new_html_path, new_content_sha256)) = html_artifact_update {
                tx.execute(
                    "UPDATE deck_slides
                     SET html_path = ?1,
                         content_sha256 = ?2,
                         speaker_notes = COALESCE(?3, speaker_notes),
                         revision = revision + 1,
                         updated_at = ?4
                     WHERE id = ?5 AND revision = ?6",
                    params![
                        new_html_path,
                        new_content_sha256,
                        speaker_notes,
                        now_ms,
                        slide_id,
                        expected_revision,
                    ],
                )?
            } else {
                tx.execute(
                    "UPDATE deck_slides
                     SET speaker_notes = COALESCE(?1, speaker_notes),
                         revision = revision + 1,
                         updated_at = ?2
                     WHERE id = ?3 AND revision = ?4",
                    params![speaker_notes, now_ms, slide_id, expected_revision],
                )?
            };
            if updated_slide != 1 {
                domain_err = Some(PresentationRepoError::StaleRevision {
                    expected: expected_revision,
                    actual: slide_rev,
                });
                return Ok(());
            }
            new_revision = expected_revision + 1;
        }

        if prune_unlisted_slots {
            let keep_ids: HashSet<&str> = slot_upserts.iter().map(|s| s.ff_id.as_str()).collect();
            let existing = load_wording_slots_v12(&tx, &slide_id)?;
            for old_slot in existing {
                if !keep_ids.contains(old_slot.ff_id.as_str()) {
                    tx.execute("DELETE FROM wording_slots WHERE id = ?1", params![old_slot.id])?;
                }
            }
        }

        upsert_slots_in_tx(&tx, &slide_id, now_ms, slot_upserts)?;
        tx.execute(
            "UPDATE presentation_decks SET updated_at = ?1 WHERE id = ?2",
            params![now_ms, deck_id],
        )?;
        let fresh_slots = load_wording_slots_v12(&tx, &slide_id)?;
        tx.commit()?;
        committed = Some(CommitSlideMutationResult {
            new_revision,
            previous_html_path,
            slots: fresh_slots,
        });
        Ok(())
    })
    .map_err(|e| translate_harness_to_repo_err(e, "commit_slide_slots_batch"))?;

    if let Some(err) = domain_err {
        return Err(err);
    }
    committed.ok_or_else(|| PresentationRepoError::NotFound("E_SLIDE_NOT_FOUND".to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn apply_custom_slide_html_commit(
    conn: &mut Connection,
    project_id: &str,
    tenant_id: &str,
    deck_id: &str,
    slide_uid: &str,
    expected_revision: i64,
    expected_html_path: Option<&str>,
    new_html_path: &str,
    new_content_sha256: &str,
    resync_structured_slots: bool,
    extracted_slots: &[SlotUpsertInput],
) -> Result<CommitSlideMutationResult, PresentationRepoError> {
    let upserts: &[SlotUpsertInput] = if resync_structured_slots {
        extracted_slots
    } else {
        &[]
    };
    commit_slide_slots_batch(
        conn,
        project_id,
        tenant_id,
        deck_id,
        slide_uid,
        expected_revision,
        expected_html_path,
        Some((new_html_path, new_content_sha256)),
        None,
        upserts,
        resync_structured_slots,
    )
}

pub fn list_deck_slide_uids(
    conn: &Connection,
    deck_id: &str,
) -> Result<HashSet<String>, PresentationRepoError> {
    let mut stmt = conn
        .prepare("SELECT slide_uid FROM deck_slides WHERE deck_id = ?1")
        .map_err(|e| map_sqlite_error(e, "prepare_deck_slide_uids"))?;
    let set = stmt
        .query_map(params![deck_id], |r| r.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "query_deck_slide_uids"))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "collect_deck_slide_uids"))?;
    Ok(set)
}

pub fn list_all_project_workspace_roots(
    conn: &Connection,
) -> Result<Vec<String>, PresentationRepoError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT workspace_root FROM projects WHERE workspace_root IS NOT NULL AND TRIM(workspace_root) != ''")
        .map_err(|e| map_sqlite_error(e, "prepare_project_roots"))?;
    let roots = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "query_project_roots"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "collect_project_roots"))?;
    Ok(roots)
}

pub fn list_active_slide_html_paths(
    conn: &Connection,
) -> Result<HashSet<String>, PresentationRepoError> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT html_path FROM deck_slides")
        .map_err(|e| map_sqlite_error(e, "prepare_active_slide_paths"))?;
    let set = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| map_sqlite_error(e, "query_active_slide_paths"))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| map_sqlite_error(e, "collect_active_slide_paths"))?;
    Ok(set)
}

fn translate_harness_to_repo_err(err: HarnessError, context: &str) -> PresentationRepoError {
    let err_str = format!("{err}");
    let err_lower = err_str.to_ascii_lowercase();
    if err_lower.contains("database is locked")
        || err_lower.contains("sqlite_busy")
        || err_lower.contains("busy")
    {
        return PresentationRepoError::SqliteBusy(format!("{context}: {err_str}"));
    }
    if let HarnessError::ValidationFailed { ref reason } = err {
        if reason.contains("QueryReturnedNoRows") || reason.contains("REFERENTIAL_INTEGRITY") {
            return PresentationRepoError::ForeignKeyInvalid(format!("{context}: {reason}"));
        }
        if reason.contains("UNIQUE_CONSTRAINT") {
            return PresentationRepoError::SlideIndexConflict(format!("{context}: {reason}"));
        }
    }
    PresentationRepoError::Internal(err)
}

pub fn map_repo_error_code(err: &PresentationRepoError) -> (u16, &'static str, bool, String) {
    match err {
        PresentationRepoError::NotFound(_) => (
            404,
            "E_DECK_NOT_FOUND",
            false,
            "Requested presentation deck was not found".to_string(),
        ),
        PresentationRepoError::StaleRevision { expected, actual } => (
            409,
            "E_STALE_REVISION",
            true,
            format!("Expected deck revision {expected}, actual revision {actual}"),
        ),
        PresentationRepoError::SlideIndexConflict(msg) => {
            (409, "E_SLIDE_INDEX_CONFLICT", true, msg.clone())
        }
        PresentationRepoError::CopilotLeaseHeld(msg) => {
            (409, "E_COPILOT_LEASE_HELD", true, msg.clone())
        }
        PresentationRepoError::UlidExhausted(msg) => {
            (500, "E_ULID_ENTROPY_EXHAUSTED", true, msg.clone())
        }
        PresentationRepoError::ForeignKeyInvalid(_) => (
            404,
            "E_DECK_NOT_FOUND",
            false,
            "Requested presentation deck was not found".to_string(),
        ),
        PresentationRepoError::SqliteBusy(msg) => (503, "E_SQLITE_BUSY_RETRY", true, msg.clone()),
        PresentationRepoError::Internal(e) => (500, "INTERNAL_ERROR", false, format!("{e}")),
    }
}
