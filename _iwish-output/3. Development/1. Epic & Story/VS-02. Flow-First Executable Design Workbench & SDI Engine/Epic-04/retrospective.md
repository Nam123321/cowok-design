---
type: I-Wish Retrospective
epic_id: "Epic-04"
status: completed
date: "2026-11-21"
assessment_loop: "flow-auto-epic-v3.5"
---

# Epic 04 Retrospective — Flow-First 2D SVG Bezier DAG Studio, Security Kernel, CAS Intake & In-Chat Vault

## 1. Outcome Summary (`6/6` Stories `completed` in `sprint-status.yaml`)
- **Story 04.1:** Unified DualMainPanel Workbench, Multi-Format CompanionCanvasTab, True 2D SVG Bezier DAG Studio & HMAC/Preview Remediation (`completed`).
- **Story 04.2:** SQLite V7 Schema Convergence, Streaming Dual-Ingress Vite BFF & Managed Root / FS Jail (`completed`).
- **Story 04.3:** Day-0 Onboarding Wizard, Passive/Active 16+ CLI Doctor (Extension), OS Keychain/0600 Vault & SSRF-Guarded BYOK Verifier (`completed`).
- **Story 04.4:** Persistent KeyedParallelCoordinator (Monotonic Fencing), SSE Last-Event-ID, -PGID CLI Reaper & 3-Level Sidebar (`completed`).
- **Story 04.5:** State-Hydrated Live Preview (`CompanionLiveHtmlRightPane`), Option CSP-B DOMParser Bridge & Persistent Element Comments (`409 STALE_MAPPING`) (`completed`).
- **Story 04.6 (`risk_tier: 3`, `ADR-006`):** SQLite `V4/V16` (`V16__story_04_6_cas_blobs.sql`, `CURRENT_SCHEMA_VERSION = 16`, `55` tables) chat persistence with auto-provisioned session FK (`ensure_session_for_tenant`) replacing `sessionStorage`, `25.25 MiB` BFF+daemon streaming CAS binary/CSV/TSV/XLSX/Fig intake (`O_NOFOLLOW` atomic write, rollback cleanup, zip-bomb & CSV formula guards), `<svg viewBox>` auto-inference & mXSS sanitizer + `DESIGN_CDN_CSP`, and In-Chat Zero-Leak Key Vault (`DecisionEngineOnboardingCard`, `VaultReceiptCard`, 3-Layer multi-secret interceptor, `redact_secrets` SSE & persistence guard, `2...2` masking for short keys) (`completed`).

---

## 2. Independent Verification & Gate Audit (`2026-11-21`)
- **Story 04.6 Stage 4 Round 3 Consensus (`review-consensus-04.6.json`):** `APPROVED` (`final_score: 9.0`) across Antigravity OCR (`395aef83-e435-46d3-af74-38ba5bee37cd`, score `9.4`, `0` blockers, `57` files `epoch-2-bc285b71`) and Codex (`gpt-5.6-terra`, score `9.0`, `0` critical/high).
- **Story 04.6 Unified Gate (`audit-360 --story 04.6 --with-ux-security`):** `OVERALL VERDICT: APPROVED` (`P1..P9, P11..P13 PASS`, `P10` WARN formal-verify advisory).
- **Story 04.6 Clean Code & Token Gates:** `audit_clean_code.py --story-id 04.6 --enforce` `APPROVED` (`27` checked, `0` unratcheted errors); `verify-design-tokens.py` `PASS` (`377` files, `182` tokens, `9` locales).
- **Story 04.6 Stage 5a Live Vite QA (`b51f6250-22fb-4e10-831f-c244694c0ad3`):** `acceptance-story-04.6.spec.ts` + `story-04.6.spec.ts` `PASS` on live Vite (`3115`) + V16 `harness-daemon`; `verify-live-evidence.py` `PASS` (`2` distinct visual states, `story-04.6-network.har` `944` requests, `0` 5xx); `verify-qa-provenance` `PASSED_CLEAN`; `pipeline-evidence-manual-test.json` `all_passed: true`; `22` files sealed in `qa-acceptance-evidence.json` & `qa/evidence/manual-test-bundle.json`. (Prior 3-failure `manual-test` circuit breaker was reset cleanly via `reset-circuit-breaker-skill` after fixing missing HAR, identical screenshots, and transcript regex collision.)
- **Story 04.6 `/self-evolve-gate` & Closeout:** `validate-evolution-gate.py --story 04.6` `PASS` (`ASSESSED_RESOLVED`, `6/6` anomalies resolved); Codex `evolution-judge` `EVOLUTION_APPROVED`; `iwish-gate closeout --story 04.6` `PASS` (`CL-02: PASS`, `CL-04: PASS`, `CL-05: PASS`); `seal-gate stage_4b_5b_completion` `SEALED`. (`--phase delivery` noted baseline ADR-091 `validate-prisma-drift.py` waiver matching Story `08.4` parity, while direct `audit_clean_code`, `validate-epic-sync`, `plugin-dod-gate` all exited `0`.)
- **Epic 04 Retrospective Unknowns Scan (`a6bab5b5-f5ae-40aa-a85f-0796704d3452`):** Zero blocking errors (`U-046-01..07`, `U-046-17..22` `RESOLVED`). Historical Epic-04 script baselines documented honestly: `evaluate-epic-drift.py --epic 04` flags negation prose in pre-completed `Story-04.3/story.md` ("treo FD Electron"); `audit_clean_code.py --epic 04` flags pre-existing findings in pre-completed `04.1..04.5` (`src/components/comments/*`); `ecc-gate.py --epic 04` reports missing legacy `04.1` evidence file.

---

## 3. Non-Blocking Technical Debt & Lessons Carried Forward
1. **Non-Blocking Advisories (Story 04.6):** Wrap `VaultReceiptCard` / `useAgentChatConversation` async replace/revoke mutations with explicit `try/catch` error propagation (`OCR-046-R3-001`), align unversioned `/api/cas/upload` BFF ceiling (`OCR-046-R3-002`), replace literal `5000` in `WorkbenchShell.tsx:105` (`R-SIMPLIFY-04`), keep raw blob pre-hash before CSV/TSV formula neutralization (`U-046-07`), and wire live `/api/v1/doctor/verify-key` verification in `chat-ingest`.
2. **Stage 5a QA Harness Discipline (`LSN-04.6-STAGE5A-QA`):** For `generic-polyglot` Vite SPAs, clear stale worktree daemon SQLite locks (`xargs kill -9` against exact worktree binary path), start live `npx vite` on the worktree-assigned port, and keep Playwright HAR/DOM/log I/O out of shell `printf`/`tee` chains to preserve clean QA provenance.
3. **SSOT Header Normalization (`Story-04.6/story.md` & `2.2. database-spec.md`):** Maintaining explicit `contract_manifest` (`schema_version: 2`, `status: IMPLEMENTED`), 8-column traceability tables, and `AC-XX` source annotations up front prevents late `audit-360` P5/P13 drift.
