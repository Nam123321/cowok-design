---
type: I-Wish Story
id: "02.1"
title: "Story 02.1: Portal-Safe Context Slicing, KV-Cache Static-Prefix Prompt Compiler & 3-Profile Live Execution"
description: "Hợp nhất 02.1 + 02.2 + 02.3 thành lát cắt dọc 5 lớp hoàn chỉnh: Context Meter & CLI --inspect-prompt <-> /api/v1/context/assemble <-> TaskHorizonRouter + ContextSlicer (>=60% SSOT/DOM, <=350 tok data-ff-id outline) + Static-Prefix/Dynamic-Tail DeterministicPromptCompiler (<=1,500 text tok + max_image_tokens_per_turn) + 3-Profile Model Capability Adapter -> SQLite telemetry."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-02/Story-02.1/story.md"
status: refactored
risk_tier: 2
delivery: core
timestamp: "2026-11-20T20:50:00+07:00"
tags:
  - "story-02.1"
  - "epic-02"
  - "uxui-os"
  - "rust-day-1"
  - "kv-cache"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["01.2", "01.3"]
provides_apis:
  - "POST /api/v1/context/assemble"
consumes_apis:
  - "GET /api/v1/projects/:id/specs"
---

# Story 02.1: Portal-Safe Context Slicing, KV-Cache Static-Prefix Prompt Compiler & 3-Profile Live Execution

- **FR Covered:** [FR-14, FR-15: Context Slicer, Static-Prefix/Dynamic-Tail KV-Cache Compiler (`<=1,500` tok) & 3-Profile Model Adapter](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 02: JIT Prompt Assembler, KV-Cache Compiler, Hidden SSOT & cowok-design-opt 7 Craft Skills](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `2`

## 1. Story Description & Context
Hợp nhất toàn bộ phạm vi của **Story 02.1, Story 02.2 và Story 02.3** thành một Vertical Slice 5 lớp khép kín: kết nối **Context Meter UI & CLI `--inspect-prompt`** <-> `POST /api/v1/context/assemble` <-> `TaskHorizonRouter` (`short / mid / long`) + `ContextSlicer` (`>=60%` trích xuất SSOT/DOM, nén outline `<=350` tokens mang tọa độ `data-ff-id`) + **`Static-Prefix / Dynamic-Tail DeterministicPromptCompiler`** (`<=1,500` text tokens + `max_image_tokens_per_turn`) + **`3-Profile Model Capability Adapter`** (`Profile A` Frontier / `Profile B` Local Small Model / `Profile C` Offline Fallback) -> ghi telemetry SQLite WAL.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn theo chủ trương không phân mảnh luồng dọc).

## 3. Acceptance Criteria (AC)
- **AC-01 (`TaskHorizonRouter`, Portal-Safe `ContextSlicer` & `<=350` Tok `data-ff-id` Outline):** Khai thác API `POST /api/v1/context/assemble` tiêu thụ spec từ `Story 01.3`; cắt lát ngữ cảnh riêng từng `portal_id` (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`, `Presentation`) với `>=60%` tỷ trọng SSOT/DOM và nén khung `data-ff-id` outline đạt `<=350` tokens (`short/mid/long` horizon).
- **AC-02 (`Static-Prefix / Dynamic-Tail` KV-Cache Compiler `<=1,500` Text Tok + `max_image_tokens_per_turn`):** Bố trí cố định phần tiền tố tĩnh (Brand Tokens, Project Constraints, Active Skill Prefix) và đẩy phần thay đổi theo lượt xuống Dynamic Tail để tối ưu KV-cache hit rate; kẹp cứng ngân sách `<=1,500` text tokens (BPE-aware) + `max_image_tokens_per_turn` bằng Deterministic Semantic Pruner có `ReasonCode` (`atomic_complete` / `pruned_by_budget`), đồng thời hiển thị `Context Meter` trên UI và `cli --inspect-prompt`.
- **AC-03 ([EDGE-CASE] Cross-Portal Isolation & Deterministic Truncation Fallback):** Chặn tuyệt đối rò rỉ ngữ cảnh hoặc bảng thuộc `SuperAdmin` khi người dùng đang dựng `MarketingWeb` hoặc Presentation công khai; nếu một block đơn lẻ vượt ngân sách, không cắt mù giữa thẻ HTML/JSON gây vỡ cú pháp mà trả mã lỗi cấu trúc hoặc chuyển sang chế độ rút gọn cấp bảng/outline.
- **AC-04 (`3-Profile Model Capability Adapter` & Live Telemetry):** Tự động điều chỉnh cấu trúc prompt, độ sâu ví dụ và danh sách tool theo **3 Profile** (hấp thụ `Story 02.2 & 02.3`): `Profile A` (Frontier LLM: full micro-batch), `Profile B` (Small/Local Model: hạ cấp micro-tool từng bước, giảm branching), và `Profile C` (Offline FTS5 fallback), ghi nhận tỷ lệ cắt & độ trễ vào bảng telemetry SQLite WAL.

## 4. Tasks
- [ ] **Task 02.1.1**: Xây dựng `TaskHorizonRouter` và Portal-Safe `ContextSlicer` (`>=60%` SSOT/DOM, `<=350` tok `data-ff-id` outline) trên `crates/context-compiler` nối qua `GET /api/v1/projects/:id/specs` (thỏa mãn `AC-01`, `AC-03`).
- [ ] **Task 02.1.2**: Triển khai `Static-Prefix / Dynamic-Tail DeterministicPromptCompiler` (`<=1,500` text tok + `max_image_tokens_per_turn` + `atomic_complete` ReasonCode) và endpoint `POST /api/v1/context/assemble` (thỏa mãn `AC-02`).
- [ ] **Task 02.1.3**: Cài đặt `3-Profile Model Capability Adapter` (`Profile A/B/C`), `Context Meter` UI widget & CLI `--inspect-prompt` + SQLite telemetry (thỏa mãn `AC-02`, `AC-04`).
- [ ] **Task 02.1.4**: Viết kiểm thử tích hợp & `[EDGE-CASE]` (Cross-Portal Leak block, KV-cache prefix byte-identity across turns, `<=1,500` BPE ceiling, Profile B micro-tool downgrade) (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 02.1.1`, `Task 02.1.4` | Portal-Safe Slicer & `<=350` Tok Outline Unit Test (`cargo test`) |
| **AC-02** | `Task 02.1.2`, `Task 02.1.3`, `Task 02.1.4` | KV-Cache Prefix Byte-Stability, `<=1,500` Tok Cap & `--inspect-prompt` Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 02.1.1`, `Task 02.1.4` | Cross-Portal Leak & HTML-Safe Pruner Regression Test |
| **AC-04** | `Task 02.1.3`, `Task 02.1.4` | 3-Profile Model Capability Downgrade & Telemetry Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["01.2", "01.3"]

### Impacts
- Cung cấp hợp đồng `POST /api/v1/context/assemble` và KV-cache Static-Prefix compiler cho `Story 02.4`, `Story 09.2` (Decision Cascade) và `Story 11.1` (`crates/bot-fleet`).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["01.2", "01.3"] (`GET /api/v1/projects/:id/specs`, 9-Spec Registry & Project Constraints).

### Shared Entities
- Đọc/Ghi `PROJECTS`, `PROJECT_CONSTRAINTS`, `SPEC_DOCUMENTS` trong `2.2. database-spec.md`.

### Cross-Portal
- Cô lập nghiêm ngặt theo `portal_id` (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`, `Presentation 16:9`).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Hợp nhất hoàn chỉnh `02.1 + 02.2 + 02.3` thành 1 Vertical Slice |
| **2. Data Integrity & State** | `9.4/10` | Cắt tỉa ngữ cảnh theo `atomic_complete`, không vỡ JSON/HTML |
| **3. Security & Validation** | `9.6/10` | Chặn tuyệt đối rò rỉ chéo giữa Portal `SuperAdmin` và Public |
| **4. Performance & Scalability** | `9.6/10` | Tối ưu KV-cache Static-Prefix & nén outline `<=350` tokens |
| **5. Error Handling & Recovery** | `9.4/10` | Fallback mượt xuống `Profile B/C` khi dùng model nhỏ/offline |
| **6. Architectural Depth & Leverage** | `9.5/10` | Triệt tiêu ranh giới gãy giữa Slicer, Compiler và Adapter |
| **7. UX Empathy** | `9.4/10` | `Context Meter` trực quan và `--inspect-prompt` minh bạch cho Dev |
| **TOTAL AVERAGE** | **9.49/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 02.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-20 | 2.0.0 | [refactor-story] Đồng bộ hoàn chỉnh phần thân Story 02.1 hợp nhất 02.1 + 02.2 + 02.3 (KV-Cache Static-Prefix/Dynamic-Tail <=1,500 tok, Outline <=350 tok, 3-Profile Adapter, Context Meter & --inspect-prompt) với dependencies ["01.2", "01.3"] | /refactor-story |
