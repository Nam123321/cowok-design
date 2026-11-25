-- V31__story_10_3_design_tournament.sql
-- Story 10.3: Anti-Slop Design Tournament (4 Orthogonal Axes <=15s, AST Structural Distance <=0.78,
-- Position-Swapped RubricShield Blind Judge, Cross-Variant Borrow, /crystallize & advisor 6-Step GEPA Loop).
-- Logical V18b schema sequenced additively at V31 in SQLite WAL (ADR-006, ADR-009, ADR-091).

CREATE TABLE IF NOT EXISTS tournament_runs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    axes_json TEXT NOT NULL,
    winning_variant_id TEXT,
    borrowed_from_variant_id TEXT,
    borrowed_ff_id TEXT,
    generation_epoch BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tournament_runs_project_created
    ON tournament_runs(project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS tournament_variants (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES tournament_runs(id) ON DELETE CASCADE,
    axis_profile TEXT NOT NULL,
    artifact_sha256 TEXT NOT NULL,
    ast_similarity REAL NOT NULL,
    clone_distance_passed INTEGER NOT NULL CHECK (clone_distance_passed IN (0, 1)),
    rubric_verdict TEXT,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1))
);

CREATE INDEX IF NOT EXISTS idx_tournament_variants_run
    ON tournament_variants(run_id);

CREATE TABLE IF NOT EXISTS creative_memory_events (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_kind TEXT NOT NULL CHECK (
        event_kind IN ('TOURNAMENT_WIN', 'TOURNAMENT_BORROW', 'ADVISOR_PATCH', 'CRYSTALLIZE')
    ),
    payload_json TEXT NOT NULL,
    regression_passed INTEGER NOT NULL DEFAULT 1 CHECK (regression_passed IN (0, 1)),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_creative_memory_events_project_created
    ON creative_memory_events(project_id, created_at DESC);
