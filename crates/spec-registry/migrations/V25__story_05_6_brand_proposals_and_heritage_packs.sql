-- V25__story_05_6_brand_proposals_and_heritage_packs.sql
-- Story 05.6: Phase 2 Brand & Component Visual Studio, User-Driven themeModes,
-- Bidirectional Hidden Spec Drawer OCC mirror, and Cross-Project Heritage Design Packs.
-- Additive migration sequential after V24; composite tenant_id isolation enforced.

CREATE TABLE IF NOT EXISTS brand_proposal_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    proposal_id TEXT NOT NULL,
    proposal_json TEXT NOT NULL,
    theme_modes_json TEXT NOT NULL DEFAULT '[]',
    is_selected INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_brand_proposal_snapshots_tenant_proj
    ON brand_proposal_snapshots(tenant_id, project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS heritage_design_packs (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    name TEXT NOT NULL,
    source_project_id TEXT NOT NULL,
    entry_mode TEXT NOT NULL DEFAULT 'GREENFIELD' CHECK (
        entry_mode IN ('GREENFIELD','ADD_FEATURE','REVAMP','MICRO_POLISH','DECK_FROM_APP')
    ),
    brand_identity_json TEXT NOT NULL DEFAULT '{}',
    component_registry_json TEXT NOT NULL DEFAULT '{}',
    presentation_theme_json TEXT NOT NULL DEFAULT '{}',
    theme_modes_json TEXT NOT NULL DEFAULT '[]',
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_heritage_design_packs_tenant_src
    ON heritage_design_packs(tenant_id, source_project_id, created_at DESC);

CREATE TABLE IF NOT EXISTS hidden_spec_documents (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    spec_key TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    content_sha256 TEXT NOT NULL,
    content_markdown TEXT NOT NULL DEFAULT '',
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id, spec_key)
);

CREATE INDEX IF NOT EXISTS idx_hidden_spec_documents_tenant_proj
    ON hidden_spec_documents(tenant_id, project_id, spec_key);
