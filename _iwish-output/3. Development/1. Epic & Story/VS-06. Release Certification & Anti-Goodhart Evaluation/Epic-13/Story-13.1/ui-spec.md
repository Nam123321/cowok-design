---
type: I-Wish UI & Host Contract Spec
story_id: "13.1"
title: "UI Specification — Workbench M1..M5 Certification Drawer & 25-Case Eval Matrix: Story 13.1"
status: refactored
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: ProjectReadinessModal / CertificationDrawer
navigation_entry: Release Certification Badge -> M1..M5 Certification Drawer
companion_surfaces:
  ai_chat_drawer: false
  command_palette: true
  notification_bell: false
shell_e2e_selectors:
  - "[data-ff-id='m1-m5-certification-drawer']"
  - "[data-ff-id='eval-tier-trigger-bar']"
  - "[data-ff-id='sim-25-cases-table']"
colocated_slice_dir: src/features/certification
registration_mode: workbench_right_drawer
components:
  - CertificationDrawerPanel [5.25]
  - EvalTierTriggerBar [5.10]
  - SimCasesTable [5.21]
  - SealedIntegrityCard [5.15]
links_to:
  - "../../../../2. Product Planning/2.3. ui-ux-spec.md"
  - "../../../../2. Product Planning/2.11. component-registry.md"
  - "./story.md"
  - "./data-spec.md"
---

# UI Specification — Story 13.1: `M1..M5` Certification Drawer & 25-Case `/simulate-user` Matrix

## Feature Hierarchy Nav Tree
- `UI/UX OS -> VS-06 -> Epic 13 -> Story 13.1 -> WorkbenchShell -> CertificationDrawerPanel`

## Host Integration Contract
- **`host_portal`:** `workbenches` (`CertificationDrawerPanel` inside `WorkbenchShell` + parity CLI `uiux-os eval run`).
- **`shell_mode`:** `shelled` (Displays immutable `V18c eval_benchmark_runs` & `.cowok-design/evals/gate-history.jsonl` telemetry, highlighting non-maskable `M1..M5` security gates).
- **`shell_e2e_selectors`:** `[data-ff-id='m1-m5-certification-drawer']`, `[data-ff-id='eval-tier-trigger-bar']`, `[data-ff-id='sim-25-cases-table']`.

## Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| `M1..M5` Release Gate Drawer | RightDrawer [5.25] | `src/features/certification/CertificationDrawerPanel.tsx` | NEW_DRAWER (`480px` right drawer) |
| Tier Runner (`pr \| stage \| release`) | Action Bar [5.10] | `src/features/certification/EvalTierTriggerBar.tsx` | REUSE (`409` single-flight guard) |
| `UC-01..UC-25` Scorecard | DataTableView [5.21] | `src/features/certification/SimCasesTable.tsx` | REUSE (`>=22/25` Pass, `0` Blocked, `0` Gap) |
| Sealed Manifest Hash Indicator | Card [5.15] | `src/features/certification/SealedIntegrityCard.tsx` | NEW_CARD (`chmod 444` + out-of-band hash status) |

## Component Hierarchy
```text
CertificationDrawerPanel (`m1-m5-certification-drawer`, width `480px`)
├── SealedIntegrityCard (verifies out-of-band root SHA-256 + `chmod 444`)
├── EvalTierTriggerBar (`eval-tier-trigger-bar`, triggers `pr <15s` | `stage` | `release`)
└── SimCasesTable (`sim-25-cases-table`, `UC-01..UC-25` rows + non-maskable `M1..M5` security locks)
```

## Design Tokens

| Token | CSS Variable | Value / Constraint | Usage |
| :--- | :--- | :--- | :--- |
| Tamper Error Banner | `--eval-tamper-bg` | `#7F1D1D` high-alert banner | `SEALED_EVAL_TAMPER_ERROR` display |
| PR Tier Budget Pill | `--eval-pr-sla` | `$0 / <15s` offline badge | `EvalTierTriggerBar` |
| Security Gate Icon | `--gate-non-maskable` | `1.85px` shield icon | `M1..M5` mandatory blocker rows |

## Interaction Patterns

| User / System Trigger | Precondition | State Transition / API Call | Verification Assertion |
| :--- | :--- | :--- | :--- |
| Click Run `--tier pr` | `COWOK_PR_TIER_OFFLINE=1` | `POST /api/v1/eval/run` (`409` if already running) | Completes `<15s` at `$0`, logs to `V18c` & `gate-history.jsonl` |
| Sealed file modified (`0644`/symlink) | Out-of-band SHA-256 mismatch | Aborts immediately with `SEALED_EVAL_TAMPER_ERROR` | Blocks Certification `PASS` regardless of scores |
| `>=22/25` but `M1` or `M3` failed | Mandatory security failure | Forces `verdict = FAIL` (`non-maskable`) | Prevents aggregate score masking critical gate |
