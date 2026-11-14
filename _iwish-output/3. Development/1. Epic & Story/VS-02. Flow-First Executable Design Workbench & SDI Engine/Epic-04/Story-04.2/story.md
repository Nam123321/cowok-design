---
type: I-Wish Story
id: "04.2"
title: "Story 04.2: SQLite V5 Schema Convergence, Streaming Dual-Ingress Vite BFF & Fail-Closed FS Folder Picker"
description: "Triển khai lát cắt hạ tầng SSOT dùng chung (FR-02, FR-03, FR-06 SSOT Enabler): nâng cấp SQLite bằng migration V5__epic04_complete_contracts.sql với PRAGMA table_info guards vá V2->V4 và thêm bảng chuẩn hóa (agent_settings, mcp_servers, storyboards, node_chat_sessions, element_comments, screen_fixtures, execution_events), nâng lockfile CURRENT_SCHEMA_VERSION = 5; xóa 100% new Map() trong src/app/api/**; sửa vite-bff-plugin.ts#sendWebResponse sang streaming Readable cho SSE; Multi-Project CRUD/Trash Journal; và thay showDirectoryPicker bằng Daemon FS Browser jail + PathTokenResponse fail-closed."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.2/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-13T12:10:00+07:00"
tags:
  - "story-04.2"
  - "epic-04"
  - "uxui-os"
  - "sqlite-v5"
  - "streaming-bff"
  - "fs-jail"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-04/evaluation-report.md"
dependencies: ["04.1"]
provides_apis:
  - "internal:epic-04/story-04.2"
  - "GET /api/v1/projects"
  - "POST /api/v1/projects"
  - "DELETE /api/v1/projects/:id"
  - "POST /api/v1/projects/:id/restore"
  - "GET /api/v1/fs/browse"
  - "POST /api/v1/fs/path-token"
  - "POST /api/v1/fs/promote"
consumes_apis:
  - "GET /api/v1/health"
  - "GET /api/v1/projects/:id/flow"
---

# Story 04.2: SQLite V5 Schema Convergence, Streaming Dual-Ingress Vite BFF & Fail-Closed FS Folder Picker

- **FR Covered:** [FR-02: Interactive Multi-Portal Flow Graph, FR-03: State-Hydrated Live Preview, FR-06: Element-Anchored Comments (SSOT Persistence & Dual-Ingress BFF Enabler)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, Onboarding Doctor & Persistent Node Execution](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-04/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Trong đợt tái cấu trúc từ đánh giá Epic 04 (CS ban đầu quá tải khi gộp cả Onboarding CLI Doctor + BYOK Vault lẫn SQLite & BFF), Story 04.2 được cô đọng thành lát cắt **Persistence & Dual-Ingress Kernel (`CS = 5.5`)**, đồng thời thay thế các giả định sai của bản V4 trước đó:
1. **Nâng cấp từ V4 lỗi thời lên V5 Idempotent (`V5__epic04_complete_contracts.sql`):** Migration V2 đã tạo bảng `chat_sessions` thiếu cột `archived_at`, khiến `CREATE TABLE IF NOT EXISTS` ở V4 âm thầm bỏ qua cột này khi upgrade; đồng thời `lockfile.rs` vẫn ghim `CURRENT_SCHEMA_VERSION = 3`. V5 dùng `PRAGMA table_info` trong `pool.rs` để vá cột thiếu (`chat_sessions.archived_at`, `tenant_id`), nâng `CURRENT_SCHEMA_VERSION = 5`, và chuẩn hóa thêm 7 bảng (`agent_settings`, `mcp_servers`, `storyboards`, `node_chat_sessions`, `element_comments`, `screen_fixtures`, `execution_events`).
2. **Sửa lỗi SSE buffering trong Vite BFF:** `src/lib/vite-bff-plugin.ts#sendWebResponse` hiện gọi `await webRes.text()`, làm treo toàn bộ SSE stream. Story 04.2 thay bằng `Readable` pass-through thực cho `text/event-stream`, xóa 100% `new Map()` trong `src/app/api/**`, và gắn `X-Cowok-Session-Token` qua `strict-loopback`.
3. **Thay thế `showDirectoryPicker` mù đường dẫn:** Trình duyệt không cấp absolute OS path qua `showDirectoryPicker()`, và `WorkbenchShell.tsx` từng làm rơi `data.token`. Story 04.2 thay bằng Daemon FS Browser (`/api/v1/fs/browse`) kèm `is_system_or_root_path` jail -> `POST /path-token` -> `POST /promote` giữ token đầy đủ và **fail closed** (tuyệt đối không fallback `./projects/*`).

*(Lưu ý: Day-0 Onboarding CLI Doctor & BYOK Vault được tách gọn sang `Story 04.3`, còn State-Hydrated Live Preview UI thuộc `Story 04.5`.)*

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.0, Data=1.5, Integration=1.0, UI=0.5, Security=1.0, Unknowns=0.5 $\rightarrow$ `CS = 5.5 < 7`, đạt ngưỡng an toàn).

## 3. Acceptance Criteria (AC)
- **AC-01 (SQLite `V5__epic04_complete_contracts.sql` Additive Migration & `CURRENT_SCHEMA_VERSION = 5`):** Thực thi migration V5 an toàn qua `PRAGMA table_info` guards trong `crates/spec-registry/src/pool.rs` để vá các cột thiếu khi upgrade từ V2/V3/V4 (`chat_sessions.archived_at`, `tenant_id` trên mọi bảng V4) và tạo 7 bảng chuẩn (`agent_settings`, `mcp_servers`, `storyboards`, `node_chat_sessions`, `element_comments`, `screen_fixtures`, `execution_events`) bảo đảm mọi cột nghiệp vụ đầy đủ không dùng blob JSON; đồng bộ nâng `crates/harness-daemon/src/lockfile.rs` `CURRENT_SCHEMA_VERSION = 5` với hàng đợi ghi đơn (Single-Writer MPSC) + `busy_timeout = 5000ms` và retry có jitter.
- **AC-02 (Zero `new Map()` Destruction in `src/app/api/**` & Streaming SSE Vite BFF):** Xóa 100% biến lưu trạng thái bằng `new Map()` khỏi `src/app/api/**` (`comments`, `fixtures`, `sdi/intent`, `proposals`); định tuyến mọi request qua Dual-Ingress (`Next/Vite BFF -> Axum Rust Daemon`) sử dụng `strict-loopback` + `X-Cowok-Session-Token`; và nâng cấp `src/lib/vite-bff-plugin.ts#sendWebResponse` + `daemon-client.ts` phát hiện `text/event-stream` để pipe luồng `Readable` không buffer `webRes.text()`.
- **AC-03 (Multi-Project CRUD, Workspace Switching, Trash Journal & F5 Hydration):** Hỗ trợ tạo, đổi dự án (`project_code`), xóa mềm vào Trash Journal, khôi phục (`POST /restore`), và tự động nạp lại đúng dự án/phiên hoạt động sau khi nhấn `F5` (reload trình duyệt) trực tiếp từ SQLite V5 qua `src/api/projects.ts` (zero `localStorage` primary state).
- **AC-04 (`[EDGE-CASE]` `EC-PATH-HMAC-002`: Daemon-Assisted FS Folder Picker, System-Root Jail & Retained `PathTokenResponse`):** Xóa bỏ giả định trình duyệt `window.showDirectoryPicker()` (vốn chỉ trả tên thư mục, mất đường dẫn tuyệt đối) và thay bằng hộp thoại chọn thư mục được Daemon duyệt qua `/api/v1/fs/browse` bị khóa chặt bởi `is_system_or_root_path` jail; khi chọn thư mục ngoài sandbox mặc định, client lấy và truyền giữ nguyên HMAC-signed `PathTokenResponse` vào `POST /api/v1/fs/promote`, **fail closed** (trả `400/403` tường minh) nếu thiếu chữ ký hoặc đường dẫn nguy hiểm — cấm tuyệt đối fallback im lặng về `./projects/*`.

## 4. Tasks
- [ ] **Task 04.2.1**: Viết `crates/spec-registry/migrations/V5__epic04_complete_contracts.sql`, cập nhật `pool.rs` với `PRAGMA table_info` idempotent column guards cho V2→V5, nâng `lockfile.rs` `CURRENT_SCHEMA_VERSION = 5` và bổ sung test nâng cấp từ DB V2/V3/V4 (thỏa mãn `AC-01`).
- [ ] **Task 04.2.2**: Xóa sạch `new Map()` trong `src/app/api/**`; sửa `vite-bff-plugin.ts#sendWebResponse` và `daemon-client.ts` hỗ trợ streaming `Readable` SSE pass-through kèm `strict-loopback` + `X-Cowok-Session-Token` (thỏa mãn `AC-02`).
- [ ] **Task 04.2.3**: Xây dựng CRUD Multi-Project, Trash Journal (`delete/restore`), và `src/api/projects.ts` phục hồi ngữ cảnh sau F5 từ SQLite V5 (thỏa mãn `AC-03`).
- [ ] **Task 04.2.4**: Cài đặt Daemon FS Browser (`/fs/browse` + `is_system_or_root_path` jail), sửa `WorkbenchShell.tsx` để giữ HMAC `PathTokenResponse` gửi qua `POST /fs/promote` fail-closed không fallback `./projects/*` (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Verification Method | Status |
| :--- | :--- | :--- | :--- | :--- |
| **AC-01** | SQLite V5 Additive Upgrade & Lockfile Schema v5 | `Task 04.2.1` | V2→V3→V4→V5 Upgrade Migration & Lockfile Test | pending |
| **AC-02** | Zero `new Map()` & Streaming SSE Vite BFF | `Task 04.2.2` | BFF SSE Streaming & Zero-Map AST Test | pending |
| **AC-03** | Multi-Project CRUD, Trash Journal & F5 Hydration | `Task 04.2.3` | Project Switch & F5 State Persistence Test | pending |
| **AC-04** | [EDGE-CASE] Daemon FS Browser & Fail-Closed PathToken | `Task 04.2.4` | Traversal / System-Root Jail & HMAC Token Test | pending |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["04.1"]`

### Impacts
- Cung cấp tầng lưu trữ SQLite `V5`, Dual-Ingress Streaming BFF và Multi-Project Workspace/FS Boundary SSOT cho `Story 04.3`, `Story 04.4`, `Story 04.5` và toàn bộ `Epic 05..12`.

### Consumes
- Tiêu thụ cấu trúc Workbench (`WorkbenchShell`, `MainSidebar`) và lớp bảo mật `daemon_auth::verify_hmac_v2` đã vá ở `Story 04.1`.

### Shared Entities
- Sở hữu schema upgrade `V5`: `AgentSetting` (`agent_settings`), `McpServer` (`mcp_servers`), `Storyboard` (`storyboards`), `NodeChatSession` (`node_chat_sessions`), `ScreenFixture` (`screen_fixtures`), `ExecutionEvent` (`execution_events`), cùng mở rộng `Project`, `ChatSession`, `ChatMessage`, `ChatMessageNodeScope`, `NodeExecutionJob`, `ChatArtifactSnapshot`, `ElementComment`.

### Cross-Portal
- Cô lập dữ liệu multi-project và multi-tenant (`tenant_id`) xuyên suốt mọi Portal Group, bảo đảm không rò rỉ ngữ cảnh khi chuyển dự án.

## 7. QA Simulator Guardian Scorecard
| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Giải quyết đúng nút thắt schema V2→V5, SSE BFF streaming & multi-project hydration |
| **2. Data Integrity & State** | `9.7/10` | `PRAGMA table_info` idempotent migration + `CURRENT_SCHEMA_VERSION = 5` fail-closed |
| **3. Security & Validation** | `9.7/10` | Xóa bỏ `showDirectoryPicker` mù đường dẫn & `./projects/*` fallback, ép FS jail + HMAC |
| **4. Performance & Scalability** | `9.5/10` | Pass-through `Readable` stream `0`-buffering cho SSE và Single-Writer MPSC 5000ms |
| **5. Error Handling & Recovery** | `9.5/10` | Trash Journal soft-delete/restore và retry jitter khi `sqlite3_busy` |
| **6. Architectural Depth & Leverage** | `9.6/10` | Hợp nhất hoàn toàn về Rust SQLite WAL, dẹp 100% RAM `new Map()` trong `app/api/**` |
| **7. UX Empathy** | `9.5/10` | Đổi dự án tức thì không mất dấu sau F5, chọn thư mục thật minh bạch |
| **TOTAL AVERAGE** | **9.59/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log
| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 04.2 (State-Hydrated Preview nguyên thủy) | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | Tái cấu trúc thành SQLite V4 SSOT + Day-0 Doctor + Multi-Project BFF | /refactor-story |
| 2026-11-13 | 3.0.0 | [evaluate-epic Deep Mode] Tách Onboarding/BYOK sang 04.3 (CS 5.5 < 7); thay V4 bằng `V5__epic04_complete_contracts.sql` (`CURRENT_SCHEMA_VERSION=5`, vá `archived_at`), sửa SSE buffering `vite-bff-plugin`, thay `showDirectoryPicker` bằng Daemon FS Browser jail; sửa FR IDs về `FR-02/03/06` | Party Mode Council & Architect |
