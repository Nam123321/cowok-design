---
name: dashboard-ux
version: 2.0.0
description: Structures calm operational dashboards with KPI metric lines, progressive filters, data tables, and detail drawers.
risk_tier: 1
---

# Dashboard UX Craft

## Purpose
Organizes operational and analytical interfaces around rapid 3-second executive scannability, calm hierarchy, and zero decorative widget bloat. Pairs natively with the verbatim `dashboard-design` upstream skill and `data-viz-storyteller`.

## Minimum Floor, Never Maximum Ceiling
Start from the calm structural floor below (top KPI metric line, progressive filter bar, primary visualization/table, non-disruptive inspection drawer). Expand naturally with linked cross-filtering, saved views, cohort tabs, or real-time status strips whenever the domain workflow requires richer operational control.

## Operating Protocol
1. **Top-Fold KPI Scan Priority:** Lead with a compact horizontal KPI Metric Line (`tabular-nums`, period delta pill, optional target tick) rather than oversized multi-row hero cards.
2. **Progressive Disclosure Controls:** Place primary filters (timeframe, segment, search) in a single-row `AdvancedFilterBar`; fold secondary facets into a collapsible filter popover so canvas height stays generous.
3. **Calm Visual Grid:** Use an adaptive 12-column responsive grid collapsing cleanly across desktop, tablet, and mobile viewports; preserve generous breathing space (`>= 24px` section rhythm) and semantic CSS tokens (`var(--bg-surface)`, `var(--bg-workspace)`, `var(--border-default)`).
4. **Non-Disruptive Detail Inspection:** Route row drill-downs and record edits into a slide-over `RightDrawer` instead of jarring page jumps that lose table scroll and filter state.
5. **Deep Reference:** Load [references/operational-layout-patterns.md](references/operational-layout-patterns.md) for density modes, table anatomy, empty/loading states, and drawer composition.
