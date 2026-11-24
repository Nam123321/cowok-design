---
name: data-viz-storyteller
version: 2.1.0
description: Designs insight-led charts, executive data stories, interactive narrative lenses, spotlight callouts, and honest HTML/SVG dashboards.
risk_tier: 1
---

# Data Visualization & Executive Storyteller (v2.1)

## Purpose
Transforms raw analytical metrics (CSV, TSV, JSON, PDF tables, or live telemetry) into trustworthy, interactive HTML5/SVG charts, dashboards, and narrative slide decks grounded in Cole Nussbaumer Knaflic (*Storytelling with Data*), Stephen Few, Edward Tufte, and the companion `dashboard-design` peer skill (`dashboard-design-upstream`).

## Minimum Floor, Never Maximum Ceiling
Every baseline below (`>= 3` AI Narrative Lenses, `>= 5` Core Interactive UX Patterns, `>= 5` Honest & Wordless Auto-Fix checks, `>= 3` narrative acts) is a **Minimum Floor**—never a closed ceiling. When the dataset reveals richer structure (cohorts, regimes, uncertainty intervals, multi-stage funnels, or multi-region splits), proactively synthesize deeper interactive patterns from `references/storytelling-arc.md` without forcing arbitrary truncation.

## Operating Protocol (3-Tier Progressive Disclosure)
1. **Load Visual Craft Baseline:** Pair with `dashboard-design` (`dashboard-design-upstream`) for calm visual hierarchy (*First glance 3s*, *Quiet*, *Wordless*, *Honest*, *One primary entity hue + neutral baseline*).
2. **Author a Data-Grounded Action Title (Never a Generic Label):**
   - Replace passive labels (*"<Metric> by <Dimension>"*) with a 1-line empirical conclusion derived dynamically from the user's dataset (*"<Leading Segment> contributed <X>% of total <Metric>, exceeding target by <Delta>"*).
   - Keep on-canvas prose wordless: encode numbers, units, and compact deltas visually; push deep rationale into tooltips, callout arrows, or interactive inspection drawers.
3. **Synthesize `>= 3` Instant Narrative Lenses (`AI Lens Strip`):**
   - Propose at least 3 distinct analytical lenses (e.g., **Driver / Variance**, **Target / Goal Attainment**, **Dynamics / Mix Shift**, plus domain-specific lenses such as Cohort Retention or Anomaly Detection) so the audience switches angles in 1 click without losing underlying `sourceRows`.
   - Load [references/storytelling-arc.md](references/storytelling-arc.md) for the JSON lens schema, all **5 Core Interactive UX Patterns**, and **8 Open-Ended Extensible Patterns**.
4. **Apply Pre-Attentive Spotlight, Contextual Point Actions & What-If Exploration:**
   - Render baseline series in muted neutral tokens (`var(--border-strong)` / `var(--text-muted)`).
   - Emphasize focal data points (`data-row-id`) with `var(--action-primary)`, attach SVG callout pointers (`data-ff-callout-for`), support 1-bar inspection + 2-bar delta comparison (`data-ff-compare-pair`), and preserve immutable `sourceRows` during What-If scenario drags (`data-whatif-handle`).
5. **Enforce Semantic Role HTML Canvas Contract & 5-Rule Honest Auto-Fix:**
   - Never hardcode demo identifiers (such as domain-specific revenue card IDs). Bind every canvas node using semantic roles (`data-ff-role`) from [references/html-canvas-contract.md](references/html-canvas-contract.md).
   - Select chart shapes by analytical question via [references/chart-selection-matrix.md](references/chart-selection-matrix.md), escalating gracefully when cardinality grows (`> 6` categories).
   - Run the 5-Rule Honest & Wordless QA gate (zero-baseline bar scale, `100%` proportional share sum, explicit `—` for missing values, small-sample `n < 5` caveat, and Action Title + Spotlight clarity) with 1-click Auto-Fix + Undo before handoff.
