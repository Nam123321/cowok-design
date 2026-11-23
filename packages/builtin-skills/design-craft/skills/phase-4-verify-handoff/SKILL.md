---
name: phase-4-verify-handoff
version: 1.0.0
description: Runs 6 read-only QC tools, 8D user simulation, and final acceptance handoff.
risk_tier: 1
---

# Phase 4 — Independent Verification & Handoff

## Objective
Execute independent, read-only quality verification (`design-auditor`) against `rubrics/design-qc-rubric.json` before packaging final handoff specs.

## Operating Procedure
1. Run all 6 deterministic QC tools in read-only mode: `qc-a11y-contrast`, `qc-anti-slop-linter`, `qc-layout-overflow`, `qc-chart-and-data`, `qc-spec-parity`, and `qc-popular-coverage`.
2. Execute `user-simulation-8d` across diverse user personas, screen sizes, and accessibility constraints.
3. Fail closed if any hard gate fails (WCAG AA < 4.5:1 normal text, jargon/placeholder leakage, viewport overflow, or missing required spec items) and route structured defect tickets back to `design-orchestrator`.
4. Suspend at Async HITL gate `GATE_3_APPROVE_V1` with the complete QC scorecard and handoff checklist.
