-- V7__epic04_cas_seq_and_vault_convergence.sql
-- Additive SQLite V7 convergence for Story 04.2:
--   1. ChatMessageNodeScope tenant_id backfill
--   2. ExecutionEvent monotonic partial-null project_seq backfill + unique/scope indexes
--   3. NodeExecutionJob 12-step FK-preserving rebuild (PAUSED/INTERRUPTED, composite_scope_key, daemon_nonce, priority 10, updated_at)
--   4. Partial unique indexes on projects(slug) & projects(workspace_root) WHERE trash_deleted_at IS NULL
--   5. ElementComment CAS anchor & NodeChatSession scope indexes

-- 1. Backfill chat_message_node_scopes.tenant_id from owning project when non-default
UPDATE chat_message_node_scopes
SET tenant_id = (
    SELECT p.tenant_id
    FROM projects p
    WHERE p.id = chat_message_node_scopes.project_id
)
WHERE tenant_id = 'local-default'
  AND EXISTS (
      SELECT 1
      FROM projects p
      WHERE p.id = chat_message_node_scopes.project_id
        AND p.tenant_id != 'local-default'
  );

-- 2. Backfill monotonic positive project_seq ONLY where NULL (collision-safe offset)
WITH numbered_missing AS (
    SELECT
        e.id,
        COALESCE((
            SELECT MAX(e2.project_seq)
            FROM execution_events e2
            WHERE e2.project_id = e.project_id
              AND e2.project_seq IS NOT NULL
        ), 0) + ROW_NUMBER() OVER (
            PARTITION BY e.project_id
            ORDER BY e.created_at ASC, e.seq_no ASC, e.id ASC
        ) AS next_seq
    FROM execution_events e
    WHERE e.project_seq IS NULL
)
UPDATE execution_events
SET project_seq = (
    SELECT nm.next_seq
    FROM numbered_missing nm
    WHERE nm.id = execution_events.id
)
WHERE project_seq IS NULL
  AND EXISTS (
      SELECT 1
      FROM numbered_missing nm
      WHERE nm.id = execution_events.id
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_execution_events_project_seq
    ON execution_events(project_id, project_seq) WHERE project_seq IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_execution_events_scope
    ON execution_events(tenant_id, project_id, composite_scope_key, project_seq);

-- 3. 12-step FK-preserving rebuild of node_execution_jobs (PRAGMA foreign_keys = OFF verified in pool.rs)
CREATE TABLE node_execution_jobs_v7 (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    composite_scope_key TEXT NOT NULL DEFAULT '',
    daemon_nonce TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'QUEUED' CHECK(status IN ('QUEUED', 'RUNNING', 'PAUSED', 'INTERRUPTED', 'COMPLETED', 'FAILED', 'CANCELLED')),
    fencing_token INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 10,
    lease_owner TEXT,
    stale_after_timestamp BIGINT,
    error_json TEXT,
    created_at BIGINT NOT NULL,
    started_at BIGINT,
    completed_at BIGINT,
    updated_at BIGINT NOT NULL DEFAULT 0
);

INSERT INTO node_execution_jobs_v7 (
    id,
    tenant_id,
    project_id,
    session_id,
    node_id,
    composite_scope_key,
    daemon_nonce,
    status,
    fencing_token,
    priority,
    lease_owner,
    stale_after_timestamp,
    error_json,
    created_at,
    started_at,
    completed_at,
    updated_at
)
SELECT
    id,
    COALESCE(tenant_id, 'local-default'),
    project_id,
    session_id,
    node_id,
    project_id || '::' || session_id || '::' || node_id,
    '',
    status,
    fencing_token,
    priority,
    lease_owner,
    stale_after_timestamp,
    error_json,
    created_at,
    started_at,
    completed_at,
    COALESCE(completed_at, started_at, created_at)
FROM node_execution_jobs;

DROP TABLE node_execution_jobs;
ALTER TABLE node_execution_jobs_v7 RENAME TO node_execution_jobs;

CREATE INDEX IF NOT EXISTS idx_node_jobs_status ON node_execution_jobs(project_id, status);
CREATE INDEX IF NOT EXISTS idx_node_jobs_node ON node_execution_jobs(project_id, node_id, status);
CREATE INDEX IF NOT EXISTS idx_node_jobs_scope ON node_execution_jobs(tenant_id, composite_scope_key);

-- 4. Partial unique indexes on active (non-trashed) projects
DROP INDEX IF EXISTS projects_slug_key;
DROP INDEX IF EXISTS projects_workspace_root_unique_idx;
DROP INDEX IF EXISTS projects_workspace_root_unique;

CREATE UNIQUE INDEX IF NOT EXISTS projects_active_slug_key
    ON projects(slug) WHERE trash_deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS projects_active_workspace_root_key
    ON projects(workspace_root) WHERE trash_deleted_at IS NULL AND workspace_root != '.';

-- 5. ElementComment CAS & NodeChatSession scope indexes
CREATE INDEX IF NOT EXISTS idx_element_comments_anchor_sha
    ON element_comments(tenant_id, project_id, node_id, anchor_kind, artifact_sha256);

CREATE INDEX IF NOT EXISTS idx_node_chat_sessions_scope
    ON node_chat_sessions(tenant_id, composite_scope_key);

-- 6. Partial unique indexes preventing SQLite NULL != NULL global default duplicates
DELETE FROM agent_settings
WHERE project_id IS NULL
  AND rowid NOT IN (
      SELECT MAX(rowid)
      FROM agent_settings
      WHERE project_id IS NULL
      GROUP BY tenant_id, provider_id
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_settings_global_unique
    ON agent_settings(tenant_id, provider_id) WHERE project_id IS NULL;

DELETE FROM mcp_servers
WHERE project_id IS NULL
  AND rowid NOT IN (
      SELECT MAX(rowid)
      FROM mcp_servers
      WHERE project_id IS NULL
      GROUP BY tenant_id, server_slug
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_mcp_servers_global_unique
    ON mcp_servers(tenant_id, server_slug) WHERE project_id IS NULL;
