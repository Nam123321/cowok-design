# Edge Case Guardian & Adversarial Review Report — Story 06.1

- **Story:** Story 06.1 — Khung trình chiếu HTML Slide Engine (`16:9`/`16:10`/`Auto`), 8 Layout Templates, `V12` Schema Foundation & `HostRuntimeContract`
- **Epic:** Epic 06 (Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench)
- **Reviewer:** Review Agent (Edge Case Guardian Skill)
- **Risk Tier:** `3`
- **Verdict:** APPROVED (PASS)

## 1. FMEA & Boundary Evaluation
- **Identified Edge Cases (`EC-E06-01`, `EC-06-07`, `EC-06-13`, `EC-06-14`, `EC-06-15`, `EC-06-16`, `EC-06-17`):**
  1. Adaptive `--slide-scale >= 0.45` across `16:9`/`16:10`/`Auto` via `--slide-w`/`--slide-h`, removing hardcoded `480px` min-width, auto-collapsing chat rail `40px`, keeping projection type scale `16px..56px`, and isolating Dark-in-Warm decks from `.live-html-frame !important`.
  2. SQLite-safe 3-step `V12` `slide_uid` ULID backfill + `CREATE UNIQUE INDEX` inside one atomic transaction + positional `SELECT *` purge + atomic index swaps / UID regeneration on duplication.
  3. Structural ingestion guard for `.slide.active` + unique `[data-ff-id]`, IME `isComposing` arrow suppression, and deprecation of legacy `PresentationDualPaneView`.
  4. Method `1 + 4` Unified Stage Capability Adapter (`STAGE_CAPABILITY_BY_MODE`): unified `Walkthrough` trigger (`walkthrough-playback-btn`) + shared `WalkthroughFloatingBar` in Presentation mode (`showViewModeToggle=false`, zero Dual Screen toggle), FE viewport presets hidden in Presentation toolbar, responsive Row 2 multi-option dropdowns (`<540px`), and shared-Project `entryKind` (`fe-dag | presentation`) initialization (`EC-06-15..17`).
- **Mitigation Verified:** Embedded into EARS Acceptance Criteria `AC-01..AC-06` (`[EDGE-CASE]` `AC-04`, `AC-05`, `AC-06`) and Layers `L0..L10`, mapped 1-to-1 to `Task 06.1.1..06.1.6`.
- **Complexity Score (CS):** `6.5/10` (< 7 threshold).

## Formal-verify candidates
- **Protocol & Concurrency Statecharts Verified (`/formal-verify`):**
  1. **HostRuntimeContract `MessageChannel` Bootstrap & Monotonic Sequence Guard** (`useSandboxedIframeBridge.ts`, `sandbox-bridge-protocol.ts`): modeled in [`DeckHostRuntime.tla`](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish/verification/formal-specs/Story-06.1/DeckHostRuntime.tla). TLC proved `NeverExecuteBeforePortBound` and `SingleInitPerIdleCycle` across `132` model states (`307` transitions, `0` invariant violations).
  2. **Presentation `V12` OCC Revision Lock, Soft-Delete Trash Isolation & Legacy `PROMOTING` Lease Recovery** (`presentation_repo.rs`, `legacy_sessions.rs`): modeled in `DeckHostRuntime.tla`. TLC proved `NeverCommitOnTrashedOrStale` and caught seeded guard omission mutation across `33` counterexample states (`MUT_RC=1`).
  3. **Verification Manifest:** [verification-manifest.json](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20%26%20Story/VS-03.%20HTML-Native%2016%3A9%20Presentation%20%26%20Dual-Pane%20CMS%20Studio/Epic-06/Story-06.1/qa/formal-verification/verification-manifest.json).

## 2. Final Verdict
**VERDICT: PASS / APPROVED**
