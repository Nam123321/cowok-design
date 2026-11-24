-- V23__story_14_1_support_telemetry_outbox.sql
-- Story 14.1: Client Edge — In-App Non-Tech Support/Request Drawer (No-GitHub),
-- Zero-PII Diagnostic Scrubber & Multi-Channel Telemetry Outbox.
-- Adds 3 tables: telemetry_outbox, local_support_tickets, telemetry_preferences.

CREATE TABLE IF NOT EXISTS telemetry_outbox (
    id TEXT PRIMARY KEY NOT NULL,
    install_id TEXT NOT NULL,
    channel TEXT NOT NULL CHECK (
        channel IN (
            'cli',
            'desktop',
            'claude_code',
            'codex',
            'antigravity',
            'cursor',
            'cowok_ai',
            'docker'
        )
    ),
    event_group TEXT NOT NULL CHECK (
        event_group IN (
            'lifecycle',
            'onboarding_funnel',
            'feature_usage',
            'reliability_errors',
            'skill_ecosystem'
        )
    ),
    event_name TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'flushing', 'sent', 'failed')
    ),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_retry_at BIGINT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_telemetry_outbox_status_retry
    ON telemetry_outbox(status, next_retry_at);

CREATE INDEX IF NOT EXISTS idx_telemetry_outbox_created_at
    ON telemetry_outbox(created_at);

CREATE TABLE IF NOT EXISTS local_support_tickets (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_code TEXT NOT NULL UNIQUE,
    install_id TEXT NOT NULL,
    channel TEXT NOT NULL CHECK (
        channel IN (
            'cli',
            'desktop',
            'claude_code',
            'codex',
            'antigravity',
            'cursor',
            'cowok_ai',
            'docker'
        )
    ),
    category TEXT NOT NULL CHECK (
        category IN (
            'bug_report',
            'feature_request',
            'skill_template_request',
            'question'
        )
    ),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    contact_email TEXT,
    screenshot_sha256 TEXT,
    diagnostics_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'submitted' CHECK (
        status IN ('submitted', 'triaged', 'in_progress', 'resolved', 'closed')
    ),
    admin_replies_json TEXT NOT NULL DEFAULT '[]',
    sync_status TEXT NOT NULL DEFAULT 'queued' CHECK (
        sync_status IN ('queued', 'synced', 'local_only', 'failed')
    ),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_local_support_tickets_status
    ON local_support_tickets(status, updated_at);

CREATE TABLE IF NOT EXISTS telemetry_preferences (
    id TEXT PRIMARY KEY NOT NULL,
    install_id TEXT NOT NULL,
    telemetry_enabled INTEGER NOT NULL DEFAULT 1 CHECK (telemetry_enabled IN (0, 1)),
    updated_at BIGINT NOT NULL
);
