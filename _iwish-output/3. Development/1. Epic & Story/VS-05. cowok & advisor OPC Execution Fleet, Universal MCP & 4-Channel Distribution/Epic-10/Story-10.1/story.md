---
type: I-Wish Story
id: "10.1"
title: "Story 10.1: Xây dựng cowok Master Orchestrator & Embedded PTY $UIUX_LIVE_SPEC (<50ms Hot-Apply + Cmd+Z)"
description: "Thiết lập cowok làm Agent điều phối trung tâm duy nhất giao tiếp với người dùng; tích hợp portable-pty tiêm tự động biến $UIUX_LIVE_SPEC cho phép các Execution Agents cập nhật thiết kế trực tiếp lên Canvas trong <50ms kèm hoàn tác 1 bước (Cmd+Z)."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/Story-10.1/story.md"
status: refactored
risk_tier: 3
delivery: core
adr_ref: "ADR-006"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-10.1"
  - "epic-10"
  - "uxui-os"
  - "rust-day-1"
  - "3-lane-router"
  - "mpsc-sqlite"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["04.4", "09.2", "11.1"]
provides_apis:
  - "POST /api/v1/orchestrator/chat"
  - "POST /api/v1/patch"
consumes_apis:
  - "POST /api/v1/decision/route"
  - "POST /api/v1/bot-fleet/execute"
---

# Story 10.1: `cowok` Master Orchestrator, `3-Lane Router`, Single MPSC SQLite Write Actor & `$UIUX_LIVE_SPEC` `<50ms` Hot-Apply

- **FR Covered:** [FR-24, FR-25: cowok Master Orchestrator, 3-Lane Router, MPSC SQLite Write Actor & $UIUX_LIVE_SPEC (<50ms + Cmd+Z)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 10: cowok Master Orchestrator, advisor Meta-Agent, OPC Execution Fleet & Design Tournament](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Thiết lập `cowok` làm Orchestrator trung tâm duy nhất giao tiếp với người dùng qua **`3-Lane Router`** (**Lane 0**: Deterministic AST/CLI `<50ms` zero LLM cost; **Lane 1**: Decision Cascade System-1 route `Story 09.2`; **Lane 2**: Full OPC Execution Fleet qua `crates/bot-fleet`), điều phối mọi giao dịch ghi SQLite WAL đi qua **Single MPSC SQLite Write Actor** (`busy_timeout = 5000`, `0` lỗi `SQLITE_BUSY` dưới tải song song), và tích hợp `portable-pty` tiêm `$UIUX_LIVE_SPEC` cho phép áp dụng trực tiếp thiết kế lên Canvas trong `<50ms` kèm hoàn tác 1 bước (`Cmd+Z`) và `Unified Asset Library` có `provenance backlink`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Giao thức Single MPSC SQLite Write Actor (`0` blocking disk/HTTP I/O inside actor, bounded backpressure `429 Retry-After`) và Atomic `$UIUX_LIVE_SPEC` (`doc-patch.jsonl`).

## 3. Acceptance Criteria (AC)
- **AC-01 (`3-Lane Router` & `cowok` Single-Voice Orchestrator):** Định tuyến thông minh mọi yêu cầu từ `POST /api/v1/orchestrator/chat` qua 3 làn: `Lane 0` xử lý xác định (`lint_fix`, đổi token, resize) trong `<50ms` ở chi phí 0 token; `Lane 1` gọi `Story 09.2` Decision Cascade phân loại intent & chọn skill; `Lane 2` điều phối swarm worker qua `Story 11.1` `crates/bot-fleet`.
- **AC-02 (Single MPSC SQLite Write Actor — Zero `SQLITE_BUSY` & `$UIUX_LIVE_SPEC` `<50ms` + `Cmd+Z`):** Toàn bộ ghi vào SQLite WAL từ Orchestrator, Worker, QC và Decision Audit phải đi qua **Single MPSC SQLite Write Actor** (`PRAGMA busy_timeout = 5000`, tuyệt đối không chạy blocking disk/HTTP I/O trong vòng lặp actor để không gây deadlock cho các caller Wave 1/2), đạt `0` lỗi `SQLITE_BUSY` dưới bài test tranh chấp đa luồng; giữ vững `<50ms` hot-apply qua `$UIUX_LIVE_SPEC` / `doc-patch.jsonl` (`O_NOFOLLOW` + cap `<= 512 KB`) và rollback tức thì `Cmd+Z`.
- **AC-03 ([EDGE-CASE] Resilience, Invalid Patch Rejection & Actor Backpressure Guard):** Chặn và loại bỏ bất kỳ JSON/patch hỏng, symlink (`O_NOFOLLOW`) hoặc file vượt `512 KB` từ Execution Agent gửi vào `$UIUX_LIVE_SPEC` mà không làm rớt trạng thái ổn định trước đó; nếu hàng đợi MPSC vượt ngưỡng an toàn, áp dụng bounded backpressure trả mã `429 Retry-After` sạch thay vì OOM hoặc drop ghi.
- **AC-04 (Zero-Port Live Patch Bridge `<50ms` & Unified Asset Library Provenance Backlinks):** `Story 10.1` cung cấp tầng transport actor & `doc-patch.jsonl` định tuyến vào bộ hòa giải DOM `Story 05.4` (`POST /api/v1/patch`), kết nối với Thư viện Tài sản Thống nhất khử trùng qua `content_hash` (`V16a cas_blobs`) kèm `provenance backlink` nhảy ngược về đúng phiên chat / node đã sinh ra tài sản.

## 4. Tasks
- [ ] **Task 10.1.1**: Xây dựng `3-Lane Router` (Lane 0 Deterministic / Lane 1 Decision Cascade / Lane 2 OPC Fleet) và endpoint `POST /api/v1/orchestrator/chat` trong `crates/harness-daemon` (thỏa mãn `AC-01`).
- [ ] **Task 10.1.2**: Cài đặt **Single MPSC SQLite Write Actor** (`busy_timeout = 5000`) thay thế các kết nối ghi trực tiếp rải rác, đảm bảo `0 SQLITE_BUSY` (thỏa mãn `AC-02`, `AC-03`).
- [ ] **Task 10.1.3**: Cài đặt `portable-pty` `$UIUX_LIVE_SPEC` Zero-Port Live Patch Bridge (`<50ms` + `Cmd+Z`) và Unified Asset Library `provenance backlink` nối `POST /api/v1/patch` (thỏa mãn `AC-02`, `AC-04`).
- [ ] **Task 10.1.4**: Viết bài test căng tải MPSC SQLite Write Actor (50 writer tasks song song `0 SQLITE_BUSY`), 3-Lane routing benchmark và invalid JSON patch `[EDGE-CASE]` rejection (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 10.1.1`, `Task 10.1.4` | 3-Lane Router Latency & Cost Routing Test (`cargo test`) |
| **AC-02** | `Task 10.1.2`, `Task 10.1.3`, `Task 10.1.4` | 50-Writer Concurrent MPSC `0 SQLITE_BUSY` Stress Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 10.1.2`, `Task 10.1.3`, `Task 10.1.4` | Corrupted `$UIUX_LIVE_SPEC` & Backpressure Regression Test |
| **AC-04** | `Task 10.1.3`, `Task 10.1.4` | Live Patch Bridge `<50ms` & Provenance Backlink Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["04.4", "09.2", "11.1"]

### Impacts
- Cung cấp backbone điều phối và ghi SQLite an toàn cho `Story 10.2` (Swarm & QC), `Story 10.3` (Tournament) và `Story 11.2` (Open MCP `apply_batch`).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["04.4", "09.2", "11.1"] (Flow & Prompt Registry, 5-Stage Decision Cascade, `crates/bot-fleet` BYOS/BYOK execution).

### Shared Entities
- Điều phối luồng ghi thống nhất cho `PROJECTS`, `SPEC_DOCUMENTS`, `FLOW_NODES`, `FLEET_TASKS`, `cas_blobs`, `decision_audit_log` trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | `3-Lane Router` định tuyến rõ giữa `0` token AST, Decision & Fleet |
| **2. Data Integrity & State** | `9.6/10` | Single MPSC SQLite Write Actor loại bỏ hoàn toàn `SQLITE_BUSY` |
| **3. Security & Validation** | `9.5/10` | Schema gate trước khi apply `$UIUX_LIVE_SPEC` |
| **4. Performance & Scalability** | `9.6/10` | Lane 0 & Live Patch Bridge `<50ms` |
| **5. Error Handling & Recovery** | `9.4/10` | Bounded queue backpressure + 1-step `Cmd+Z` rollback |
| **6. Architectural Depth & Leverage** | `9.5/10` | Tuân thủ ADR-006 |
| **7. UX Empathy** | `9.5/10` | Patch hiện ngay lên Canvas theo thời gian thực |
| **TOTAL AVERAGE** | **9.51/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 10.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Zero-Port Live Patch Bridge <50ms & Unified Asset Library (GAP-08) theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Bổ sung 3-Lane Router (Lane 0/1/2), Single MPSC SQLite Write Actor (busy_timeout=5000, 0 SQLITE_BUSY) và cập nhật dependencies ["04.4", "09.2", "11.1"] theo ADR-006 | /refactor-story |
