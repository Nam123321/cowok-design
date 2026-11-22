---
type: I-Wish Data & Contract Spec
story_id: "13.1"
title: "Data, V18 Sealed Eval Benchmark Runs & Tamper-Proof Certification Specification: Story 13.1"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-007-Split-DE-DR-Plugin-Runtime-Tool-Jail-and-Sealed-Eval.md"
data_decision_records:
  - model: EvalBenchmarkRun
    action: NEW_AGGREGATE
    bounded_context: release-certification
    justification: "V18 eval_benchmark_runs stores sealed chmod-444 SHA-256 verification, paired ablation, Builder!=Scorer fault injection, and UC-01..UC-25 results"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Anti-Goodhart Eval Contract Specification — Story 13.1

## 1. Bounded Context & Fixed-Ruler Isolation (`ADR-007`)

Story `13.1` enforces strict isolation between self-evolution (`LEARN` / `advisor`) and the evaluation ruler (`tests/eval/sealed/` verified `chmod 444` + SHA-256 manifest, raising `SEALED_EVAL_TAMPER_ERROR` immediately upon symlink or writable permission drift). Supports `$0 <15s` `--tier pr` CI, paired `with_skill` vs `baseline` ablation (`>=12` briefs), 3-provider Decision calibration, `Builder != Scorer` fault injection (`>=95%`), and 25 automated `/simulate-user` cases (`UC-01..UC-25`, gate `>=22/25` Pass + `0` Blocked + `0` Gap + mandatory M1/M3 security gates).

## 2. Additive SQLite WAL DDL (`V18 eval_benchmark_runs`)

```sql
CREATE TABLE IF NOT EXISTS eval_benchmark_runs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    tier TEXT NOT NULL CHECK (tier IN ('pr','stage','release')),
    sealed_manifest_sha256 TEXT NOT NULL,
    tamper_detected INTEGER NOT NULL DEFAULT 0 CHECK (tamper_detected IN (0, 1)),
    ablation_lift_percent REAL,
    fault_catch_rate REAL,
    sim_cases_passed INTEGER NOT NULL DEFAULT 0,
    sim_cases_total INTEGER NOT NULL DEFAULT 25,
    security_gates_passed INTEGER NOT NULL CHECK (security_gates_passed IN (0, 1)),
    verdict TEXT NOT NULL CHECK (verdict IN ('PASS','FAIL','SEALED_EVAL_TAMPER_ERROR')),
    duration_ms BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);
```

## 3. Prisma Schema Parity

```prisma
model EvalBenchmarkRun {
  id                   String  @id @default(uuid())
  projectId            String? @map("project_id")
  tier                 String
  sealedManifestSha256 String  @map("sealed_manifest_sha256")
  tamperDetected       Int     @default(0) @map("tamper_detected")
  ablationLiftPercent  Float?  @map("ablation_lift_percent")
  faultCatchRate       Float?  @map("fault_catch_rate")
  simCasesPassed       Int     @default(0) @map("sim_cases_passed")
  simCasesTotal        Int     @default(25) @map("sim_cases_total")
  securityGatesPassed  Int     @map("security_gates_passed")
  verdict              String
  durationMs           BigInt  @map("duration_ms")
  createdAt            BigInt  @map("created_at")

  @@index([tier, createdAt])
  @@map("eval_benchmark_runs")
}
```

## 4. API Endpoints & Zod Contracts

- POST `/api/v1/eval/run`: Runs `--tier pr|stage|release`, verifies `tests/eval/sealed/` (`chmod 444` + SHA-256), executes fixed-ruler checks, appends `.cowok-design/evals/gate-history.jsonl` and inserts `V18 eval_benchmark_runs`.
- GET `/api/v1/eval/history`: Returns immutable historical certification runs and per-milestone `M1..M5` status for Workbench Certification Drawer.
- POST `/api/v1/qc/run`, POST `/api/v1/de/run`, POST `/api/v1/decision/route`: Consumed subsystem gates under evaluation.

```typescript
import { z } from "zod";

export const RunEvalSuiteRequestSchema = z.object({
  projectId: z.string().optional(),
  tier: z.enum(["pr", "stage", "release"]),
  includeSimSuite: z.boolean().default(true),
  failFastOnTamper: z.literal(true).default(true),
});
```
