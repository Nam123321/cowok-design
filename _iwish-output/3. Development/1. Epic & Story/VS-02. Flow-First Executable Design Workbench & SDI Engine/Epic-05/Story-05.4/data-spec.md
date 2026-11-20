---
type: I-Wish Data & Contract Spec
story_id: "05.4"
title: "Data, HTML Forge & Surgical Patch Specification: Story 05.4"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: ScreenArtifactPatch
    action: EXTENDS_AGGREGATE
    bounded_context: html-forge
    justification: "Shares presentation-cms::cms_patcher (lol_html + html5ever) into crates/html-forge with SurgicalPatcher::reconcile_ff_ids"
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
-- Extends existing screen_artifacts & change_proposals (no conflicting V-migration)
-- app_screen_revisions tracked atomically via revision + artifact_sha256 OCC
```

```prisma
model ScreenArtifactPatch {
  id              String @id @default(uuid())
  projectId       String @map("project_id")
  screenId        String @map("screen_id")
  baseArtifactSha String @map("base_artifact_sha")
  applyTier       String @map("apply_tier")
  reconciledFfIds Int    @map("reconciled_ff_ids")
  lockedRuleJson  String? @map("locked_rule_json")
  createdAt       BigInt @map("created_at")

  @@index([projectId, screenId])
  @@map("screen_artifact_patches")
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
