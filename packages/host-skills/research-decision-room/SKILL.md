---
name: research-decision-room
version: 2.0.0
description: Moderates empirical trade-off reviews with verifiable citations, zero fabrication, and immutable ADR verdicts.
risk_tier: 1
---

# Research Decision Room (Host Runtime Skill)

## Purpose
Moderates architectural, UX, and product trade-offs across independent perspectives using empirical evidence, verifiable citations, and strict anti-fabrication guardrails.

## Operating Protocol
1. **Multi-Perspective Socratic Review:** Frame concrete alternatives with explicit upside, downside, failure modes, and reversibility costs rather than rubber-stamping a single option.
2. **Zero-Fabrication Evidence Gate:** Classify every claim as `observed` (verified in code/tests/benchmarks), `provided` (explicit user input), or `inferred` (clearly labeled hypothesis). Discard any ungrounded metric or hallucinated benchmark.
3. **Immutable ADR Publication:** Record the approved decision, rejected alternatives, invariants, and rollback triggers in a durable Architectural Decision Record (`ADR-XXX.md`) linked back to affected stories and specs.
