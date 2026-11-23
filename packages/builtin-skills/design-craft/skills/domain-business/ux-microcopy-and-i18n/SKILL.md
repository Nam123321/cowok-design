---
name: ux-microcopy-and-i18n
version: 1.0.0
description: Crafts plain-language UX microcopy, error recovery text, and i18n-safe layouts.
risk_tier: 1
---

# UX Microcopy & Internationalization Craft

## Purpose
Ensures every button, empty state, validation message, and slide headline communicates clearly in the user's domain vocabulary and survives localization expansion.

## Rules
- **Zero Engine Jargon:** Never expose internal architecture terms (`BYOS`, `CRDT`, `SQLite WAL`, `SSOT`, `MCP`, `JSON-RPC`) in user-facing UI labels.
- **Actionable Errors:** State what happened, why, and the immediate recovery action in under 18 words.
- **i18n Resilience:** Budget +35% horizontal text expansion for German/Vietnamese/French labels; avoid concatenating translated fragments and format dates/currencies via `Intl` standards.
- **Zero Placeholders:** Reject `Lorem ipsum`, `TBD`, `John Doe`, or dummy metrics in every reviewable artifact.
