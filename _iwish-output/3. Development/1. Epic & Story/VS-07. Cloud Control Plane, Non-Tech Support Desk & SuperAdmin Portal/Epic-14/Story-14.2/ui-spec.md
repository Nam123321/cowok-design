---
type: I-Wish UI & Host Contract Spec
story_id: "14.2"
title: "UI Specification — SuperAdmin Portal MVP (2-Way Helpdesk, Feature Clusters, Activation Funnel & Crash Center): Story 14.2"
status: active
has_ui: true
host_portal: superadmin
shell_mode: shelled
chromeless_reason: null
router_file: src/App.jsx
route_mount_parent: SuperAdminPortalPage (`/admin`)
navigation_entry: Dedicated Cloud Portal route `/admin` (separated from local WorkbenchShell)
companion_surfaces:
  ai_chat_drawer: false
  command_palette: false
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='superadmin-portal-root']"
  - "[data-ff-id='superadmin-kpi-metric-line']"
  - "[data-ff-id='superadmin-helpdesk-table']"
  - "[data-ff-id='superadmin-ticket-drawer']"
  - "[data-ff-id='superadmin-funnel-chart']"
  - "[data-ff-id='superadmin-crash-table']"
colocated_slice_dir: src/features/superadmin
registration_mode: dedicated_portal_route
components:
  - MetricLine [5.10]
  - DataTableView [5.21]
  - AdvancedFilterBar [5.22]
  - RightDrawer [5.25]
  - Toast [5.12]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../../DESIGN.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 14.2: SuperAdmin Portal MVP (`/admin`)

## Registered UX Patterns & Components Alignment (`DESIGN.md §5`)

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| DAU / WAU / MAU + Channel KPI Strip | Metric Line `[5.10]` | `src/features/superadmin/SuperAdminKpiStrip.tsx` | REUSE (`DESIGN.md §5.1`, semantic CSS variables) |
| Helpdesk Ticket Inbox & Crash Triage List | DataTableView `[5.21]` | `src/features/superadmin/HelpdeskInboxTab.tsx` / `CrashCenterTab.tsx` | REUSE (`40px` compact rows, zero Tailwind) |
| Priority / Channel / Host / Search Filters | AdvancedFilterBar `[5.22]` | `src/features/superadmin/SuperAdminFilterBar.tsx` | REUSE (multi-facet search & priority pills) |
| Ticket Detail, Sanitized Diagnostics & 2-Way Reply | RightDrawer `[5.25]` | `src/features/superadmin/TicketDetailDrawer.tsx` | REUSE (`P0-P3` selector, 2-way reply composer, 1-click GitHub link) |
| Action Feedback & RBAC Errors | Toast `[5.12]` | `src/components/ui/CowokToast.tsx` | REUSE |

## Portal Navigation & Tabs
1. **OIDC + MFA Gate Bar:** Displays active operator role badge (`SUPER_ADMIN | SUPPORT_AGENT | SKILL_CURATOR | ANALYST_READONLY`) and enforces UI read-only locks for `ANALYST_READONLY`.
2. **Tab 1 — Helpdesk Inbox (`2-Way Sync`):** `AdvancedFilterBar` + `DataTableView` + `RightDrawer` with Sanitized Diagnostic Bundle viewer, priority P0–P3 switcher, and 2-way reply pushing back to `Story 14.1` client inbox.
3. **Tab 2 — Feature Request Clusters:** Deduplicated cluster cards + merge tickets action + 1-click GitHub Issue link generator.
4. **Tab 3 — Activation Funnel & Adoption:** `Metric Line` (DAU/WAU/MAU, 4 channels, 5 plugin hosts) + 5-step funnel drop-off bar (`Day-0 -> AI Connected -> Project -> Preview -> Export`) + feature frequency table.
5. **Tab 4 — Crash Center:** Fingerprint-grouped crash table by `app_version`, sanitized stack preview, and status triage.

## Design System Invariants
- 100% Semantic CSS Variables (`var(--bg-card)`, `var(--text-primary)`, `var(--border-subtle)`) — zero Tailwind classes, zero hardcoded dark/light literals.
- `<BrandIcon>` monoline icons + localized across all 9 locales (`src/i18n/locales/*.json`) + `Intl` formatting (`useFormatter` / `Intl.NumberFormat`).
