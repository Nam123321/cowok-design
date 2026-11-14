---
type: I-Wish Story
id: "01.3"
title: "Story 01.3: Xây dựng crates/harness-daemon (Axum + Tokio) & 9-Spec Synthesis Pipeline"
description: "Khởi chạy máy chủ HTTP nội bộ bằng Axum với cơ chế dynamic port discovery (.cowok-design/daemon.lock); cung cấp REST API quản lý Project & 9 Foundational Specs bảo vệ bởi Cryptographic Dual-Ingress Auth, tổng hợp domain-structured specs gắn nhãn CONFIRMED/INFERRED/MISSING, kế thừa brand guideline (03-ui-ux-spec.md & 09-data-and-copy.md), và In-Process Guarded Vite BFF + SDK."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/Story-01.3/story.md"
status: completed
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
- **Status:** `completed`
- **Risk Tier:** `2`

## 1. Story Description & Context
Xây dựng máy chủ điều phối trung tâm `crates/harness-daemon` bằng Axum + Tokio lắng nghe trên địa chỉ loopback (`127.0.0.1:4170` kèm cơ chế tự động chọn cổng rảnh và ghi `.cowok-design/daemon.lock` gắn trực tiếp vào `DaemonAuthState`). Cung cấp các REST APIs bảo vệ bởi Cryptographic Dual-Ingress Auth (`verify_hmac_v2` + `record_nonce_once` hoặc `X-Cowok-Session-Token`), In-Process Guarded Vite BFF (`vite-bff-plugin.ts` + `src/app/api/**`), và Typed Client SDK (`src/api/harness-daemon.ts` + Zod schemas `src/lib/validations/harness-daemon.ts`) để khởi tạo dự án, tổng hợp cấu trúc **Bộ 9 Foundational Project Context Specs** (`01-product-overview.md` .. `09-data-and-copy.md`) trung thực theo nhãn (`[CONFIRMED]`, `[INFERRED]`, `[MISSING]`), và kế thừa Brand/UI specs (`03-ui-ux-spec.md` & `09-data-and-copy.md`) xuyên dự án — nghiêm cấm vỏ RAM shadow stores và mock stub.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.0` (Scope=1, Data=1, Integration=1, UI=0.5, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 5.0 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Loopback Axum Daemon & Core REST Endpoints):** Triển khai `crates/harness-daemon` chỉ lắng nghe trên loopback (`127.0.0.1`) với đầy đủ endpoint: `GET /api/v1/health` (ẩn `daemon_nonce` & secrets), `GET/POST /api/v1/projects`, `GET /api/v1/projects/:id/specs`, `PUT /api/v1/projects/:id/specs/:spec_key`, `POST /api/v1/projects/:id/specs/synthesize`, và Vite BFF + `src/api/harness-daemon.ts` / `src/lib/validations/harness-daemon.ts`, ánh xạ `HarnessError` sang HTTP status (`409` `STALE_FILE`, `422` `INVALID_SPEC_KEY`/`VALIDATION_FAILED`, `404` `PROJECT_NOT_FOUND`).
- **AC-02 (Structured 9-Spec Synthesis Pipeline with Truthful Provenance):** Khi gọi `POST /api/v1/projects/:id/specs/synthesize` với Product Brief hoặc `seed_repo_path`, pipeline đọc Brief trực tiếp từ `base_registry`, phân tích domain riêng cho từng `spec_key` (không dùng 3 dòng template giống hệt nhau), lưu đủ 9 file trong `product/specs/` và `provenance_summary_json` vào `PROJECT_SPECS` (`BS-01.2`).
- **AC-03 (Cross-Project Brand & UI Reuse — `FR-12` & `BS-01.1`):** Khi `inherit_brand_from_project_id` được chỉ định, chỉ sao chép `03-ui-ux-spec.md` và `09-data-and-copy.md` từ dự án gốc (cấm tuyệt đối sao chép `04-architecture.md` hay `08-security-tenancy.md`) kèm lưu vết `source_project_id` & `source_revision_sha256` để cảnh báo `upstream_outdated`.
- **AC-04 (`[EDGE-CASE]` `AS-01.1` — Port Collision Resilience, `.cowok-design/daemon.lock` Discovery & Bound `DaemonAuthState`):** Nếu cổng mặc định bị chiếm dụng, daemon tự động bind sang cổng loopback rảnh, ghi nguyên tử `{ pid, host, port, started_at, daemon_nonce, hmac_key_hex }` vào `.cowok-design/daemon.lock` (với `0700`/`0600` & `O_NOFOLLOW`), và gắn trực tiếp `DaemonLockInfo` vào `DaemonAuthState` (bỏ `OnceLock` lệch lockfile và key dự phòng `0x42`).
- **AC-05 (`[EDGE-CASE]` `EC-E01-03` & Honest `[MISSING]` Sparse Brief Guard):** Khi Brief ngắn hoặc thiếu bằng chứng cho từng miền (`Database`, `Architecture`, `Security`, `UI`, `API`), pipeline tuyệt đối không đánh dấu `[CONFIRMED]` giả; phải gắn `[MISSING]` / `[INFERRED]` trung thực kèm `open_questions`, đồng thời bảo vệ `seed_repo_path` và `ensure_project_token` bằng `O_NOFOLLOW`/symlink jail.
- **AC-06 (`[EDGE-CASE]` `EC-E01-09` — Cryptographic Dual-Ingress Auth Across ALL Daemon Routes):** Xóa bỏ hoàn toàn `is_loopback_hmac` header-presence bypass trong `legacy_specs.rs`; bắt buộc `verify_hmac_v2` + `record_nonce_once` hoặc Bearer token trên toàn bộ legacy routes, bảo vệ `GET/POST /api/v1/projects`, khóa quyền tự cấp `admin` ở `/api/v1/auth/local-session` & `/api/auth/local-session`, và thêm `CorsLayer` chặn cross-origin loopback.
- **AC-07 (`[EDGE-CASE]` `EC-E01-14` — Guarded Vite BFF, Zero RAM-Shell Persistence & Session-Authenticated Workbench):** Xóa symlink `src/app/api/v1/projects`, bỏ `forwardDirect` trong `vite-bff-plugin.ts`, sửa `stripSpoofedHeaders` & `api-guard.ts` + bổ sung `body: ZodSchema` cho mọi POST/PUT route, chuyển `proposals`/`sessions` hoàn toàn sang SQLite (cấm RAM `OnceLock` list hay 22-byte empty ZIP mock -> ZIP thật hoặc `501`), sửa `logger.ts`, và bootstrap `X-Cowok-Session-Token` trong `useWorkbenchSessionSettings.ts`.

## 4. Tasks
- [x] **Task 01.3.1**: Xây dựng Axum HTTP server (`crates/harness-daemon`) với `/api/v1/health`, `/api/v1/projects` (kèm `workspace_root`), `/api/v1/projects/:id/specs*`, BFF routes + `src/api/harness-daemon.ts` + `src/lib/validations/harness-daemon.ts` (xóa symlink `v1/projects`) (thỏa mãn `AC-01`).
- [x] **Task 01.3.2**: Cài đặt pipeline tổng hợp có cấu trúc theo `spec_key`, sửa `load_project_brief_and_lineage` đọc từ `base_registry`, gắn nhãn trung thực `[CONFIRMED]`/`[INFERRED]`/`[MISSING]` (thỏa mãn `AC-02`, `AC-05`).
- [x] **Task 01.3.3**: Sửa cơ chế kế thừa `inherit_brand_from_project_id` chỉ áp dụng `03-ui-ux-spec.md` & `09-data-and-copy.md` (không copy `08-security-tenancy.md`) kèm kiểm tra `upstream_outdated` (`BS-01.1`) (thỏa mãn `AC-03`).
- [x] **Task 01.3.4**: Gắn `DaemonLockInfo` trực tiếp vào `DaemonAuthState` (`.cowok-design/daemon.lock`), xóa key dự phòng `0x42`, siết `O_NOFOLLOW` cho `ensure_project_token` (thỏa mãn `AC-04`, `AC-05`).
- [x] **Task 01.3.5**: Đóng toàn bộ lỗ hổng auth (`is_loopback_hmac` removal, `verify_hmac_v2` + `record_nonce_once`, `/api/v1/projects` & `/auth/local-session` hardening, `CorsLayer`, ẩn `daemon_nonce` ở `/health`) (thỏa mãn `AC-06`).
- [x] **Task 01.3.6**: Xóa RAM-shell (`proposals`/`sessions`/fake ZIP), thay `forwardDirect` bằng `guardedRoute({ body })`, sửa `stripSpoofedHeaders`, `logger.ts`, và bootstrap session token trong `useWorkbenchSessionSettings.ts` (thỏa mãn `AC-07`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 01.3.1` | Axum Router + Vite BFF + Zod + `harness-daemon.ts` SDK integration tests |
| **AC-02** | `Task 01.3.2` | Domain-differentiated 9-Spec Synthesis & `base_registry` brief lookup test |
| **AC-03** | `Task 01.3.3` | Brand reuse (`03-ui-ux-spec.md`, `09-data-and-copy.md` only) & `upstream_outdated` test |
| **AC-04 (`[EDGE-CASE]`)** | `Task 01.3.4` | `.cowok-design/daemon.lock` dynamic port & bound `DaemonAuthState` multi-instance test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 01.3.2`, `Task 01.3.4` | Sparse Brief `[MISSING]` tagging & `O_NOFOLLOW` token/seed_repo jail test |
| **AC-06 (`[EDGE-CASE]`)** | `Task 01.3.5` | Forged `X-Cowok-Hmac-Sig` rejection, replay prevention & unauthenticated `role` block test (`EC-E01-09`) |
| **AC-07 (`[EDGE-CASE]`)** | `Task 01.3.6` | SQLite persistence across restart, BFF `guardedRoute` body validation & Workbench session bootstrap test (`EC-E01-14`) |

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
| 2026-11-13 | 1.2.0 | Reconciled `.cowok-design/` dotdir, `03-ui-ux-spec.md` + `09-data-and-copy.md` brand reuse, `AC-06` (`EC-E01-09`) auth hardening, `AC-07` (`EC-E01-14`) Vite Guarded BFF / SQLite persistence / Workbench session bootstrap | cowok Orchestrator |
