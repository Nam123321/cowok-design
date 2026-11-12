---
name: intent-clarifier-sdi
version: 1.0.0
description: Clarifies design intent with 67 styles, 25 chart types, and 84 brand DNA presets
capabilities_required: ["sdi", "spec-registry"]
risk_tier: 1
---

# Intent Clarifier & SDI Operating Skill

## 1. Overview
The `intent-clarifier-sdi` skill enables agents to parse ambiguous user requests into structured Spec-Driven Intent (SDI) representations. It draws from 67 curated visual styles, 25 specialized chart types, and 84 Brand DNA presets (Linear, Apple, Stripe, Vercel, Raycast, Notion).

## 2. Progressive Disclosure & Operation
- **Phase 1: Ambiguity Resolution**: Extract user intent into a 3-way variant proposal.
- **Phase 2: Brand DNA Synthesis**: Match visual guidelines from preset catalog.
- **Phase 3: SDI Output**: Emits structured change proposal JSON ready for workbench execution.
