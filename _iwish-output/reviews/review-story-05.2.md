# Stage 4 Edge Case Guardian & Adversarial Review Report: Story 05.2

- **Story:** [Story 05.2: Per-Node / Batch ChangeProposal, cowok-design-diff (pixelmatch + SSIM + changed_bbox), Designer Talk (T) & Zero-Tailwind Cleanup](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-05/Story-05.2/story.md)
- **Review Engine:** Edge Case Guardian (13-Pillar FMEA Scan)
- VERDICT: APPROVED (Score: 9.5/10)
- **Status:** `PROVEN_SAFE`

## 1. 13-Pillar FMEA Risk Analysis Table

| Failure Mode ID | Potential Failure / Edge Case Mode | Severity (S) | Occurrence (O) | Detection (D) | RPN | Mitigation & Remediation Action |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **FMEA-05.2-01** | Client sends `validationStatus: "passed"` directly without real AST/Typecheck/Build receipt | 5 | 2 | 1 | **10** | Server ignores client validation claims; Apply verifies daemon-signed HMAC-SHA256 `proposal_gate_receipts` (`exit_code === 0`, `diff_sha256`, live `workspace_head_sha256`) or returns `412 UNVERIFIED_GATE_CLAIM`. |
| **FMEA-05.2-02** | Node-A & Node-B concurrent proposals overwrite `src/styles/tokens.css` or `SharedAppShell` silently | 5 | 2 | 1 | **10** | Validate `base_file_hashes_json` inside `TransactionBehavior::Immediate`; abort with `409 PROPOSAL_STALE_BASE_HASH` on drift. |
| **FMEA-05.2-03** | Oversized RGBA buffer or missing Chromium causes visual diff OOM / HTTP 500 | 4 | 2 | 1 | **8** | Bound dimensions (`<= 1280x800`), YIQ `pixelmatch` (`0.1` threshold) + SSIM, and graceful `RASTERIZER_UNAVAILABLE` structured fallback. |
| **FMEA-05.2-04** | Designer Talk (`T`) floods DOM (>12 notes) or orphans invalid `data-ff-id` targets | 4 | 2 | 1 | **8** | Enforce `badge_index BETWEEN 1 AND 12` in SQLite V10 + Zod and non-empty `element_ff_id` binding. |
| **FMEA-05.2-05** | Tailwind utility leakage or `#FF9A2F`/`#e89a35` amber tokens in `src/features/proposals/**` | 4 | 2 | 1 | **8** | Pure Vanilla CSS classes (`.cp-*`) backed by `src/styles/tokens.css`, enforced by `verify-design-tokens.py` + `AstLinter`. |

## 2. Adversarial Boundary Verification
- **RPN Threshold:** All RPN scores `<= 10` (`< 100` threshold).
- **Verdict:** All FMEA edge-case mitigations bound into `data-spec.md`, `impl-plan.md`, and automated tests.

## 3. Formal-verify candidates Triage
- **Formal-verify candidates:** `ProposalCasApplyAndHmacReceiptGate` (`crates/flow-graph/src/lib.rs` / `crates/flow-graph/src/deploy_draft.rs` `TransactionBehavior::Immediate` WAL write serialization + `base_file_hashes_json` `409 PROPOSAL_STALE_BASE_HASH` CAS + HMAC-SHA256 `proposal_gate_receipts` atomic apply + `pre_apply_safety` snapshot). Verified deterministically via Rust integration suite (`crates/harness-daemon/tests/story_05_2_proposals.rs`) and Playwright E2E (`tests/e2e/story-05.2.spec.ts`); dedicated TLA+/TLC model deferred beyond the `01.2`/`04.2` formal pilot scope.

## 4. Audit-360 Post-Hardening Delta Review (Reviewer `ff0ec11e-a43c-4101-93bc-20394cb68b20`)
- **Round 4 Verdict:** `VERDICT: APPROVED` — Score: **9.3 / 10**
- **Verified Fixes:**
  - `src/features/proposals/hooks/useNodeProposalController.ts`: extracted `useProposalGateActions`, `useProposalDiffAndTradeoffs`, `runVisualDiffComputation`, `runTradeoffDecision`, and `executeQuickDraftCreation` (`CC <= 15`), added `try/catch` error surfacing, and localized receipt/draft strings (`9`-locale parity).
  - `src/features/flow/hooks/dispatchNodeExecution.ts` & `src/features/flow/components/CompanionCanvasTab.tsx`: localized SSE formatting, mutable `activeStreams` listener refs, localized proposal synthesis warning + daemon coordinator reconciliation without premature local completion/failure, and `flushPending(true)` persistence.
