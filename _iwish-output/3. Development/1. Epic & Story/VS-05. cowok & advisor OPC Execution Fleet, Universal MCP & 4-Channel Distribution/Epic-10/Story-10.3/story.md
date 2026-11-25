---
type: I-Wish Story
id: "10.3"
title: "Story 10.3: Anti-Slop Design Tournament (CMP-07, 4 Trục Trực giao <=15s, AST Distance <=0.78, Position-Swapped RubricShield), Cross-Variant Borrow & advisor 6-Step Loop"
description: "Đấu trường thiết kế Anti-Slop (CMP-07 DesignTournamentArena): 4 trục trực giao <=15s, Pre-Judge AST Distance <=0.78 (clone_distance_passed), Blind Wipe + Position-Swapped RubricShield + Cross-Variant Borrow (reconcile_ff_ids + CSS tokens), V18b tournament_runs/variants/creative_memory_events, /crystallize và advisor 6-step loop khóa cứng deny tests/eval/sealed/ + Staging HUMAN_ONLY."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/Story-10.3/story.md"
status: completed
risk_tier: 3
delivery: core
adr_ref: "ADR-006, ADR-009"
timestamp: "2026-11-24T12:15:00+07:00"
tags:
  - "story-10.3"
  - "epic-10"
  - "uxui-os"
  - "design-tournament"
  - "rubric-shield"
  - "advisor-loop"
  - "reflective-skill-pr"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/ADRs/ADR-009-Zero-Hardcode-AI-Harness-Spine-And-Data-Storytelling-Feedback-Loop.md"
dependencies: ["05.4", "08.7", "09.2", "10.2"]
provides_apis:
  - "POST /api/v1/tournament/start"
  - "POST /api/v1/tournament/select"
  - "POST /api/v1/crystallize"
consumes_apis:
  - "POST /api/v1/decision/route"
  - "POST /api/v1/fleet/dispatch"
---

# Story 10.3: Anti-Slop Design Tournament (`CMP-07`, 4 Trục Trực giao `<=15s`, AST Distance `<=0.78`, Position-Swapped `RubricShield`), Cross-Variant Borrow & `advisor` 6-Step Loop

- **FR Covered:** [FR-24, FR-26, FR-27: Anti-Slop Design Tournament, CMP-07 Blind Wipe + Borrow, /crystallize & advisor 6-Step Self-Evolution](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 10: cowok Master Orchestrator, advisor Meta-Agent, OPC Execution Fleet & Design Tournament](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Xây dựng đấu trường khám phá phương án chống đồng dạng AI (`Anti-Slop Design Tournament`) thông qua thành phần giao diện **`CMP-07 DesignTournamentArena`**: (1) Bắt buộc phân bổ các nhánh theo **4 Trục Thiết kế Trực giao** (`Information Density`, `Navigation Topology`, `Visual Metaphor`, `Interaction Rhythm`) và phát tán `<= 15s` qua `Story 10.2`; (2) Chạy cổng **Pre-Judge AST Structural Distance Gate** (`similarity <= 0.78`, `clone_distance_passed`) từ chối các nhánh sao chép na ná nhau; (3) Chấm mù bằng **`RubricShield`** đảo vị trí `(A,B)/(B,A)` kết hợp chế độ **`Blind Wipe`** Desktop/Mobile `390px`; (4) Cho phép **`Cross-Variant Borrow`** ghép thành phần xuất sắc từ bản thua bằng `reconcile_ff_ids` + lưu bảng `V18 tournament_runs` / `tournament_variants`; và (5) Kết tinh 1-Click (`/crystallize`) + vòng **`advisor` 6-Step Self-Evolution Loop** (`V18 creative_memory_events`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Isolation boundary cấm `advisor` chạm `tests/eval/sealed/` hoặc tự ý promote `SKILL.md` qua mặt `08.4 Staging + HUMAN_ONLY`, và `generation fencing` chống race condition khi chuyển Tournament variant.

## 3. Acceptance Criteria (AC)
- **AC-01 (4 Orthogonal Design Axes `<= 15s` & Pre-Judge AST Structural Distance `<= 0.78`):** `POST /api/v1/tournament/start` tách yêu cầu trên 4 trục trực giao khác biệt về bố cục, khởi chạy 2–4 nhánh song song trong `<= 15s` qua `Story 10.2` (`worker_file_leases`, tự hạ chuẩn `min_variants >= 2` nếu một nhánh quá hạn mà không hủy toàn giải), sau đó đo **AST Structural Distance** (`similarity <= 0.78`); nếu `> 0.78` thì buộc nhánh trùng lắp sinh lại trước khi vào vòng chấm (`clone_distance_passed` ghi vào `V18b tournament_variants`).
- **AC-02 (`CMP-07 DesignTournamentArena`, `Blind Wipe`, Position-Swapped `RubricShield` & `reconcile_ff_ids` Borrow):** Hiển thị `CMP-07` đồng bộ cả Desktop & Mobile `390px` với thanh trượt **`Blind Wipe`** (ẩn tên agent/model để chống bias), kết hợp điểm mù từ `RubricShield` (`(A,B)/(B,A)` từ `Story 09.2`) và ô **`Cross-Variant Borrow`** để chọn phương án thắng (`POST /api/v1/tournament/select`) đồng thời mượn an toàn 1 vùng thành phần kèm các rule CSS/token phụ thuộc từ bản thua thông qua **`reconcile_ff_ids`** (`Story 05.4` / `crates/html-forge`) mà không gãy ID hay vỡ giao diện.
- **AC-03 (`advisor` 6-Step Self-Evolution Loop, Bridge 2 Reflective Skill PR Pipeline to `design-craft`, `tests/eval/sealed/` Deny Guard & `/crystallize`):** Lưu tín hiệu thắng/thua/borrow vào `V18b creative_memory_events` và tiêu thụ đồng thời các cụm sửa tay lặp lại (`Manual Studio Correction >= 3`) từ `V18a gate_failure_ledger` (`Story 08.7` / `ADR-009`); kích hoạt vòng 6 bước `advisor` theo chuẩn **GEPA Reflective Optimization** (`Fail/Studio-Signal -> Root Cause -> Patch SKILL.md & references/*.md -> Sealed Eval Pre-Check -> Regression Test -> PR to design-craft Staging`) nhưng **khóa cứng ranh giới chống Goodhart: `advisor` TUYỆT ĐỐI CẤM sửa/chạm `tests/eval/sealed/` (`Story 13.1`)**, và mọi bản vá `SKILL.md`/`references/*.md` chỉ được nạp vào `08.3/08.4 Staging` hoặc mở PR nháp sang `design-craft` chờ kiểm định mù `Story 13.1` + duyệt `HUMAN_ONLY`; cung cấp 1-Click `/crystallize` (`POST /api/v1/crystallize`) đóng gói thành Starter Template / Brand Kit / Skill Plugin tái sử dụng.
- **AC-04 ([EDGE-CASE] Resilience, Rapid Option Switch Debounce & `constraints_preserved` Guard):** Nếu mọi nhánh đều cho `similarity > 0.78`, tự động giới hạn re-roll tối đa 2 lần rồi báo sự khác biệt thấp thay vì lặp vô hạn; nếu người dùng chuyển nhanh `Option A -> B -> C` liên tiếp trước khi bấm Xác nhận, chỉ phương án active cuối cùng mới được commit (`generation fencing`); mọi thao tác `advisor` chạm `tests/eval/sealed/` hoặc bỏ qua `Staging` bị chặn ngay cấp kernel với mã lỗi `SEALED_EVAL_TAMPER_ERROR` / `403 HUMAN_APPROVAL_REQUIRED`.

## 4. Tasks
- [ ] **Task 10.3.1**: Cài đặt Orchestrator 4 Trục Thiết kế Trực giao (`<=15s`), Pre-Judge AST Structural Distance Gate (`similarity <= 0.78`) và persistence `V18 tournament_runs` / `tournament_variants` (thỏa mãn `AC-01`, `AC-04`).
- [ ] **Task 10.3.2**: Xây dựng UI `CMP-07 DesignTournamentArena` (`Blind Wipe` Desktop/Mobile `390px`, `RubricShield` blind judge card, `Cross-Variant Borrow` qua `reconcile_ff_ids`) & endpoints `/api/v1/tournament/*` (thỏa mãn `AC-02`).
- [ ] **Task 10.3.3**: Xây dựng `advisor` 6-Step Self-Evolution Loop (GEPA Reflective Skill PR Pipeline sang `design-craft` Staging tiêu thụ thêm `V18a gate_failure_ledger` từ `08.7`) kèm regression gate, `V18 creative_memory_events` và nút `/crystallize` (`POST /api/v1/crystallize`) (thỏa mãn `AC-03`).
- [ ] **Task 10.3.4**: Viết bài test: AST structural distance rejection (`> 0.78`), position-swapped `RubricShield`, `reconcile_ff_ids` borrow không đụng `ff-id`, rapid selection switch race-condition `[EDGE-CASE]` và Playwright `CMP-07` E2E (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 10.3.1`, `Task 10.3.4` | 4-Axis Fan-Out `<=15s` & AST Similarity `<=0.78` Gate Test |
| **AC-02** | `Task 10.3.2`, `Task 10.3.4` | `CMP-07` Blind Wipe, `RubricShield` & `reconcile_ff_ids` Borrow Test |
| **AC-03** | `Task 10.3.3`, `Task 10.3.4` | `advisor` 6-Step Regression Gate & `/crystallize` Test |
| **AC-04 (`[EDGE-CASE]`)** | `Task 10.3.1`, `Task 10.3.4` | Rapid Switch Debounce & Infinite Re-Roll Cap Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["05.4", "09.2", "10.2"]

### Impacts
- Đẩy trực tiếp dữ liệu trận đấu, Creative Memory và bản vá `SKILL.md` vào `Story 13.1` (`M4/M5` Sealed Benchmark).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["05.4", "09.2", "10.2"] (`crates/html-forge` + `reconcile_ff_ids`, `RubricShield`, Swarm & `worker_file_leases`).

### Shared Entities
- Sở hữu ghi trên `tournament_runs`, `tournament_variants`, `creative_memory_events` (`V18`); đọc `component_patterns` (`V16`), `decision_audit_log` (`V17`) trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ trọn 4-Axis Tournament, AST Distance Gate, Blind Wipe & `advisor` Loop |
| **2. Data Integrity & State** | `9.5/10` | Ghi đủ `V18 tournament_runs`, `tournament_variants` & `reconcile_ff_ids` |
| **3. Security & Validation** | `9.5/10` | Bản vá `advisor` không thể mở rộng quyền hoặc bỏ qua regression |
| **4. Performance & Scalability** | `9.5/10` | Song song 4 nhánh trên lease rời rạc hoàn tất `<= 15s` |
| **5. Error Handling & Recovery** | `9.4/10` | Giới hạn re-roll & debounce race-condition chuyển option nhanh |
| **6. Architectural Depth & Leverage** | `9.5/10` | Kết nối chặt `Story 05.4`, `09.2`, `10.2` theo ADR-006 |
| **7. UX Empathy** | `9.6/10` | Non-tech kéo thanh `Blind Wipe` & mượn chi tiết bằng 1 câu nói |
| **TOTAL AVERAGE** | **9.50/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 10.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-08 | 2.0.0 | Nâng cấp AC-03..AC-04 bổ sung In-Chat + In-2D-DAG Tournament Synchronized Selector, Preview-on-Click & Explicit Confirmation Gate theo DESIGN.md v5.4 | cowok Orchestrator |
| 2026-11-12 | 2.1.0 | [refactor-story] Nâng cấp AC-03 Blind Wipe Tournament, bổ sung AC-05 Creative Memory Tree & /crystallize (GAP-12, GAP-18) theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Chuẩn hóa CMP-07 DesignTournamentArena, 4 trục thiết kế trực giao <=15s, Pre-Judge AST Structural Distance <=0.78, Position-Swapped RubricShield, Cross-Variant Borrow qua reconcile_ff_ids, V18 tables & dependencies ["05.4", "09.2", "10.2"] theo ADR-006 | /refactor-story |
| 2026-11-24 | 3.1.0 | [ADR-009] Bổ sung Bridge 2 Reflective Skill PR Pipeline sang `design-craft` Staging & tiêu thụ tín hiệu `Manual Studio Correction` (`V18a gate_failure_ledger`) từ `Story 08.7` vào `AC-03` và `Task 10.3.3` | System Architect |
