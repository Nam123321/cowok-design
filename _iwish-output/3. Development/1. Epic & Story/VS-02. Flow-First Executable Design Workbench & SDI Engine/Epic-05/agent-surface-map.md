---
schema: agent-surface-map/v1
epic_id: Epic-05
feature_group: "VS-02. Flow-First Executable Design Workbench & SDI Engine"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 05 Agent Surface Map (`05.1..05.3` Baseline + `05.4`, `05.5`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `05.1..05.3` | `sdi_storyboard_and_proposals` | `KERNEL` | `HUMAN_ONLY` / `AGENT_PROPOSE_ONLY` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `crates/harness-daemon` *(completed)* | `EXISTING_SCHEMA` | `CONSISTENT` |
| `05.4` | `sketch_paper_and_steering_ui` | `KERNEL` | `HUMAN_ONLY` | `UI_KIT` | `LANE_1_DETERMINISTIC` | `VisualClarificationModal` + `data-stage-skin="sketch-paper"` + `SteeringChipsBar` + `DesignerTalkOverlay` | `EXISTING_SCHEMA` (flush `localStorage`) | `CONSISTENT` |
| `05.4` | `shared_lol_html_and_reconcile_ff` | `KERNEL` | `AGENT_FACING` | `TOOL` | `LANE_1_DETERMINISTIC` | `crates/html-forge` sharing `presentation-cms::cms_patcher` + `SurgicalPatcher::reconcile_ff_ids` | `EXISTING_SCHEMA` | `CONSISTENT` |
| `05.5` | `simulate_user_fmea_dag_synth` | `DOMAIN` | `AGENT_PROPOSE_ONLY` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `/api/v1/flows/synthesize` + `[Show Edge-Case Flows]` lens (`<=8` nodes) | `EXISTING_SCHEMA` (`flow_nodes`, `flow_edges`) | `CONSISTENT` |
