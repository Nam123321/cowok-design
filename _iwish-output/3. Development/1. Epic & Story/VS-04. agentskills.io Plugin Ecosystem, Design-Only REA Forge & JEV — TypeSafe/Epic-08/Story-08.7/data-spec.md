---
type: I-Wish Data & Contract Spec
story_id: "08.7"
title: "Data & Contract Specification: Story 08.7 (UnifiedSkillCatalog, Dynamic Model Registry, Wire-Native Prompt Caching & Bridge 1 Studio Corrections)"
status: active
adr_ref: "ADR-009"
data_decision_records:
  - model: ModelRegistryProfile
    action: NEW_AGGREGATE
    bounded_context: bot-fleet
    justification: "Replaces hardcoded default_model_for_provider with dynamic, configurable model definitions and wire-native prompt caching capabilities."
  - model: StudioCorrectionSignal
    action: NEW_AGGREGATE
    bounded_context: plugin-host
    justification: "Tracks opt-in Zero-PII repeated manual corrections in Chart/Dashboard Studio; escalates signatures reaching hit_count >= 3 into V18a gate_failure_ledger for HUMAN_ONLY LEARN review."
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/ADRs/ADR-009-Zero-Hardcode-AI-Harness-Spine-And-Data-Storytelling-Feedback-Loop.md"
  - "./story.md"
---

# Data & Contract Specification — Story 08.7 (`ADR-009`)

## 1. Boundary Zod Contracts (`src/lib/validations/ai-harness.ts` ↔ `crates/domain` / `crates/harness-daemon`)

```typescript
import { z } from "zod";

export const SkillSourceBucketSchema = z.enum([
  "design_craft",
  "host_skills",
  "upstream_verbatim",
  "sqlite_plugin",
  "learn_card",
]);

export const UnifiedSkillEntrySchema = z.object({
  skillId: z.string().min(1).max(128),
  name: z.string().min(1).max(128),
  version: z.string().min(1).max(32),
  bucket: SkillSourceBucketSchema,
  description: z.string().min(1).max(600),
  tags: z.array(z.string().min(1).max(48)).max(24),
  referencePaths: z.array(z.string().min(1).max(256)).max(32),
  tier1TokenEstimate: z.number().int().min(1).max(100),
  contentSha256: z.string().regex(/^[0-9a-f]{64}$/),
});

export const SkillCatalogResponseSchema = z.object({
  catalogSha256: z.string().regex(/^[0-9a-f]{64}$/),
  tier1IndexSha256: z.string().regex(/^[0-9a-f]{64}$/),
  totalSkills: z.number().int().min(0),
  bucketCounts: z.record(SkillSourceBucketSchema, z.number().int().min(0)),
  skills: z.array(UnifiedSkillEntrySchema),
  warnings: z.array(z.string()).default([]),
});

export const ModelRegistryEntrySchema = z.object({
  id: z.string().min(1).max(128),
  providerId: z.string().min(1).max(32),
  modelId: z.string().min(1).max(128),
  displayName: z.string().min(1).max(128),
  tier: z.enum(["tier1_frontier", "tier2_fast", "tier3_local"]),
  contextWindow: z.number().int().min(4096).max(2_000_000),
  supportsPromptCache: z.boolean(),
  cacheStrategy: z.enum(["anthropic_ephemeral", "gemini_cached_content", "openai_cache_key", "none"]),
  isDefaultForProvider: z.boolean(),
  enabled: z.boolean(),
});

export const StudioCorrectionKindSchema = z.enum([
  "chart_type_override",
  "qa_autofix_click",
  "palette_contrast_fix",
  "density_mode_override",
  "callout_placement_fix",
]);

export const RecordStudioCorrectionRequestSchema = z.object({
  projectId: z.string().min(1).max(128),
  correctionKind: StudioCorrectionKindSchema,
  signatureKey: z.string().min(3).max(160).regex(/^[a-z0-9:_-]+$/),
  fromValue: z.string().min(1).max(64).optional(),
  toValue: z.string().min(1).max(64).optional(),
  skillId: z.string().min(1).max(128).optional(),
});

export const RecordStudioCorrectionResponseSchema = z.object({
  signatureKey: z.string(),
  hitCount: z.number().int().min(0),
  escalatedToLedger: z.boolean(),
  ledgerEntryId: z.string().nullable(),
  optOutSkipped: z.boolean().default(false),
});
```

## 2. Additive SQLite WAL Migration (`V24__story_08_7_model_registry_and_studio_corrections.sql`) & ADR-091 Prisma Mirror

- **DDL Authoritative File:** `crates/spec-registry/migrations/V24__story_08_7_model_registry_and_studio_corrections.sql`
- **Tables Added (2 additive tables, zero destructive edits):**
  1. `model_registry_profiles` (`id`, `tenant_id`, `provider_id`, `model_id`, `display_name`, `tier`, `context_window`, `supports_prompt_cache`, `cache_strategy`, `is_default`, `enabled`, `updated_at`) — unique index on `(tenant_id, provider_id, model_id)`.
  2. `studio_correction_signals` (`id`, `tenant_id`, `project_id`, `correction_kind`, `signature_key`, `from_value`, `to_value`, `skill_id`, `hit_count`, `escalated_ledger_id`, `first_seen_at`, `last_seen_at`) — unique index on `(tenant_id, project_id, signature_key)`.
- **Bridge 1 Invariant:** When `hit_count >= 3` and `escalated_ledger_id IS NULL`, atomically insert a Zero-PII row into `gate_failure_ledger` (`V18a`, `source = 'studio_correction_bridge1'`, `approval_policy = 'HUMAN_ONLY'`) inside `TransactionBehavior::Immediate` and store `escalated_ledger_id`.
- **Prisma Mirror:** Mirror `ModelRegistryProfile` and `StudioCorrectionSignal` in `prisma/schema.prisma` (validated via `npx prisma validate`, zero `prisma/migrations/`).

## 3. API Contracts

| Endpoint | Method | Auth Tier | Request / Response Schema |
| :--- | :---: | :---: | :--- |
| `/api/v1/skills/catalog` | `GET` | `READ_ONLY` | `SkillCatalogResponseSchema` (optional `?q=` multilingual VI/EN filter) |
| `/api/v1/skills/catalog/reload` | `POST` | `AGENT_FACING` | Triggers hot-reload of 4 buckets -> returns `SkillCatalogResponseSchema` |
| `/api/v1/models/registry` | `GET` | `READ_ONLY` | `{ models: ModelRegistryEntry[] }` |
| `/api/v1/learn/studio-corrections` | `POST` | `AGENT_FACING` | `RecordStudioCorrectionRequestSchema` -> `RecordStudioCorrectionResponseSchema` |
