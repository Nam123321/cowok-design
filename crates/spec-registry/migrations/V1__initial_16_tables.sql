-- V1__initial_16_tables.sql
-- Deterministic 16-Table SQLite WAL Genesis Schema for cowok-design (crates/spec-registry)

-- 1. projects
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    workspace_root TEXT NOT NULL DEFAULT '.',
    product_brief TEXT NOT NULL DEFAULT '',
    inherit_brand_from_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'ACTIVE',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS projects_slug_key ON projects(slug);
CREATE INDEX IF NOT EXISTS projects_inherit_brand_idx ON projects(inherit_brand_from_project_id);
CREATE UNIQUE INDEX IF NOT EXISTS projects_workspace_root_unique_idx ON projects(workspace_root) WHERE workspace_root != '.';

-- 2. project_specs
CREATE TABLE IF NOT EXISTS project_specs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_key TEXT NOT NULL,
    file_path TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'CONFIRMED',
    version INTEGER NOT NULL DEFAULT 1 CHECK(version >= 1),
    content_sha256 TEXT NOT NULL DEFAULT '',
    summary_digest TEXT NOT NULL DEFAULT '',
    provenance_summary_json TEXT NOT NULL DEFAULT '{}',
    source_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    source_revision_sha256 TEXT,
    created_at BIGINT NOT NULL DEFAULT 0,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS project_specs_project_id_spec_key_key ON project_specs(project_id, spec_key);
CREATE INDEX IF NOT EXISTS project_specs_project_id_idx ON project_specs(project_id);
CREATE INDEX IF NOT EXISTS project_specs_project_sha_idx ON project_specs(project_id, content_sha256);

-- 3. spec_revisions
CREATE TABLE IF NOT EXISTS spec_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    spec_id TEXT NOT NULL REFERENCES project_specs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_key TEXT NOT NULL,
    revision_number INTEGER NOT NULL CHECK(revision_number >= 1),
    operation_id TEXT NOT NULL,
    previous_sha256 TEXT,
    content_hash TEXT NOT NULL,
    actor_kind TEXT NOT NULL DEFAULT 'SYSTEM',
    actor_id TEXT NOT NULL DEFAULT 'system',
    change_summary TEXT NOT NULL DEFAULT '',
    source_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    source_revision_sha256 TEXT,
    created_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS spec_revisions_spec_id_revision_number_key ON spec_revisions(spec_id, revision_number);
CREATE UNIQUE INDEX IF NOT EXISTS spec_revisions_spec_id_operation_id_key ON spec_revisions(spec_id, operation_id);
CREATE INDEX IF NOT EXISTS spec_revisions_spec_id_idx ON spec_revisions(spec_id);
CREATE INDEX IF NOT EXISTS spec_revisions_project_spec_ver_idx ON spec_revisions(project_id, spec_key, revision_number);

-- 4. constraint_profiles
CREATE TABLE IF NOT EXISTS constraint_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    preset_profile TEXT NOT NULL,
    styling_policy TEXT NOT NULL,
    forbid_tailwind INTEGER NOT NULL DEFAULT 1,
    forbid_raw_hex INTEGER NOT NULL DEFAULT 1,
    tokens_css_path TEXT NOT NULL,
    icon_manifest_path TEXT NOT NULL,
    allowed_icon_set_json TEXT NOT NULL,
    typography_contract_json TEXT NOT NULL,
    min_wcag_contrast REAL NOT NULL DEFAULT 4.5,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS constraint_profiles_project_id_preset_profile_key ON constraint_profiles(project_id, preset_profile);
CREATE INDEX IF NOT EXISTS constraint_profiles_project_id_idx ON constraint_profiles(project_id);

-- 5. constraint_rules
CREATE TABLE IF NOT EXISTS constraint_rules (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES constraint_profiles(id) ON DELETE CASCADE,
    rule_code TEXT NOT NULL,
    severity TEXT NOT NULL,
    config_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS constraint_rules_profile_id_rule_code_key ON constraint_rules(profile_id, rule_code);
CREATE INDEX IF NOT EXISTS constraint_rules_profile_id_idx ON constraint_rules(profile_id);

-- 6. flow_graphs
CREATE TABLE IF NOT EXISTS flow_graphs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    portal_key TEXT NOT NULL,
    title TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS flow_graphs_project_id_portal_key_key ON flow_graphs(project_id, portal_key);
CREATE INDEX IF NOT EXISTS flow_graphs_project_id_idx ON flow_graphs(project_id);

-- 7. flow_nodes
CREATE TABLE IF NOT EXISTS flow_nodes (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL REFERENCES flow_graphs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    screen_key TEXT NOT NULL,
    title TEXT NOT NULL,
    route_path TEXT NOT NULL,
    fixture_ref TEXT NOT NULL,
    viewport_preset TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS flow_nodes_graph_id_screen_key_key ON flow_nodes(graph_id, screen_key);
CREATE INDEX IF NOT EXISTS flow_nodes_graph_id_idx ON flow_nodes(graph_id);
CREATE INDEX IF NOT EXISTS flow_nodes_project_id_idx ON flow_nodes(project_id);

-- 8. flow_edges
CREATE TABLE IF NOT EXISTS flow_edges (
    id TEXT PRIMARY KEY NOT NULL,
    graph_id TEXT NOT NULL REFERENCES flow_graphs(id) ON DELETE CASCADE,
    from_node_id TEXT NOT NULL REFERENCES flow_nodes(id) ON DELETE CASCADE,
    to_node_id TEXT NOT NULL REFERENCES flow_nodes(id) ON DELETE CASCADE,
    trigger_action TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS flow_edges_graph_from_to_action_key ON flow_edges(graph_id, from_node_id, to_node_id, trigger_action);
CREATE INDEX IF NOT EXISTS flow_edges_graph_id_idx ON flow_edges(graph_id);

-- 9. screen_artifacts
CREATE TABLE IF NOT EXISTS screen_artifacts (
    id TEXT PRIMARY KEY NOT NULL,
    node_id TEXT NOT NULL REFERENCES flow_nodes(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    artifact_path TEXT NOT NULL,
    anchor_status TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS screen_artifacts_node_id_content_hash_key ON screen_artifacts(node_id, content_hash);
CREATE INDEX IF NOT EXISTS screen_artifacts_node_id_idx ON screen_artifacts(node_id);

-- 10. presentation_decks
CREATE TABLE IF NOT EXISTS presentation_decks (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    theme_preset TEXT NOT NULL,
    slides_count INTEGER NOT NULL DEFAULT 0,
    last_exported_at BIGINT,
    updated_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS presentation_decks_project_id_idx ON presentation_decks(project_id);

-- 11. deck_slides
CREATE TABLE IF NOT EXISTS deck_slides (
    id TEXT PRIMARY KEY NOT NULL,
    deck_id TEXT NOT NULL REFERENCES presentation_decks(id) ON DELETE CASCADE,
    slide_index INTEGER NOT NULL CHECK(slide_index >= 0),
    layout_kind TEXT NOT NULL,
    html_path TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS deck_slides_deck_id_slide_index_key ON deck_slides(deck_id, slide_index);
CREATE INDEX IF NOT EXISTS deck_slides_deck_id_idx ON deck_slides(deck_id);

-- 12. wording_slots
CREATE TABLE IF NOT EXISTS wording_slots (
    id TEXT PRIMARY KEY NOT NULL,
    slide_id TEXT NOT NULL REFERENCES deck_slides(id) ON DELETE CASCADE,
    ff_id TEXT NOT NULL,
    slot_label TEXT NOT NULL,
    text_value TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS wording_slots_slide_id_ff_id_key ON wording_slots(slide_id, ff_id);
CREATE INDEX IF NOT EXISTS wording_slots_slide_id_idx ON wording_slots(slide_id);

-- 13. skill_plugins
CREATE TABLE IF NOT EXISTS skill_plugins (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    plugin_slug TEXT NOT NULL,
    version TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    source_repo_url TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS skill_plugins_project_slug_ver_key ON skill_plugins(project_id, plugin_slug, version);
CREATE INDEX IF NOT EXISTS skill_plugins_project_id_idx ON skill_plugins(project_id);

-- 14. model_tournaments
CREATE TABLE IF NOT EXISTS model_tournaments (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    prompt_digest TEXT NOT NULL,
    candidates_json TEXT NOT NULL,
    winner_model TEXT,
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS model_tournaments_project_id_idx ON model_tournaments(project_id);

-- 15. bot_jobs
CREATE TABLE IF NOT EXISTS bot_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_type TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    enqueued_at BIGINT NOT NULL,
    completed_at BIGINT
);
CREATE INDEX IF NOT EXISTS bot_jobs_project_id_idx ON bot_jobs(project_id);

-- 16. gate_reports
CREATE TABLE IF NOT EXISTS gate_reports (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    gate_name TEXT NOT NULL,
    passed INTEGER NOT NULL,
    checker_id TEXT NOT NULL,
    evidence_hash TEXT NOT NULL,
    timestamp BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS gate_reports_project_id_idx ON gate_reports(project_id);

-- Infrastructure Recovery Journal: spec_write_intents
CREATE TABLE IF NOT EXISTS spec_write_intents (
    operation_id TEXT PRIMARY KEY NOT NULL,
    spec_id TEXT NOT NULL REFERENCES project_specs(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    spec_key TEXT NOT NULL,
    expected_version INTEGER NOT NULL,
    old_sha256 TEXT,
    new_sha256 TEXT NOT NULL,
    actor_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    change_summary TEXT NOT NULL,
    provenance_summary_json TEXT NOT NULL,
    source_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    source_revision_sha256 TEXT,
    state TEXT NOT NULL CHECK(state IN ('PENDING', 'COMMITTED', 'ABORTED')),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS spec_write_intents_spec_id_operation_id_key ON spec_write_intents(spec_id, operation_id);
CREATE INDEX IF NOT EXISTS spec_write_intents_state_created_at_idx ON spec_write_intents(state, created_at);
