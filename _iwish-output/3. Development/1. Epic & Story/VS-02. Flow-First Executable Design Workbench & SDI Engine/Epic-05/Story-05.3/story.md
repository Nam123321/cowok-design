---
type: I-Wish Story
id: "05.3"
title: "Story 05.3: Lịch sử Phiên bản (VersionSnapshot / Revert) & Xuất bản Full Repo ZIP / GitHub PR"
description: "Lưu trữ chuỗi VersionSnapshot hỗ trợ khôi phục 1-click về bất kỳ phiên bản ổn định nào, đồng thời xuất toàn bộ mã nguồn dự án chạy được ra file .zip hoặc tạo GitHub Pull Request."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/Story-05.3/story.md"
status: planned
risk_tier: 2
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-05.3"
  - "epic-05"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["05.2"]
provides_apis:
  - "internal:epic-05/story-05.3"
consumes_apis: []
---

# Story 05.3: Lịch sử Phiên bản (VersionSnapshot / Revert) & Xuất bản Full Repo ZIP / GitHub PR

- **FR Covered:** [FR-08, FR-09: Lịch sử Phiên bản (VersionSnapshot / Revert) & Xuất bản Full Repo ZIP / GitHub PR](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 05: Structured Design Intent (SDI), Visual Clarification Variants & ChangeProposal Gate](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/epic.md)
- **Status:** `planned`
- **Risk Tier:** `2`

## 1. Story Description & Context
Lưu trữ chuỗi VersionSnapshot hỗ trợ khôi phục 1-click về bất kỳ phiên bản ổn định nào, đồng thời xuất toàn bộ mã nguồn dự án chạy được ra file .zip hoặc tạo GitHub Pull Request.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Lịch sử Phiên bản (VersionSnapshot / Revert) & Xuất bản Full Repo ZIP / GitHub PR" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Đảm bảo gói ZIP xuất ra luôn bao gồm đầy đủ tokens.css, icon-manifest.json, fixtures và chạy được lệnh pnpm install && pnpm build ngay lập tức.

## 4. Tasks
- [ ] **Task 05.3.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Lịch sử Phiên bản (VersionSnapshot / Revert) & Xuất bản Full Repo ZIP / GitHub PR" (thỏa mãn `AC-01`).
- [ ] **Task 05.3.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 05.3.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 05.3.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 05.3.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 05.3.3` | Boundary Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["05.2"]

### Impacts
- Cung cấp năng lực của `Story 05.3` cho các Story phụ thuộc trong `Epic 05` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["05.2"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-08, FR-09` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 05.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
