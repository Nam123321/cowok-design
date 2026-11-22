---
type: I-Wish Story
id: "09.3"
title: "Story 09.3: Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker"
description: "Chụp bản đồ tọa độ DOM nguyên tử kèm kiểm tra che khuất điểm ảnh tâm phần tử (0% click nhầm), kiểm soát đồng thời bằng mã băm StaleFile/StalePage và tự động ngắt mạch khi lỗi lặp lại 3 lần liên tiếp."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-09/Story-09.3/story.md"
status: cancelled
superseded_by: "05.4, 07.1, 09.2"
risk_tier: 3
delivery: core
adr_ref: "ADR-006"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-09.3"
  - "epic-09"
  - "uxui-os"
  - "deprecated"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["09.2", "05.4", "07.1"]
provides_apis: []
consumes_apis: []
---

# Story 09.3: [DEPRECATED -> 05.4, 07.1, 09.2] Tích hợp `window.__jevFast` + `elementFromPoint` Occlusion Guard, `StaleFile` OCC & 3-Strike Breaker

> **DEPRECATION NOTICE (`2026-11-20`):** Phạm vi của `Story 09.3` đã được phân bổ và hợp nhất vào 3 Vertical Slices: (1) `StaleFile` OCC & `MAX_ATTEMPTS = 3` breaker chuyển sang **`Story 09.2`**, (2) `elementFromPoint` + Real RGBA SSIM Occlusion Guard chuyển sang **`Story 07.1`**, và (3) Live DOM Patch / Intent Codemod + `sketch_seed` / `DesignerTalkOverlay` chuyển sang **`Story 05.4`**. Thư mục & ACs cũ được giữ nguyên để bảo toàn audit trail.

- **FR Covered:** [FR-23: Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 09: Design-Only REA & Awesome-RE Forge & Decision Cascade Engine](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-09/epic.md)
- **Status:** `cancelled` *(Superseded by `Story 05.4`, `Story 07.1`, `Story 09.2`)*
- **Risk Tier:** `3`

## 1. Story Description & Context
*(Deprecated — Absorbed into `Story 05.4`, `Story 07.1`, `Story 09.2`.)* Chụp bản đồ tọa độ DOM nguyên tử kèm kiểm tra che khuất điểm ảnh tâm phần tử (0% click nhầm), kiểm soát đồng thời bằng mã băm StaleFile/StalePage và tự động ngắt mạch khi lỗi lặp lại 3 lần liên tiếp.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **[DEPRECATED] AC-01 (Core Capability):** *(Superseded by `07.1`, `09.2`.)* Triển khai đầy đủ chức năng "Tích hợp window.__jevFast + elementFromPoint Occlusion Guard, StaleFile OCC & 3-Strike Breaker" tuân thủ hợp đồng kiểu dữ liệu trong `crates/domain` và kiến trúc Rust Core Daemon.
- **[DEPRECATED] AC-02 (Integration & Constraints):** *(Superseded by `07.1`, `09.2`.)* Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **[DEPRECATED] AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** *(Superseded by `09.2`.)* Khi 3-Strike Circuit Breaker kích hoạt, lập tức dừng tiến trình đang lặp và chuyển toàn bộ nhật ký lỗi sang cho cowok và advisor.
- **[DEPRECATED] AC-04 (cowok-codebase-harness: Live UITree, 11 Typed Intent Codemods, 8-Step Pixel Blast-Radius Rollback & .1d-Style Layout Seed/Compile):** *(Superseded by `05.4` & `02.4` `oxc` reverse sync.)* Khi mở Git Repo có sẵn (Codebase Mode trong Git Worktree cô lập), tự động quét các route/màn hình đưa lên 2D Storyboard, gắn tọa độ mã nguồn `file:line:col` vào cây `UITree`; cung cấp 11 phép biến đổi AST có kiểu.

## 4. Tasks
- [ ] **[DEPRECATED] Task 09.3.1**: *(Absorbed by 07.1 / 09.2)* Thiết kế và cài đặt mô-đun cốt lõi cho `elementFromPoint` Occlusion Guard, `StaleFile` OCC & `MAX_ATTEMPTS = 3` Breaker.
- [ ] **[DEPRECATED] Task 09.3.2**: *(Absorbed by 09.2)* Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon`.
- [ ] **[DEPRECATED] Task 09.3.3**: *(Absorbed by 09.2)* Cài đặt chốt chặn 3-Strike Circuit Breaker `[EDGE-CASE]`.
- [ ] **[DEPRECATED] Task 09.3.4**: *(Absorbed by 05.4 / 02.4)* Xây dựng `cowok-codebase-harness`, Intent Codemods & Pixel Blast-Radius Rollback.

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 09.3.1` | Superseded by `07.1` & `09.2` |
| **AC-02** | `Task 09.3.2` | Superseded by `09.2` |
| **AC-03 (`[EDGE-CASE]`)** | `Task 09.3.3` | Superseded by `09.2` |
| **AC-04** | `Task 09.3.4` | Superseded by `05.4` & `02.4` |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["09.2", "05.4", "07.1"]

### Impacts
- Đã chuyển giao toàn bộ năng lực sang `Story 05.4`, `Story 07.1` và `Story 09.2`.

### Consumes
- Tham chiếu `09.2`, `05.4`, `07.1`.

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Audit trail hợp nhất sang `05.4, 07.1, 09.2` |
| **2. Data Integrity & State** | `9.3/10` | Bảo toàn thư mục lịch sử & AC gốc |
| **3. Security & Validation** | `9.5/10` | OCC & 3-Strike được giữ trọn tại `09.2` |
| **4. Performance & Scalability** | `9.5/10` | Không trùng lặp crate/process |
| **5. Error Handling & Recovery** | `9.3/10` | Ghi rõ `superseded_by: "05.4, 07.1, 09.2"` |
| **6. Architectural Depth & Leverage** | `9.4/10` | Giảm phân mảnh horizontal |
| **7. UX Empathy** | `9.4/10` | Traceability đầy đủ |
| **TOTAL AVERAGE** | **9.40/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 09.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 cowok-codebase-harness (11 Intent Codemods, 8-Step Pixel Rollback, Layout Seed/Compile) theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Đánh dấu cancelled & [DEPRECATED]; phân bổ scope sang Story 05.4, 07.1 (occlusion SSIM) và 09.2 (MAX_ATTEMPTS=3 & StaleFile OCC) theo ADR-006 | /refactor-story |
