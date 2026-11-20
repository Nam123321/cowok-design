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
    justification: "V16 component_patterns persists canonical UI components with portal scoping and oxc reverse sync metadata"
  - model: AppShellLock
    action: NEW_AGGREGATE
    bounded_context: ssot-registry
    justification: "V16 app_shell_locks enforces Single Lock Owner concurrency control across multi-screen generation"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Contract Specification — Story 02.4

## 1. Bounded Context & Storage Boundary (`V16` Dual-Use Hidden SSOT)

Story `02.4` owns write operations for `component_patterns` and `app_shell_locks` inside additive migration `V16` (`.cowok-design/cowok-design.sqlite3`), coordinates `Single Lock Owner` semantics in `crates/constraint-engine`, compiles `<15ms` 3-Zone `CachePreservingSkillRouter` bundles for `cowok-design-opt` (`<=250` tokens) + 7 3-tier Craft Skills (`packages/builtin-skills`), and exports Dual-Use Markdown/JSON artifacts to `.cowok-design/exports/`.

## 2. Additive SQLite WAL DDL (`V16`)

```sql
CREATE TABLE IF NOT EXISTS component_patterns (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    portal_id TEXT NOT NULL,
    pattern_slug TEXT NOT NULL,
    component_name TEXT NOT NULL,
    source_path TEXT,
    ast_signature_sha256 TEXT NOT NULL,
    tokens_snapshot_json TEXT NOT NULL DEFAULT '{}',
    doc_markdown TEXT NOT NULL DEFAULT '',
    revision BIGINT NOT NULL DEFAULT 1,
    updated_at BIGINT NOT NULL,
    UNIQUE(project_id, portal_id, pattern_slug)
);

CREATE TABLE IF NOT EXISTS app_shell_locks (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    portal_id TEXT NOT NULL,
    lock_owner_id TEXT NOT NULL,
    lock_token TEXT NOT NULL,
    locked_slots_json TEXT NOT NULL DEFAULT '[]',
    expires_at BIGINT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY(project_id, portal_id)
);
```

## 3. Prisma Schema Parity

```prisma
model ComponentPattern {
  id                 String @id @default(uuid())
  projectId          String @map("project_id")
  portalId           String @map("portal_id")
  patternSlug        String @map("pattern_slug")
  componentName      String @map("component_name")
  sourcePath         String? @map("source_path")
  astSignatureSha256 String @map("ast_signature_sha256")
  tokensSnapshotJson String @default("{}") @map("tokens_snapshot_json")
  docMarkdown        String @default("") @map("doc_markdown")
  revision           BigInt @default(1)
  updatedAt          BigInt @map("updated_at")

  @@unique([projectId, portalId, patternSlug])
  @@map("component_patterns")
}

model AppShellLock {
  projectId       String @map("project_id")
  portalId        String @map("portal_id")
  lockOwnerId     String @map("lock_owner_id")
  lockToken       String @map("lock_token")
  lockedSlotsJson String @default("[]") @map("locked_slots_json")
  expiresAt       BigInt @map("expires_at")
  revision        BigInt @default(1)

  @@id([projectId, portalId])
  @@map("app_shell_locks")
}
```

## 4. API Endpoints & Zod Contracts

- GET `/api/v1/ssot/components`: Returns portal-scoped `ComponentPattern[]` and active `AppShellLock` status.
- POST `/api/v1/ssot/appshell/lock`: Acquires, renews, or releases `Single Lock Owner` lease using `expectedRevision` OCC (`409` on mismatch).
- POST `/api/v1/ssot/export`: Exports Dual-Use Human/Agent Markdown + JSON snapshots to `.cowok-design/exports/`.
- POST `/api/v1/skills/route`: Runs 3-Zone `CachePreservingSkillRouter` (`<15ms`), returning KV-prefix stable `SkillRouteBundle`.
- POST `/api/v1/context/assemble`: Consumed upstream endpoint from `Story 02.1`.

```typescript
import { z } from "zod";

export const AppShellLockRequestSchema = z.object({
  projectId: z.string().min(1),
  portalId: z.enum(["SuperAdmin", "TenantAdmin", "SalesApp", "MarketingWeb", "DeveloperPortal", "Presentation"]),
  action: z.enum(["ACQUIRE", "RELEASE", "TRANSFER"]),
  lockOwnerId: z.string().min(1),
  expectedRevision: z.number().int().min(1),
});

export const SkillRouteRequestSchema = z.object({
  projectId: z.string().min(1),
  intentSummary: z.string().min(1).max(1000),
  portalId: z.string().min(1),
  maxTokens: z.number().int().min(100).max(4000).default(250),
});
```
