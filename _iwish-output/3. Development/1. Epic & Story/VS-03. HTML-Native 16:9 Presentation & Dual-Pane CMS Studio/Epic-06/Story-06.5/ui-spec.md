---
type: I-Wish UI & Host Contract Spec
story_id: "06.5"
title: "UI Specification — AI Multi-Slide Deck Gen & ChartSpecV12 Intake: Story 06.5"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: PresentationStudio / ProjectLaunchpadModal
navigation_entry: Deck Launchpad CSV Drop -> Slide Copilot Rewrite & Stageshow Frame Bar
companion_surfaces:
  ai_chat_drawer: true
  command_palette: false
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='deck-launchpad-form']"
  - "[data-ff-id='slide-copilot-panel']"
  - "[data-ff-id='stageshow-frame-selector']"
colocated_slice_dir: src/features/presentation-copilot
registration_mode: workbench_presentation_studio
components:
  - DeckLaunchpadForm [5.26]
  - SlideCopilotPanel [5.29]
  - StageshowFrameBar [5.10]
  - Toast [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 06.5: AI Multi-Slide Deck Gen & `ChartSpecV12` Intake

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-03 -> Epic 06 -> Story 06.5 -> PresentationStudio -> DeckLaunchpadForm / SlideCopilotPanel`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`PresentationStudio` 16:9 workspace).
- **`shell_mode`:** `shelled` (`DeckLaunchpadForm` + `SlideCopilotPanel` bind directly to `V15 presentation_decks` & `06.2` CMS).
- **`shell_e2e_selectors`:** `[data-ff-id='deck-launchpad-form']`, `[data-ff-id='slide-copilot-panel']`, `[data-ff-id='stageshow-frame-selector']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Deck Brief + `.csv/.tsv/.pdf` Intake | Form Modal [5.26] | `src/features/presentation-copilot/DeckLaunchpadForm.tsx` | EXTEND (connect real `/api/v1/decks/generate`) |
| Live Slide Copilot Rewrite | AI Copilot Panel [5.29] | `src/features/presentation-copilot/SlideCopilotPanel.tsx` | EXTEND (remove mock preview text) |
| `<Stage>/<Window>/<Phone>` Preset Picker | Segmented Bar [5.10] | `src/features/presentation-copilot/StageshowFrameBar.tsx` | REUSE (`stageshow-slide` skill) |
| Scanned PDF / CSV Validation Error | Toast [5.12] | `src/components/ui/Toast.tsx` | REUSE (`role="alert"`) |

## Component Hierarchy
```text
PresentationStudioSection
├── DeckLaunchpadForm (`deck-launchpad-form`, brief + CAS `.csv/.tsv/.pdf` upload)
├── StageshowFrameBar (`stageshow-frame-selector`, `Stage | Window | Phone` presets)
└── SlideCopilotPanel (`slide-copilot-panel`, `<=30` words/slide budget meter + `ChartSpecV12` chart preview)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Slide Canvas Ratio | `--slide-aspect` | `16 / 9` (`1920x1080`) | `PresentationStudioSection` |
| Word Budget Pill | `--slide-max-words` | `<= 30 words/slide`, `3-7 words/bullet` | `SlideCopilotPanel` live meter |
| Proportional Chart Sum | `--chart-pie-tolerance` | `100 +/- 0.05` (original units in labels) | `ChartSpecV12` validation badge |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Submit Brief + `.csv` | Valid CAS SHA-256 | `POST /api/v1/decks/normalize-chart-source` -> `POST /api/v1/decks/generate` | Multi-slide `V15` deck created (`0` Q1..Q3 mock echo) |
| Upload image-only/corrupt PDF | `0` extractable tables | Structured error shown via Toast [5.12] | No empty slide crash; guides user to paste CSV/TSV |
| Trigger Copilot rewrite | `slide_uid` active | `POST /api/v1/slides/copilot/rewrite` | Enforces `<=30` words or atomic split to new ULID slide |
