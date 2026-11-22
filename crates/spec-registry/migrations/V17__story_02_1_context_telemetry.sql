-- V17__story_02_1_context_telemetry.sql
-- Story 02.1: Persist portal-safe context slicing, KV-cache static-prefix SHA-256,
-- BPE token budgets, idempotency replay receipts, ReasonCode outcomes, and 3-Profile adapter telemetry.

CREATE TABLE IF NOT EXISTS context_assembly_telemetry (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    idempotency_key TEXT,
    request_fingerprint_sha256 TEXT NOT NULL DEFAULT '',
    portal_id TEXT NOT NULL CHECK (
        portal_id IN ('SuperAdmin','TenantAdmin','SalesApp','MarketingWeb','DeveloperPortal','Presentation')
    ),
    horizon TEXT NOT NULL CHECK (horizon IN ('short','mid','long')),
    model_profile TEXT NOT NULL CHECK (
        model_profile IN ('A','B','C','ProfileA_Frontier','ProfileB_LocalSmall','ProfileC_OfflineFTS5')
    ),
    adapter_mode TEXT NOT NULL DEFAULT 'frontier_full' CHECK (
        adapter_mode IN ('frontier_full','local_reduced','offline_fts5')
    ),
    prefix_sha256 TEXT NOT NULL,
    static_prefix_tokens INTEGER NOT NULL DEFAULT 0 CHECK (static_prefix_tokens >= 0),
    dynamic_tail_tokens INTEGER NOT NULL DEFAULT 0 CHECK (dynamic_tail_tokens >= 0),
    outline_tokens INTEGER NOT NULL CHECK (outline_tokens >= 0),
    total_text_tokens INTEGER NOT NULL CHECK (total_text_tokens >= 0),
    image_tokens INTEGER NOT NULL DEFAULT 0 CHECK (image_tokens >= 0),
    ssot_dom_tokens INTEGER NOT NULL DEFAULT 0 CHECK (ssot_dom_tokens >= 0),
    non_ssot_tokens INTEGER NOT NULL DEFAULT 0 CHECK (non_ssot_tokens >= 0),
    ssot_dom_ratio REAL NOT NULL DEFAULT 1.0 CHECK (ssot_dom_ratio >= 0.0 AND ssot_dom_ratio <= 1.0),
    sources_requested INTEGER NOT NULL DEFAULT 0 CHECK (sources_requested >= 0),
    sources_included INTEGER NOT NULL DEFAULT 0 CHECK (sources_included >= 0),
    sources_pruned INTEGER NOT NULL DEFAULT 0 CHECK (sources_pruned >= 0),
    reason_code TEXT NOT NULL CHECK (
        reason_code IN ('atomic_complete','pruned_by_budget','cross_portal_leak_blocked','atomic_block_over_budget')
    ),
    response_json TEXT NOT NULL DEFAULT '{}',
    latency_ms INTEGER NOT NULL CHECK (latency_ms >= 0),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ctx_telemetry_proj_portal
    ON context_assembly_telemetry(project_id, portal_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_ctx_telemetry_tenant_proj
    ON context_assembly_telemetry(tenant_id, project_id, created_at DESC, id DESC);
CREATE UNIQUE INDEX IF NOT EXISTS uq_ctx_telemetry_idem
    ON context_assembly_telemetry(tenant_id, project_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
