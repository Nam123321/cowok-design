-- V9__epic08_plugins_learn_cron.sql
-- Story 08.4 (AC-01, EC-E08-09, EC-E08-10, EC-LEARN-001):
-- 1. 12-Step Rebuild of `skill_plugins` with `CHECK(lifecycle_state IN (...))`,
--    deduplicated `'ENABLED' -> 'ACTIVE'` backfill, partial unique `ACTIVE` index,
--    and strict `tenant_id` + staging isolation columns.
-- 2. Durable `knowledge_cards`, `learn_consumed_nonces`, and `plugin_cron_schedules` tables.

DROP TABLE IF EXISTS skill_plugins_v9_new;

CREATE TABLE skill_plugins_v9_new (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    plugin_slug TEXT NOT NULL,
    version TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL CHECK (
        lifecycle_state IN ('DISCOVERED','VALIDATED','INSTALLED','ACTIVE','DEPRECATED','ARCHIVED')
    ),
    source_type TEXT NOT NULL DEFAULT 'local' CHECK (
        source_type IN ('builtin','local','github','upload')
    ),
    source_repo_url TEXT NOT NULL,
    upstream_repo_url TEXT,
    upstream_commit_sha TEXT,
    cron_schedule TEXT,
    skill_md_path TEXT NOT NULL DEFAULT 'SKILL.md',
    plugin_json_path TEXT NOT NULL DEFAULT 'plugin.json',
    manifest_json TEXT NOT NULL DEFAULT '{}',
    last_synced_at BIGINT,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision >= 1),
    staging_version TEXT,
    staging_manifest_json TEXT,
    staging_diff_reasons_json TEXT,
    staging_bundle_dir TEXT,
    staging_package_sha256 TEXT,
    breaking_change_isolated INTEGER NOT NULL DEFAULT 0 CHECK (breaking_change_isolated IN (0, 1)),
    quarantined INTEGER NOT NULL DEFAULT 0 CHECK (quarantined IN (0, 1)),
    quarantine_reason TEXT,
    updated_at BIGINT NOT NULL
);

INSERT INTO skill_plugins_v9_new (
    id,
    tenant_id,
    project_id,
    plugin_slug,
    version,
    lifecycle_state,
    source_type,
    source_repo_url,
    upstream_repo_url,
    upstream_commit_sha,
    cron_schedule,
    skill_md_path,
    plugin_json_path,
    manifest_json,
    last_synced_at,
    revision,
    staging_version,
    staging_manifest_json,
    staging_diff_reasons_json,
    staging_bundle_dir,
    staging_package_sha256,
    breaking_change_isolated,
    quarantined,
    quarantine_reason,
    updated_at
)
WITH resolved AS (
    SELECT
        sp.id,
        COALESCE(
            NULLIF(NULLIF(TRIM(sp.tenant_id), ''), 'local-default'),
            (SELECT pr.tenant_id FROM projects pr WHERE pr.id = sp.project_id),
            NULLIF(TRIM(sp.tenant_id), ''),
            'local-default'
        ) AS norm_tenant_id,
        sp.project_id,
        sp.plugin_slug,
        sp.version,
        UPPER(TRIM(sp.lifecycle_state)) AS upper_state,
        LOWER(TRIM(COALESCE(NULLIF(sp.source_type, ''), 'local'))) AS norm_source_type,
        sp.source_repo_url,
        sp.upstream_repo_url,
        sp.upstream_commit_sha,
        sp.cron_schedule,
        COALESCE(NULLIF(sp.skill_md_path, ''), 'SKILL.md') AS norm_skill_md_path,
        COALESCE(NULLIF(sp.plugin_json_path, ''), 'plugin.json') AS norm_plugin_json_path,
        COALESCE(NULLIF(sp.manifest_json, ''), '{}') AS norm_manifest_json,
        sp.last_synced_at,
        CASE WHEN COALESCE(sp.revision, 1) < 1 THEN 1 ELSE COALESCE(sp.revision, 1) END AS norm_revision,
        sp.staging_version,
        sp.staging_manifest_json,
        sp.staging_diff_reasons_json,
        sp.staging_bundle_dir,
        sp.staging_package_sha256,
        CASE WHEN COALESCE(sp.breaking_change_isolated, 0) != 0 THEN 1 ELSE 0 END AS norm_breaking,
        CASE WHEN COALESCE(sp.quarantined, 0) != 0 THEN 1 ELSE 0 END AS norm_quarantined,
        sp.quarantine_reason,
        sp.updated_at
    FROM skill_plugins sp
),
ranked AS (
    SELECT
        r.*,
        CASE
            WHEN r.upper_state IN ('ENABLED', 'ACTIVE') THEN
                ROW_NUMBER() OVER (
                    PARTITION BY r.norm_tenant_id, r.project_id, r.plugin_slug,
                        (CASE WHEN r.upper_state IN ('ENABLED', 'ACTIVE') THEN 1 ELSE 0 END)
                    ORDER BY r.updated_at DESC, r.id DESC
                )
            ELSE 0
        END AS active_rank
    FROM resolved r
)
SELECT
    id,
    norm_tenant_id,
    project_id,
    plugin_slug,
    version,
    CASE
        WHEN upper_state IN ('ENABLED', 'ACTIVE') AND active_rank = 1 THEN 'ACTIVE'
        WHEN upper_state IN ('ENABLED', 'ACTIVE') THEN 'DEPRECATED'
        WHEN upper_state IN ('DISCOVERED','VALIDATED','INSTALLED','DEPRECATED','ARCHIVED') THEN upper_state
        ELSE 'DEPRECATED'
    END,
    CASE
        WHEN norm_source_type IN ('builtin','local','github','upload') THEN norm_source_type
        ELSE 'local'
    END,
    source_repo_url,
    upstream_repo_url,
    upstream_commit_sha,
    cron_schedule,
    norm_skill_md_path,
    norm_plugin_json_path,
    norm_manifest_json,
    last_synced_at,
    norm_revision,
    staging_version,
    staging_manifest_json,
    staging_diff_reasons_json,
    staging_bundle_dir,
    staging_package_sha256,
    norm_breaking,
    norm_quarantined,
    quarantine_reason,
    updated_at
FROM ranked;

DROP TABLE skill_plugins;
ALTER TABLE skill_plugins_v9_new RENAME TO skill_plugins;

CREATE UNIQUE INDEX IF NOT EXISTS uq_skill_plugins_scope_slug_ver
    ON skill_plugins(tenant_id, project_id, plugin_slug, version);
CREATE UNIQUE INDEX IF NOT EXISTS uq_skill_plugins_active_slug
    ON skill_plugins(tenant_id, project_id, plugin_slug)
    WHERE lifecycle_state = 'ACTIVE';
CREATE INDEX IF NOT EXISTS skill_plugins_project_id_idx
    ON skill_plugins(project_id);
CREATE INDEX IF NOT EXISTS idx_skill_plugins_tenant_project
    ON skill_plugins(tenant_id, project_id, lifecycle_state);

CREATE TRIGGER IF NOT EXISTS trg_skill_plugins_sync_tenant_id
AFTER INSERT ON skill_plugins
WHEN NEW.tenant_id = 'local-default'
BEGIN
    UPDATE skill_plugins
    SET tenant_id = COALESCE(
        (SELECT pr.tenant_id FROM projects pr WHERE pr.id = NEW.project_id),
        NEW.tenant_id
    )
    WHERE id = NEW.id;
END;

CREATE TABLE IF NOT EXISTS knowledge_cards (
    id TEXT NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    idempotency_key TEXT,
    category TEXT NOT NULL CHECK (category IN ('RULE','ANTI_PATTERN','PATTERN','LESSON')),
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

CREATE UNIQUE INDEX IF NOT EXISTS uq_knowledge_cards_idempotency
    ON knowledge_cards(tenant_id, project_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_knowledge_cards_scope_status
    ON knowledge_cards(tenant_id, project_id, status, created_at, id);

CREATE TABLE IF NOT EXISTS learn_consumed_nonces (
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    nonce TEXT NOT NULL,
    expires_at_s BIGINT NOT NULL CHECK (expires_at_s > 0),
    consumed_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, project_id, nonce)
);

CREATE INDEX IF NOT EXISTS idx_learn_consumed_nonces_expiry
    ON learn_consumed_nonces(expires_at_s);

CREATE TABLE IF NOT EXISTS plugin_cron_schedules (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL REFERENCES skill_plugins(id) ON DELETE CASCADE,
    cron_expression TEXT NOT NULL,
    upstream_repo_url TEXT NOT NULL,
    last_etag TEXT,
    last_commit_sha TEXT,
    lease_owner TEXT,
    fencing_token BIGINT NOT NULL DEFAULT 0 CHECK (fencing_token >= 0),
    stale_after_timestamp BIGINT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
    next_retry_at BIGINT,
    sync_status TEXT NOT NULL DEFAULT 'IDLE' CHECK (
        sync_status IN ('IDLE','RUNNING','SUCCEEDED','BACKOFF','AUTH_REQUIRED','FAILED')
    ),
    last_error_code TEXT,
    last_error_message TEXT,
    updated_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_plugin_cron_schedules_scope_plugin
    ON plugin_cron_schedules(tenant_id, project_id, plugin_id);
CREATE INDEX IF NOT EXISTS idx_plugin_cron_schedules_lease
    ON plugin_cron_schedules(sync_status, stale_after_timestamp, next_retry_at);
