---
schema: agent-surface-map/v1
epic_id: Epic-02
feature_group: "VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 02 Agent Surface Map (`02.1`, `02.4`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `02.1` | `context_slicer_and_kv_compiler` | `KERNEL` | `AGENT_FACING` | `TOOL` | `LANE_1_DETERMINISTIC` | `crates/spec-registry::ContextSlicer` & `DeterministicPromptCompiler` | `EXISTING_SCHEMA` (`V1..V15` + telemetry) | `CONSISTENT` |
| `02.4` | `hidden_ssot_appshell_lock` | `DOMAIN` | `AGENT_PROPOSE_ONLY` / `HUMAN_ONLY` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `crates/spec-registry` `V16 component_patterns, app_shell_locks` + `.cowok-design/exports/` | `NEW_SCHEMA` (`V16`) | `CONSISTENT` |
| `02.4` | `cowok_design_opt_3zone_router` | `DOMAIN` | `AGENT_FACING` | `SKILL` / `CAPABILITY_PACK` | `LANE_1_DETERMINISTIC` | `CachePreservingSkillRouter` (`<15ms`, `<=250 tok` spine + 7 Craft Skills) | `NONE` | `CONSISTENT` |
