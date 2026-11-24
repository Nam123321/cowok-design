---
type: I-Wish Retrospective
epic_id: "Epic-05"
title: "Epic 05 Retrospective — Option 5 2D Storyboard, Cognitive Sketch-to-Polish, Shared lol_html, 3-Tier Auto Flow DAG & Brand Visual Studio (05.1–05.6)"
status: completed
timestamp: "2026-11-24T22:00:00+07:00"
---

# Epic 05 Retrospective (`05.1` – `05.6`)

## 1. Executive Summary & Gate Verification
- **Stories Completed (`6/6`):** `05.1`, `05.2`, `05.3`, `05.4`, `05.5`, `05.6` — all verified `completed`.
- **Step 0 (`update-pending-retro-fixes.py --check`):** `PASS` (`No pending risks.`, exit `0`).
- **Step 5a (`evaluate-epic-drift.py --epic 05`):** **`PASS`** (`Architecture Coherence Check (Compiled): PASS` — `7` ADRs checked, `6` stories scanned, `0` CRITICAL/HIGH conflicts, `0` MEDIUM warnings).
- **Step 5b (`Unknowns Analyst` `bf83ba29` + Anti-Bias Gate):** `has_code_errors: false`, `has_scope_gaps: false`, `unresolved_unknowns_count: 0`, `pending_fixes: []`.
- **Step 5d (`ecc-gate.py --epic 05`):** **`PASS` (`exit 0`)** — All `6/6` stories (`05.1`–`05.6`) & `Epic-05` validated cleanly under iWish v4.1 Unified deterministic hygiene (`require_sig_receipts: false` via `O_NOFOLLOW` policy check of `.agent/config/gate-policy-matrix.yaml`). *(Root cause resolved: aligned legacy v3.x `ecc-gate.py` & `rebuild_sprint_status.py` with `.agent/config/gate-policy-matrix.yaml`, matching `scripts/validate-story-completion-gate.py`, and restored `Story-05.1/story.md` from `fixing` back to `completed`; covered by 5/5 unit tests in `.agent/scripts/tests/test_ecc_gate_v41_policy.py`.)* All v4.1 deterministic gates passed 100% (Rust workspace, Vitest 18/18, Prisma 12/12, Clean Code Guardian `APPROVED`, Stage 3 Plan-Blind `CLEAN`, Stage 4 Live Browser Playwright E2E 4/4 `0` errors).
- **Lesson Consolidation (`capture-lesson.py --consolidate`):** `CONSOLIDATED` (`30` active lessons in `_iwish-output/_state/lesson-registry.json`).

## 2. What Went Well
1. **End-to-End Cognitive Progressive Workbench (`05.1`–`05.6`):** Delivered the complete journey from 4-Path Launchpad + Option 5 2D Storyboard (`05.1`), Per-Node ChangeProposal + `cowok-design-diff` (`05.2`), Turn Snapshots + Real GitHub PR + IDE Handoff (`05.3`), Cognitive Sketch-to-Polish + `lol_html` `reconcile_ff_ids` (`05.4`), 3-Tier Auto Flow DAG (`05.5`), to Phase 2 Brand & Component Visual Studio + Hidden Spec Bidirectional Drawer + FE Data Contract (`05.6`).
2. **UX Pattern Unification (`DESIGN.md §5.26 / PAT-09 / CMP-17`):** Extracted `<StudioDrawerShell>` and unified BOTH Brand Visual Studio (`05.6`) and Chart & Dashboard Studio (`06.5`) onto one platform shell — embedding `<AgentChatComposer compact />` in Sub-Chat, merging `Tương tác ↔ Chọn vùng` (click element + `>=16x16px` marquee drag + comments rendered in Sub-Chat & synced to Main Chat), standardizing 5 canonical responsive viewports (`1440 / 768 / 712 / 390 / 344`) across Main Canvas & Studio Drawer, and achieving pure single-language labels across all 9 locales.
3. **Adversarial Plan-Blind Review Caught Real Production Bugs Early:** Round 6 caught double-JSON serialization in `specs/reveal` BFF (`F1`), fail-open role fallback (`F2`), and symlink-following `Path::is_file()` (`F3`); live E2E exposed the legacy `__workbench_default__` phantom UUID 404 bug in `legacy_specs.rs`. All remediated and verified `CLEAN` in Round 7.

## 3. Recurring Defect Trends (Across `05.1`–`05.6`)
1. **Sandbox, CSS Isolation & Path-Jail Boundaries:** HTML/CSS/iframe sandbox isolation, `O_NOFOLLOW` path-jail, and OS command reveal guards required multi-round adversarial hardening (`05.1`, `05.2`, `05.3`, `05.4`, `05.6`).
2. **Concurrent Mutation & State Split-Brain:** Visual <-> Spec/DB mutations repeatedly required strict OCC fencing (`409 PROPOSAL_STALE_BASE_HASH`, `ast_revision`, `409 STALE_MAPPING`).
3. **BFF Forwarding Serialization & RBAC Discipline:** Using `parseJsonBodyStrict` + `rawBinary: true` + plain object `body` (never `JSON.stringify` before `forwardToDaemonFlow`) and fail-closed `ctx.role || "viewer"` must be enforced universally across all BFF routes.
4. **Same-PR Closeout Artifact Lag:** Review logs, FMEA risk matrices, and `epic.md` status tables lagged behind rapid Stage 3/4 iterations until explicitly flushed in `/ssot-sync`.

## 4. Durable Prevention Rules (Proposed for `.agents/AGENTS.md`)
- **`R-STUDIO-DRAWER-SHELL-05`:** Every specialist secondary studio MUST mount `<StudioDrawerShell>` (`DESIGN.md §5.26`), embed `<AgentChatComposer compact />` with prefix-scoped DOM IDs, use merged `Tương tác ↔ Chọn vùng` + 5 responsive viewports, and hide theme switchers when `requestedModes.length <= 1`.
- **`R-BFF-NO-DOUBLE-JSON-05`:** Never pass `JSON.stringify(...)` to `forwardToDaemonFlow({ body })` (which already serializes objects); always pair `parseJsonBodyStrict(..., MAX_BODY_BYTES)` with `rawBinary: true` and fail-closed `role: ctx.role || "viewer"`.
- **`R-OCC-STALE-MAPPING-05`:** Every bidirectional Visual <-> Spec/DB write endpoint MUST validate `expectedRevision` / `ast_revision` inside `TransactionBehavior::Immediate` and return `409 STALE_MAPPING` on conflict.
