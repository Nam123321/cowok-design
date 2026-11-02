# DESIGN.md — cowok.ai

Version: 3.0.0
Status: APPROVED SYSTEM

## Brand
cowok.ai is an AI-native Work Operating System / Human + Agent Workspace.

Primary descriptor:
**Human + AI. One Workflow.**

Central strategic idea:
**Human judgment. Agent execution. One workflow.**

## Product principle
AI is not a separate chat layer. Agents participate inside projects, workflows, knowledge, tools and operational execution.

## Logo
Approved direction:
**Option 3 Refined — Human + Agent**

Wordmark:
**Customized Geist Option C**

Do not:
- replace the approved symbol with later experiments;
- redraw the wordmark with generic Geist;
- crop presentation boards and reuse them as production assets;
- use a guessed SVG as a final master.

## Typography
Primary family:
**Geist**

Usage:
- Product UI
- Website
- Marketing
- Documents
- Presentations

Customized Geist Option C is reserved for the cowok.ai wordmark artwork.

Tagline:
**Human + AI. One Workflow.**

## Brand colors
- Blue: `#0057FF`
- Cyan: `#00D4FF`
- Orange: `#FF8A00`
- Navy: `#081E3A`
- Gray: `#667085`
- Light: `#F7FAFC`
- White: `#FFFFFF`

Brand colors are identity primitives.
Product UI uses semantic theme roles instead of raw brand values.

## Long-session themes

### Warm Light
Primary daytime theme.
No Low-chroma Focus.

### Focus Light
Dim / soft-light theme for long reading and late-day use.
No Low-chroma Focus.

### Night Workspace
Deep Warm Navy / Charcoal Navy.
Supports optional **Low-chroma Focus**.

Design principle:
**Warm in light. Calm in dark. Quiet by default. Color only when meaningful.**

## Semantic abstraction
Components use roles such as:
- `bg.canvas`
- `bg.workspace`
- `bg.surface`
- `bg.surfaceSubtle`
- `bg.surfaceRaised`
- `text.primary`
- `text.secondary`
- `text.muted`
- `text.disabled`
- `border.default`
- `border.strong`
- `border.focus`
- `human.accent`
- `human.soft`
- `agent.accent`
- `agent.soft`
- `action.primary`

Do not select local hex colors inside components.

## Human / Agent semantics
Human Blue and Agent Orange are semantic colors.

Human Blue is not a generic primary CTA.

Agent Orange is not:
- generic warning;
- generic CTA;
- decorative AI color.

Use Agent Orange only when Agent ownership/execution/review meaning is important.

## Iconography
Selected direction:
**Option 1 — Geometric Linear**

Canonical rules:
- 24×24
- 1.75px stroke
- rounded line caps and joins
- `currentColor`
- line-first
- no gradients
- no 3D
- no theme-specific SVG variants

Theme/component layer owns color.

## Components
Canonical families:
- Button
- Input
- Select
- Tabs
- Badge / Status
- Card
- Table
- Modal / Drawer
- Toast / Tooltip
- Human / Agent chips
- Review-required
- Agent activity
- Automation state

Components consume semantic roles.

## Product patterns
Canonical product patterns:
- Dashboard
- Project
- Workflow / Kanban
- Agent Workspace
- Knowledge / Wiki
- Review & Approval
- Automation
- Mobile Review

## AI UX
Canonical lifecycle:
`Request → Interpret → Draft → Review → Execute → Report`

Important:
**Draft ≠ Approved ≠ Executed**

Agent states:
- Idle
- Interpreting
- Working
- Draft ready
- Review required
- Executing
- Completed
- Failed
- Blocked

Risk model:
- R0 Informational
- R1 Low-impact reversible
- R2 Operational change
- R3 External / financial / bulk

Human authority increases with risk.

## Provenance
Show operational provenance when meaningful:
- source
- freshness
- tool/action
- project/workflow context
- Human owner/approver
- execution timestamp
- actual effect/result

Do not expose private chain-of-thought.

## Supervisor
Supervisor is a control environment, not another chat persona.

It checks:
- risk
- approval requirement
- tool permission
- source/data validity
- workflow stage
- execution result
- exception handling

Healthy supervisor behavior should stay quiet.

## Long-session UX
Hierarchy should primarily come from:
1. layout
2. typography
3. spacing
4. surface hierarchy
5. border/elevation
6. color last

Avoid:
- full-screen saturated color fields
- glowing Agent panels
- excessive notification color
- gradient-heavy workspace UI
- purple AI visual tropes
- constant movement

## Source hierarchy
When files disagree:
1. approved production master asset
2. locked token/manifest
3. DESIGN.md
4. specific system documentation
5. AGENTS.md
6. master HTML guideline
7. visual boards/reference images

## Extension rule
If a new visual/component/pattern is reused in 2+ product areas, promote it into the canonical system.
