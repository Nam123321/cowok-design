# Operational Dashboard Layout Patterns (`dashboard-ux`)

## 1. Canonical Four-Band Dashboard Anatomy
1. **Header & Action Band:** Concise view title, timeframe selector, audience density toggle (`CEO 3s` vs `Detailed`), and at most one primary CTA (`var(--action-primary)`).
2. **KPI Metric Line:** `3–6` scannable metric tiles using `font-variant-numeric: tabular-nums`, explicit units, compact delta indicators (`var(--state-success)` / `var(--state-danger)`), and subtle hover provenance.
3. **Analytical Focal Band:** Primary chart (`data-ff-role="chart-panel"`) paired with a ranked breakdown or secondary comparison card in an asymmetric `8/4` or `7/5` split on wide screens, stacking cleanly below `960px`.
4. **Operational Table + RightDrawer:** Sticky-header `DataTableView` with right-aligned numeric columns, inline status pills, keyboard row navigation, and a non-modal `RightDrawer` (`380px–480px`) for entity inspection without losing filter state.

## 2. Anti-Clutter & State Resilience Checklist
- **No Mystery Icons:** Pair unfamiliar icon-only controls with concise labels or accessible tooltips.
- **Complete State Coverage:** Provide skeleton loaders matching exact row/card heights (zero CLS), helpful empty states with clear recovery actions, and inline retry banners for partial fetch failures.
- **Semantic Tokens Only:** Never mix raw hex values or Tailwind `dark:` classes; inherit surfaces and borders from the active `tokens.css`.
