---
name: archify-diagram
version: 2.0.0
description: Renders clean system, flow, sequence, and topology SVG diagrams using semantic tokens and collision-free routing.
risk_tier: 1
---

# Archify Diagram Craft

## Purpose
Transforms architectural nodes, bounded subsystems, state machines, and directed data flows into crisp, offline-rendered SVG diagrams bound strictly to project semantic CSS variables.

## Minimum Floor, Never Maximum Ceiling
Enforce the baseline structural floor below (`>= 24px` lane gutters, directional flow readability, accessible `<title>`/`<desc>`, semantic tokens). Freely enrich complex distributed architectures with latency badges, trust boundaries, zoomable legend strips, or sequence swimlanes when the user's domain warrants deeper clarity.

## Operating Protocol
1. **Subsystem Grouping:** Partition nodes into semantic lanes or bounded trust zones (`<g data-lane="...">`) with consistent top-to-bottom or left-to-right flow direction.
2. **Collision-Free Geometry:** Maintain `>= 24px` lane spacing and `>= 16px` node padding; route orthogonal or smooth bezier edges with single-purpose labels and zero overlapping text boxes.
3. **Dynamic Semantic Tokens:** Bind fills, strokes, and text strictly to active `tokens.css` variables (`var(--bg-surface)`, `var(--border-subtle)`, `var(--border-strong)`, `var(--text-primary)`, `var(--action-primary)`). Zero raw hex colors.
4. **Accessible Vector Output:** Include `<title>`, `<desc>`, explicit `viewBox`, and crisp `1.5px–2px` stroke hierarchy so diagrams scale cleanly in both web portals and slides.
5. **Deep Reference:** Load [references/svg-topology-patterns.md](references/svg-topology-patterns.md) for lane layouts, arrowhead specs, trust-boundary framing, and sequence swimlanes.
