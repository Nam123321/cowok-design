-- V24__story_08_7_model_registry_and_studio_corrections.sql
-- Story 08.7 (ADR-009): Dynamic Model Registry (`model_registry_profiles`) and
-- Bridge 1 Manual Studio Correction Signals (`studio_correction_signals` -> V18a `gate_failure_ledger` at hit_count >= 3, HUMAN_ONLY).
-- Additive-only SQLite WAL migration (2 new tables, zero destructive edits).

CREATE TABLE IF NOT EXISTS model_registry_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    tier TEXT NOT NULL CHECK (tier IN ('tier1_frontier', 'tier2_fast', 'tier3_local')),
    context_window INTEGER NOT NULL CHECK (context_window >= 4096 AND context_window <= 2000000),
    supports_prompt_cache INTEGER NOT NULL DEFAULT 0 CHECK (supports_prompt_cache IN (0, 1)),
    cache_strategy TEXT NOT NULL DEFAULT 'none' CHECK (
        cache_strategy IN ('anthropic_ephemeral', 'gemini_cached_content', 'openai_cache_key', 'none')
    ),
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    updated_at BIGINT NOT NULL,
    UNIQUE(tenant_id, provider_id, model_id)
);

CREATE INDEX IF NOT EXISTS idx_model_registry_profiles_tenant_provider
    ON model_registry_profiles(tenant_id, provider_id, enabled, is_default DESC);

-- Seed canonical dynamic defaults across all 6 BYOK/BYOS providers (editable without binary rebuild)
INSERT OR IGNORE INTO model_registry_profiles (
    id, tenant_id, provider_id, model_id, display_name, tier, context_window,
    supports_prompt_cache, cache_strategy, is_default, enabled, updated_at
) VALUES
    ('seed-claude-sonnet', 'local-default', 'claude', 'claude-sonnet-4-5', 'Claude Sonnet 4.5', 'tier1_frontier', 200000, 1, 'anthropic_ephemeral', 1, 1, 1763960000000),
    ('seed-gemini-pro', 'local-default', 'gemini', 'gemini-2.5-pro', 'Gemini 2.5 Pro', 'tier1_frontier', 1000000, 1, 'gemini_cached_content', 1, 1, 1763960000000),
    ('seed-openai-gpt41', 'local-default', 'openai', 'gpt-4.1', 'GPT-4.1', 'tier1_frontier', 1000000, 1, 'openai_cache_key', 1, 1, 1763960000000),
    ('seed-openrouter-claude', 'local-default', 'openrouter', 'anthropic/claude-sonnet-4', 'OpenRouter Claude Sonnet 4', 'tier1_frontier', 200000, 1, 'openai_cache_key', 1, 1, 1763960000000),
    ('seed-deepseek-chat', 'local-default', 'deepseek', 'deepseek-chat', 'DeepSeek Chat', 'tier2_fast', 128000, 0, 'none', 1, 1, 1763960000000),
    ('seed-byos-qwen', 'local-default', 'byos', 'qwen2.5-coder:7b', 'Local Qwen 2.5 Coder 7B (BYOS)', 'tier3_local', 32768, 0, 'none', 1, 1, 1763960000000);

CREATE TABLE IF NOT EXISTS studio_correction_signals (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    correction_kind TEXT NOT NULL CHECK (
        correction_kind IN (
            'chart_type_override',
            'qa_autofix_click',
            'palette_contrast_fix',
            'density_mode_override',
            'callout_placement_fix'
        )
    ),
    signature_key TEXT NOT NULL,
    from_value TEXT,
    to_value TEXT,
    skill_id TEXT,
    hit_count INTEGER NOT NULL DEFAULT 1 CHECK (hit_count >= 1),
    escalated_ledger_id TEXT,
    first_seen_at BIGINT NOT NULL,
    last_seen_at BIGINT NOT NULL,
    UNIQUE(tenant_id, project_id, signature_key)
);

CREATE INDEX IF NOT EXISTS idx_studio_correction_signals_tenant_project
    ON studio_correction_signals(tenant_id, project_id, last_seen_at DESC);
