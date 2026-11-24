# Interactive HTML Canvas Contract (Host Workbench)

When generating or patching HTML charts & dashboards inside an interactive HTML workbench host, obey these deterministic host hooks so manual UI controls and AI Sub-Chat co-editing work seamlessly:

1. **Zero-CDN Offline Self-Containment:** Pure semantic HTML5 + inline SVG + CSS custom properties (`var(--bg-surface)`, `var(--bg-workspace)`, `var(--border-default)`, `var(--text-primary)`, `var(--text-secondary)`, `var(--action-primary)`, `var(--state-success)`, `var(--state-warning)`, `var(--state-danger)`). Never emit hardcoded hex colors or external `<script src="...">` CDNs.
2. **Mandatory `data-ff-id` Selectable Anchors:**
   - `dashboard-title` (Action Title + subtitle)
   - `kpi-revenue-card`, `kpi-growth-card`, `kpi-target-card`
   - `main-chart-panel` (with per-row `data-row-id="<id>"` for Cross-Highlight, Spotlight Grey-Out, and What-If drag)
   - `ranked-share-bar`
   - `dashboard-footnote`
3. **Honest-Data Divergence Invariant:**
   - Never silently overwrite user-supplied source numbers when running What-If simulations or AI refinements; always preserve `sourceRows` so the host UI displays an explicit divergence badge and 1-click revert.
4. **Light & Dark Theme Polarity:**
   - All callout badges, hover tooltips, and comment popovers MUST use `--bg-elevated` / `--bg-surface` + `--text-primary` + `--border-strong` so contrast stays WCAG AA/AAA in both Warm Light and Night Dark modes.
