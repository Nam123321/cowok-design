---
schema: agent-surface-map/v1
epic_id: Epic-13
feature_group: "VS-06. Release Certification & Anti-Goodhart Evaluation"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 13 Agent Surface Map (`13.1`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `13.1` | `sealed_eval_and_sim_suite` | `KERNEL` | `HUMAN_ONLY` / `READ_ONLY` | `UI_KIT` / `CLI` | `LANE_1_DETERMINISTIC` | `M1..M5` Certification Drawer, `uiux-os eval run --tier pr\|stage\|release`, `/api/v1/eval/*`, `tests/eval/sealed/` | `NEW_SCHEMA` (`V18 eval_benchmark_runs`) + `gate-history.jsonl` | `CONSISTENT` |
