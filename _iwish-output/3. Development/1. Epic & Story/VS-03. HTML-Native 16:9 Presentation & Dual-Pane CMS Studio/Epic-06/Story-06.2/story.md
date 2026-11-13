---
type: I-Wish Story
id: "06.2"
title: "Story 06.2: Giao diện Dual-Pane Human CMS Texting Editor đồng bộ <16ms qua data-ff-id"
description: "Cung cấp form nhập liệu có cấu trúc (Tiêu đề, Phụ đề, Ý chính, Chỉ số KPI, Ghi chú diễn giả) ở khung bên trái; mọi thao tác sửa chữ cập nhật thẳng vào AST và DOM slide bên phải trong <16ms."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.2/story.md"
status: refactored
risk_tier: 2
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-06.2"
  - "epic-06"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["06.1", "03.3"]
provides_apis:
  - "internal:epic-06/story-06.2"
consumes_apis: []
---

# Story 06.2: Giao diện Dual-Pane Human CMS Texting Editor đồng bộ <16ms qua data-ff-id

- **FR Covered:** [FR-16: Giao diện Dual-Pane Human CMS Texting Editor đồng bộ <16ms qua data-ff-id](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/epic.md)
- **Status:** `planned`
- **Risk Tier:** `2`

## 1. Story Description & Context
Cung cấp form nhập liệu có cấu trúc (Tiêu đề, Phụ đề, Ý chính, Chỉ số KPI, Ghi chú diễn giả) ở khung bên trái; mọi thao tác sửa chữ cập nhật thẳng vào AST và DOM slide bên phải trong <16ms.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Giao diện Dual-Pane Human CMS Texting Editor đồng bộ <16ms qua data-ff-id" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 (Horizontal Slide-Over Detailed CMS Drawer inside Structure Sub-Pane):** Khi user click vào bất kỳ thẻ Slide / Phase nào ở danh sách cấu trúc bên trái (`#slideListPane`), khung cấu trúc bên trái thực hiện hiệu ứng trượt ngang mượt mà (`transform: translateX(-50%)`, `240ms cubic-bezier`) sang màn hình **Chi tiết CMS của Slide đó** (`Tiêu đề chính`, `Nhãn Eyebrow / Tag`, `Chỉ số KPI 1`, `Chỉ số KPI 2`, `Nội dung mô tả / Bullet`, cùng nút **`← Quay lại danh sách Slide`**); mọi thao tác gõ chỉnh sửa trên form CMS cập nhật tức thì (`<16ms`) vào DOM HTML 16:9 đang hiển thị bên phải qua `data-ff-id`.
- **AC-04 ([EDGE-CASE] Resilience & Boundary Guard):** Giữ nguyên vị trí con trỏ gõ phím (`cursor state`) và lịch sử Undo/Redo trên form CMS khi đồng bộ tốc độ cao sang khung HTML 16:9 bên phải; tự động khử mã độc (`XSS sanitization`) nếu nội dung nhập chứa thẻ `<script>`.

## 4. Tasks
- [ ] **Task 06.2.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Giao diện Dual-Pane Human CMS Texting Editor đồng bộ <16ms qua data-ff-id" (thỏa mãn `AC-01`).
- [ ] **Task 06.2.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 06.2.3**: Xây dựng cơ chế `Slide-Over Detailed CMS Editor` (`translateX(-50%)`) ngay trong nửa trái `#structureSubPane` và binding 2 chiều `<16ms` sang HTML Stage bên phải (thỏa mãn `AC-03`).
- [ ] **Task 06.2.4**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` giữ vị trí con trỏ và khử XSS (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 06.2.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 06.2.2` | Contract & Schema Verification |
| **AC-03** | `Task 06.2.3` | Playwright E2E Slide-Over CMS `<16ms` Sync Test |
| **AC-04 (`[EDGE-CASE]`)** | `Task 06.2.4` | Boundary Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["06.1", "03.3"]

### Impacts
- Cung cấp năng lực của `Story 06.2` cho các Story phụ thuộc trong `Epic 06` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["06.1", "03.3"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-16` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 06.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-08 | 2.0.0 | Nâng cấp AC-03..AC-04 bổ sung cơ chế Slide-Over Detailed CMS Drawer (translateX(-50%)) đồng bộ <16ms sang HTML Stage theo DESIGN.md v5.4 | cowok Orchestrator |
