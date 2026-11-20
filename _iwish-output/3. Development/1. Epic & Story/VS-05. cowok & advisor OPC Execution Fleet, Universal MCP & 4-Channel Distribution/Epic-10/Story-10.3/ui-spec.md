---
type: I-Wish UI & Host Contract Spec
story_id: "10.3"
title: "UI Specification — CMP-07 DesignTournamentArena, Blind Wipe & /crystallize: Story 10.3"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: CanvasStageView / DesignTournamentArena
navigation_entry: Chat & Canvas Compare Trigger -> CMP-07 DesignTournamentArena Overlay
companion_surfaces:
  ai_chat_drawer: true
  command_palette: false
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='design-tournament-arena']"
  - "[data-ff-id='tournament-blind-wipe-slider']"
  - "[data-ff-id='cross-variant-borrow-input']"
  - "[data-ff-id='crystallize-action-button']"
colocated_slice_dir: src/features/tournament
registration_mode: workbench_stage_arena
components:
  - DesignTournamentArenaPanel [5.29]
  - BlindWipeSliderBar [5.15]
  - CrossVariantBorrowForm [5.22]
  - CrystallizeActionButton [5.3]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 10.3: `CMP-07 DesignTournamentArena`, `Blind Wipe` & `/crystallize`

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-05 -> Epic 10 -> Story 10.3 -> WorkbenchShell -> DesignTournamentArenaPanel (CMP-07)`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`CMP-07 DesignTournamentArena` synchronized between Chat and 2D Stage).
- **`shell_mode`:** `shelled` (Supports side-by-side Desktop and `390px` Mobile `Blind Wipe` comparison with anonymized model labels).
- **`shell_e2e_selectors`:** `[data-ff-id='design-tournament-arena']`, `[data-ff-id='tournament-blind-wipe-slider']`, `[data-ff-id='cross-variant-borrow-input']`, `[data-ff-id='crystallize-action-button']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Tournament Multi-Axis Arena | DesignTournamentArena (`CMP-07`) | `src/features/tournament/DesignTournamentArenaPanel.tsx` | REUSE / EXTEND (`CMP-07`) |
| `Blind Wipe` Split Judge (`Desktop/390px`) | Comparison Wipe Slider [5.15] | `src/features/tournament/BlindWipeSliderBar.tsx` | NEW_SLIDER_BAR (hides model/agent names) |
| Cross-Variant Element Borrow Input | Form Bar [5.22] | `src/features/tournament/CrossVariantBorrowForm.tsx` | NEW_FORM (`reconcile_ff_ids` + CSS rules) |
| 1-Click `/crystallize` Packaging | Primary Action Button [5.3] | `src/features/tournament/CrystallizeActionButton.tsx` | REUSE |

## Component Hierarchy
```text
DesignTournamentArenaPanel (`design-tournament-arena`, `CMP-07`)
├── BlindWipeSliderBar (`tournament-blind-wipe-slider`, Desktop + 390px split view, anonymized labels)
├── RubricShieldSummaryCard (position-swapped `(A,B)/(B,A)` blind verdict pill)
├── CrossVariantBorrowForm (`cross-variant-borrow-input`, optional borrow instruction)
└── CrystallizeActionButton (`crystallize-action-button` -> `/api/v1/crystallize`)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Blind Wipe Divider | `--wipe-handle-width` | `4px` high-contrast rail | Drag comparison split |
| Mobile Judge Width | `--mobile-judge-viewport` | `390px` fixed preview frame | Mobile side-by-side parity |
| AST Diversity Badge | `--ast-similarity-max` | `<= 0.78` verified green pill | Shown per `tournament_variants` row |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Launch Design Tournament | Prompt ready | `POST /api/v1/tournament/start` (`<=15s`) | Shows `>=2` distinct orthogonal variants (`similarity <= 0.78`) |
| Rapid switch Option A -> B -> C | Multiple previews hovered | Debounced `generationEpoch` tracks last active tab only | Commits ONLY confirmed active winner (`0` race conditions) |
| Confirm Winner + Borrow | Borrow note entered | `POST /api/v1/tournament/select` | Safely merges zone + CSS tokens via `reconcile_ff_ids` |
