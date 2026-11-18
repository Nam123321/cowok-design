-- V9__epic05_storyboard_proposals_handoff.sql
-- Story 05.1: Idempotency records ledger, flow_edges rebuild with loopback flow_type,
-- max_loop_iterations (1..=10), source_cta_ff_id, and revision.

-- 1. Two-phase idempotency records ledger
CREATE TABLE IF NOT EXISTS idempotency_records (
    tenant_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_sha256 TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('IN_PROGRESS','COMPLETED')),
    lease_expires_at BIGINT NOT NULL,
    response_status INTEGER,
    response_body_json TEXT,
    expires_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, actor_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_idempotency_records_expires_at
    ON idempotency_records(expires_at);

-- 2. 12-step FK-preserving rebuild of flow_edges with loopback support & V9 columns
CREATE TABLE flow_edges_v9 (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL REFERENCES flow_graphs(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_node_id TEXT NOT NULL,
    target_node_id TEXT NOT NULL,
    flow_type TEXT NOT NULL DEFAULT 'main' CHECK (flow_type IN ('main', 'if_else', 'if_not', 'loopback')),
    trigger_event TEXT NOT NULL DEFAULT 'onClick',
    guard_condition TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1),
    source_cta_ff_id TEXT CHECK(source_cta_ff_id IS NULL OR length(trim(source_cta_ff_id)) > 0),
    max_loop_iterations INTEGER NOT NULL DEFAULT 3 CHECK(max_loop_iterations BETWEEN 1 AND 10),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    UNIQUE(project_id, source_node_id, target_node_id, trigger_event),
    CHECK (source_node_id != target_node_id),
    FOREIGN KEY (source_node_id, tenant_id, project_id) REFERENCES flow_nodes(id, tenant_id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (target_node_id, tenant_id, project_id) REFERENCES flow_nodes(id, tenant_id, project_id) ON DELETE CASCADE
);

INSERT INTO flow_edges_v9 (
    id,
    graph_id,
    tenant_id,
    project_id,
    source_node_id,
    target_node_id,
    flow_type,
    trigger_event,
    guard_condition,
    version,
    source_cta_ff_id,
    max_loop_iterations,
    revision,
    created_at,
    updated_at
)
SELECT
    id,
    graph_id,
    tenant_id,
    project_id,
    source_node_id,
    target_node_id,
    flow_type,
    trigger_event,
    guard_condition,
    version,
    NULL,
    3,
    1,
    created_at,
    updated_at
FROM flow_edges;

DROP TABLE flow_edges;
ALTER TABLE flow_edges_v9 RENAME TO flow_edges;

CREATE INDEX IF NOT EXISTS idx_flow_edges_tenant_project ON flow_edges(tenant_id, project_id);
CREATE INDEX IF NOT EXISTS idx_flow_edges_graph ON flow_edges(graph_id);
