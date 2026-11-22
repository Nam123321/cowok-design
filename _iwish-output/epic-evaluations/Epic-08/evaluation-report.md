# Epic 08 Evaluation Report (`evaluation-report.md`)

- **Epic ID:** `Epic-08`
- **Epic Title:** `agentskills.io Repackager Engine, 6 Built-in Harness Skills & GitHub Cron Lifecycle`
- **Value Stream:** `VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe`
- **Execution Mode:** **Deep Mode** (`Complexity Score CS = 7.0` — Brownfield Multi-Axis Registry, Zero-Trust Skill Repackaging, `daemon_auth` Human Safety Valve, SQLite WAL Persistence & `tokio-cron-scheduler` Sync)
- **Architecture Coherence Pre-Check:** `PASS` (`152` ADRs checked, `3` stories scanned, `0` conflicts, `0` warnings, `6` aligned)
- **NotebookLM Discovery Evidence:** `PASS` (`validate-nlm-hook-execution.py` verified against `Cowok/Core-PRD-Context`, `Cowok/Core-Architecture`, and `RAG Poisoning and Indirect Prompt Injection`)
- **Evaluation Integrity:** `VERIFIED`
- **Epic Readiness:** `CONDITIONAL` *(Ready for specification/story reconciliation & Story 08.4 authoring; blocked from `IMPLEMENT` until `AUTHOR_ONLY` deliverables and `plan-proven-safe` pass)*
- **Story Creation Envelope:** `max_phase: "AUTHOR_ONLY"`, `allowed_story_ids: ["08.1", "08.2", "08.3", "08.4"]`

---

## 1. Executive Summary & Architectural Verdict

Cross-verification of `2.1. product-brief-or-prd.md` (`FR-19`..`FR-21`), `2.2. database-spec.md`, `2.5. architecture.md`, and brownfield workspace code (`crates/plugin-host`, `packages/builtin-skills`, `crates/spec-registry`, `crates/harness-daemon`, `prisma/schema.prisma`) reveals that Epic 08 has **substantial library foundations in place** alongside **critical security, parity, persistence, cron-sync, and specification gaps**:

1. **Specification & Status Drift:** `Story-08.1`..`08.3` frontmatter claim `status: refactored` with `2.0.0` changelogs saying `"Triển khai hoàn tất..."`, even though all 9 tasks remain unchecked `[ ]`, `FR-19` specifies `6` skills while `Story-08.1` builds `9` seed skills, and `FR-21` **GitHub Cron Sync (`tokio-cron-scheduler`)** plus SQLite WAL / Axum API / Workbench HITL UI wiring were omitted from Story 08.3.
2. **Dual-Engine Security Drift (`TS` vs `Rust`):** Both `packages/builtin-skills` and `crates/plugin-host` implement separate validation, security scanning, repackaging, and lifecycle state machines whose rules have already diverged. Under `D2 (Single Core Port)`, **`crates/plugin-host` MUST be locked as the sole authoritative runtime**, with `packages/builtin-skills` narrowed to static seed assets and typed DTO/adapter contracts.
3. **Newly Discovered High-Severity Security & Reliability Flaws (`[EDGE-CASE]` Gaps):**
   - `crates/harness-daemon/src/daemon_auth.rs` trusts client-supplied `principal_b64` in HMAC v2 and allows any workspace reader of `daemon.lock` (`daemon_nonce`) to mint an `editor` session token — meaning workspace Agents/PTYs could forge `accept_card` (`EC-LEARN-001`). Requires unforgeable daemon-derived `ActorKind::HumanWorkbenchSession` vs `AgentHmac`.
   - `crates/plugin-host` lacks `serde_yaml` frontmatter parsing (`find("---")` delimiter breakout + missing opening-delimiter fallback), lacks `skill_name` traversal validation in `SkillRepackager::repackage_directory`, uses `from_utf8_lossy` on opaque binary assets, omits `prompt_overview` from `compute_stable_prompt_hash`, measures `discovery-stub.json` without enforcing any fail-closed byte limit, and runs `LearnPipeline` / `ManagedPlugin` strictly in volatile RAM.
   - `V1__initial_16_tables.sql` (`skill_plugins`) misses 7 columns specified in `2.2. database-spec.md`, drifts with `prisma/schema.prisma` (`@@unique([projectId, pluginSlug])`), conflicts with Story 04.3/04.4 `'ENABLED'` rows, and cannot hardcode `V9` because concurrent `Story 05.1` already targets `V9`.

**Verdict:** Grant **`max_phase: "AUTHOR_ONLY"`** across `["08.1", "08.2", "08.3", "08.4"]` to reconcile existing specs/stories/FMEA risk matrix and author **`Story 08.4`** cleanly before any implementation or `/plan-proven-safe` human gate begins.

---

## 2. Plugin Decomposition Strategy

- **Plugin Boundary Applicability:** `REQUIRED_REVIEW` (extracted from Step 2.6 [plugin-boundary-review.md](../../3.%20Development/1.%20Epic%20&%20Story/VS-04.%20agentskills.io%20Plugin%20Ecosystem,%20Design-Only%20REA%20Forge%20&%20JEV%20%E2%80%94%20TypeSafe/Epic-08/plugin-boundary-review.md))
- **Agent Surface Blueprint:** [agent-surface-map.md](../../3.%20Development/1.%20Epic%20&%20Story/VS-04.%20agentskills.io%20Plugin%20Ecosystem,%20Design-Only%20REA%20Forge%20&%20JEV%20%E2%80%94%20TypeSafe/Epic-08/agent-surface-map.md)

| Component ID | Trust Owner | Packaging | Agent Exposure (`agent-surface-map.md`) | Boundary & Runtime Mandate |
| :--- | :---: | :---: | :---: | :--- |
| `triangle_linter_and_auto_fix` | `KERNEL` | `KERNEL_CORE` | `AGENT_FACING` (`TOOL`, `LANE_1`) | Non-bypassable AST/Geometric/WCAG + HTML/XSS scanner (`< 25ms`); cannot be disabled by plugin manifests. |
| `zero_trust_security_scanner` | `KERNEL` | `KERNEL_CORE` | `AGENT_FACING` (`READ_ONLY`, `LANE_1`) | Sole authoritative pre-admission scanner (`crates/plugin-host`); enforces shell/eval/exfil/IPI/binary/traversal checks. |
| `plugin_6state_lifecycle_machine` | `KERNEL` | `KERNEL_CORE` | `AGENT_PROPOSE_ONLY` (`PLAYBOOK`, `LANE_1`) | Authoritative state machine (`DISCOVERED..ARCHIVED`), structural breaking-change isolation (`EC-E08-03`), quarantine guard. |
| `learn_knowledge_card_pipeline` | `KERNEL` | `KERNEL_CORE` | `propose`: `AGENT_FACING`; `accept`/`reject`: `HUMAN_ONLY` | Enforces `EC-LEARN-001`; human-only gate enforced by daemon `ActorKind::HumanWorkbenchSession` (never exposed via MCP). |
| `per_run_stager_kv_cache` | `KERNEL` | `INTERNAL_MODULE` | `AGENT_FACING` (`SKILL`, `LANE_1`) | Atomic `.cowok-skills/<run_id>/` snapshot + pinned `learn_snapshot_sha256` & complete `stable_prompt_hash`. |
| `github_cron_sync_scheduler` | `KERNEL` | `INTERNAL_MODULE` | `HUMAN_ONLY` *(config)* | Hosted in `crates/harness-daemon` (`tokio-cron-scheduler`, `LeaseReaper`, single-flight, SSRF/Vault, no nested cron). |
| `builtin_seed_skills_suite` | `DOMAIN` | `BUILTIN_CELL` | `AGENT_FACING` (`CAPABILITY_PACK`) | `packages/builtin-skills` static multi-axis assets (`SKILL.md <= 100` tokens, `plugin.json`) + `ts-rs`/Zod DTOs. |
| `github_skill_repackager` | `DOMAIN` | `BUILTIN_CELL` | `AGENT_PROPOSE_ONLY` (`CAPABILITY_PACK`) | Repackages validated local/GitHub skills into `agentskills.io` layout + fail-closed `<= 1,536`-byte `discovery-stub.json`. |
| `video_intake_processor` | `DOMAIN` | `BUILTIN_CELL` | `AGENT_FACING` (`TOOL`) | Parses `[frame: mm:ss.xx] -> Action: Intent` timelines with strict timestamp/body validation. |
| `external_repackaged_plugins` | `EXTENSION` | `EXTENSION_PLUGIN` | `UNTRUSTED_DATA` | Third-party repackaged bundles; pinned via `skills-lock.json` SHA-256, sandboxed, rendered safely in data fences. |

---

## 3. Actionable Implementation Plan (Spec, AC & Story-Gap Authoring Only — ZERO Code)

> **Strict Scope Rule:** This plan specifies ONLY document reconciliations, AC additions, FMEA risk updates, and the `Story 08.4` authoring scope required during `AUTHOR_ONLY`. No Rust, TypeScript, or SQL code is authored or modified at this stage.

### 3.1 Cross-Cutting Spec & SSOT Reconciliations
1. **Reconcile `FR-19` & Crate Naming (`2.1. product-brief-or-prd.md` & `Epic-08/epic.md`):**
   - Explicitly document the `6` core `FR-19` operating skills plus the `3` supplementary seed skills (`reference-design-contract`, `impeccable-design-polish`, `research-decision-room`) totaling `9` built-in seed skills in `packages/builtin-skills/skills/`.
   - Standardize crate references on `crates/plugin-host` (retaining `crates/skill-repackager` solely as PRD legacy alias) and normalize `file://` absolute desktop links to repository/worktree-relative paths.
2. **Reset Truthful Story Statuses & Changelogs (`Story-08.1..08.3/story.md` & `sprint-status.yaml`):**
   - Resolve contradictory statuses (`refactored` vs `ready-for-dev`/`planned`) and remove premature `2.0.0` `"Triển khai hoàn tất..."` changelog claims while tasks remain `[ ]`.
3. **Lock `D2 (Single Core Port)` Boundary:**
   - Update Stories `08.1`..`08.3` Context sections to designate `crates/plugin-host` as the **sole authoritative runtime** for scanning, YAML/token validation, repackaging, per-run staging, and lifecycle/LEARN rules, narrowing `packages/builtin-skills` to canonical assets + Zod/`ts-rs` DTO contracts + shared golden fixtures.

### 3.2 Story `08.1` AC & Edge-Case Additions (`Multi-Axis Registry, Per-Run Staging & Triangle Linter`)
- **Add `AC-06 ([EDGE-CASE] EC-E08-01 Strictly Delimited YAML Parser & Manifest Parity)`:**
  - Require `serde_yaml` frontmatter parsing strictly bounded by opening `---` and closing `\n---` (removing permissive `else { trimmed }` fallback and quoted `---` early cutoff, rejecting YAML anchors/aliases/custom tags), multilingual/punctuation-calibrated `<= 100`-token + byte ceiling (`<= 1,024` bytes), and exact cross-field parity (`name`, `version`, `description`, `risk_tier`) between `SKILL.md` and `plugin.json`.
- **Add `AC-07 ([EDGE-CASE] EC-E08-08 Canonical KV-Cache Hash & Atomic Read-Only Staging)`:**
  - Include `prompt_overview`, normalized `parameters`, `risk_tier`, `tags`, `SKILL.md` SHA-256, and sorted `capabilities_required` using length-prefixed framing inside `compute_stable_prompt_hash`; sort `SkillRegistry::discover` and `staged_skills` deterministically; reject pre-existing or symlinked `<run_id>` dirs via atomic `create_dir`; strip/block `<IMPORTANT>`, `<SYSTEM>`, HTML comments, invisible/bidi Unicode, and external `![...]`/`<img>` beacons in `PROMPT.md`.
- **Add `AC-08 (Triangle Linter Security Hygiene Rules)`:**
  - Extend `slop_scanner` rules (`< 25ms` budget preserved) to flag `<script`, `dangerouslySetInnerHTML`, `javascript:` URIs, and external `![...](http...)` image beacons in generated outputs.

### 3.3 Story `08.2` AC & Edge-Case Additions (`Repackager, Security Scanner, Stubs & Video Intake`)
- **Add `AC-05 ([EDGE-CASE] EC-E08-02 Normalized Security Scanner & Binary Spoofing Guard)`:**
  - Upgrade `scan_external_script_security` to normalize whitespace/line-continuations across `.py/.js/.mjs/.cjs/.ts/.mts/.cts/.jsx/.tsx/.sh/.bash/.zsh/.rb/.pl/.svg/.html`, detect obfuscated `eval`/`Function`/`base64 -d`/`atob`/`std::process::Command`/`Deno.Command`/`Bun.spawn` and 1–2 level `../`/`..\`/`.ssh`/`/proc/self`, scan `SKILL.md` & `references/` for Indirect Prompt Injection (`<IMPORTANT>`, delimiter escapes, Markdown `<img src>` exfil), and reject non-UTF-8 or executable magic bytes (`ELF`, `Mach-O`, `PE`, `Wasm`) instead of `from_utf8_lossy` fallback into `assets/`.
- **Add `AC-06 ([EDGE-CASE] EC-E08-04 Fail-Closed discovery-stub.json <= 1,536 Bytes)`:**
  - Enforce a **fail-closed `<= 1,536`-byte (`1.5 KiB`)** serialized limit on `discovery-stub.json` plus field-level caps (`name <= 64`, `version <= 32`, `upstream_url <= 256`, `capabilities <= 16 x 48`, `description <= 320` bytes) with zero silent truncation.
- **Add `AC-07 ([EDGE-CASE] EC-E08-05 Rust skill_name Path-Jail, Safe YAML & Resource Quotas)`:**
  - Validate `skill_name` via strict segment allowlist before `dest_dir.join(skill_name)`, serialize `SKILL.md` frontmatter via `serde_yaml` (preventing `format!()` delimiter breakout), reject hardlinks (`nlink > 1`) & symlinks in `dest_dir`, bound file count / depth / unpacked byte quota, validate malformed timestamps cleanly in `VideoIntakeProcessor`, and lock `skills-lock.json` SHA-256 pinning rules.

### 3.4 Story `08.3` AC & Edge-Case Additions (`6-State Lifecycle & LEARN Domain Core`)
- **Add `AC-05 ([EDGE-CASE] EC-E08-03 & EC-E08-10 Complete Transitions, Diff Engine & Quarantine)`:**
  - Add `Validated -> Archived` and `Installed -> Archived` transitions, `promote_staging` / `discard_staging` APIs, a `Quarantined`/`Revoked` guard blocking re-activation after security failure, and deterministic structural manifest/SemVer breaking-change diffing (replacing untrusted caller boolean).
- **Add `AC-06 ([EDGE-CASE] EC-LEARN-001 Authenticated Actor Gate & Prompt-Injection Defense)`:**
  - Enforce explicit `ApproverContext` (`HUMAN_ONLY`) + rationale metadata on `accept_card`/`reject_card`, forbid duplicate card IDs and direct `Rejected -> Accepted` promotion, sanitize `source_evidence`/`content` on `propose_card`, and enforce whole-card priority selection within the `<= 1,500`-token `FR-14` budget on `get_active_prompt_injections` (paired with strict data fences).

### 3.5 Author New `Story 08.4`: `SQLite Additive Persistence, Domain ts-rs Sync, Daemon HUMAN_ONLY Auth, GitHub Cron Sync & Workbench UI`
To prevent exceeding the `Plan-Tune Heuristic` (`CS >= 7`) on Story 08.3 and cleanly isolate cross-cutting infrastructure, create **`Story-08.4/story.md`** covering:
1. **Symbolic Additive SQLite Convergence & `ADR-091` Prisma Parity:** Migrate `skill_plugins` at **`next additive schema_migrations.version (>= 9)`** (avoiding hard `V9` collision with concurrent `Story 05.1`) adding `tenant_id`, `source_type`, `upstream_repo_url`, `upstream_commit_sha`, `cron_schedule`, `skill_md_path`, `plugin_json_path`, `last_synced_at`, `staging_version`, `breaking_change_isolated`, uppercase `lifecycle_state CHECK`, partial unique `ACTIVE` index per `(tenant_id, project_id, plugin_slug)`, and `'ENABLED' -> 'ACTIVE'` backfill; add `knowledge_cards` (`project_id`/`tenant_id` scoped) and `plugin_cron_schedules` (`lease_owner`, `fencing_token`, `stale_after_timestamp`, `consecutive_failures`, `next_retry_at`, `sync_status`); update `prisma/schema.prisma` to close existing `SkillPlugin` `@@unique` drift and match all new tables.
2. **`crates/domain` `ts-rs` Contract Expansion:** Expand `SkillPlugin` (compatible with `#[serde(deny_unknown_fields)]`) and export typed `PluginState`, `KnowledgeCard`, `DiscoveryStub`, `CronSchedule` DTOs to `packages/shared-types`.
3. **Unforgeable `daemon_auth` `ActorKind` + Axum/BFF Routes (`FR-29` `HUMAN_ONLY` Enforcement):** Derive unforgeable `ActorKind` (`HumanWorkbenchSession | AgentHmac | McpClient | SuperadminCli`) server-side in `daemon_auth.rs` (never trusting `principal_b64` or allowing `daemon.lock` readers to self-mint `HUMAN_ONLY` tokens); expose `discover`, `stage`, `lint/triangle`, `repackage`, `discovery-stub`, `video-intake`, `transition`, `staged/promote|discard`, `learn/cards` + `accept|reject`, and `schedule`/`sync-now` routes in `harness-daemon` + `vite-bff-routes.ts`; wire `compile_prompt` to immutable run snapshots (`learn_snapshot_sha256` inside `<SKILL_CATALOG_DATA>` / `<LEARNED_CARDS_DATA>` data fences).
4. **`tokio-cron-scheduler` GitHub Sync (`FR-21`):** Implement jittered cron sync in `harness-daemon` using **claim-release-work-commit** transaction splitting, startup `LeaseReaper`, per-plugin single-flight locks, no-nested-cron invariant, SSRF validation (`validate_ssrf_safe_url` blocking RFC1918 / `127.0.0.0/8` / `169.254.169.254` + pinned git hardening flags), `0o600` Vault PAT/OAuth lookup, `ETag`/`ls-remote` SHA check, exponential backoff on `429`/`5xx`, and `AUTH_REQUIRED` fail-closed pause on `401`/`403` — never mutating active `.cowok-skills/<run_id>/`.
5. **Workbench UI Surfaces & Event Audit:** Render 6-state Plugin Lifecycle + Breaking-Change Staging diff resolution (`Promote`/`Discard`), Cron Sync status/re-auth prompt, and LEARN `Proposed` Knowledge Card Review Queue (`Accept`/`Reject`) with sanitized Markdown rendering and `execution_events` / `gate_reports` receipts.

### 3.6 Expand `_iwish-output/edge-case-knowledge/epics/Epic-08-risk-matrix.md`
During `AUTHOR_ONLY`, add missing FMEA rows `EC-LEARN-001` and `EC-E08-04`..`EC-E08-10` alongside updated `EC-E08-01..03`.

---

## 4. Edge-Case Guardian Audit Matrix (Step 6.5 Candidate Scope for Guardian Seal)

| FMEA ID | Failure Mode / Boundary Condition | Planned Mitigation Scope | Target Story | Status |
| :--- | :--- | :--- | :---: | :---: |
| **`EC-E08-01`** | `SKILL.md` `<= 100`-token breach, YAML delimiter breakout (`find("---")`), missing opening `---`, & BPE underestimate | Strict `serde_yaml` frontmatter parser + byte/multilingual token cap + `plugin.json` cross-parity | `08.1` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-02`** | `security_scanner` substring bypass & TS vs Rust rule drift | Single authoritative `crates/plugin-host` scanner; normalized multi-line/obfuscation/HTML/XSS checks + shared golden fixtures | `08.2` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-03`** | Breaking change overwrites active plugin or relies on caller boolean | Structural manifest/SemVer diffing + `Staging` isolation + `promote_staging`/`discard_staging` | `08.3` | `PLANNED_IN_ENVELOPE` |
| **`EC-LEARN-001`** | `daemon_auth` HMAC/`daemon.lock` spoofing lets agent call `accept_card`; unapproved/poisoned/cross-tenant cards in prompts | Server-derived `ActorKind::HumanWorkbenchSession`, `HUMAN_ONLY` route guard, card sanitization, `project_id`/`tenant_id` scope, `Accepted`-only filter | `08.3` / `08.4` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-04`** | `discovery-stub.json` exceeds `~1.2KB` or catalog exceeds `<= 1,500`-token `FR-14` cap | Fail-closed `<= 1,536`-byte stub cap + field caps + priority whole-card `FR-14` budget cap | `08.2` / `08.4` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-05`** | Rust `skill_name` path traversal, hardlinks, & unbounded zip/dir recursion | `validate_safe_segment` path-jail, `nlink == 1`, symlink/TOCTOU checks, depth/file/byte quotas | `08.2` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-06`** | `from_utf8_lossy` binary asset smuggling (`.wasm`/`.so`/`.exe`/`.svg`) | Reject non-UTF-8 in text/scripts + block executable magic bytes before writing `assets/` | `08.2` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-07`** | `SKILL.md` Indirect Prompt Injection, raw `format!()` YAML breakout, & `<img src>` exfil | `serde_yaml` serialization + IPI/bidi-Unicode/image-beacon rejection + `<DATA>` prompt delimiters | `08.1` / `08.2` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-08`** | `prompt_overview` / params omitted from `compute_stable_prompt_hash` + unsorted discovery | Length-prefixed hash over all prompt fields + deterministic sorting + atomic `create_dir` `<run_id>` | `08.1` | `PLANNED_IN_ENVELOPE` |

---

## 5. Post-PR #14 Addendum: Unlocking `Story 08.5` (`TDR-007`, `2026-11-20`)
- Sau khi `08.1..08.4` đạt `completed`, mở khóa **`Story 08.5`** (`delivery: hybrid`, hợp nhất `08.5 + 13.1-split + 11.2` Tool Jail) dưới envelope `IMPLEMENT`: đóng gói **7 Built-in DE Tools** (`<=25ms` linear-NFA, `0` RCE), Ephemeral Tool Audit Drawer (`HUMAN_ONLY`, `env_clear`, `O_NOFOLLOW`), và `V18 gate_failure_ledger` + System-1 `LEARN` (`hit_count>=3`, `SLPR>=0.95`, `auto-retire`).
| **`EC-E08-09`** | GitHub Cron SSRF, nested cron fork-bomb, rate-limit loop, & mid-run KV cache bust | SSRF IP blocklist + pinned git flags, no-nested-cron, `ETag`/backoff/`AUTH_REQUIRED`, immutable per-run snapshots | `08.4` | `PLANNED_IN_ENVELOPE` |
| **`EC-E08-10`** | RAM-only loss, `'ENABLED'` legacy drift, Story 05.1 `V9` migration collision, & Prisma drift | Symbolic `next additive version (>= 9)` SQLite WAL + `LeaseReaper` + `ADR-091` `prisma/schema.prisma` parity check | `08.4` | `PLANNED_IN_ENVELOPE` |
