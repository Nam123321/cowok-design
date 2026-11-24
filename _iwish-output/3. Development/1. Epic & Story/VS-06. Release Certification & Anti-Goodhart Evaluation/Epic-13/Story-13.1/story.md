---
type: I-Wish Story
id: "13.1"
title: "Story 13.1: End-to-End Sealed Anti-Goodhart Benchmark Runner & 25-Case /simulate-user M1..M5 Certification Suite"
description: "Xây dựng Workbench M1..M5 Certification Drawer + CLI uiux-os eval run --tier pr|stage|release <-> /api/v1/eval/* <-> Fixed-Ruler chmod 444 SHA-256 tests/eval/sealed/ Runner (Paired with_skill vs baseline ablation >=12 briefs, 3-provider Decision calibration, Swarm Builder != Scorer fault-injection >=95% catch, Automated 25-Case /simulate-user Playwright E2E Golden Suite UC-01..UC-25) -> V18 eval_benchmark_runs + .cowok-design/evals/gate-history.jsonl."
resource: "iwish://uxui-os/development/epic-13/story-13.1"
status: refactored
risk_tier: 3
delivery: core
adr_ref: "ADR-007, ADR-009"
timestamp: "2026-11-24T12:15:00+07:00"
tags:
  - "story-13.1"
  - "epic-13"
  - "anti-goodhart"
  - "sealed-eval"
  - "simulate-user-25"
  - "design-craft-semver-gate"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.2. database-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../../../../2. Product Planning/ADRs/ADR-009-Zero-Hardcode-AI-Harness-Spine-And-Data-Storytelling-Feedback-Loop.md]]"
  - "[[../epic.md]]"
dependencies: ["05.5", "07.1", "08.5", "08.7", "09.2", "10.3", "12.1", "12.3"]
provides_apis:
  - "POST /api/v1/eval/run"
  - "GET /api/v1/eval/history"
consumes_apis:
  - "POST /api/v1/qc/run"
  - "POST /api/v1/de/run"
  - "POST /api/v1/decision/route"
---

# Story 13.1: End-to-End Sealed Anti-Goodhart Benchmark Runner & 25-Case `/simulate-user` `M1..M5` Certification Suite

- **FR Covered:** [FR-36: Sealed Anti-Goodhart Benchmark & 25-Case /simulate-user Suite](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 13: End-to-End Sealed Anti-Goodhart Benchmark & 25-Case /simulate-user Certification Suite](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Khóa chuẩn nghiệm thu khách quan cho toàn bộ `cowok-design`: bộ benchmark `tests/eval/sealed/` được bảo vệ chống Goodhart (`chmod 444` + kiểm SHA-256), đánh giá chênh lệch thực tế `with_skill` vs `baseline`, đo lường năng lực bắt lỗi `Builder != Scorer`, hiệu chuẩn 3 nhà cung cấp Decision Cascade, và thực thi tự động 25 kịch bản `/simulate-user` (`UC-01..UC-25`) xuyên suốt 5 cột mốc `M1..M5`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=0.5, Unknowns=0.5 $\rightarrow$ Lát cắt dọc hoàn chỉnh: `M1..M5` Certification Drawer & CLI `uiux-os eval run` <-> `/api/v1/eval/*` <-> Sealed Runner <-> `V18c eval_benchmark_runs` + `gate-history.jsonl`).
- **Formal-Verify Candidate (`risk_tier: 3`):** Giao thức single-flight eval lock (`409 EVAL_ALREADY_RUNNING`), đồng bộ atomic JSONL + `V18c` MPSC writer, và kiểm chứng out-of-band root manifest hash fail-closed (`SEALED_EVAL_TAMPER_ERROR`).

## 3. Acceptance Criteria (AC)
- **AC-01 (Out-of-Band Pinned SHA-256 + `chmod 444` Sealed Eval Guard):** Khóa `chmod 444` cho toàn bộ thư mục `tests/eval/sealed/` khi khởi chạy CI/Runner và đối chiếu hash với **out-of-band pinned root manifest hash** (nhúng compile-time trong binary Rust / CI protected env `COWOK_SEALED_MANIFEST_SHA256` / Watchmen receipt — chống vượt qua qua `chmod u+w` + sửa manifest trong repo); sai lệch kích hoạt ngay **`100% SEALED_EVAL_TAMPER_ERROR`** fail-closed.
- **AC-02 (`$0 <15s` Air-Gapped PR CI Tier + Paired Ablation `>=12` Briefs & `design-craft` SemVer Release Gate — `ADR-009`):** `--tier pr` cưỡng chế `COWOK_PR_TIER_OFFLINE=1` (cấm tuyệt đối gọi LLM mạng ngoài hay boot Chromium mới) hoàn tất trong `<15s` với chi phí `$0`; `--tier stage|release` chạy đối chứng bắt cặp `with_skill` vs `baseline` trên tối thiểu `12` briefs (bắt buộc bao gồm bộ Data Storytelling briefs kiểm định cặp **`data-viz-storyteller v2.0` + `dashboard-design`**), 3-provider Decision calibration, và Swarm `Builder != Scorer` fault-injection đạt **`>=95%` catch rate**; đồng thời đóng vai trò **cổng kiểm định phát hành SemVer `v1.x` bắt buộc** cho mọi Reflective Skill PR do `advisor` (`10.3`) mở vào `design-craft` (chỉ cho phép merge/release khi `with_skill > baseline` và `0` hồi quy trên các brief hiện có).
- **AC-03 (Automated 25-Case `/simulate-user` Playwright Golden Suite):** Thực thi tự động 25 kịch bản người dùng (`UC-01..UC-25` phủ đủ `M1..M5` theo `Story 05.5` & container `12.3`), bắt buộc đạt **`>=22/25` Full Pass, `0` Blocked, `0` Gap**.
- **AC-04 (`M1..M5` Certification Drawer, Single-Flight Lock & `V18c` / `gate-history.jsonl`):** Hiển thị tiến độ & bằng chứng 25 ca trong Workbench `M1..M5` Certification Drawer; khóa chạy single-flight (`409 EVAL_ALREADY_RUNNING` khi gọi trùng) để ghi bất biến song song vào bảng `V18c eval_benchmark_runs` (qua MPSC actor `10.1`) và atomic append tệp `.cowok-design/evals/gate-history.jsonl`.
- **AC-05 ([EDGE-CASE] Security/Integrity Non-Masking & Sealed File Mutation Guard):** Từ chối ghi nhận `PASSED` nếu thiếu bất kỳ ca bảo mật/toàn vẹn bắt buộc nào across `M1..M5` (`M1` secret intercept, `M2` CSP/XSS & localStorage zero split-brain, `M3` RCE Tool Jail, `M4` `0` float leak, `M5` `0` offline 404) kể cả khi tổng điểm đạt `>=22/25`; phát `SEALED_EVAL_TAMPER_ERROR` ngay nếu phát hiện symlink hoặc quyền ghi writable trên `tests/eval/sealed/`.

## 4. Tasks
- [ ] **Task 13.1.1**: Thiết lập `tests/eval/sealed/` (`chmod 444` + out-of-band SHA-256 manifest verifier `SEALED_EVAL_TAMPER_ERROR`), migration slot `V18c eval_benchmark_runs`, `/api/v1/eval/*`, single-flight lock và `.cowok-design/evals/gate-history.jsonl` (thỏa mãn `AC-01`, `AC-04`, `AC-05`).
- [ ] **Task 13.1.2**: Triển khai CLI `cowok-design eval run --tier pr|stage|release` cùng Paired Ablation (`>=12` briefs bao gồm cặp Data Storytelling `data-viz-storyteller` + `dashboard-design`), cổng kiểm định `design-craft` SemVer PR release (`ADR-009`), 3-provider Decision calibration và Swarm `Builder != Scorer` fault injection (`>=95%`) (thỏa mãn `AC-02`).
- [ ] **Task 13.1.3**: Cài đặt Playwright E2E Golden Suite 25 ca `/simulate-user` (`UC-01..UC-25` trên `M1..M5`) và Workbench `M1..M5` Certification Drawer UI (thỏa mãn `AC-03`, `AC-04`).
- [ ] **Task 13.1.4**: Chạy kiểm chứng PR CI `<15s` (`$0`), tamper detection (`100% SEALED_EVAL_TAMPER_ERROR`) và báo cáo 25 ca E2E (thỏa mãn `AC-01..05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 13.1.1`, `Task 13.1.4` | Tamper Injection `SEALED_EVAL_TAMPER_ERROR` Test |
| **AC-02** | `Task 13.1.2`, `Task 13.1.4` | `<15s` PR CI, Paired Ablation & `>=95%` Fault Catch Run |
| **AC-03** | `Task 13.1.3`, `Task 13.1.4` | Playwright `UC-01..UC-25` (`>=22/25` Pass, 0 Blocked, 0 Gap) |
| **AC-04** | `Task 13.1.1`, `Task 13.1.3` | `V18` SQLite + `gate-history.jsonl` + Drawer UI Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 13.1.1`, `Task 13.1.4` | Mandatory M1/M3 Security Gate Non-Masking Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["05.5", "07.1", "08.5", "09.2", "10.3", "12.1", "12.3"]

### Impacts
- Cổng phát hành cuối cùng xác nhận toàn hệ thống `M1..M5`.

### Consumes
- Tiêu thụ 3-Tier Flow DAG (`05.5`), Pooled CDP QC (`07.1`), 7 DE Tools & Tool Jail (`08.5`), `DecisionCascadeClient` (`09.2`), Tournament & `advisor` (`10.3`), Standalone CLI `/api/*` (`12.1`), và OCI Container Runner (`12.3`).

### Shared Entities
- `eval_benchmark_runs` (`V18`), `qc_gate_runs` (`V17`), `gate_failure_ledger` (`V18`).

### Cross-Portal
- Bao phủ toàn bộ Portal UI (`SalesApp`, `SuperAdmin`, `MarketingWeb`), Presentation `16:9` và CLI/MCP.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.7/10` | Chứng nhận trọn 25 ca `/simulate-user`, paired ablation & calibration |
| **2. Data Integrity & State** | `9.6/10` | `V18` + `gate-history.jsonl` bất biến |
| **3. Security & Validation** | `9.8/10` | `chmod 444` SHA-256 tamper guard & non-masking security assertions |
| **4. Performance & Scalability** | `9.5/10` | `$0 <15s` `--tier pr` chạy liên tục không tốn token |
| **5. Error Handling & Recovery** | `9.5/10` | Fail-closed ngay khi sửa thước đo hoặc thiếu ca bảo mật |
| **6. Architectural Depth & Leverage** | `9.6/10` | Thước đo độc lập tuyệt đối với `LEARN` & Tournament (`ADR-007`) |
| **7. UX Empathy** | `9.5/10` | Drawer hiển thị rõ kết quả từng hành trình non-tech |
| **TOTAL AVERAGE** | **9.60/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0 | Khởi tạo Story 13.1 Sealed Anti-Goodhart Benchmark & 25-Case E2E Suite (`TDR-007`) | /make-story |
| 2026-11-24 | 1.1 | [ADR-009] Bổ sung cổng kiểm định SemVer `v1.x` cho Reflective Skill PRs của `design-craft` (`10.3`) & paired ablation coverage cho `data-viz-storyteller` + `dashboard-design` trong `AC-02` và `Task 13.1.2` | System Architect |
