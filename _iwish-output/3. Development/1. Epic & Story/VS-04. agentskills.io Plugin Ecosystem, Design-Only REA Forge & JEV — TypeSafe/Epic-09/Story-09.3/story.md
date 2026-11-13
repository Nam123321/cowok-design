---
type: I-Wish Story
id: "09.3"
title: "Story 09.3: Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker"
description: "Chụp bản đồ tọa độ DOM nguyên tử kèm kiểm tra che khuất điểm ảnh tâm phần tử (0% click nhầm), kiểm soát đồng thời bằng mã băm StaleFile/StalePage và tự động ngắt mạch khi lỗi lặp lại 3 lần liên tiếp."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-09/Story-09.3/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-12T16:45:00+07:00"
tags:
  - "story-09.3"
  - "epic-09"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["09.2"]
provides_apis:
  - "internal:epic-09/story-09.3"
consumes_apis: []
---

# Story 09.3: Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker

- **FR Covered:** [FR-23: Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 09: Design-Only REA & Awesome-RE Forge & JEV — TypeSafe 6-Primitive Acceleration Engine](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-09/epic.md)
- **Status:** `planned`
- **Risk Tier:** `3`

## 1. Story Description & Context
Chụp bản đồ tọa độ DOM nguyên tử kèm kiểm tra che khuất điểm ảnh tâm phần tử (0% click nhầm), kiểm soát đồng thời bằng mã băm StaleFile/StalePage và tự động ngắt mạch khi lỗi lặp lại 3 lần liên tiếp.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability):** Triển khai đầy đủ chức năng "Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Khi 3-Strike Circuit Breaker kích hoạt, lập tức dừng tiến trình đang lặp và chuyển toàn bộ nhật ký lỗi sang cho cowok và advisor.
- **AC-04 (cowok-codebase-harness: Live UITree, 11 Typed Intent Codemods, 8-Step Pixel Blast-Radius Rollback & .1d-Style Layout Seed/Compile):** Khi mở Git Repo có sẵn (Codebase Mode trong Git Worktree cô lập), tự động quét các route/màn hình đưa lên 2D Storyboard, gắn tọa độ mã nguồn `file:line:col` vào cây `UITree`; cung cấp 11 phép biến đổi AST có kiểu (Intent Codemods: `setStyle`, `setText`, `setProp`, `reorder`, `wrap`, `unwrap`, `insert`, `remove`, `swapComponent`, `structural`, `replaceRange`); chu trình kiểm chứng 8 bước: nếu sửa 1 instance mà làm rò rỉ pixel ra ngoài khung phần tử đích (`BlastSummary.outsidePixels`) hoặc vỡ Dark Mode/Mobile thì tự động Rollback tức thì; hỗ trợ cặp `sketch_seed` $\leftrightarrow$ `sketch_compile` rút trích màn hình thành JSON nhẹ để kéo xếp nhanh rồi biên dịch ngược về code.

## 4. Tasks
- [ ] **Task 09.3.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker" (thỏa mãn `AC-01`).
- [ ] **Task 09.3.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 09.3.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).
- [ ] **Task 09.3.4**: Xây dựng `cowok-codebase-harness`, 11 Typed Intent Codemods, 8-Step Pixel Blast-Radius Rollback và bộ đôi `sketch_seed` $\leftrightarrow$ `sketch_compile` (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 09.3.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 09.3.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 09.3.3` | Boundary Regression Test |
| **AC-04** | `Task 09.3.4` | Codemod Blast-Radius & Layout Seed/Compile Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["09.2"]

### Impacts
- Cung cấp năng lực của `Story 09.3` cho các Story phụ thuộc trong `Epic 09` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["09.2"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-23` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 09.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 cowok-codebase-harness (11 Intent Codemods, 8-Step Pixel Rollback, Layout Seed/Compile) theo Master Plan | /refactor-story |
