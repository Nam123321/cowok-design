-- Epic 08 audit remediation V10 migration: align knowledge_cards CHECK constraint with LearnCardCategory::Flow ('FLOW').
-- Safe to run on top of V9 without modifying immutable V9 checksums; preserves table count (45) and all partial indexes.

DROP TABLE IF EXISTS knowledge_cards_v10_new;

CREATE TABLE knowledge_cards_v10_new (
    id TEXT NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    idempotency_key TEXT,
    category TEXT NOT NULL CHECK (category IN ('RULE','ANTI_PATTERN','PATTERN','LESSON','FLOW')),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    source_evidence TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('PROPOSED','ACCEPTED','REJECTED')),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision >= 1),
    reviewed_by TEXT,
    decision_reason TEXT,
    created_at BIGINT NOT NULL,
    accepted_at BIGINT,
    PRIMARY KEY (tenant_id, project_id, id)
);

INSERT INTO knowledge_cards_v10_new (
    id, tenant_id, project_id, idempotency_key, category, title, content, content_sha256,
    source_evidence, status, revision, reviewed_by, decision_reason, created_at, accepted_at
)
SELECT
    id, tenant_id, project_id, idempotency_key, category, title, content, content_sha256,
    source_evidence, status, revision, reviewed_by, decision_reason, created_at, accepted_at
FROM knowledge_cards;

DROP TABLE knowledge_cards;
ALTER TABLE knowledge_cards_v10_new RENAME TO knowledge_cards;

CREATE UNIQUE INDEX IF NOT EXISTS uq_knowledge_cards_idempotency
    ON knowledge_cards(tenant_id, project_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_knowledge_cards_scope_status
    ON knowledge_cards(tenant_id, project_id, status, created_at, id);
