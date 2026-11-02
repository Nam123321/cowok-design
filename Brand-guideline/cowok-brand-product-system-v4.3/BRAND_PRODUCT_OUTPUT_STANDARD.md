# BRAND_PRODUCT_OUTPUT_STANDARD.md
## Lean Brand & Product System Deliverable Standard

This standard is derived from the useful production structure in the supplied Distro brand-product-system template, with duplicate and non-production files removed.

## Required root files

### `VERSION.json`
One authoritative version record.

### `PACKAGE_MANIFEST.json`
Machine-readable inventory, module versions and production status.

### `README.md`
Human quick-start only. Do not duplicate module specifications here.

### `DESIGN.md`
Global design rules and locked decisions.

### `AGENTS.md`
Instructions for Codex / Antigravity / coding agents.

### `brand-guideline.html`
Single human-readable interactive overview.

---

# Required modules

## 1. `/brand`
Keep:
- `BRAND_GUIDELINE.md`

Do not create extra brand README files unless the module is distributed separately.

## 2. `/foundations`
Keep:
- `tokens/tokens.json`
- `css/tokens.css`
- `typescript/tokens.ts`
- one reference HTML

The single tokens object should contain:
- brand primitives
- theme aliases
- typography
- spacing
- radius
- shadows
- density/layout if product needs them

Split token JSON only when independent package consumption requires it.

## 3. `/logo`
Keep:
- `LOGO_GUIDELINE.md`
- `ASSET_MANIFEST.json`
- actual production assets
- one approved reference board
- one HTML preview

Required logo outputs:
- horizontal light
- horizontal dark
- horizontal no-tagline
- stacked light
- stacked dark
- stacked no-tagline
- symbol-only
- wordmark-only when approved
- app icon light/dark
- favicon 16/32
- mono black
- mono white/reverse
- high-resolution raster print exports
- true vector masters **only after exact geometry is approved**

Never create a fake/auto-traced vector merely to satisfy a deliverables checklist.

## 4. `/iconography`
Keep:
- `ICONOGRAPHY.md`
- `ICON_PROMPT.md`
- `icon-manifest.json`
- `/svg`
- `/react`
- one reference HTML

The prompt is mandatory so an implementation agent can safely extend the icon system.

## 5. `/components`
Keep:
- `COMPONENTS.md`
- `component-manifest.json`
- CSS implementation reference
- one HTML review

## 6. `/application-patterns`
Keep:
- `PRODUCT_PATTERNS.md`
- `pattern-manifest.json`
- one HTML review

## 7. `/ai-ux`
Keep:
- `AI_UX_SYSTEM.md`
- `ai-ux-manifest.json`
- one HTML review

## 8. `/qa`
Keep:
- `FINAL_QA_CHECKLIST.md`

---

# Files intentionally excluded

The following should not ship in the canonical release package:

- `__MACOSX/`
- `.DS_Store`
- module-level `README.md` duplicates
- module-level `AGENTS.md` duplicates when root AGENTS covers the same rules
- duplicate asset manifests at parent and child levels
- legacy guideline HTML inside the active package
- exploratory logo boards once a single approved reference exists
- generated screenshots that duplicate interactive HTML
- deprecated assets mixed into active production folders
- auto-traced or approximate vectors marked as final masters

Historical files may live in a separate archive, not the canonical system package.

---

# Production status labeling

Every production-sensitive module must declare one of:

- `APPROVED`
- `APPROVED VISUAL / VECTOR MASTER PENDING`
- `REVIEW CANDIDATE`
- `DEPRECATED`

Agents must not infer approval from filename.

---

# Logo production quality standard

For a logo to be considered fully production-complete:

1. exact vector symbol geometry approved;
2. exact vector wordmark artwork approved;
3. horizontal/stacked/symbol/mono vector exports;
4. RGB digital master;
5. CMYK/spot-color production definition when print work begins;
6. black/white one-color test;
7. minimum-size test;
8. embroidery / engraving simplification when required;
9. app-icon safe-area test;
10. legal/trademark master derived from approved vectors.

Until items 1–2 are complete, high-resolution raster files are useful production derivatives but are not the legal/vector master.
