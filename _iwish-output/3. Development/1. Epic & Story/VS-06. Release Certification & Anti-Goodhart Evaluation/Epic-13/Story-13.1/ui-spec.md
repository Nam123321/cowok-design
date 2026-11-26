---
type: I-Wish UI & Host Contract Spec
story_id: "13.1"
title: "UI Specification — Workbench M1..M5 Release Certification Drawer: Story 13.1"
status: ready-for-build
risk_tier: 3
has_ui: true
host_portal: workbenches
shell_mode: shelled
chromeless_reason: null
router_file: src/pages/workbench/WorkbenchShell.tsx
route_mount_parent: WorkbenchShell -> ContextualStageToolbar -> CertificationDrawer
navigation_entry: ContextualStageToolbar -> CertificationLauncherButton (data-testid="nav-certification-btn") -> CertificationDrawer
colocated_slice_dir: src/features/eval-certification
registration_mode: workbench_right_drawer
shell_e2e_selectors:
  - "[data-testid='nav-certification-btn']"
  - "[data-testid='certification-drawer']"
  - "[data-testid='eval-tier-selector']"
  - "[data-testid='eval-run-trigger-btn']"
  - "[data-testid='eval-milestone-pills']"
  - "[data-testid='eval-sim-cases-table']"
  - "[data-testid='eval-security-nonmasking-banner']"
  - "[data-testid='eval-history-timeline']"
links_to:
  - "../../../../DESIGN.md"
  - "./story.md"
  - "./data-spec.md"
  - "./impl-plan.md"
---

# UI Specification — Story 13.1: Workbench `M1..M5` Release Certification Drawer

## 1. Host Integration & Workbench Entry Point

- **Live Trigger Entry Point:** Mounted in `src/pages/workbench/components/ContextualStageToolbar.tsx` right action group beside `VisualQcTriggerButton` (`nav-visual-qc-btn`) and `TournamentLauncherButton` (`nav-tournament-btn`) across all stage tabs (`html`, `slide`, `md`, `code`).
  - Button `data-testid="nav-certification-btn"` with `<BrandIcon name="shield-check" size={11} strokeWidth={1.85} />` and localized non-tech label (e.g., `vi`: `"Chứng nhận Phát hành"`, `en`: `"Release Certification"`, `ja`: `"リリース認証"`).
  - Wired through `WorkbenchShell.tsx` (`isCertificationOpen` state + `Escape` close handler) -> `WorkbenchBodyLayout.tsx` -> `WorkbenchCanvasTabArea.tsx` -> `ContextualStageToolbar.tsx`.
- **Drawer Pattern Rationale (`[5.25]` vs `[5.26]`):** `CertificationDrawer` is an operational inspection & execution slide-over panel (matching `SupportRequestDrawer` and `RightDrawer [5.25]`, `width: 520px` desktop / `100vw` mobile, fixed backdrop overlay). `StudioDrawerShell [5.26]` is reserved for spatial visual editing canvases with Sub-Chat + marquee selection, so `CertificationDrawer` reuses `[5.25]` slide-over ergonomics + `[5.10]` `MetricLine` + `[5.21]` `DataTableView` + `[5.22]` `AdvancedFilterBar` + `[5.12]` `CowokToast`.

---

## 2. Registered UX Patterns & Components Alignment

| Story Widget / AC Feature | Matched Registry Pattern (`DESIGN.md` §5) | Physical Code Path / Component | Leverage Decision |
| :--- | :--- | :--- | :--- |
| `M1..M5` Certification Slide-Over Panel | `RightDrawer [5.25]` (`PAT-05`) | `src/features/eval-certification/CertificationDrawer.tsx` | REUSE_PATTERN (`520px` desktop / `100vw` mobile, semantic tokens, Escape & backdrop close) |
| KPI Telemetry Strip (Pass Ratio, Lift %, Fault Catch %, SLA) | `MetricLine [5.10]` | `src/features/eval-certification/CertificationMetricStrip.tsx` | REUSE_PATTERN (`tabular-nums`, `var(--font-mono)`, compact horizontal bar) |
| Milestone & Case Status Filter Strip | `AdvancedFilterBar [5.22]` | `src/features/eval-certification/SimCasesFilterBar.tsx` | REUSE_PATTERN (Milestone pills `All \| M1..M5`, Status filter, Security-only toggle, instant text search) |
| 25-Case `UC-01..UC-25` Checklist Matrix | `DataTableView [5.21]` | `src/features/eval-certification/SimCasesChecklistTable.tsx` | REUSE_PATTERN (sticky header, keyboard row focus, mandatory security lock badges) |
| Run History Ledger Timeline | `Run Timeline [PAT-03]` | `src/features/eval-certification/EvalHistoryTimeline.tsx` | REUSE_PATTERN (immutable `V33` + `gate-history.jsonl` chronological runs) |
| Action Receipts & Single-Flight `409` / Tamper `422` Notices | `Toast [5.12]` | `src/components/ui/CowokToast.tsx` | REUSE_COMPONENT (direct import of `<CowokToast />`) |
| Monoline `1.85px` Icons | `BrandIcon` Registry (`DESIGN.md` §4) | `src/components/ui/BrandIcon.tsx` | REUSE_COMPONENT (strictly registered keys: `shield-check`, `lock`, `play`, `history`, `clock`, `filter`, `layers`, `refresh`, `activity`, `check`, `x`) |

---

## 3. Component Hierarchy & Data Flow

```text
CertificationDrawer (`data-testid="certification-drawer"`, role="dialog", aria-modal="true")
├── Header Bar (`48px`, `var(--bg-elevated)`)
│   ├── <BrandIcon name="shield-check" /> + Title + Sealed Integrity Pill (`<BrandIcon name="lock" />`)
│   └── Refresh (`<BrandIcon name="refresh" />`) + Close (`<BrandIcon name="x" />`)
├── Alert Banners (Conditional, Non-Dismissable while active)
│   ├── Tamper Alert Banner (`data-testid="eval-tamper-banner"`) — shown on `SEALED_EVAL_TAMPER_ERROR`
│   └── Security Non-Masking Callout (`data-testid="eval-security-nonmasking-banner"`) — shown if `securityGatesPassed === false` even when `simCasesPassed >= 22`
├── Tier Selector & Run Bar (`data-testid="eval-tier-selector"`)
│   ├── 3 Radio/Segmented Tier Cards (localized non-tech copy):
│   │   ├── PR Quick Check (`pr`): "<15s · $0 · Ngoại tuyến" / "<15s · $0 · Offline"
│   │   ├── Stage Evaluation (`stage`): "Đối chứng 12+ kịch bản & SemVer"
│   │   └── Release Certification (`release`): "Chứng nhận đầy đủ 25 hành trình"
│   └── Primary CTA (`data-testid="eval-run-trigger-btn"`, disabled with spinner during flight; handles `409` gracefully)
├── CertificationMetricStrip (`MetricLine [5.10]`, `data-testid="eval-metric-strip"`)
│   ├── Simulated Users Pass (`25/25`, target `>=22/25`)
│   ├── Paired Ablation Lift (`+18.4%`, `with_skill` vs `baseline`, `0` regressions, `design-craft` SemVer badge)
│   ├── Fault-Injection Catch (`95.0%+`, `Builder != Scorer`)
│   └── Duration & Cost (`<15s · $0` formatted via `useFormatter()`)
├── Milestone Status Pills (`data-testid="eval-milestone-pills"`)
│   └── 5 clickable pills (`M1`..`M5`) with localized milestone names, `passed/total` count, and shield indicator
├── Main Section Tabs (`Checklist 25 Ca` | `Đối chứng & Hiệu chuẩn` | `Lịch sử Chạy`)
│   ├── Tab 1 — `SimCasesFilterBar` + `SimCasesChecklistTable` (`UC-01..UC-25` rows, mandatory security lock badge, evidence summary)
│   ├── Tab 2 — Paired Ablation (`>=12` briefs incl. Data Storytelling `data-viz-storyteller` + `dashboard-design`), 3-Provider Decision Calibration (`0` float leaks), Swarm `Builder != Scorer` catch metrics
│   └── Tab 3 — `EvalHistoryTimeline` (previous runs from `GET /api/v1/eval/history`, clickable to inspect historical run snapshots)
└── <CowokToast /> slot (single-flight `409` warning, run completed receipt, error notices)
```

---

## 4. Non-Tech UX Copy & 9-Locale i18n Contract

- **Zero Raw Code Leaks:** User-facing text MUST NEVER display raw backend enum strings (`SEALED_EVAL_TAMPER_ERROR`, `EVAL_ALREADY_RUNNING`, `compiled_binary`, `local_fts5`), raw story IDs (`Story 13.1`, `AC-01`, `V33`), or developer jargon (`chmod 444`, `MPSC`). Map every status/error code through `useTranslation()` to clear human language (e.g., `EVAL_ALREADY_RUNNING` -> `"Đang có một phiên kiểm định chạy song song. Vui lòng đợi hoàn tất."` / `"An evaluation run is already in progress. Please wait for it to finish."`; `SEALED_EVAL_TAMPER_ERROR` -> `"Phát hiện thay đổi trái phép trên bộ tiêu chuẩn niêm phong. Chứng nhận bị khóa an toàn."`).
- **100% 9-Locale Coverage:** Add complete `evalCertification.*` keys across all 9 files in `src/i18n/locales/` (`en.json`, `vi.json`, `ja.json`, `zh.json`, `ko.json`, `fr.json`, `es.json`, `pt.json`, `it.json`) and verify via `src/i18n/locales.test.ts`.
- **Locale-Aware Formatting:** All numbers, percentages, durations, currency (`$0`), and timestamps MUST format via `useFormatter({ locale })` (`formatNumber`, `formatDateTime`) rather than manual string interpolation.

---

## 5. Styling, Theme & Runtime Safety Rules

- **100% Semantic CSS Variables:** Use ONLY canonical variables (`--bg-surface`, `--bg-elevated`, `--bg-workspace`, `--bg-subtle`, `--bg-selected`, `--bg-overlay`, `--text-primary`, `--text-secondary`, `--text-muted`, `--border-default`, `--border-subtle`, `--action-primary`, `--action-primary-fg`, `--state-success`, `--state-warning`, `--state-error`, `--shadow-lg`, `--radius-xs`, `--radius-sm`, `--radius-pill`, `--font-sans`, `--font-mono`).
- **Strictly Forbidden:** Zero hardcoded hex/rgb/hsl color literals in `src/features/eval-certification/*`, zero Tailwind `dark:` classes, zero Node `Buffer.*` calls in browser modules (`R-BROWSER-NO-NODE-BUFFER-13`).
- **Theme & Responsive Parity:** Must render cleanly in Desktop `1440×900` Light (`warm`) & Dark (`night`) themes and Mobile `390×844` (full-width drawer, horizontally scrollable milestone pills, no clipped controls or horizontal page overflow).
- **Runtime Boundary Validation (`MG-05`):** Every `src/api/eval-certification.ts` response MUST call `EvalRunResponseSchema.parse()` / `EvalHistoryResponseSchema.parse()`.

---

## 6. Interaction Matrix (Button-to-Backend E2E)

| User Action | Precondition | Network Call | Visible UI Outcome |
| :--- | :--- | :--- | :--- |
| Click `nav-certification-btn` in Workbench toolbar | Workbench hydrated | `GET /api/v1/eval/history` | Drawer opens smoothly; displays sealed integrity badge, latest `M1..M5` pills, and run history |
| Select Tier (`pr` / `stage` / `release`) & click `eval-run-trigger-btn` | No active eval lock | `POST /api/v1/eval/run` | Button enters loading state -> receives `200 OK` -> updates Metric Strip, `M1..M5` pills, `UC-01..UC-25` table, History timeline, and shows success `<CowokToast />` |
| Trigger concurrent run while active | Lock held | `POST /api/v1/eval/run` -> `409` | Shows non-tech warning `<CowokToast />`; preserves existing drawer state |
| Sealed fixture tampered / writable / symlink | Tamper injected | `POST /api/v1/eval/run` -> `422` | Renders high-contrast `eval-tamper-banner` (`var(--state-error)`) explaining tamper lock; blocks PASS badge |
| Filter `UC-01..UC-25` by Milestone (`M1..M5`) or Security-Only toggle | Run data loaded | Client-side filter on validated DTO | Table narrows instantaneously; security rows display `<BrandIcon name="shield-check" />` non-maskable lock pill |
| Mandatory security gate fails with `>=22/25` passed | Simulated/real failure | `POST /api/v1/eval/run` -> `200` with `verdict: "FAIL"` | Renders `eval-security-nonmasking-banner` explicitly explaining why overall certification is blocked despite aggregate count |
