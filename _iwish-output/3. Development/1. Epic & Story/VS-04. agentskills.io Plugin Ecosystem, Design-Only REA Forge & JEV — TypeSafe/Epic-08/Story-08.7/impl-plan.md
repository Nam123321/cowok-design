---
type: I-Wish Implementation Plan
story_id: "08.7"
risk_tier: 3
adr_ref: "ADR-009"
status: pending_approval
target_files:
  - packages/design-craft/**
  - packages/host-skills/**
  - packages/builtin-skills/design-craft/**
  - packages/builtin-skills/skills/**
  - crates/plugin-host/src/unified_catalog.rs
  - crates/plugin-host/src/lib.rs
  - crates/spec-registry/migrations/V24__story_08_7_model_registry_and_studio_corrections.sql
  - crates/spec-registry/src/skill_router.rs
  - crates/bot-fleet/src/model_registry.rs
  - crates/bot-fleet/src/byok_gateway.rs
  - crates/bot-fleet/src/lib.rs
  - crates/harness-daemon/src/routes/skills_catalog.rs
  - crates/harness-daemon/src/routes/models_registry.rs
  - crates/harness-daemon/src/routes/learn_studio_corrections.rs
  - crates/harness-daemon/src/routes/orchestrator.rs
  - crates/harness-daemon/tests/story_08_7_ai_harness_spine.rs
  - src/lib/validations/ai-harness.ts
  - src/api/ai-harness.ts
  - src/app/api/v1/skills/catalog/route.ts
  - src/app/api/v1/skills/catalog/reload/route.ts
  - src/app/api/v1/models/registry/route.ts
  - src/app/api/v1/learn/studio-corrections/route.ts
  - src/features/flow/components/dashboard-studio/studioCorrectionClient.ts
  - src/features/flow/components/dashboard-studio/useDashboardStudioState.ts
  - tests/unit/story-08.7-skills-refactor.test.ts
coordinator_glue_files:
  - crates/spec-registry/src/lib.rs
  - crates/harness-daemon/src/routes/mod.rs
  - crates/harness-daemon/src/lib.rs
  - prisma/schema.prisma
  - src/lib/vite-bff-routes.ts
---

# Implementation Plan — Story 08.7: Zero-Hardcode AI Harness Spine, 3-Bucket Skill Refactor, Wire-Native Prompt Caching & Bridge 1

## 1. Architecture & Execution Breakdown

### Phase A — 3-Bucket Skill Separation & Zero-Hardcode Skill Refactor (`Task 08.7.1`)
1. **Canonical 3-Bucket Layout (with zero-breakage compatibility for existing `Story 08.1–08.6` tests):**
   - Author canonical standalone `packages/design-craft/` (syncing `packages/builtin-skills/design-craft/` as assembly mirror or resolving `packages/design-craft/` first so `story-08.6-design-craft.test.ts` and `story_08_6_design_craft_mount.rs` stay 100% green).
   - Absorb the 3 broken cross-bucket skills (`flow-state-completer`, `reference-design-contract`, `self-healing-eval-gate`) into `packages/design-craft/skills/` and merge the 4 semantic split-brain pairs (`impeccable-design-polish` → `craft/impeccable-polish`, `html-presentation-storyteller` → `craft/stageshow-slide`, `design-taste-and-ux-guard` → `craft/oklch-palette` + `impeccable-polish`, `intent-clarifier-sdi` → `phase-0-intent-framing`).
   - Create `packages/host-skills/` containing strictly host-runtime skills (`cowok-host-spine`, `project-spec-synthesizer`, `research-decision-room`). Keep `packages/builtin-skills/upstream-skills/dashboard-design/` 100% verbatim untouched.
2. **Data Storytelling `data-viz-storyteller v2.1` + 27-Skill Refactor (Zero Hardcode / Zero Hardlink / Minimum Floor):**
   - Upgrade `data-viz-storyteller/{SKILL.md, references/storytelling-arc.md, references/chart-selection-matrix.md, references/html-canvas-contract.md}`:
     - Replace hardcoded `kpi-revenue-card` / `Q4 Revenue` demo anchors with **Semantic Role Contract** (`data-ff-role`, `data-row-id`, `data-whatif-handle`, `data-ff-callout-for`, `data-ff-compare-pair`, `data-audience-mode`).
     - Codify all **5 Core UX Patterns** (`AI Lens Strip >= 3`, `Contextual Point Actions 1-bar & 2-bar`, `5-Rule Honest & Wordless Auto-Fix + Undo`, `What-If Scenario Drag + Divergence Banner + Save Slide`, `Audience Mode CEO 3s ⇄ Chi tiết`) **+** **8 Open-Ended Extensible Patterns** (`Small Multiples`, `Event-Marker Bands`, `Cross-Filtering`, `Scrollytelling Reveal`, `Variance Waterfall`, `Distribution/Uncertainty`, `Slopegraph/Benchmark Bands`, `Interactive Sensitivity Sliders`).
   - Upgrade all **7 ultra-thin 9-line Craft Stubs** (`archify-diagram`, `dashboard-ux`, `impeccable-polish`, `oklch-palette`, `sketch-monochrome`, `stageshow-slide`, `typography`) to full `SKILL.md` (`<= 100 tok` frontmatter) + Tier-3 `references/*.md` (zero hardcoded token-name mismatches, zero static catalog counts, adaptive `16:9/16:10/Auto`, Minimum Floor `>= 4` states in `flow-state-completer` and `>= 8` dimensions in `user-simulation-8d`).

### Phase B — `UnifiedSkillCatalog` & 2-Breakpoint Cache-Safe Prompt Compiler (`Task 08.7.2`)
1. **`crates/plugin-host/src/unified_catalog.rs`:**
   - Dynamically discovers & merges 4 sources with `O_NOFOLLOW` + path-jail enforcement and per-plugin fault isolation: (a) `packages/host-skills`, (b) `mount_default_design_craft_plugin()`, (c) `upstream-skills/*` (verbatim MIT/Apache), (d) SQLite `skill_plugins (ACTIVE)` + `HUMAN_ONLY` approved `LEARN` cards.
   - Computes deterministic `catalog_sha256` + `zone1_prefix_sha256` (sorted by `skill_id`, Tier-1 metadata `<= 100 tok`/skill).
   - Provides multilingual (VI/EN) scoring via normalized tokenization + FTS5/BM25 lexical & tag matching + `DecisionCascadeClient` tie-breaking.
2. **`crates/spec-registry/src/skill_router.rs`:**
   - Replace hardcoded 7-item `CRAFT_SKILL_SPECS` matching with `UnifiedSkillCatalog`-backed routing (preserving backward-compatible public symbol signatures for existing Story 02.4 tests while routing dynamically when catalog is attached).
   - Enforce **2-Breakpoint Cache-Safe Compilation**: Zone 1 (`zone1_prefix_sha256` 100% invariant across turns — System + Tool Schemas + Tier-1 Skill Index + `DESIGN.md` hash) strictly separated from Zone 2 (JIT Tier-2 `SKILL.md` body + Tier-3 lazy `references/*.md` + 75% Cache-Safe Compaction).
3. **`crates/harness-daemon/src/routes/{skills_catalog.rs, orchestrator.rs}`:**
   - Wire `UnifiedSkillCatalog` into daemon boot + `POST /api/v1/orchestrator/chat`, exposing `GET /api/v1/skills/catalog` and `POST /api/v1/skills/catalog/reload`.

### Phase C — Dynamic Model Registry & Wire-Native Provider Prompt Caching (`Task 08.7.3`)
1. **`crates/bot-fleet/src/model_registry.rs` & `V24` SQLite Migration:**
   - Seed default providers (`claude`, `gemini`, `openai`, `openrouter`, `deepseek`, `byos`) into `model_registry_profiles` (`V24`), replacing hardcoded `default_model_for_provider` lookups with live registry resolution + `GET /api/v1/models/registry`.
2. **`crates/bot-fleet/src/byok_gateway.rs`:**
   - Emit wire-native caching payloads per provider capability:
     - **Anthropic (`claude`):** `cache_control: { "type": "ephemeral" }` at 2 breakpoints (Zone 1 static system prefix + tool/history boundary).
     - **Gemini (`gemini`):** `cachedContent` handle lifecycle when prefix exceeds threshold.
     - **OpenAI (`openai`):** deterministic `prompt_cache_key: format!("cw:{}:{}", project_id, catalog_sha256[..16])`.
   - Record Zero-PII `cache_read_tokens` / `cache_creation_tokens` telemetry; auto-fallback cleanly without caching on `400`/`501` from custom OpenAI-compatible endpoints or air-gapped runs.

### Phase D — Bridge 1 (`Manual Studio Correction → V18a gate_failure_ledger`, `Task 08.7.4`)
1. **`crates/harness-daemon/src/routes/learn_studio_corrections.rs`:**
   - Endpoint `POST /api/v1/learn/studio-corrections` validates Zero-PII payload (`RecordStudioCorrectionRequestSchema`), respects telemetry/learning opt-out, increments `studio_correction_signals.hit_count` in `TransactionBehavior::Immediate`, and escalates at `hit_count >= 3` into `V18a gate_failure_ledger` (`HUMAN_ONLY`, never auto-activating skills).
2. **`src/features/flow/components/dashboard-studio/studioCorrectionClient.ts`:**
   - Fire-and-forget non-blocking client hook fired from `useDashboardStudioState.ts` when user clicks `✦ Chuẩn hóa` (`qa_autofix_click`), overrides AI-selected chart type (`chart_type_override`), or switches palette/density post-generation.

## 2. Four-Pillar Risk & Edge-Case Verification (`/plan-proven-safe`)
- **Drift & Contract Safety:** Keep backward-compatible Rust/TS exports so all 493 existing Vitest tests + `story_02_4` + `story_08_6_design_craft_mount` remain 100% green.
- **Security & Path Jail:** Every dynamically loaded `SKILL.md` and `references/*.md` passes `O_NOFOLLOW` + canonical root containment; corrupt plugin isolation logs warning without crashing catalog.
- **Cache-Key Invariance:** Dedicated unit test proves `zone1_prefix_sha256` stays byte-identical across 3 different Vietnamese & English intents activating different Tier-2/Tier-3 skills (including `data-viz-storyteller` & `dashboard-design`).
- **Zero-PII & `HUMAN_ONLY` Bridge 1:** Strict regex/allowlist rejects raw CSV data in `signatureKey`, requires `hit_count >= 3`, and never flips `skill_plugins.state = 'ACTIVE'` automatically.

## 3. Verification Plan
- `cargo test -p plugin-host -p spec-registry -p bot-fleet -p harness-daemon` (including new `story_08_7_ai_harness_spine.rs` with wire-mock HTTP checks for Anthropic/Gemini/OpenAI caching + fallback + Bridge 1 threshold)
- `npx vitest run tests/unit/story-08.7-skills-refactor.test.ts tests/unit/story-08.6-design-craft.test.ts` (asserts 0 hardcoded demo anchors, 0 broken cross-bucket links, `<= 100 tok` frontmatter across all skills, Tier-3 `references/*.md` present on all 7 craft skills + `data-viz-storyteller v2.1`)
- `pnpm run typecheck && pnpm run build && npx prisma validate`
