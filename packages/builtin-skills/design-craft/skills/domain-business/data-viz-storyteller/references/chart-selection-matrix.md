# Chart Selection Matrix (Aligned with `dashboard-design` Peer Skill)

Choose the chart shape by the **analytical question**, not aesthetics. All 10 shapes below render offline in zero-CDN HTML5/SVG inside any conforming HTML workbench host:

| Intent Group | Chart Shape | Best Question Answered | Honest-Data Guardrail |
| :--- | :--- | :--- | :--- |
| **Comparison** | `bar` (Grouped Column / Bar) | Which category leads vs prior period? | Always start numeric axis at `0`. |
| **Comparison** | `movement` (Movement Bars) | What grew (`+`) vs shrank (`-`) between periods? | Separate positive growth hue from contraction hue cleanly. |
| **Trend** | `line` (Line / Area) | How is the metric trending over ordered intervals? | Direct end-of-line label; avoid legend ping-pong. |
| **Trend** | `sparkline` (Inline Sparkline Strip) | What is the micro-trajectory of every entity at a glance? | Same Y scale across comparable rows or explicit min/max ticks. |
| **Composition** | `pie` (Donut / Pie) | What share does each part contribute? | Strictly `<= 6` slices; assert sum `== 100%`. |
| **Composition** | `segmented` (Segmented 100% Share Bar) | Compact part-to-whole bar for narrow cards & slides. | Sort descending; group long tail into `Other`. |
| **Goal / KPI** | `bullet` (Bullet Graph) | Actual performance vs target marker & prior baseline? | Clear target line + qualitative threshold bands. |
| **Goal / KPI** | `tick-bar` (Target Tick Bar · 2026) | Tactile segmented progress ticks toward a numeric goal. | Show exact `%` and gap/surplus beside the tick strip. |
| **Goal / KPI** | `kpi` (KPI Strip Cards) | 3-second executive pulse across headline metrics. | `tabular-nums`, compact delta pill, 1 selected focal tile. |
| **Matrix** | `heatmap` (Matrix Heatmap) | Where are hotspots/outliers across Category × Metric/Period? | Monotonic opacity ramp of 1 entity hue; exact value on hover/cell. |
