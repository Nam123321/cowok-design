---
type: I-Wish Story
id: "09.2"
title: "Story 09.2: Decision Cascade Engine (~450 LOC Rust DecisionCascadeClient), 5-Stage DE->Decision#1->DR/DE->DE->Decision#2 & MAX_ATTEMPTS=3 Breaker"
description: "Thay thế crates/jev-engine bằng Rust DecisionCascadeClient (~450 LOC trong crates/domain + crates/bot-fleet) tuân thủ ADR-006: máy trạng thái 5 bước DE -> Decision #1 -> DR/DE -> DE -> Decision #2 (Local FTS5 BM25 -> gemini-decision-1 / TypeSafe JEV -> Tiered Fallback), ConfidenceKind, egress_guard, VerdictShield (0% float leak), RubricShield (>=90%), MAX_ATTEMPTS=3 breaker & StaleFile OCC -> V17 decision_audit_log."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-09/Story-09.2/story.md"
status: refactored
risk_tier: 3
delivery: core
adr_ref: "ADR-006"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-09.2"
  - "epic-09"
  - "decision-cascade"
  - "adr-006"
  - "rust-day-1"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["02.1", "02.4", "04.6", "08.5"]
provides_apis:
  - "POST /api/v1/decision/route"
  - "GET /api/v1/decision/audit"
consumes_apis:
  - "POST /api/v1/de/run"
---

# Story 09.2: Decision Cascade Engine (`DecisionCascadeClient` ~450 LOC), 5-Stage `DE -> Decision #1 -> DR/DE -> DE -> Decision #2`, `RubricShield` & `MAX_ATTEMPTS = 3`

- **FR Covered:** [FR-23, FR-31: System-1 Decision Cascade (gemini-decision-1 / TypeSafe JEV / Offline FTS5), VerdictShield & MAX_ATTEMPTS=3 Breaker](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 09: Design-Only REA & Awesome-RE Forge & Decision Cascade Engine](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-09/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Triển khai kiến trúc **Decision Cascade** theo **ADR-006 / TDR-006** bằng mô-đun **`DecisionCascadeClient`** (~450 LOC Rust đặt gọn trong `crates/domain` + `crates/bot-fleet`, **loại bỏ hoàn toàn crate thừa `crates/jev-engine`**) để vận hành máy trạng thái 5 bước `DE -> Decision #1 -> DR/DE -> DE -> Decision #2` (`POST /api/v1/decision/route`): kết hợp **3-Tier Cascade** (`Local FTS5 BM25` offline -> `Google gemini-decision-1` / `TypeSafe JEV` đọc khóa qua Rust `VaultStore` nội bộ từ `Story 04.6` -> `Tiered Heuristic Fallback`), chuẩn hóa **`ConfidenceKind`** khớp 100% `2.2. database-spec.md`, bảo vệ dữ liệu ra ngoài bằng **`egress_guard`** (`4KB/64KB` + nonce-tagged `<untrusted_artifact_bytes nonce="...">`), tách log xác suất bằng **`VerdictShield` (`0%` float leak)**, chấm cặp chống thiên vị bằng **`RubricShield`** (`(X,Y)/(Y,X)` position swap `>= 90%` + tie-break xác định), tích hợp **`MAX_ATTEMPTS = 3` oscillation breaker & `StaleFile` OCC** từ `09.3` cũ, và ghi vết bất biến vào **`V17a decision_audit_log`** (sở hữu migration slot `V17a`, không đụng `V17b` của `10.2` / `V17c` của `07.1`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=0.5, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Máy trạng thái 5-stage `MAX_ATTEMPTS = 3` oscillation breaker + `StaleFile` OCC và bất biến `VerdictShield` (`0%` raw float leak).

## 3. Acceptance Criteria (AC)
- **AC-01 (5-Stage `DE -> Decision #1 -> DR/DE -> DE -> Decision #2` & ~450 LOC `DecisionCascadeClient`):** Cài đặt `DecisionCascadeClient` (~450 LOC Rust trong `crates/domain` + `crates/bot-fleet`, không tạo `crates/jev-engine`), thực thi đầy đủ chu trình 5 bước (Step 1 DE deterministic pre-filter -> Step 2 Decision #1 route model/skill/lane -> Step 3 DR/DE execution -> Step 4 DE deterministic post-check -> Step 5 Decision #2 accept/retry/escalate) hỗ trợ `Local FTS5 BM25`, `gemini-decision-1` và `TypeSafe JEV` đọc khóa trực tiếp từ Rust `VaultStore` (`Story 04.6`).
- **AC-02 (`ConfidenceKind`, `egress_guard`, `VerdictShield` & `RubricShield >= 90%`):**
  1. Gán nhãn tường minh `ConfidenceKind` khớp `2.2. database-spec.md`: **`ExactMatch` | `ModelVerdict` | `DeterministicFallback`** cho mọi quyết định.
  2. `egress_guard` băm/cắt payload trước khi gọi external decision API (`<= 4KB` prompt / `<= 64KB` total context, bọc thẻ kèm nonce chống prompt-breakout `<untrusted_artifact_bytes nonce="...">`, escape thẻ đóng bên trong, xóa sạch mọi API key/PII).
  3. **`VerdictShield`** bảo đảm `0%` raw logprobs hay calibrated floats rò rỉ ra reasoning prompt của Worker/Orchestrator.
  4. **`RubricShield`** chạy đối chiếu hai chiều `(X, Y)` / `(Y, X)` đạt tỷ lệ nhất quán `>= 90%`, kèm quy tắc phá vỡ thế cân bằng xác định (`deterministic tie-break` bằng điểm DE khi đảo vị trí cho kết quả trái ngược).
- **AC-03 ([EDGE-CASE] `MAX_ATTEMPTS = 3` Oscillation Breaker, `StaleFile` OCC & Graceful Offline Fallback):** Nếu hai quyết định lặp dao động qua lại hoặc thất bại `MAX_ATTEMPTS = 3` lần liên tiếp, kích hoạt circuit breaker chuyển thẳng về Step-6 `advisor`/human kèm audit snapshot; kiểm tra SHA-256 `StaleFile` OCC chống ghi đè stale state; khi mất mạng hoặc chưa nhập API key vẫn tự động lùi mượt về `Local FTS5 BM25` offline (`<5ms`, không treo flow).
- **AC-04 (Immutable `V17a decision_audit_log` & `GET /api/v1/decision/audit`):** Ghi nhận toàn bộ trace (`stage`, `tier_used`, `confidence_kind` ∈ `('ExactMatch','ModelVerdict','DeterministicFallback')`, `verdict_only`, `attempt_count`, `latency_ms`) vào bảng `V17a decision_audit_log` trong SQLite WAL phục vụ audit QC và Self-Evolve (`Story 13.1`).

## 4. Tasks
- [ ] **Task 09.2.1**: Cài đặt `DecisionCascadeClient` (~450 LOC trong `crates/domain` + `crates/bot-fleet`), máy trạng thái 5 bước `DE -> Decision #1 -> DR/DE -> DE -> Decision #2` và `Local FTS5 BM25` / `gemini-decision-1` / `TypeSafe JEV` providers (thỏa mãn `AC-01`).
- [ ] **Task 09.2.2**: Xây dựng `ConfidenceKind`, `egress_guard` (`4KB/64KB` + `<untrusted_artifact_bytes>`), `VerdictShield` (`0%` float leak) và `RubricShield` (`(X,Y)/(Y,X)` `>=90%`) (thỏa mãn `AC-02`).
- [ ] **Task 09.2.3**: Tích hợp `MAX_ATTEMPTS = 3` oscillation breaker, `StaleFile` OCC (di sản từ `09.3`), offline fallback và ghi vết bảng `V17 decision_audit_log` + endpoints `/api/v1/decision/*` (thỏa mãn `AC-03`, `AC-04`).
- [ ] **Task 09.2.4**: Viết bộ kiểm thử tự động: 0% float leak (`VerdictShield`), `RubricShield >= 90%` swap consistency, `MAX_ATTEMPTS=3` circuit breaker, `egress_guard` secret scrub & offline FTS5 fallback (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 09.2.1`, `Task 09.2.4` | 5-Stage Decision Cascade State Machine Test (`cargo test`) |
| **AC-02** | `Task 09.2.2`, `Task 09.2.4` | `VerdictShield` (0% Float Leak), `egress_guard` & `RubricShield >=90%` Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 09.2.3`, `Task 09.2.4` | `MAX_ATTEMPTS=3` Breaker, `StaleFile` OCC & Offline Fallback Test |
| **AC-04** | `Task 09.2.3`, `Task 09.2.4` | `V17 decision_audit_log` & `/api/v1/decision/audit` Persistence Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["02.1", "02.4", "04.6", "08.5"]

### Impacts
- Là trái tim định tuyến & thẩm định System-1 cho `Story 05.5` (DAG discovery), `Story 06.5` (Slide gen), `Story 10.1` (`3-Lane Router`), `Story 10.3` (`RubricShield` Tournament) và `Story 13.1` (Eval Telemetry).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["02.1", "02.4", "04.6", "08.5"] (KV-cache skill router, Craft/Opt skills, In-Chat Zero-Leak Key Vault, 7 Built-in DE tools).

### Shared Entities
- Sở hữu ghi trên `decision_audit_log` (`V17`); đọc `PROJECTS`, `SKILL_PLUGINS`, `component_patterns` (`V16`) trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ đủ 5-stage cascade `Local BM25 -> gemini-decision-1/JEV -> Fallback` |
| **2. Data Integrity & State** | `9.4/10` | `StaleFile` OCC + audit bất biến vào `V17 decision_audit_log` |
| **3. Security & Validation** | `9.6/10` | `egress_guard` `4KB/64KB` + `<untrusted_artifact_bytes>` + `0%` float leak |
| **4. Performance & Scalability** | `9.5/10` | Gọn ~450 LOC Rust (không tách crate thừa `jev-engine`), offline `<5ms` |
| **5. Error Handling & Recovery** | `9.5/10` | `MAX_ATTEMPTS = 3` breaker + fallback ngay khi offline/429 |
| **6. Architectural Depth & Leverage** | `9.5/10` | Đạt chuẩn ADR-006 (`RubricShield >= 90%`) |
| **7. UX Empathy** | `9.4/10` | Tự động lùi FTS5 khi user chưa nhập key, không chặn trải nghiệm |
| **TOTAL AVERAGE** | **9.49/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 09.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-20 | 2.0.0 | [refactor-story] Tái kiến trúc thành Decision Cascade Engine (DecisionCascadeClient ~450 LOC, loại bỏ crates/jev-engine theo ADR-006), thêm 5-stage DE/DR, VerdictShield, RubricShield, MAX_ATTEMPTS=3 (từ 09.3), V17 decision_audit_log & dependencies ["02.1", "02.4", "04.6", "08.5"] | /refactor-story |
