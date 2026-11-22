---
schema: agent-surface-map/v1
epic_id: Epic-08
feature_group: "VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 08 Agent Surface Map (`agent-capability-packager` v2.0 — Axis 2a)

Aggregates child stories across Epic 08 (`08.1`, `08.2`, `08.3`, `08.4`) under the **Single Core Port, Multiple Surfaces (D2)** invariant (`crates/plugin-host` authoritative runtime; `packages/builtin-skills` static assets + DTOs; `crates/harness-daemon` + Workbench UI / MCP invoke identical domain ports).

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence (`state_change`) | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `08.1` | `builtin_skills_registry` | `DOMAIN` | `AGENT_FACING` | `SKILL` / `CAPABILITY_PACK` | `LANE_1_DETERMINISTIC` | `crates/plugin-host::SkillRegistry` & `PerRunStager` | `EPHEMERAL` (`.cowok-skills/<run_id>/`) | `CONSISTENT` |
| `08.1` | `triangle_deterministic_linter` | `KERNEL` | `AGENT_FACING` | `TOOL` (`lint`, `lint_fix`) | `LANE_1_DETERMINISTIC` (`< 25ms`, `0` tokens) | `crates/plugin-host::TriangleLinter` | `NONE` | `CONSISTENT` |
| `08.2` | `zero_trust_security_scanner` | `KERNEL` | `AGENT_FACING` (`READ_ONLY`) | `TOOL` | `LANE_1_DETERMINISTIC` | `crates/plugin-host::repackager::scan_external_script_security` | `NONE` | `CONSISTENT` *(AC-05..07 hardened)* |
| `08.2` | `github_skill_repackager` | `DOMAIN` | `AGENT_PROPOSE_ONLY` | `CAPABILITY_PACK` | `LANE_1_DETERMINISTIC` | `crates/plugin-host::SkillRepackager` | `EXISTING_SCHEMA` (`skill_plugins` + `skills-lock.json`) | `CONSISTENT` *(AC-06 fail-closed <= 1,536B)* |
| `08.2` | `video_intake_processor` | `DOMAIN` | `AGENT_FACING` | `TOOL` | `LANE_1_DETERMINISTIC` | `crates/plugin-host::VideoIntakeProcessor` | `EPHEMERAL` (`VideoStoryboardReceipt`) | `CONSISTENT` |
| `08.3` | `plugin_6state_lifecycle` | `KERNEL` | `AGENT_PROPOSE_ONLY` | `PLAYBOOK` / `TOOL` | `LANE_1_DETERMINISTIC` | `crates/plugin-host::ManagedPlugin` (`transition_to`, `apply_update`, `promote_staging`, `discard_staging`) | `EXISTING_SCHEMA` (`skill_plugins`) | `CONSISTENT` |
| `08.3` | `learn_propose_card` | `KERNEL` | `AGENT_FACING` *(propose-only write)* | `TOOL` | `LANE_1_DETERMINISTIC` / `LANE_2_OPERATIONAL_NLP` | `crates/plugin-host::LearnPipeline::propose_card` | `NEW_SCHEMA` (`knowledge_cards` `PROPOSED`) | `CONSISTENT` |
| `08.4` | `github_cron_sync_scheduler` | `KERNEL` | `HUMAN_ONLY` *(config)* | `PLAYBOOK` | `LANE_1_DETERMINISTIC` | `crates/harness-daemon` (`tokio-cron-scheduler` + `LeaseReaper`) | `NEW_SCHEMA` (`plugin_cron_schedules`) | `CONSISTENT` *(scoped in 08.4 AC-04)* |
| `08.4` | `learn_accept_reject_gate` | `KERNEL` | `HUMAN_ONLY` | `UI_KIT` (`HumanGateCard`) | `LANE_1_DETERMINISTIC` | `crates/harness-daemon` (`ActorKind::HumanWorkbenchSession` -> `LearnPipeline::accept_card` / `reject_card`) | `NEW_SCHEMA` (`knowledge_cards` `ACCEPTED` / `REJECTED`) | `CONSISTENT` *(scoped in 08.4 AC-03 & AC-05)* |
| `08.4` | `sqlite_v9_plus_convergence_and_ui` | `KERNEL` | `HUMAN_ONLY` *(promotion/diff)* | `UI_KIT` | `LANE_1_DETERMINISTIC` | `crates/spec-registry` (`>= V9`) + `PluginRegistryPanel` / `StagingDiffCard` | `NEW_SCHEMA` (`skill_plugins` + `prisma/schema.prisma`) | `CONSISTENT` *(scoped in 08.4 AC-01..05)* |
| `08.5` | `split_de_7tools_and_jail` | `KERNEL` | `HUMAN_ONLY` *(jail)* / `AGENT_FACING` *(7 DE tools)* | `TOOL` / `UI_KIT` | `LANE_1_DETERMINISTIC` | `/api/v1/de/*`, `/api/v1/learn/*`, `uiux-os de-check`, `HUMAN_ONLY` `env_clear`+`O_NOFOLLOW` Jail | `NEW_SCHEMA` (`V18 gate_failure_ledger`) | `CONSISTENT` *(TDR-007)* |
| `08.5` | `user_repo_skill_extension_pack` | `EXTENSION` | `HUMAN_ONLY` *(approval)* | `EXTENSION_PLUGIN` | `LANE_1_DETERMINISTIC` | Plugin Center Repo Import UI + `V16 user_skill_registry` | `NEW_SCHEMA` (`V16/V18`) | `CONSISTENT` *(TDR-007)* |

## Consistency & Safety Highlights
1. **Zero Unapproved Poisoning (`EC-LEARN-001`):** `learn_accept_reject_gate` is strictly `HUMAN_ONLY`, enforced at `daemon_auth` via unforgeable `ActorKind::HumanWorkbenchSession` (never trusted from client `principal_b64` or `daemon.lock`).
2. **Authoritative D2 Single Core Port:** `crates/plugin-host` is the sole authoritative execution/scanning/lifecycle runtime; `packages/builtin-skills` hosts static assets and DTOs only.
