---
type: I-Wish Data & Contract Spec
story_id: "07.1"
title: "Data & V17 Visual QC Gate Run Contract Specification: Story 07.1"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: QcGateRun
    action: NEW_AGGREGATE
    bounded_context: visual-qc
    justification: "V17 qc_gate_runs stores immutable Pooled 1-Chromium CDP geometry/WCAG audits, Real RGBA SSIM metrics & SHA-1 layout cache entries"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & QC Telemetry Specification — Story 07.1

## 1. Bounded Context & `V17 qc_gate_runs`

Story `07.1` consolidates Pooled `<= 1` Chromium CDP audits (`lib-audit` across `1440/1024/768/390` + sweep `320..1440` and `slides-check` `1920x1080`), Real RGBA SSIM + `elementsFromPoint` occlusion checks, E2E Flow Completeness scans (`Dead-End CTAs`, `Orphan Screens`, missing `4-SubStates`), and `<10ms` deterministic `lint_fix` (`0` LLM tokens) vs `MAX_ATTEMPTS = 3` Agent Auto-Fix. When no Chromium binary is available, persists `UNVERIFIED_NO_BROWSER` honestly.

## 2. Additive SQLite WAL DDL (`V17`)

```sql
CREATE TABLE IF NOT EXISTS qc_gate_runs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('APP_SCREEN','PRESENTATION_SLIDE','FULL_FLOW')),
    target_id TEXT NOT NULL,
    layout_sha1 TEXT NOT NULL,
    viewports_json TEXT NOT NULL,
    verdict TEXT NOT NULL CHECK (verdict IN ('PASS','FAIL','UNVERIFIED_NO_BROWSER')),
    p0_count INTEGER NOT NULL DEFAULT 0,
    p1_count INTEGER NOT NULL DEFAULT 0,
    ssim_score REAL,
    issues_json TEXT NOT NULL DEFAULT '[]',
    autofix_attempt INTEGER NOT NULL DEFAULT 0 CHECK (autofix_attempt BETWEEN 0 AND 3),
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_qc_gate_runs_cache ON qc_gate_runs(project_id, layout_sha1, created_at);
```

## 3. Prisma Schema Parity

```prisma
model QcGateRun {
  id              String  @id @default(uuid())
  projectId       String  @map("project_id")
  targetKind      String  @map("target_kind")
  targetId        String  @map("target_id")
  layoutSha1      String  @map("layout_sha1")
  viewportsJson   String  @map("viewports_json")
  verdict         String
  p0Count         Int     @default(0) @map("p0_count")
  p1Count         Int     @default(0) @map("p1_count")
  ssimScore       Float?  @map("ssim_score")
  issuesJson      String  @default("[]") @map("issues_json")
  autofixAttempt  Int     @default(0) @map("autofix_attempt")
  createdAt       BigInt  @map("created_at")

  @@index([projectId, layoutSha1, createdAt])
  @@map("qc_gate_runs")
}
```

## 4. API Endpoints & Zod Contracts

- POST `/api/v1/qc/run`: Executes Pooled 1-Chromium CDP + 25+ rule solver + Real RGBA SSIM + E2E flow completeness audit, using `layout_sha1` cache when unchanged (`UNVERIFIED_NO_BROWSER` fallback if Chromium absent).
- POST `/api/v1/qc/auto-fix`: Runs deterministic `lint_fix` (`<10ms`, `0` tokens) for token/spacing/radius violations or bounded `MAX_ATTEMPTS = 3` Agent surgical patch via POST `/api/v1/patch`.

```typescript
import { z } from "zod";

export const QcAutoFixRequestSchema = z.object({
  projectId: z.string().min(1),
  qcRunId: z.string().min(1),
  issueIds: z.array(z.string().min(1)).min(1),
  mode: z.enum(["DETERMINISTIC_LINT_FIX", "AGENT_PATCH"]),
  attemptNumber: z.number().int().min(1).max(3),
});
```
