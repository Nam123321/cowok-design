---
type: I-Wish Data & Contract Spec
story_id: "10.3"
title: "Data, V18 Tournament Runs, AST Distance Gate & Creative Memory Specification: Story 10.3"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: TournamentRun
    action: NEW_AGGREGATE
    bounded_context: design-tournament
    justification: "V18 tournament_runs & tournament_variants record 4-axis variant runs, AST structural distance <=0.78, blind judge picks and Cross-Variant Borrow merges"
  - model: CreativeMemoryEvent
    action: NEW_AGGREGATE
    bounded_context: self-evolution
    justification: "V18 creative_memory_events stores win/loss/borrow signals for advisor 6-step loop & /crystallize"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data, Tournament & Creative Memory Specification — Story 10.3

## 1. Bounded Context & `V18` Tournament Tables

Story `10.3` persists 4-axis tournament dispatches (`<=15s`) into `V18 tournament_runs` and `tournament_variants`, records `ast_similarity` + `clone_distance_passed` (`similarity <= 0.78`), executes position-swapped `RubricShield` blind judging (`(A,B)/(B,A)`), applies safe cross-variant element borrows using `reconcile_ff_ids` (`05.4`), and records preference/repair events in `V18 creative_memory_events` powering the regression-gated `advisor` 6-step loop and `/crystallize`.

## 2. Additive SQLite WAL DDL (`V18`)

```sql
CREATE TABLE IF NOT EXISTS tournament_runs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    axes_json TEXT NOT NULL,
    winning_variant_id TEXT,
    borrowed_from_variant_id TEXT,
    borrowed_ff_id TEXT,
    generation_epoch BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS tournament_variants (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES tournament_runs(id) ON DELETE CASCADE,
    axis_profile TEXT NOT NULL,
    artifact_sha256 TEXT NOT NULL,
    ast_similarity REAL NOT NULL,
    clone_distance_passed INTEGER NOT NULL CHECK (clone_distance_passed IN (0, 1)),
    rubric_verdict TEXT,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1))
);

CREATE TABLE IF NOT EXISTS creative_memory_events (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_kind TEXT NOT NULL CHECK (event_kind IN ('TOURNAMENT_WIN','TOURNAMENT_BORROW','ADVISOR_PATCH','CRYSTALLIZE')),
    payload_json TEXT NOT NULL,
    regression_passed INTEGER NOT NULL DEFAULT 1 CHECK (regression_passed IN (0, 1)),
    created_at BIGINT NOT NULL
);
```

## 3. Prisma Schema Parity

```prisma
model TournamentRun {
  id                    String  @id @default(uuid())
  projectId             String  @map("project_id")
  axesJson              String  @map("axes_json")
  winningVariantId      String? @map("winning_variant_id")
  borrowedFromVariantId String? @map("borrowed_from_variant_id")
  borrowedFfId          String? @map("borrowed_ff_id")
  generationEpoch       BigInt  @default(1) @map("generation_epoch")
  createdAt             BigInt  @map("created_at")

  @@index([projectId, createdAt])
  @@map("tournament_runs")
}

model TournamentVariant {
  id                  String  @id @default(uuid())
  runId               String  @map("run_id")
  axisProfile         String  @map("axis_profile")
  artifactSha256      String  @map("artifact_sha256")
  astSimilarity       Float   @map("ast_similarity")
  cloneDistancePassed Int     @map("clone_distance_passed")
  rubricVerdict       String? @map("rubric_verdict")
  archived            Int     @default(0)

  @@index([runId])
  @@map("tournament_variants")
}
```

## 4. API Endpoints & Zod Contracts

- POST `/api/v1/tournament/start`: Launches 2-4 variants across orthogonal axes (`<=15s`) and runs Pre-Judge AST Structural Distance Gate (`similarity <= 0.78`).
- POST `/api/v1/tournament/select`: Commits debounced winner with `generationEpoch`, runs `reconcile_ff_ids` Cross-Variant Borrow, archives losing branches to `variants/_archive/`, logs `V18 creative_memory_events`.
- POST `/api/v1/crystallize`: Packages finished screen/project into reusable Starter Template, Brand Kit, or `SKILL.md` plugin.

```typescript
import { z } from "zod";

export const SelectTournamentVariantRequestSchema = z.object({
  projectId: z.string().min(1),
  runId: z.string().min(1),
  winningVariantId: z.string().min(1),
  generationEpoch: z.number().int().min(1),
  borrowFromVariantId: z.string().optional(),
  borrowSelectorOrInstruction: z.string().max(400).optional(),
});
```
