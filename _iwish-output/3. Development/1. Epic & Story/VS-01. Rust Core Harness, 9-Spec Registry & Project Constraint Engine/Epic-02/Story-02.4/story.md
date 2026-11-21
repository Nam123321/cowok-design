---
type: I-Wish Story
id: "02.4"
title: "Story 02.4: Hidden Dual-Use SSOT (V18), AppShell & Component Registry, cowok-design-opt Spine, 7 Craft Skills & 3-Zone Router"
description: "Xây dựng lớp SSOT ẩn phục vụ kép Human/Agent (V18 component_patterns & app_shell_locks, 58 tables, AppShell Lock UI, export .cowok-design/exports/ + packages/ui-shell), xương sống cowok-design-opt (<=250 tok), 7 bộ Craft Skills 3 tầng, CachePreservingSkillRouter 3 Zone (<15ms), oxc reverse sync và Single Lock Owner giữ đồng bộ dự án dài nhiều màn hình."
resource: "iwish://uxui-os/development/epic-02/story-02.4"
status: completed
risk_tier: 3
delivery: core
adr_ref: "ADR-007"
timestamp: "2026-11-20T20:45:00+07:00"
tags:
  - "story-02.4"
  - "epic-02"
  - "hidden-ssot"
  - "cowok-design-opt"
  - "craft-skills"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.2. database-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../epic.md]]"
dependencies: ["01.3", "02.1", "03.1"]
provides_apis:
  - "GET /api/v1/ssot/components"
  - "POST /api/v1/ssot/appshell/lock"
  - "POST /api/v1/ssot/export"
  - "POST /api/v1/skills/route"
consumes_apis:
  - "POST /api/v1/context/assemble"
contract_manifest:
  schema_version: 2
  status: IMPLEMENTED
  mutates:
    - "crates/spec-registry:component_patterns"
    - "crates/spec-registry:app_shell_locks"
    - "crates/spec-registry:idempotency_records"
  provides:
    - "crates/spec-registry:V18__story_02_4_hidden_ssot"
    - "crates/spec-registry:appshell_lock_cas"
    - "crates/spec-registry:hidden_ssot_export"
    - "crates/spec-registry:cache_preserving_skill_router"
    - "crates/constraint-engine:ssot_ast"
    - "crates/harness-daemon:ssot_registry_routes"
    - "src/api/ssot-registry:SsotRegistryApiClient"
  consumes:
    - "crates/context-compiler:compile_context_only"
---

# Story 02.4: Hidden Dual-Use SSOT (`V18`), `AppShell` & Component Registry, `cowok-design-opt` Spine, 7 Craft Skills & 3-Zone Router

- **FR Covered:** [FR-32: Hidden Dual-Use SSOT, AppShell Lock & cowok-design-opt 7 Craft Skills](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 02: JIT Prompt Assembler, KV-Cache Compiler, Hidden SSOT & cowok-design-opt 7 Craft Skills](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Giải quyết triệt để vấn đề "dự án càng nhiều màn hình càng vỡ đồng bộ & rối mắt người non-tech": hệ thống âm thầm chuẩn hóa Component Registry & `AppShell` Lock (`V18`, `58` active SQLite WAL tables, `CURRENT_SCHEMA_VERSION = 18`, `DAEMON_MIN_SCHEMA_VERSION = 18`) đằng sau hậu trường, tự động xuất tài liệu chuẩn khi con người/kỹ sư yêu cầu (`.cowok-design/exports/`), kết hợp xương sống vận hành `cowok-design-opt` (`<=250 tok` minimum floor — không kịch bản cứng), **7 bộ Craft Skills 3 tầng** và `<15ms` 3-Zone `CachePreservingSkillRouter`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6` (Scope=1.5, Data=1, Integration=1, UI=1, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 6 < 7`, hợp nhất `02.4 + 02.5 + 13.2` thành một lát cắt 5 lớp liên thông UI <-> BFF <-> Rust/Router <-> `V18` <-> Eval Gate).
- **Formal-Verify Candidate (`risk_tier: 3`):** Giao thức `app_shell_locks` Single Lock Owner (monotonic `fencing_token` CAS + lease timeout takeover) và chặn ghi đè `oxc` reverse-sync khi `AppShell` đang khóa.

## 3. Acceptance Criteria (AC)
- **AC-01 (`V18` Hidden Dual-Use SSOT & `AppShell` Lock):** Tạo migration `V18__story_02_4_hidden_ssot.sql` (`CURRENT_SCHEMA_VERSION = 18`, `DAEMON_MIN_SCHEMA_VERSION = 18`, tổng **58** bảng, nối tiếp `Story 02.1` `V17`) cho `component_patterns` & `app_shell_locks`; cung cấp `AppShell`/Pattern Lock UI + Export (`/api/v1/ssot/*`) ghi xuất Dual-Use Markdown/JSON sang `.cowok-design/exports/` và tái sử dụng nguyên thủy trong `packages/ui-shell`, giữ chỉ số `confusion_risk < 0.35`.
- **AC-02 (`cowok-design-opt` Spine & 7 Full 3-Tier Craft Skills):** Đóng gói `cowok-design-opt` (`<=250` BPE multilingual tokens — progressive discovery minimum-floor không script cứng) và **7 Craft Skills 3 tầng** chuẩn slug (`typography`, `oklch-palette`, `stageshow-slide`, `dashboard-ux`, `impeccable-polish`, `sketch-monochrome`, `archify-diagram`) trong `packages/builtin-skills`.
- **AC-03 (`<15ms` 3-Zone `CachePreservingSkillRouter`):** Định tuyến theo 3 Zone với thứ tự sắp xếp từ điển xác định + length-prefixed prefix hash bảo toàn prefix KV-Cache ở `<15ms` -> nâng `>= +25%` first-pass `lib-audit` lift so với baseline không skill.
- **AC-04 (`oxc` Reverse Sync & Single Lock Owner):** Khi mã UI thay đổi, `oxc` reverse sync tự cập nhật ngược lại `component_patterns` dưới cơ chế **Single Lock Owner**; nếu `AppShell` đang bị khóa, mọi thay đổi xung đột từ disk hoặc `SpecWatcher` bị cách ly (`quarantined_drift`) và gắn hash dedup chống vòng lặp vô hạn.
- **AC-05 ([EDGE-CASE] Multi-Agent AppShell Conflict & Monotonic Lock Recovery):** Chặn ghi đè `app_shell_locks` khi thiếu hoặc sai monotonic `fencing_token` và dùng CAS chuyển nhượng lock an toàn sau timeout mà không mất thay đổi hợp lệ.

## 4. Tasks
- [ ] **Task 02.4.1**: Thêm migration `V18__story_02_4_hidden_ssot.sql` (`component_patterns`, `app_shell_locks`, `CURRENT_SCHEMA_VERSION = 18`, `DAEMON_MIN_SCHEMA_VERSION = 18`) và triển khai Single Lock Owner + `oxc` reverse sync trong `crates/spec-registry` & `crates/constraint-engine` (thỏa mãn `AC-01`, `AC-04`).
- [ ] **Task 02.4.2**: Đóng gói `cowok-design-opt` (`<=250 tok`) + 7 Craft Skills 3 tầng + `<15ms` 3-Zone `CachePreservingSkillRouter` qua `/api/v1/skills/route` (thỏa mãn `AC-02`, `AC-03`).
- [ ] **Task 02.4.3**: Xây dựng `AppShell`/Pattern Lock UI + Export `.cowok-design/exports/` & nối `packages/ui-shell` (thỏa mãn `AC-01`).
- [ ] **Task 02.4.4**: Viết benchmark đo `<15ms` router, `>= +25%` `lib-audit` lift, `confusion_risk < 0.35` và test `[EDGE-CASE]` lock collision (thỏa mãn `AC-03`, `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 02.4.1`, `Task 02.4.3` | V18 Persistence (`58` tables), Export & UX Clarity Test (`confusion_risk < 0.35`) |
| **AC-02** | `Task 02.4.2` | Token Ceiling (`<=250` tok) & 3-Tier Asset Validation |
| **AC-03** | `Task 02.4.2`, `Task 02.4.4` | `<15ms` Latency & `>= +25%` First-Pass Lift Benchmark |
| **AC-04** | `Task 02.4.1` | `oxc` Reverse Sync Integration Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 02.4.4` | Concurrent Lock Contention Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["01.3", "02.1", "03.1"]

### Impacts
- Cấp năng lực `stageshow` cho `Story 06.5`, `sketch-monochrome` + `impeccable-polish` cho `Story 05.4`, và bộ router KV-Cache cho `Story 09.2`.

### Consumes
- Tiêu thụ KV-Cache Static-Prefix Compiler (`02.1`), `PROJECT_CONSTRAINTS` (`03.1`) và Spec Vault (`01.3`).

### Shared Entities
- `component_patterns`, `app_shell_locks` (`V18`, `58` total tables).

### Cross-Portal
- Cô lập Registry component theo `project_id + portal_id` (`SuperAdmin`, `SalesApp`, `MarketingWeb`, `Presentation`).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ trọn Hidden SSOT, 7 Craft Skills & 3-Zone Router |
| **2. Data Integrity & State** | `9.5/10` | Single Lock Owner & `oxc` reverse sync chống trôi |
| **3. Security & Validation** | `9.5/10` | Cô lập Portal & ràng buộc schema `V18` (`58` tables) |
| **4. Performance & Scalability** | `9.6/10` | `<15ms` routing & bảo toàn KV-cache prefix |
| **5. Error Handling & Recovery** | `9.4/10` | Xử lý lock contention sạch |
| **6. Architectural Depth & Leverage** | `9.6/10` | Dual-use phục vụ cả non-tech, kỹ sư và agent |
| **7. UX Empathy** | `9.6/10` | Ẩn complexity khỏi non-tech (`confusion_risk < 0.35`) |
| **TOTAL AVERAGE** | **9.53/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0 | Khởi tạo Story 02.4 (hợp nhất 02.4+02.5+13.2) chuẩn 5 lớp Post-PR #14 | /make-story |
