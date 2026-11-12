---
name: design-taste-and-ux-guard
version: 1.0.0
description: Enforces 50 OKLCH palettes, Hallmark 3-Tier gates, and 62 anti-slop rules
capabilities_required: ["triangle-linter", "design-tokens"]
risk_tier: 1
---

# Design Taste & UX Guard Operating Skill

## 1. Overview
The `design-taste-and-ux-guard` skill embeds Hallmark's 3-Tier Gate Architecture (Floor, Reflex, Finish) and 50 physical material themes (Dusk, Cobalt, Studio, Atelier, Ribbon) alongside 62 deterministic anti-slop rules.

## 2. Progressive Disclosure & Operation
- **Tier 1 (Floor Gate)**: Blocks ungrounded metrics, broken contrast, unclosed tags.
- **Tier 2 (Reflex Gate)**: Warns against generic AI cards, indigo gradients, emoji icons.
- **Tier 3 (Finish Gate)**: Suggests micro-typography and concentric border radii.
