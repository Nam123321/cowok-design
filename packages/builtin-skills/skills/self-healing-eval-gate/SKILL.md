---
name: self-healing-eval-gate
version: 1.0.0
description: Self-healing evaluation combining Hallmark Floor, Impeccable Audit, and Coverage
capabilities_required: ["triangle-linter", "eval-gate"]
risk_tier: 2
---

# Self-Healing Evaluation Gate Operating Skill

## 1. Overview
The `self-healing-eval-gate` skill orchestrates an automated 3-layer quality gate: (1) Hallmark Floor Gate for hard accessibility & responsive baselines, (2) Impeccable Audit for zero-slop craft compliance, and (3) Coverage Verifier ensuring complete state coverage.

## 2. Progressive Disclosure & Operation
- **Phase 1: Deterministic Lint**: Runs Triangle Linter in < 25ms.
- **Phase 2: Single-Pass Auto-Fix**: Automatically resolves fixable syntax and token errors.
- **Phase 3: Gate Receipt**: Emits immutable JSON gate receipt signed for CI release.
