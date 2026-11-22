# Party Mode Transcript — Epic 08 (`cowok-design` / `uxui-os`)

- **Epic:** `Epic-08` — `agentskills.io Repackager Engine, 6 Built-in Harness Skills & GitHub Cron Lifecycle`
- **Value Stream:** `VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe`
- **Complexity Score (CS):** `7.0` (`Deep Mode` -> 2+ adversarial rounds required)
- **Roles Invoked:**
  - **Architect (`architect-agent-persona`):** Subagent `94fed9fa-a264-43e6-a8aa-b93d59785e47` (`inherit`)
  - **AI & Security (`ai-engineer-agent`):** Subagent `f0b9fa6a-a114-4b2b-ab12-c60d3fc0d756` (`inherit`)
  - **PM / Edge-Case & Unknowns Critic (`review-agent-persona`):** Subagent `45005ecd-d345-4698-922c-89de39085976` (`inherit`)

---

## Round 1 — Independent Adversarial Discovery & Pushback

### 1. Architect Pushback (`94fed9fa`)
- **SQLite `skill_plugins` Schema & Drift:** `crates/spec-registry/migrations/V1__initial_16_tables.sql` (L181-193) defines only 8 columns (`id`, `project_id`, `plugin_slug`, `version`, `lifecycle_state`, `source_repo_url`, `manifest_json`, `updated_at`) without `tenant_id`, without `CHECK` constraint on `lifecycle_state`, and missing `2.2. database-spec.md` fields (`source_type`, `upstream_repo_url`, `upstream_commit_sha`, `cron_schedule`, `skill_md_path`, `plugin_json_path`, `last_synced_at`). Furthermore, `prisma/schema.prisma` (L738-750) already drifts (`@@unique([projectId, pluginSlug])` vs SQL `UNIQUE(project_id, plugin_slug, version)`).
- **3-Way `lifecycle_state` Mismatch:** Brownfield rows from Story 04.3/04.4 (`doctor_vault.rs` L1439, extensions) write `'ENABLED'`, `2.2. database-spec.md` writes lowercase (`discovered..archived`), and `PluginState` (`plugin_state_machine.rs`) serializes PascalCase (`Discovered..Archived`). Must unify on uppercase `CHECK(lifecycle_state IN ('DISCOVERED', 'VALIDATED', 'INSTALLED', 'ACTIVE', 'DEPRECATED', 'ARCHIVED'))` and backfill `'ENABLED' -> 'ACTIVE'`.
- **In-Memory Volatile State & Domain Contract Starvation:** `SkillRegistry`, `LearnPipeline` (`Vec<KnowledgeCard>`), and `ManagedPlugin` live strictly in memory with zero SQLite WAL persistence (`spec_registry` is unused inside `plugin-host`). `crates/domain/src/models.rs` `SkillPlugin` uses `#[serde(deny_unknown_fields)]` with only 5 fields.
- **Missing `tokio-cron-scheduler` & Daemon/BFF/UI Routes:** Neither `Cargo.toml` nor `crates/harness-daemon` depends on `tokio-cron-scheduler`. `SqlitePool::connection()` opens per-call connections (`busy_timeout = 5000ms`), so concurrent cron syncs require jittered triggers + **claim-release-work-commit** transaction splitting, `execution_sweeper.rs`-style `LeaseReaper` + `fencing_token`, per-plugin single-flight locks, no-nested-cron invariant, SSRF validation (`validate_ssrf_safe_url`), Vault token lookup, and `401/403` `AUTH_REQUIRED` fail-closed pause. `routes/mod.rs` / `flow_routes.rs` currently expose only read-only `GET /api/v1/plugins` & `/registry`.

### 2. AI & Security Pushback (`f0b9fa6a`)
- **YAML Delimiter Breakout & Unchecked Parser (`validator.rs`, `repackager_engine.rs`):** `SkillRepackager::repackage_directory` builds `SKILL.md` via raw `format!()` interpolation (`\n---\n<IMPORTANT>...` breaks frontmatter); `validate_skill_frontmatter` uses `after_first.find("---")` (matches `---` inside quoted fields, bypassing trailing token count) and silently falls back to `else { trimmed }` when missing leading `---`.
- **Indirect Prompt Injection & Image Exfil (`staging.rs`, `learn_pipeline.rs`, `slop_scanner.rs`):** `PerRunStager` writes raw `prompt_overview` into `PROMPT.md`; no scanner checks `<IMPORTANT>`, `<SYSTEM>`, HTML comments, invisible/bidi Unicode (`U+200B..U+200F`, `U+202A..U+202E`), or Markdown/HTML `<img src>` exfiltration beacons. `LearnPipeline::accept_card(&mut self, id)` has zero caller-identity/human gate and permits `Rejected -> Accepted`.
- **`security_scanner.rs` & Repackager Bypasses:** Lowercase substring checks miss extra whitespace (`curl  |  bash`), `wget -qO- | sh`, `base64 -d`, `atob`, dynamic calls, `exec(` outside 3 extensions, 1-2 level `../`, `std::process::Command`, `Deno.Command`, `Bun.spawn`. `repackager_engine.rs` falls back to `from_utf8_lossy` on opaque files into `assets/`, lacks `validate_safe_segment` on `dest_dir.join(skill_name)`, has symlink TOCTOU + no hardlink/file-count/byte limits, and has zero SSRF / git DoS hardening (`--depth=1 --no-recurse-submodules`, `protocol.file.allow=never`, `core.symlinks=false`).
- **Token Estimation & KV-Cache Drift (`validator.rs`, `staging.rs`):** `chars/4` underestimates non-Latin/Vietnamese/JSON tokens; `compute_stable_prompt_hash` omits `prompt_overview`, `parameters`, `risk_tier`, `tags`, `SKILL.md` hash and skips sorting `capabilities_required`.

### 3. PM / Edge-Case & Unknowns Pushback (`45005ecd`)
- **Specification Contradictions:** PRD (`FR-19`) & Risk Matrix specify `6` built-in skills; `Story-08.1` adds `3` more (`9` seed skills). `epic.md` says `status: planned` and `crates/skill-repackager`, while `Story-08.1..08.3` frontmatter say `status: refactored`, bodies say `ready-for-dev`/`planned`, changelogs claim `2.0.0` "completed", and all 9 tasks remain unchecked `[ ]`.
- **`FR-21` GitHub Cron Sync Omitted & Stub Cap Unenforced:** `Story-08.3` silently dropped `tokio-cron-scheduler` GitHub sync from ACs/Tasks. Both `repackager.ts` and `repackager_engine.rs` measure `stub_size_bytes` without enforcing any upper bound (`~1.2KB`), and TS vs Rust repackager/scanner rules already diverged.

---

## Round 2 — Cross-Critique & Converged Architectural Resolutions

1. **SQLite Migration Version Coordination (Epic 05 Concurrent Work):**
   - Peer work on `feature/story-05.1` concurrently introduces V9 SQLite migration while `CURRENT_SCHEMA_VERSION = 8`. Epic 08 **MUST NOT hardcode `V9`** in story specs; specify **`next additive schema_migrations.version (>= 9)`** (`V{N}__epic08_plugins_learn_cron.sql`) with idempotent schema-readiness guards AND resolve the pre-existing `prisma/schema.prisma` `SkillPlugin` `@@unique` drift under `ADR-091` in the same story.
2. **D2 `Single Core Port` Authority (`crates/plugin-host` SSOT):**
   - Security + Architect unanimously rejected maintaining dual independent security scanners/validators in TypeScript and Rust (parser-differential bypasses already observed).
   - **Decision:** **`crates/plugin-host` is the SOLE authoritative runtime** for validation, security scanning, repackaging, per-run staging, 6-state lifecycle, and `LEARN`. `packages/builtin-skills` is narrowed strictly to static seed assets, `ts-rs`/Zod DTO schemas, and thin IPC/BFF adapters + shared golden-fixture regression tests.
3. **Unforgeable `HUMAN_ONLY` Route Enforcement (`daemon_auth.rs` Finding):**
   - Architect inspected `crates/harness-daemon/src/daemon_auth.rs` and discovered `verify_hmac_v2` currently trusts client-supplied `principal_b64` (`role: "admin"`) and `handle_issue_local_session` allows any local reader of `daemon.lock` (`daemon_nonce`) to mint an `editor` session token!
   - **Decision:** Introduce daemon-derived `ActorKind` (`HumanWorkbenchSession | AgentHmac | McpClient | SuperadminCli`) never parsed from `principal_b64`; decouple human session bootstrap from workspace-readable `daemon.lock`; and gate `accept_card`, `reject_card`, `staged/promote`, `ACTIVE` promotions, and `cron schedule` behind `require_human_operator` (`403 HUMAN_APPROVAL_REQUIRED` for agents/MCP; completely omitted from `crates/mcp-gateway`).
4. **`discovery-stub.json` Fail-Closed Policy:**
   - Security vetoed silent truncation (risks dropping capability disclosures or preserving front-loaded injections). **Decision:** **FAIL-CLOSED `<= 1,536` bytes (`1.5 KiB`)** with strict field-level caps (`name <= 64`, `version <= 32`, `upstream_url <= 256`, `capabilities <= 16 x 48`, `description <= 320` bytes targeting `~1.2 KiB`).
5. **Immutable Per-Run Prompt Snapshot & Data Fencing (`compile_prompt`):**
   - `HarnessDaemon::compile_prompt` MUST pin a run-scoped immutable snapshot (`.cowok-skills/<run_id>/` created via atomic `create_dir` + read-only staged files + `learn_snapshot_sha256` from `Accepted` cards `<= 1,500` tokens selected by whole-card priority). All skill & card texts MUST sit inside strict data fences (`<SKILL_CATALOG_DATA>`, `<LEARNED_CARDS_DATA>`), isolated from Cron Sync background mutations.
6. **Story 08.4 Split & Envelope Verdict (`max_phase = "AUTHOR_ONLY"`):**
   - Unanimous convergence across all 3 critics: Keep Stories `08.1`..`08.3` focused on domain/library hardening + truthful status/spec reconciliation, **split out `Story 08.4`** for SQLite `V{N}` persistence + `prisma/schema.prisma` parity + `domain::SkillPlugin` `ts-rs` + `daemon_auth` `ActorKind` + Axum/BFF routes + `tokio-cron-scheduler` (`LeaseReaper` + SSRF/Vault + backoff) + Workbench HITL UI, and set `story_creation_envelope.max_phase = "AUTHOR_ONLY"` until specs & FMEA risk matrix (`EC-E08-01..10`, `EC-LEARN-001`) are reconciled cleanly.

---

## Round 3 — Post-PR #14 Delta Evaluation (`Story 08.5` Split DE+DR, 7 DE Tools & Controlled Tool Jail, `2026-11-20`)
- **Panel Consensus (`PM`, `Architect`, `Review`, `Dev`, `UX`):** Giữ bất biến `08.1..08.4` (`completed`), mở khóa **`Story 08.5`** (`IMPLEMENT`) theo `TDR-007` hợp nhất Split DE+DR Plugin Runtime, **7 Built-in DE Tools** (`<=25ms` linear-NFA, `0` tokens), Controlled Ephemeral Tool Jail (`HUMAN_ONLY`, `env_clear`, `O_NOFOLLOW`, hấp thụ từ `11.2` cũ), và System-1 `LEARN` filter (`hit_count>=3`, `SLPR>=0.95`, `auto-retire`).
