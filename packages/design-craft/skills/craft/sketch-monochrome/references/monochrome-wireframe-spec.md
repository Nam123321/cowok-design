# Monochrome Structural Wireframe Spec (`sketch-monochrome`)

## 1. Visual Restraint Rules
- **Hairline Hierarchy:** Differentiate container depth with `1px solid var(--border-subtle)` and `var(--border-strong)` rather than shadows or saturated fills.
- **Typographic Contrast Only:** Establish scan order through font size, weight (`600` headings vs `400` body), and generous whitespace (`>= 24px` section separation) before adding brand color in Phase 2.
- **Single Primary Action per Region:** Highlight only the primary CTA in high-contrast monochrome ink; demote secondary actions to hairline outline or ghost treatments.

## 2. Structural Verification Floor
- Every interactive route/screen declares:
  - Semantic landmark skeleton (`header`, `nav`, `main`, `aside` where applicable).
  - Baseline `>= 4` core states (`default`, `loading`, `empty`, `error`) + domain-specific states (`permission-denied`, `conflict`, `offline-queue`) when relevant.
  - Stable `data-ff-id` / `data-ff-role` hooks so downstream Phase 2/3 styling and DOM patches preserve structure cleanly.
