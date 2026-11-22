---
type: I-Wish Story
id: "11.2"
title: "Story 11.2: Compiler-Grade Bidirectional Open MCP Server, 1-Click Host Installer UI & Run-Scoped Tool Bundles"
description: "Kế thừa phạm vi Open MCP Server + 1-Click Host Installer UI từ Story 11.3 cũ (phần Ephemeral Tool Forging đã chuyển sang Story 08.5 theo ADR-007): phơi bày 4 tầng phân quyền, get_app_state (<500ms), apply_batch (@alias/dry_run/diff_image), Self-Healing ToolError, Run-Scoped Tool Bundles (Banner Solver G1-G15, elk.js) và cài đặt 1-click vào Claude/Codex/Cursor/Antigravity/Cowok.ai."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.2/story.md"
status: refactored
risk_tier: 3
delivery: hybrid
adr_ref: "ADR-007"
components:
  - id: "open-mcp-gateway"
    packaging: "KERNEL_CORE"
  - id: "host-mcp-installer-pack"
    packaging: "EXTENSION_PLUGIN"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-11.2"
  - "epic-11"
  - "uxui-os"
  - "open-mcp"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["10.1", "11.1", "08.5"]
provides_apis:
  - "POST /api/v1/mcp/install"
  - "GET /api/v1/mcp/external-activity"
  - "POST /api/v1/mcp/tools/toggle"
consumes_apis:
  - "POST /api/v1/bot-fleet/execute"
  - "POST /api/v1/de/run"
---

# Story 11.2: Compiler-Grade Bidirectional Open MCP Server, 1-Click Host Installer UI & Run-Scoped Tool Bundles

- **FR Covered:** [FR-29: Universal Open MCP Server (stdio & Streamable HTTP/SSE) với 4 Tầng Phân quyền & 1-Click Host Installer](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 11: POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Kế thừa trọn vẹn phạm vi **Compiler-Grade Bidirectional Open MCP Server** và **1-Click External Host MCP Installer UI** từ Story 11.3 cũ (trong khi phần Ephemeral Tool Forging & Security Jail được tách về `Story 08.5` theo ADR-007): xây dựng `crates/mcp-gateway` hỗ trợ cả `stdio` lẫn Streamable HTTP/SSE, `get_app_state` (`<500ms`), `apply_batch` (`@alias` + `dry_run` + `diff_image`), `Self-Healing ToolError`, **Run-Scoped Outbound MCP Tool Bundles** (`Banner Solver G1–G15`, `Grounded Diagram elk.js`), và kết nối giao diện `SettingsMcpAutomationsPane.tsx` với `POST /api/v1/mcp/install` kèm bảng nhật ký `external-activity`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Khóa cứng bất biến `EC-LEARN-001` chặn 100% `ActorKind::McpClient` khỏi `HUMAN_ONLY` / Ephemeral Tool Jail và atomic `apply_batch` rollback khi bước `k` của `N` gặp lỗi.

## 3. Acceptance Criteria (AC)
- **AC-01 (Compiler-Grade Bidirectional Open MCP Server & 4-Tier Permission Model):** Phơi bày công cụ và tài nguyên qua `stdio` & Streamable HTTP/SSE theo 4 tầng (`READ_ONLY`, `INSPECT`, `REVISE / PROPOSE`, `FULL_CREATE / HUMAN_ONLY` — tầng thứ 4 bị ẩn và từ chối hoàn toàn khỏi bề mặt `McpClient`): `get_app_state` (`<500ms`), `apply_batch` (`@alias` + `dry_run` + `idempotency_key` + `diff_image` + `issues` + rollback nguyên tử toàn bộ `1..k` nếu bước `k` lỗi), truy vấn vị từ `has-problem('text-truncated')`, `Self-Healing ToolError`, và resources `cowok-design://rules`, `cowok://skills/*`, `ui://cowok-design/brief-and-preview.html`.
- **AC-02 (Run-Scoped Outbound MCP Tool Bundles & Deterministic Solvers):** Đóng gói công cụ MCP theo đúng phạm vi từng lượt chạy (`Run-Scoped Tool Bundles`) tích hợp với `Story 08.5` (Built-in DE Tools) cùng các bộ giải bố cục xác định (`Banner Solver G1–G15` và `Grounded Diagram elk.js + repo_extract` nhúng WASM/Rust-embedded kèm giới hạn `MAX_NODES` chống DoS không phụ thuộc Node ngoài ở bản đóng gói).
- **AC-03 ([EDGE-CASE] Unconditional `HUMAN_ONLY` / `EC-LEARN-001` Block & Safe Host Config Backup):** Tuân thủ tuyệt đối **`EC-LEARN-001` (`Story 08.4`)**: mọi lời gọi từ `ActorKind::McpClient` chạm tool/route tầng `FULL_CREATE / HUMAN_ONLY` (bao gồm Ephemeral Tool Jail `08.5` và `LEARN`/`Staging` promote) đều bị **từ chối vô điều kiện** với `403 HUMAN_APPROVAL_REQUIRED` / JSON-RPC `-32003` kèm deep-link dẫn human mở Workbench UI xác nhận trực tiếp (`ActorKind::HumanWorkbenchSession`, tuyệt đối không chấp nhận approval token giả từ MCP); khi `POST /api/v1/mcp/install` ghi cấu hình vào host ngoài luôn kiểm tra `O_NOFOLLOW` + atomic JSON/TOML AST merge + khóa ghi file + backup checksum để rollback 1-click.
- **AC-04 (1-Click External Host MCP Installer UI & Live Activity Audit Log):** Nối thật `SettingsMcpAutomationsPane.tsx` với `cowok mcp install <agent>` (`Cowok.ai`, `Antigravity`, `Claude Code`, `Codex`, `Cursor`, `OpenCode`...), bật/tắt quyền từng tool (`POST /api/v1/mcp/tools/toggle`) và hiển thị luồng audit `GET /api/v1/mcp/external-activity` theo thời gian thực.

## 4. Tasks
- [ ] **Task 11.2.1**: Cài đặt `crates/mcp-gateway` với `get_app_state`, `apply_batch`, `Self-Healing ToolError`, 4 tầng phân quyền và dynamic resources `cowok-design://*` (thỏa mãn `AC-01`, `AC-03`).
- [ ] **Task 11.2.2**: Tích hợp Run-Scoped Outbound MCP Tool Bundles, Banner Solver G1–G15 và Grounded Diagram `elk.js` liên thông DE tools `Story 08.5` (thỏa mãn `AC-02`).
- [ ] **Task 11.2.3**: Xây dựng 1-Click Host Installer (`POST /api/v1/mcp/install`), backup/rollback cấu hình an toàn, Tool Toggle & `external-activity` live audit log nối với `SettingsMcpAutomationsPane.tsx` (thỏa mãn `AC-03`, `AC-04`).
- [ ] **Task 11.2.4**: Viết bộ test MCP JSON-RPC, `HUMAN_ONLY` rejection, `Self-Healing ToolError` round-trip và Playwright test màn hình `SettingsMcpAutomationsPane.tsx` (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 11.2.1`, `Task 11.2.4` | Bidirectional MCP Protocol & `get_app_state` Latency Test |
| **AC-02** | `Task 11.2.2`, `Task 11.2.4` | Run-Scoped Bundler & Layout Solver Integration Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 11.2.1`, `Task 11.2.3`, `Task 11.2.4` | `HUMAN_ONLY` JSON-RPC Rejection & Backup Rollback Test |
| **AC-04** | `Task 11.2.3`, `Task 11.2.4` | 1-Click Host Installer UI & Live Audit Playwright Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["10.1", "11.1", "08.5"]

### Impacts
- Cung cấp kênh MCP chuẩn cho `Story 12.1` (`uiux-os mcp`), `Story 12.2` (Tauri/Cowok Native Plugin) và các host AI ngoài.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["10.1", "11.1", "08.5"] (`3-Lane Router` / MPSC Write Actor, `crates/bot-fleet` & `$COWOK_SESSION_DIR`, Built-in DE tools + `HUMAN_ONLY` Ephemeral Tool Jail policy).

### Shared Entities
- Đọc/Ghi `PROJECTS`, `SPEC_DOCUMENTS`, `SKILL_PLUGINS`, `decision_audit_log` (`V17`) trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Hợp nhất trọn vẹn Bidirectional MCP + Host Installer UI |
| **2. Data Integrity & State** | `9.4/10` | Atomic `apply_batch` với `idempotency_key` & backup host config |
| **3. Security & Validation** | `9.6/10` | Chặn chặt `HUMAN_ONLY` qua JSON-RPC, tách Ephemeral Jail về `08.5` |
| **4. Performance & Scalability** | `9.5/10` | `get_app_state < 500ms`, Run-Scoped bundle tránh bloat context |
| **5. Error Handling & Recovery** | `9.5/10` | `Self-Healing ToolError` giúp Agent ngoài tự hồi phục lượt kế |
| **6. Architectural Depth & Leverage** | `9.5/10` | `hybrid` packaging theo ADR-007 |
| **7. UX Empathy** | `9.5/10` | 1-click cài đặt vào mọi IDE/CLI phổ biến không cần sửa JSON tay |
| **TOTAL AVERAGE** | **9.50/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 11.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Run-Scoped Outbound MCP Tool Bundles & Deterministic Layout Solvers theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Kế thừa toàn bộ Bidirectional Open MCP Server & 1-Click Host Installer UI từ Story 11.3 cũ (Ephemeral Tool Forging chuyển về 08.5 theo ADR-007); chuyển hybrid packaging & dependencies ["10.1", "11.1", "08.5"] | /refactor-story |
