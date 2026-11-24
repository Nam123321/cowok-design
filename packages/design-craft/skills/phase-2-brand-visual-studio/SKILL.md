---
name: phase-2-brand-visual-studio
version: 1.0.0
description: Generates OKLCH tokens for requested theme modes and reference contracts.
risk_tier: 1
---

# Phase 2 — Brand & Visual Studio

## Objective
Synthesize perceptual OKLCH color ramps, typography scales, spacing tokens, and Design-Only reference contracts exclusively for the user-requested `themeModes`.

## Operating Procedure
1. Invoke `de-extract-contract` on any supplied reference app/URL (via optional upstream REA when available) or fall back to `catalogs/reference-archetypes.json` with a strict `Keep | Change | Do-Not-Copy` matrix.
2. Invoke `de-tokens-oklch` passing the exact `themeModes` list requested by the user (e.g. `["warm", "dim"]` or `["editorial-sand"]`); never inject unrequested Light/Dark modes.
3. Invoke `de-brand-preview` to produce an interactive HTML brand showcase covering only the requested `themeModes`.
4. Suspend at Async HITL gate `GATE_2_BRAND` until explicit human approval.
