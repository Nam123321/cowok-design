---
type: I-Wish UI & Host Contract Spec
story_id: "07.2"
title: "UI Specification — Split Export (Share & Present vs Developer Handoff): Story 07.2"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: ExportBundleModal / ExportActionDropdown
navigation_entry: Top Right Export Button -> Split Export Modal
companion_surfaces:
  ai_chat_drawer: false
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='export-bundle-modal']"
  - "[data-ff-id='share-present-tab']"
  - "[data-ff-id='developer-handoff-tab']"
  - "[data-ff-id='export-pptx-plugin-cta']"
colocated_slice_dir: src/features/split-export
registration_mode: workbench_export_modal
components:
  - ExportBundleModal [5.26]
  - SharePresentPanel [5.29]
  - DeveloperHandoffPanel [5.29]
  - Toast [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
---

# UI Specification — Story 07.2: Split Export (`Share & Present` vs `Developer Handoff`)

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-03 -> Epic 07 -> Story 07.2 -> WorkbenchShell -> ExportBundleModal -> SharePresentPanel | DeveloperHandoffPanel`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`WorkbenchShell` Export Action Dropdown & Drawer).
- **`shell_mode`:** `shelled` (Defaults non-tech users into **`Share & Present`** tab; separates React/Tailwind code & specs cleanly into **`Developer Handoff`**).
- **`shell_e2e_selectors`:** `[data-ff-id='export-bundle-modal']`, `[data-ff-id='share-present-tab']`, `[data-ff-id='developer-handoff-tab']`, `[data-ff-id='export-pptx-plugin-cta']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Split Export Modal | Modal [5.26] | `src/features/split-export/ExportBundleModal.tsx` | EXTEND (split 2 audience tabs) |
| Non-Tech Offline HTML/PDF/PPTX Tab | Action Bento Panel [5.29] | `src/features/split-export/SharePresentPanel.tsx` | NEW_TAB_PANEL |
| Dev TSX/CSS/Tokens/DAG Tab | Action Bento Panel [5.29] | `src/features/split-export/DeveloperHandoffPanel.tsx` | NEW_TAB_PANEL |
| P0 QC Blocker & Override Alert | Toast [5.12] | `src/components/ui/Toast.tsx` | REUSE (`role="alert"`) |

## Component Hierarchy
```text
ExportBundleModal (`export-bundle-modal`)
├── SharePresentPanel (`share-present-tab`, Offline 1-file HTML, Vector PDF, optional `.pptx` plugin CTA)
├── DeveloperHandoffPanel (`developer-handoff-tab`, TSX/CSS/Tokens/`SPEC.md`/Playwright bundle)
└── QcGateBannerCard (shows Story 07.1 pass status or explicit `override_qc_warning` toggle)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Default Audience Tab | `--export-default-tab` | `share-present` first | Prevents non-tech confusion |
| Offline Asset Bound | `--export-max-asset-mb` | `<= 25MB` inline Data-URI limit | Guard against Base64 OOM |
| CSP Export Policy | `--export-csp-header` | `default-src 'self' data:` (`0` CDN) | Verified by Playwright offline test |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Click 1-File Offline HTML | `qc_gate_runs` PASS or `override_qc_warning` | `POST /api/v1/export/html-bundle` | Downloads self-contained `.html` with `0` network requests |
| Click Export Vector PDF | 16:9 Slide Deck ready | `POST /api/v1/export/vector-pdf` | Generates vector PDF via `crates/html-forge` |
| Click Export `.pptx` (no plugin) | PPTX extension absent | Displays guided Extension Install CTA (`export-pptx-plugin-cta`) | Core binary runs safely with zero missing-LibreOffice crash |
