---
schema: plugin-boundary-review/v1
epic_id: Epic-13
feature_group: "VS-06. Release Certification & Anti-Goodhart Evaluation"
applicability: NOT_APPLICABLE
delivery_strategy: SEALED_CORE_CERTIFICATION_HARNESS
---

# Plugin Boundary Review — Epic 13 (`13.1`)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `sealed_eval_runner_and_25case_e2e` | `13.1` | `KERNEL` | `KERNEL_CORE` | `tests/eval/sealed/` `chmod 444` + SHA-256 verification (`100% SEALED_EVAL_TAMPER_ERROR`), Paired `with_skill` vs `baseline` ablation, 3-provider calibration, Swarm fault injection (`>=95%`), and Automated 25-Case `/simulate-user` Playwright E2E (`ADR-007`). |

## Boundary Guardrails
- Bộ thước đo `tests/eval/sealed/` hoàn toàn bất biến (`chmod 444`, SHA-256 checked); tuyệt đối cấm bất kỳ plugin hoặc `advisor` LEARN rule nào tự chỉnh sửa nội dung thước đo.
