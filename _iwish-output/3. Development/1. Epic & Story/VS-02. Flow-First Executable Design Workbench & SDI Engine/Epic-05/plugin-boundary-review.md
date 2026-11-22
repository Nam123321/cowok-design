---
schema: plugin-boundary-review/v1
epic_id: Epic-05
feature_group: "VS-02. Flow-First Executable Design Workbench & SDI Engine"
applicability: NOT_APPLICABLE
delivery_strategy: CORE_WORKBENCH_ONLY
---

# Plugin Boundary Review — Epic 05 (`05.1..05.5`)

## 1. Per-Component 3-Axis Decomposition
| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `storyboard_proposals_and_snapshots` | `05.1..05.3` | `KERNEL` | `KERNEL_CORE` | Completed baseline: Launchpad, Storyboard, ChangeProposal, Snapshots & GitHub PR. |
| `cognitive_sketch_polish_and_html_forge` | `05.4` | `KERNEL` | `KERNEL_CORE` | Shares `crates/presentation-cms::cms_patcher` (`lol_html` + `html5ever` `SanitizeProfile::AppPreview`) into `crates/html-forge`, fixes `EMPTY_ELEMENT_RECTS`, flushes `localStorage` -> SQLite. |
| `simulate_user_and_fmea_dag_synth` | `05.5` | `DOMAIN` | `KERNEL_CORE` | Synthesizes 3-Tier Interactive Flow DAG (`<=8` happy-path nodes/cluster, 4-SubState pills) with 0 heuristic float leaks. |

## 2. Boundary Guardrails
- Mọi thao tác trực tiếp trên Preview (`DesignerTalkOverlay`, `SteeringChipsBar`) qua `2-Tier Auto-Apply`: text/spacing/token cho phép undo tức thì (`Cmd+Z`), còn rule bị khóa phải trả `LockedRuleChangeReceipt` không cho vượt qua `PROJECT_CONSTRAINTS`.
