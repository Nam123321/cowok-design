---
name: reference-design-contract
version: 1.0.0
description: Audits reference designs with Keep, Change, and Do-Not-Copy matrices
capabilities_required: ["design-tokens", "audit-matrix"]
risk_tier: 1
---

# Reference Design Contract Operating Skill

## 1. Overview
The `reference-design-contract` skill audits external UI inspirations (.fig, screenshots, live URLs) to extract tokens and structure without copying IP. It structures findings into a tri-column evidence table (`observed | provided | inferred`) and a strict `Keep | Change | Do-Not-Copy` matrix.

## 2. Progressive Disclosure & Operation
- **Phase 1: Evidence Capture**: Classifies facts into observed, provided, or inferred.
- **Phase 2: Tri-Partition Matrix**: Marks elements to Keep (core layout), Change (brand colors/typography), or Do-Not-Copy (trademarks/unique artwork).
- **Phase 3: Clean Extraction**: Emits sanitized design tokens and layout geometry.
