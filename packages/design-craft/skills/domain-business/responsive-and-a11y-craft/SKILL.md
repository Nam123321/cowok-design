---
name: responsive-and-a11y-craft
version: 2.0.0
description: Enforces WCAG 2.1 AA+ accessibility, fluid responsive baseline viewports, and adaptive slide framing.
risk_tier: 1
---

# Responsive & Accessibility Craft

## Purpose
Guarantees layouts remain crisp, usable, and accessible across fluid web viewports and adaptive presentation frames while satisfying WCAG 2.1 AA (and AAA where feasible) across every user-requested `themeMode`.

## Minimum Floor, Never Maximum Ceiling
Test at minimum across the baseline viewport floor (**Desktop `>= 1440px`**, **Tablet `768px`**, **Mobile `390px`**, and adaptive presentation frames **`16:9` / `16:10` / `Auto`**). Freely verify additional breakpoints (ultra-wide `1920px+`, compact `320px`, landscape mobile, or split-pane embedded canvases) whenever the product context demands it.

## Requirements
- **Zero Unintended Overflow:** Prevent horizontal clipping/scrolling across web viewports and vertical overflow across adaptive slide frames (`qc-layout-overflow`).
- **Semantic Landmarks, Keyboard & Motion Safety:** Use native `<button>`, `<nav>`, `<main>`, logical heading order (`h1` -> `h2` -> `h3`), clear `:focus-visible` outlines, full keyboard reachability, `>= 44x44px` touch targets on mobile, and `@media (prefers-reduced-motion: reduce)` fallbacks.
- **Theme-Mode Agnostic Contrast:** Verify every user-requested `themeMode` passes `>= 4.5:1` for normal text and `>= 3.0:1` for large text and interactive UI controls (`qc-a11y-contrast`) using dynamic `tokens.css` CSS variables—never Tailwind `dark:` classes.
