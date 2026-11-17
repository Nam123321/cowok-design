---
type: I-Wish Story
id: "06.2"
title: "Story 06.2: Dual-Pane Slide-Over CMS Workbench (inert-Guarded), Two-Tier <16ms Non-Reloading Sync, lol_html + html5ever mXSS & Crash Recovery"
description: "Cung cấp ngăn kéo trượt ngang CMS trong #structureSubPane (bọc inert/aria-hidden, form động theo 8 layout_kind templates + speaker_notes), tách targetCacheKey khỏi PATCH_BY_FF_ID để đồng bộ <16ms không reload srcDoc, ghim slot trong POOL_MAX_SLOTS, bảo vệ IME isComposing, Unified Reverse-Patch Undo, crash-draft recovery, SSOT APPLY_HTML, cảnh báo tràn E3 ResizeObserver và động cơ crates/presentation-cms (lol_html + html5ever SVG/CSS token mXSS + atomic .tmp/WAL)."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.2/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-17T23:30:00+07:00"
tags:
  - "story-06.2"
  - "epic-06"
  - "uxui-os"
  - "rust-day-1"
  - "ui"
  - "data"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["06.1", "03.3"]
provides_apis:
  - "internal:epic-06/story-06.2"
  - "internal:epic-06/html5-cms-patcher"
consumes_apis:
  - "internal:epic-06/story-06.1"
  - "internal:epic-03/story-03.3"
---

# Story 06.2: Dual-Pane Slide-Over CMS Workbench (`inert`-Guarded), Two-Tier `<16ms` Non-Reloading Sync, `lol_html` + `html5ever` mXSS & Crash Recovery

- **FR Covered:** [FR-16: Dual-Pane Human CMS Texting Workbench & Two-Tier `<16ms` Sync](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-03.%20HTML-Native%2016:9%20Presentation%20&%20Dual-Pane%20CMS%20Studio/Epic-06/epic.md)
- **Evaluation Report:** [Epic 06 Evaluation & Actionable Plan](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-06/evaluation-report.md)
- **Status:** `refactored` *(Gated on Technical Spikes `S1` + `S2` per `max_phase: "INVESTIGATE"` before coding)*
- **Risk Tier:** `3`

## 1. Story Description & Context
Xây dựng toàn bộ hệ thống nhập liệu CMS cho người dùng non-tech tại nửa trái `#structureSubPane` (`translateX(-50%)`) gắn kết 2 chiều với sân khấu HTML `16:9` bên phải (`useSandboxedIframeBridge.ts`) và bộ vá HTML5 trong `crates/presentation-cms`:
1. **Dynamic Slide-Over CMS + `speaker_notes` (`KU-05`):** Tự động sinh trường nhập liệu theo từng `layout_kind` trong 8 templates (vượt qua giới hạn 5 trường tĩnh) cộng trường **`Ghi chú diễn giả (speaker_notes)`** lưu vào cột `V12`, bọc `inert` + `aria-hidden="true"` lên nửa ẩn của track `translateX(-50%)`.
2. **Two-Tier `<16ms` Non-Reloading Sync + Pool Pinning (`UU-01`, `KU-04`, `UU-05`):** Tách `targetCacheKey` khỏi `PATCH_BY_FF_ID` để không xoay vòng `resolvedSha256`/reload `srcDoc`; ghim slot Audience active, Presenter active & Presenter next-slide trong `POOL_MAX_SLOTS`; cô lập `one-page` `<section data-slide-uid="SLD-*">`; sửa stale closure `channel.port1.onmessage` bằng React `ref` + queue `32` frame coalesce pre-`PORT_ACK` + `rAF` rect throttle.
3. **`crates/presentation-cms` Hybrid HTML5 Patcher & mXSS Defense (`TDR-005`):** Khác `surgical_patcher.rs` (`oxc_parser` TSX-only), dùng **`lol_html`** vá byte-preserving `[data-ff-id]` `<1ms` + **`html5ever`** SVG/DOM allowlist + CSS token sanitizer cho `<style>` và inline `style=""` (bỏ `'unsafe-inline'`, nonce-rewrite hoặc fail-closed), cộng staging `.tmp` + `fsync` + SQLite WAL + atomic `rename()`, và audit mở rộng `Story 03.3` `patch_by_ff_id`/`postPatchByFfId`.
4. **Crash-Draft Recovery, IME Guard, Unified Undo, `APPLY_HTML` SSOT & `E3` Overflow Guard (`UU-03`, `UU-04`, `BS-03`, `KU-07`):** Bảo vệ `isComposing` (`vi-VN`/CJK), Unified Reverse-Patch Undo (`⌘Z`/`⇧⌘Z`) giữ qua lần đóng/mở drawer, `beforeunload` + `sessionStorage`/`IndexedDB` draft recovery, `APPLY_HTML` re-parse hoặc `custom_html_override` lock, và cảnh báo tràn `scrollHeight > clientHeight` (`E3`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6.5` (Scope=1.5, Data=1.5, Integration=1.5, UI=1.0, Security=1.0, Unknowns=0 [Spike S1/S2 spec đã khóa toàn bộ biên] $\rightarrow$ `CS = 6.5 < 7`, đạt ngưỡng thực thi đơn lẻ).

## 3. Acceptance Criteria (AC)

### AC-01 (Dynamic Slide-Over CMS Drawer with `inert` Guard & `speaker_notes`) `[DATA: wording_slots, deck_slides]` `[FLOW-OUT: cms.slot.edited]`
WHEN the user selects any slide card in the left structure pane THE left pane track SHALL transition `#structureSubPane` via `translateX(-50%)` (`240ms cubic-bezier`) to reveal dynamic CMS form controls mapped to the `layout_kind` schema of that slide plus a persisted `speaker_notes` field, apply `inert` and `aria-hidden="true"` strictly to the offscreen track half, move keyboard focus to the CMS header, and restore focus to the originating slide card upon clicking **`← Quay lại danh sách Slide`**.
```yaml
oracle:
  given: "Deck with slides across multiple layout_kind templates loaded in CompanionDualPaneStage"
  when: "User activates a slide card, tabs through CMS inputs including speaker_notes, and returns to slide list"
  then: "Rendered fields match the active layout_kind schema, Tab focus never enters the offscreen pane (inert enforced), and closing returns focus"
  invariants:
    - "INV-06.2-FOCUS: Inactive half of 200% translateX(-50%) container strictly carries inert=true and aria-hidden=true"
```

### AC-02 (`Story 03.3` Contract Extension & `lol_html` + `html5ever` Hybrid Rust Patcher) `[DATA: wording_slots, deck_slides]` `[FLOW-IN: epic-03.patch-contract]` `[FLOW-OUT: cms.ast.committed]`
WHEN CMS edits persist from the debounced slow write path THE presentation patcher SHALL extend `Story 03.3` `patch_by_ff_id` / `postPatchByFfId` (supporting `chart_json`, structured bullet arrays, and `revision` receipts, while keeping `crates/constraint-engine/src/surgical_patcher.rs` TSX-only) inside `crates/presentation-cms` to apply `<1ms` streaming `lol_html` updates by `[data-ff-id]` without corrupting sibling markup or triggering conflicting rewrites.
```yaml
oracle:
  given: "HTML5 slide file on disk with multiple [data-ff-id] text, metric, bullet, and chart slots plus revision N"
  when: "Debounced patch commit runs through crates/presentation-cms"
  then: "Target slot content updates in <1ms via lol_html, SQLite WAL ast_revision increments atomically to N+1, and untouched HTML bytes remain intact"
  invariants:
    - "INV-06.2-BYTE: crates/constraint-engine surgical_patcher.rs never runs on HTML5 deck files and untouched BYTE spans stay immutable"
```

### AC-03 (Unscaled `ResizeObserver` `E3` Overflow Warning & Two-Tier `<16ms` Non-Reloading `MessageChannel` Sync) `[FLOW-OUT: cms.dom.patched]`
WHILE the user types inside any CMS input control THE Host bridge SHALL dispatch `PATCH_BY_FF_ID` over the active `MessageChannel` (`<16ms`) mutating only the mounted `[data-ff-id]` DOM node without changing `targetCacheKey` (`resolvedSha256`) or reloading `srcDoc`, inspect unscaled `scrollHeight > clientHeight` via `ResizeObserver` inside the slide container to display an inline `E3` overflow warning badge on offending CMS fields, and ignore live `PATCH_BY_FF_ID` echoes while IME `compositionstart`..`compositionend` is active.
```yaml
oracle:
  given: "Mounted sandboxed slide iframe slot and user typing rapidly with Vietnamese/CJK IME into Headline CMS field"
  when: "Keystrokes arrive continuously across frames including text exceeding 1920x1080 container height"
  then: "Iframe srcDoc reload count equals 0 during typing, DOM updates within <16ms, IME diacritics remain uncorrupted, and E3 overflow badge activates on overflow"
  invariants:
    - "INV-06.2-BRIDGE: targetCacheKey stays invariant across text/metric PATCH_BY_FF_ID edits; active composition blocks programmatic input replacement"
```

### AC-04 (`[EDGE-CASE]` Pinned Keep-Alive Pool, `one-page` Scoping, `PORT_ACK` Queue & `APPLY_HTML` SSOT Resolution)
IF a multi-page deck exceeds pool capacity or runs as a single-page deck or executes manual HTML edits THEN THE iframe keep-alive bridge SHALL pin Audience active, Presenter active, and Presenter next-slide slots against eviction inside `POOL_MAX_SLOTS = 5`, scope `one-page` `<section data-slide-uid="SLD-*">` slots under shared `content_sha256` OCC, queue up to `32` coalesced frames pre-`PORT_ACK` (`500ms` retry × `3`) with `channel.port1.onmessage` reading React `ref` accessors and `rAF`-throttled `publishElementRects()`, and resolve `APPLY_HTML` split-brain by re-extracting `[data-ff-id]` into `wording_slots` or locking damaged bindings into `custom_html_override` (`EC-06-01`, `EC-06-06`, `EC-06-12`).
```yaml
oracle:
  given: "15-slide multi-page deck, 1 one-page deck, rapid slide switch before PORT_ACK, and manual APPLY_HTML edit"
  when: "User switches slides rapidly and submits APPLY_HTML breaking one structured slot binding"
  then: "Audience/Presenter slots never evict, coalesced patches deliver without drop after PORT_ACK, and unmatched CMS inputs enter locked custom_html_override state with re-sync option"
  invariants:
    - "INV-06.2-LOCK: Zero silent message drops during PORT_ACK handshake; customHtmlByTarget and wording_slots stay isolated under explicit LOCK"
```

### AC-05 (`[EDGE-CASE]` Deep mXSS / CSS Token Defense & Cross-Boundary Disk + SQLite WAL Atomicity)
IF untrusted markup contains script injection vectors or exceeds size limits or encounters disk commit failure THEN THE Rust ingestion pipeline SHALL block `<script>`, `<svg><style>`, `<foreignObject>`, `<animate*>`, `<set>`, `<use>`, `on*`, `xlink:href`, `<meta>`, `<base>`, nested `srcdoc`, `formaction`, obfuscated `javascript:`/`data:image/svg+xml`, and `</style` / `@import` / `url()` / `expression()` across `<style>` and inline `style=""`, reject `'unsafe-inline'` offline fallbacks (nonce-rewriting valid CSS tokens or failing closed with `securitypolicyviolation` telemetry), cap payloads at `512 KB` / `8000` slot chars, and enforce `.tmp` + `fsync` in `fs_jail` (`O_NOFOLLOW`) -> SQLite WAL tx -> atomic `rename()` with full transaction rollback + `.tmp` unlink on failure and startup checksum sweep (`EC-06-02`, `EC-06-05`).
```yaml
oracle:
  given: "Adversarial mXSS payload (<style></styl/*x*/e><img onerror=...>, <svg><style>...) and simulated rename() ENOSPC fault"
  when: "Sanitizer ingests HTML and atomic staging runs"
  then: "Zero unescaped scripts/event handlers or dangerous SVG/CSS tokens survive in sanitized HTML, CSP violations emit telemetry, and SQLite WAL leaves prior valid revision intact with zero orphaned .tmp files"
  invariants:
    - "INV-06.2-BYTE: Byte size <= 524288 under fs_jail O_NOFOLLOW; zero partial write state between html_path and SQLite content_sha256"
```

### AC-06 (`[EDGE-CASE]` Unified Reverse-Patch Undo/Redo, Dirty-Draft Guard & Crash Recovery Journal)
IF the user navigates or closes the tab with unflushed edits or triggers Undo across CMS transitions THEN THE Workbench SHALL prompt dirty-draft protection (`beforeunload` and route change), persist unflushed drafts in a `(slide_uid, revision)` `sessionStorage`/`IndexedDB` recovery journal, surface a `Toast [5.12]` retry/rollback banner on `SQLITE_BUSY` / OCC async failure, and execute a Unified Reverse-Patch command stack `(slot_uid, before, after, source)` surviving `translateX(-50%)` drawer unmounts (`EC-06-03`, `EC-06-10`).
```yaml
oracle:
  given: "Unsaved keystrokes inside debounce window followed by simulated SQLITE_BUSY / browser reload and subsequent Undo command"
  when: "Async flush fails, tab reloads, and user triggers Undo"
  then: "Recovery journal restores unflushed draft, failure banner offers retry, and Undo reverts DOM, CMS value, and AST revision synchronously"
  invariants:
    - "INV-06.2-UNDO: Single Undo frame atomically reverts prior edit across CMS, DOM, and persisted wording_slots"
```

## 4. Specification Layers (`L0`–`L10`, Tier 3 Compliance)

### L0 Context & Metadata
- **Story Key:** `06.2` | **Epic:** `Epic-06` (`VS-03`) | **Risk Tier:** `3` | **Delivery:** `core` (`crates/presentation-cms`, `src/features/flow`).
- **ADR References:** `ADR-002`, `ADR-003`, `TDR-003`, `TDR-004`, `TDR-005` (`2.5. architecture.md`).

### L1 User Experience
- Left `#structureSubPane` (`translateX(-50%)` + `inert`) hiển thị schema CMS động + `speaker_notes`; gõ `<16ms` mượt tiếng Việt Telex/VNI; cảnh báo tràn chữ `E3`; nút re-sync khi `APPLY_HTML` lệch cấu trúc; banner phục hồi draft khi mất kết nối/lỗi đĩa.

### L2 Interface & Contract
- Rust IPC / BFF routes (`POST /api/v1/decks/:id/slides/:slideUid/slots/batch`, `POST /api/v1/decks/:id/slides/:slideUid/apply-html`) sử dụng `apiPresentation` (`src/api/presentation.ts`) + Zod validation + `X-Cowok-Session-Token`.
- `useSandboxedIframeBridge` mở rộng `postPatchByFfId` (`textContent`, `safeStyles`, `chart_json`, `bullets`, `revision`).

### Host Integration Contract
- **host_portal:** `DualMainPanel [5.29]` (`CompanionDualPaneStage.tsx`)
- **shell_mode:** `workbench-dual-pane`
- **chromeless_reason:** `not_applicable`
- **router_file:** `src/features/flow/components/CompanionCanvasTab.tsx`
- **route_mount_parent:** `src/features/flow/components/CompanionDualPaneStage.tsx`
- **navigation_entry:** Click Slide card inside `#slideListPane` $\rightarrow$ `translateX(-50%)`
- **companion_surfaces:** `CompanionStructureLeftPane` (CMS form + `speaker_notes`), `CompanionLiveHtmlRightPane` (16:9 DOM target)
- **shell_e2e_selectors:** `[data-testid="cms-slide-over-drawer"]`, `[data-testid="cms-input-speaker-notes"]`, `[data-testid="e3-overflow-badge"]`, `[data-testid="btn-back-slide-list"]`

### L3 Migration & Schema Evolution
- Tiêu thụ bảng `wording_slots` (`slot_kind`, `charts_json`, `safe_styles_json`, `updated_by`, `ast_revision`) & `deck_slides` (`slide_uid`, `speaker_notes`, `revision`) từ `V12` (`Story 06.1`).

### L4 Transaction & Concurrency
- Write staging 2 pha (`fs_jail` `.tmp` + `fsync` -> `BEGIN IMMEDIATE` WAL `content_sha256`/`revision` -> atomic `rename()`); OCC mismatch trả `409 STALE_REVISION`; UI debounce `150–300ms` có monotonic `seq` chống out-of-order echo.

### L5 Idempotency & Lifecycle
- **Telemetry:** `L5` recovery draft clears automatically once the daemon returns a persisted `ast_revision` confirmation; `PATCH_BY_FF_ID` pre-`PORT_ACK` coalesce by `(slide_uid, ff_id)` stays idempotent.

### L6 State Machine & Transition
- Slot Sync States: `SYNCED` $\rightarrow$ `DIRTY_FAST_PATCHED` (draft journal active) $\rightarrow$ `COMMITTING` $\rightarrow$ `SYNCED` | `FLUSH_FAILED_RETRY`.
- Custom Override States: `STRUCTURED_SSOT` $\leftrightarrow$ `CUSTOM_HTML_OVERRIDE` (via `APPLY_HTML` broken bindings / Resync).

### L7 Error Taxonomy
- `E_MXSS_BLOCKED` (`422`), `E_PAYLOAD_TOO_LARGE` (`413`: `> 512 KB` / `> 8000` chars), `E_STALE_REVISION` (`409`), `E_CUSTOM_HTML_OVERRIDE_LOCK` (`409`), `E_SQLITE_BUSY_RETRY` (`503`).

### L8 UI State Matrix
- `Default` (CMS form synced), `Overflow Warning (E3 badge)`, `IME Composing`, `Custom Override Locked` (`APPLY_HTML` divergence), `Crash Draft Restored Banner`, `Flush Error Retry Toast`.

### L9 Observability & Telemetry
- Đo `cms.patch_by_ff_id.latency_ms` (mục tiêu p95 `< 16ms`), `cms.e3_overflow.count`, `cms.csp_violation.count`, `cms.sanitizer_blocked.count`, `cms.flush_failure.count`.

### L10 Clarifications & Open Questions
- Đã giải quyết toàn bộ qua `Spikes S1 & S2` trong `evaluation-report.md`; không còn câu hỏi mở chưa đóng.

## 5. Explicit Assumptions
- `Story 03.3` `patch_by_ff_id` sẽ được mở rộng backward-compatible theo kế hoạch audit ở `Spike S1`.
- Chỉ duyệt HTML5 slide dưới `512 KB`; deck lớn hơn bắt buộc dùng mode `multi-page` hoặc tối ưu asset.

## 6. Autonomy Boundaries
- **Always:** Viết unit/adversarial test cho toàn bộ payload mXSS (`<svg><style>`, `</styl/*x*/e>`, `style=""`), IME composition test và `beforeunload` crash-recovery test.
- **Ask First:** Nới lỏng danh sách allowlist của `html5ever` SVG sanitizer hoặc tăng giới hạn `512 KB`.
- **Never:** Dùng `innerHTML` hay regex comment stripping trong JS/Rust cho dữ liệu CMS, dùng `'unsafe-inline'`, hoặc bỏ qua `O_NOFOLLOW` containment khi ghi `.tmp`.

## 7. Tasks
- [ ] **Task 06.2.1**: Build Left `#structureSubPane` Slide-Over CMS Drawer (`translateX(-50%)`, `inert` + `aria-hidden` focus management, template-driven dynamic fields + `speaker_notes`) (covers `AC-01`).
- [ ] **Task 06.2.2**: Audit & extend `Story 03.3` `patch_by_ff_id` / `postPatchByFfId` and implement `crates/presentation-cms` `lol_html` `<1ms` slot patcher + WAL `ast_revision` OCC (covers `AC-02`).
- [ ] **Task 06.2.3**: Implement `<16ms` non-reloading `MessageChannel` sync (decoupled `targetCacheKey`), unscaled `ResizeObserver` `E3` overflow warning, IME `isComposing` guard, `POOL_MAX_SLOTS` pinning, `one-page` `SLD-*` scoping, `rAF` rect throttling, `ref`-safe `port1.onmessage`, `32`-frame pre-`PORT_ACK` queue, and `APPLY_HTML` SSOT resolution (covers `AC-03`, `AC-04`).
- [ ] **Task 06.2.4**: Implement `html5ever` SVG/DOM allowlist + CSS token sanitizer (`<style>` + `style=""`, zero `'unsafe-inline'`) + `.tmp`/`fsync`/WAL atomic `rename()` staging & startup sweep (covers `AC-05`).
- [ ] **Task 06.2.5**: Implement Unified Reverse-Patch Undo (`⌘Z`/`⇧⌘Z`), `beforeunload`/route dirty guard, `(slide_uid, revision)` recovery journal, `Toast [5.12]` flush-error banner, and full unit/Playwright regression suite (covers `AC-06`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| AC1 | (Dynamic Slide-Over CMS Drawer with `inert` Guard & `speaker_notes`) `[DATA: wording_slots, deck_slides]` `[FLOW-OUT: cms.slot.edited]` | TBD | Yes | TBD | Yes | Pending | Task 06.2.1 |
| AC2 | (`Story 03.3` Contract Extension & `lol_html` + `html5ever` Hybrid Rust Patcher) `[DATA: wording_slots, deck_slides]` `[FLOW-IN: epic-03.patch-contract]` `[FLOW-OUT: cms.ast.committed]` | TBD | Yes | TBD | Yes | Pending | Task 06.2.2 |
| AC3 | (Unscaled `ResizeObserver` `E3` Overflow Warning & Two-Tier `<16ms` Non-Reloading `MessageChannel` Sync) `[FLOW-OUT: cms.dom.patched]` | TBD | Yes | TBD | Yes | Pending | Task 06.2.3 |
| AC4 | (`[EDGE-CASE]` Pinned Keep-Alive Pool, `one-page` Scoping, `PORT_ACK` Queue & `APPLY_HTML` SSOT Resolution) | TBD | Yes | TBD | Yes | Pending | Task 06.2.3 |
| AC5 | (`[EDGE-CASE]` Deep mXSS / CSS Token Defense & Cross-Boundary Disk + SQLite WAL Atomicity) | TBD | Yes | TBD | Yes | Pending | Task 06.2.4 |
| AC6 | (`[EDGE-CASE]` Unified Reverse-Patch Undo/Redo, Dirty-Draft Guard & Crash Recovery Journal) | TBD | Yes | TBD | Yes | Pending | Task 06.2.5 |

## 9. Cross-Feature Dependencies
- **Story Dependencies:** `["06.1", "03.3"]`

### Impacts
- Cung cấp dữ liệu `wording_slots`, `speaker_notes` và `<16ms` patch channel cho `Story 06.3` (Copilot + Charts) và `Story 06.4` (Presenter Screen).

### Consumes
- Tiêu thụ `Story 06.1` (`V12` schema, `CompanionDualPaneStage`) và mở rộng `Story 03.3` (`patch_by_ff_id`).

### Shared Entities
- Sử dụng các bảng trong `2.2. database-spec.md`: `presentation_decks`, `deck_slides`, `wording_slots`, `chat_artifact_snapshots`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ Portal (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`).

## 10. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.3/10` | Dynamic template schema + `speaker_notes` + Unified Undo covered |
| **2. Data Integrity & State** | `9.4/10` | Resolves `UU-01`/`UU-03`/`BS-02` with `.tmp`+WAL atomicity & recovery journal |
| **3. Security & Validation** | `9.5/10` | Eliminates `<svg><style>`, CSS comment & `'unsafe-inline'` mXSS classes |
| **4. Performance & Scalability** | `9.2/10` | Decoupled `targetCacheKey` + `rAF` rect throttle keeps typing `<16ms` |
| **5. Error Handling & Recovery** | `9.2/10` | `SQLITE_BUSY` retry banner, pre-`PORT_ACK` queue & `custom_html_override` lock |
| **6. Architectural Depth & Leverage** | `9.1/10` | Clean split between `oxc_parser` TSX & `lol_html`+`html5ever` HTML5 |
| **7. UX Empathy** | `9.3/10` | Zero IME diacritic breakage & accessible `inert` drawer keyboard flow |
| **TOTAL AVERAGE** | **9.29/10** | **PASSED (>= 8.5/10)** |

## 11. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 06.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-08 | 2.0.0 | Nâng cấp AC-03..AC-04 bổ sung Slide-Over Detailed CMS Drawer (translateX(-50%)) đồng bộ <16ms sang HTML Stage | cowok Orchestrator |
| 2026-11-17 | 3.0.0 | [evaluate-epic Reconciliation] Nâng Tier 3, bổ sung EARS + YAML oracles, lol_html/html5ever mXSS, decoupled targetCacheKey, POOL_MAX_SLOTS pinning, IME/Crash recovery & EC-06-01/02/03/05/06/10/11/12 | Evaluation Reconciliation Agent |
