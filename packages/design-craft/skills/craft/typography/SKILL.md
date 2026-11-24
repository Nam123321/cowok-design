---
name: typography
version: 2.0.0
description: Defines modular type scales, tabular numerals, multilingual CJK/RTL readability, and accessible line measure.
risk_tier: 1
---

# Typography Craft

## Purpose
Establishes clear typographic hierarchy, effortless long-form readability, crisp numeric alignment, and multilingual resilience across Latin, Vietnamese diacritics, CJK, and RTL scripts.

## Minimum Floor, Never Maximum Ceiling
Enforce the baseline legibility floor below (coherent modular scale, `tabular-nums` for metrics/tables, comfortable reading measure, minimum `400` body weight on dark surfaces). Freely layer expressive display pairings, fluid `clamp()` sizing, and optical tracking refinements when the brand identity calls for stronger editorial voice.

## Operating Protocol
1. **Modular Type Scale:** Use a consistent ratio (such as `1.200` minor third for dense operational apps or `1.250` major third for editorial/presentation canvases) with unmistakable visual separation between `h1`, `h2`, `h3`, body, and caption tiers.
2. **Tabular Numerals & Monospace Discipline:** Apply `font-variant-numeric: tabular-nums` to all KPIs, chart axes, table columns, and timestamps; reserve monospace strictly for IDs, hashes, code, and exact paths.
3. **Readable Measure & Diacritic Safety:** Keep prose line length around `45–75ch` and body `line-height >= 1.5` (increasing slightly to `1.6` for Vietnamese stacked diacritics and CJK scripts so ascenders/descenders never collide).
4. **Dark-Surface Legibility:** Avoid ultra-thin weights (`< 400`) for body copy on dark or dim backgrounds; ensure contrast passes `>= 4.5:1` across all requested `themeModes`.
5. **Deep Reference:** Load [references/type-scale-and-i18n.md](references/type-scale-and-i18n.md) for modular ramps, fluid CSS tokens, and CJK/Vietnamese/RTL typographic tuning.
