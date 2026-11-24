---
name: stageshow-slide
version: 2.0.0
description: Composes adaptive executive slides (16:9, 16:10, Auto) with action titles, miniature UI frames, and zero overflow.
risk_tier: 1
---

# StageShow Adaptive Presentation Craft

## Purpose
Composes crisp, executive-ready HTML5/SVG presentation decks and live miniature UI walkthroughs across adaptive viewport ratios (`16:9`, `16:10`, or `Auto` responsive framing) with clear visual hierarchy, causal storytelling, and zero clipping. Canonical owner absorbing legacy HTML presentation storytelling patterns.

## Minimum Floor, Never Maximum Ceiling
Enforce the baseline slide clarity floor below (adaptive framing, `>= 24px` outer safe margins, `>= 30%` breathing whitespace, action-oriented headline, keyboard navigation). Freely enrich complex narratives with multi-beat walkthrough animations, embedded live charts (`data-viz-storyteller`), device frames (`<Stage>`, `<Window>`, `<Phone>`), or speaker notes when the story benefits.

## Operating Protocol
1. **Adaptive Aspect Ratio (`16:9 | 16:10 | Auto`):** Never hardcode a single rigid pixel canvas; honor the deck's requested aspect ratio via CSS `aspect-ratio` and fluid container scaling with `>= 24px` outer safe margins.
2. **One Core Claim per Slide:** Lead with a concise Action Title backed by visual evidence (chart, diagram, or miniature UI frame in an asymmetric `60/40` split) instead of walls of bullet points.
3. **Live Miniature Product Frames:** When showcasing software flows, embed clean semantic UI frames (`Window`, `Phone`, `Stage`) with focal callouts or step-through highlights rather than blurry static screenshots.
4. **Overflow & Accessibility Guard:** Verify zero horizontal or vertical clipping (`qc-layout-overflow`), semantic CSS variables across requested `themeModes`, tabular numerals on metrics, and `ArrowLeft` / `ArrowRight` keyboard navigation.
5. **Deep Reference:** Load [references/adaptive-slide-patterns.md](references/adaptive-slide-patterns.md) for framing ratios, device frames, and causal deck choreography.
