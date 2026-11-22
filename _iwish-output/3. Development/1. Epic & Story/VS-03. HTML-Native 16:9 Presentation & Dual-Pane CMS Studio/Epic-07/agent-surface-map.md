---
schema: agent-surface-map/v1
epic_id: Epic-07
feature_group: "VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 07 Agent Surface Map (`07.1`, `07.2`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `07.1` | `pooled_cdp_qc_and_overlay` | `KERNEL` | `AGENT_FACING` / `HUMAN_ONLY` | `TOOL` / `UI_KIT` | `LANE_1_DETERMINISTIC` | `/api/v1/qc/*`, Pooled 1-Chromium CDP + `VisualQcOverlay` + 1-Click Auto-Fix | `NEW_SCHEMA` (`V17 qc_gate_runs`) | `CONSISTENT` |
| `07.2` | `split_share_vs_dev_exporter` | `HYBRID` | `HUMAN_ONLY` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `/api/v1/export/*` -> `crates/html-forge` + CDP PDF + PPTX plugin (`TDR-006`) | `EPHEMERAL` / User Export | `CONSISTENT` |
