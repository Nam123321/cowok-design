---
type: I-Wish Implementation Plan
story_id: "07.1"
epic_id: "07"
risk_tier: 3
migration_slot: "V26__story_07_1_qc_gate_runs.sql"
parallel_wave: "Wave-C"
status: pending_approval
target_files:
  - crates/spec-registry/migrations/V26__story_07_1_qc_gate_runs.sql
  - crates/html-forge/src/visual_qc.rs
  - crates/html-forge/tests/story_07_1_visual_qc.rs
  - crates/harness-daemon/src/routes/visual_qc.rs
  - crates/harness-daemon/tests/story_07_1_visual_qc.rs
  - src/lib/validations/visual-qc.ts
  - src/api/visual-qc.ts
  - src/app/api/v1/qc/run/route.ts
  - src/app/api/v1/qc/auto-fix/route.ts
  - src/features/visual-qc/VisualQcOverlayPanel.tsx
  - src/features/visual-qc/ProjectReadinessModal.tsx
  - src/features/visual-qc/LintFixActionButton.tsx
  - src/features/visual-qc/useVisualQcController.ts
  - src/features/visual-qc/visual-qc.test.tsx
  - tests/e2e/story-07.1.spec.ts
coordinator_shared_wiring:
  - crates/html-forge/src/lib.rs
  - crates/spec-registry/src/pool.rs
  - crates/harness-daemon/src/routes/mod.rs
  - crates/harness-daemon/src/routes/legacy_router.rs
  - prisma/schema.prisma
  - prisma/schema.contract.test.ts
  - src/lib/vite-bff-routes.ts
  - src/pages/workbench/WorkbenchShell.tsx
  - src/i18n/locales/*.json
---

# Implementation Plan — Story 07.1 (Pooled 1-Chromium CDP Visual QC, Real RGBA SSIM, E2E Flow Completeness & 1-Click Auto-Fix)

## 1. Architectural Strategy
1. **Additive SQLite WAL Migration (`V26`):** Create `qc_gate_runs` with `layout_sha1` composite cache index (`project_id, layout_sha1, created_at`) and `autofix_attempt CHECK (BETWEEN 0 AND 3)`. Mirror `QcGateRun` into `prisma/schema.prisma` (ADR-091 read-only mirror).
2. **Visual QC Core Engine (`crates/html-forge/src/visual_qc.rs`):**
   - `PooledCdpEvaluator` with semaphore concurrency `<= 1` Chromium instance (`<250MB` RSS), ephemeral `IncognitoBrowserContext` per run, and circuit-breaker crash/OOM recovery.
   - Honest fallback `iframe_bridge_fallback` -> emits `verdict: "UNVERIFIED_NO_BROWSER"` when no local Chromium binary is available (never fakes SSIM PASS).
   - 25+ geometry/WCAG 2.2 AA rules (`lib-audit` across canonical 5 viewports `1440 / 768 / 712 / 390 / 344` + `1024` & sweep `320..1440`, plus `slides-check` `1920x1080` `CONTENT_MAX_Y`), Real RGBA SSIM computation (`compute_rgba_ssim`), `elementsFromPoint` occlusion verification, cross-origin iframe sandbox guard (`sandbox="allow-scripts"`).
   - E2E Flow Completeness Auditor scanning Flow DAG for: (a) Unwired Dead-End CTAs, (b) Orphan Screens unreachable from entry node, (c) Missing 4-SubStates (`default | loading | empty | error`).
   - `<10ms` deterministic `lint_fix` (`0` LLM tokens) for token/spacing/radius/contrast fixes with reversible pre-patch snapshot (`Cmd+Z`), plus bounded `MAX_ATTEMPTS = 3` cap for Agent surgical patches.
3. **Daemon & BFF Routes (`crates/harness-daemon/src/routes/visual_qc.rs` + `src/app/api/v1/qc/*`):**
   - `POST /api/v1/qc/run` (supports `layout_sha1` cache hit + `forceRefresh`, persists `qc_gate_runs` inside `TransactionBehavior::Immediate`).
   - `POST /api/v1/qc/auto-fix` (`DETERMINISTIC_LINT_FIX` `<10ms` or `AGENT_PATCH` capped at `attemptNumber <= 3`; rejects attempt `> 3` with `422 AUTOFIX_MAX_ATTEMPTS_EXCEEDED`).
   - Strict BFF forwarding (`parseJsonBodyStrict` + `rawBinary: true` + fail-closed `ctx.role || "viewer"` + Zod `.parse()` in `src/api/visual-qc.ts`).
4. **Frontend UI (`src/features/visual-qc/`):**
   - `VisualQcOverlayPanel.tsx` (`[data-ff-id='visual-qc-overlay']`) rendering coordinate bounding boxes & viewport bar over preview without mutating iframe source.
   - `ProjectReadinessModal.tsx` (`[data-ff-id='project-readiness-panel']`) summarizing P0/P1 counts, E2E Flow Completeness findings, Real SSIM score or `UNVERIFIED_NO_BROWSER` honest badge.
   - `LintFixActionButton.tsx` (`[data-ff-id='one-click-lint-fix-button']`) executing `<10ms` deterministic fix + 1-step rollback (`Cmd+Z`) + hard lock after 3 agent attempts.

## 2. Verification & TDD Gates
- Rust: `cargo test -p html-forge --test story_07_1_visual_qc` & `cargo test -p harness-daemon --test story_07_1_visual_qc`
- Frontend: `npx vitest run src/features/visual-qc/visual-qc.test.tsx` & `pnpm run typecheck`
- Stage 3: Dual-Engine Plan-Blind Adversarial Review (`risk_tier: 3`)
- Stage 4: Live Browser Playwright E2E `tests/e2e/story-07.1.spec.ts` against allocated `http://127.0.0.1:<PORT>`
