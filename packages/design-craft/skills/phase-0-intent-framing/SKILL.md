---
name: phase-0-intent-framing
version: 1.0.0
description: Frames product intent, entry mode, output track, and custom theme modes.
risk_tier: 1
---

# Phase 0 — Intent Framing & Adaptive Entry Routing

## Objective
Classify the request into one of 5 adaptive entry modes (`GREENFIELD`, `ADD_FEATURE`, `REVAMP`, `MICRO_POLISH`, `DECK_FROM_APP`), lock `outputTrack` (`app-fe-be` or `presentation-16-9`), and capture exact user-requested `themeModes` without assuming Light/Dark.

## Operating Procedure
1. Invoke `de-phase-router` to classify entry mode, execution strategy, and pending HITL gates.
2. Invoke `de-intake` to normalize user brief, constraints, reference apps (or fallback archetype), and target domain blueprint from `catalogs/domain-blueprints.json`.
3. Pair with `business-requirements-distiller` to convert ambiguous goals into verifiable jobs-to-be-done, actors, and measurable success outcomes.
4. Persist normalized `cognitive-state.json` (schema v1.0).
