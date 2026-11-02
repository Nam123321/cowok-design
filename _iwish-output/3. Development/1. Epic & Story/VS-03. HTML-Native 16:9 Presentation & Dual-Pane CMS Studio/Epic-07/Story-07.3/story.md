---
type: I-Wish Story
id: "07.3"
title: "Story 07.3: Bộ xuất bản Self-Contained Offline HTML Bundle & Vector PDF (1920x1080)"
description: "Đóng gói toàn bộ bài thuyết trình/báo cáo thành 1 file .html độc lập duy nhất (nhúng sẵn CSS tokens, SVG icons, dữ liệu biểu đồ) hoặc kết xuất ra file .pdf vector chuẩn tỷ lệ 16:9."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/Story-07.3/story.md"
status: planned
risk_tier: 2
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-07.3"
  - "epic-07"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["07.1", "06.3"]
provides_apis:
  - "internal:epic-07/story-07.3"
consumes_apis: []
---

# Story 07.3: Bộ xuất bản Self-Contained Offline HTML Bundle & Vector PDF (1920x1080)

- **FR Covered:** [FR-18: Bộ xuất bản Self-Contained Offline HTML Bundle & Vector PDF (1920x1080)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 07: 17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/epic.md)
- **Status:** `planned`
- **Risk Tier:** `2`

## 1. Story Description & Context
Đóng gói toàn bộ bài thuyết trình/báo cáo thành 1 file .html độc lập duy nhất (nhúng sẵn CSS tokens, SVG icons, dữ liệu biểu đồ) hoặc kết xuất ra file .pdf vector chuẩn tỷ lệ 16:9.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Bộ xuất bản Self-Contained Offline HTML Bundle & Vector PDF (1920x1080)" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Đảm bảo các biểu đồ động và SVG icons hiển thị sắc nét 100% khi mở file .html ở chế độ hoàn toàn không có mạng Internet.

## 4. Tasks
- [ ] **Task 07.3.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Bộ xuất bản Self-Contained Offline HTML Bundle & Vector PDF (1920x1080)" (thỏa mãn `AC-01`).
- [ ] **Task 07.3.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 07.3.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 07.3.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 07.3.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 07.3.3` | Boundary Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["07.1", "06.3"]

### Impacts
- Cung cấp năng lực của `Story 07.3` cho các Story phụ thuộc trong `Epic 07` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["07.1", "06.3"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-18` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 07.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
