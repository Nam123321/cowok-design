# SVG Topology, Swimlane & Sequence Patterns (`archify-diagram`)

## 1. Canonical Semantic SVG Primitives
- **Root `<svg>`:** Always emit explicit `viewBox="0 0 W H"`, `role="img"`, `<title>`, and `<desc>`. Never hardcode pixel dimensions that clip on narrow viewports (`width="100%" height="auto"`).
- **Subsystem Lane (`<g class="arch-lane">`):** `rx="12"`, `fill="var(--bg-workspace)"`, `stroke="var(--border-subtle)"`, `stroke-dasharray="4 4"` for trust boundaries, `>= 24px` outer lane margin.
- **Node Card (`<g class="arch-node">`):** `rx="8"`, `fill="var(--bg-surface)"`, `stroke="var(--border-strong)"`, `stroke-width="1.5"`; highlight focal entry/exit nodes with `stroke="var(--action-primary)"`.
- **Edge & Label:** Single directional arrow marker (`<marker>`) inheriting `var(--text-muted)` or `var(--action-primary)`; place edge labels inside a subtle `var(--bg-surface)` pill so lines never strike through text.

## 2. Diagram Archetypes (Minimum Floor + Extensible)
| Archetype | Ideal Use Case | Layout Rule |
| :--- | :--- | :--- |
| **Layered Architecture** | Client → BFF → Core Engine → Persistence | Horizontal tiers with `>= 32px` inter-tier gutter and explicit protocol labels |
| **Event / Pipeline DAG** | Ingestion → Transform → Validate → Emit | Left-to-right flow with branch/merge split markers and error/DLQ side channel |
| **Sequence Swimlane** | Auth handshakes, multi-actor protocols | Vertical actor lifelines, numbered causal steps, synchronous vs async dashed returns |
| **State Machine** | Entity lifecycles, approval gates | Distinct initial/terminal markers, guarded transition labels, zero dead-end states |
