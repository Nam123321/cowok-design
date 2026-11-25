-- V30__story_07_2_split_export_audit.sql
-- Story 07.2: Split Export & Packaging Engine (Share & Present vs Developer Handoff)
-- Persists tenant-scoped SHA-256 checksums, zero-CDN audit verdicts, byte size bounds (<= 25 MiB),
-- QC gate override receipts, and plugin status for offline HTML, Vector PDF, Plugin PPTX & Developer Handoff exports.

CREATE TABLE IF NOT EXISTS export_bundle_runs (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    audience_mode TEXT NOT NULL CHECK (audience_mode IN ('SHARE_PRESENT', 'DEVELOPER_HANDOFF')),
    format TEXT NOT NULL CHECK (format IN ('OFFLINE_HTML', 'VECTOR_PDF', 'PLUGIN_PPTX', 'DEV_HANDOFF_BUNDLE')),
    file_name TEXT NOT NULL,
    artifact_sha256 TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    external_cdn_refs_count INTEGER NOT NULL DEFAULT 0,
    qc_gate_status TEXT NOT NULL,
    qc_override_acknowledged INTEGER NOT NULL DEFAULT 0 CHECK (qc_override_acknowledged IN (0, 1)),
    plugin_status TEXT NOT NULL DEFAULT 'NOT_APPLICABLE',
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_export_bundle_runs_tenant_project_created
    ON export_bundle_runs (tenant_id, project_id, created_at DESC);
