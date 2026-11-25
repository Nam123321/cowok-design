---
type: I-Wish Story
id: "07.2"
title: "Story 07.2: Non-Tech Share & Present Export vs Developer Handoff (Offline HTML, Vector PDF & Plugin PPTX)"
description: "Kế thừa phạm vi xuất bản từ Story 07.3 cũ: chia đôi ExportBundleModal & ExportActionDropdown thành tab Share & Present (mặc định cho non-tech) vs Developer Handoff <-> /api/v1/export/* <-> 1-file offline .html bundler (crates/html-forge, 0 external 404 via file://) + CDP Page.printToPDF Vector 1920x1080 .pdf + plugin-isolated .pptx (TDR-006)."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/Story-07.2/story.md"
status: completed
risk_tier: 2
delivery: hybrid
adr_ref: "ADR-006"
components:
  - id: "offline-html-pdf-exporter"
    packaging: "KERNEL_CORE"
  - id: "pptx-deck-exporter-plugin"
    packaging: "EXTENSION_PLUGIN"
timestamp: "2026-11-20T20:50:00+07:00"
tags:
  - "story-07.2"
  - "epic-07"
  - "split-export"
  - "offline-html"
  - "vector-pdf"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["06.1", "06.5", "07.1"]
provides_apis:
  - "POST /api/v1/export/html-bundle"
  - "POST /api/v1/export/vector-pdf"
  - "POST /api/v1/export/pptx"
consumes_apis:
  - "POST /api/v1/qc/run"
---

# Story 07.2: Split Export & Packaging Engine (Share & Present vs Developer Handoff — Offline HTML, Vector PDF & Plugin-Isolated PPTX)

- **FR Covered:** [FR-19: Xuất bản Tách luồng Share & Present vs Developer Handoff — Self-Contained Offline HTML, Vector PDF & Plugin PPTX](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 07: 17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `2`

## 1. Story Description & Context
Kế thừa và mở rộng toàn bộ phạm vi đóng gói xuất bản từ Story 07.3 cũ thành Vertical Slice **Split Export**: tách rõ luồng **`Share & Present`** dành cho người dùng non-tech (1-file HTML offline tự chứa 100% font/SVG/chart từ `crates/html-forge` dùng `lol_html`, Vector PDF 16:9 chuẩn in ấn, gói PPTX tùy chọn qua plugin cô lập không bắt buộc LibreOffice trong core binary) và luồng **`Developer Handoff`** (xuất `SPEC.md`, `DESIGN_TOKENS.css/.json`, React TSX/HTML Clean Bundle & Playwright spec), bị khóa cứng sau cổng kiểm định chất lượng của `Story 07.1` trừ khi có xác nhận override tường minh từ người dùng.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Self-Contained HTML Bundle & Vector PDF via Shared `crates/html-forge`):** Sử dụng nhân `lol_html` dùng chung trong `crates/html-forge` (ADR-006) để đóng gói trọn vẹn dự án/bộ slide 16:9 thành **1 file `.html` offline duy nhất** (inline CSS/JS/font subset/SVG/ChartSpecV12) và xuất **Vector PDF** sắc nét qua `POST /api/v1/export/html-bundle` và `POST /api/v1/export/vector-pdf`.
- **AC-02 (Split Export UX — `Share & Present` vs `Developer Handoff` & Plugin-Isolated PPTX):** Trên giao diện Export Drawer, tách 2 tab rõ ràng bằng ngôn ngữ thân thiện: **`Share & Present`** (1-Click Offline HTML, Vector PDF, optional `.pptx` qua `POST /api/v1/export/pptx` được phục vụ bởi Extension Plugin cô lập — khi plugin chưa cài thì hiển thị hướng dẫn cài không gây lỗi core binary) và **`Developer Handoff`** (React/Tailwind TSX, `DESIGN.md` tokens, JSON/API contracts, E2E Flow DAG).
- **AC-03 ([EDGE-CASE] Resilience, Offline Zero-Leak & Pre-Export QC Gate Enforcement):** Bảo đảm file `.html` xuất ra có **0 external CDN requests** (hoạt động 100% khi tắt mạng, vượt kiểm định CSP `default-src 'self' data:`), chặn hoặc yêu cầu xác nhận tường minh (`override_qc_warning`) nếu `Story 07.1` còn lỗi Severity P0, và kiểm tra kích thước font/asset giới hạn (`<= 25MB`) trước khi ghi đĩa để chống cạn bộ nhớ.
- **AC-04 (Interactive Presenter Parity & Cross-Slide Asset Integrity):** Bản HTML offline giữ nguyên tương tác bàn phím (`ArrowLeft/Right`), chế độ Presenter 16:9 (`stageshow`), và tự động kiểm tra toàn bộ `<svg viewBox>` + CAS image blobs không bị gãy liên kết sau khi nhúng Base64/data-URI.

## 4. Tasks
- [ ] **Task 07.2.1**: Cài đặt trình đóng gói `lol_html` Offline Self-Contained HTML Bundle và Vector PDF Exporter trong `crates/html-forge` nối qua `POST /api/v1/export/html-bundle` & `/api/v1/export/vector-pdf` (thỏa mãn `AC-01`, `AC-04`).
- [ ] **Task 07.2.2**: Xây dựng giao diện Export Modal tách 2 luồng `Share & Present` và `Developer Handoff` cùng cổng `POST /api/v1/export/pptx` định tuyến sang Extension Plugin cô lập (thỏa mãn `AC-02`).
- [ ] **Task 07.2.3**: Cài đặt Pre-Export QC Gate kiểm tra trạng thái `qc_gate_runs` từ `Story 07.1` và cơ chế xác thực 0 external CDN / asset size bound `[EDGE-CASE]` (thỏa mãn `AC-03`).
- [ ] **Task 07.2.4**: Viết kiểm thử tự động xác nhận offline HTML mở được trong Playwright với mạng tắt hoàn toàn, SVG/Chart còn nguyên vẹn và PPTX fallback khi chưa cài plugin (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 07.2.1`, `Task 07.2.4` | Offline HTML & Vector PDF Export Test (`cargo test`) |
| **AC-02** | `Task 07.2.2`, `Task 07.2.4` | Split Export UI & Plugin PPTX Fallback Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 07.2.3`, `Task 07.2.4` | Zero-CDN & QC Gate Blocking Regression Test |
| **AC-04** | `Task 07.2.1`, `Task 07.2.4` | Presenter Keyboard & SVG Asset Integrity Playwright Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["06.1", "06.5", "07.1"]

### Impacts
- Cung cấp gói xuất bản cuối cùng cho `Story 12.1` (CLI headless export) và nghiệm thu `Story 13.1` (`M5`).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["06.1", "06.5", "07.1"] (`POST /api/v1/qc/run`, Deck slides + ChartSpecV12, `lol_html` token injection).

### Shared Entities
- Đọc `PROJECTS`, `SPEC_DOCUMENTS`, `PRESENTATIONS`, `deck_slides`, `cas_blobs`, `qc_gate_runs` trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ trọn Offline HTML, Vector PDF, Developer Handoff & Plugin PPTX |
| **2. Data Integrity & State** | `9.4/10` | Bảo toàn 100% asset CAS/SVG và token khi nhúng offline |
| **3. Security & Validation** | `9.5/10` | 0 external CDN request, plugin PPTX cô lập khỏi core binary |
| **4. Performance & Scalability** | `9.5/10` | Streaming `lol_html` dùng chung trong `crates/html-forge` |
| **5. Error Handling & Recovery** | `9.4/10` | Khóa xuất khi có lỗi QC P0 và thông báo rõ khi plugin PPTX chưa cài |
| **6. Architectural Depth & Leverage** | `9.5/10` | Tuân thủ chặt ADR-006 (`hybrid` packaging) |
| **7. UX Empathy** | `9.6/10` | Tách ngôn ngữ Non-tech (`Share & Present`) vs Dev (`Developer Handoff`) |
| **TOTAL AVERAGE** | **9.49/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 07.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Project Readiness Dashboard & 1-Click Deterministic lint_fix (GAP-14) theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Tái định vị Story 07.2 kế thừa toàn bộ Split Export & Packaging Engine (Offline HTML, Vector PDF, Plugin-Isolated PPTX) từ 07.3 cũ; cập nhật dependencies ["06.1", "06.5", "07.1"] theo ADR-006 | /refactor-story |
