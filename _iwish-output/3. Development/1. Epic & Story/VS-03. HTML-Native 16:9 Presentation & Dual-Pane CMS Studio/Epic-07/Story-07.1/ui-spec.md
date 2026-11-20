---
type: I-Wish UI & Host Contract Spec
story_id: "07.1"
title: "UI Specification — Visual QC Overlay, Project Readiness Dashboard & 1-Click Auto-Fix: Story 07.1"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: CanvasStageView / ProjectReadinessModal
navigation_entry: Top Readiness Pill -> Visual QC Overlay & 1-Click Auto-Fix Drawer
companion_surfaces:
  ai_chat_drawer: false
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='visual-qc-overlay']"
  - "[data-ff-id='project-readiness-panel']"
  - "[data-ff-id='one-click-lint-fix-button']"
colocated_slice_dir: src/features/visual-qc
registration_mode: workbench_stage_overlay
components:
  - VisualQcOverlayPanel [5.15]
  - ProjectReadinessModal [5.26]
  - LintFixActionButton [5.3]
  - Toast [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 07.1: Visual QC Overlay & 1-Click Auto-Fix

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-03 -> Epic 07 -> Story 07.1 -> WorkbenchShell -> VisualQcOverlayPanel / ProjectReadinessModal`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`CanvasStageView` + `PresentationStudio` `1920x1080`).
- **`shell_mode`:** `shelled` (Renders violation bounding boxes from Pooled 1-Chromium CDP + `elementsFromPoint` directly over preview without mutating iframe source).
- **`shell_e2e_selectors`:** `[data-ff-id='visual-qc-overlay']`, `[data-ff-id='project-readiness-panel']`, `[data-ff-id='one-click-lint-fix-button']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Live Coordinate Bounding Boxes | Overlay Layer [5.15] | `src/features/visual-qc/VisualQcOverlayPanel.tsx` | REUSE (`lib-audit` + `slides-check`) |
| Project Health Summary | Modal / Drawer [5.26] | `src/features/visual-qc/ProjectReadinessModal.tsx` | NEW_REGISTRY_MODAL (`P0/P1`, `UNVERIFIED_NO_BROWSER` badge) |
| `<10ms` Deterministic Auto-Fix | Action Button [5.3] | `src/features/visual-qc/LintFixActionButton.tsx` | REUSE (`0` LLM tokens) |

## Component Hierarchy
```text
WorkbenchShell
├── ProjectReadinessModal (`project-readiness-panel`, E2E Flow Completeness + CDP status)
└── VisualQcOverlayPanel (`visual-qc-overlay`, WCAG/overflow/occlusion bounding boxes)
    ├── ViewportAuditBar (`1440 | 1024 | 768 | 390 | 16:9` switcher)
    └── LintFixActionButton (`one-click-lint-fix-button`, `Cmd+Z` diff compare, max `3` agent retries)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| P0 Violation Stroke | `--qc-p0-outline` | `2px solid #DC2626` | Overlap / Overflow / Dead-End CTA box |
| Deterministic Fix SLA | `--qc-lintfix-ms` | `< 10ms`, `0` LLM tokens | Spacing/token/radius auto-fix |
| No-Browser Honest Badge | `--qc-unverified-bg` | `var(--bg-subtle)` warning pill | Displayed when `verdict == UNVERIFIED_NO_BROWSER` |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Click Run QC | Pooled CDP idle or queued | `POST /api/v1/qc/run` -> caches by `layout_sha1` | Highlights real CDP/SSIM rects (never `drawWireframeCard`) |
| Click 1-Click Auto-Fix | Deterministic rule violation | `POST /api/v1/qc/auto-fix` (`DETERMINISTIC_LINT_FIX`) | Fixes layout `<10ms`, records snapshot for `Cmd+Z` |
| Trigger Agent Auto-Fix >= 3x | `autofixAttempt == 3` | Locks button, offers manual restore + diff comparison | Prevents runaway CSS rewrites |
