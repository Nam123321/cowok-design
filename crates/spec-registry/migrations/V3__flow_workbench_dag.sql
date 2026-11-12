-- V3__flow_workbench_dag.sql
-- Story 04.1: 12-Step SQLite Table Rebuild for Flow-First 2D DAG Studio (PD-01 = A)
-- Executed inside BEGIN EXCLUSIVE by crates/spec-registry/src/pool.rs AFTER:
--   1) Advisory flock on db-migrate.lock (0600)
--   2) PRAGMA foreign_keys = OFF (assert == 0) & PRAGMA legacy_alter_table = OFF outside transaction
--   3) Idempotent ALTER TABLE projects ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'local-default'

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    checksum_sha256 TEXT NOT NULL,
    applied_at BIGINT NOT NULL
);

CREATE TABLE flow_graphs_v3 (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    portal_key TEXT NOT NULL DEFAULT 'default',
    title TEXT NOT NULL DEFAULT 'Default Flow',
    version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1),
    layout_version INTEGER NOT NULL DEFAULT 1 CHECK(layout_version >= 1),
    updated_at BIGINT NOT NULL,
    UNIQUE(project_id, portal_key),
    UNIQUE(id, tenant_id, project_id)
);

INSERT INTO flow_graphs_v3 (id, tenant_id, project_id, portal_key, title, version, layout_version, updated_at)
SELECT
    fg.id,
    COALESCE(p.tenant_id, 'local-default'),
    fg.project_id,
    COALESCE(NULLIF(fg.portal_key, ''), 'default'),
    COALESCE(NULLIF(fg.title, ''), 'Default Flow'),
    1,
    1,
    fg.updated_at
FROM flow_graphs fg
JOIN projects p ON p.id = fg.project_id;

-- Synthesize fallback graph if legacy flow_nodes referenced a graph_id not yet present
INSERT OR IGNORE INTO flow_graphs_v3 (id, tenant_id, project_id, portal_key, title, version, layout_version, updated_at)
SELECT DISTINCT
    fn.graph_id,
    COALESCE(p.tenant_id, 'local-default'),
    fn.project_id,
    'legacy-' || fn.graph_id,
    'Recovered Flow',
    1,
    1,
    fn.updated_at
FROM flow_nodes fn
JOIN projects p ON p.id = fn.project_id
WHERE fn.graph_id NOT IN (SELECT id FROM flow_graphs_v3);

CREATE TABLE flow_nodes_v3 (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL REFERENCES flow_graphs(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    portal_id TEXT NOT NULL DEFAULT 'default',
    screen_key TEXT NOT NULL,
    title TEXT NOT NULL,
    node_type TEXT NOT NULL DEFAULT 'screen' CHECK (node_type IN ('screen', 'modal', 'condition', 'action')),
    route_path TEXT NOT NULL DEFAULT '/',
    component_entry_path TEXT NOT NULL DEFAULT '',
    fixture_ref TEXT NOT NULL DEFAULT 'Default',
    fixture_json TEXT NOT NULL DEFAULT '{}',
    note TEXT NOT NULL DEFAULT '',
    viewport_preset TEXT NOT NULL DEFAULT 'desktop-1440',
    pos_x REAL NOT NULL DEFAULT 0.0,
    pos_y REAL NOT NULL DEFAULT 0.0,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1),
    layout_version INTEGER NOT NULL DEFAULT 1 CHECK(layout_version >= 1),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(project_id, screen_key),
    UNIQUE(graph_id, screen_key),
    UNIQUE(id, tenant_id, project_id)
);

INSERT INTO flow_nodes_v3 (
    id, graph_id, tenant_id, project_id, portal_id, screen_key, title, node_type,
    route_path, component_entry_path, fixture_ref, fixture_json, note, viewport_preset,
    pos_x, pos_y, version, layout_version, created_at, updated_at
)
SELECT
    fn.id,
    fn.graph_id,
    COALESCE(fg.tenant_id, p.tenant_id, 'local-default'),
    fn.project_id,
    COALESCE(fg.portal_key, 'default'),
    fn.screen_key,
    fn.title,
    'screen',
    COALESCE(NULLIF(fn.route_path, ''), '/'),
    '',
    COALESCE(NULLIF(fn.fixture_ref, ''), 'Default'),
    '{}',
    '',
    COALESCE(NULLIF(fn.viewport_preset, ''), 'desktop-1440'),
    0.0,
    0.0,
    1,
    1,
    fn.updated_at,
    fn.updated_at
FROM flow_nodes fn
JOIN projects p ON p.id = fn.project_id
LEFT JOIN flow_graphs_v3 fg ON fg.id = fn.graph_id;

CREATE TABLE flow_edges_v3 (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL REFERENCES flow_graphs(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_node_id TEXT NOT NULL,
    target_node_id TEXT NOT NULL,
    flow_type TEXT NOT NULL DEFAULT 'main' CHECK (flow_type IN ('main', 'if_else', 'if_not')),
    trigger_event TEXT NOT NULL DEFAULT 'onClick',
    guard_condition TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(project_id, source_node_id, target_node_id, trigger_event),
    CHECK (source_node_id != target_node_id),
    FOREIGN KEY (source_node_id, tenant_id, project_id) REFERENCES flow_nodes(id, tenant_id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (target_node_id, tenant_id, project_id) REFERENCES flow_nodes(id, tenant_id, project_id) ON DELETE CASCADE
);

INSERT INTO flow_edges_v3 (
    id, graph_id, tenant_id, project_id, source_node_id, target_node_id,
    flow_type, trigger_event, guard_condition, version, created_at, updated_at
)
SELECT
    fe.id,
    fe.graph_id,
    src.tenant_id,
    src.project_id,
    fe.from_node_id,
    fe.to_node_id,
    'main',
    COALESCE(NULLIF(fe.trigger_action, ''), 'onClick'),
    NULL,
    1,
    fe.updated_at,
    fe.updated_at
FROM flow_edges fe
JOIN flow_nodes_v3 src ON src.id = fe.from_node_id
JOIN flow_nodes_v3 dst ON dst.id = fe.to_node_id
    AND dst.tenant_id = src.tenant_id
    AND dst.project_id = src.project_id
WHERE fe.from_node_id != fe.to_node_id;

-- Drop legacy tables directly (NEVER RENAME old -> old_bak so screen_artifacts FK stays bound to flow_nodes(id))
DROP TABLE flow_edges;
DROP TABLE flow_nodes;
DROP TABLE flow_graphs;

ALTER TABLE flow_graphs_v3 RENAME TO flow_graphs;
ALTER TABLE flow_nodes_v3 RENAME TO flow_nodes;
ALTER TABLE flow_edges_v3 RENAME TO flow_edges;

CREATE UNIQUE INDEX IF NOT EXISTS flow_graphs_project_id_portal_key_key ON flow_graphs(project_id, portal_key);
CREATE INDEX IF NOT EXISTS flow_graphs_project_id_idx ON flow_graphs(project_id);
CREATE INDEX IF NOT EXISTS idx_flow_graphs_tenant_project ON flow_graphs(tenant_id, project_id);

CREATE INDEX IF NOT EXISTS idx_flow_nodes_tenant_project ON flow_nodes(tenant_id, project_id);
CREATE INDEX IF NOT EXISTS idx_flow_nodes_graph ON flow_nodes(graph_id);
CREATE INDEX IF NOT EXISTS flow_nodes_project_id_idx ON flow_nodes(project_id);

CREATE INDEX IF NOT EXISTS idx_flow_edges_tenant_project ON flow_edges(tenant_id, project_id);
CREATE INDEX IF NOT EXISTS idx_flow_edges_graph ON flow_edges(graph_id);

CREATE TABLE IF NOT EXISTS flow_draft_deployments (
    tenant_id TEXT NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    draft_id TEXT NOT NULL,
    request_sha256 TEXT NOT NULL,
    resulting_graph_version INTEGER NOT NULL,
    response_json TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, project_id, draft_id)
);
CREATE INDEX IF NOT EXISTS idx_flow_draft_deployments_expires ON flow_draft_deployments(expires_at);
