---
schema: agent-surface-map/v1
epic_id: Epic-12
feature_group: "VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 12 Agent Surface Map (`12.1..12.3`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `12.1` | `cli_standalone_api_parity` | `KERNEL` | `AGENT_FACING` | `CLI` / `WEB` | `LANE_1_DETERMINISTIC` | `uiux-os up/mcp` + `/api/*` daemon alias | Local binary assets | `CONSISTENT` |
| `12.2` | `tauri_v2_and_cowok_plugin` | `HYBRID` | `HUMAN_ONLY` / `AGENT_FACING` | `DESKTOP` / `EXTENSION_PLUGIN` | `LANE_1_DETERMINISTIC` | Tauri v2 + `cowok-plugin-pack` (`08.5`) | Local OS packages | `CONSISTENT` |
| `12.3` | `oci_container_pooled_chromium` | `KERNEL` | `AGENT_FACING` | `CONTAINER` | `LANE_1_DETERMINISTIC` | Docker/OCI + Pooled Headless Chromium | OCI Image | `CONSISTENT` |
