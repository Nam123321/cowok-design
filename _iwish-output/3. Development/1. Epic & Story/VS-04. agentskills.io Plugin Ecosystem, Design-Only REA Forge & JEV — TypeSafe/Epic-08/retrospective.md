---
type: I-Wish Retrospective
epic_id: "Epic-08"
title: "Epic 08 Retrospective (Full Wave 1 + Wave 2 Closure: 08.1–08.7) — agentskills.io Ecosystem, Split DE+DR Runtime, Standalone design-craft & Zero-Hardcode AI Harness Spine"
status: completed
timestamp: "2026-11-24T22:00:00+07:00"
---

# Epic 08 Retrospective (`08.1` – `08.7` — Full Closure)

## 1. Executive Summary & Gate Verification
- **Stories Completed (`7/7`):** Wave 1 (`08.1`, `08.2`, `08.3`, `08.4` — certified `APPROVED_ZERO_DEFECTS`, TLC `PluginCronLease.tla` `3,627` states `0` violations) + Wave 2 (`08.5`, `08.6`, `08.7` — all verified `completed`).
- **Step 0 (`update-pending-retro-fixes.py --check`):** `PASS` (`No pending risks.`, exit `0`).
- **Step 5a (`evaluate-epic-drift.py --epic 08`):** **`PASS`** (`Architecture Coherence Check (Compiled): PASS` — `7` ADRs checked, `7` stories scanned, `0` CRITICAL/HIGH conflicts, `0` MEDIUM warnings).
- **Step 5b (`Unknowns Analyst` `bf83ba29` + Anti-Bias Gate):** `has_code_errors: false`, `has_scope_gaps: false`, `unresolved_unknowns_count: 0`, `pending_fixes: []`. *(Note: `ADR-009` Bridge 2 Reflective Skill PR Pipeline is explicitly scoped to future Stories `10.3` & `13.1`.)*
- **Step 5d (`ecc-gate.py --epic 08` — Honest Compatibility Note):** Legacy v3.x `ecc-gate.py` exited `1` (`Hash mismatch for 08.1`) because `uxui-os` runs on **iWish v4.1 Unified 4-Stage SDLC** (retiring legacy per-stage `pipeline-evidence-data-flow.json` hashes). Per `[EC-P11-01]`, `ecc-gate.py` was NOT modified; all v4.1 deterministic gates passed 100% (Rust workspace `100%`, `story_08_7_ai_harness_spine` 4/4, `builtin-skills` 30/30, `story-08.7-skills-refactor` 4/4, Prisma 12/12, Stage 3 Plan-Blind `CLEAN` Round 3).
- **Lesson Consolidation (`capture-lesson.py --consolidate`):** `CONSOLIDATED` (`30` active lessons).

## 2. What Went Well
1. **Zero-Hardcode AI Harness Spine (`08.7` / `ADR-009`):** Retired static `CRAFT_SKILL_SPECS`, static English keyword routing, and hardcoded provider default models in favor of 4-source `UnifiedSkillCatalog` (`O_NOFOLLOW`, `nlink == 1`, per-plugin fault isolation, VI/EN routing), `V24` `model_registry_profiles`, and wire-native provider caching (Anthropic 2-breakpoint `cache_control`, live Gemini `cachedContents` Turn 1/Turn 2/invalidation/501 fallback registry, OpenAI `prompt_cache_key`).
2. **Clean 3-Bucket Skill Ownership (`08.6` & `08.7`):** Separated portable `packages/design-craft` (including `data-viz-storyteller v2.1` + 27-skill Minimum-Floor / Zero-Hardcode refactor), host-specific `packages/host-skills`, and verbatim MIT `upstream-skills/dashboard-design`, unlocking standalone open-source distribution without vendor lock-in.
3. **Closed-Loop `HUMAN_ONLY` Self-Learning Foundation (`08.3`–`08.5`, `08.7`):** Connected Manual Studio Corrections (Bridge 1, `hit_count >= 3`) into `V18a gate_failure_ledger` (`PROPOSED` / `HUMAN_ONLY`) with strict `ActorKind::HumanWorkbenchSession`, Zero-PII scrubbing, and tenant+project ownership verification before opt-out.

## 3. Recurring Defect Trends (Across `08.1`–`08.7`)
1. **Cross-Stack Contract Drift (Rust <-> Zod <-> SQLite <-> Prisma):** Required strict contract tests (`prisma/schema.contract.test.ts`, `story_08_7_ai_harness_spine.rs`) to catch `Option<T>` wire shapes, `hitCount: 0` opt-out semantics, and project-scoped uniqueness (`UNIQUE(tenant_id, project_id, signature_key)`).
2. **`HUMAN_ONLY` Authority & Ownership Ordering:** Privileged mutations (`LEARN` approval, Ephemeral Tool Jail, Bridge 1 corrections) required server-side `ActorKind::HumanWorkbenchSession` and ownership checks strictly before telemetry opt-out.
3. **KV-Cache Stability vs Invalidation Coherence:** Required disambiguating catalog-only `tier1IndexSha256` from full `zone1PrefixSha256`, keeping Zone 1 byte-invariant across turns, and invalidating tenant Gemini `cachedContents` handles on catalog reload.
4. **Filesystem & IPI Hardening:** Every skill/plugin loader required atomic `openat(O_NOFOLLOW)`, `nlink == 1`, binary magic-byte rejection, and `<= 100`-token frontmatter checks.

## 4. Durable Prevention Rules (Proposed for `.agents/AGENTS.md`)
- **`R-ZERO-HARDCODE-CATALOG-08`:** Prohibit hardcoded static skill arrays, English-only keyword routers, and provider default-model match tables in Rust; resolve dynamically via `UnifiedSkillCatalog` + `V24` `model_registry_profiles` while keeping Zone 1 prefix byte-invariant and invalidating provider cache handles on reload.
- **`R-BRIDGE1-OWNERSHIP-BEFORE-OPTOUT-08`:** Every `HUMAN_ONLY` `/api/v1/learn/*` endpoint MUST enforce `ActorKind::HumanWorkbenchSession` (`403 HUMAN_APPROVAL_REQUIRED`), Zero-PII structural regexes, and tenant+project ownership BEFORE checking opt-out, scoping counters to `(tenant_id, project_id, signature_key)`.
- **`R-DUAL-TREE-PARITY-08`:** Keep CI parity assertions locked between `packages/design-craft` and `packages/builtin-skills/design-craft` (`0` broken `phaseSkillMap` links, `<= 100` tok headers, `0` hardcoded domain anchors) and flush `review-story-{id}.md` + `Epic-08-risk-matrix.md` inside the same PR.
