-- Epic 08 Party-Mode remediation V11 migration:
-- 1) Partial unique index preventing duplicate active/proposed knowledge cards per (tenant_id, project_id, content_sha256).
-- 2) Index on gate_reports(project_id, gate_name, target_id) for LEARN human-decision audit receipt lookup.
-- Additive only (preserves 45 tables and immutable V1..V10 checksums).

DELETE FROM knowledge_cards
WHERE status != 'REJECTED'
  AND rowid NOT IN (
      SELECT MIN(rowid)
      FROM knowledge_cards
      WHERE status != 'REJECTED'
      GROUP BY tenant_id, project_id, content_sha256
  );

CREATE UNIQUE INDEX IF NOT EXISTS uq_knowledge_cards_active_hash
    ON knowledge_cards(tenant_id, project_id, content_sha256)
    WHERE status != 'REJECTED';

CREATE INDEX IF NOT EXISTS idx_gate_reports_project_gate_target
    ON gate_reports(project_id, gate_name, target_id, timestamp DESC, id DESC);
