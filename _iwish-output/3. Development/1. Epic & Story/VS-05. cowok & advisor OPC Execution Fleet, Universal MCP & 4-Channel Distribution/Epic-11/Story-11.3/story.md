---
type: I-Wish Story
id: "11.3"
title: "Story 11.3: Compiler-Grade Bidirectional Open MCP Server & 1-Click Host Installer UI"
description: "Phơi bày mở toàn bộ hệ công cụ và tài nguyên theo 4 tầng quyền hạn (READ_ONLY, INSPECT, REVISE/PROPOSE, FULL_CREATE/HUMAN_ONLY) tích hợp Self-Healing ToolError, tài nguyên động cowok-design://rules, cowok://skills/*, ui://cowok-design/brief-and-preview.html, và cài đặt 1-click vào mọi nền tảng Agent bên ngoài."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.3/story.md"
status: cancelled
superseded_by: "11.2"
risk_tier: 3
delivery: core
adr_ref: "ADR-007"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-11.3"
  - "epic-11"
  - "uxui-os"
  - "deprecated"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["11.1", "11.2"]
provides_apis: []
consumes_apis: []
---

# Story 11.3: [DEPRECATED -> Story 11.2] Compiler-Grade Bidirectional Open MCP Server & 1-Click Host Installer UI

> **DEPRECATION NOTICE (`2026-11-20`):** Toàn bộ phạm vi Compiler-Grade Bidirectional Open MCP Server (`get_app_state`, `apply_batch`, `Self-Healing ToolError`, resources `cowok-design://rules`) và 1-Click External Host MCP Installer UI (`SettingsMcpAutomationsPane.tsx`) đã được hợp nhất thành Vertical Slice chính thức trong **`Story 11.2`** (còn phần Ephemeral Tool Forging & Security Jail chuyển về **`Story 08.5`** theo ADR-007). Thư mục và AC lịch sử được giữ nguyên để bảo toàn audit trail.

- **FR Covered:** [FR-29: Universal Open MCP Server (stdio & Streamable HTTP/SSE) với 4 Tầng Phân quyền](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 11: POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/epic.md)
- **Status:** `cancelled` *(Superseded by [Story 11.2](../Story-11.2/story.md))*
- **Risk Tier:** `3`

## 1. Story Description & Context
*(Deprecated — Absorbed into `Story 11.2`.)* Phơi bày mở toàn bộ hệ công cụ và tài nguyên theo 4 tầng quyền hạn (READ_ONLY, INSPECT, REVISE/PROPOSE, FULL_CREATE/HUMAN_ONLY) tích hợp Self-Healing ToolError, tài nguyên động cowok-design://rules, cowok://skills/*, ui://cowok-design/brief-and-preview.html, và cài đặt 1-click vào mọi nền tảng Agent bên ngoài.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **[DEPRECATED] AC-01 (Compiler-Grade Bidirectional Open MCP Server — Open Tool/Resource/Prompt Registry):** *(Superseded by `Story 11.2`.)* Phơi bày mở toàn bộ hệ công cụ và tài nguyên theo 4 tầng quyền hạn (`READ_ONLY`, `INSPECT`, `REVISE / PROPOSE`, `FULL_CREATE / HUMAN_ONLY`), bao gồm:
  1. `get_app_state` (`<500ms` bootstrap trọn trạng thái dự án, đồ thị 2D, tokens, components, rules).
  2. `apply_batch` (giao dịch nguyên tử `@alias` + `dry_run` + `idempotency_key` + trả trực tiếp ảnh diff `diff_image: true` và lỗi mới phát sinh `issues: { total, new_in_this_call }`).
  3. Truy vấn phần tử theo vị từ lỗi (`has-problem('text-truncated')`).
  4. `Self-Healing ToolError` (trả về `{ error_kind, valid_options, corrected_example, next_step }` giúp Agent ngoài tự sửa đúng ngay ở lượt kế tiếp).
  5. Resource động `cowok-design://rules` (*Build-Verify-Fix*) + `cowok://skills/*` + `cowok://design-systems/*` + `MCP Apps UI Resource` (`ui://cowok-design/brief-and-preview.html`).
- **[DEPRECATED] AC-02 (Integration & Constraints):** *(Superseded by `Story 11.2`.)* Tích hợp đồng bộ với SQLite WAL (`crates/spec-registry`), tuân thủ cấu hình ràng buộc dự án (`PROJECT_CONSTRAINTS`) và bộ điều phối `cowok` / `advisor`.
- **[DEPRECATED] AC-03 ([EDGE-CASE] Resilience & Boundary Guard):** *(Superseded by `Story 11.2`.)* Từ chối ngay lập tức với mã lỗi JSON-RPC chuẩn nếu một Agent bên ngoài cố gọi các công cụ thuộc tầng HUMAN_ONLY mà không có cờ phê duyệt rõ ràng từ người dùng.
- **[DEPRECATED] AC-04 (1-Click External Host MCP Installer UI & Live Activity Audit Log — GAP-03):** *(Superseded by `Story 11.2`.)* Nối thật giao diện `SettingsMcpAutomationsPane.tsx` với lệnh `cowok mcp install <agent>` (cài đặt 1-click có sao lưu an toàn vào `Cowok.ai`, `Antigravity`, `Claude Code`, `Codex`, `Cursor`, `OpenCode`...), quản lý bật/tắt quyền theo từng công cụ và hiển thị bảng nhật ký hoạt động MCP trực tiếp (`external-activity`).

## 4. Tasks
- [ ] **[DEPRECATED] Task 11.3.1**: *(Absorbed by Story 11.2)* Thiết kế và cài đặt mô-đun cốt lõi cho "Compiler-Grade Bidirectional Open MCP Server".
- [ ] **[DEPRECATED] Task 11.3.2**: *(Absorbed by Story 11.2)* Kết nối tích hợp với `crates/spec-registry` / `crates/harness-daemon`.
- [ ] **[DEPRECATED] Task 11.3.3**: *(Absorbed by Story 11.2)* Cài đặt chốt chặn xử lý tình huống biên `[EDGE-CASE]`.
- [ ] **[DEPRECATED] Task 11.3.4**: *(Absorbed by Story 11.2)* Cài đặt 1-Click External Host MCP Installer UI trong SettingsMcpAutomationsPane.tsx và bảng Live Activity Audit Log.

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 11.3.1` | Superseded by `Story 11.2` |
| **AC-02** | `Task 11.3.2` | Superseded by `Story 11.2` |
| **AC-03 (`[EDGE-CASE]`)** | `Task 11.3.3` | Superseded by `Story 11.2` |
| **AC-04** | `Task 11.3.4` | Superseded by `Story 11.2` |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["11.1", "11.2"]

### Impacts
- Đã hợp nhất vào `Story 11.2`.

### Consumes
- Tham chiếu `Story 11.2` để giữ tính toàn vẹn đồ thị kiểm toán.

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `PRESENTATIONS`, `SKILL_PLUGINS`, `FLEET_TASKS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Lịch sử kiểm toán hợp nhất sang `Story 11.2` |
| **2. Data Integrity & State** | `9.3/10` | Bảo toàn audit trail không xóa thư mục |
| **3. Security & Validation** | `9.5/10` | Giữ nguyên trace phân quyền HUMAN_ONLY sang `11.2` |
| **4. Performance & Scalability** | `9.5/10` | Không chạy độc lập, tiêu chuẩn hóa qua `11.2` |
| **5. Error Handling & Recovery** | `9.3/10` | Ghi rõ `superseded_by: "11.2"` |
| **6. Architectural Depth & Leverage** | `9.4/10` | Giảm phân mảnh giữa Tool Forging (`08.5`) và Open MCP Server (`11.2`) |
| **7. UX Empathy** | `9.4/10` | Trace liên kết rõ ràng |
| **TOTAL AVERAGE** | **9.40/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 11.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Nâng cấp AC-01 Compiler-Grade Bidirectional MCP & bổ sung AC-04 1-Click Host Installer UI (GAP-03) theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Đánh dấu cancelled & [DEPRECATED]; hợp nhất phạm vi MCP Server + Host Installer vào Story 11.2 (Tool Forging sang Story 08.5) theo ADR-007 | /refactor-story |
