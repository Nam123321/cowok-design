---
schema: agent-surface-map/v1
epic_id: Epic-10
feature_group: "VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution"
default_execution_lane: LANE_2_OPERATIONAL_NLP
---

# Epic 10 Agent Surface Map (`10.1`, `10.2`, `10.3`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `10.1` | `cowok_3lane_router_and_pty` | `KERNEL` | `AGENT_FACING` | `PLAYBOOK` / `UI_KIT` | `LANE_2_OPERATIONAL_NLP` | `/api/v1/orchestrator/*`, MPSC Write Actor, `$UIUX_LIVE_SPEC` | SQLite WAL | `CONSISTENT` |
| `10.2` | `swarm_leases_and_design_jury` | `KERNEL` | `AGENT_PROPOSE_ONLY` / `HUMAN_ONLY` | `PLAYBOOK` / `UI_KIT` | `LANE_2_OPERATIONAL_NLP` | `.cowok-design/coordination/`, `worker_file_leases`, `Design Jury` | `NEW_SCHEMA` (`V17`) | `CONSISTENT` |
| `10.3` | `cmp07_tournament_and_advisor` | `DOMAIN` | `HUMAN_ONLY` *(borrow/`/crystallize`)* | `UI_KIT` / `PLAYBOOK` | `LANE_2_OPERATIONAL_NLP` | `/api/v1/tournament/*`, `CMP-07 DesignTournamentArena`, `Blind Wipe` | `NEW_SCHEMA` (`V18`) | `CONSISTENT` |
