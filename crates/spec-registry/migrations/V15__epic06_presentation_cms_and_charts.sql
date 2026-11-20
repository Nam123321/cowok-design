-- V15__epic06_presentation_cms_and_charts.sql
-- Story 06.1 / 05.2: Additive V15 columns on presentation_decks, deck_slides, wording_slots, change_proposals
-- applied idempotently via SqlitePool::apply_v15_columns, followed by Crockford Base32
-- ULID backfill (^SLD-[0-9A-HJKMNP-TV-Z]{26}$) and unique index enforcement.

CREATE UNIQUE INDEX IF NOT EXISTS deck_slides_slide_uid_key
    ON deck_slides(slide_uid);

CREATE INDEX IF NOT EXISTS idx_presentation_decks_project_rev
    ON presentation_decks(project_id, revision);
