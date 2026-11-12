-- Migration V4: Storyboard Chat, Keyed Execution Engine, Composite Node Scopes & Multi-Project Isolation
-- Story 04.2 (Tier A Shared Infrastructure): Implements AC-01

-- 1. Idempotent columns on projects for Project Code, Trash Journal & Provenance
-- Note: SQLite ALTER TABLE ADD COLUMN is idempotent when checked via pragma_table_info in Rust pool,
-- or safe when executed in migration blocks.

-- 2. chat_sessions (ensure table and columns)
CREATE TABLE IF NOT EXISTS chat_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    model_provider TEXT NOT NULL DEFAULT 'default',
    archived_at BIGINT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_project ON chat_sessions(project_id, updated_at DESC);

-- 3. chat_messages: immutable turn history with fork support
CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    prose_markdown TEXT NOT NULL,
    parent_message_id TEXT REFERENCES chat_messages(id) ON DELETE SET NULL,
    turn_receipt_json TEXT,
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at ASC);

-- 4. chat_message_node_scopes: composite scoping ${projectId}::${sessionId}::${nodeId}
CREATE TABLE IF NOT EXISTS chat_message_node_scopes (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    composite_scope_key TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE(message_id, node_id)
);
CREATE INDEX IF NOT EXISTS idx_chat_node_scopes_key ON chat_message_node_scopes(composite_scope_key);
CREATE INDEX IF NOT EXISTS idx_chat_node_scopes_proj_node ON chat_message_node_scopes(project_id, node_id);

-- 5. node_execution_jobs: Keyed Parallel Execution Queue
CREATE TABLE IF NOT EXISTS node_execution_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'QUEUED' CHECK(status IN ('QUEUED', 'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED')),
    fencing_token INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 10,
    lease_owner TEXT,
    stale_after_timestamp BIGINT,
    error_json TEXT,
    created_at BIGINT NOT NULL,
    started_at BIGINT,
    completed_at BIGINT
);
CREATE INDEX IF NOT EXISTS idx_node_jobs_status ON node_execution_jobs(project_id, status);
CREATE INDEX IF NOT EXISTS idx_node_jobs_node ON node_execution_jobs(project_id, node_id, status);

-- 6. chat_artifact_snapshots: immutable per-turn visual snapshots (SHA-256)
CREATE TABLE IF NOT EXISTS chat_artifact_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    node_id TEXT,
    artifact_type TEXT NOT NULL DEFAULT 'html_state',
    content_hash TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_artifact_snapshots_msg ON chat_artifact_snapshots(message_id);
CREATE INDEX IF NOT EXISTS idx_artifact_snapshots_proj ON chat_artifact_snapshots(project_id, node_id);
