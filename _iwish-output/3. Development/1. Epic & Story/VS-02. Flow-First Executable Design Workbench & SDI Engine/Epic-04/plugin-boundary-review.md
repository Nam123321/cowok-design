---
schema: plugin-boundary-review/v1
epic_id: Epic-04
feature_group: "VS-02. Flow-First Executable Design Workbench & SDI Engine"
applicability: CANDIDATE
delivery_strategy: HYBRID_KERNEL_AND_CLI_DOCTOR_EXTENSION
---

# Plugin Boundary Review — Epic 04 (`Story 04.1..04.6`)

## 1. Per-Component 3-Axis Decomposition
| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `dag_canvas_and_csp_preview` | `04.1..04.5` | `KERNEL` | `KERNEL_CORE` | Completed baseline: Native 2D DAG, SQLite `V7`, KeyedParallelCoordinator, `CompanionLiveHtmlRightPane`. |
| `cli_doctor_discovery` | `04.3` | `EXTENSION` | `EXTENSION_PLUGIN` | Completed baseline: Isolated `execFile` local CLI probe. |
| `chat_persistence_cas_and_vault` | `04.6` | `KERNEL` | `KERNEL_CORE` | Reuses `V4 chat_messages`, CAS `.cowok-design/cas/<sha256>`, `viewBox` auto-inference, and 3-Layer Secret Interceptor + `0o600`/Keychain Vault (`ADR-006`). |

## 2. Boundary Guardrails
- Toàn bộ API key từ khung chat (`Gemini` / `TypeSafe JEV` / BYOK) phải qua `redact_secrets` trước khi ghi log/SSE hoặc lưu `V4 chat_messages`.
