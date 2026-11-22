---
type: I-Wish Data & Contract Spec
story_id: "05.4"
title: "Data, HTML Forge & Surgical Patch Specification: Story 05.4"
status: synced
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: ScreenArtifactPatch
    action: EXTEND
    bounded_context: html-forge
    justification: "Shares presentation-cms::cms_patcher (lol_html + html5ever) into crates/html-forge with SurgicalPatcher::reconcile_ff_ids and V19 SQLite WAL OCC persistence"
  - model: SdiClarificationSession
    action: NEW_AGGREGATE
    bounded_context: html-forge
    justification: "Persists Day-0 Goal+Locale clarification lock state, 2 Live Direction Switcher pills, and monotonic generation_epoch for -PGID preemption"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & HTML Forge Contract Specification — Story 05.4

## 1. Bounded Context & Shared `crates/html-forge`

Story `05.4` extracts `presentation-cms::cms_patcher` (`lol_html + html5ever`) into shared crate `crates/html-forge` supporting `SanitizeProfile::Slide` and `SanitizeProfile::AppPreview`, implements `SurgicalPatcher::reconcile_ff_ids` to maintain zero `409 STALE_MAPPING` cascades, flushes `useAppScreenCmsState` edits from browser `localStorage` into SQLite `screen_artifacts`, and stores clarification state (`clarifying` + `-PGID` preempt tracking) + `LockedRuleChangeReceipt`.

## 2. Persistence Schema & OCC Invariants

```sql
-- V19__story_05_4_html_forge_and_sketch_polish.sql (CURRENT_SCHEMA_VERSION = 19, 60 SQLite WAL tables)
-- screen_artifact_patches & sdi_clarification_sessions with composite FKs (project_id, tenant_id)
-- and OCC UNIQUE(tenant_id, project_id, screen_id, ast_revision)
```

```prisma
model ScreenArtifactPatch {
  id                String  @id @default(uuid())
  tenantId          String  @map("tenant_id")
  projectId         String  @map("project_id")
  screenId          String  @map("screen_id")
  astRevision       Int     @map("ast_revision")
  baseArtifactSha   String  @map("base_artifact_sha")
  resultArtifactSha String  @map("result_artifact_sha")
  ffId              String  @map("ff_id")
  opKind            String  @map("op_kind")
  applyTier         String  @map("apply_tier")
  reconciledFfIds   Int     @default(0) @map("reconciled_ff_ids")
  payloadJson       String  @default("{}") @map("payload_json")
  htmlSnapshot      String  @map("html_snapshot")
  lockedRuleJson    String? @map("locked_rule_json")
  createdAt         BigInt  @map("created_at")

  @@unique([tenantId, projectId, screenId, astRevision])
  @@index([tenantId, projectId, screenId])
  @@map("screen_artifact_patches")
}

model SdiClarificationSession {
  id                  String  @id @default(uuid())
  tenantId            String  @map("tenant_id")
  projectId           String  @map("project_id")
  phase               String  @default("clarifying")
  goalPrompt          String  @map("goal_prompt")
  locale              String  @default("vi")
  activeDirectionId   String  @map("active_direction_id")
  generationEpoch     Int     @default(1) @map("generation_epoch")
  directionsJson      String  @default("[]") @map("directions_json")
  lastPreemptedPgid   Int?    @map("last_preempted_pgid")
  activeSkillsJson    String  @default("[]") @map("active_skills_json")
  updatedAt           BigInt  @map("updated_at")
  createdAt           BigInt  @map("created_at")

  @@unique([tenantId, projectId])
  @@map("sdi_clarification_sessions")
}
```

## 3. API Endpoints & Zod Contracts

- POST `/api/v1/sdi/clarify`: Locks flow into `clarifying`, dispatches clarification choices or invokes `-PGID` preempt when user switches direction pill.
- POST `/api/v1/patch`: Executes shared `lol_html + html5ever` patch via `SurgicalPatcher::reconcile_ff_ids`, handles 2-Tier Auto-Apply (`INSTANT_UNDO` vs `LOCKED_RULE_RECEIPT`), flushes `useAppScreenCmsState` to SQLite WAL.
- POST `/api/v1/skills/route`: Consumed from `Story 02.4` to inject `sketch-monochrome` and `impeccable-polish`.

```typescript
import { z } from "zod";

export const ClarifyDirectionRequestSchema = z.object({
  projectId: z.string().min(1),
  goalPrompt: z.string().min(1),
  locale: z.enum(["vi", "en"]),
  activeDirectionId: z.string().optional(),
  preemptPgid: z.number().int().positive().optional(),
});

export const SurgicalPatchRequestSchema = z.object({
  projectId: z.string().min(1),
  screenId: z.string().min(1),
  baseArtifactSha256: z.string().length(64),
  ffId: z.string().min(1),
  opKind: z.enum(["TEXT_EDIT", "TOKEN_SWAP", "STEERING_CHIP", "LOCKED_RULE_CHANGE"]),
  payloadJson: z.record(z.unknown()),
});
```
