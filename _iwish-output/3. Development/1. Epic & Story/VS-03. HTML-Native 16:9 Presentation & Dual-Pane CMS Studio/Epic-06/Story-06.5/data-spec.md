---
type: I-Wish Data & Contract Spec
story_id: "06.5"
title: "Data, V15 Multi-Slide Generation & ChartSpecV12 Normalizer Specification: Story 06.5"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: PresentationDeck
    action: EXTENDS_AGGREGATE
    bounded_context: presentation-cms
    justification: "Retires presentation_copilot.rs:1767-1782 echo stub and writes real multi-slide AI decks + normalized ChartSpecV12 into V15 tables"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & `ChartSpecV12` Contract Specification — Story 06.5

## 1. Bounded Context & `V15` Atomic Deck Staging

Story `06.5` wires Slide Copilot and Deck Launchpad through `crates/bot-fleet` (`11.1`) and 5-Stage Decision Cascade (`09.2`), enforces `stageshow` frame constraints (`<Stage>`, `<Window>`, `<Phone>`, `3-7` words/bullet, `<=30` words/slide), normalizes `.csv/.tsv/.pdf` CAS sources (`04.6`) into `ChartSpecV12` (`MAX_SERIES = 12`, `MAX_POINTS = 200`, proportional sum `100 +/- 0.05` while preserving raw values & units in tooltip/legend labels), and stages HTML atomically via `.tmp + fsync + WAL` into `V15 presentation_decks`, `deck_slides`, and `wording_slots`.

## 2. Prisma Schema Parity

```prisma
model PresentationDeck {
  id           String @id @default(uuid())
  projectId    String @map("project_id")
  title        String
  aspectRatio  String @default("16:9") @map("aspect_ratio")
  framePreset  String @default("Stage") @map("frame_preset")
  revision     BigInt @default(1)
  updatedAt    BigInt @map("updated_at")

  @@index([projectId])
  @@map("presentation_decks")
}
```

## 3. API Endpoints & Zod Contracts

- POST `/api/v1/decks/generate`: Generates complete multi-slide deck into `V15` tables using real `crates/bot-fleet` (`0` echo stubs) and `stageshow` budget (`<=30` words/slide).
- POST `/api/v1/slides/copilot/rewrite`: Rewrites targeted `slide_uid` / `[data-ff-id]` slots through real LLM + `lol_html` sanitizer (`presentation_copilot.rs` stub retired).
- POST `/api/v1/decks/normalize-chart-source`: Parses CAS `.csv/.tsv/.pdf` SHA-256 into validated `ChartSpecV12` (`sum = 100 +/- 0.05`, preserved raw units in labels).
- POST `/api/v1/execution/run` & POST `/api/v1/decision/route`: Consumed execution & routing services.

```typescript
import { z } from "zod";

export const NormalizeChartSourceRequestSchema = z.object({
  projectId: z.string().min(1),
  casSha256: z.string().length(64),
  chartKind: z.enum(["bar", "line", "pie", "donut", "funnel", "kpi"]),
  maxSeries: z.number().int().min(1).max(12).default(12),
  maxPoints: z.number().int().min(1).max(200).default(200),
});
```
