---
schema: agent-surface-map/v1
epic_id: Epic-09
feature_group: "VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 09 Agent Surface Map (`09.1`, `09.2`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `09.1` | `rea_dna_forge_and_firewall` | `KERNEL` | `AGENT_FACING` | `TOOL` / `UI_KIT` | `LANE_1_DETERMINISTIC` | `crates/rea-bridge`, `rea.search_patterns`, `/api/v1/rea/*` (replaces `ProjectLaunchpadModal` mock JSON) | `Tier-1B` `rea-reference.sqlite3` FTS5 | `CONSISTENT` |
| `09.2` | `rust_decision_cascade_client` | `KERNEL` | `AGENT_FACING` | `TOOL` / `UI_KIT` | `LANE_1_DETERMINISTIC` | `/api/v1/decision/*`, `uiux-os decide`, `DecisionCascadeClient` (`crates/domain`+`crates/bot-fleet`) | `NEW_SCHEMA` (`V17 decision_audit_log`) | `CONSISTENT` |
