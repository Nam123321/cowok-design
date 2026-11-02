# cowok.ai Core Components System v1.0

## Dependencies
- Visual Foundations v1.1
- Iconography v2.0

## Principle
Components consume semantic roles; they never choose raw brand/theme hex values.

## Component families
- Button
- Input
- Select
- Tabs
- Badge / Status
- Card
- Table
- Modal
- Drawer
- Toast
- Tooltip
- Human / Agent chips
- Review-required
- Agent activity
- Automation state

## Human vs Agent
Human Blue and Agent Orange are semantic colors.

Do not use:
- Human blue as a generic primary CTA
- Agent orange as generic warning
- Agent orange as generic CTA
- large orange/blue surface fills

## Low-chroma
Only Night mode can remap Human/Agent/status accents into low-chroma variants.

Warm Light and Focus Light remain unchanged.

## Review-required
Use:
- neutral/subtle surface
- default border
- Agent accent edge/icon
- Agent action button only if the action is Agent-specific

Do not fill the full card orange/amber.

## Icon rule
Use Iconography v2.0 and `currentColor`.

## Extension
A component reused in 2+ areas should be promoted into the canonical component system.
