---
type: I-Wish Story
id: "01.2"
title: "Story 01.2: Xây dựng crates/spec-registry với rusqlite SQLite WAL (16 Bảng) & 2-Way File Sync"
description: "Khởi tạo 16 bảng SQLite WAL qua refinery migrations với đầy đủ PRAGMA đồng thời; cài đặt đồng bộ 2 chiều nguyên tử giữa SQLite và product/specs/*.md kèm Hash-Gated Watcher Suppression, debounce 50ms chống đọc 0-byte, và kiểm tra StaleFile SHA-256 OCC."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/Story-01.2/story.md"
status: planned
risk_tier: 3
delivery: core
timestamp: "2026-11-02T14:22:00+07:00"
tags:
  - "story-01.2"
  - "epic-01"
  - "uxui-os"
  - "sqlite-wal"
  - "occ-sha256"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-01/evaluation-report.md"
dependencies: []
intra_epic_dependencies: ["01.1"]
provides_apis:
  - "internal:epic-01/story-01.2"
  - "crate:spec-registry"
consumes_apis:
  - "internal:epic-01/story-01.1"
---

# Story 01.2: Xây dựng crates/spec-registry với rusqlite SQLite WAL (16 Bảng) & 2-Way File Sync

- **FR Covered:** [FR-11: 9 Foundational Project Context Specs Synthesis](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md), [FR-12: Versioned Spec Registry & Cross-Project Reuse](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 01: Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/epic.md)
- **Status:** `planned`
- **Risk Tier:** `3`

## 1. Story Description & Context
Xây dựng crate `crates/spec-registry` quản lý cơ sở dữ liệu nhúng `rusqlite` SQLite ở chế độ WAL gồm trọn vẹn **16 bảng** phân vùng cho cả 5 luồng thực thi theo `2.2. database-spec.md`, kết hợp đồng bộ 2 chiều nguyên tử giữa đĩa vật lý (`product/specs/*.md`) và bảng `PROJECT_SPECS` / `SPEC_REVISIONS`. Cung cấp năng lực đọc/ghi `LANE_1_DETERMINISTIC` (0 token LLM) cho `cowok` Orchestrator và `advisor` Meta-Agent, đồng thời bảo vệ dữ liệu tuyệt đối bằng cơ chế kiểm tra `expected_sha256` (`StaleFile` OCC) và chống vòng lặp `notify` watcher.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1, Data=1.5, Integration=1, UI=0.5, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 5.5 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (16-Table SQLite WAL Genesis & Connection Pragmas):** Khởi tạo toàn bộ 16 bảng SQLite (`PROJECTS`, `PROJECT_SPECS`, `SPEC_REVISIONS`, `CONSTRAINT_PROFILES`, `CONSTRAINT_RULES`, `FLOW_GRAPHS`, `FLOW_NODES`, `FLOW_EDGES`, `SCREEN_ARTIFACTS`, `PRESENTATION_DECKS`, `DECK_SLIDES`, `WORDING_SLOTS`, `SKILL_PLUGINS`, `MODEL_TOURNAMENTS`, `BOT_JOBS`, `GATE_REPORTS`) cùng chỉ mục trong một migration transaction duy nhất (`< 15ms` cold start, `KU-01.2`), và cưỡng chế thực thi `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;` trên mọi kết nối.
- **AC-02 (Atomic 2-Way Disk $\leftrightarrow$ SQLite Sync & Provenance Metadata Indexing):** Mọi thao tác ghi spec đều ghi qua file tạm `.tmp.<uuid>` cùng thư mục, gọi `sync_all()` rồi `rename()` nguyên tử vào `product/specs/<spec_key>.md`, đồng thời cập nhật `content_sha256`, `summary_digest`, `provenance_summary_json`, và bản ghi bất biến trong `SPEC_REVISIONS` (kèm `source_project_id` và `source_revision_sha256` phục vụ kế thừa xuyên dự án `BS-01.1`).
- **AC-03 (`[EDGE-CASE]` `EC-E01-01` & `UU-01.1` — Hash-Gated `notify` Watcher Echo Loop Suppression):** Khi `notify` watcher nhận sự kiện thay đổi file từ hệ điều hành (bao gồm FSEvents coalescing trên macOS), nó phải băm `sha256(disk_bytes)` và đối chiếu với `PROJECT_SPECS.content_sha256` trong SQLite; nếu trùng khớp (`disk_sha256 == db_sha256`), sự kiện phải bị bỏ qua ngay lập tức (`NO-OP`), ngăn chặn 100% vòng lặp ghi vô tận.
- **AC-04 (`[EDGE-CASE]` `EC-E01-02` & `EC-E01-05` — Mandatory `expected_sha256` `StaleFile` OCC & Multi-Bot Concurrency):** Mọi lệnh cập nhật spec đã tồn tại (`version >= 1`) bắt buộc phải truyền `expected_sha256`; nếu khác với `content_sha256` hiện tại trong SQLite hoặc trên đĩa, phải từ chối ghi và trả về `HarnessError::StaleFile { spec_key, expected_sha256, actual_sha256 }` mà không làm hỏng dữ liệu khi nhiều Execution Bots đọc/ghi song song.
- **AC-05 (`[EDGE-CASE]` `EC-E01-04`, `UU-01.2` & `AS-01.2` — Transient `0-Byte` IDE Save Debounce & Symlink Jail Guard):** Nếu watcher phát hiện file spec đang có nội dung đột ngột về `0 bytes` (do VS Code/Cursor truncate khi Save), phải chờ debounce `50ms` và đọc lại trước khi đồng bộ (từ chối xóa trắng nội dung nếu không có lệnh xóa tường minh); đồng thời chặn mọi symlink hoặc đường dẫn thoát khỏi `workspace_root` bằng `canonicalize()`.

## 4. Tasks
- [ ] **Task 01.2.1**: Cài đặt `SqlitePool::open_wal()` với đầy đủ 4 `PRAGMA` và migration `V1__initial_16_tables.sql` khởi tạo 16 bảng + indexes (thỏa mãn `AC-01`).
- [ ] **Task 01.2.2**: Cài đặt `SpecRegistry` hỗ trợ đọc/ghi nguyên tử (`.tmp.<uuid>` $\rightarrow$ `sync_all()` $\rightarrow$ `rename()`), lưu vết `SPEC_REVISIONS` và truy vết kế thừa `source_project_id` / `source_revision_sha256` (thỏa mãn `AC-02`).
- [ ] **Task 01.2.3**: Cài đặt `notify` file watcher với cơ chế **Hash-Gated Watcher Suppression** (`disk_sha256 == db_sha256` $\rightarrow$ `NO-OP`) và debounce `50ms` chống đọc file `0-byte` tạm thời (thỏa mãn `AC-03`, `AC-05`).
- [ ] **Task 01.2.4**: Cài đặt chốt chặn `StaleFile` SHA-256 OCC, kiểm tra `canonicalize()` workspace jail, và viết bộ kiểm thử đồng thời đa luồng + chống echo loop (thỏa mãn `AC-03`, `AC-04`, `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 01.2.1` | Migration & PRAGMA inspection test (`cargo test -p spec-registry`) |
| **AC-02** | `Task 01.2.2` | Atomic write & `SPEC_REVISIONS` lineage test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 01.2.3`, `Task 01.2.4` | Watcher echo-loop suppression test (`EC-E01-01`, `UU-01.1`) |
| **AC-04 (`[EDGE-CASE]`)** | `Task 01.2.4` | Concurrent Human+Bot `StaleFile` OCC conflict test (`EC-E01-02`, `EC-E01-05`) |
| **AC-05 (`[EDGE-CASE]`)** | `Task 01.2.3`, `Task 01.2.4` | `0-byte` truncate debounce & symlink escape rejection test (`EC-E01-04`, `AS-01.2`) |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["01.1"]`

### Impacts
- Cung cấp tầng lưu trữ 16 bảng SQLite WAL và `crates/spec-registry` cho `Story 01.3` và toàn bộ các Epics từ `Epic 02` đến `Epic 12`.

### Consumes
- Tiêu thụ các structs/enums (`Project`, `CanonicalSpecKey`, `ProjectSpec`, `HarnessError`) từ `Story 01.1` (`crates/domain`).

### Shared Entities
- Khởi tạo và quản lý trực tiếp cả 16 bảng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_SPECS`, `SPEC_REVISIONS`, `CONSTRAINT_PROFILES`, `CONSTRAINT_RULES`, `FLOW_GRAPHS`, `FLOW_NODES`, `FLOW_EDGES`, `SCREEN_ARTIFACTS`, `PRESENTATION_DECKS`, `DECK_SLIDES`, `WORDING_SLOTS`, `SKILL_PLUGINS`, `MODEL_TOURNAMENTS`, `BOT_JOBS`, `GATE_REPORTS`).

### Cross-Portal
- Áp dụng thống nhất cho mọi thao tác lưu trữ của cả giao diện Human (Web/Desktop Shell) và giao diện Agent (`cowok`, `advisor`, Execution Bots, MCP Gateway).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Đáp ứng đầy đủ `FR-11, FR-12` và 16 bảng trong `2.2. database-spec.md` |
| **2. Data Integrity & State** | `9.6/10` | Chống mất dữ liệu bằng `StaleFile` OCC, `.tmp` atomic rename & `0-byte` debounce |
| **3. Security & Validation** | `9.5/10` | Chặn thoát thư mục bằng `canonicalize()` workspace jail |
| **4. Performance & Scalability** | `9.5/10` | Cold start migration `< 15ms`, truy vấn metadata `< 2ms` với 0 token LLM |
| **5. Error Handling & Recovery** | `9.5/10` | Xử lý triệt để `SQLITE_BUSY`, `STALE_FILE` và FSEvents echo loop |
| **6. Architectural Depth & Leverage** | `9.4/10` | Tách biệt hoàn toàn logic lưu trữ trong `crates/spec-registry` |
| **7. UX Empathy** | `9.4/10` | Cho phép người dùng sửa file Markdown bằng IDE ngoài mà vẫn đồng bộ tức thì |
| **TOTAL AVERAGE** | **9.49/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 01.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-02 | 1.1.0 | Bổ sung AC-03, AC-04, AC-05 và các Edge Cases (`EC-E01-01`, `EC-E01-02`, `EC-E01-04`, `EC-E01-05`, `UU-01.1`, `UU-01.2`, `AS-01.2`, `BS-01.1`) từ kết quả `/evaluate-epic` Epic 01 | cowok Orchestrator |
