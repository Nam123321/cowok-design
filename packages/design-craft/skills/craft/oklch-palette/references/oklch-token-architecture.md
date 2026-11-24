# OKLCH Perceptual Token Architecture (`oklch-palette`)

## 1. Canonical Semantic Variable Contract
Always resolve actual token names from the active project `tokens.css`. Standard baseline token set:
```css
[data-theme="<mode>"] {
  --bg-workspace: oklch(...);
  --bg-surface: oklch(...);
  --bg-elevated: oklch(...);
  --border-subtle: oklch(...);
  --border-default: oklch(...);
  --border-strong: oklch(...);
  --text-primary: oklch(...);
  --text-secondary: oklch(...);
  --text-muted: oklch(...);
  --action-primary: oklch(...);
  --state-success: oklch(...);
  --state-warning: oklch(...);
  --state-danger: oklch(...);
}
```

## 2. Perceptual Lightness Ladders
- **Light Polarity Modes** (e.g., `light`, `warm`, `paper`): Workspace `L = 0.96–0.98`, Surface `L = 0.99–1.00`, Elevated `L = 1.00`, Primary Text `L = 0.18–0.24`, Secondary Text `L = 0.40–0.48`.
- **Dark / Dim Polarity Modes** (e.g., `dark`, `dim`, `midnight`): Workspace `L = 0.14–0.18`, Surface `L = 0.18–0.22`, Elevated `L = 0.23–0.27`, Primary Text `L = 0.92–0.96`, Secondary Text `L = 0.72–0.78`.
- **Gamut Clamping & Status Hues:** Keep chroma within sRGB/Display-P3 safe bounds (`C <= 0.22` for primary actions, `C <= 0.18` for data series) so rendered hues never clip awkwardly across standard monitors.
- **Data-Viz Calm Rule:** Default to 1 focal entity hue (`--action-primary`) paired with neutral baseline greys; only expand to multi-hue categorical ramps when encoding genuinely distinct qualitative categories, separated by `>= 45°` hue steps and distinct lightness values for deuteranopia/protanopia clarity.
