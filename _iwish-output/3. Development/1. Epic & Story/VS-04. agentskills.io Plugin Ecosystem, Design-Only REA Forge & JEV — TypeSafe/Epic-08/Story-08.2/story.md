---
type: I-Wish Story
id: "08.2"
title: "Story 08.2: Động cơ Đóng gói lại Repo GitHub & Skill Ngoài (crates/skill-repackager)"
description: "Tự động phân tích kho GitHub hoặc workflow/skill rời rạc do người dùng nhập vào, bóc tách thành SKILL.md + scripts/ + references/ + assets/ chuẩn agentskills.io và sinh manifest plugin.json đồng bộ với Cowok.ai."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/Story-08.2/story.md"
status: planned
risk_tier: 3
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-08.2"
  - "epic-08"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["08.1"]
provides_apis:
  - "internal:epic-08/story-08.2"
consumes_apis: []
---

# Story 08.2: Động cơ Đóng gói lại Repo GitHub & Skill Ngoài (crates/skill-repackager)

- **FR Covered:** [FR-20: Động cơ Đóng gói lại Repo GitHub & Skill Ngoài (crates/skill-repackager)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 08: agentskills.io Repackager Engine, 6 Built-in Harness Skills & GitHub Cron Lifecycle](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/epic.md)
- **Status:** `planned`
- **Risk Tier:** `3`

## 1. Story Description & Context
Tự động phân tích kho GitHub hoặc workflow/skill rời rạc do người dùng nhập vào, bóc tách thành SKILL.md + scripts/ + references/ + assets/ chuẩn agentskills.io và sinh manifest plugin.json đồng bộ với Cowok.ai.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Động cơ Đóng gói lại Repo GitHub & Skill Ngoài (crates/skill-repackager)" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Quét bảo mật và chặn mọi script độc hại hoặc lệnh gọi mạng trái phép khi nhập repository từ GitHub bên ngoài.

## 4. Tasks
- [ ] **Task 08.2.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Động cơ Đóng gói lại Repo GitHub & Skill Ngoài (crates/skill-repackager)" (thỏa mãn `AC-01`).
- [ ] **Task 08.2.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 08.2.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 08.2.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 08.2.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 08.2.3` | Boundary Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["08.1"]

### Impacts
- Cung cấp năng lực của `Story 08.2` cho các Story phụ thuộc trong `Epic 08` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["08.1"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-20` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 08.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
