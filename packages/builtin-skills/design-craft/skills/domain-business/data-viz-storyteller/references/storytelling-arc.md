# 3-Act Data Storytelling Arc & Audience Modes

## 1. The 3-Act Executive Arc (For Multi-Slide HTML Decks)
When turning a dataset into a presentation deck, never output disconnected chart dumps. Structure slides along a causal decision arc:

| Act | Narrative Role | Visual Pattern | Word Budget |
| :--- | :--- | :--- | :---: |
| **Act 1 · Context & Headline** | Anchor the audience in 3 seconds: what happened overall vs baseline/target? | **CEO 3-Second Lens**: 1 Action Title + 3 KPI tiles (with Target Tick Bar) + 1 headline takeaway | `<= 25` words |
| **Act 2 · Driver & Evidence** | Show *what* caused the outcome (spotlight the hero segment or bottleneck). | **Pre-attentive Grey-Out Chart** (Bar / Movement / Heatmap) + 1–2 SVG Callout Arrows on anomalies | `<= 30` words |
| **Act 3 · Scenario & Action** | Show *what happens next* (What-If sensitivity) and the concrete decision requested. | **Bullet Graph / What-If Comparison** (Baseline vs Simulated Scenario) + explicit next-step CTA | `<= 25` words |

## 2. Audience Density Modes
Every generated HTML dashboard MUST support toggling cleanly between two cognitive densities without regenerating data:
- **Executive / Slide Mode (`CEO 3s`)**: Keeps Action Title, top 3 KPIs, primary Spotlight chart with callouts, and hides auxiliary filters/tables/footnotes.
- **Analyst / Dashboard Mode (`Chi tiết`)**: Reveals timeframe picker, prior-period ghost comparison bars, Ranked Share-Bar breakdown, Honest-Data QA badge, and source footnote.
