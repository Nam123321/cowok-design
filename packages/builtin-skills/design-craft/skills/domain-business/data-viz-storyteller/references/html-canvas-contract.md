# Interactive HTML Canvas Contract (Semantic Role & Honest Auto-Fix)

When generating or patching offline HTML5/SVG charts and dashboards for any conforming workbench host, obey this **Zero-Hardcode Semantic Role Contract** so manual controls, QA checks, and AI co-editing bind reliably to ANY domain dataset.

---

## 1. Zero-CDN Offline Self-Containment & Theme Polarity
- Emit pure semantic HTML5 + inline SVG + CSS custom properties resolved dynamically from the active project `tokens.css`:
  `var(--bg-surface)`, `var(--bg-elevated)`, `var(--bg-workspace)`, `var(--border-default)`, `var(--border-strong)`, `var(--text-primary)`, `var(--text-secondary)`, `var(--text-muted)`, `var(--action-primary)`, `var(--state-success)`, `var(--state-warning)`, `var(--state-danger)`.
- **Zero raw hex leaks** and **zero Tailwind `dark:` utilities** or external CDN script tags.
- All callout badges, tooltips, divergence banners, and popovers MUST pair `--bg-elevated` / `--bg-surface` with `--text-primary` and `--border-strong` to maintain **WCAG AA (`>= 4.5:1`) / AAA** readability in both Light and Dark (or any custom user-requested) `themeModes`.

---

## 2. Semantic Role Contract (Zero Hardcoded Demo IDs)

NEVER hardcode domain-specific demo identifiers (such as hardcoded revenue/growth/target card IDs). Instead, annotate structural regions with semantic `data-ff-role` attributes paired with dataset-derived IDs:

| Attribute | Allowed / Pattern Values | Host & Agent Binding Semantics |
| :--- | :--- | :--- |
| `data-ff-role` | `action-title` | Headline conclusion banner + subtitle (auto-checked by Action Title QA rule). |
| `data-ff-role` | `kpi-card` | Repeatable KPI metric tile (`tabular-nums`, optional target tick bar); attach dataset-derived `data-ff-id="kpi-<metric-slug>"`. |
| `data-ff-role` | `chart-panel` | Primary or secondary SVG visualization container. |
| `data-ff-role` | `data-datum` | Individual bar, line vertex, heatmap cell, or slice; MUST carry `data-row-id="<row-id>"`. |
| `data-ff-role` | `annotation-callout` | SVG/HTML callout card bound to a datum via `data-ff-callout-for="<row-id>"`. |
| `data-ff-role` | `breakdown-panel` | Secondary ranked share-bar, cohort breakdown, or sortable table (auto-collapsed in `ceo-3s` mode). |
| `data-ff-role` | `provenance-footnote` | Dataset source, timeframe, sample-size caveat (`n < 5`), and unit disclosure. |
| `data-row-id` | `<stable-row-id>` | Cross-Highlight, Pre-Attentive Spotlight Grey-Out, and Contextual Point Actions anchor. |
| `data-whatif-handle` | `<stable-row-id>` or `target-line` | Interactive drag handle for live What-If scenario testing. |
| `data-ff-callout-for` | `<stable-row-id>` | Links an `annotation-callout` pointer directly to its anchor datum. |
| `data-ff-compare-pair` | `<rowIdA>:<rowIdB>` | Active 2-bar comparison bracket showing absolute `Δ` and relative `%` delta. |
| `data-audience-mode` | `ceo-3s` \| `detailed` | Root attribute toggling Executive 3-second density vs Analyst detailed density cleanly via CSS. |

---

## 3. Five-Rule Honest & Wordless Auto-Fix Table (+ Mandatory Undo)

Every canvas MUST pass or offer 1-click reversible Auto-Fix for these 5 deterministic honesty & clarity rules:

| # | Rule ID | Defect Detected | 1-Click Auto-Fix Action | Undo / Safety Guarantee |
| :-: | :--- | :--- | :--- | :--- |
| **1** | `HONEST_ACTION_TITLE` | Title is a passive label (lacks metric conclusion or numeric delta). | Synthesize a concise empirical Action Title naming the top driver and exact delta from live rows. | Pushes prior snapshot to Undo history (`Cmd+Z` restores exact title). |
| **2** | `HONEST_ZERO_BASELINE` | Bar/column chart truncates Y-axis away from `0` or plots negative values on a non-signed chart. | Enforce `yMin = 0` on positive bars or switch to signed `movement` bars when negative deltas exist. | Reversible via 1-click Undo; never mutates underlying `sourceRows`. |
| **3** | `HONEST_MISSING_NULL` | Blank, `NaN`, or missing cells rendered as fake `0` or unlabeled bars. | Render explicit `—` gap markers and clear fallback labels without fabricating zero values. | Preserves raw missing flag in `sourceRows` provenance metadata. |
| **4** | `HONEST_PROPORTION_SAMPLE` | Pie/segmented `%` sum deviates from `100%` (`> ±0.5%`) OR sample size is tiny (`n < 5`). | Normalize proportional shares to `100%` (flagging rounding) and surface a compact `n < 5` caveat pill in `provenance-footnote`. | Reversible snapshot + explicit caveat footnote; never hides small base. |
| **5** | `WORDLESS_SPOTLIGHT_CLUTTER` | `>= 4` series/bars compete with equal visual weight or verbose body paragraphs clutter canvas. | Enable Pre-Attentive Spotlight on the focal `data-row-id`, mute peers to neutral grey, and distill text into 1–2 concise callouts. | Single-step Undo restores full-color un-spotlighted state. |

---

## 4. Honest-Data Divergence Invariant (`sourceRows`)
- Always keep an immutable reference copy of the user's imported `sourceRows`.
- During **What-If Scenario Drags** or AI-assisted simulations, compute divergence against `sourceRows` in real time. Whenever any value differs from `sourceRows`, display a visible **Divergence Banner** (`Simulated Scenario ≠ Source Data`) with two explicit actions: **Save Scenario Slide** (forking a labeled scenario artifact) and **Revert to Source** (1-click restoration).
