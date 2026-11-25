-- @story 14.2
-- @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Cloud Ingestion API & SuperAdmin Portal MVP — V29 Additive DDL)

CREATE TABLE IF NOT EXISTS cloud_support_tickets (
    id TEXT PRIMARY KEY NOT NULL,
    install_id TEXT NOT NULL,
    client_ticket_id TEXT,
    category TEXT NOT NULL CHECK (category IN ('bug','feature_request','ux_friction','billing','other')),
    priority TEXT NOT NULL DEFAULT 'P2' CHECK (priority IN ('P0','P1','P2','P3')),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','triaged','replied','resolved','closed')),
    title TEXT NOT NULL,
    description_sanitized TEXT NOT NULL,
    diagnostic_bundle_json TEXT NOT NULL DEFAULT '{}',
    attachment_sha256 TEXT,
    attachment_mime TEXT,
    attachment_bytes INTEGER,
    cluster_id TEXT,
    channel TEXT NOT NULL DEFAULT 'cli',
    host_client TEXT NOT NULL DEFAULT 'standalone',
    app_version TEXT NOT NULL DEFAULT '0.1.0',
    idempotency_key TEXT UNIQUE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_support_tickets_status_priority
    ON cloud_support_tickets(status, priority, updated_at);
CREATE INDEX IF NOT EXISTS idx_cloud_support_tickets_install_id
    ON cloud_support_tickets(install_id, created_at);
CREATE INDEX IF NOT EXISTS idx_cloud_support_tickets_cluster_id
    ON cloud_support_tickets(cluster_id);

CREATE TABLE IF NOT EXISTS cloud_ticket_replies (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_id TEXT NOT NULL REFERENCES cloud_support_tickets(id) ON DELETE CASCADE,
    author_actor TEXT NOT NULL,
    author_role TEXT NOT NULL,
    body_sanitized TEXT NOT NULL,
    synced_to_client INTEGER NOT NULL DEFAULT 0 CHECK (synced_to_client IN (0, 1)),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_ticket_replies_ticket_id
    ON cloud_ticket_replies(ticket_id, created_at);

CREATE TABLE IF NOT EXISTS cloud_feature_clusters (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    ticket_count INTEGER NOT NULL DEFAULT 0,
    github_issue_url TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','planned','shipped','declined')),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_feature_clusters_status_updated
    ON cloud_feature_clusters(status, updated_at);

CREATE TABLE IF NOT EXISTS cloud_telemetry_events (
    id TEXT PRIMARY KEY NOT NULL,
    install_id TEXT NOT NULL,
    event_name TEXT NOT NULL,
    funnel_stage TEXT,
    channel TEXT NOT NULL DEFAULT 'cli',
    host_client TEXT NOT NULL DEFAULT 'standalone',
    app_version TEXT NOT NULL DEFAULT '0.1.0',
    properties_json TEXT NOT NULL DEFAULT '{}',
    occurred_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_telemetry_events_occurred_at
    ON cloud_telemetry_events(occurred_at);
CREATE INDEX IF NOT EXISTS idx_cloud_telemetry_events_install_stage
    ON cloud_telemetry_events(install_id, funnel_stage, occurred_at);
CREATE INDEX IF NOT EXISTS idx_cloud_telemetry_events_channel_host
    ON cloud_telemetry_events(channel, host_client);

CREATE TABLE IF NOT EXISTS cloud_crash_groups (
    fingerprint TEXT PRIMARY KEY NOT NULL,
    app_version TEXT NOT NULL,
    error_signature TEXT NOT NULL,
    sample_stack_sanitized TEXT NOT NULL,
    occurrence_count INTEGER NOT NULL DEFAULT 1,
    affected_installs INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','investigating','resolved')),
    first_seen_at BIGINT NOT NULL,
    last_seen_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_crash_groups_version_last_seen
    ON cloud_crash_groups(app_version, last_seen_at);

CREATE TABLE IF NOT EXISTS admin_audit_logs (
    id TEXT PRIMARY KEY NOT NULL,
    actor_subject TEXT NOT NULL,
    actor_role TEXT NOT NULL CHECK (actor_role IN ('SUPER_ADMIN','SUPPORT_AGENT','SKILL_CURATOR','ANALYST_READONLY')),
    mfa_verified INTEGER NOT NULL DEFAULT 1 CHECK (mfa_verified = 1),
    action TEXT NOT NULL,
    target_resource TEXT NOT NULL,
    details_json TEXT NOT NULL DEFAULT '{}',
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_created_at
    ON admin_audit_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_admin_audit_logs_actor_action
    ON admin_audit_logs(actor_subject, action, created_at);

CREATE TRIGGER IF NOT EXISTS trg_admin_audit_logs_no_update
BEFORE UPDATE ON admin_audit_logs
BEGIN
    SELECT RAISE(ABORT, 'admin_audit_logs is immutable');
END;

CREATE TRIGGER IF NOT EXISTS trg_admin_audit_logs_no_delete
BEFORE DELETE ON admin_audit_logs
BEGIN
    SELECT RAISE(ABORT, 'admin_audit_logs is immutable');
END;
