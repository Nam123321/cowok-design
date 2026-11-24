---
name: data-viz-storyteller
version: 2.0.0
description: Designs insight-led charts, executive data stories, 3-lens hypotheses, spotlight callouts, and honest HTML/SVG dashboards.
risk_tier: 1
---

# Data Visualization & Executive Storyteller

## Purpose
Transforms raw analytical metrics (CSV/TSV/PDF/tables) into trustworthy, interactive HTML5/SVG charts and narrative slide decks grounded in Cole Nussbaumer Knaflic (*Storytelling with Data*), Stephen Few, Edward Tufte, and the companion `dashboard-design` peer skill (`dashboard-design-upstream`).

## Operating Protocol (Reasoning → Structured Story → Interactive HTML)
1. **Load Visual Craft Baseline:** Always pair with the `dashboard-design` peer skill (`dashboard-design-upstream`) for visual calm (*First glance 3s*, *Quiet*, *Wordless*, *Honest*, *One entity hue + grey baseline*).
2. **Write an Action Title (Never a Label Title):**
   - Replace generic labels (*"Q4 Revenue by Channel"*) with a 1-line conclusion (*"Partner drove 55% of Q4 revenue, beating target by $200"*).
   - Keep body prose wordless: numbers, units, and compact deltas on-canvas; explanations live in tooltips or targeted callout arrows.
3. **Synthesize 3 Instant Narrative Lenses (`AI Lens Strip`):**
   - For every dataset, propose 3 distinct story angles so the user can toggle in 1 click without losing data:
     - **Driver Lens** (*Who leads / drives variance?*) → Grouped Bar or Ranked Share-Bar with Spotlight Grey-Out.
     - **Target Lens** (*Are we on track vs goal?*) → Bullet Graph or Target Tick Bar with draggable target threshold.
     - **Dynamics / Mix Lens** (*How is mix shifting?*) → Movement Bars (New/Expansion/Contraction/Churn), Segmented Share Bar, Line/Sparkline, or Heatmap Matrix.
4. **Apply Pre-Attentive Spotlight & Callout Arrows:**
   - Render baseline series in muted neutral grey (`var(--border-strong)` / `var(--text-muted)`).
   - Highlight ONLY the focal series/bar (`spotlightRowId`) with the primary entity hue (`var(--action-primary)`) and attach concise SVG callout annotations pointing directly to anomalies or inflection points.
5. **Structure Multi-Slide Decks via the 3-Act Narrative Arc:**
   - See [references/storytelling-arc.md](references/storytelling-arc.md): **Act 1 Context/Hook** → **Act 2 Complication/Driver** → **Act 3 Resolution/Decision**, keeping every slide `<= 30` words.
6. **Run Mandatory Honest-Data QA Before Handoff:**
   - Verify zero-baseline bar scales, `100%` share sums, explicit `—` for missing values (never fake `0`), tiny-base caveats (`n < 5`), and WCAG contrast across Light & Dark semantic CSS tokens.
   - See [references/chart-selection-matrix.md](references/chart-selection-matrix.md) and [references/html-canvas-contract.md](references/html-canvas-contract.md).
