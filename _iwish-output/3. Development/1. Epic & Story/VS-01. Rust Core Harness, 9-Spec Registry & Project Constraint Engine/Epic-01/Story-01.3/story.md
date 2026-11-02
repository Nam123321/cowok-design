---
type: I-Wish Story
id: "01.3"
title: "Story 01.3: Xây dựng crates/harness-daemon (Axum + Tokio) & 9-Spec Synthesis Pipeline"
description: "Khởi chạy máy chủ HTTP/WS nội bộ bằng Axum với cơ chế dynamic port discovery (.uxui-os/daemon.lock); cung cấp REST API quản lý Project & 9 Foundational Specs kèm gắn nhãn provenance (CONFIRMED/INFERRED/MISSING) và kế thừa Brand Guideline xuyên dự án."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/Story-01.3/story.md"
status: planned
risk_tier: 2
delivery: core
timestamp: "2026-11-02T14:22:00+07:00"
tags:
  - "story-01.3"
  - "epic-01"
  - "uxui-os"
  - "axum-daemon"
  - "9-spec-synthesis"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-01/evaluation-report.md"
dependencies: []
intra_epic_dependencies: ["01.2"]
provides_apis:
  - "internal:epic-01/story-01.3"
  - "GET /api/v1/health"
  - "POST /api/v1/projects"
  - "GET /api/v1/projects/:id/specs"
  - "PUT /api/v1/projects/:id/specs/:spec_key"
  - "POST /api/v1/projects/:id/specs/synthesize"
consumes_apis:
  - "internal:epic-01/story-01.1"
  - "internal:epic-01/story-01.2"
---

# Story 01.3: Xây dựng crates/harness-daemon (Axum + Tokio) & 9-Spec Synthesis Pipeline

- **FR Covered:** [FR-01: Project Bootstrap from Product Brief or Seeded Repo](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md), [FR-11: 9 Foundational Project Context Specs Synthesis](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md), [FR-12: Versioned Spec Registry & Cross-Project Reuse](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 01: Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/epic.md)
- **Status:** `planned`
- **Risk Tier:** `2`

## 1. Story Description & Context
Xây dựng máy chủ điều phối trung tâm `crates/harness-daemon` bằng Axum + Tokio lắng nghe trên địa chỉ loopback (`127.0.0.1:4170` kèm cơ chế tự động chọn cổng rảnh và ghi `.uxui-os/daemon.lock`). Cung cấp các REST API và kênh sự kiện WebSocket để khởi tạo dự án từ Product Brief hoặc repo mẫu, tổng hợp tự động **Bộ 9 Foundational Project Context Specs** (`01-product-overview.md` .. `09-data-and-copy.md`) có gắn nhãn mức độ tin cậy (`[CONFIRMED]`, `[INFERRED]`, `[MISSING]`), và hỗ trợ kế thừa Brand Guideline xuyên dự án cho các đội Product UI/UX, Marketing HTML Deck và Presentation.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.0` (Scope=1, Data=1, Integration=1, UI=0.5, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 5.0 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Loopback Axum Daemon & Core REST Endpoints):** Triển khai `crates/harness-daemon` chỉ lắng nghe trên loopback (`127.0.0.1`) với đầy đủ các endpoint: `GET /api/v1/health`, `POST /api/v1/projects`, `GET /api/v1/projects/:id/specs`, `PUT /api/v1/projects/:id/specs/:spec_key`, và `POST /api/v1/projects/:id/specs/synthesize`, ánh xạ chính xác các lỗi `HarnessError` sang HTTP status (`409 Conflict` cho `STALE_FILE`, `422 Unprocessable Entity` cho `INVALID_SPEC_KEY` / `BRIEF_EMPTY`, `404 Not Found` cho `PROJECT_NOT_FOUND`).
- **AC-02 (9-Spec Synthesis Pipeline with `[CONFIRMED]`/`[INFERRED]`/`[MISSING]` Provenance):** Khi gọi `POST /api/v1/projects/:id/specs/synthesize` với Product Brief hoặc đường dẫn repo mẫu, pipeline sinh đủ 9 file Foundational Specs trong `product/specs/` và lưu thống kê provenance (`confirmed_count`, `inferred_count`, `missing_count`) vào bảng `PROJECT_SPECS` để `cowok` và `advisor` kiểm tra mà không cần đọc toàn bộ nội dung văn bản (`BS-01.2`).
- **AC-03 (Cross-Project Brand & Design Direction Reuse — `FR-12` & `BS-01.1`):** Hỗ trợ trường `inherit_brand_from_project_id` khi khởi tạo dự án hoặc tổng hợp spec, tự động sao chép có kiểm chứng phiên bản của `04-design-direction.md` và `08-brand-guidelines.md` sang dự án mới kèm lưu vết `source_project_id` và `source_revision_sha256` nhằm phát hiện khi Brand Guideline gốc có bản cập nhật mới (`Upstream Brand Guideline Updated`).
- **AC-04 (`[EDGE-CASE]` `AS-01.1` — Port Collision Resilience & `.uxui-os/daemon.lock` Discovery):** Nếu cổng mặc định `127.0.0.1:4170` đang bị chiếm dụng hoặc truyền `--port 0` / `UXUI_OS_DAEMON_PORT`, daemon tự động bind sang cổng loopback khả dụng và ghi nguyên tử thông tin `{ pid, host, port, started_at }` vào `.uxui-os/daemon.lock` để UI Shell và CLI kết nối chính xác.
- **AC-05 (`[EDGE-CASE]` `EC-E01-03` & Sparse Brief Fallback Guard):** Khi Product Brief đầu vào quá ngắn hoặc thiếu thông tin Brand/Portal, pipeline không được phép bịa thông tin dưới nhãn `[CONFIRMED]`; bắt buộc điền khung mặc định an toàn gắn nhãn `[INFERRED]` hoặc `[MISSING]` kèm danh sách câu hỏi làm rõ (`open_questions`) cho `advisor` (Dr. Eggbot), đồng thời từ chối mọi `seed_repo_path` trỏ ra ngoài phạm vi cho phép hoặc chứa symlink độc hại.

## 4. Tasks
- [ ] **Task 01.3.1**: Xây dựng Axum HTTP server trong `crates/harness-daemon` với các routes `/api/v1/health`, `/api/v1/projects`, `/api/v1/projects/:id/specs` và bộ chuyển đổi lỗi HTTP chuẩn hóa (thỏa mãn `AC-01`).
- [ ] **Task 01.3.2**: Cài đặt pipeline tổng hợp 9 Foundational Specs (`POST /api/v1/projects/:id/specs/synthesize`) kèm bộ phân tích gắn nhãn `[CONFIRMED]`/`[INFERRED]`/`[MISSING]` và thống kê `provenance_summary_json` (thỏa mãn `AC-02`, `AC-05`).
- [ ] **Task 01.3.3**: Cài đặt cơ chế kế thừa `inherit_brand_from_project_id` cho `04-design-direction.md` và `08-brand-guidelines.md` kèm đối chiếu lệch phiên bản upstream (`BS-01.1`) (thỏa mãn `AC-03`).
- [ ] **Task 01.3.4**: Cài đặt cơ chế fallback cổng động + ghi file `.uxui-os/daemon.lock` (`AS-01.1`), kiểm tra an toàn `seed_repo_path`, và viết bài kiểm thử tích hợp E2E cho toàn bộ API của daemon (thỏa mãn `AC-04`, `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 01.3.1` | Axum Router Integration Tests (`cargo test -p harness-daemon`) |
| **AC-02** | `Task 01.3.2` | 9-Spec Synthesis & Provenance Badge Verification Test |
| **AC-03** | `Task 01.3.3` | Cross-Project Brand Inheritance & Upstream Drift Detection Test (`BS-01.1`) |
| **AC-04 (`[EDGE-CASE]`)** | `Task 01.3.4` | Port Collision & `.uxui-os/daemon.lock` Discovery Test (`AS-01.1`) |
| **AC-05 (`[EDGE-CASE]`)** | `Task 01.3.2`, `Task 01.3.4` | Sparse Brief `[INFERRED]`/`[MISSING]` Tagging & `409 STALE_FILE` HTTP Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["01.2"]`

### Impacts
- Cung cấp HTTP/WS Control Plane (`crates/harness-daemon`) và Bộ 9 Foundational Specs Vault cho `Epic 02` (Constraint Engine), `Epic 03` (Flow Graph), `Epic 06` (Presentation CMS) và `Epic 09` (`cowok` Orchestrator & `advisor`).

### Consumes
- Tiêu thụ `crates/domain` từ `Story 01.1` và `crates/spec-registry` (16-table SQLite WAL + 2-Way File Sync) từ `Story 01.2`.

### Shared Entities
- Đọc/ghi trực tiếp trên các bảng `PROJECTS`, `PROJECT_SPECS`, và `SPEC_REVISIONS` trong `2.2. database-spec.md`.

### Cross-Portal
- Phục vụ đồng thời các dự án thiết kế Product UI/UX, Marketing HTML Deck, và Executive Report thông qua cơ chế kế thừa thương hiệu (`inherit_brand_from_project_id`).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Đáp ứng trọn vẹn `FR-01`, `FR-11`, `FR-12` |
| **2. Data Integrity & State** | `9.5/10` | Bảo toàn chuỗi truy vết `source_revision_sha256` và `409 STALE_FILE` OCC |
| **3. Security & Validation** | `9.5/10` | Loopback-only binding (`127.0.0.1`) & kiểm soát chặt `seed_repo_path` |
| **4. Performance & Scalability** | `9.5/10` | Axum + Tokio non-blocking I/O & truy vấn metadata 0-token |
| **5. Error Handling & Recovery** | `9.5/10` | Tự động xử lý trùng cổng qua `.uxui-os/daemon.lock` (`AS-01.1`) |
| **6. Architectural Depth & Leverage** | `9.4/10` | Kết nối liền mạch `domain` $\rightarrow$ `spec-registry` $\rightarrow$ `harness-daemon` |
| **7. UX Empathy** | `9.5/10` | Không bịa dữ liệu khi Brief ngắn; gắn nhãn `[INFERRED]`/`[MISSING]` rõ ràng |
| **TOTAL AVERAGE** | **9.49/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 01.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-02 | 1.1.0 | Bổ sung AC-03, AC-04, AC-05 và các Edge Cases (`AS-01.1`, `BS-01.1`, `BS-01.2`, `EC-E01-03`) từ kết quả `/evaluate-epic` Epic 01 | cowok Orchestrator |
