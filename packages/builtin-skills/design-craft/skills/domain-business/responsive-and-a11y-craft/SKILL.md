---
name: responsive-and-a11y-craft
version: 1.0.0
description: Enforces WCAG 2.1 AA accessibility, responsive breakpoints, and 16:9 slide bounds.
risk_tier: 1
---

# Responsive & Accessibility Craft

## Purpose
Guarantees layouts remain crisp across Desktop (`1440px`), Tablet (`768px`), Mobile (`390px`), and 16:9 Presentation viewports (`1280x720`) while satisfying WCAG 2.1 AA.

## Requirements
- **Zero Overflow:** Prevent horizontal scrolling on Mobile/Desktop and vertical clipping on 16:9 slides (`qc-layout-overflow`).
- **Semantic Landmarks & Focus:** Use native `<button>`, `<nav>`, `<main>`, visible `:focus-visible` outlines, logical heading hierarchy (`h1` -> `h2` -> `h3`), and reduced-motion media queries.
- **Theme-Mode Agnostic Contrast:** Verify every requested `themeMode` passes `>= 4.5:1` for normal text and `>= 3.0:1` for large text / UI controls (`qc-a11y-contrast`).
