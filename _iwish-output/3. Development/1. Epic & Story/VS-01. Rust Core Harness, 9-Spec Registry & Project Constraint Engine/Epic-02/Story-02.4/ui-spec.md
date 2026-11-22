---
type: I-Wish UI & Host Contract Spec
story_id: "02.4"
title: "UI Specification — Hidden Dual-Use SSOT, AppShell Lock & cowok-design-opt: Story 02.4"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: WorkbenchHeader / ProjectConstraintPanel
navigation_entry: Workbench Top Bar Lock Toggle & Dual-Use Export Action
companion_surfaces:
  ai_chat_drawer: false
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='appshell-lock-panel']"
  - "[data-ff-id='ssot-export-button']"
  - "[data-ff-id='pattern-registry-drawer']"
colocated_slice_dir: src/features/ssot-registry
registration_mode: workbench_shell_control
components:
  - BrandIcon [5.1] (CMP-01)
  - AppShell [5.2] (CMP-02)
  - ProjectConstraintBadge [5.10] (CMP-03)
  - Toast [5.12]
  - RightDrawer [5.25]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.6. feature-hierarchy.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "../../../../2. Product Planning/2.13. host-integration-contract.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 02.4: Hidden Dual-Use SSOT & AppShell Lock Surface

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-01 -> Epic 02 -> Story 02.4 -> WorkbenchShell -> AppShellLockPanel -> PatternRegistryDrawer`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`WorkbenchShell.tsx`, keeping `confusion_risk < 0.35` by hiding raw technical schemas from non-tech users).
- **`shell_mode`:** `shelled` (`AppShellLockPanel` mounts unobtrusively in Workbench Inspector/Settings; exports `.cowok-design/exports/` on explicit request).
- **`shell_e2e_selectors`:** `[data-ff-id='appshell-lock-panel']`, `[data-ff-id='ssot-export-button']`, `[data-ff-id='pattern-registry-drawer']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| AppShell Lock Pill & Badge | ProjectConstraintBadge [5.10] (`CMP-03`) | `src/features/ssot-registry/AppShellLockPanel.tsx` | REUSE / EXTEND |
| Dual-Use Export Trigger | Primary Action Button [5.3] | `src/features/ssot-registry/SsotExportButton.tsx` | REUSE |
| Pattern Drift Drawer | RightDrawer [5.25] | `src/features/ssot-registry/PatternRegistryDrawer.tsx` | REUSE |
| Lock Conflict Alert (`409`) | Toast [5.12] | `src/components/ui/Toast.tsx` | REUSE |

## Component Hierarchy
```text
WorkbenchShell (`CMP-02`)
└── AppShellLockPanel (`appshell-lock-panel`, `Single Lock Owner` indicator)
    ├── LockOwnerBadgeCard (`fencing_token` owner pill)
    ├── SsotExportButton (`ssot-export-button` -> `.cowok-design/exports/`)
    └── PatternRegistryDrawer (`pattern-registry-drawer`, `quarantined_drift` list + `packages/ui-shell` sync)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Lock Active Fill | `--status-locked-bg` | `var(--bg-subtle)` | `AppShellLockPanel` active pill |
| Stroke Weight | `--icon-stroke` | `1.85px` | `<BrandIcon />` lock glyph |
| Drawer Width | `--drawer-width` | `420px` | `PatternRegistryDrawer` |
| State Transition | `--motion-fast` | `150ms cubic-bezier(0.2, 0, 0, 1)` | Lock toggle & drawer open |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Human clicks Lock AppShell | Valid `expectedRevision` | `POST /api/v1/ssot/appshell/lock` -> `ACQUIRE` | Renders Lock badge & `confusion_risk < 0.35` |
| Human clicks Export Docs | Project active | `POST /api/v1/ssot/export` -> Toast [5.12] | Writes `.cowok-design/exports/` MD/JSON |
| Stale agent write attempt | Missing lock token | Daemon rejects `409` -> `quarantined_drift` alert | `AppShell` navigation preserved |
