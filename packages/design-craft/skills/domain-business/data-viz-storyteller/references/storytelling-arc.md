# Data Storytelling Arc, 5 Core UX Patterns & 8 Extensible Patterns

> **Autonomy Principle — Minimum Floor, Never Maximum Ceiling:**
> The **5 Core Interactive UX Patterns** and the **3-Act Executive Arc** below establish the mandatory quality floor for every generated chart, dashboard, or data-story deck. Agents are actively encouraged to combine, compose, and invent beyond this floor—drawing from the **8 Open-Ended Extensible Patterns** or domain-native visual idioms whenever the user's dataset warrants deeper analytical insight.

---

## 1. The 3-Act Executive Arc (Minimum Floor for Multi-Slide Data Decks)

Structure multi-slide presentations along a causal decision arc (add sub-acts or deep-dive appendix slides freely when complex datasets require it):

| Act | Narrative Role | Visual Pattern | Copy Guidance |
| :--- | :--- | :--- | :--- |
| **Act 1 · Context & Headline** | Anchor the audience in 3 seconds: what happened overall vs baseline or target? | **CEO 3s Lens**: 1 Action Title + top KPI strip (`data-ff-role="kpi-card"`) + target tick indicator | Crisp headline + numeric proof; zero filler prose |
| **Act 2 · Driver & Evidence** | Reveal *what* caused the variance or shift by spotlighting focal segments. | **Pre-Attentive Grey-Out Chart** + targeted SVG callouts (`data-ff-callout-for`) on inflection points | Annotate drivers directly on data; avoid legend ping-pong |
| **Act 3 · Scenario & Decision** | Demonstrate sensitivity (*What-If*) and frame the concrete decision requested. | **Baseline vs Simulated Scenario** (`data-whatif-handle` + target reference line) + explicit next-step CTA | Quantify upside/downside delta and decision owner |

---

## 2. Five Core Interactive UX Patterns (Mandatory Floor)

Every interactive HTML chart or dashboard produced by `data-viz-storyteller` MUST support these 5 foundational patterns:

### Core Pattern 1 · `AI Lens Strip` (`>= 3` Instant Narrative Angles)
Synthesize at least 3 switchable narrative lenses from the live dataset (e.g., `driver`, `target`, `dynamics`, plus optional domain lenses like `outlier` or `cohort`) emitted with a deterministic JSON descriptor so host workbenches can render 1-click lens chips:
```json
{
  "lenses": [
    {
      "id": "driver",
      "label": "Driver & Concentration",
      "recommendedChart": "bar",
      "actionTitle": "<Top segment> drove <share>% of total <metric> (<delta> vs prior)",
      "spotlightRowId": "<row-id>",
      "highlightMetrics": ["<metric-key>"],
      "calloutText": "<Concise empirical annotation>"
    },
    {
      "id": "target",
      "label": "Goal Attainment",
      "recommendedChart": "tick-bar",
      "actionTitle": "<k>/<n> segments met the <target> benchmark",
      "spotlightRowId": "<row-id>",
      "targetThreshold": 1000
    },
    {
      "id": "dynamics",
      "label": "Mix & Momentum Shift",
      "recommendedChart": "movement",
      "actionTitle": "<Fastest segment> shifted <delta>%, reshaping period mix",
      "spotlightRowId": "<row-id>"
    }
  ]
}
```

### Core Pattern 2 · Contextual Point Actions (1-Bar Inspection & 2-Bar Comparison)
- **Single Datum Focus (`1-bar` / `1-point`):** Clicking or focusing any element carrying `data-ff-role="data-datum"` and `data-row-id="<id>"` opens a lightweight contextual popover allowing 1-click **Spotlight Grey-Out**, **Pin SVG Callout** (`data-ff-callout-for="<id>"`), or **Ask AI to Explain** using exact tabular values and period delta.
- **Pairwise Comparison (`2-bar` / `Shift+Click`):** Selecting a second datum attaches `data-ff-compare-pair="<idA>:<idB>"`, rendering an inline bridge bracket showing exact absolute difference (`Δ`) and relative ratio (`%` / `×`) between the two points without leaving the canvas.

### Core Pattern 3 · 5-Rule Honest & Wordless 1-Click Auto-Fix + Undo
Expose a non-destructive QA indicator powered by the 5-Rule Honest & Wordless audit (detailed in `references/html-canvas-contract.md`):
1. Label-Title → Empirical Action Title promotion.
2. Zero-Baseline / Negative-Value Chart Guardrail.
3. Missing/Null Cell Explicit `—` Treatment (never fabricate `0`).
4. `100%` Proportional Sum Normalization & Small-Sample (`n < 5`) Caveat Badge.
5. Pre-Attentive Spotlight + Callout Clutter Reduction.
Applying Auto-Fix MUST push a reversible snapshot onto the host Undo stack (`Cmd+Z` / 1-click Revert) so users always stay in control.

### Core Pattern 4 · What-If Scenario Drag + Divergence Banner + Save Scenario Slide / Revert
- Attach draggable handles (`data-whatif-handle="<row-id>"`) to bar tops / data vertices and optional draggable target threshold lines so users can test sensitivity live.
- **Immutable `sourceRows` Invariant:** Modifying simulated values MUST NEVER overwrite the original imported `sourceRows`. As soon as simulated values diverge from `sourceRows`, render an explicit **Divergence Banner** displaying the delta (`Simulated vs Source`), a **Save Scenario Slide** action to snapshot the simulation into a deck slide, and a **1-Click Revert to Source** button.

### Core Pattern 5 · Audience Mode Toggle (`CEO 3s` ⇄ `Chi tiết`)
Expose `data-audience-mode="ceo-3s | detailed"` on the root container so viewers toggle cognitive density instantaneously without re-querying data:
- **`ceo-3s` (Executive Slide Mode):** Retains Action Title, headline KPI cards, primary Spotlight chart with top callouts, and hides auxiliary filters, tables, and dense footnotes.
- **`detailed` (Analyst / Dashboard Mode):** Reveals comparison ghosts, breakdown panels (`data-ff-role="breakdown-panel"`), provenance footnotes (`data-ff-role="provenance-footnote"`), QA status, and granular filters.

---

## 3. Eight Open-Ended Extensible Patterns (Go Beyond the Floor)

When the user's dataset or decision question calls for richer visual reasoning, compose any of these 8 extensible patterns cleanly using semantic CSS tokens and offline inline SVG:

1. **Small Multiples (Trellis / Faceted Panels):** Split high-cardinality dimensions (`> 6` series or multi-region trends) into aligned mini-charts sharing a locked Y-axis scale so trajectories compare effortlessly without spaghetti overlap.
2. **Event-Marker & Regime-Shift Bands:** Shade vertical time ranges or policy/release cutovers (`<rect>` translucent bands + crisp top labels) to connect operational events directly to metric inflections.
3. **Cross-Filtering & Linked Brushing:** Clicking a segment in a breakdown bar or heatmap dynamically filters or highlights matching `data-row-id` elements across every sibling chart panel on the canvas.
4. **Step-Through Scrollytelling Reveal:** Progressive narrative beats (Step 1: baseline → Step 2: isolate outlier → Step 3: overlay target/forecast) with keyboard `←`/`→` or scroll triggers for executive walkthroughs.
5. **Variance Bridge / Waterfall Decomposition:** Decompose net period change (`Start → +Expansion → +New → -Contraction → -Churn → End`) using floating bridge connectors and distinct semantic positive/negative fills.
6. **Distribution & Uncertainty Encoding:** Pair point estimates with confidence intervals, interquartile bands, or dot-strip/beeswarm overlays whenever sample variance or forecast error materially affects the decision.
7. **Slopegraph & Benchmark Reference Bands:** Connect before/after ranks across two periods (Slopegraph) or shade P25–P75 peer benchmark bands behind bars/lines to anchor performance against industry norms.
8. **Interactive Sensitivity Sliders (Multi-Factor What-If):** Complement direct vertex drags with synchronized parameter sliders (e.g., conversion rate, unit cost, volume growth) that live-recompute derived KPIs while preserving the `sourceRows` divergence banner and 1-click Revert.
