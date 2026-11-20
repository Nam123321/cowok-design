---
schema: plugin-boundary-review/v1
epic_id: Epic-02
feature_group: "VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine"
applicability: CANDIDATE
delivery_strategy: KERNEL_AND_BUILTIN_SKILL_PACK
---

# Plugin Boundary Review — Epic 02 (`JIT Prompt Assembler, Hidden SSOT & cowok-design-opt 7 Craft Skills`)

## 1. Per-Component 3-Axis Decomposition
| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `task_horizon_and_context_slicer` | `02.1` | `KERNEL` | `KERNEL_CORE` | Prevents cross-portal leaks (`SuperAdmin` -> `Marketing`); must never be bypassed by external skills. |
| `kv_cache_static_prefix_compiler` | `02.1` | `KERNEL` | `KERNEL_CORE` | Locks `<=1,500` text token budget & `max_image_tokens_per_turn` determinism. |
| `app_shell_and_component_ssot` | `02.4` | `DOMAIN` | `KERNEL_CORE` | Owns SQLite `V16 component_patterns, app_shell_locks` & single lock owner (`oxc` reverse sync). |
| `cowok_design_opt_and_7_craft_skills` | `02.4` | `DOMAIN` | `BUILTIN_CELL` | Packaged inside `packages/builtin-skills` (`<=250 tok` spine + 7 Craft Skills 3 tầng), routed by `<15ms` 3-Zone `CachePreservingSkillRouter`. |

## 2. Boundary Guardrails
- `cowok-design-opt` và 7 Craft Skills dùng định dạng chuẩn `SKILL.md` nhưng đóng gói làm `BUILTIN_CELL` dưới `ADR-007`; các skill ngoài từ người dùng đi qua cổng kiểm duyệt `Story 08.5`.
