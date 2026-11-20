---
type: I-Wish Data & Contract Spec
story_id: "09.2"
title: "Data, Decision Cascade Engine & V17 decision_audit_log Specification: Story 09.2"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: DecisionAuditLog
    action: NEW_AGGREGATE
    bounded_context: decision-cascade
    justification: "V17 decision_audit_log records immutable 5-stage Decision Cascade verdicts, VerdictShield flags and RubricShield consistency scores"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Decision Audit Specification — Story 09.2

## 1. Bounded Context & `DecisionCascadeClient` (~450 LOC)

Story `09.2` replaces the unused `crates/jev-engine` concept with `DecisionCascadeClient` (~450 LOC inside `crates/domain` + `crates/bot-fleet`) executing the 5-stage `DE -> Decision #1 -> DR/DE -> DE -> Decision #2` machine. It enforces `ConfidenceKind` (`ExactMatch | ModelVerdict | DeterministicFallback` matching `2.2. database-spec.md`), `egress_guard` (`<=4KB` prompt / `<=64KB` total wrapped in nonce-tagged `<untrusted_artifact_bytes nonce="...">`), `VerdictShield` (`0%` float/logprob exposure), `RubricShield` (`(X,Y)/(Y,X)` `>=90%` + deterministic tie-break), `MAX_ATTEMPTS = 3` oscillation breaker, `StaleFile` SHA-256 OCC, and reads BYOK secrets via internal Rust `VaultStore`.

## 2. Additive SQLite WAL DDL (`V17a decision_audit_log`)

```sql
CREATE TABLE IF NOT EXISTS decision_audit_log (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    cascade_stage TEXT NOT NULL CHECK (cascade_stage IN ('DECISION_1','DECISION_2','RUBRIC_SHIELD')),
    tier_used TEXT NOT NULL CHECK (tier_used IN ('LOCAL_FTS5_BM25','GEMINI_DECISION_1','TYPESAFE_JEV','HEURISTIC_FALLBACK')),
    confidence_kind TEXT NOT NULL CHECK (confidence_kind IN ('ExactMatch','ModelVerdict','DeterministicFallback')),
    verdict_label TEXT NOT NULL,
    rubric_swap_consistent INTEGER CHECK (rubric_swap_consistent IN (0, 1)),
    attempt_number INTEGER NOT NULL CHECK (attempt_number BETWEEN 1 AND 3),
    breaker_tripped INTEGER NOT NULL DEFAULT 0 CHECK (breaker_tripped IN (0, 1)),
    latency_ms INTEGER NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decision_audit_project ON decision_audit_log(project_id, created_at);
```

## 3. Prisma Schema Parity

```prisma
model DecisionAuditLog {
  id                   String @id @default(uuid())
  projectId            String @map("project_id")
  cascadeStage         String @map("cascade_stage")
  tierUsed             String @map("tier_used")
  confidenceKind       String @map("confidence_kind")
  verdictLabel         String @map("verdict_label")
  rubricSwapConsistent Int?   @map("rubric_swap_consistent")
  attemptNumber        Int    @map("attempt_number")
  breakerTripped       Int    @default(0) @map("breaker_tripped")
  latencyMs            Int    @map("latency_ms")
  createdAt            BigInt @map("created_at")

  @@index([projectId, createdAt])
  @@map("decision_audit_log")
}
```

## 4. API Endpoints & Zod Contracts

- POST `/api/v1/decision/route`: Runs the 5-Stage Cascade with `egress_guard`, `VerdictShield`, `MAX_ATTEMPTS = 3` breaker, and automatic offline FTS5 BM25 fallback, persisting into `V17 decision_audit_log`.
- GET `/api/v1/decision/audit`: Queries verdict trace logs for QC and `Story 13.1` calibration.
- POST `/api/v1/de/run` & POST `/api/v1/vault/chat-ingest`: Consumed deterministic pre/post checks and Key Vault credentials.

```typescript
import { z } from "zod";

export const DecisionRouteRequestSchema = z.object({
  projectId: z.string().min(1),
  stage: z.enum(["DECISION_1", "DECISION_2", "RUBRIC_SHIELD"]),
  untrustedArtifactSha256: z.string().length(64),
  attemptNumber: z.number().int().min(1).max(3).default(1),
  staleFileSha256: z.string().length(64).optional(),
});
```
