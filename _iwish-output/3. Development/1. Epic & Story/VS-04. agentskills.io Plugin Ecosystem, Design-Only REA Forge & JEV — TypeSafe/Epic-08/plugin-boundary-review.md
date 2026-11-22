---
schema: plugin-boundary-review/v1
epic_id: Epic-08
feature_group: "VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe"
applicability: REQUIRED_REVIEW
delivery_strategy: HYBRID_KERNEL_AND_EXTENSION_HOST
---

# Plugin Boundary Review — Epic 08 (`crates/plugin-host` & `packages/builtin-skills`)

## 1. Per-Component 3-Axis Decomposition (Trục 1: Trust, Packaging, Delivery)

| Component ID | Story | Trust Ownership | Packaging Category | Delivery Maturity | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :---: | :--- |
| `triangle_linter_and_auto_fix` | `08.1` | `KERNEL` | `KERNEL_CORE` | `BUILT` (in-memory/file) | **Veto G1 + G3 (`< 25ms`)**: Must never be uninstalled/bypassed by plugins or tenants (`crates/plugin-host/src/triangle_linter/`). |
| `per_run_stager_kv_cache` | `08.1` | `KERNEL` | `INTERNAL_MODULE` | `BUILT` (local FS only) | Owns `.cowok-skills/<run_id>/` staging & `stable_prompt_hash` determinism (`crates/plugin-host/src/staging.rs`). |
| `builtin_seed_skills_suite` | `08.1` | `DOMAIN` | `BUILTIN_CELL` | `BUILT` (9 skills on disk) | Platform-managed `agentskills.io` cells in `packages/builtin-skills/skills/` (`SKILL.md <= 100 tokens`, `plugin.json`). |
| `zero_trust_security_scanner` | `08.2` | `KERNEL` | `KERNEL_CORE` | `STAGED` (needs IPI/binary harden) | **Veto G1 + G2**: Enforces symlink rejection, command-injection/eval/path-traversal checks BEFORE any external repackaged plugin is admitted. Must be extended for Indirect Prompt Injection (`SKILL.md`) & disguised binary screening. |
| `github_skill_repackager` | `08.2` | `DOMAIN` | `BUILTIN_CELL` | `STAGED` (local dir only; missing GitHub fetch + SSRF guard) | Converts untrusted external repos/scripts into `SKILL.md` + `plugin.json` + `~1.2KB` `discovery-stub.json`. |
| `external_repackaged_plugins` | `08.2` | `EXTENSION` | `EXTENSION_PLUGIN` | `CANDIDATE` | Third-party GitHub/community bundles produced by `SkillRepackager`. Requires SHA-256 `skills-lock.json` pinning, strict `FS Jail`, and isolated staging promotion. |
| `video_intake_processor` | `08.2` | `DOMAIN` | `BUILTIN_CELL` | `BUILT` (parser module) | Deterministic storyboard timeline extractor producing `VideoStoryboardReceipt` (`frame_id`). |
| `plugin_6state_lifecycle_machine` | `08.3` | `KERNEL` | `KERNEL_CORE` | `STAGED` (in-memory only; needs SQLite WAL + Story 04.3/04.4 state reconcile) | **Veto G1**: Governs `Discovered -> Validated -> Installed -> Active -> Deprecated -> Archived` transitions & breaking-change staging isolation (`EC-E08-03`). |
| `github_cron_sync_scheduler` | `08.3` | `KERNEL` | `INTERNAL_MODULE` | `CANDIDATE` (MISSING `tokio-cron-scheduler` & lease fencing) | Runs in `crates/harness-daemon` with atomic SQLite leases, SSRF egress allowlist, and anti-fork-bomb no-nested-cron invariant. |
| `learn_knowledge_card_pipeline` | `08.3` | `KERNEL` | `KERNEL_CORE` | `STAGED` (in-memory only; needs SQLite + HITL UI + sanitization) | **Veto G1 + G2 (`EC-LEARN-001`)**: Owns Human Safety Valve; `Proposed`/`Rejected` cards MUST never enter runtime prompts. |
| `split_de_7tools_and_ephemeral_tool_jail` | `08.5` | `KERNEL` + `EXTENSION` | `KERNEL_CORE` + `EXTENSION_PLUGIN` | `PLANNED_CONSOLIDATION` (`08.5+13.1+11.2`) | Executes 7 deterministic DE tools (`<=25ms` linear-NFA, `0` LLM cost), `HUMAN_ONLY` quarantined script/tool jail (`env_clear`, `O_NOFOLLOW`), and System-1 `LEARN` filter (`hit_count>=3`, `SLPR>=0.95`, `auto-retire`). |

## 2. Boundary Guardrails for Downstream `/make-story` & 3-Layer Design
1. **KERNEL Isolation Invariant:** External plugins (`EXTENSION_PLUGIN`) and repackaged `SKILL.md` texts are **untrusted data**, never Kernel policy. `triangle_linter`, `security_scanner`, `plugin_state_machine`, and `learn_pipeline` (`EC-LEARN-001`) cannot be disabled or overridden by external plugin manifests.
2. **DB & Sandbox Partitioning:** Only `crates/spec-registry` + `crates/harness-daemon` hold SQLite WAL write locks. External scripts in `scripts/` must execute only inside isolated POSIX/sandboxed boundaries (`fs_jail`, `O_NOFOLLOW`, egress SSRF blocklist) — never directly inside the daemon process.
3. **Host Shell / Portal Constraint:** Downstream 3-Layer FE (`layer-epic-fe.md`) must render third-party plugin manifests and Knowledge Card diffs safely (escaped Markdown, zero raw HTML/`<img src>` external exfil, zero unapproved auto-promotion).
