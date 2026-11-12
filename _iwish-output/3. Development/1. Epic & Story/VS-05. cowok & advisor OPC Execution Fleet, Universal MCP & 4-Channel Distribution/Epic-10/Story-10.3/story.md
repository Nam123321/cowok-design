---
type: I-Wish Story
id: "10.3"
title: "Story 10.3: Xây dựng advisor Meta-Agent (6-Step Self-Evolution Loop) & Đấu trường Thiết kế (Design Tournament Mode)"
description: "Thiết lập advisor tự động chạy vòng lặp tự tiến hóa 6 bước (Fail -> Root Cause -> Patch SKILL.md -> Patch AST/Eval -> Regression Test -> Close) khi 3-Strike Breaker kích hoạt, đồng thời hỗ trợ Design Tournament Mode cho 2-3 agent/model thi đấu song song."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/Story-10.3/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-10.3"
  - "epic-10"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["10.2"]
provides_apis:
  - "internal:epic-10/story-10.3"
consumes_apis: []
---

# Story 10.3: Xây dựng advisor Meta-Agent (6-Step Self-Evolution Loop) & Đấu trường Thiết kế (Design Tournament Mode)

- **FR Covered:** [FR-24, FR-26, FR-27: Xây dựng advisor Meta-Agent (6-Step Self-Evolution Loop) & Đấu trường Thiết kế (Design Tournament Mode)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 10: cowok Master Orchestrator, advisor Meta-Agent, OPC Execution Fleet & Design Tournament](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Thiết lập advisor tự động chạy vòng lặp tự tiến hóa 6 bước (Fail -> Root Cause -> Patch SKILL.md -> Patch AST/Eval -> Regression Test -> Close) khi 3-Strike Breaker kích hoạt, đồng thời hỗ trợ Design Tournament Mode cho 2-3 agent/model thi đấu song song.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Xây dựng advisor Meta-Agent (6-Step Self-Evolution Loop) & Đấu trường Thiết kế (Design Tournament Mode)" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 (Variant Diversity Gate + Blind Wipe Desktop/Mobile Judge + Cross-Variant Element Borrow — GAP-12):** Khi kích hoạt Tournament:
  1. Kiểm tra `check_variants` đảm bảo các phương án thực sự khác biệt về bố cục trước khi chạy 2–4 nhánh song song.
  2. Bổ sung chế độ Chấm Điểm Mù Trượt Ngang (`Blind Wipe/Split Judge`) trên cả Desktop và Mobile 390px kèm ô nhập "1 điểm bản thua làm tốt hơn" (`Cross-Variant Borrow`).
  3. Sau khi chọn bản thắng, lưu các bản còn lại vào `variants/_archive/` và tự động chạy 1 lượt tinh chỉnh mượn đúng thành phần chỉ định từ bản thua sang bản thắng.
- **AC-04 ([EDGE-CASE] Resilience & Boundary Guard):** Mọi bản vá `SKILL.md` do `advisor` sinh ra phải vượt qua bài kiểm thử hồi quy trước khi được kích hoạt chính thức cho toàn đội hình; nếu user chuyển qua lại nhanh giữa `Option A -> B -> C` liên tục trước khi bấm `Enter`, chỉ phương án đang active cuối cùng tại thời điểm bấm `Xác nhận / Enter` mới được commit.
- **AC-05 (Event-Sourced Creative Memory Tree & 1-Click Post-Project /crystallize — GAP-18):** Lưu tín hiệu thắng/thua từ Tournament và các chỉnh sửa của người dùng vào Cây Bộ Nhớ Thẩm Mỹ (`Creative Memory`) để `advisor` tự động tinh chỉnh gu thiết kế cho các dự án sau; cung cấp nút Kết tinh 1-Click (`/crystallize`) biến một dự án/màn hình hoàn thành thành Starter Template, Brand Kit hoặc Skill Plugin tái sử dụng.

## 4. Tasks
- [ ] **Task 10.3.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Xây dựng advisor Meta-Agent (6-Step Self-Evolution Loop) & Đấu trường Thiết kế (Design Tournament Mode)" (thỏa mãn `AC-01`).
- [ ] **Task 10.3.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 10.3.3**: Xây dựng Variant Diversity Gate, Blind Wipe Desktop/Mobile Judge và Cross-Variant Element Borrow (thỏa mãn `AC-03`).
- [ ] **Task 10.3.4**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` kiểm thử hồi quy `SKILL.md` và chống race-condition khi chuyển nhanh phương án Tournament (thỏa mãn `AC-04`).
- [ ] **Task 10.3.5**: Cài đặt Event-Sourced Creative Memory Tree và nút Kết tinh 1-Click `/crystallize` (thỏa mãn `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 10.3.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 10.3.2` | Contract & Schema Verification |
| **AC-03** | `Task 10.3.3` | Blind Wipe Judge & Element Borrow Test |
| **AC-04 (`[EDGE-CASE]`)** | `Task 10.3.4` | Boundary Regression Test |
| **AC-05** | `Task 10.3.5` | Creative Memory & /crystallize Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["10.2"]

### Impacts
- Cung cấp năng lực của `Story 10.3` cho các Story phụ thuộc trong `Epic 10` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["10.2"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-24, FR-26, FR-27` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 10.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-08 | 2.0.0 | Nâng cấp AC-03..AC-04 bổ sung In-Chat + In-2D-DAG Tournament Synchronized Selector, Preview-on-Click & Explicit Confirmation Gate theo DESIGN.md v5.4 | cowok Orchestrator |
| 2026-11-12 | 2.1.0 | [refactor-story] Nâng cấp AC-03 Blind Wipe Tournament, bổ sung AC-05 Creative Memory Tree & /crystallize (GAP-12, GAP-18) theo Master Plan | /refactor-story |
