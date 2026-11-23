-- V21__story_09_2_decision_audit_log.sql
-- Story 09.2: Decision Cascade Engine (~450 LOC Rust DecisionCascadeClient),
-- 5-Stage DE -> Decision #1 -> DR/DE -> DE -> Decision #2, VerdictShield (0% float leak),
-- RubricShield (>=90% swap consistency + deterministic DE tie-break), MAX_ATTEMPTS=3 breaker & StaleFile OCC.
-- CURRENT_SCHEMA_VERSION = 21, DAEMON_MIN_SCHEMA_VERSION = 21, total 63 SQLite WAL tables.

CREATE TABLE IF NOT EXISTS decision_audit_log (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    cascade_stage TEXT NOT NULL CHECK (cascade_stage IN ('DECISION_1','DECISION_2','RUBRIC_SHIELD')),
    tier_used TEXT NOT NULL CHECK (tier_used IN ('LOCAL_FTS5_BM25','GEMINI_DECISION_1','TYPESAFE_JEV','HEURISTIC_FALLBACK')),
    confidence_kind TEXT NOT NULL CHECK (confidence_kind IN ('ExactMatch','ModelVerdict','DeterministicFallback')),
    verdict_label TEXT NOT NULL,
    rubric_swap_consistent INTEGER CHECK (rubric_swap_consistent IN (0, 1)),
    attempt_number INTEGER NOT NULL CHECK (attempt_number BETWEEN 1 AND 3),
    breaker_tripped INTEGER NOT NULL DEFAULT 0 CHECK (breaker_tripped IN (0, 1)),
    stale_file_sha256 TEXT,
    latency_ms INTEGER NOT NULL CHECK (latency_ms >= 0),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decision_audit_log_tenant_project
    ON decision_audit_log(tenant_id, project_id, created_at DESC);
