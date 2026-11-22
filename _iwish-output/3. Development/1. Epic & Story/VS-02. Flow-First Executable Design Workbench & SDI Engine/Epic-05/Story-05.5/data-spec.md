---
type: I-Wish Data & Contract Spec
story_id: "05.5"
title: "Data & 3-Tier DAG Synthesis Contract Specification: Story 05.5"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: FlowNodeSubstate
    action: EXTENDS_AGGREGATE
    bounded_context: flow-graph
    justification: "Maps /simulate-user 8D and FMEA edge-cases into 3-tier flow_nodes/flow_edges + screen_fixtures with <=8 default happy-path clustering"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & DAG Synthesis Contract Specification — Story 05.5

## 1. Bounded Context & 3-Color DFS Invariants

Story `05.5` compiles `/simulate-user` (8 dimensions) and `/edge-case-guardian` (FMEA) traces into hierarchical DAG structures inside `crates/flow-graph`: grouping primary user journeys into `<= 8` happy-path clusters (`tier: "PRIMARY"`), attaching secondary `ERROR` / `EMPTY` / `FALLBACK` branches (`tier: "EDGE_CASE"`), binding 4 per-node SubStates (`DEFAULT`, `LOADING`, `EMPTY`, `ERROR`) to `screen_fixtures`, enforcing zero cycles/orphan edges via 3-Color DFS, and enforcing `0` raw float score leakage via `VerdictShield`.

## 2. Prisma Schema Parity

```prisma
model FlowNodeSubstate {
  nodeId       String @map("node_id")
  projectId    String @map("project_id")
  substate     String
  fixtureId    String @map("fixture_id")
  flowTier     String @default("PRIMARY") @map("flow_tier")
  severityBand String @default("LOW") @map("severity_band")
  updatedAt    BigInt @map("updated_at")

  @@id([nodeId, substate])
  @@index([projectId, flowTier])
  @@map("flow_node_substates")
}
```

## 3. API Endpoints & Zod Contracts

- POST `/api/v1/flows/synthesize`: Generates 3-Tier Flow DAG (`<= 8` happy-path default cluster nodes), binds 4 SubStates to `SampleDataStudio` fixtures, runs 3-Color DFS cycle/orphan guard, persists `flow_nodes` and `flow_edges`.
- POST `/api/v1/decision/route`: Consumed from `Story 09.2` with `VerdictShield` (`ConfidenceKind` ∈ `ExactMatch | ModelVerdict | DeterministicFallback`, zero floats).

```typescript
import { z } from "zod";

export const FlowSynthesizeRequestSchema = z.object({
  projectId: z.string().min(1),
  briefSummary: z.string().min(1),
  maxDefaultClusterNodes: z.number().int().min(1).max(8).default(8),
  includeFmeaBranches: z.boolean().default(true),
});

export const SynthesizedFlowSummarySchema = z.object({
  primaryClusterCount: z.number().int().max(8),
  edgeCaseBranchCount: z.number().int().min(0),
  substateCoveragePercent: z.literal(100),
  floatLeakCount: z.literal(0),
});
```
