-- Migration V6: Epic 04 Complete Contracts & .cowok-design Config Dir Unification
-- Adds 8 normalized tables (agent_settings, mcp_servers, storyboards, node_chat_sessions,
-- element_comments, screen_fixtures, execution_events, sdi_intents) and eliminates legacy '.opendesign' config_dir.

-- 1. agent_settings (normalized provider settings without raw secret persistence in plain blobs)
CREATE TABLE IF NOT EXISTS agent_settings (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    endpoint_url TEXT NOT NULL DEFAULT '',
    credential_ref TEXT NOT NULL DEFAULT '',
    is_default INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'configured',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(tenant_id, project_id, provider_id)
);
CREATE INDEX IF NOT EXISTS idx_agent_settings_tenant ON agent_settings(tenant_id, project_id);

-- 2. mcp_servers
CREATE TABLE IF NOT EXISTS mcp_servers (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    server_slug TEXT NOT NULL,
    display_name TEXT NOT NULL,
    transport_kind TEXT NOT NULL DEFAULT 'stdio' CHECK(transport_kind IN ('stdio', 'http_sse')),
    command_or_url TEXT NOT NULL,
    permission_tier TEXT NOT NULL DEFAULT 'read_only',
    enabled INTEGER NOT NULL DEFAULT 1,
    last_health_status TEXT NOT NULL DEFAULT 'unknown',
    last_checked_at BIGINT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(tenant_id, project_id, server_slug)
);
CREATE INDEX IF NOT EXISTS idx_mcp_servers_tenant ON mcp_servers(tenant_id, project_id);

-- 3. storyboards
CREATE TABLE IF NOT EXISTS storyboards (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    flow_graph_id TEXT REFERENCES flow_graphs(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    narrative_summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'DRAFT',
    version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_storyboards_project ON storyboards(tenant_id, project_id);

-- 4. node_chat_sessions
CREATE TABLE IF NOT EXISTS node_chat_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    chat_session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'ACTIVE',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(project_id, node_id, chat_session_id)
);
CREATE INDEX IF NOT EXISTS idx_node_chat_sessions_lookup ON node_chat_sessions(tenant_id, project_id, node_id);

-- 5. element_comments
CREATE TABLE IF NOT EXISTS element_comments (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    element_ff_id TEXT NOT NULL,
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    dom_hash TEXT NOT NULL,
    anchor_status TEXT NOT NULL DEFAULT 'active' CHECK(anchor_status IN ('active', 'stale', 'resolved')),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_element_comments_scope ON element_comments(tenant_id, project_id, node_id);

-- 6. screen_fixtures
CREATE TABLE IF NOT EXISTS screen_fixtures (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    variant TEXT NOT NULL CHECK(variant IN ('DEFAULT', 'EMPTY', 'LOADING', 'ERROR')),
    payload_json TEXT NOT NULL DEFAULT '{}',
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(tenant_id, project_id, node_id, variant)
);
CREATE INDEX IF NOT EXISTS idx_screen_fixtures_scope ON screen_fixtures(tenant_id, project_id, node_id);

-- 7. execution_events
CREATE TABLE IF NOT EXISTS execution_events (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    job_id TEXT NOT NULL REFERENCES node_execution_jobs(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    payload_text TEXT NOT NULL DEFAULT '',
    seq_no INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_execution_events_job ON execution_events(job_id, seq_no);
CREATE INDEX IF NOT EXISTS idx_execution_events_project ON execution_events(tenant_id, project_id, created_at);

-- 8. sdi_intents
CREATE TABLE IF NOT EXISTS sdi_intents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    raw_prompt TEXT NOT NULL,
    intent_type TEXT NOT NULL DEFAULT 'create_screen',
    target_scope_json TEXT NOT NULL DEFAULT '{}',
    ambiguity_score REAL NOT NULL DEFAULT 0.0,
    risk_score REAL NOT NULL DEFAULT 0.0,
    variants_json TEXT,
    selected_variant_id TEXT,
    status TEXT NOT NULL DEFAULT 'ready',
    design_intent_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sdi_intents_scope ON sdi_intents(tenant_id, project_id, updated_at DESC);

-- 9. Unify project_boundaries config_dir off legacy '.opendesign'
UPDATE project_boundaries
SET config_dir = '.cowok-design'
WHERE config_dir = '.opendesign';
