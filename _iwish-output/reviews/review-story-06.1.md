# Edge Case Guardian & Adversarial Review Report — Story 06.1

- **Story:** Story 06.1 — Khung trình chiếu HTML Slide Engine (`16:9`/`16:10`/`Auto`), 8 Layout Templates, `V12` Schema Foundation & `HostRuntimeContract`
- **Epic:** Epic 06 (Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench)
- **Reviewer:** Review Agent (Edge Case Guardian Skill)
- **Risk Tier:** `3`
- **Verdict:** APPROVED (PASS)

## 1. FMEA & Boundary Evaluation
- **Identified Edge Cases (`EC-E06-01`, `EC-06-07`, `EC-06-13`, `EC-06-14`):**
  1. Adaptive `--slide-scale >= 0.45` across `16:9`/`16:10`/`Auto` via `--slide-w`/`--slide-h`, removing hardcoded `480px` min-width, auto-collapsing chat rail `40px`, keeping projection type scale `16px..56px`, and isolating Dark-in-Warm decks from `.live-html-frame !important`.
  2. SQLite-safe 3-step `V12` `slide_uid` ULID backfill + `CREATE UNIQUE INDEX` inside one atomic transaction + positional `SELECT *` purge + atomic index swaps / UID regeneration on duplication.
  3. Structural ingestion guard for `.slide.active` + unique `[data-ff-id]`, IME `isComposing` arrow suppression, and deprecation of legacy `PresentationDualPaneView`.
- **Mitigation Verified:** Embedded into EARS Acceptance Criteria `AC-01..AC-05` (`[EDGE-CASE]` `AC-04`, `AC-05`) and Layers `L0..L10`, mapped 1-to-1 to `Task 06.1.1..06.1.5`.
- **Complexity Score (CS):** `6.0/10` (< 7 threshold).

## 2. Final Verdict
**VERDICT: PASS / APPROVED**
