---
name: oklch-palette
version: 2.0.0
description: Generates perceptually uniform OKLCH color ramps and semantic CSS tokens for any user-requested theme modes.
risk_tier: 1
---

# OKLCH Palette Craft

## Purpose
Synthesizes perceptually uniform, WCAG-verified OKLCH color systems mapped cleanly into canonical semantic CSS variables (`tokens.css`) for the exact `themeModes` requested by the user—never assuming or injecting unrequested modes.

## Minimum Floor, Never Maximum Ceiling
Enforce the perceptual contrast and semantic token naming floor below across every requested `themeMode` (`warm`, `dim`, `light`, `dark`, `high-contrast`, or any custom brand mode). Freely derive domain-appropriate chromatic accents, data-visualization categorical ramps, and status scales beyond basic surface/text pairs.

## Operating Protocol
1. **Honor Exact User `themeModes`:** Generate `[data-theme="<mode>"]` blocks strictly for the user's chosen `themeModes`; never hardcode Light/Dark if the user requested custom modes (or pause and ask when unspecified).
2. **Perceptual Lightness Hierarchy:** Anchor surface elevation using predictable OKLCH `L` steps (`ΔL ≈ 0.02–0.04` between workspace, card surface, and elevated popover) while keeping neutral chroma low (`C <= 0.015`) for comfortable long-session reading.
3. **Canonical Token Names (`tokens.css` Alignment):** Emit semantic tokens matching the project contract (`--bg-workspace`, `--bg-surface`, `--bg-elevated`, `--border-subtle`, `--border-default`, `--border-strong`, `--text-primary`, `--text-secondary`, `--text-muted`, `--action-primary`, `--state-success`, `--state-warning`, `--state-danger`). Zero Tailwind `dark:` utilities.
4. **Verify Contrast Programmatically:** Assert `>= 4.5:1` for normal body text, `>= 3.0:1` for large text and interactive UI borders/icons, and distinct luminance/hue separation for color-blind safety (`qc-a11y-contrast`).
5. **Deep Reference:** Load [references/oklch-token-architecture.md](references/oklch-token-architecture.md) for lightness ladders, gamut safety, and categorical data ramps.
