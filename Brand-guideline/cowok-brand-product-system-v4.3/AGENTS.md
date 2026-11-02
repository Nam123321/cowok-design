# AGENTS.md — cowok.ai Brand & Product System v3.0

## Read first
1. `SOURCE_OF_TRUTH.md`
2. `DESIGN.md`
3. relevant system manifest
4. relevant module documentation

## Locked brand decisions
- Logo: Option 3 Refined
- Wordmark: Customized Geist Option C
- Primary descriptor: Human + AI. One Workflow.
- Typography: Geist
- Iconography: Option 1 — Geometric Linear
- Themes: Warm Light, Focus Light, Night
- Low-chroma Focus: Night only

## Non-negotiable implementation rules

### Logo
- Never crop logo assets from visual boards.
- Never recreate the approved wordmark with generic Geist.
- Never replace the approved logo with a generated alternative.
- Never promote a guessed vector approximation as final.

### Color
- Never hard-code brand/theme colors inside reusable product components.
- Use semantic theme tokens.
- Agent Orange is not a generic warning or CTA.
- Human Blue is not a generic primary CTA.

### Themes
- Warm Light: no Low-chroma Focus.
- Focus Light: no Low-chroma Focus.
- Night: optional Low-chroma Focus.

### Icons
- Use canonical cowok icon library.
- SVG uses `currentColor`.
- 24×24, 1.75px rounded stroke.
- Do not mix Lucide / Material / Font Awesome / Phosphor directly into canonical UI.
- If an icon is missing, follow ICONOGRAPHY rules before creating a new one.

### Components
- Components consume semantic tokens.
- Do not invent local colors.
- Do not fill large Agent surfaces with orange.
- Review-required uses neutral surface + Agent accent edge/action.

### AI UX
- Draft ≠ Approved ≠ Executed.
- R2/R3 actions require Human review/confirmation according to policy.
- Show operational provenance.
- Do not expose private chain-of-thought.
- Agent UX should be operational, not theatrical/chat-first.

### Long-session
Prefer calm neutral surfaces.
Use color only when meaningful.
Avoid excessive saturation, motion and decorative AI effects.

## If context is incomplete
Do not guess.
Report missing asset/token/spec explicitly.

## New work
Before adding a new visual primitive:
1. search the current system;
2. reuse existing semantic pattern if possible;
3. create only if needed;
4. document it;
5. promote to canonical system only if reused in 2+ places.
