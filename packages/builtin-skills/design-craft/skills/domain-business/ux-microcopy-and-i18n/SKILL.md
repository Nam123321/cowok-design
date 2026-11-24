---
name: ux-microcopy-and-i18n
version: 2.0.0
description: Crafts plain-language UX microcopy, actionable error recovery, and i18n/RTL/CJK-resilient layouts.
risk_tier: 1
---

# UX Microcopy & Internationalization Craft

## Purpose
Ensures every button, empty state, validation prompt, tooltip, and slide headline speaks naturally in the user's domain language and adapts cleanly across global locales, script directions, and text expansion.

## Minimum Floor, Never Maximum Ceiling
Apply the clarity and localization floor below without enforcing artificial word-count ceilings that sacrifice clarity on complex warnings or compliance notices.

## Rules
- **Plain Domain Language (Zero Internal Jargon):** Never leak internal engineering, storage, protocol, or orchestration jargon into user-facing UI labels, buttons, or error banners; explain outcomes in the user's own vocabulary.
- **Clear, Actionable Recovery Copy:** Every error or empty state must explain (1) what happened, (2) why it matters, and (3) the concrete next action—concisely for simple alerts, and with structured guidance when high-stakes recovery requires nuance.
- **i18n, RTL & CJK Resilience (`>= 35%` Expansion Floor):** Budget at least `+35%` horizontal and vertical text expansion for verbose locales (German, Vietnamese, French), use logical CSS properties (`margin-inline`, `padding-inline`, `text-align: start`) for seamless RTL mirroring, preserve generous line-height for Vietnamese diacritics and CJK scripts, avoid string concatenation of translated fragments, and format dates/numbers/currencies via standard `Intl` APIs.
- **Zero Placeholders:** Reject `Lorem ipsum`, `TBD`, dummy names, or fabricated metrics in every reviewable artifact.
