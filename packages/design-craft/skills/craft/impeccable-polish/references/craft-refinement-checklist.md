# Impeccable Craft Refinement Checklist (`impeccable-polish`)

## 1. Three-Tier Visual Quality Gate
- **Tier 1 (Floor — Must Never Fail):** Zero broken contrast (`>= 4.5:1` body text, `>= 3.0:1` UI/large text), zero text clipping or horizontal overflow, zero raw hex overrides bypassing `tokens.css`, zero placeholder copy (`Lorem ipsum`, `TBD`, fake user names).
- **Tier 2 (Reflex — Anti-Slop Elimination):** Eliminate nested card-inside-card boxes where a single `1px solid var(--border-default)` divider suffices; remove decorative backdrop blur over noisy text; replace emojis in functional UI chrome with crisp monoline SVG icons (`1.85px` stroke).
- **Tier 3 (Finish — Optical Craft):**
  - **Concentric Radii:** Nested pill/input corners must satisfy `r_inner = max(2px, r_outer - gap)` so corners curve harmoniously.
  - **Optical Alignment:** Nudge icon+label baselines and circular play/chevron glyphs optically so visual center matches perceived center.
  - **Calm Motion:** Transition opacity/transform (`150ms cubic-bezier(0.2, 0, 0, 1)`); wrap non-essential animations inside `@media (prefers-reduced-motion: no-preference)`.

## 2. Critique-to-Token Mapping Examples
| Human Critique | Deterministic Craft Action |
| :--- | :--- |
| *"Feels busy / noisy"* (`quieter`) | Reduce card border opacity, drop secondary shadow elevation, collapse auxiliary metadata into hover/drawer |
| *"Weak hierarchy"* (`bolder`) | Increase headline weight/scale contrast, spotlight single primary CTA (`var(--action-primary)`), mute peer buttons to ghost/secondary |
| *"Looks generic"* (`distill` + `typeset`) | Replace boilerplate 3-column icon cards with structured asymmetric layout, domain-specific metrics, and tighter typographic rhythm |
