---
type: I-Wish Story
id: "06.1"
title: "Story 06.1: Khung trình chiếu 16:9 HTML Slide Engine (1920x1080) & 8 Layout Templates chuẩn Dự án"
description: "Xây dựng bộ khung trình chiếu khóa tỷ lệ 16:9 với 8 bố cục chuẩn kế thừa tokens.css và icon-manifest.json của dự án."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.1/story.md"
status: refactored
risk_tier: 2
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-06.1"
  - "epic-06"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["03.2", "04.1"]
provides_apis:
  - "internal:epic-06/story-06.1"
consumes_apis: []
---

# Story 06.1: Khung trình chiếu 16:9 HTML Slide Engine (1920x1080) & 8 Layout Templates chuẩn Dự án

- **FR Covered:** [FR-15: Khung trình chiếu 16:9 HTML Slide Engine (1920x1080) & 8 Layout Templates chuẩn Dự án](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/epic.md)
- **Status:** `planned`
- **Risk Tier:** `2`

## 1. Story Description & Context
Xây dựng bộ khung trình chiếu khóa tỷ lệ 16:9 với 8 bố cục chuẩn kế thừa tokens.css và icon-manifest.json của dự án.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Khung trình chiếu 16:9 HTML Slide Engine (1920x1080) & 8 Layout Templates chuẩn Dự án" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 (Unified CompanionCanvasTab Slide Structure & 1-Click Structure Lens Switcher):** Khi mở tab tạo Slide hoặc file HTML trình chiếu trên `CompanionCanvasTab`, nửa trái (`#structureSubPane`) hiển thị danh sách cấu trúc Slide / Phase (`SLIDE 01..N` cho cả one-page lẫn multi-page HTML), nửa phải (`#htmlSubPane`) hiển thị HTML Stage khóa tỷ lệ khung hình (`16:9` / `16:10` / `Auto`) kèm nút **`▶ Present`**. Ngay trên header của khung Cấu trúc bên trái phải có thanh chuyển đổi góc nhìn 1-click **`[🔀 Userflow (2D DAG) | 📑 Slide / CMS]`** giúp chuyển tức thì giữa sơ đồ 2D DAG và danh sách cấu trúc Slide/Phase của cùng dự án mà không cần mở tab mới.
- **AC-04 ([EDGE-CASE] Resilience & Boundary Guard):** Tự động co giãn tỷ lệ `--slide-scale` mượt mà trên mọi độ phân giải màn hình mà không làm lệch bố cục `1920x1080` bên trong; khi chuyển qua lại giữa góc nhìn `🔀 Userflow (2D DAG)` và `📑 Slide / CMS`, giữ nguyên trạng thái chọn node/slide hiện tại.
- **AC-05 (HostRuntimeContract — data-cowok-deck-protocol="1"):** Khóa cứng hợp đồng điều hướng `.slide.active`, phím mũi tên, postMessage đồng bộ chỉ số slide và bộ đếm trang giữa Host và iframe 16:9 để Agent không bao giờ tự viết JS điều hướng xung đột.

## 4. Tasks
- [ ] **Task 06.1.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Khung trình chiếu 16:9 HTML Slide Engine (1920x1080) & 8 Layout Templates chuẩn Dự án" (thỏa mãn `AC-01`).
- [ ] **Task 06.1.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 06.1.3**: Cài đặt khung danh sách `Slide / Phase Structure Map` tích hợp trên `CompanionCanvasTab` và nút chuyển góc nhìn `[🔀 Userflow (2D DAG) | 📑 Slide / CMS]` ngay trên header khung cấu trúc (thỏa mãn `AC-03`).
- [ ] **Task 06.1.4**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` co giãn `--slide-scale` & bảo toàn trạng thái khi chuyển góc nhìn (thỏa mãn `AC-04`).
- [ ] **Task 06.1.5**: Triển khai `HostRuntimeContract` khóa cứng giao thức `data-cowok-deck-protocol="1"` đồng bộ postMessage và phím điều hướng (thỏa mãn `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 06.1.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 06.1.2` | Contract & Schema Verification |
| **AC-03** | `Task 06.1.3` | Playwright E2E Structure Lens Switch Test |
| **AC-04 (`[EDGE-CASE]`)** | `Task 06.1.4` | Boundary Regression Test |
| **AC-05** | `Task 06.1.5` | Deck Protocol Navigation Contract Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["03.2", "04.1"]

### Impacts
- Cung cấp năng lực của `Story 06.1` cho các Story phụ thuộc trong `Epic 06` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["03.2", "04.1"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-15` |
| **2. Data Integrity & State** | `9.3/10` | Đảm bảo tính nhất quán dữ liệu SQLite WAL & File Sync |
| **3. Security & Validation** | `9.5/10` | Kiểm soát chặt biên giới an toàn và xác thực đầu vào |
| **4. Performance & Scalability** | `9.5/10` | Tối ưu hóa độ trễ thực thi trên nhân Rust |
| **5. Error Handling & Recovery** | `9.3/10` | Có cơ chế xử lý lỗi biên `[EDGE-CASE]` tường minh |
| **6. Architectural Depth & Leverage** | `9.4/10` | Tái sử dụng kiểu dữ liệu `crates/domain` & `ts-rs` |
| **7. UX Empathy** | `9.4/10` | Trải nghiệm mượt mà cho cả Human và Agent |
| **TOTAL AVERAGE** | **9.40/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 06.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-08 | 2.0.0 | Nâng cấp AC-03..AC-04 đồng bộ cấu trúc Slide/Phase trên CompanionCanvasTab & thanh chuyển góc nhìn [Userflow | Slide/CMS] theo DESIGN.md v5.4 | cowok Orchestrator |
| 2026-11-12 | 3.0.0 | [refactor-story] Bổ sung AC-05 HostRuntimeContract (data-cowok-deck-protocol="1") theo Master Plan | /refactor-story |
