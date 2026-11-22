-- V19__story_05_4_html_forge_and_sketch_polish.sql
-- Story 05.4: Shared html-forge surgical patch persistence (OCC via ast_revision + base_artifact_sha),
-- 2-Tier Auto-Apply receipts (INSTANT_UNDO vs LOCKED_RULE_RECEIPT), and SDI clarification sessions
-- with -PGID preempt tracking & monotonic generation_epoch.
-- CURRENT_SCHEMA_VERSION = 19, DAEMON_MIN_SCHEMA_VERSION = 19, total 60 SQLite WAL tables.

CREATE TABLE IF NOT EXISTS screen_artifact_patches (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    screen_id TEXT NOT NULL,
    ast_revision BIGINT NOT NULL DEFAULT 1,
    base_artifact_sha TEXT NOT NULL,
    result_artifact_sha TEXT NOT NULL,
    ff_id TEXT NOT NULL,
    op_kind TEXT NOT NULL CHECK (
        op_kind IN ('TEXT_EDIT','TOKEN_SWAP','STEERING_CHIP','LOCKED_RULE_CHANGE','UNDO')
    ),
    apply_tier TEXT NOT NULL CHECK (
        apply_tier IN ('INSTANT_UNDO','LOCKED_RULE_RECEIPT')
    ),
    reconciled_ff_ids INTEGER NOT NULL DEFAULT 0,
    payload_json TEXT NOT NULL DEFAULT '{}',
    html_snapshot TEXT NOT NULL DEFAULT '',
    locked_rule_json TEXT,
    created_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id, screen_id, ast_revision)
);

CREATE INDEX IF NOT EXISTS idx_screen_artifact_patches_proj_screen
    ON screen_artifact_patches(tenant_id, project_id, screen_id, ast_revision DESC);

CREATE TABLE IF NOT EXISTS sdi_clarification_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'clarifying' CHECK (
        status IN ('clarifying','locked','preempted')
    ),
    goal_prompt TEXT NOT NULL,
    locale TEXT NOT NULL DEFAULT 'vi' CHECK (
        locale IN ('vi','en')
    ),
    active_direction_id TEXT NOT NULL,
    directions_json TEXT NOT NULL DEFAULT '[]',
    generation_epoch BIGINT NOT NULL DEFAULT 1,
    preempt_pgid BIGINT,
    routed_skill_slugs_json TEXT NOT NULL DEFAULT '[]',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id)
);

CREATE INDEX IF NOT EXISTS idx_sdi_clarification_sessions_proj
    ON sdi_clarification_sessions(tenant_id, project_id, updated_at DESC);
