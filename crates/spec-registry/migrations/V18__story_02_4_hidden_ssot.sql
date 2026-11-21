-- V18__story_02_4_hidden_ssot.sql
-- Story 02.4: Hidden Dual-Use SSOT (component_patterns & app_shell_locks),
-- Single Lock Owner monotonic fencing_token, portal-scoped quarantine/reverse sync,
-- CURRENT_SCHEMA_VERSION = 18, DAEMON_MIN_SCHEMA_VERSION = 18, total 58 SQLite WAL tables.

CREATE UNIQUE INDEX IF NOT EXISTS uq_projects_id_tenant
    ON projects(id, tenant_id);

CREATE TABLE IF NOT EXISTS component_patterns (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    portal_id TEXT NOT NULL CHECK (
        portal_id IN ('SuperAdmin','TenantAdmin','SalesApp','MarketingWeb','DeveloperPortal','Presentation')
    ),
    pattern_slug TEXT NOT NULL,
    component_name TEXT NOT NULL,
    source_path TEXT,
    ast_signature_sha256 TEXT NOT NULL,
    dedup_marker_sha256 TEXT NOT NULL DEFAULT '',
    source_revision BIGINT NOT NULL DEFAULT 0,
    last_sync_source TEXT NOT NULL DEFAULT 'manual' CHECK (
        last_sync_source IN ('oxc_reverse_sync','manual','export_sync')
    ),
    tokens_snapshot_json TEXT NOT NULL DEFAULT '{}',
    doc_markdown TEXT NOT NULL DEFAULT '',
    drift_status TEXT NOT NULL DEFAULT 'synced' CHECK (
        drift_status IN ('synced','quarantined_drift')
    ),
    quarantined_diff_json TEXT NOT NULL DEFAULT '{}',
    revision BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id, portal_id, pattern_slug)
);

CREATE INDEX IF NOT EXISTS idx_component_patterns_proj_portal
    ON component_patterns(tenant_id, project_id, portal_id, updated_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS app_shell_locks (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    portal_id TEXT NOT NULL CHECK (
        portal_id IN ('SuperAdmin','TenantAdmin','SalesApp','MarketingWeb','DeveloperPortal','Presentation')
    ),
    lock_owner_id TEXT NOT NULL,
    fencing_token BIGINT NOT NULL DEFAULT 1,
    locked_slots_json TEXT NOT NULL DEFAULT '[]',
    expires_at BIGINT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id, portal_id)
);

CREATE INDEX IF NOT EXISTS idx_app_shell_locks_proj_portal
    ON app_shell_locks(tenant_id, project_id, portal_id);
