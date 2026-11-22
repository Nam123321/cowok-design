# Stage 4 Plan-Blind Tournament Code Review — Story 08.4 (`plugins-ecosystem-learn-pipeline`)

- **Story ID:** `08.4` (`plugins-ecosystem-learn-pipeline`)
- **Review Engine:** `tournament` (Track 1: Antigravity OCR SAST + Unified Intent Verifier | Track 3: Codex CLI `gpt-5.6-terra`)
- **Final Verdict:** `APPROVED`
- **Review Rounds:** Round 1 -> Round 2 (Wave 2 remediation) -> Round 3 / Epoch 3 (`epoch-3-6f8aadb9`)
- **Final Score:** Antigravity Unified Intent `9.8 / 10` (`PASS`) | Codex `gpt-5.6-terra` Round 3 `9.0 / 10` (`APPROVED`, `0` in-scope CRITICAL/HIGH findings)

---

## 1. Cross-Model Tournament Consensus Summary

| Track / Engine | Epoch / Round | Verdict | Critical/High | Scope / Evidence |
| :--- | :--- | :--- | :---: | :--- |
| **Antigravity OCR SAST & Unified Verifier** (`6f9a367d`) | Epoch 3 (`epoch-3-6f8aadb9`) | `PASS` / `APPROVED` | `0` | 138 reviewable files, `ocr-findings.json` (`review_epoch_id: epoch-3-6f8aadb9`), `review-unified-intent-08.4.json` |
| **Codex CLI Cross-Check** (`gpt-5.6-terra`) | Round 3 (`73790-c640dfda`) | `APPROVED` (`[GP-17.R]`) | `0` in-scope | `review-codex-story-08.4-round3.md` (`F-02` partial-index parity verified & dismissed; `F-01` deferred) |

---

## 2. Remediations Verified Across Waves 1–3

1. **Repackager Metadata & Cron Staging Alignment (`crates/plugin-host/src/repackager/repackager_engine.rs`, `crates/harness-daemon/src/routes/plugins_cron.rs`):**
   - Preserved `version`, `risk_tier`, and `capabilities_required` through `SkillRepackager::repackage_directory_with_manifest`.
   - Fixed `apply_synced_bundle_to_plugin` to normalize scripts cleanly and emit verified staging artifacts (`plugin.json`, `SKILL.md`, `skills-lock.json`) consumed identically by `promote_verified_staged_bundle_from_dir`.
   - Replaced process-global mutex nesting guard with task-local `tokio::task_local!` `IN_CRON_WORKER_TICK` and enforced per-hop SSRF DNS socket pinning (`.resolve(&host, pinned_addr)`).
2. **Prisma ↔ SQLite V9 Index & Partial Unique Parity (`prisma/schema.prisma`, `crates/spec-registry/migrations/V9__epic08_plugins_learn_cron.sql`, `prisma/schema.contract.test.ts`):**
   - Enabled `previewFeatures = ["partialIndexes"]` and declared `uq_skill_plugins_active_slug` (`where: { lifecycleState: "ACTIVE" }`), `uq_knowledge_cards_idempotency` (`where: { idempotencyKey: { not: null } }`), and `uq_plugin_cron_schedules_scope_plugin`.
   - Removed legacy cross-tenant index `skill_plugins_project_slug_ver_key` from V9 migration and verified `sqlite_master` ↔ Prisma 1:1 parity in `UT-DB-06`.
3. **LEARN Single-Receipt Atomicity, XML Escaping & Authenticated SSE Streaming (`crates/harness-daemon/src/routes/plugins.rs`, `src/api/harness-daemon-transport.ts`, `src/api/plugin-center.ts`, `src/features/plugins/hooks/usePluginRegistry.ts`):**
   - Consolidated `record_gate_report_tx` inside `commit_learn_card_decision` (zero duplicate audit receipts).
   - Removed stale `"FLOW"` LEARN category and `"CLI_OPERATOR"` actor aliases; aligned `reason` Zod bounds (`4..500`); added `xml_escape_attr` for compiled `<SKILL>` attributes.
   - Replaced unauthenticated browser `EventSource` with authenticated `fetch` streaming (`HarnessDaemonTransport.openEventStream` -> `PluginEcosystemClient.subscribePluginEvents`) sending `X-Cowok-Session-Token` + `Accept: text/event-stream` (`ut_084_ui_sse_06`).
4. **HUMAN_ONLY Workbench Gate & Empty Rationale Enforcement (`src/features/plugins/*.tsx`, `src/i18n/locales/*.json`):**
   - Enforced empty initial rationale inputs (`>= 4` chars required before Promote/Discard/Approve/Reject enable), added `[data-ff-id]` selectors, `<= 1,500` token meter, filter tabs, `QUARANTINED` badge, `CronScheduleEditor`, and full 9-locale parity.

---

## 3. Formal-verify candidates & Concurrency Primitives Assessment

- **Formal-verify candidates:**
  - **Cron Lease Fencing, `AUTH_REQUIRED` Recovery & `HUMAN_ONLY` Promotion Statechart:** Modeled and exhaustively verified via TLC TLA+ statechart [`PluginCronLease.tla`](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish/verification/formal-specs/plugin-cron-lease-08.4/PluginCronLease.tla) (`PluginCronLease.cfg`, `PluginCronLease.map.mjs`, manifest `qa/formal-verification/verification-manifest.json`).
  - **TLC Result:** `3,627` states generated (`24` worker/lease transitions exercised via `run-statechart.sh from-spec ... --cover`), `0` invariant violations across `NoSplitBrainLease`, `TerminalAuthRequiresRefresh`, and `HumanOnlyPromotion`.
  - **In-Scope Daemon & Constraint Files:** `crates/harness-daemon/src/routes/plugins_cron.rs` (task-local tick guard + lease fencing + SSRF redirect pinning) and `crates/harness-daemon/src/routes/plugins.rs` (LEARN CAS + human actor verification) verified free of split-brain locks or non-atomic state transitions. Any untouched files outside Story `08.4` diff (`crates/flow-graph/src/deploy_draft.rs`, `crates/flow-graph/src/lib.rs`, `crates/constraint-engine/src/icon_validator.rs`) remain read-only/unchanged in Story `08.4`.

---

## 4. Verification Status at Stage 4 Seal

- `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`: PASS
- `npx prisma validate` + `pnpm exec tsc --noEmit`: PASS
- `pnpm vitest run` (including `UT-DB-06`, `ut_084_ui_sse_06`, 9-locale parity): PASS
- `clean_code_guardian` (`Unratcheted Errors: 0`), `deep-audit` (`0` mismatches), `validate-zod-contract-coverage.py --parity` (`0` gaps), `validate-vertical-slice-anti-cheat.py` (`PROVEN_CLEAN`): PASS
