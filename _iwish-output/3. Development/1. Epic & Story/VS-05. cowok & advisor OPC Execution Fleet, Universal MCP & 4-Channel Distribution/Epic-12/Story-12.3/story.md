---
type: I-Wish Story
id: "12.3"
title: "Story 12.3: Kênh 4 — Container Image (Docker / OCI) kèm Headless Chromium cho Đội nhóm & CI/CD"
description: "Đóng gói image ghcr.io/cowok-ai/uiux-os tích hợp sẵn Rust Daemon, MCP SSE Server và Headless Chromium phục vụ kiểm định tự động E1..E4 trên CI/CD."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-12/Story-12.3/story.md"
status: refactored
risk_tier: 2
delivery: core
adr_ref: "ADR-006"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-12.3"
  - "epic-12"
  - "uxui-os"
  - "rust-day-1"
  - "pooled-cdp"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["07.1", "12.1"]
provides_apis:
  - "internal:epic-12/story-12.3"
consumes_apis:
  - "POST /api/v1/qc/run"
---

# Story 12.3: Kênh 4 — Container Image (Docker / OCI) kèm Pooled 1-Chromium CDP cho Đội nhóm & CI/CD

- **FR Covered:** [FR-30: Kênh 4 — Container Image (Docker / OCI) kèm Pooled 1-Chromium CDP cho Đội nhóm & CI/CD](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 12: 4-Channel Packaging & Distribution Architecture](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-12/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `2`

## 1. Story Description & Context
Đóng gói multi-arch container image `ghcr.io/cowok-ai/uiux-os` tích hợp sẵn Rust Core Daemon (`crates/harness-daemon`), Universal MCP SSE Server (`crates/mcp-gateway`) và **Pooled 1-Chromium CDP** dùng chung cho cả `lib-audit` và `slides-check` (tuân thủ ADR-006) phục vụ kiểm định E1..E4 và benchmark anti-Goodhart trên CI/CD mà không phụ thuộc LibreOffice trong core image.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0 $\rightarrow$ `CS = 4 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Multi-Arch Docker / OCI Image kèm Pooled 1-Chromium CDP):** Đóng gói image `ghcr.io/cowok-ai/uiux-os` (`linux/amd64`, `linux/arm64`) chứa `uiux-os` Rust binary, MCP SSE endpoint và `chromium` headless được cấu hình sẵn ở chế độ Pooled 1-Chromium CDP cho `Story 07.1`.
- **AC-02 (CI/CD Headless Gate & MCP Streamable HTTP Integration):** Hỗ trợ chạy một lệnh CI gate (`uiux-os ci gate --json`) gọi `POST /api/v1/qc/run` & `Story 12.1` CLI parity trên CI/CD pipeline, cung cấp môi trường Pooled Chromium sạch để `Story 13.1` gắn chạy suite mà không tạo phụ thuộc vòng ngược từ `12.3` sang `13.1`.
- **AC-03 ([EDGE-CASE] Non-Root Container, Linux Bind-Mount UID/Lockfile, Cgroup OOM Cap & `/dev/shm` Guard):** Chạy 100% dưới user non-root (`uid 10001`), xử lý sạch quyền bind-mount workspace trên Linux host (`chown`/local staging SQLite WAL không kẹt lockfile trên volume chỉ đọc/khác UID), áp đặt giới hạn bộ nhớ Pooled Chromium (`--max-memory-mb` + fallback `--disable-dev-shm-usage`), dọn sạch zombie child processes khi container nhận `SIGTERM`, và không gộp LibreOffice vào core image.

## 4. Tasks
- [ ] **Task 12.3.1**: Viết multi-stage Dockerfile (`linux/amd64`, `linux/arm64`), entrypoint khởi tạo Rust Daemon + MCP SSE + Pooled Chromium CDP và kiểm tra image size (thỏa mãn `AC-01`, `AC-03`).
- [ ] **Task 12.3.2**: Cài đặt lệnh `uiux-os ci gate` kết nối `POST /api/v1/qc/run` & `Story 12.1` CLI parity trên CI/CD pipeline (thỏa mãn `AC-02`).
- [ ] **Task 12.3.3**: Cài đặt non-root security hardening, `/dev/shm` fallback, `SIGTERM` graceful teardown và viết container integration test (thỏa mãn `AC-03`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 12.3.1` | Multi-Arch Image Build & Pooled Chromium Boot Test |
| **AC-02** | `Task 12.3.2` | CI Gate Headless Integration Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 12.3.3` | Non-Root, `/dev/shm` & SIGTERM Cleanup Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["07.1", "12.1"]

### Impacts
- Phục vụ trực tiếp pipeline CI/CD và môi trường chạy cô lập cho `Story 13.1` (`eval_benchmark_runs`).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["07.1", "12.1"] (Pooled CDP engine & single-binary CLI).

### Shared Entities
- Đọc/Ghi volume SQLite WAL cho `PROJECTS`, `SPEC_DOCUMENTS`, `qc_gate_runs`, `eval_benchmark_runs` trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.4/10` | Đóng gói đủ Rust Daemon + MCP SSE + Pooled CDP cho CI/CD |
| **2. Data Integrity & State** | `9.3/10` | SQLite WAL mount & graceful shutdown bảo toàn trạng thái |
| **3. Security & Validation** | `9.5/10` | Non-root user (`uid 10001`), bỏ dependency LibreOffice khỏi core |
| **4. Performance & Scalability** | `9.5/10` | Image gọn, dùng Pooled 1-Chromium CDP thay vì boot lại |
| **5. Error Handling & Recovery** | `9.4/10` | Fallback `--disable-dev-shm-usage` & `SIGTERM` process reaper |
| **6. Architectural Depth & Leverage** | `9.4/10` | Đồng bộ ADR-006 & CLI `12.1` |
| **7. UX Empathy** | `9.4/10` | 1-command CI gate JSON output |
| **TOTAL AVERAGE** | **9.41/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 12.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Cập nhật trạng thái refactored và đồng bộ quy hoạch đóng gói CI/CD Container Image theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Đồng bộ Pooled 1-Chromium CDP (loại LibreOffice khỏi core image theo ADR-006), thêm ci gate parity và cập nhật dependencies ["07.1", "12.1"] | /refactor-story |
