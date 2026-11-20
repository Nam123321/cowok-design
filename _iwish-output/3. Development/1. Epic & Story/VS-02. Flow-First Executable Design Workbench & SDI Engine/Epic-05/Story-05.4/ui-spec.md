---
type: I-Wish UI & Host Contract Spec
story_id: "05.4"
title: "UI Specification — Cognitive Sketch-to-Polish, Steering Chips & DesignerTalkOverlay: Story 05.4"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: DualPaneStageSubpanes / CanvasStageView
navigation_entry: Day-0 Onboarding -> Canvas Stage Toolbar -> Sketch/Color Switcher & Overlay
companion_surfaces:
  ai_chat_drawer: true
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='visual-clarification-modal']"
  - "[data-ff-id='stage-skin-toggle']"
  - "[data-ff-id='steering-chips-bar']"
  - "[data-ff-id='designer-talk-overlay']"
colocated_slice_dir: src/features/sketch-polish
registration_mode: workbench_canvas_stage
components:
  - VisualClarificationModal [5.26]
  - SteeringChipsBar [5.10]
  - DesignerTalkOverlay [5.15]
  - LockedRuleChangeReceiptCard [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 05.4: Cognitive Sketch-to-Polish & Direct Manipulation

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-02 -> Epic 05 -> Story 05.4 -> Day-0 Welcome -> VisualClarificationModal -> DualPaneStageSubpanes ([data-stage-skin])`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`DualPaneStageSubpanes.tsx` inside `WorkbenchShell`).
- **`shell_mode`:** `shelled` (`iframe` sandbox preserves `0 allow-same-origin`; `bridge.elementRects` & `__jevFast` communicate over `MessageChannel` with `epoch + bootSecret`).
- **`shell_e2e_selectors`:** `[data-ff-id='visual-clarification-modal']`, `[data-ff-id='stage-skin-toggle']`, `[data-ff-id='steering-chips-bar']`, `[data-ff-id='designer-talk-overlay']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Live Direction Modal (`-PGID` preempt) | Modal [5.26] | `src/features/sketch-polish/VisualClarificationModal.tsx` | EXTEND (`2 Live Direction Switcher Pills`) |
| `[Sketch \| Brand Color]` Skin Switch | Segmented Toggle Bar [5.10] | `src/features/sketch-polish/StageSkinToggleBar.tsx` | REUSE (`data-stage-skin="sketch-paper"`) |
| `impeccable-polish` Quick Adjustments | Chip Bar [5.10] | `src/features/sketch-polish/SteeringChipsBar.tsx` | EXTEND (`impeccable-polish` tokens) |
| Direct HTML Click Coordinates (`bridge.elementRects`) | Canvas Overlay [5.15] | `src/pages/workbench/DualPaneStageSubpanes.tsx` | EXTEND (replace `EMPTY_ELEMENT_RECTS`) |

## Component Hierarchy
```text
CanvasStageView
├── VisualClarificationModal (`visual-clarification-modal`, 2 Live Direction Switcher Pills)
├── StageSkinToggleBar (`stage-skin-toggle`, `sketch-paper` <-> Brand Color `<16ms`)
├── SteeringChipsBar (`steering-chips-bar`, 1-click typography/density/radius chips)
└── DesignerTalkOverlay (`designer-talk-overlay`, `elementFromPoint` + 2-Tier `LockedRuleChangeReceiptCard`)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Sketch Paper Background | `--stage-sketch-bg` | `#FAF9F6` (monochrome wireframe) | `[data-stage-skin="sketch-paper"]` |
| Sketch Line Ink | `--stage-sketch-ink` | `#2E2D2B` (`4.5:1` AA contrast) | Borders, labels, placeholders |
| Brand Swap Latency Budget | `--skin-swap-duration` | `<= 16ms` CSS-variable swap | Zero DOM mutation on toggle |
| OKLCH Contrast Guard | `--wcag-min-ratio` | `>= 4.5` (`AA`) auto-clamp | Prevents unreadable tint swaps |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Click Live Direction Pill 2 | Job 1 active | `POST /api/v1/sdi/clarify` with `preemptPgid` | Kills Job 1 `-PGID`, increments `generationEpoch` |
| Toggle `[Sketch \| Brand Color]` | Preview loaded | Swaps CSS tokens `<16ms` + OKLCH guard | Zero DOM reload; `ast_revision` unchanged |
| Click element on canvas | `bridge.elementRects` populated | Computes hit via `MessagePort` -> opens inline popover | Flushes `useAppScreenCmsState` to SQLite WAL (`0` localStorage drift) |
