---
name: sketch-monochrome
version: 2.0.0
description: Draws monochrome structural wireframes focused on hierarchy, spatial rhythm, landmarks, and interaction states.
risk_tier: 1
---

# Sketch Monochrome Craft

## Purpose
Validates information architecture, reading hierarchy, user journeys, and state completeness in calm grayscale before any brand color or visual polish is introduced.

## Minimum Floor, Never Maximum Ceiling
Enforce the structural wireframe floor below (semantic HTML landmarks, hairline `1px` borders, explicit primary action focus, `>= 4` core UI states). Expand freely with domain-specific sub-states (permissions, partial failure, inline validation, multi-step drawers) whenever user flows require it.

## Operating Protocol
1. **Colorless Restraint:** Render layouts strictly with neutral semantic tokens (`var(--bg-workspace)`, `var(--bg-surface)`, `var(--border-subtle)`, `var(--text-primary)`, `var(--text-muted)`). Reserve a single high-contrast ink strictly for primary CTA or active focus.
2. **Real Domain Copy Over Placeholders:** Use realistic domain labels, concrete numbers, and authentic action verbs—never `Lorem ipsum` or empty grey boxes without semantic labels.
3. **Semantic Landmarks & Anchors:** Structure every screen using `<header>`, `<nav>`, `<main>`, `<aside>`, `<footer>` and stable semantic `data-ff-id` / `data-ff-role` hooks for Phase 3 DOM patching.
4. **State Coverage Floor:** Wire explicit `default`, `loading`, `empty`, and `error` (plus domain states like `permission-denied` or `offline`) before locking the sketch.
5. **Deep Reference:** Load [references/monochrome-wireframe-spec.md](references/monochrome-wireframe-spec.md) for wireframe anatomy, density rhythm, and review sign-off gates.
