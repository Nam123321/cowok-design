-- V22__story_05_5_flow_node_substates.sql
-- Story 05.5: 3-Tier Interactive Flow DAG Synthesizer, [Show Edge-Case Flows] Lens & 4-SubState Node Pills.
-- Persists per-node 4-SubState (DEFAULT/LOADING/EMPTY/ERROR) fixture bindings, flow_tier (PRIMARY/EDGE_CASE),
-- cluster_id (<=8 default happy-path clusters), and VerdictShield discrete severity_band (0 raw floats).

CREATE TABLE IF NOT EXISTS flow_node_substates (
    node_id TEXT NOT NULL REFERENCES flow_nodes(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    substate TEXT NOT NULL CHECK (substate IN ('DEFAULT','LOADING','EMPTY','ERROR')),
    fixture_id TEXT NOT NULL REFERENCES screen_fixtures(id) ON DELETE CASCADE,
    flow_tier TEXT NOT NULL DEFAULT 'PRIMARY' CHECK (flow_tier IN ('PRIMARY','EDGE_CASE')),
    cluster_id TEXT NOT NULL DEFAULT 'cluster-1',
    severity_band TEXT NOT NULL DEFAULT 'LOW' CHECK (severity_band IN ('LOW','MEDIUM','HIGH','CRITICAL')),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (node_id, substate)
);

CREATE INDEX IF NOT EXISTS idx_flow_node_substates_project_tier
    ON flow_node_substates(project_id, flow_tier);
