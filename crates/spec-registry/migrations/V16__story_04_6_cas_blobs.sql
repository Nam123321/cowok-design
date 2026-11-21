-- V16__story_04_6_cas_blobs.sql
-- Story 04.6: Persistent chat sequence ordering, CAS physical blobs, tenant usage & 24h ingress ledgers,
-- and strictly decoupled 24h TTL idempotency receipt tables.

WITH numbered_missing_chat_seq AS (
    SELECT
        m.id,
        COALESCE((
            SELECT MAX(m2.seq_no)
            FROM chat_messages m2
            WHERE m2.session_id = m.session_id
              AND m2.seq_no IS NOT NULL
        ), 0) + ROW_NUMBER() OVER (
            PARTITION BY m.session_id
            ORDER BY m.created_at ASC, m.rowid ASC, m.id ASC
        ) AS next_seq
    FROM chat_messages m
    WHERE m.seq_no IS NULL
)
UPDATE chat_messages
SET seq_no = (
    SELECT nm.next_seq
    FROM numbered_missing_chat_seq nm
    WHERE nm.id = chat_messages.id
)
WHERE seq_no IS NULL
  AND EXISTS (
      SELECT 1
      FROM numbered_missing_chat_seq nm
      WHERE nm.id = chat_messages.id
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_chat_messages_session_seq
    ON chat_messages(session_id, seq_no) WHERE seq_no IS NOT NULL;

CREATE TABLE IF NOT EXISTS cas_blobs (
    sha256 TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    mime_type TEXT NOT NULL,
    file_ext TEXT NOT NULL CHECK (file_ext IN ('csv','tsv','xlsx','fig','pdf','png','svg','txt')),
    storage_relpath TEXT NOT NULL,
    ref_count INTEGER NOT NULL DEFAULT 1 CHECK (ref_count >= 0),
    gc_lease_until BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cas_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    blob_sha256 TEXT NOT NULL REFERENCES cas_blobs(sha256) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS tenant_cas_blob_usage (
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    blob_sha256 TEXT NOT NULL REFERENCES cas_blobs(sha256) ON DELETE CASCADE,
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    first_attached_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, blob_sha256)
);

CREATE TABLE IF NOT EXISTS cas_upload_ingress (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    session_id TEXT NOT NULL,
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    accepted_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cas_command_idempotency (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL,
    attachment_id TEXT NOT NULL REFERENCES cas_attachments(id) ON DELETE CASCADE,
    accepted_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    UNIQUE(tenant_id, session_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS vault_command_idempotency (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    session_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL,
    receipt_id TEXT NOT NULL,
    receipt_json TEXT NOT NULL,
    accepted_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    UNIQUE(tenant_id, session_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_cas_blobs_gc ON cas_blobs(gc_lease_until, ref_count);
CREATE INDEX IF NOT EXISTS idx_cas_attachments_tenant_session ON cas_attachments(tenant_id, session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_cas_attachments_blob ON cas_attachments(blob_sha256);
CREATE INDEX IF NOT EXISTS idx_cas_upload_ingress_window ON cas_upload_ingress(tenant_id, accepted_at);
CREATE INDEX IF NOT EXISTS idx_cas_cmd_idempotency_exp ON cas_command_idempotency(tenant_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_vault_cmd_idempotency_exp ON vault_command_idempotency(tenant_id, expires_at);
