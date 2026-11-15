-- V8__daemon_auth_sessions_and_nonces.sql
-- Persistent Loopback Auth Sessions, HMAC Replay Nonce Ledger, and Workspace Sync Intents.
-- Makes daemon restarts fail-closed for nonce replay protection and preserves valid loopback sessions.

CREATE TABLE IF NOT EXISTS daemon_sessions (
    token_sha256 TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    user_id TEXT NOT NULL DEFAULT 'local-operator',
    role TEXT NOT NULL CHECK (role IN ('viewer', 'editor', 'admin')),
    expires_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    revoked_at BIGINT
);

CREATE INDEX IF NOT EXISTS idx_daemon_sessions_expires_at
    ON daemon_sessions(expires_at);

CREATE INDEX IF NOT EXISTS idx_daemon_sessions_tenant_user
    ON daemon_sessions(tenant_id, user_id);

CREATE TABLE IF NOT EXISTS daemon_replay_nonces (
    nonce TEXT PRIMARY KEY NOT NULL,
    daemon_nonce TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    seen_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_daemon_replay_nonces_expires_at
    ON daemon_replay_nonces(expires_at);

CREATE TABLE IF NOT EXISTS workspace_sync_intents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    intent_kind TEXT NOT NULL CHECK (intent_kind IN ('SPEC_SYNC', 'PROJECT_PROMOTE', 'WORKSPACE_RESTORE')),
    target_path TEXT NOT NULL,
    expected_sha256 TEXT,
    state TEXT NOT NULL CHECK (state IN ('PENDING', 'COMMITTED', 'ROLLED_BACK', 'FAILED')),
    fencing_token BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workspace_sync_intents_project_state
    ON workspace_sync_intents(project_id, state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_workspace_sync_intents_tenant
    ON workspace_sync_intents(tenant_id, updated_at DESC);
