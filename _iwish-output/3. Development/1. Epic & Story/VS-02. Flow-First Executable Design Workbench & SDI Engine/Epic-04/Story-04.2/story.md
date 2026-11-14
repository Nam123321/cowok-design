---
type: I-Wish Story
id: "04.2"
title: "Story 04.2: SQLite V7 Schema Convergence (CAS/Seq/Vault), Streaming Dual-Ingress Vite BFF & Managed Root / FS Jail"
description: "Triển khai lát cắt hạ tầng SSOT dùng chung (FR-02, FR-03, FR-06 SSOT Enabler): giữ nguyên V5/V6 đang checksum-locked trên master (pool.rs:569, lockfile.rs:14) và bổ sung migration cộng dồn V7__epic04_cas_seq_and_vault_convergence.sql (nâng CURRENT_SCHEMA_VERSION = 7) gồm PRAGMA table_info guards, execution_events.project_seq, rebuild 12-bước node_execution_jobs ('PAUSED'|'INTERRUPTED' + composite_scope_key + daemon_nonce + updated_at), element_comments.artifact_sha256 + anchor_kind, agent_settings (permission_mode, vault_mode, onboarding_done), node_chat_sessions (composite_scope_key, unread_count, last_message_id); giữ regression BFF SSE Readable.fromWeb & zero-Map comments/fixtures trên master (381dd87) đồng thời xóa nốt RAM Map ở sdi/intent & proposals; hoàn thiện Multi-Project CRUD/Trash Journal qua src/api/harness-daemon.ts (apiHarnessDaemon); và thay showDirectoryPicker bằng thư mục quản lý 1-click ~/CowokProjects/<slug> + Daemon FS Browser /api/v1/fs/browse kèm O_NOFOLLOW + post-canonicalize() jail + HMAC PathToken (/projects/:id/path-token, /verify-path-token, /promote) fail-closed."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.2/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-14T08:47:00+07:00"
tags:
  - "story-04.2"
  - "epic-04"
  - "uxui-os"
  - "sqlite-v7"
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
  - "POST /api/v1/projects/:id/path-token"
  - "POST /api/v1/projects/:id/verify-path-token"
  - "POST /api/v1/projects/:id/promote"
consumes_apis:
  - "GET /api/v1/health"
  - "GET /api/v1/projects/:id/flow/snapshot"
---

# Story 04.2: SQLite V7 Schema Convergence (CAS/Seq/Vault), Streaming Dual-Ingress Vite BFF & Managed Root / FS Jail

- **FR Covered:** [FR-02: Interactive Multi-Portal Flow Graph, FR-03: State-Hydrated Live Preview, FR-06: Element-Anchored Comments (SSOT Persistence & Dual-Ingress BFF Enabler)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, Onboarding Doctor & Persistent Node Execution](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-04/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Trong đợt tái cấu trúc & đối chiếu `master` của Epic 04, Story 04.2 đóng vai trò lát cắt **Persistence V7, Dual-Ingress Kernel & Managed FS Boundary (`CS = 5.5`)**, cập nhật thẳng theo thực tế `V5/V6` đã checksum-locked trên `master`:
1. **Nâng cấp cộng dồn V7 (`V7__epic04_cas_seq_and_vault_convergence.sql`, `CURRENT_SCHEMA_VERSION = 7`):** Trên `master`, `V5` và `V6__epic04_complete_contracts.sql` đã triển khai và khóa checksum (`pool.rs:569`, `lockfile.rs:14`), nên tuyệt đối **không** sửa lại file V5/V6 cũ. Thay vào đó V7 thực thi nâng cấp cộng dồn idempotent với `PRAGMA table_info` guards cho `chat_sessions.archived_at` & `chat_message_node_scopes.tenant_id`, thêm `execution_events.project_seq` (`UNIQUE(project_id, project_seq)`) + `composite_scope_key`, rebuild 12-bước bảng `node_execution_jobs` (bổ sung `'PAUSED' | 'INTERRUPTED'`, `composite_scope_key`, `daemon_nonce`, `updated_at`), thêm `element_comments.artifact_sha256` + `anchor_kind`, `agent_settings` (`permission_mode`, `vault_mode`, `onboarding_done`), và `node_chat_sessions` (`composite_scope_key`, `unread_count`, `last_message_id`), đồng thời nâng `lockfile.rs` `CURRENT_SCHEMA_VERSION = 7`.
2. **Bảo toàn SSE Streaming & dọn nốt RAM `new Map()` trong Vite BFF:** Commit `381dd87` trên `master` đã nâng `vite-bff-plugin.ts#sendWebResponse` dùng `Readable.fromWeb` và chuyển route comments/fixtures khỏi RAM `Map`. Story 04.2 giữ kiểm thử hồi quy cho phần này, đồng thời xóa sạch `new Map()` còn sót ở `sdi/intent` & `proposals`, nối đủ BFF `DELETE /api/projects/:id` & `POST /api/projects/:id/restore`, và gắn `X-Cowok-Session-Token` qua `strict-loopback`.
3. **Thay `showDirectoryPicker` bằng Managed Root 1-Click + Hardened Promote Jail:** Bỏ hoàn toàn `window.showDirectoryPicker()`, mặc định 1-click tạo workspace an toàn tại `~/CowokProjects/<slug>` (`^[a-z0-9][a-z0-9_-]{0,62}$`) và dùng `GET /api/v1/fs/browse` khi trỏ codebase sẵn có; đồng thời gia cố `handle_promote_scratch` (`legacy_sessions.rs:487-532`) với **`O_NOFOLLOW` + danh sách chặn sau `canonicalize()`** (`~/.cowok-design`, `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.kube`, `/etc`, `/private`) và **HMAC `PathToken` đã xác thực `(daemon_nonce, canonical_path, dev_ino)`** — **fail closed** (tuyệt đối không fallback `./projects/*`). Mọi lời gọi client đi qua `src/api/harness-daemon.ts` (`apiHarnessDaemon`).

*(Lưu ý: Day-0 Onboarding CLI Doctor & BYOK Vault thuộc `Story 04.3`, Keyed Parallel Coordinator thuộc `Story 04.4`, và State-Hydrated Live Preview UI thuộc `Story 04.5`.)*

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.0, Data=1.5, Integration=1.0, UI=0.5, Security=1.0, Unknowns=0.5 $\rightarrow$ `CS = 5.5 < 7`, đạt ngưỡng an toàn).

## 3. Acceptance Criteria (AC)
- **AC-01 (SQLite `V7__epic04_cas_seq_and_vault_convergence.sql` Additive Migration & `CURRENT_SCHEMA_VERSION = 7`):** Không sửa checksum V5/V6 đã phát hành; tạo migration V7 với `PRAGMA table_info` guards cho `chat_sessions.archived_at` & `chat_message_node_scopes.tenant_id`, thêm `execution_events.project_seq` (`UNIQUE(project_id, project_seq)`) + `composite_scope_key`, rebuild 12-bước bảng `node_execution_jobs` (`'PAUSED' | 'INTERRUPTED'`, `composite_scope_key`, `daemon_nonce`, `updated_at`), thêm `element_comments.artifact_sha256` + `anchor_kind`, `agent_settings` (`permission_mode`, `vault_mode`, `onboarding_done`), và `node_chat_sessions` (`composite_scope_key`, `unread_count`, `last_message_id`); nâng `crates/harness-daemon/src/lockfile.rs` `CURRENT_SCHEMA_VERSION = 7` với Single-Writer MPSC + `busy_timeout = 5000ms` và retry jitter.
- **AC-02 (Zero `new Map()` in `src/app/api/**`, BFF Regression & Project Delete/Restore Routes):** Bảo đảm hồi quy `vite-bff-plugin.ts#sendWebResponse` `Readable.fromWeb` SSE streaming và comments/fixtures trên `master`; xóa 100% biến `new Map()` còn sót khỏi `src/app/api/**` (`sdi/intent`, `proposals`); bổ sung BFF forwarder `DELETE /api/projects/:id` & `POST /api/projects/:id/restore`; mọi request đi qua `Dual-Ingress (BFF -> Axum Daemon)` dùng `strict-loopback` + `X-Cowok-Session-Token` (với `POST /api/auth/local-session` yêu cầu capability proof + bound Host port/TTL eviction và mọi `proxyRoute` mutating trong `vite-bff-routes.ts` bảo toàn JSON payload).
- **AC-03 (Multi-Project CRUD, Trash Journal, F5 Hydration via `src/api/harness-daemon.ts`):** Hỗ trợ tạo, đổi dự án (`project_code`), xóa mềm vào Trash Journal, khôi phục (`POST /api/v1/projects/:id/restore`), và tự động nạp lại đúng dự án/phiên sau `F5` trực tiếp từ SQLite V7 thông qua canonical SDK **`src/api/harness-daemon.ts` (`apiHarnessDaemon`)** (zero `localStorage` primary state).
- **AC-04 (`[EDGE-CASE]` `EC-PATH-HMAC-002`: 1-Click `~/CowokProjects/<slug>`, Daemon FS Browser & `O_NOFOLLOW` / `canonicalize()` Promote Jail):** Xóa bỏ hoàn toàn `window.showDirectoryPicker()`; cấp mặc định 1-click thư mục quản lý `~/CowokProjects/<slug>` (regex `^[a-z0-9][a-z0-9_-]{0,62}$`) cùng `GET /api/v1/fs/browse` cho codebase có sẵn; nâng cấp `POST /api/v1/projects/:id/path-token`, `POST /api/v1/projects/:id/verify-path-token`, và `handle_promote_scratch` (`POST /api/v1/projects/:id/promote` tại `legacy_sessions.rs:487-532`) bắt buộc kiểm tra **`O_NOFOLLOW`**, **blocklist sau `canonicalize()`** (`~/.cowok-design`, `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.kube`, `/etc`, `/private`), và **HMAC `PathToken` ràng buộc `(daemon_nonce, canonical_path, dev_ino)`**, **fail closed** (`400/403`) — cấm tuyệt đối fallback im lặng về `./projects/*`.

## 4. Tasks
- [ ] **Task 04.2.1**: Viết `crates/spec-registry/migrations/V7__epic04_cas_seq_and_vault_convergence.sql`, cập nhật `pool.rs` với `PRAGMA table_info` idempotent guards + 12-step `node_execution_jobs` rebuild, nâng `lockfile.rs` `CURRENT_SCHEMA_VERSION = 7` và bổ sung test nâng cấp từ V2..V6 -> V7 (thỏa mãn `AC-01`).
- [ ] **Task 04.2.2**: Xóa nốt `new Map()` trong `src/app/api/**` (`sdi/intent`, `proposals`); bổ sung BFF `DELETE /api/projects/:id` & `POST /api/projects/:id/restore`; khóa regression test cho `vite-bff-plugin.ts#sendWebResponse` `Readable.fromWeb` SSE + `strict-loopback` + `X-Cowok-Session-Token` (thỏa mãn `AC-02`).
- [ ] **Task 04.2.3**: Hoàn thiện CRUD Multi-Project, Trash Journal (`DELETE/POST restore`), và phục hồi ngữ cảnh sau F5 từ SQLite V7 qua `src/api/harness-daemon.ts` (`apiHarnessDaemon`) (thỏa mãn `AC-03`).
- [ ] **Task 04.2.4**: Loại bỏ `showDirectoryPicker()`, triển khai mặc định `~/CowokProjects/<slug>` + `GET /api/v1/fs/browse`, và gia cố `handle_promote_scratch` (`legacy_sessions.rs:487-532`) với `O_NOFOLLOW`, post-`canonicalize()` jail và HMAC `PathToken` `(daemon_nonce, canonical_path, dev_ino)` fail-closed (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Verification Method | Status |
| :--- | :--- | :--- | :--- | :--- |
| **AC-01** | SQLite V7 Convergence & Lockfile `CURRENT_SCHEMA_VERSION = 7` | `Task 04.2.1` | V2..V6->V7 Upgrade Migration & Lockfile v7 Test | pending |
| **AC-02** | Zero `new Map()` (`sdi/intent`, `proposals`), Restore BFF & SSE Regression | `Task 04.2.2` | BFF SSE `Readable.fromWeb`, Restore & Zero-Map AST Test | pending |
| **AC-03** | Multi-Project CRUD, Trash Journal & F5 Hydration (`apiHarnessDaemon`) | `Task 04.2.3` | Project Switch, Restore & F5 Persistence Test | pending |
| **AC-04** | [EDGE-CASE] `~/CowokProjects/<slug>`, FS Browse & `O_NOFOLLOW` PathToken Jail | `Task 04.2.4` | Symlink `O_NOFOLLOW`, Canonicalize Blocklist & HMAC Token Test | pending |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["04.1"]`

### Impacts
- Cung cấp tầng lưu trữ SQLite `V7`, Dual-Ingress Streaming BFF (`Readable.fromWeb` + `DELETE/POST restore`) và Multi-Project Workspace / `O_NOFOLLOW` FS Jail SSOT cho `Story 04.3`, `Story 04.4`, `Story 04.5` và toàn bộ `Epic 05..12`.

### Consumes
- Tiêu thụ cấu trúc Workbench (`WorkbenchShell`, `MainSidebar`) và baseline `VerifiedLegacyIngress` từ `Story 04.1`.

### Shared Entities
- Sở hữu schema convergence `V7` trên `agent_settings`, `mcp_servers`, `storyboards`, `node_chat_sessions`, `screen_fixtures`, `execution_events` (`project_seq`, `composite_scope_key`), `node_execution_jobs` (12-step `'PAUSED'|'INTERRUPTED'`), `element_comments` (`artifact_sha256`, `anchor_kind`), `chat_sessions` (`archived_at`), `chat_message_node_scopes` (`tenant_id`), cùng `projects`.

### Cross-Portal
- Cô lập dữ liệu multi-project và multi-tenant (`tenant_id`) xuyên suốt mọi Portal Group, bảo đảm không rò rỉ ngữ cảnh khi chuyển dự án.

## 7. QA Simulator Guardian Scorecard
| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Giải quyết đúng delta `V7` cộng dồn trên `V6`, SSE regression + BFF restore & `apiHarnessDaemon` |
| **2. Data Integrity & State** | `9.8/10` | `PRAGMA table_info` idempotent migration + 12-step rebuild + `CURRENT_SCHEMA_VERSION = 7` |
| **3. Security & Validation** | `9.8/10` | `O_NOFOLLOW` + post-`canonicalize()` jail + HMAC `PathToken` `(daemon_nonce, canonical_path, dev_ino)` |
| **4. Performance & Scalability** | `9.5/10` | Pass-through `Readable.fromWeb` `0`-buffering cho SSE và Single-Writer MPSC `5000ms` |
| **5. Error Handling & Recovery** | `9.6/10` | Trash Journal `DELETE/restore` và retry jitter khi `sqlite3_busy` |
| **6. Architectural Depth & Leverage** | `9.7/10` | Hợp nhất về Rust SQLite WAL V7, giữ bất biến checksum V5/V6 & canonical `apiHarnessDaemon` |
| **7. UX Empathy** | `9.6/10` | 1-click `~/CowokProjects/<slug>` không phiền hộp thoại hệ thống, đổi dự án tức thì sau F5 |
| **TOTAL AVERAGE** | **9.66/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log
| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 04.2 (State-Hydrated Preview nguyên thủy) | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | Tái cấu trúc thành SQLite V4 SSOT + Day-0 Doctor + Multi-Project BFF | /refactor-story |
| 2026-11-13 | 3.0.0 | [evaluate-epic Deep Mode] Tách Onboarding/BYOK sang 04.3; nâng SQLite V5, sửa SSE buffering, thay `showDirectoryPicker` bằng FS Jail | Party Mode Council & Architect |
| 2026-11-14 | 3.1.0 | [evaluate-epic Report Sync] Đổi thành migration cộng dồn `V7__epic04_cas_seq_and_vault_convergence.sql` (`CURRENT_SCHEMA_VERSION = 7`, giữ nguyên checksum V5/V6 trên `master`); khóa SSE `Readable.fromWeb` regression & dọn nốt `sdi/intent`/`proposals`; dùng canonical `src/api/harness-daemon.ts`, `~/CowokProjects/<slug>`, `/projects/:id/{path-token,verify-path-token,promote}` kèm `O_NOFOLLOW` + post-`canonicalize()` jail | /evaluate-epic Council & Architect |
