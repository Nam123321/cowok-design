-- @story 11.2
-- @implements AC-01, AC-03, AC-04 (V28 Additive SQLite WAL DDL for Bidirectional Open MCP Server, 4-Tier Tool Policies, Live External Activity Audit & Host Install Backups)

CREATE TABLE IF NOT EXISTS mcp_tool_policies (
    tool_name TEXT PRIMARY KEY NOT NULL,
    permission_tier TEXT NOT NULL CHECK (permission_tier IN ('READ_ONLY','INSPECT','REVISE_PROPOSE','HUMAN_ONLY')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    mcp_callable INTEGER NOT NULL DEFAULT 1 CHECK (mcp_callable IN (0, 1)),
    updated_at BIGINT NOT NULL,
    CHECK (permission_tier <> 'HUMAN_ONLY' OR mcp_callable = 0)
);

CREATE TABLE IF NOT EXISTS mcp_external_activity_logs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('MCP_CLIENT','HUMAN_WORKBENCH_SESSION')),
    host_client TEXT NOT NULL,
    method_name TEXT NOT NULL,
    tool_name TEXT,
    permission_tier TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('OK','SELF_HEALING_ERROR','DENIED_HUMAN_ONLY','VALIDATION_ERROR')),
    jsonrpc_code INTEGER,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    idempotency_key TEXT,
    summary TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mcp_activity_created ON mcp_external_activity_logs(created_at DESC);

CREATE TABLE IF NOT EXISTS mcp_host_install_backups (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    config_path TEXT NOT NULL,
    backup_path TEXT NOT NULL,
    before_sha256 TEXT NOT NULL,
    after_sha256 TEXT NOT NULL,
    rolled_back INTEGER NOT NULL DEFAULT 0 CHECK (rolled_back IN (0, 1)),
    created_at BIGINT NOT NULL
);
