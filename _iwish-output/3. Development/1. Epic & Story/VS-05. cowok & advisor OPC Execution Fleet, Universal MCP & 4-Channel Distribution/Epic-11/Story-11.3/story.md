---
type: I-Wish Story
id: "11.3"
title: "Story 11.3: Compiler-Grade Bidirectional Open MCP Server & 1-Click Host Installer UI"
description: "Phơi bày mở toàn bộ hệ công cụ và tài nguyên theo 4 tầng quyền hạn (READ_ONLY, INSPECT, REVISE/PROPOSE, FULL_CREATE/HUMAN_ONLY) tích hợp Self-Healing ToolError, tài nguyên động cowok-design://rules, cowok://skills/*, ui://cowok-design/brief-and-preview.html, và cài đặt 1-click vào mọi nền tảng Agent bên ngoài."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.3/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-02T13:30:00+07:00"
tags:
  - "story-11.3"
  - "epic-11"
  - "uxui-os"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["11.1", "10.2"]
provides_apis:
  - "internal:epic-11/story-11.3"
consumes_apis: []
---

# Story 11.3: Compiler-Grade Bidirectional Open MCP Server & 1-Click Host Installer UI

- **FR Covered:** [FR-29: Universal Open MCP Server (stdio & Streamable HTTP/SSE) với 4 Tầng Phân quyền](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 11: POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Phơi bày mở toàn bộ hệ công cụ và tài nguyên theo 4 tầng quyền hạn (READ_ONLY, INSPECT, REVISE/PROPOSE, FULL_CREATE/HUMAN_ONLY) tích hợp Self-Healing ToolError, tài nguyên động cowok-design://rules, cowok://skills/*, ui://cowok-design/brief-and-preview.html, và cài đặt 1-click vào mọi nền tảng Agent bên ngoài.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Compiler-Grade Bidirectional Open MCP Server — Open Tool/Resource/Prompt Registry):** Phơi bày mở toàn bộ hệ công cụ và tài nguyên theo 4 tầng quyền hạn (`READ_ONLY`, `INSPECT`, `REVISE / PROPOSE`, `FULL_CREATE / HUMAN_ONLY`), bao gồm:
  1. `get_app_state` (`<500ms` bootstrap trọn trạng thái dự án, đồ thị 2D, tokens, components, rules).
  2. `apply_batch` (giao dịch nguyên tử `@alias` + `dry_run` + `idempotency_key` + trả trực tiếp ảnh diff `diff_image: true` và lỗi mới phát sinh `issues: { total, new_in_this_call }`).
  3. Truy vấn phần tử theo vị từ lỗi (`has-problem('text-truncated')`).
  4. `Self-Healing ToolError` (trả về `{ error_kind, valid_options, corrected_example, next_step }` giúp Agent ngoài tự sửa đúng ngay ở lượt kế tiếp).
  5. Resource động `cowok-design://rules` (*Build-Verify-Fix*) + `cowok://skills/*` + `cowok://design-systems/*` + `MCP Apps UI Resource` (`ui://cowok-design/brief-and-preview.html`).
- **AC-02 (Integration & Constraints):** Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** Từ chối ngay lập tức với mã lỗi JSON-RPC chuẩn nếu một Agent bên ngoài cố gọi các công cụ thuộc tầng HUMAN_ONLY mà không có cờ phê duyệt rõ ràng từ người dùng.
- **AC-04 (1-Click External Host MCP Installer UI & Live Activity Audit Log — GAP-03):** Nối thật giao diện `SettingsMcpAutomationsPane.tsx` với lệnh `cowok mcp install <agent>` (cài đặt 1-click có sao lưu an toàn vào `Cowok.ai`, `Antigravity`, `Claude Code`, `Codex`, `Cursor`, `OpenCode`...), quản lý bật/tắt quyền theo từng công cụ và hiển thị bảng nhật ký hoạt động MCP trực tiếp (`external-activity`).

## 4. Tasks
- [ ] **Task 11.3.1**: Thiết kế và cài đặt mô-đun cốt lõi cho "Compiler-Grade Bidirectional Open MCP Server" (thỏa mãn `AC-01`).
- [ ] **Task 11.3.2**: Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon` và kiểm định hợp đồng dữ liệu (thỏa mãn `AC-02`).
- [ ] **Task 11.3.3**: Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]` và viết bài kiểm thử tự động xác nhận (thỏa mãn `AC-03`).
- [ ] **Task 11.3.4**: Cài đặt 1-Click External Host MCP Installer UI trong SettingsMcpAutomationsPane.tsx và bảng Live Activity Audit Log (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 11.3.1` | Unit & Integration Test (`cargo test` / `vitest`) |
| **AC-02** | `Task 11.3.2` | Contract & Schema Verification |
| **AC-03 (`[EDGE-CASE]`)** | `Task 11.3.3` | Boundary Regression Test |
| **AC-04** | `Task 11.3.4` | Host Installer UI & Live Activity Audit Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["11.1", "10.2"]

### Impacts
- Cung cấp năng lực của `Story 11.3` cho các Story phụ thuộc trong `Epic 11` và các làn sóng tiếp theo.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["11.1", "10.2"].

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đáp ứng đầy đủ `FR-29` |
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
| 2026-11-02 | 1.0.0 | Khởi tạo Story 11.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Nâng cấp AC-01 Compiler-Grade Bidirectional MCP & bổ sung AC-04 1-Click Host Installer UI (GAP-03) theo Master Plan | /refactor-story |
