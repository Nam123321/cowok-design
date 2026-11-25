-- V27__story_10_2_worker_file_leases.sql
-- Story 10.2: Parallel Swarm Dispatch, Disjoint worker_file_leases (fencing_token + canonicalized owned_paths[]),
-- 429 Semaphore Clamp (3 -> 1), Independent Builder != Scorer QC (E1..E4 >= 95%) & Tier-3 Single-Session Design Jury.

CREATE TABLE IF NOT EXISTS worker_file_leases (
    lease_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    worker_id TEXT NOT NULL,
    worker_role TEXT NOT NULL CHECK (worker_role IN ('BUILDER','VERIFIER_QC','DESIGN_JURY')),
    worker_pgid INTEGER,
    fencing_token BIGINT NOT NULL,
    owned_paths_json TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    released INTEGER NOT NULL DEFAULT 0 CHECK (released IN (0, 1)),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_worker_file_leases_active
    ON worker_file_leases(project_id, released, expires_at);

CREATE TABLE IF NOT EXISTS fleet_jury_verdicts (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    builder_worker_id TEXT NOT NULL,
    scorer_worker_id TEXT NOT NULL,
    generation_fence BIGINT NOT NULL,
    freeze_snapshot_sha256 TEXT NOT NULL,
    qc_run_id TEXT,
    e1_e4_catch_rate REAL NOT NULL,
    verdict TEXT NOT NULL CHECK (verdict IN ('PASS','FAIL','ESCALATED_TO_LEDGER')),
    violations_json TEXT NOT NULL DEFAULT '[]',
    ledger_entry_id TEXT,
    created_at BIGINT NOT NULL,
    CHECK (builder_worker_id <> scorer_worker_id)
);

CREATE INDEX IF NOT EXISTS idx_fleet_jury_project
    ON fleet_jury_verdicts(project_id, created_at);
