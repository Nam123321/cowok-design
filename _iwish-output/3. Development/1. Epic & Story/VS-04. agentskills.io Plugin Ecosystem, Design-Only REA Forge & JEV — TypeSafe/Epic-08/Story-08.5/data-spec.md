---
type: I-Wish Data & Contract Spec
story_id: "08.5"
title: "Data, Split DE+DR Plugin Runtime, Tool Jail & V18 LEARN Ledger Specification: Story 08.5"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-007-Split-DE-DR-Plugin-Runtime-Tool-Jail-and-Sealed-Eval.md"
data_decision_records:
  - model: UserSkillRegistry
    action: NEW_AGGREGATE
    bounded_context: plugin-ecosystem
    justification: "V16 user_skill_registry stores scanned user/imported skills packaged into JIT progressive disclosure (<=100 tok header)"
  - model: GateFailureLedger
    action: NEW_AGGREGATE
    bounded_context: self-evolution
    justification: "V18 gate_failure_ledger captures deterministic gate failures for System-1 LEARN filtering (hit_count>=3, SLPR>=0.95, auto-retire)"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Security Specification — Story 08.5

## 1. Bounded Context & Split DE+DR (`ADR-007`)

Story `08.5` separates **Deterministic Engineering (DE)** tools (7 built-in `$0`, `<=25ms` linear-NFA checks) from **Design/Domain Reasoning (DR)** skills (`SKILL.md`), secures user repo imports & ephemeral tool execution inside a `HUMAN_ONLY` Jail (`env_clear()`, `O_NOFOLLOW`, `<=5s` timeout, path-jail), registers user skills in `V16 user_skill_registry`, and filters `V18 gate_failure_ledger` (`hit_count >= 3`, Jev `noul` dedupe, `SLPR >= 0.95`, auto-retire) before proposing cards to `Story 08.4` `knowledge_cards`.

## 2. Additive SQLite WAL DDL (`V16` & `V18`)

```sql
CREATE TABLE IF NOT EXISTS user_skill_registry (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    skill_slug TEXT NOT NULL,
    repo_source_url TEXT,
    header_tokens INTEGER NOT NULL CHECK (header_tokens <= 100),
    security_scan_verdict TEXT NOT NULL CHECK (security_scan_verdict IN ('PASS','QUARANTINED','REJECT')),
    license_classification TEXT NOT NULL CHECK (license_classification IN ('ADOPT_PRIOR_ART','REFERENCE_ONLY','REJECT')),
    skill_md_path TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE(project_id, skill_slug)
);

CREATE TABLE IF NOT EXISTS gate_failure_ledger (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    rule_id TEXT NOT NULL,
    fingerprint_sha256 TEXT NOT NULL,
    hit_count INTEGER NOT NULL DEFAULT 1,
    slpr_score REAL NOT NULL DEFAULT 0.0,
    promoted_card_id TEXT,
    status TEXT NOT NULL DEFAULT 'ACCUMULATING' CHECK (status IN ('ACCUMULATING','PROPOSED','PROMOTED','RETIRED')),
    last_seen_at BIGINT NOT NULL,
    UNIQUE(project_id, fingerprint_sha256)
);
```

## 3. Prisma Schema Parity

```prisma
model UserSkillRegistry {
  id                    String  @id @default(uuid())
  projectId             String? @map("project_id")
  skillSlug             String  @map("skill_slug")
  repoSourceUrl         String? @map("repo_source_url")
  headerTokens          Int     @map("header_tokens")
  securityScanVerdict   String  @map("security_scan_verdict")
  licenseClassification String  @map("license_classification")
  skillMdPath           String  @map("skill_md_path")
  createdAt             BigInt  @map("created_at")

  @@unique([projectId, skillSlug])
  @@map("user_skill_registry")
}

model GateFailureLedger {
  id                String  @id @default(uuid())
  projectId         String  @map("project_id")
  ruleId            String  @map("rule_id")
  fingerprintSha256 String  @map("fingerprint_sha256")
  hitCount          Int     @default(1) @map("hit_count")
  slprScore         Float   @default(0.0) @map("slpr_score")
  promotedCardId    String? @map("promoted_card_id")
  status            String  @default("ACCUMULATING")
  lastSeenAt        BigInt  @map("last_seen_at")

  @@unique([projectId, fingerprintSha256])
  @@map("gate_failure_ledger")
}
```

## 4. API Endpoints & Zod Contracts

- POST `/api/v1/de/run`: Executes the 7 Built-in DE tools (`scan-ui`, `intake`, `tokens`, `tells/impeccable/hallmark/stageshow` <=25ms, `archify`, `get_artifact`, `notes-check`) matching CLI `uiux-os de-check`.
- POST `/api/v1/plugins/import-repo`: Validates SSRF + License/ToS + AST security, packages user repo/skill (`<=100` tok header) into `V16 user_skill_registry`.
- POST `/api/v1/plugins/ephemeral-tools/execute`: Strictly **`HUMAN_ONLY`** gate (`403 HUMAN_APPROVAL_REQUIRED` for non-human actors); runs AST-verified script inside `env_clear()` + `O_NOFOLLOW` + `<=5s` sandbox.
- GET `/api/v1/learn/ledger`: Returns `V18a gate_failure_ledger` candidates qualifying for `hit_count >= 3` and `SLPR >= 0.95` (`promote >= 0.95`, `auto-retire < 0.85` hysteresis) before proposing cards to `LEARN Queue` via POST `/api/v1/projects/:id/learn/cards` (`HUMAN_ONLY` UI approval via `08.4`).

```typescript
import { z } from "zod";

export const RunBuiltInDeRequestSchema = z.object({
  projectId: z.string().min(1),
  tools: z.array(z.enum(["scan-ui", "intake", "tokens", "nfa-packs", "archify", "get_artifact", "notes-check"])).min(1),
  targetArtifactPath: z.string().min(1),
});
```
