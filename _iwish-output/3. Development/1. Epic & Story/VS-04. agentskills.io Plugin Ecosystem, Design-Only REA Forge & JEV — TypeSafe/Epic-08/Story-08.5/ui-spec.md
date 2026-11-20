---
type: I-Wish UI & Host Contract Spec
story_id: "08.5"
title: "UI Specification — Plugin Center Repo Import, Ephemeral Tool Audit & LEARN Queue: Story 08.5"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/plugins/PluginCenterPage.tsx
route_mount_parent: PluginCenterWorkspace / SettingsModal
navigation_entry: Plugin Center Repo Import -> Ephemeral Tool Audit Drawer & LEARN Queue
companion_surfaces:
  ai_chat_drawer: false
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='plugin-repo-import-form']"
  - "[data-ff-id='ephemeral-tool-audit-drawer']"
  - "[data-ff-id='de-tools-status-list']"
  - "[data-ff-id='learn-ledger-queue']"
colocated_slice_dir: src/features/plugin-center
registration_mode: five_nouns_host_slot
components:
  - PluginCenterWorkspacePage [5.2]
  - RepoImportForm [5.26]
  - EphemeralToolAuditDrawerPanel [5.25]
  - LearnLedgerQueueList [5.21]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 08.5: Split DE+DR Plugin Runtime, Tool Jail & `LEARN` Loop

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-04 -> Epic 08 -> Story 08.5 -> PluginCenterPage -> RepoImportForm | EphemeralToolAuditDrawerPanel | LearnLedgerQueueList`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`PluginCenterPage.tsx` under `host-plugin-center` slot + `SettingsModal`).
- **`shell_mode`:** `shelled` (Preserves `EC-LEARN-001` strict `ActorKind::HumanWorkbenchSession` separation from MCP/Agent calls).
- **`shell_e2e_selectors`:** `[data-ff-id='plugin-repo-import-form']`, `[data-ff-id='ephemeral-tool-audit-drawer']`, `[data-ff-id='de-tools-status-list']`, `[data-ff-id='learn-ledger-queue']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| Repo / User Skill Import UI | Modal Form [5.26] | `src/features/plugin-center/RepoImportForm.tsx` | EXTEND (add License/ToS + AST scan status) |
| Ephemeral Tool Jail (`HUMAN_ONLY`) | RightDrawer [5.25] | `src/features/plugin-center/EphemeralToolAuditDrawerPanel.tsx` | NEW_DRAWER (`env_clear`, `O_NOFOLLOW`, `-PGID`) |
| 7 Built-in DE Tools (`<=25ms`) | Status List [5.20] | `src/features/plugin-center/DeToolsStatusList.tsx` | REUSE |
| `V18a` Ledger -> `LEARN Queue` | Review List [5.21] | `src/features/plugin-center/LearnLedgerQueueList.tsx` | EXTEND (`08.4` `LearnCardReviewQueue`) |

## Component Hierarchy
```text
PluginCenterWorkspacePage (`/plugins`)
├── RepoImportForm (`plugin-repo-import-form`, License/ToS + `<=100`-tok header preview)
├── DeToolsStatusList (`de-tools-status-list`, 7 `$0` DE tools `<=25ms`)
├── EphemeralToolAuditDrawerPanel (`ephemeral-tool-audit-drawer`, `HUMAN_ONLY` sandbox execute/kill)
└── LearnLedgerQueueList (`learn-ledger-queue`, `hit_count>=3`, `SLPR>=0.95` promote / `<0.85` auto-retire)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Jail Security Badge | `--badge-human-only` | `1px solid #B91C1C` | `EphemeralToolAuditDrawerPanel` |
| DE Execution SLA | `--de-runtime-budget` | `<= 25ms`, `$0` cost | `DeToolsStatusList` badge |
| Header Token Cap | `--jit-header-max` | `<= 100 tokens` | `RepoImportForm` registration preview |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Submit external repo URL | SSRF + License PASS | `POST /api/v1/plugins/import-repo` | Packages `SKILL.md` (`<=100` tok) into `V16c` |
| Click Execute Ephemeral Script | `HumanWorkbenchSession` only | `POST /api/v1/plugins/ephemeral-tools/execute` | Runs inside `env_clear` + `O_NOFOLLOW` + `-PGID` `<=5s` |
| Candidate hits `SLPR >= 0.95` | `hit_count >= 3` | Surfaces as `PROPOSED` in `LearnLedgerQueueList` | Waits for explicit `08.4 HUMAN_ONLY` acceptance |
