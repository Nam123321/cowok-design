---
type: I-Wish Data & Contract Spec
story_id: "14.2"
title: "Data & V29 Cloud Ingestion API & SuperAdmin Portal MVP Contract: Story 14.2"
status: active
adr_ref: "../../../../2. Product Planning/ADRs/ADR-008-Unified-Branding-and-Cloud-Control-Plane.md"
data_decision_records:
  - model: CloudSupportTicket
    action: NEW_AGGREGATE
    bounded_context: cloud-control-plane
    justification: "V29 cloud_support_tickets & cloud_ticket_replies power 2-way non-tech helpdesk sync, server-side PII defense-in-depth, and P0-P3 triage"
  - model: CloudFeatureCluster
    action: NEW_AGGREGATE
    bounded_context: cloud-control-plane
    justification: "V29 cloud_feature_clusters groups deduplicated feature requests with 1-click internal GitHub issue tracking"
  - model: CloudTelemetryEvent
    action: NEW_AGGREGATE
    bounded_context: cloud-control-plane
    justification: "V29 cloud_telemetry_events & cloud_crash_groups drive DAU/WAU/MAU, 4-channel + 5-host activation funnel drop-off, and crash fingerprint triage"
  - model: AdminAuditLog
    action: NEW_AGGREGATE
    bounded_context: cloud-control-plane
    justification: "V29 admin_audit_logs provides append-only immutable audit trail for every OIDC+MFA 4-role RBAC SuperAdmin action"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "./story.md"
  - "./ui-spec.md"
---

# Data & Contract Specification — Story 14.2 (`V29`)

## 1. Additive SQLite WAL DDL (`V29__story_14_2_cloud_control_plane.sql`)
*(PostgreSQL-compatible DDL mirrored in `prisma/schema.prisma` for local/dev parity & Cloud Control Plane deployment)*

```sql
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

CREATE TABLE IF NOT EXISTS cloud_ticket_replies (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_id TEXT NOT NULL REFERENCES cloud_support_tickets(id) ON DELETE CASCADE,
    author_actor TEXT NOT NULL,
    author_role TEXT NOT NULL,
    body_sanitized TEXT NOT NULL,
    synced_to_client INTEGER NOT NULL DEFAULT 0 CHECK (synced_to_client IN (0, 1)),
    created_at BIGINT NOT NULL
);

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
```

## 2. API Endpoints & RBAC Matrix
- **Public Cloud Ingestion (Rate-Limited by `(install_id, IP)`, Server-Side PII Scrubbed, Image Magic-Byte + Re-Encode `<= 5 MB`):**
  - `POST /v1/ingest/tickets`
  - `POST /v1/ingest/telemetry` (also updates `cloud_crash_groups` when `event_name == 'app_crash'`)
- **SuperAdmin Protected Endpoints (Require OIDC Session + `mfa_verified == true` + 4-Role RBAC + Append Immutable `admin_audit_logs`):**
  - `POST /v1/admin/auth/session` (OIDC + MFA verification / session bootstrap)
  - `GET /v1/admin/tickets` (`SUPER_ADMIN | SUPPORT_AGENT | SKILL_CURATOR | ANALYST_READONLY`)
  - `POST /v1/admin/tickets/:id/reply` (`SUPER_ADMIN | SUPPORT_AGENT` only — `ANALYST_READONLY` & `SKILL_CURATOR` receive `403 RBAC_FORBIDDEN`)
  - `POST /v1/admin/clusters` (`SUPER_ADMIN | SUPPORT_AGENT | SKILL_CURATOR`)
  - `GET /v1/admin/analytics/funnel` (all 4 roles)
  - `GET /v1/admin/crashes` (all 4 roles)
  - `GET /v1/admin/audit-logs` (`SUPER_ADMIN` only)
