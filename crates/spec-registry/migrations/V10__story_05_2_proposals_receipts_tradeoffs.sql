-- V10__story_05_2_proposals_receipts_tradeoffs.sql
-- Story 05.2: Additive ChangeProposal scoping, multi-node target mapping,
-- HMAC-signed gate receipts, visual diff payloads, and inline tradeoff badges.

CREATE INDEX IF NOT EXISTS idx_change_proposals_project_node
    ON change_proposals(project_id, node_id);

-- 1. Multi-node target mapping with composite FK to flow_nodes(id, tenant_id, project_id)
CREATE TABLE IF NOT EXISTS proposal_target_nodes (
    proposal_id TEXT NOT NULL REFERENCES change_proposals(id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    PRIMARY KEY (proposal_id, node_id),
    FOREIGN KEY (node_id, tenant_id, project_id) REFERENCES flow_nodes(id, tenant_id, project_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_proposal_target_nodes_lookup
    ON proposal_target_nodes(tenant_id, project_id, node_id);

-- 2. Cryptographically signed gate receipts bound to diff, constraints, and workspace HEAD
CREATE TABLE IF NOT EXISTS proposal_gate_receipts (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    proposal_id TEXT NOT NULL REFERENCES change_proposals(id) ON DELETE CASCADE,
    diff_sha256 TEXT NOT NULL,
    constraints_sha256 TEXT NOT NULL,
    workspace_head_sha256 TEXT NOT NULL,
    checks_json TEXT NOT NULL DEFAULT '[]',
    exit_code INTEGER NOT NULL,
    receipt_hmac TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_proposal_gate_receipts_proposal
    ON proposal_gate_receipts(proposal_id, created_at DESC);

-- 3. Inline numbered tradeoff badges (1..=12) per proposal
CREATE TABLE IF NOT EXISTS proposal_tradeoffs (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    proposal_id TEXT NOT NULL REFERENCES change_proposals(id) ON DELETE CASCADE,
    node_id TEXT,
    element_ff_id TEXT NOT NULL,
    badge_index INTEGER NOT NULL CHECK (badge_index BETWEEN 1 AND 12),
    design_note TEXT NOT NULL DEFAULT '',
    instead_label TEXT NOT NULL DEFAULT '',
    instead_patch_json TEXT NOT NULL DEFAULT 'null',
    decision TEXT NOT NULL DEFAULT 'pending' CHECK (decision IN ('pending', 'agreed', 'disagreed_instead', 'asked', 'skipped')),
    ask_prompt TEXT,
    decided_at TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(proposal_id, badge_index)
);

CREATE INDEX IF NOT EXISTS idx_proposal_tradeoffs_proposal
    ON proposal_tradeoffs(proposal_id, badge_index);
