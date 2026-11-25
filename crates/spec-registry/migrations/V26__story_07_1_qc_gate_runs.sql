-- V26__story_07_1_qc_gate_runs.sql
-- Story 07.1: Pooled 1-Chromium Rust CDP Visual QC, Real RGBA SSIM,
-- E2E Flow Completeness Auditor & 1-Click Deterministic Auto-Fix.
-- Additive SQLite WAL migration sequential after V25.

CREATE TABLE IF NOT EXISTS qc_gate_runs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('APP_SCREEN','PRESENTATION_SLIDE','FULL_FLOW')),
    target_id TEXT NOT NULL,
    layout_sha1 TEXT NOT NULL,
    viewports_json TEXT NOT NULL,
    verdict TEXT NOT NULL CHECK (verdict IN ('PASS','FAIL','UNVERIFIED_NO_BROWSER')),
    p0_count INTEGER NOT NULL DEFAULT 0,
    p1_count INTEGER NOT NULL DEFAULT 0,
    ssim_score REAL,
    issues_json TEXT NOT NULL DEFAULT '[]',
    autofix_attempt INTEGER NOT NULL DEFAULT 0 CHECK (autofix_attempt BETWEEN 0 AND 3),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_qc_gate_runs_cache ON qc_gate_runs(project_id, layout_sha1, created_at);
