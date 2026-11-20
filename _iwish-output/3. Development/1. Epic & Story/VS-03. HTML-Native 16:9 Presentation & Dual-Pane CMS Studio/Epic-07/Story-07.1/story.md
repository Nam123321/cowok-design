---
type: I-Wish Story
id: "07.1"
title: "Story 07.1: Pooled 1-Chromium Rust CDP Visual QC (lib-audit + slides-check), QC Overlay & 1-Click Auto-Fix"
description: "Hợp nhất 07.1 + 07.2 + 09.3 occlusion thành lát cắt dọc Pooled <=1 Chromium Rust CDP (1440/1024/768/390 + 320..1440 sweep + 1920x1080 slides-check + elementsFromPoint occlusion + real RGBA SSIM thay drawWireframeCard) + iframe_bridge_fallback (UNVERIFIED_NO_BROWSER), Visual QC Overlay & 1-Click Auto-Fix -> V17 qc_gate_runs (SHA-1 cache)."
resource: "iwish://uxui-os/development/epic-07/story-07.1"
status: refactored
risk_tier: 3
delivery: core
adr_ref: "ADR-006"
timestamp: "2026-11-20T20:50:00+07:00"
tags:
  - "story-07.1"
  - "epic-07"
  - "pooled-cdp"
  - "visual-qc"
links_to:
  - "../../../../2. Product Planning/2.1. product-brief-or-prd.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.2. database-spec.md"
dependencies: ["05.4", "05.5", "06.2", "06.5"]
provides_apis:
  - "POST /api/v1/qc/run"
  - "POST /api/v1/qc/auto-fix"
consumes_apis:
  - "POST /api/v1/patch"
---

# Story 07.1: Pooled 1-Chromium Rust CDP Visual QC (`lib-audit` + `slides-check`), QC Overlay & 1-Click Auto-Fix

- **FR Covered:** [FR-18: Bộ kiểm định 25+ Quy tắc Bố cục (Pooled CDP + Real SSIM), E2E Flow Completeness & Visual QC Overlay Auto-Fix](../../../../2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 07: 17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Hợp nhất đầy đủ năng lực của Story 07.1 cũ, Story 07.2 (Visual QC Overlay & Auto-Fix) và phần kiểm định che khuất/occlusion từ Story 09.3 thành một Vertical Slice hoàn chỉnh: sử dụng **Pooled 1-Chromium CDP** cho cả `lib-audit` (App screens `1440/1024/768/390` + sweep `320..1440`) và `slides-check` (Slide 16:9 `1920x1080`), tính toán **Real RGBA SSIM** (`pixel/region occlusion`, contrast WCAG 2.2 AA, overflow, Concentric Radii, R-tree overlap, Footer-Rail `CONTENT_MAX_Y`), chạy E2E Flow Completeness Auditor, và hiển thị **Visual QC Overlay** với nút **1-Click Deterministic `lint_fix` (`<10ms`, `0` token)** kèm vòng Auto-Fix qua Agent (tối đa `MAX_ATTEMPTS = 3`), lưu vết kiểm định vào bảng **`V17a qc_gate_runs`** trong SQLite WAL.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn theo chủ trương không phân mảnh luồng dọc).
- **Formal-Verify Candidate (`risk_tier: 3`):** Pooled `<= 1` Chromium semaphore + ephemeral `IncognitoBrowserContext` lifecycle và composite `SHA-1` layout cache key khi chạy song song từ `Story 10.2`.

## 3. Acceptance Criteria (AC)
- **AC-01 (Pooled 1-Chromium CDP & 25+ Rule Geometry Solver + Real RGBA SSIM):** Dùng chung một tiến trình **Pooled 1-Chromium CDP** (`<= 1` Chromium instance, `<250MB` RSS) điều phối qua semaphore + `IncognitoBrowserContext` cô lập theo từng lượt chạy trong `crates/harness-daemon` cho cả `lib-audit` (các breakpoint `1440/1024/768/390` + sweep `320..1440`) và `slides-check` (`1920x1080`); kiểm định 25+ luật hình học/WCAG 2.2 AA (R-tree overlap, `scrollHeight > clientHeight`, Concentric Radii, `Footer-Rail CONTENT_MAX_Y`), thay thế hoàn toàn `drawWireframeCard` bằng **Real RGBA SSIM** & `elementsFromPoint` hit-testing với khóa cache composite `SHA-1(html + PROJECT_CONSTRAINTS + viewport + fixtureRef)`, và ghi kết quả bất biến vào bảng `V17a qc_gate_runs`.
- **AC-02 (E2E Flow Completeness Auditor):** Quét toàn bộ đồ thị màn hình & slide để bắt lỗi cấu trúc: (1) CTA chết chưa nối đích (`Unwired Dead-End CTAs`), (2) Màn hình mồ côi (`Orphan Screens`) không thể đi tới từ entry node, và (3) Thiếu 4 trạng thái dữ liệu (`Default / Loading / Empty / Error`), liên kết ngược về các node của `Story 05.4` và `Story 05.5`.
- **AC-03 ([EDGE-CASE] Resilience, `UNVERIFIED_NO_BROWSER` Fallback & Max 3 Auto-Fix Cap):** Khi môi trường không có Chromium khả dụng, kích hoạt `iframe_bridge_fallback` gắn nhãn trung thực `UNVERIFIED_NO_BROWSER` (không bao giờ giả PASS SSIM); tự động khởi động lại Pooled Chromium khi worker CDP crash hoặc OOM (circuit breaker cooldown) mà không làm treo daemon; mọi vòng Auto-Fix gọi Agent bị khóa cứng tối đa `MAX_ATTEMPTS = 3` kèm Diff Trước/Sau và hồi quy 1 bước (`Cmd+Z`) khi Agent làm hỏng ý đồ thiết kế; các thẻ `<iframe>`/`<canvas>` cross-origin bị chặn an toàn ở sandbox `sandbox="allow-scripts"`.
- **AC-04 (Visual QC Overlay, Project Readiness Dashboard & `<10ms` Deterministic `lint_fix`):** Đánh dấu tọa độ vi phạm trực tiếp trên canvas Preview/Slide, hiển thị Project Readiness Dashboard trước khi xuất bản; các lỗi token/spacing/radius xác định được sửa ngay qua `POST /api/v1/qc/auto-fix` (`lint_fix < 10ms`, `0` LLM token), chỉ các lỗi viết lại layout phức tạp mới đẩy sang vòng Agent + `POST /api/v1/patch`.

## 4. Tasks
- [ ] **Task 07.1.1**: Xây dựng `PooledCdpEvaluator` (1 Chromium pool dùng chung `lib-audit` & `slides-check`), bộ giải 25+ quy tắc hình học/WCAG 2.2 AA, Real RGBA SSIM & hit-testing, và ghi nhận bảng `V17 qc_gate_runs` (thỏa mãn `AC-01`, `AC-03`).
- [ ] **Task 07.1.2**: Cài đặt E2E Flow Completeness Auditor quét Dead-End CTAs, Orphan Screens và thiếu 4-SubStates trên toàn bộ đồ thị dự án (thỏa mãn `AC-02`).
- [ ] **Task 07.1.3**: Cài đặt `VisualQcOverlay` & `ProjectReadinessDashboard` UI, API `POST /api/v1/qc/run`, `POST /api/v1/qc/auto-fix` (`lint_fix <10ms` xác định) và giới hạn vòng Auto-Fix `MAX_ATTEMPTS = 3` kèm rollback (thỏa mãn `AC-03`, `AC-04`).
- [ ] **Task 07.1.4**: Viết bộ kiểm thử tích hợp & `[EDGE-CASE]` (CDP OOM recovery, slide 16:9 text overflow, real RGBA SSIM occlusion, deterministic `lint_fix` latency `<10ms`) (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 07.1.1`, `Task 07.1.4` | Pooled CDP + Real RGBA SSIM Integration Test (`cargo test`) |
| **AC-02** | `Task 07.1.2`, `Task 07.1.4` | Flow Completeness AST Graph Scan Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 07.1.1`, `Task 07.1.3`, `Task 07.1.4` | Crash Recovery & `MAX_ATTEMPTS=3` Boundary Regression Test |
| **AC-04** | `Task 07.1.3`, `Task 07.1.4` | Deterministic `lint_fix` (`<10ms`) & QC Overlay Playwright Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["05.4", "05.5", "06.2", "06.5"]

### Impacts
- Cung cấp kết quả `qc_gate_runs` sạch và bề mặt `VisualQcOverlay` làm cổng bắt buộc cho `Story 07.2` (Split Export), `Story 10.2` (`Builder != Scorer` QC), `Story 12.3` (CI/CD), và `Story 13.1` (Sealed Benchmark).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["05.4", "05.5", "06.2", "06.5"] (`POST /api/v1/patch`, HTML Sketch/Brand frames, 4-SubState Flow DAG, 16:9 Slide DOM & ChartSpecV12).

### Shared Entities
- Sở hữu ghi trên `qc_gate_runs` (`V17`); đọc `PROJECTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `deck_slides` trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Hợp nhất trọn vẹn kiểm định CDP + SSIM + E2E Flow + 1-Click Auto-Fix |
| **2. Data Integrity & State** | `9.4/10` | Lưu vết kiểm định vào `V17 qc_gate_runs` qua SQLite WAL |
| **3. Security & Validation** | `9.5/10` | Cô lập Pooled Chromium & sandbox frame |
| **4. Performance & Scalability** | `9.5/10` | Pooled 1-Chromium CDP + deterministic `lint_fix` `<10ms` (`0` token) |
| **5. Error Handling & Recovery** | `9.4/10` | Circuit breaker CDP crash + khóa cứng `MAX_ATTEMPTS = 3` |
| **6. Architectural Depth & Leverage** | `9.5/10` | Dùng chung engine cho cả Web App và 16:9 Slides |
| **7. UX Empathy** | `9.5/10` | Non-tech bấm 1-Click Fix ngay trên canvas không cần hiểu CSS |
| **TOTAL AVERAGE** | **9.47/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 07.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 E2E Flow Completeness Auditor (Dead-End CTA, Orphan Screen, Missing 4-SubStates — GAP-13) theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Hợp nhất 07.1 + 07.2 + 09.3 occlusion thành Vertical Slice hoàn chỉnh (Pooled 1-Chromium CDP, Real RGBA SSIM, V17 qc_gate_runs, QC Overlay & 1-Click Auto-Fix); cập nhật dependencies ["05.4", "06.2", "06.5"] theo ADR-006 | /refactor-story |
