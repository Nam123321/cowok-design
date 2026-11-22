---
schema: agent-surface-map/v1
epic_id: Epic-11
feature_group: "VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 11 Agent Surface Map (`11.1`, `11.2`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `11.1` | `byos_byok_runner_and_normalizer` | `HYBRID` | `AGENT_FACING` | `TOOL` / `EXTENSION_PLUGIN` | `LANE_1_DETERMINISTIC` | `/api/v1/execution/*`, `crates/bot-fleet` (`0o600` FD + `-PGID` + Normalizer, replaces stubs) | `.cowok-design/workspace` | `CONSISTENT` |
| `11.2` | `open_mcp_gateway_and_installer` | `HYBRID` | `READ_ONLY..HUMAN_ONLY` | `MCP_GATEWAY` / `UI_KIT` | `LANE_1_DETERMINISTIC` | `crates/mcp-gateway` (`stdio`+SSE) + Settings 1-Click Installer UI | Host MCP configs | `CONSISTENT` |
