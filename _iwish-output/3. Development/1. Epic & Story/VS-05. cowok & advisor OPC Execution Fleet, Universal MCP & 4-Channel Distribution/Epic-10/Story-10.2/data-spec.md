---
type: I-Wish Data & Contract Spec
story_id: "10.2"
title: "Data, V17 Disjoint Worker File Leases & Builder != Scorer Fleet Contract Specification: Story 10.2"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: WorkerFileLease
    action: NEW_AGGREGATE
    bounded_context: coordination-fleet
    justification: "V17 worker_file_leases enforces mutually disjoint owned_paths[] and monotonic fencing_token across parallel swarm workers"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Fleet Lease Coordination Specification — Story 10.2

## 1. Bounded Context & Kernel Lease Enforcement

Story `10.2` persists parallel swarm coordination state in `.cowok-design/coordination/` (`ONE_JOB + NEVER`) and SQLite WAL table `V17 worker_file_leases` (written exclusively via `Story 10.1` Single MPSC SQLite Write Actor). Each worker acquires disjoint `owned_paths[]` bound to a monotonic `fencing_token`. HTTP `429` responses dynamically clamp fleet semaphore concurrency `3 -> 1`. Independent QC Verifier workers run `E1..E4` (`>=95%` catch) strictly separated from Builder workers, plus Tier-3 `Single-Session Design Jury` over `review_freeze` snapshots with `generation fencing`.

## 2. Additive SQLite WAL DDL (`V17 worker_file_leases`)

```sql
CREATE TABLE IF NOT EXISTS worker_file_leases (
    lease_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    worker_id TEXT NOT NULL,
    worker_role TEXT NOT NULL CHECK (worker_role IN ('BUILDER','VERIFIER_QC','DESIGN_JURY')),
    fencing_token BIGINT NOT NULL,
    owned_paths_json TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    released INTEGER NOT NULL DEFAULT 0 CHECK (released IN (0, 1))
);

CREATE INDEX IF NOT EXISTS idx_worker_file_leases_active
    ON worker_file_leases(project_id, released, expires_at);
```

## 3. Prisma Schema Parity

```prisma
model WorkerFileLease {
  leaseId        String @id @map("lease_id")
  projectId      String @map("project_id")
  workerId       String @map("worker_id")
  workerRole     String @map("worker_role")
  fencingToken   BigInt @map("fencing_token")
  ownedPathsJson String @map("owned_paths_json")
  expiresAt      BigInt @map("expires_at")
  released       Int    @default(0)

  @@index([projectId, released, expiresAt])
  @@map("worker_file_leases")
}
```

## 4. API Endpoints & Zod Contracts

- POST `/api/v1/fleet/dispatch`: Validates disjoint write-sets, acquires `V17 worker_file_leases`, enforces `Builder != Scorer`, applies `429` semaphore clamp (`3 -> 1`).
- POST `/api/v1/fleet/leases/acquire`: Acquires atomic MPSC lease with monotonic `fencing_token` or returns `409 LEASE_PATH_CONFLICT`.
- POST `/api/v1/orchestrator/chat` & POST `/api/v1/qc/run`: Upstream orchestration & downstream QC verifier APIs.

```typescript
import { z } from "zod";

export const AcquireWorkerLeaseRequestSchema = z.object({
  projectId: z.string().min(1),
  workerId: z.string().min(1),
  workerRole: z.enum(["BUILDER", "VERIFIER_QC", "DESIGN_JURY"]),
  ownedPaths: z.array(z.string().min(1)).min(1),
  ttlMs: z.number().int().min(5000).max(300000).default(60000),
});
```
