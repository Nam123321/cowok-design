---
name: reference-design-contract
version: 2.0.0
description: Audits UI references with observed/provided/inferred evidence and IP-safe Keep, Change, Do-Not-Copy matrices.
risk_tier: 1
---

# Reference Design Contract (Design-Only IP Boundary)

## Purpose
Extracts reusable layout geometry, interaction patterns, and perceptual token structure from external UI inspirations (screenshots, design files, live URLs via optional upstream REA, or offline reference archetypes) without infringing intellectual property or leaking proprietary business logic.

## Operating Protocol
1. **Tri-Column Evidence Classification:** Tag every finding strictly as `observed` (measured from artifact), `provided` (explicit user constraint), or `inferred` (synthesized recommendation)—never fabricate unverified metrics.
2. **IP-Safe Tri-Partition Matrix:** Partition every reference attribute into:
   - **Keep:** Structural layout rhythm, spatial hierarchy, interaction ergonomics, accessibility mechanics.
   - **Change:** Brand palette (re-synthesized via `oklch-palette`), typography pairing, domain vocabulary, and iconography.
   - **Do-Not-Copy:** Trademarks, logos, proprietary copy, bespoke artwork, and confidential backend logic.
3. **Clean Token & Contract Emission:** Output sanitized semantic CSS tokens aligned with `tokens.css` plus an offline-safe contract summary referencing `catalogs/reference-archetypes.json` when live inspection is unavailable.
