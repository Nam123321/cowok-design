-- V20__story_08_5_de_dr_learn_ledger.sql
-- Story 08.5: Split DE+DR Plugin Runtime, Auto-Packaged User Skills (`user_skill_registry`, <=100 tok header)
-- and System-1 LEARN Failure Ledger (`gate_failure_ledger`, hit_count>=3, hysteresis promote>=0.95 / retire<0.85).
-- CURRENT_SCHEMA_VERSION = 20, DAEMON_MIN_SCHEMA_VERSION = 20, total 62 SQLite WAL tables.

CREATE TABLE IF NOT EXISTS user_skill_registry (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    skill_slug TEXT NOT NULL,
    repo_source_url TEXT,
    header_tokens INTEGER NOT NULL CHECK (header_tokens >= 0 AND header_tokens <= 100),
    security_scan_verdict TEXT NOT NULL CHECK (security_scan_verdict IN ('PASS','QUARANTINED','REJECT')),
    license_classification TEXT NOT NULL CHECK (license_classification IN ('ADOPT_PRIOR_ART','REFERENCE_ONLY','REJECT')),
    skill_md_path TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE(tenant_id, project_id, skill_slug)
);

CREATE INDEX IF NOT EXISTS idx_user_skill_registry_tenant_project
    ON user_skill_registry(tenant_id, project_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS uq_user_skill_registry_tenant_global_slug
    ON user_skill_registry(tenant_id, skill_slug)
    WHERE project_id IS NULL;

CREATE TABLE IF NOT EXISTS gate_failure_ledger (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    rule_id TEXT NOT NULL,
    fingerprint_sha256 TEXT NOT NULL,
    sample_message TEXT NOT NULL DEFAULT '',
    hit_count INTEGER NOT NULL DEFAULT 1 CHECK (hit_count >= 1),
    slpr_score REAL NOT NULL DEFAULT 0.0 CHECK (slpr_score >= 0.0 AND slpr_score <= 1.0),
    promoted_card_id TEXT,
    status TEXT NOT NULL DEFAULT 'ACCUMULATING' CHECK (status IN ('ACCUMULATING','PROPOSED','PROMOTED','RETIRED')),
    last_seen_at BIGINT NOT NULL,
    UNIQUE(tenant_id, project_id, fingerprint_sha256)
);

CREATE INDEX IF NOT EXISTS idx_gate_failure_ledger_scope_status
    ON gate_failure_ledger(tenant_id, project_id, status, last_seen_at DESC);
