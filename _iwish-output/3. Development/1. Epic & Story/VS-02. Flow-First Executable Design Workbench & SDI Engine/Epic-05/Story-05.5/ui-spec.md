---
type: I-Wish UI & Host Contract Spec
story_id: "05.5"
title: "UI Specification — 3-Tier Interactive Flow DAG & 4-SubState Node Pills: Story 05.5"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: DagCanvas2D
navigation_entry: Flow Storyboard Canvas Toolbar & FlowNode Header Pills
companion_surfaces:
  ai_chat_drawer: true
  command_palette: false
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='show-edge-case-flows-toggle']"
  - "[data-ff-id='dag-flow-node-substate-pills']"
  - "[data-ff-id='dag-happy-path-cluster']"
colocated_slice_dir: src/features/flow-dag
registration_mode: workbench_canvas_toolbar
components:
  - DagCanvasPanel [5.2]
  - FlowNodeCard [5.15]
  - EdgeCaseFilterBar [5.22]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 05.5: 3-Tier Flow DAG & 4-SubState Pills

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-02 -> Epic 05 -> Story 05.5 -> DagCanvas2D -> FlowNodeCard (DEFAULT|LOADING|EMPTY|ERROR)`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`DagCanvas2D` inside `WorkbenchShell`).
- **`shell_mode`:** `shelled` (Maintains default `<= 8` happy-path cluster nodes so non-tech users are not overwhelmed on large 10+ screen apps).
- **`shell_e2e_selectors`:** `[data-ff-id='show-edge-case-flows-toggle']`, `[data-ff-id='dag-flow-node-substate-pills']`, `[data-ff-id='dag-happy-path-cluster']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| `[Show Edge-Case Flows]` Switch | AdvancedFilterBar [5.22] | `src/features/flow-dag/EdgeCaseFilterBar.tsx` | EXTEND (`tier="PRIMARY \| EDGE_CASE"`) |
| `FlowNode` 4-SubState Selector | FlowNodeCard [5.15] | `src/features/flow-dag/FlowNodeCard.tsx` | EXTEND (`DEFAULT \| LOADING \| EMPTY \| ERROR` pills) |
| Cluster Grouping Badge (`<=8` nodes) | Group Header Pill [5.10] | `src/features/flow-dag/HappyPathClusterSection.tsx` | REUSE |

## Component Hierarchy
```text
DagCanvasPanel (`src/pages/workbench/DagCanvas2D.tsx`)
├── EdgeCaseFilterBar (`show-edge-case-flows-toggle`, switches FMEA branch visibility)
├── HappyPathClusterSection (`dag-happy-path-cluster`, caps default nodes <= 8)
└── FlowNodeCard (`dag-flow-node-substate-pills`, binds `SampleDataStudio` fixtures per SubState)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Primary Flow Stroke | `--edge-primary` | `2px solid var(--action-primary)` | Happy-path edge |
| Edge-Case Dashed Stroke | `--edge-fmea` | `1.5px dashed var(--amber-600)` | `[Show Edge-Case Flows]` ON |
| Node Cluster Cap | `--max-default-nodes` | `8` visible happy-path nodes | Keeps SVG DAG `60fps` |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Synthesize from Brief | App goal captured | `POST /api/v1/flows/synthesize` | Renders `<= 8` primary cluster nodes; `0` raw floats in labels |
| Click `[Show Edge-Case Flows]` | Primary DAG visible | Reveals FMEA recovery edges (`ERROR`/`EMPTY` loopbacks) | Preserves valid `05.1` user `loopback` arcs |
| Click node `EMPTY` pill | `SampleDataStudio` fixture ready | Switches live preview `fixtureId` to empty dataset | Live iframe renders `EMPTY` state in `<50ms` |
