---
name: phase-1-structural-sketch
version: 1.0.0
description: Builds monochrome wireframes, complete interaction states, and UX microcopy.
risk_tier: 1
---

# Phase 1 — Structural Sketch & Information Architecture

## Objective
Validate layout hierarchy, navigation flow, state completeness, and plain-language microcopy in monochrome before committing to visual brand styling.

## Operating Procedure
1. Select canonical screen or slide primitives from `catalogs/popular-components.fe.json` or `catalogs/popular-components.deck.json`.
2. Draft monochrome structural wireframes with semantic landmarks (`header`, `nav`, `main`, `aside`, `footer`) and stable `data-ff-id` anchors.
3. Run `de-scan-sketch` to detect missing interactive states (`loading`, `empty`, `error`, `permission-denied`) and incomplete forms.
4. Pair with `ux-microcopy-and-i18n` to verify all labels, CTAs, and error messages use clear domain language.
5. Suspend at Async HITL gate `GATE_1_SKETCH` until explicit human confirmation (unless `--auto-pilot` with valid Heritage Pack).
