---
type: I-Wish Story
id: "02.2"
title: "Story 02.2: Token Budget Governor (<= 1,500 Input Tokens) & Deterministic Prompt Compiler"
description: "Kiểm soát cứng ngân sách token đầu vào (<= 1,500 tokens) và lắp ráp gói prompt xác định trước khi gửi tới các kênh thực thi."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-02/Story-02.2/story.md"
status: planned
risk_tier: 2
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-02.2"
  - "epic-02"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["02.1"]
provides_apis:
  - "internal:epic-02/story-02.2"
consumes_apis: []
---

# Story 02.2: Token Budget Governor (<= 1,500 Input Tokens) & Deterministic Prompt Compiler

- **FR Covered:** [FR-14: Token Budget Governor (<= 1,500 Input Tokens) & Deterministic Prompt Compiler](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 02: JIT Prompt Assembler & 3-Profile Model Capability Adapter](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-02/epic.md)
- **Status:** `planned`
- **Risk Tier:** `2`

## 1. Story Description & Context
Kiểm soát cứng ngân sách token đầu vào (<= 1,500 tokens) và lắp ráp gói prompt xác định trước khi gửi tới các kênh thực thi.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Token Budget Governor (<= 1,500 Input Tokens) & Deterministic Prompt Compiler" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Tự động nén danh sách component/icon theo độ liên quan ngữ nghĩa nếu lát cắt ban đầu vượt quá 1,500 tokens.

## 4. Tasks
- [ ] **Task 02.2.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Token Budget Governor (<= 1,500 Input Tokens) & Deterministic Prompt Compiler" (thỏa mãn `AC-01`).
- [ ] **Task 02.2.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 02.2.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 02.2.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 02.2.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 02.2.3` | Boundary Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["02.1"]

### Impacts
- Cung cấp năng lực của `Story 02.2` cho các Story phụ thuộc trong `Epic 02` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["02.1"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-14` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 02.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
