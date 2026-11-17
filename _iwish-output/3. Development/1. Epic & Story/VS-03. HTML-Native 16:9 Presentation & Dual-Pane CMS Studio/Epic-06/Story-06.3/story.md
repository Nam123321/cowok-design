---
type: I-Wish Story
id: "06.3"
title: "Story 06.3: Contextual Slide Copilot (KeyedParallelCoordinator Lease) & Offline CSP-B 2D Live Interactive Charts"
description: "Cho phép chọn từng khối data-ff-id trên Slide Node SLD-* để điều phối KeyedParallelCoordinator (có pre-flush bản nháp CMS và lease lock 60s TTL, xếp hàng FIFO trên từng slide và song song đa slide) cập nhật wording hoặc 2D Live Interactive Charts offline không CDN dưới CSP-B với validator theo từng loại biểu đồ và ngưỡng chống DoS (tạm hoãn 3D widget khỏi Epic 06; Presenter Screen tách sang Story 06.4)."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.3/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-17T23:30:00+07:00"
tags:
  - "story-06.3"
  - "epic-06"
  - "uxui-os"
  - "rust-day-1"
  - "ui"
  - "data"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["06.2", "04.4"]
provides_apis:
  - "internal:epic-06/story-06.3"
  - "internal:epic-06/csp-b-2d-charts"
consumes_apis:
  - "internal:epic-06/story-06.2"
  - "internal:epic-04/story-04.4"
---

# Story 06.3: Contextual Slide Copilot (`KeyedParallelCoordinator` Lease) & Offline CSP-B 2D Live Interactive Charts

- **FR Covered:** [FR-17: Contextual Slide Agent Copilot & Offline CSP-B 2D Live Charts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-03.%20HTML-Native%2016:9%20Presentation%20&%20Dual-Pane%20CMS%20Studio/Epic-06/epic.md)
- **Evaluation Report:** [Epic 06 Evaluation & Actionable Plan](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-06/evaluation-report.md)
- **Status:** `refactored` *(Gated on Technical Spikes `S1` + `S3` + `S4` per `max_phase: "INVESTIGATE"` before coding)*
- **Risk Tier:** `3`

## 1. Story Description & Context
Thu hẹp phạm vi `Story 06.3` sau khi tách **Presenter Screen Mode** sang **`Story 06.4`** và tạm hoãn `3D widget` khỏi `Epic-06`:
1. **Offline CSP-B 2D Live Interactive Charts (`KU-03`, `AS-04`, `EC-06-08`):** Kết xuất 7 loại biểu đồ 2D tương tác (`pie`, `donut`, `100_stacked`, `bar`, `line`, `area`, `funnel`) hoàn toàn offline không phụ thuộc CDN dưới `default-src 'none'; connect-src 'none'; script-src 'nonce-...'` bằng SVG allowlisted hoặc `<canvas>` do Host điều phối (kèm overlay `[data-ff-id]` cho Comment Pin & bảng dữ liệu `DataTableView [5.21]` + `aria-label`).
2. **Chart-Type-Aware Validator & Numeric DoS Caps (`EC-06-08`, `EC-06-13`):** Rust & Zod (`deny_unknown_fields`) áp `|sum - 100.0| <= 0.05` & giá trị không âm (`>= 0`) riêng cho `pie`, `donut`, `100_stacked`, `funnel` trong khi cho phép số âm trên `bar`, `line`, `area`; chặn `NaN`/`Infinity`, tọa độ `> 1e6`, `MAX_SERIES = 12`, `MAX_POINTS_PER_SERIES = 200`, `MAX_SVG_PRIMITIVES = 2000`, `MAX_LABEL_CHARS = 120`. Khi Copilot trả `charts_json` lỗi, giữ nguyên biểu đồ hợp lệ trước đó và hiển thị nút retry `E2`.
3. **Contextual Slide Copilot, Pre-Flush & `60s` TTL Lock Lease (`BS-07`, `EC-06-09`, `EC-06-11`):** Tái sử dụng & mở rộng `KeyedParallelCoordinator` (`Story 04.4`/`04.3`) theo từng `slide_uid` (`SLD-*`), thực hiện pre-flush đồng bộ bản nháp CMS ngay trước khi stream Copilot, hiển thị huy hiệu khóa tạm thời (`60s` TTL auto-release khi worker drop/abort/timeout), đẩy Copilot batch mutation thành 1 khung Undo nguyên tử, và dùng `--agent-accent` tách biệt với `--human-accent` trên cả 4 themes (`tokens.css`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.0, Data=1.0, Integration=1.5, UI=1.0, Security=1.0, Unknowns=0 [Spike S3 & S4 Spec] $\rightarrow$ `CS = 5.5 < 7`, gọn sau khi tách Presenter View sang `Story 06.4` và hoãn `3D widget`).

## 3. Acceptance Criteria (AC)

### AC-01 (Offline CSP-B 2D Live Interactive Charts & Accessible Data Fallback) `[DATA: wording_slots, deck_slides]` `[FLOW-OUT: chart.rendered]`
WHEN the user or Copilot binds structured chart data (`pie`, `donut`, `100_stacked`, `bar`, `line`, `area`, `funnel`; `3D widget` deferred out of Epic 06) to a `[data-ff-id]` container THE chart renderer SHALL render interactive 2D visuals locally with zero external network/CDN calls under `default-src 'none'; connect-src 'none'; script-src 'nonce-...'`, preserve positioned `[data-ff-id]` geometry for spatial pins (`CMP-09`), and synchronize an accessible `aria-label` plus tabular `DataTableView [5.21]` representation in the CMS drawer.
```yaml
oracle:
  given: "Sandboxed 16:9 slide HTML with CSP default-src 'none'; connect-src 'none' and chart wording_slots payload"
  when: "Chart data updates for pie, bar, and line templates"
  then: "2D chart renders offline with zero CSP console violations, [data-ff-id] bounds stay addressable, and accessible table reflects updated values"
  invariants:
    - "INV-06.3-CSPB: Zero external script or stylesheet URLs and zero outbound network requests"
```

### AC-02 (`KeyedParallelCoordinator` Extension, Multi-Slide Parallel & Single-Slide FIFO Queue) `[FLOW-IN: epic-04.coordinator-contract]` `[FLOW-OUT: slide.copilot.completed]`
WHEN the user opens Floating Slide Chat on an anchored slide block and dispatches prompts targeting distinct slides or the same slide THE extended coordinator SHALL execute `KeyedParallelCoordinator` (`Story 04.4`) jobs concurrently across distinct `slide_uid` items (`SLD-02`, `SLD-05`) while enforcing FIFO ordering per `slide_uid`, apply Copilot wording/chart updates to `wording_slots` (`updated_by = 'copilot'`) as a single atomic Undo frame, and mark active agent surfaces with Warm/Dim-distinct `--agent-accent`.
```yaml
oracle:
  given: "Active Copilot prompts queued simultaneously for SLD-02 (twice) and SLD-05"
  when: "KeyedParallelCoordinator schedules jobs and streams responses"
  then: "SLD-02 and SLD-05 execute concurrently while SLD-02 requests run FIFO; Undo reverses the entire Copilot batch in one step"
  invariants:
    - "INV-06.3-LOCK: Slide FIFO queue preserves per-slide_uid LOCK order and Warm/Dim --agent-accent differs distinctly from --human-accent"
```

### AC-03 (`[EDGE-CASE]` Chart-Type-Aware JSON Schema, Numeric DoS Caps & Last-Valid Preservation)
IF Copilot or CMS submits proportional, cartesian, or out-of-bounds chart JSON payloads THEN THE validator SHALL run Rust and Zod checks (`deny_unknown_fields`) enforcing `|sum - 100.0| <= 0.05` and non-negative values strictly on proportional/funnel charts (`pie`, `donut`, `100_stacked`, `funnel`) while allowing signed negative values on cartesian series (`bar`, `line`, `area`), rejecting `NaN`/`Infinity`, coordinates `> 1e6`, `> 12` series, `> 200` points/series, `> 2000` SVG primitives, or `> 120` char labels with explicit error codes, and preserving the last valid rendered chart intact alongside an `E2` inline retry card (`EC-06-08`, `EC-06-13`).
```yaml
oracle:
  given: "Proportional pie dataset summing to 94%, negative cartesian bar dataset [-12, 45], and DoS payload with 500 points / Infinity"
  when: "Validator runs prior to DOM patch and SQLite write"
  then: "Invalid pie and DoS payloads fail closed preserving prior valid chart DOM with inline E2 retry UI, while negative cartesian bars succeed"
  invariants:
    - "INV-06.3-BYTE: Chart validator enforces numeric DoS bounds and never mutates DOM or DB on schema violation"
```

### AC-04 (`[EDGE-CASE]` Pre-Flush Human Drafts, `60s` TTL Lock Lease & Cancellation Release)
IF the user triggers Slide Copilot on a slide that still holds unflushed debounced CMS edits OR a Copilot worker drops SSE, times out, or is canceled THEN THE Workbench SHALL flush pending CMS drafts to Rust + SQLite WAL synchronously prior to dispatching the Copilot read context (eliminating TOCTOU races), display a slide-level CMS lock lease indicator carrying a hard **`60s` TTL**, and deterministically release the lock on stream completion, abort, or `60s` timeout (`EC-06-09`).
```yaml
oracle:
  given: "Slide SLD-03 with unflushed keystrokes in 200ms debounce buffer followed by Copilot dispatch and simulated SSE timeout"
  when: "Copilot request triggers pre-flush and then loses worker connection past 60s TTL"
  then: "Human keystrokes commit before Copilot snapshot read, CMS edits pause during active lease, and lease releases automatically on abort or 60s timeout"
  invariants:
    - "INV-06.3-LOCK: Active 60s TTL lock prevents CMS/Copilot TOCTOU and releases on worker abort or timeout"
```

## 4. Specification Layers (`L0`–`L10`, Tier 3 Compliance)

### L0 Context & Metadata
- **Story Key:** `06.3` | **Epic:** `Epic-06` (`VS-03`) | **Risk Tier:** `3` | **Delivery:** `core` (`crates/presentation-cms`, `crates/harness-daemon`, `src/features/flow`).
- **ADR References:** `ADR-002`, `ADR-003`, `TDR-003`, `TDR-005` (`2.5. architecture.md`).

### L1 User Experience
- Click khối `[data-ff-id]` ở Stage $\rightarrow$ mở Floating Slide Chat (`360x480..900x850`) gắn badge Slide `SLD-*` màu `--agent-accent` $\rightarrow$ chạy song song nhiều slide, khóa form CMS của slide đang stream tối đa `60s`, giữ biểu đồ cũ + nút retry nếu `charts_json` sai.

### L2 Interface & Contract
- `POST /api/v1/decks/:id/slides/:slideUid/copilot` & `POST /api/v1/decks/:id/slides/:slideUid/charts/validate` qua `src/api/presentation.ts` (`apiPresentation` + Zod `ChartSpecV12Schema`).
- Streaming events qua `KeyedParallelCoordinator` SSE (`Last-Event-ID` + `lease_expires_at`).

### Host Integration Contract
- **host_portal:** `DualMainPanel [5.29]` + Movable Floating Slide Chat Overlay
- **shell_mode:** `workbench-dual-pane` (floating copilot widget layered inside Stage bounds)
- **chromeless_reason:** `not_applicable`
- **router_file:** `src/features/flow/components/CompanionCanvasTab.tsx`
- **route_mount_parent:** `src/features/flow/components/CompanionDualPaneStage.tsx`
- **navigation_entry:** Spatial `[data-ff-id]` select or Chat pill on Slide card `SLD-*`
- **companion_surfaces:** `CompanionLiveHtmlRightPane` (`2D Chart`), `CompanionStructureLeftPane` (Lease Lock Badge + `DataTableView`)
- **shell_e2e_selectors:** `[data-testid="slide-copilot-chat"]`, `[data-testid="slide-copilot-lease-badge"]`, `[data-testid="chart-validation-retry"]`

### L3 Migration & Schema Evolution
- Đọc/ghi `wording_slots.charts_json`, `slot_kind = 'chart_json'`, `updated_by = 'copilot'`, và `ast_revision` từ `V12` (`Story 06.1`).

### L4 Transaction & Concurrency
- Pre-flush sync bắt buộc (chờ `PATCH_BY_FF_ID` AST commit hoàn tất trước khi đọc prompt context); `BEGIN IMMEDIATE` ghi `charts_json` kèm `fencing_token` + `ast_revision` OCC; slide lock lease tự hết hạn sau `60s`.

### L5 Idempotency & Lifecycle
- Job SSE hỗ trợ `Last-Event-ID` replay an toàn; kết quả cùng `job_id` chỉ cộng 1 Undo frame khi apply thành công.

### L6 State Machine & Transition
- Slide Copilot Lock States: `UNLOCKED` $\xrightarrow{\text{PreFlushOk}}$ `LEASE_ACTIVE (<=60s)` $\xrightarrow{\text{Done / Abort / TTL}}$ `UNLOCKED`.

### L7 Error Taxonomy
- `E_CHART_SUM_NOT_100` (`422`), `E_CHART_NEGATIVE_PROPORTIONAL` (`422`), `E_CHART_DOS_LIMIT` (`422`), `E_COPILOT_LEASE_TIMEOUT` (`504`), `E_PREFLUSH_CONFLICT` (`409`).

### L8 UI State Matrix
- `Idle Chart`, `Copilot Streaming (Lease Locked 60s)`, `Retry Chart Error (Last-Valid Preserved)`, `Parallel Multi-Slide Executing`, `Disabled on Historical SNAP-*`.

### L9 Observability & Telemetry
- Emit `copilot.preflush_ms`, `copilot.lease.expired`, `chart.validation.rejected` (by rule code), `chart.csp_render.ok`.

### L10 Clarifications & Open Questions
- Closed by `Spikes S1, S3 & S4`: `3D widget` deferred; Presenter View moved to `Story 06.4`; all open questions resolved.

## 5. Explicit Assumptions
- `Story 04.4` `KeyedParallelCoordinator` SSE streaming & fencing token infra is `completed`.
- Host handles chart table editing (`DataTableView [5.21]`) and serializes sanitized JSON matching `ChartSpecV12Schema`.

## 6. Autonomy Boundaries
- **Always:** Add boundary tests for `pie` sum tolerance (`99.96..100.04`), signed `bar`/`line` values, `NaN`/`Infinity`/`1e308` rejections, pre-flush ordering, and `60s` lease timeout release.
- **Ask First:** Enabling `3D widget` WebGL/Three.js rendering in future Epics or increasing DoS thresholds beyond `MAX_SERIES=12` / `MAX_POINTS_PER_SERIES=200`.
- **Never:** Load external CDN chart scripts inside `srcDoc`, trust unvalidated LLM `charts_json`, or leave CMS inputs deadlocked after worker termination.

## 7. Tasks
- [ ] **Task 06.3.1**: Implement offline CSP-B 2D Chart renderer (allowlisted SVG / Host-bridged `<canvas>` for 7 chart kinds, positioned `[data-ff-id]`, accessible `aria-label` + `DataTableView`) (covers `AC-01`).
- [ ] **Task 06.3.2**: Audit & extend `KeyedParallelCoordinator` (`Story 04.4`) per `slide_uid` (parallel multi-slide, per-slide FIFO, atomic single Undo frame, `--agent-accent` separation) (covers `AC-02`).
- [ ] **Task 06.3.3**: Build Rust & Zod `ChartSpecV12Schema` validator (`deny_unknown_fields`, proportional `100%` sum vs signed cartesian, DoS caps `12/200/2000/120/1e6`, last-valid preservation + `E2` retry card) (covers `AC-03`).
- [ ] **Task 06.3.4**: Implement synchronous dirty-CMS pre-flush, visual lock badge with `60s` TTL auto-release on drop/cancel/timeout, and full unit + Playwright E2E tests (covers `AC-04`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| AC1 | (Offline CSP-B 2D Live Interactive Charts & Accessible Data Fallback) `[DATA: wording_slots, deck_slides]` `[FLOW-OUT: chart.rendered]` | TBD | Yes | TBD | Yes | Pending | Task 06.3.1 |
| AC2 | (`KeyedParallelCoordinator` Extension, Multi-Slide Parallel & Single-Slide FIFO Queue) `[FLOW-IN: epic-04.coordinator-contract]` `[FLOW-OUT: slide.copilot.completed]` | TBD | Yes | TBD | Yes | Pending | Task 06.3.2 |
| AC3 | (`[EDGE-CASE]` Chart-Type-Aware JSON Schema, Numeric DoS Caps & Last-Valid Preservation) | TBD | Yes | TBD | Yes | Pending | Task 06.3.3 |
| AC4 | (`[EDGE-CASE]` Pre-Flush Human Drafts, `60s` TTL Lock Lease & Cancellation Release) | TBD | Yes | TBD | Yes | Pending | Task 06.3.4 |

## 9. Cross-Feature Dependencies
- **Story Dependencies:** `["06.2", "04.4"]`

### Impacts
- Cung cấp Live 2D Charts và Copilot edits đồng bộ vào `wording_slots` cho `Story 06.4` (Presenter Screen) và `Epic 07` (`Story 07.3` Exporter).

### Consumes
- Tiêu thụ `Story 06.2` (`wording_slots`, AST pre-flush, Undo stack) và `Story 04.4` (`KeyedParallelCoordinator`).

### Shared Entities
- Sử dụng các bảng trong `2.2. database-spec.md`: `deck_slides`, `wording_slots`, `node_execution_jobs`, `execution_events`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ Portal (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`).

## 10. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.2/10` | Scope bounded to 2D charts & Copilot after `06.4` split |
| **2. Data Integrity & State** | `9.3/10` | Pre-flush eliminates CMS vs Copilot TOCTOU race; single-frame Undo |
| **3. Security & Validation** | `9.5/10` | Zero-CDN `CSP-B`, `deny_unknown_fields`, and explicit DoS coordinate caps |
| **4. Performance & Scalability** | `9.1/10` | Caps primitives at `2000` & points at `200` to keep SVG/Canvas `<16ms` |
| **5. Error Handling & Recovery** | `9.3/10` | Preserves last-valid chart DOM & guarantees `60s` lock TTL release |
| **6. Architectural Depth & Leverage** | `9.2/10` | Directly leverages `KeyedParallelCoordinator` & `DataTableView [5.21]` |
| **7. UX Empathy** | `9.0/10` | Clear lock feedback, accessible chart table & retry without data loss |
| **TOTAL AVERAGE** | **9.23/10** | **PASSED (>= 8.5/10)** |

## 11. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 06.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Unified Contextual Slide Copilot & Parallel Slide Execution theo Master Plan | /refactor-story |
| 2026-11-17 | 3.0.0 | [evaluate-epic Reconciliation] Tách Presenter Screen Mode sang Story 06.4, hoãn 3D widget, bổ sung EARS + oracles, Tier 3, CSP-B 2D charts, DoS caps & Copilot 60s TTL lease | Evaluation Reconciliation Agent |
