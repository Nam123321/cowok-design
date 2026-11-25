-- @story 14.3
-- @implements AC-01, AC-02, AC-03, AC-04, AC-05 (V32 Additive SQLite WAL DDL for 4-Channel Distribution Ops, Plugin Catalog Lifecycle, A/B Experiments, Global Quarantine & Offline Licenses)

CREATE TABLE IF NOT EXISTS cloud_release_rollouts (
    id TEXT PRIMARY KEY NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('cli', 'desktop', 'plugin', 'docker')),
    version TEXT NOT NULL,
    version_seq BIGINT NOT NULL,
    rollout_percent INTEGER NOT NULL CHECK (rollout_percent BETWEEN 0 AND 100),
    kill_switch INTEGER NOT NULL DEFAULT 0 CHECK (kill_switch IN (0, 1)),
    announcement_json TEXT NOT NULL DEFAULT '{}',
    download_url TEXT NOT NULL,
    artifact_sha256 TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    published_by TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_plugin_catalog_entries (
    plugin_id TEXT PRIMARY KEY NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('DEFAULT_BUILTIN', 'RECOMMENDED_GITHUB')),
    lifecycle_status TEXT NOT NULL CHECK (lifecycle_status IN ('ACTIVE', 'INACTIVE', 'DEPRECATED', 'QUARANTINED', 'PROCESSING', 'PENDING_APPROVAL')),
    repo_url TEXT,
    pinned_commit_sha TEXT,
    min_app_version TEXT NOT NULL DEFAULT '0.1.0',
    default_enabled INTEGER NOT NULL DEFAULT 1 CHECK (default_enabled IN (0, 1)),
    default_config_json TEXT NOT NULL DEFAULT '{}',
    title_i18n_json TEXT NOT NULL DEFAULT '{}',
    summary_i18n_json TEXT NOT NULL DEFAULT '{}',
    manifest_seq BIGINT NOT NULL DEFAULT 1,
    updated_by TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_ab_experiments (
    experiment_key TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    target_surface TEXT NOT NULL CHECK (target_surface IN ('ONBOARDING_DEFAULTS', 'PLUGIN_RANKING', 'UX_COPY_LAYOUT')),
    status TEXT NOT NULL CHECK (status IN ('DRAFT', 'RUNNING', 'PAUSED', 'KILLED', 'COMPLETED')),
    variants_json TEXT NOT NULL,
    kill_switch INTEGER NOT NULL DEFAULT 0 CHECK (kill_switch IN (0, 1)),
    manifest_seq BIGINT NOT NULL DEFAULT 1,
    updated_by TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_skill_advisories (
    id TEXT PRIMARY KEY NOT NULL,
    skill_or_plugin_id TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('CRITICAL', 'HIGH', 'MEDIUM')),
    action TEXT NOT NULL CHECK (action IN ('QUARANTINE', 'WARN', 'REVOKE_QUARANTINE')),
    reason TEXT NOT NULL,
    affected_sha256 TEXT,
    advisory_seq BIGINT NOT NULL,
    issued_by TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cloud_offline_licenses (
    license_id TEXT PRIMARY KEY NOT NULL,
    tier TEXT NOT NULL CHECK (tier IN ('FREE', 'PRO', 'TEAM', 'ENTERPRISE')),
    subject_org TEXT NOT NULL,
    cowok_org_id TEXT,
    max_seats INTEGER NOT NULL DEFAULT 1,
    issued_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    signed_token TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0 CHECK (revoked IN (0, 1)),
    issued_by TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_release_rollouts_channel_seq
    ON cloud_release_rollouts (channel, version_seq DESC);
CREATE INDEX IF NOT EXISTS idx_cloud_skill_advisories_seq
    ON cloud_skill_advisories (advisory_seq DESC);
CREATE INDEX IF NOT EXISTS idx_cloud_offline_licenses_org
    ON cloud_offline_licenses (cowok_org_id, issued_at DESC);
