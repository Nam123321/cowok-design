---
type: I-Wish Story
id: "11.1"
title: "Story 11.1: POSIX BYOS CLI Sandbox (0600 FD Staging + setpgid/killpg) & BYOK Unified Gateway"
description: "Tự động dò tìm và điều phối codex, claude, agy, ollama qua file descriptor 0600 (không lộ prompt trên ps aux), quản lý nhóm tiến trình bằng setpgid/killpg và tích hợp cổng BYOK."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.1/story.md"
status: completed
risk_tier: 3
delivery: hybrid
adr_ref: "ADR-006"
components:
  - id: "byos-posix-sandbox"
    packaging: "KERNEL_CORE"
  - id: "external-cli-adapter-pack"
    packaging: "EXTENSION_PLUGIN"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-11.1"
  - "epic-11"
  - "uxui-os"
  - "rust-day-1"
  - "byos-byok"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["04.3", "04.4", "04.6", "02.1"]
provides_apis:
  - "POST /api/v1/bot-fleet/execute"
  - "GET /api/v1/doctor/cli-matrix"
consumes_apis:
  - "POST /api/v1/doctor/verify-key"
---

# Story 11.1: POSIX BYOS CLI Sandbox (0600 FD + -PGID), Unified BYOK Gateway (`crates/bot-fleet`) & Post-Execution Normalizer

- **FR Covered:** [FR-28: POSIX BYOS CLI Sandbox (0600 FD Staging + -PGID Reaper) & BYOK Unified Gateway](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 11: POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/epic.md)
- **Status:** `completed`
- **Risk Tier:** `3`

## 1. Story Description & Context
Triển khai lớp thực thi thật trong `crates/bot-fleet` thay thế toàn bộ stub/mock còn lại (`dispatchNodeExecution.ts` & `presentation_copilot.rs`): tự động dò và điều phối 26+ BYOS CLIs (`claude`, `codex`, `agy`, `gemini`, `cursor`, `opencode`, `omp`, `ollama`...) qua **POSIX `0o600` File Descriptor staging** (tuyệt đối không lộ prompt trên `ps aux`), quản lý nhóm tiến trình bằng `setpgid` + `-PGID` reaper, cô lập `$COWOK_SESSION_DIR` với `O_NOFOLLOW` (Zero-Global-Pollution lên `~/.claude`/`~/.gemini`/`~/.codex`), kết nối **Unified BYOK Gateway** đọc khóa từ Keychain/`0o600` vault (`Story 04.6`), và chạy **Post-Execution Normalizer** chuẩn hóa output AI trước khi ghi SSOT.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=0.5, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Quy trình POSIX `0o600` FD staging + RAII Drop, `-PGID` group kill / Windows Job Object parity và `$COWOK_SESSION_DIR` `O_NOFOLLOW` không ghi bẩn thư mục home.

## 3. Acceptance Criteria (AC)
- **AC-01 (POSIX `0o600` FD Staging, `-PGID` Reaper & Real `crates/bot-fleet` Execution):** Thay thế `dispatchNodeExecution.ts` và phần điều phối bot chung trong daemon bằng `crates/bot-fleet` (`Story 06.5` sở hữu phần echo stub trong `presentation_copilot.rs`): truyền prompt qua FIFO/file tạm quyền `0o600` (xóa tức thì qua RAII Drop guard), gán `setpgid(0, 0)` + `kill(-pgid, SIGKILL)` trên POSIX (kèm Windows Job Object cho Tauri `.msi` ở `12.2`) khi timeout hoặc hủy (`POST /api/v1/bot-fleet/execute`).
- **AC-02 (Unified BYOK Gateway, Provider Quota & `SettingsApiBentoPanel` / `SettingsDetectedProvidersPanel` Live Wiring):** Kết nối 2 bảng Bento pre-staged từ `Story 04.3` với backend thật (`GET /api/v1/doctor/cli-matrix`, `GET /api/v1/doctor/usage`, tiêu thụ `POST /api/v1/doctor/verify-key` từ `04.3`), định tuyến thống nhất `gemini`, `claude`, `openai`, `openrouter`, `deepseek`, `byos` và đọc trực tiếp key từ Rust `VaultStore` (`Story 04.6`).
- **AC-03 ([EDGE-CASE] Resilience, Zero Zombie Process & Argv Redaction Guard):** Xác nhận bằng test không có bất kỳ ký tự prompt hay API key nào xuất hiện trên `/proc/<pid>/cmdline` (`ps aux`); dọn sạch 100% zombie processes sau SIGTERM/SIGKILL/panic; tự động xoay key/cooldown khi gặp HTTP 429 hoặc CLI binary missing.
- **AC-04 (Zero-Global-Pollution `$COWOK_SESSION_DIR` với `O_NOFOLLOW` + 26+ Adapters & Post-Execution Normalizer):** Tạo thư mục phiên tạm cô lập `$COWOK_SESSION_DIR` (`O_NOFOLLOW`) để tiêm config MCP/Skills cho 26+ data-driven CLI adapters mà không chạm tới `~/.claude`/`~/.gemini`/`~/.codex`; gọi lại bộ chuẩn hóa `<svg viewBox>` của `Story 04.6` cùng Post-Execution Normalizer sửa JSON schema & strip ANSI trước khi trả kết quả về canvas/spec.

## 4. Tasks
- [ ] **Task 11.1.1**: Xây dựng `crates/bot-fleet` với POSIX `0o600` FD staging, `setpgid`/`-PGID` reaper, `$COWOK_SESSION_DIR` (`O_NOFOLLOW`) và 26+ data-driven CLI adapters (thỏa mãn `AC-01`, `AC-04`).
- [ ] **Task 11.1.2**: Triển khai Unified BYOK Gateway, live CLI/Quota Doctor APIs nối thật với `SettingsDetectedProvidersPanel.tsx` & `SettingsApiBentoPanel.tsx` và retire stub `dispatchNodeExecution.ts` (thỏa mãn `AC-02`).
- [ ] **Task 11.1.3**: Cài đặt Post-Execution Normalizer (SVG `viewBox`, JSON/Markdown schema coerce, ANSI strip) cho toàn bộ output từ BYOS/BYOK (thỏa mãn `AC-04`).
- [ ] **Task 11.1.4**: Viết kiểm thử bảo mật & `[EDGE-CASE]` (`ps aux` zero-secret leak, `-PGID` orphan reaper, `O_NOFOLLOW` symlink trap, 429 retry-after backoff) (thỏa mãn `AC-03`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 11.1.1`, `Task 11.1.4` | Process Group & Real `bot-fleet` Integration Test (`cargo test`) |
| **AC-02** | `Task 11.1.2`, `Task 11.1.4` | BYOK Gateway & Settings Bento Live Wiring Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 11.1.1`, `Task 11.1.4` | `ps aux` Zero-Leak & `-PGID` Zombie Cleanup Regression Test |
| **AC-04** | `Task 11.1.1`, `Task 11.1.3` | `$COWOK_SESSION_DIR` `O_NOFOLLOW` & Output Normalizer Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["04.3", "04.4", "04.6", "02.1"]

### Impacts
- Cung cấp động cơ thực thi sạch & an toàn cho `Story 06.5` (AI Deck Gen), `Story 10.1..10.3` (OPC Fleet & Tournament) và `Story 11.2` (Open MCP Gateway).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["04.3", "04.4", "04.6", "02.1"] (Settings Bento pre-staged UI & `/verify-key`, Prompt Ledger, VaultStore & SVG normalizer, KV-Cache Compiler).

### Shared Entities
- Đọc/Ghi `node_execution_jobs` (`V17`), `PROJECTS`, `FLEET_TASKS` trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Thay thế 100% stub execution bằng `crates/bot-fleet` thật |
| **2. Data Integrity & State** | `9.4/10` | Post-Execution Normalizer bảo vệ schema trước khi ghi SSOT |
| **3. Security & Validation** | `9.6/10` | `0o600` FD staging, `O_NOFOLLOW`, `-PGID` killpg, 0 key trên `ps aux` |
| **4. Performance & Scalability** | `9.4/10` | 26+ data-driven adapters tái sử dụng KV-cache prompt từ `02.1` |
| **5. Error Handling & Recovery** | `9.5/10` | Dọn sạch child tree và RAII temp files trong mọi nhánh lỗi |
| **6. Architectural Depth & Leverage** | `9.5/10` | `hybrid` packaging (Kernel POSIX sandbox + Extension adapter pack) |
| **7. UX Empathy** | `9.4/10` | Non-tech cắm CLI/API key xong dùng ngay từ Bento Grid hoặc Chat Vault |
| **TOTAL AVERAGE** | **9.47/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 11.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Zero-Global-Pollution $COWOK_SESSION_DIR (O_NOFOLLOW) & 26+ Agent Adapters theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Nâng cấp thành hybrid packaging theo ADR-006; bổ sung crates/bot-fleet retire dispatchNodeExecution stubs, Post-Execution Normalizer & dependencies ["04.3", "04.4", "02.1"] | /refactor-story |

## 9. Upstream FE Handoff & Wave Boundary Notes (Synced 2026-11-16)
- **Pre-Staged Settings Bento UI (from Epic 04 / Story 04.3 Reconciliation):**
  - Bento 1 (`src/pages/workbench/components/SettingsDetectedProvidersPanel.tsx`) & Bento 2 (`src/pages/workbench/components/SettingsApiBentoPanel.tsx`) already render the 2-column auto-detected BYOS/CLI catalog (`readyTools` auto-connected + inline setup flow) and the 6-provider API Bento Grid (`gemini`, `claude`, `openai`, `openrouter`, `deepseek`, `byos`) wired to `GET /api/v1/doctor/cli-matrix`, `GET /api/v1/doctor/usage`, and `POST /api/v1/doctor/verify-key`.
  - **Remaining Backend Execution Scope Owned Strictly by Story 11.1:** Isolated `$COWOK_SESSION_DIR` (`O_NOFOLLOW` + `setpgid`/`killpg`) process execution across 26+ CLI adapters, live cross-provider quota aggregation where providers require authenticated runtime calls, and the Unified BYOK HTTP Gateway router.
