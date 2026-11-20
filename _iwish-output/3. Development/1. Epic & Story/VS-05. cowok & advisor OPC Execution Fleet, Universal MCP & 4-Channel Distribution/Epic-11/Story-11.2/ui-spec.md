---
type: I-Wish UI & Host Contract Spec
story_id: "11.2"
title: "UI Specification — 1-Click External Host MCP Installer & Live Activity Audit: Story 11.2"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: SettingsModal / SettingsMcpAutomationsPane
navigation_entry: SettingsModal -> MCP Tab -> External Host Installer & Live Audit Table
companion_surfaces:
  ai_chat_drawer: false
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='mcp-host-installer-list']"
  - "[data-ff-id='mcp-tool-permission-matrix']"
  - "[data-ff-id='mcp-external-activity-table']"
colocated_slice_dir: src/features/mcp-gateway
registration_mode: workbench_settings_tab
components:
  - SettingsMcpAutomationsPanePanel [5.2]
  - HostInstallerList [5.20]
  - ExternalActivityTable [5.21]
  - Toast [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
---

# UI Specification — Story 11.2: 1-Click Host MCP Installer UI & Live Activity Audit

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-05 -> Epic 11 -> Story 11.2 -> SettingsModal -> SettingsMcpAutomationsPanePanel`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`SettingsMcpAutomationsPane.tsx` inside `SettingsModal`).
- **`shell_mode`:** `shelled` (`ActorKind::McpClient` is strictly isolated from `HUMAN_ONLY` controls per `EC-LEARN-001`).
- **`shell_e2e_selectors`:** `[data-ff-id='mcp-host-installer-list']`, `[data-ff-id='mcp-tool-permission-matrix']`, `[data-ff-id='mcp-external-activity-table']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| 1-Click Host Installer Cards | List / Card Grid [5.20] | `src/pages/workbench/components/SettingsMcpAutomationsPane.tsx` | EXTEND (connect `POST /api/v1/mcp/install`) |
| 4-Tier Permission Toggles | Toggle Table [5.21] | `src/features/mcp-gateway/McpToolPermissionTable.tsx` | NEW_TABLE (`HUMAN_ONLY` permanently locked for MCP) |
| Live External Activity Audit Log | DataTableView [5.21] | `src/features/mcp-gateway/ExternalActivityTable.tsx` | REUSE (`GET /api/v1/mcp/external-activity`) |

## Component Hierarchy
```text
SettingsMcpAutomationsPanePanel (`src/pages/workbench/components/SettingsMcpAutomationsPane.tsx`)
├── HostInstallerList (`mcp-host-installer-list`, `Cowok.ai | Antigravity | Claude Code | Codex | Cursor`)
├── McpToolPermissionTable (`mcp-tool-permission-matrix`, locks `HUMAN_ONLY` rows offline from MCP)
└── ExternalActivityTable (`mcp-external-activity-table`, SSE live JSON-RPC call & Self-Healing log)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| `HUMAN_ONLY` Locked Badge | `--perm-human-only` | `#B91C1C` lock pill | Shown non-editable on MCP permission matrix |
| Checksum Backup Pill | `--install-backup-bg` | `var(--bg-subtle)` | Confirms pre-write host config backup |
| Row Density Height | `--table-row-compact` | `40px` | `ExternalActivityTable` entries |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Click 1-Click Install (`Cursor`) | Target detected | `POST /api/v1/mcp/install` (`O_NOFOLLOW` + atomic backup) | Writes MCP stanza without clobbering sibling keys; rollback available |
| External MCP calls `HUMAN_ONLY` | `ActorKind::McpClient` | Gateway returns JSON-RPC `-32003` + Workbench deep-link | Logged in `ExternalActivityTable`; zero privileged mutation |
