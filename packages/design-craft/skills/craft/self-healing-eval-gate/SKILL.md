---
name: self-healing-eval-gate
version: 2.0.0
description: Runs 3-layer design evaluation (Hallmark Floor, Impeccable Audit, State Coverage) with bounded self-healing.
risk_tier: 2
---

# Self-Healing Evaluation Gate

## Purpose
Orchestrates an independent, deterministic 3-layer design quality gate prior to handoff, pairing read-only defect detection (`design-auditor`) with targeted, reversible remediation passes.

## Minimum Floor, Never Maximum Ceiling
Enforce all three baseline verification layers below as a non-negotiable floor; expand checks freely with domain-specific compliance audits (financial precision, clinical legibility, data honesty, RTL mirroring) whenever the project domain requires deeper verification.

## Operating Protocol
1. **Layer 1 — Hallmark Floor Gate:** Verify WCAG contrast (`>= 4.5:1` body / `>= 3.0:1` UI), zero viewport overflow across target frames, valid semantic HTML/SVG, and zero raw hex or hardcoded demo leaks.
2. **Layer 2 — Impeccable Anti-Slop Audit:** Flag visual clutter, placeholder strings, engine jargon, inconsistent spacing rhythm, and weak hierarchy.
3. **Layer 3 — State & Spec Parity Coverage:** Confirm every required screen, requested `themeMode`, and `>= 4` core + domain interaction states are present and navigable.
4. **Bounded Single-Pass Remediation & Receipt:** Route fixable defects back to the builder for surgical DOM/token patches (`de-patch-dom`) and emit a clean structured QC receipt before final human acceptance.
