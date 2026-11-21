---
type: I-Wish Data & Contract Spec
story_id: "02.4"
title: "Data, Persistence, Dual-Use SSOT & Router Contract Specification: Story 02.4"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-007-Split-DE-DR-Plugin-Runtime-Tool-Jail-and-Sealed-Eval.md"
data_decision_records:
  - model: ComponentPattern
    action: NEW_AGGREGATE
    bounded_context: ssot-registry
    justification: "V18 component_patterns persists canonical UI components with portal scoping, drift quarantine, and oxc reverse sync metadata"
  - model: AppShellLock
    action: NEW_AGGREGATE
    bounded_context: ssot-registry
    justification: "V18 app_shell_locks enforces Single Lock Owner monotonic fencing_token concurrency control across multi-screen generation"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Contract Specification — Story 02.4

## 1. Bounded Context & Storage Boundary (`V18` Dual-Use Hidden SSOT — 58 Tables)

Story `02.4` owns write operations for `component_patterns` and `app_shell_locks` inside additive migration **`V18__story_02_4_hidden_ssot.sql`** (`CURRENT_SCHEMA_VERSION = 18`, `DAEMON_MIN_SCHEMA_VERSION = 18`, total **58** SQLite WAL tables sequential after `Story 02.1` `V17`), coordinates `Single Lock Owner` monotonic `fencing_token` semantics in `crates/spec-registry` & `crates/constraint-engine`, compiles `<15ms` 3-Zone `CachePreservingSkillRouter` bundles for `cowok-design-opt` (`<=250` BPE tokens via `tiktoken-rs`) + 7 3-tier Craft Skills (`packages/builtin-skills`), and exports Dual-Use Markdown/JSON artifacts to `.cowok-design/exports/`.

## 2. Additive SQLite WAL DDL (`V18__story_02_4_hidden_ssot.sql`)

```sql
CREATE UNIQUE INDEX IF NOT EXISTS uq_projects_id_tenant
    ON projects(id, tenant_id);

CREATE TABLE IF NOT EXISTS component_patterns (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    portal_id TEXT NOT NULL CHECK (
        portal_id IN ('SuperAdmin','TenantAdmin','SalesApp','MarketingWeb','DeveloperPortal','Presentation')
    ),
    pattern_slug TEXT NOT NULL,
    component_name TEXT NOT NULL,
    source_path TEXT,
    ast_signature_sha256 TEXT NOT NULL,
    dedup_marker_sha256 TEXT NOT NULL DEFAULT '',
    source_revision BIGINT NOT NULL DEFAULT 0,
    last_sync_source TEXT NOT NULL DEFAULT 'manual' CHECK (
        last_sync_source IN ('oxc_reverse_sync','manual','export_sync')
    ),
    tokens_snapshot_json TEXT NOT NULL DEFAULT '{}',
    doc_markdown TEXT NOT NULL DEFAULT '',
    drift_status TEXT NOT NULL DEFAULT 'synced' CHECK (
        drift_status IN ('synced','quarantined_drift')
    ),
    quarantined_diff_json TEXT NOT NULL DEFAULT '{}',
    revision BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id, portal_id, pattern_slug)
);

CREATE INDEX IF NOT EXISTS idx_component_patterns_proj_portal
    ON component_patterns(tenant_id, project_id, portal_id, updated_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS app_shell_locks (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT NOT NULL,
    portal_id TEXT NOT NULL CHECK (
        portal_id IN ('SuperAdmin','TenantAdmin','SalesApp','MarketingWeb','DeveloperPortal','Presentation')
    ),
    lock_owner_id TEXT NOT NULL,
    fencing_token BIGINT NOT NULL DEFAULT 1,
    locked_slots_json TEXT NOT NULL DEFAULT '[]',
    expires_at BIGINT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (project_id, tenant_id) REFERENCES projects(id, tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, project_id, portal_id)
);

CREATE INDEX IF NOT EXISTS idx_app_shell_locks_proj_portal
    ON app_shell_locks(tenant_id, project_id, portal_id);
```

## 3. Prisma Schema Parity

Add `uq_projects_id_tenant` (`@@unique([id, tenantId], map: "uq_projects_id_tenant")`) and inverse relations `componentPatterns ComponentPattern[]`, `appShellLocks AppShellLock[]` onto `model Project`, plus unconditional composite `@relation` on both V18 models:

```prisma
model ComponentPattern {
  id                  String  @id @default(uuid())
  tenantId            String  @default("local-default") @map("tenant_id")
  projectId           String  @map("project_id")
  portalId            String  @map("portal_id")
  patternSlug         String  @map("pattern_slug")
  componentName       String  @map("component_name")
  sourcePath          String? @map("source_path")
  astSignatureSha256  String  @map("ast_signature_sha256")
  dedupMarkerSha256   String  @default("") @map("dedup_marker_sha256")
  sourceRevision      BigInt  @default(0) @map("source_revision")
  lastSyncSource      String  @default("manual") @map("last_sync_source")
  tokensSnapshotJson  String  @default("{}") @map("tokens_snapshot_json")
  docMarkdown         String  @default("") @map("doc_markdown")
  driftStatus         String  @default("synced") @map("drift_status")
  quarantinedDiffJson String  @default("{}") @map("quarantined_diff_json")
  revision            BigInt  @default(1)
  createdAt           BigInt  @map("created_at")
  updatedAt           BigInt  @map("updated_at")

  project Project @relation(fields: [projectId, tenantId], references: [id, tenantId], onDelete: Cascade)

  @@unique([tenantId, projectId, portalId, patternSlug])
  @@index([tenantId, projectId, portalId, updatedAt, id], map: "idx_component_patterns_proj_portal")
  @@map("component_patterns")
}

model AppShellLock {
  id              String @id @default(uuid())
  tenantId        String @default("local-default") @map("tenant_id")
  projectId       String @map("project_id")
  portalId        String @map("portal_id")
  lockOwnerId     String @map("lock_owner_id")
  fencingToken    BigInt @default(1) @map("fencing_token")
  lockedSlotsJson String @default("[]") @map("locked_slots_json")
  expiresAt       BigInt @map("expires_at")
  revision        BigInt @default(1)
  createdAt       BigInt @map("created_at")
  updatedAt       BigInt @map("updated_at")

  project Project @relation(fields: [projectId, tenantId], references: [id, tenantId], onDelete: Cascade)

  @@unique([tenantId, projectId, portalId])
  @@index([tenantId, projectId, portalId], map: "idx_app_shell_locks_proj_portal")
  @@map("app_shell_locks")
}
```

## 4. API Endpoints & Zod Contracts

- GET `/api/v1/ssot/components`: Returns portal-scoped `ComponentPattern[]` and active `AppShellLock` status.
- POST `/api/v1/ssot/appshell/lock`: Acquires, renews, releases, or force-transfers `Single Lock Owner` lease using `expectedRevision` & monotonic `expectedFencingToken` CAS (`409 LOCK_FENCING_CONFLICT` on mismatch).
- POST `/api/v1/ssot/export`: Exports Dual-Use Human/Agent Markdown + JSON snapshots to `.cowok-design/exports/`.
- POST `/api/v1/skills/route`: Runs 3-Zone `CachePreservingSkillRouter` (`<15ms`), returning KV-prefix stable `SkillRouteBundle`.
- POST `/api/v1/context/assemble`: Consumed upstream endpoint from `Story 02.1`.

```typescript
import { z } from "zod";

export const SsotPortalIdSchema = z.enum([
  "SuperAdmin",
  "TenantAdmin",
  "SalesApp",
  "MarketingWeb",
  "DeveloperPortal",
  "Presentation",
]);

export const CraftSkillSlugSchema = z.enum([
  "archify-diagram",
  "dashboard-ux",
  "impeccable-polish",
  "oklch-palette",
  "sketch-monochrome",
  "stageshow-slide",
  "typography",
]);

export const SsotComponentsQuerySchema = z.object({
  projectId: z.string().min(1).max(128),
  portalId: SsotPortalIdSchema,
  cursor: z.string().max(256).optional(),
  limit: z.coerce.number().int().min(1).max(100).default(50),
}).strict();

export const AppShellLockRequestSchema = z.object({
  projectId: z.string().min(1).max(128),
  portalId: SsotPortalIdSchema,
  action: z.enum(["ACQUIRE", "RENEW", "RELEASE", "TRANSFER"]),
  lockOwnerId: z.string().min(1).max(128),
  expectedRevision: z.number().int().min(0),
  expectedFencingToken: z.number().int().min(0).nullable().default(null),
  lockedSlots: z.array(z.string().min(1).max(64)).max(32).default([]),
  ttlMs: z.number().int().min(5000).max(3600000).default(30000),
  forceTransferReason: z.string().min(1).max(256).optional(),
}).strict();

export const SsotExportRequestSchema = z.object({
  projectId: z.string().min(1).max(128),
  portalId: SsotPortalIdSchema,
  formats: z.array(z.enum(["markdown", "json"])).min(1).max(2).default(["markdown", "json"]),
  includeQuarantined: z.boolean().default(false),
}).strict();

export const SkillRouteRequestSchema = z.object({
  projectId: z.string().min(1).max(128),
  intentSummary: z.string().min(1).max(1000),
  portalId: SsotPortalIdSchema,
  requestedSlugs: z.array(CraftSkillSlugSchema).max(7).optional(),
  maxSpineTokens: z.number().int().min(1).max(250).default(250),
}).strict();

export const ComponentPatternDtoSchema = z.object({
  id: z.string().min(1),
  projectId: z.string().min(1),
  portalId: SsotPortalIdSchema,
  patternSlug: z.string().min(1),
  componentName: z.string().min(1),
  sourcePath: z.string().nullable(),
  astSignatureSha256: z.string().length(64),
  dedupMarkerSha256: z.string(),
  sourceRevision: z.number().int().min(0),
  lastSyncSource: z.enum(["oxc_reverse_sync", "manual", "export_sync"]),
  tokensSnapshotJson: z.string(),
  docMarkdown: z.string(),
  driftStatus: z.enum(["synced", "quarantined_drift"]),
  quarantinedDiffJson: z.string(),
  revision: z.number().int().min(1),
  createdAt: z.number().int().min(0),
  updatedAt: z.number().int().min(0),
}).strict();

export const AppShellLockDtoSchema = z.object({
  id: z.string().min(1),
  projectId: z.string().min(1),
  portalId: SsotPortalIdSchema,
  lockOwnerId: z.string(),
  fencingToken: z.number().int().min(1),
  lockedSlots: z.array(z.string()),
  expiresAt: z.number().int().min(0),
  isExpired: z.boolean(),
  revision: z.number().int().min(1),
  createdAt: z.number().int().min(0),
  updatedAt: z.number().int().min(0),
}).strict();

export const AppShellLockConflictDetailsSchema = z.object({
  currentRevision: z.number().int().min(1),
  currentFencingToken: z.number().int().min(1),
  expiresAt: z.number().int().min(0),
  isExpired: z.boolean(),
}).strict();

export const SsotComponentsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    patterns: z.array(ComponentPatternDtoSchema),
    lock: AppShellLockDtoSchema.nullable(),
    nextCursor: z.string().nullable(),
    quarantinedCount: z.number().int().min(0),
  }).strict(),
  requestId: z.string().min(1),
}).strict();

export const AppShellLockMutationResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    lock: AppShellLockDtoSchema,
  }).strict(),
  requestId: z.string().min(1),
}).strict();

export const SsotExportResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    exportId: z.string().min(1),
    revision: z.number().int().min(1),
    portalId: SsotPortalIdSchema,
    generatedAt: z.number().int().min(0),
    markdownPath: z.string().min(1),
    markdownSha256: z.string().length(64),
    jsonPath: z.string().min(1),
    jsonSha256: z.string().length(64),
  }).strict(),
  requestId: z.string().min(1),
}).strict();

export const SkillRouteResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    portalId: SsotPortalIdSchema,
    spineTokens: z.number().int().min(1).max(250),
    selectedSlugs: z.array(CraftSkillSlugSchema),
    prefixSha256: z.string().length(64),
    zone1Spine: z.string().min(1),
    zone2CraftPrefix: z.string(),
    zone3IntentTail: z.string().min(1),
    latencyMs: z.number().min(0),
  }).strict(),
  requestId: z.string().min(1),
}).strict();

export const SsotApiErrorSchema = z.object({
  error: z.object({
    code: z.enum([
      "INVALID_REQUEST",
      "UNAUTHORIZED",
      "AUTH_FORBIDDEN",
      "CROSS_PORTAL_LEAK_BLOCKED",
      "PROJECT_TENANT_MISMATCH",
      "PROJECT_NOT_FOUND",
      "LOCK_FENCING_CONFLICT",
      "LOCK_TRANSFER_FORBIDDEN",
      "COMPONENT_PATTERN_CONFLICT",
      "COMPONENT_PATTERN_UNIQUE_CONFLICT",
      "PATH_JAIL_VIOLATION",
      "SPINE_TOKEN_BUDGET_EXCEEDED",
      "STORAGE_BUSY",
      "EXPORT_SUPERSEDED",
      "EXPORT_MANIFEST_INVALID",
      "INTERNAL_ERROR",
    ]),
    message: z.string().min(1),
    field: z.string().optional(),
    requestId: z.string().min(1),
    details: z.union([AppShellLockConflictDetailsSchema, z.record(z.unknown())]).optional(),
  }).strict(),
}).strict();
```
