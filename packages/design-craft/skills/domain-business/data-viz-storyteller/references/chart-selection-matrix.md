# Chart Selection Matrix (Aligned with `dashboard-design` Peer Skill)

Choose the chart shape by the **analytical question** and data cardinality, not aesthetics. All 10 core shapes below render offline in zero-CDN HTML5/SVG inside any conforming HTML workbench host, with graceful escalation when category cardinality exceeds compact thresholds.

| Intent Group | Chart Shape | Best Question Answered | Honest-Data Guardrail |
| :--- | :--- | :--- | :--- |
| **Comparison** | `bar` (Grouped Column / Horizontal Bar) | Which category leads vs prior period? | Always start numeric axis at `0`; pivot to Horizontal Ranked Bar when labels are long or `> 6` categories exist. |
| **Comparison** | `movement` (Movement Bars) | What grew (`+`) vs shrank (`-`) between periods? | Separate positive growth hue from contraction hue cleanly around explicit `0` baseline. |
| **Trend** | `line` (Line / Area) | How is the metric trending over ordered intervals? | Direct end-of-line label; avoid legend ping-pong; switch to Small Multiples when `> 4` overlapping series compete. |
| **Trend** | `sparkline` (Inline Sparkline Strip) | What is the micro-trajectory of every entity at a glance? | Shared Y scale across comparable rows or explicit min/max ticks. |
| **Composition** | `pie` (Donut / Pie) | What share does each part contribute? | Recommended for `<= 6` slices; assert sum `== 100%`; escalate cleanly when `> 6` categories (see below). |
| **Composition** | `segmented` (Segmented 100% Share Bar) | Compact part-to-whole bar for narrow cards & slides. | Sort descending; group long tail into an explicit `Other (n items)` segment when needed. |
| **Goal / KPI** | `bullet` (Bullet Graph) | Actual performance vs target marker & prior baseline? | Clear target tick line + qualitative threshold bands using semantic tokens. |
| **Goal / KPI** | `tick-bar` (Target Tick Bar) | Tactile segmented progress ticks toward a numeric goal. | Show exact `%` attainment and gap/surplus beside the tick strip. |
| **Goal / KPI** | `kpi` (KPI Strip Cards) | 3-second executive pulse across headline metrics. | `tabular-nums`, compact delta pill, `data-ff-role="kpi-card"`, 1 focal highlight. |
| **Matrix** | `heatmap` (Matrix Heatmap) | Where are hotspots/outliers across Category × Metric/Period? | Monotonic opacity ramp of 1 entity hue; exact value on cell/hover. |

## Graceful High-Cardinality Escalation (`> 6` Categories — Never Hard Refusal)
Do **not** fail or drop user data when a dataset contains more than 6 categories or series. Instead, escalate transparently to an honest high-cardinality encoding:
1. **Ranked Horizontal Bar (`bar` horizontal):** Sort descending by primary metric, highlight top focal rows via Pre-Attentive Spotlight, and keep every category legible without rotated X-axis text.
2. **Segmented 100% Share Bar (`segmented`) with Explicit Tail Grouping:** Retain the top `N` drivers individually and aggregate the long tail into an expandable `Other (<k> categories)` bucket that still sums to `100%`.
3. **Small Multiples (Faceted Grid):** When comparing trends across `> 6` cohorts, regions, or products, render aligned trellis mini-charts sharing identical axes so every entity remains inspectable without spaghetti clutter.
4. **Matrix Heatmap (`heatmap`) or Variance Waterfall:** For multi-dimensional breakdowns (`> 6` rows × multiple periods/metrics), encode intensity in a compact heatmap or decompose net variance in a waterfall bridge.
