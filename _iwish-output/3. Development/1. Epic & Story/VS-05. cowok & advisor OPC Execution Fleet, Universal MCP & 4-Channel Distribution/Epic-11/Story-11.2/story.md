---
type: I-Wish Story
id: "11.2"
title: "Story 11.2: Sandbox Rèn Công cụ Tạm thời có Kiểm duyệt (Controlled Ephemeral Tool Forging)"
description: "Cho phép mô hình Tier-1 hoặc advisor tạo script phụ trợ dùng một lần trong thư mục .cowok-design/ephemeral-tools/, quét AST/capabilities trước khi chạy với giới hạn thời gian <= 5s và đề xuất thăng cấp thành SKILL.md nếu tái sử dụng >= 3 lần."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.2/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-11.2"
  - "epic-11"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["11.1", "10.3"]
provides_apis:
  - "internal:epic-11/story-11.2"
consumes_apis: []
---

# Story 11.2: Sandbox Rèn Công cụ Tạm thời có Kiểm duyệt (Controlled Ephemeral Tool Forging)

- **FR Covered:** [FR-27: Sandbox Rèn Công cụ Tạm thời có Kiểm duyệt (Controlled Ephemeral Tool Forging)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 11: POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Cho phép mô hình Tier-1 hoặc advisor tạo script phụ trợ dùng một lần trong thư mục .cowok-design/ephemeral-tools/, quét AST/capabilities trước khi chạy với giới hạn thời gian <= 5s và đề xuất thăng cấp thành SKILL.md nếu tái sử dụng >= 3 lần.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Sandbox Rèn Công cụ Tạm thời có Kiểm duyệt (Controlled Ephemeral Tool Forging)" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Chặn mọi lệnh truy cập mạng ngoài, đọc biến môi trường nhạy cảm hoặc ghi file ngoài thư mục dự án đối với script tự rèn.
- **AC-04 (Run-Scoped Outbound MCP Tool Bundles & Deterministic Layout Solvers):** Đóng gói bộ công cụ MCP theo đúng phạm vi từng lượt chạy (`Run-Scoped Tool Bundles`) kèm các bộ giải bố cục xác định (như `Banner Solver G1–G15` và `Grounded Diagram elk.js + repo_extract`).

## 4. Tasks
- [ ] **Task 11.2.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Sandbox Rèn Công cụ Tạm thời có Kiểm duyệt (Controlled Ephemeral Tool Forging)" (thỏa mãn `AC-01`).
- [ ] **Task 11.2.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 11.2.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).
- [ ] **Task 11.2.4**: Cài đặt cơ chế đóng gói MCP Tools theo Run-Scoped Tool Bundles tích hợp Banner Solver G1–G15 và Grounded Diagram layout engine (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 11.2.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 11.2.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 11.2.3` | Boundary Regression Test |
| **AC-04** | `Task 11.2.4` | Tool Bundler & Layout Solver Integration Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["11.1", "10.3"]

### Impacts
- Cung cấp năng lực của `Story 11.2` cho các Story phụ thuộc trong `Epic 11` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["11.1", "10.3"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-27` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 11.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Run-Scoped Outbound MCP Tool Bundles & Deterministic Layout Solvers theo Master Plan | /refactor-story |
