---
name: phase-3-synthesis-polish
version: 1.0.0
description: Synthesizes polished screens or 16:9 decks using semantic tokens and DOM patching.
risk_tier: 1
---

# Phase 3 — Synthesis, Polish & Craft Execution

## Objective
Assemble production-grade application screens (`app-fe-be`) or executive 16:9 slide decks (`presentation-16-9`) using Phase 2 CSS variables, catalog components, and responsive accessibility craft.

## Operating Procedure
1. Bind every visual property to semantic CSS custom properties (`var(--surface-primary)`, `var(--text-primary)`, `var(--accent-primary)`) — zero raw hex leaks and zero Tailwind `dark:` utilities.
2. Pair with `data-viz-storyteller` for charts and `responsive-and-a11y-craft` for Desktop/Mobile/16:9 viewport resilience and keyboard focus order.
3. For targeted refinements (`MICRO_POLISH` or surgical fixes), use `de-patch-dom` on `data-ff-id` targets so untouched nodes stay byte-stable.
4. Keep every parallel writer confined to its assigned disjoint `write_scope`.
