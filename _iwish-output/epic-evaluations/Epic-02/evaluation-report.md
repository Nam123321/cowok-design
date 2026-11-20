# Epic 02 Evaluation & Actionable Implementation Plan (`VS-01`)

**Verdict:** `READY` | **Envelope (`max_phase`):** `IMPLEMENT` | **Edge-Case Guardian:** `APPROVED`

## Plugin Decomposition Strategy
- **KERNEL (`Story 02.1`):** `TaskHorizonRouter`, `ContextSlicer` (`>=60%` SSOT/DOM, `<=350` tok outline), Static-Prefix/Dynamic-Tail Compiler (`<=1,500` text tok + `max_image_tokens_per_turn`) & `3-Profile Adapter`.
- **DOMAIN + BUILTIN_CELL (`Story 02.4`):** `V16` `component_patterns`, `app_shell_locks` (Single Lock Owner + `oxc` reverse sync), `cowok-design-opt` (`<=250` tok), 7 Craft Skills 3 tầng & `<15ms` 3-Zone `CachePreservingSkillRouter`.

## Actionable Implementation Plan
1. Hợp nhất `02.1 + 02.2 + 02.3` vào `Story 02.1` qua `/refactor-story`; deprecate (`cancelled`) `02.2` và `02.3`.
2. Tạo mới `Story 02.4` với đủ ACs/Tasks/FMEA Edge-Cases (`adr_ref: "ADR-007"`).
3. Kiểm định gate `0` budget overflows, `0` cross-portal leaks, `confusion_risk < 0.35`, `>= +25%` first-pass `lib-audit` lift.
