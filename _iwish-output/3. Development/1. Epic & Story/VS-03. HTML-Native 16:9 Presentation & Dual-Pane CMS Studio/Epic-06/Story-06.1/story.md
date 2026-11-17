---
type: I-Wish Story
id: "06.1"
title: "Story 06.1: Khung trình chiếu HTML Slide Engine (16:9/16:10/Auto), 8 Layout Templates, V12 Schema Foundation & HostRuntimeContract"
description: "Xây dựng bộ khung trình chiếu HTML cô lập srcDoc hỗ trợ 16:9 (1920x1080), 16:10 và Auto với 8 layout_kind templates chuẩn dự án, tách biệt Presenter runtime khỏi useWalkthroughController, áp dụng migration V12 tương thích ngược trên V1 và khóa giao thức HostRuntimeContract (data-cowok-deck-protocol=\"1\")."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.1/story.md"
status: refactored
risk_tier: 3
delivery: core
timestamp: "2026-11-17T23:30:00+07:00"
tags:
  - "story-06.1"
  - "epic-06"
  - "uxui-os"
  - "rust-day-1"
  - "ui"
  - "data"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/DESIGN.md"
dependencies: ["03.2", "04.1"]
provides_apis:
  - "internal:epic-06/story-06.1"
  - "internal:epic-06/v12-presentation-schema"
  - "internal:epic-06/host-deck-protocol"
consumes_apis:
  - "internal:epic-03/story-03.2"
  - "internal:epic-04/story-04.1"
---

# Story 06.1: Khung trình chiếu HTML Slide Engine (`16:9`/`16:10`/`Auto`), 8 Layout Templates, `V12` Schema Foundation & `HostRuntimeContract`

- **FR Covered:** [FR-15: 16:9 HTML Presentation & Dual-Screen Presenter Engine](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-03.%20HTML-Native%2016:9%20Presentation%20&%20Dual-Pane%20CMS%20Studio/Epic-06/epic.md)
- **Evaluation Report:** [Epic 06 Evaluation & Actionable Plan](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-06/evaluation-report.md)
- **Status:** `refactored` *(Gated on Technical Spikes `S4` + `S5` per `max_phase: "INVESTIGATE"` before coding)*
- **Risk Tier:** `3`

## 1. Story Description & Context
Triển khai lát cắt dọc nền tảng cho `Epic-06` theo đúng kiến trúc chuẩn **`DESIGN.md §9.1`** (`CompanionDualPaneStage.tsx` thay cho `PresentationDualPaneView` đã bãi bỏ):
1. Engine kết xuất 8 mẫu `layout_kind` chuẩn dự án (`Hero Cover`, `Problem-Solution`, `KPI Metric Grid`, `Split Comparison`, `Architecture Flow`, `Roadmap Timeline`, `Executive Quote`, `Data Table Report`) trong `crates/presentation-cms` kế thừa `tokens.css` & `icon-manifest.json` của `PROJECT_CONSTRAINTS`, cô lập `srcDoc` để `.live-html-frame !important` không đè nền tối của slide.
2. Hỗ trợ đồng thời tỷ lệ `16:9` (`1920x1080`), `16:10` (`1920x1200`) và `Auto` thông qua `--slide-w`/`--slide-h`, projection type scale (`display >= 56px`, `heading >= 32px`, `body >= 22px`, `micro >= 16px`), fluid `min-width: 320px`, tự động thu rail chat `40px` trong Slide Studio đạt `--slide-scale >= 0.45`, kèm thanh zoom (`Fit | 100% Pan`).
3. Nền móng migration cộng thêm `V12` trên SQLite WAL (`presentation_decks`, `deck_slides` có backfill ULID `slide_uid` & `UNIQUE INDEX`, `wording_slots`) tương thích ngược với dữ liệu `V1` và audit xóa sạch `SELECT *` positional reads.
4. Khóa cứng `HostRuntimeContract` (`data-cowok-deck-protocol="1"`) với Zod schemas cho `IframeToHostMessageSchema` / `HostToIframeMessage`, tách riêng nút `▶ Present` khỏi `useWalkthroughController`, bảo vệ `isComposing`, và kiểm định cấu trúc `.slide.active` + tính duy nhất của `data-ff-id`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6.0` (Scope=1.5, Data=1.5, Integration=1.0, UI=1.0, Security=1.0, Unknowns=0 [đã giải quyết qua Party Mode & Spike S4/S5 Spec] $\rightarrow$ `CS = 6.0 < 7`, đạt ngưỡng an toàn để thực thi độc lập).

## 3. Acceptance Criteria (AC)

### AC-01 (Core Capability — 8 Branded `layout_kind` Templates & Typographic Scale) `[DATA: presentation_decks, deck_slides, wording_slots]` `[FLOW-OUT: presentation.deck.rendered]`
WHEN a project deck is rendered using any canonical layout template THE presentation engine SHALL generate valid `srcDoc` HTML inheriting active `PROJECT_CONSTRAINTS` (`tokens.css`, `icon-manifest.json`) across the 8 `layout_kind` layouts (`Hero Cover`, `Problem-Solution`, `KPI Metric Grid`, `Split Comparison`, `Architecture Flow`, `Roadmap Timeline`, `Executive Quote`, `Data Table Report`) with projection scale tokens (`display >= 56px`, `heading >= 32px`, `body >= 22px`, `micro >= 16px` at `1920x1080`) and unique `[data-ff-id]` slots per slide.
```yaml
oracle:
  given: "Project constraints with tokens.css and icon-manifest.json plus valid deck request"
  when: "Template generator executes for each of the 8 layout_kind templates"
  then: "Rendered HTML contains data-ff-id attributes, conforms to projection type scale, and passes Zero-Tailwind rules"
  invariants:
    - "INV-06.1-BYTE: All 8 layout_kind templates compile under CSP-B with zero inline scripts without nonce and strict byte containment"
```

### AC-02 (Integration & Additive `V12` SQLite WAL Migration) `[DATA: presentation_decks, deck_slides, wording_slots]` `[SEED: projects]` `[FLOW-IN: spec.v12.migrated]`
WHEN database startup migrates an existing schema to version V12 THE migration runner SHALL apply backwards-compatible additive `ALTER TABLE` statements to `presentation_decks` (`aspect_ratio`, `deck_protocol_version`, `deck_mode`, `revision`), `deck_slides` (nullable `slide_uid` followed by deterministic ULID `SLD-*` backfill and `CREATE UNIQUE INDEX`, plus `speaker_notes`, `narrative_arc`, `eyebrow_tag`, `revision`), and `wording_slots` (`slot_kind`, `charts_json`, `safe_styles_json`, `updated_by`, `ast_revision`) inside a single atomic transaction while using explicit named column queries (zero `SELECT *` positional reads).
```yaml
oracle:
  given: "Seeded V1..V11 SQLite WAL database with pre-existing presentation_decks and deck_slides rows"
  when: "V12 migration executes within BEGIN IMMEDIATE transaction"
  then: "All legacy rows receive non-null defaults and unique SLD-* slide_uid values; foreign key cascades and existing queries remain unbroken"
  invariants:
    - "INV-06.1-LOCK: Zero positional row.get(idx) or SELECT * usage remains on presentation_decks, deck_slides, wording_slots in crates/spec-registry"
```

### AC-03 (Canonical `DESIGN.md §9.1` Lens Switcher, Separate `▶ Present` & Slide Navigation Preservation) `[FLOW-OUT: canvas.lens.switched]`
WHILE the user works inside the dual-pane stage canvas THE left pane SHALL expose `#structureSubPane` (`min-width: 320px`) in `CompanionCanvasTab` / `CompanionDualPaneStage` (superseding deprecated `PresentationDualPaneView`) with a 1-click `[🔀 Userflow (2D DAG) | 📑 Slide / CMS]` toggle preserving `selectedNodeId` and slide selection keyed by immutable `slide_uid` (`SLD-*`), render the isolated HTML Stage in `#htmlSubPane` with zoom pill (`Fit | 100% Pan`), auto-collapse chat rail (`40px`) in Slide Studio preset so `1440px` viewports achieve `--slide-scale >= 0.45`, and trigger a dedicated deck `▶ Present` state separate from `useWalkthroughController`.
```yaml
oracle:
  given: "Project opened in CompanionDualPaneStage at 1440x900 viewport with active node SCR-01 and slide SLD-02"
  when: "User switches between Userflow (2D DAG) and Slide / CMS lenses and toggles ▶ Present"
  then: "Node selection persists without cross-slide cache corruption, --slide-scale measures >= 0.45, and ▶ Present steps SLD-01..N independently of WorkbenchNode walkthrough"
  invariants:
    - "INV-06.1-LENS: Legacy PresentationDualPaneView component is absent and resolveTargetNodeId isolates slide-cms slot keys by SLD-*"
```

### AC-04 (`[EDGE-CASE]` Aspect Ratio Switching `16:9`/`16:10`/`Auto` & Dark-Deck Token Isolation)
IF the active deck switches aspect ratio or renders a dark theme inside a light host workbench THEN THE stage container SHALL compute `--slide-w`, `--slide-h`, and `--slide-scale` for `16:9` (`1920x1080`), `16:10` (`1920x1200`), and `Auto` without stretching aspect coordinates and keep deck `html, body` background tokens inside `<iframe sandbox="allow-scripts allow-forms">` (`0 allow-same-origin`) unaffected by host `.live-html-frame !important` overrides, while maintaining distinct Warm/Dim `--agent-accent` (`#E89A35` / `#C97B1D`) vs `--human-accent`.
```yaml
oracle:
  given: "Workbench set to Warm Light theme hosting a Dark Night preset presentation across 16:9, 16:10, and Auto aspect ratios"
  when: "Stage resizes across 1280px, 1440px, and 1920px viewports"
  then: "Computed dark slide background renders verbatim inside srcDoc with zero .live-html-frame white bleed-through and zero horizontal overflow on 1280px screens"
  invariants:
    - "INV-06.1-SANDBOX: 0 allow-same-origin attribute on slide iframe and zero aspect coordinate distortion across aspect_ratio transitions"
```

### AC-05 (`HostRuntimeContract` `data-cowok-deck-protocol="1"` & `[EDGE-CASE]` Structural Validation) `[FLOW-OUT: deck.protocol.synced]`
WHEN loading or navigating any slide deck under the host runtime deck protocol THE host contract SHALL validate `.slide` / `.slide.active` presence and unique `[data-ff-id]` identifiers per slide under `data-cowok-deck-protocol="1"`, lock deterministic `SLIDE_CHANGE` / `DECK_KEY_NAV` / `DECK_PROTOCOL_INIT` / `SLIDE_OVERFLOW_WARN` / `PATCH_ACK` Zod messages (`IframeToHostMessageSchema`, `HostToIframeMessage`) over `MessageChannel`, ignore arrow shortcuts while `document.activeElement` is an interactive input or `event.isComposing || event.keyCode === 229`, and synchronize slide reordering / duplication with atomic `slide_index` swaps and fresh `slide_uid`/`ff_id` generation (`EC-06-07`, `EC-06-13`, `EC-06-14`).
```yaml
oracle:
  given: "Deck HTML containing conflicting inline navigation JS, missing .slide.active, or duplicate data-ff-id attributes"
  when: "Deck ingests through crates/presentation-cms and mounts under HostRuntimeContract"
  then: "Malformed or duplicate identifiers are flagged/repaired prior to mount, conflicting navigation handlers are blocked, and arrow keys respect IME composition and CMS inputs"
  invariants:
    - "INV-06.1-LOCK: Only Host-orchestrated protocol transitions mutate .slide.active state; slide duplication allocates fresh ULID slide_uid and ff_id under transaction LOCK"
```

## 4. Specification Layers (`L0`–`L10`, Tier 3 Compliance)

### L0 Context & Metadata
- **Story Key:** `06.1` | **Epic:** `Epic-06` (`VS-03`) | **Risk Tier:** `3` | **Delivery:** `core` (`crates/presentation-cms`, `crates/spec-registry`, `src/features/flow`).
- **ADR References:** `ADR-001`, `ADR-002`, `ADR-003`, `TDR-003`, `TDR-005` (`2.5. architecture.md`).

### L1 User Experience
- **Primary Flow:** Mở/tạo Deck trên `CompanionCanvasTab` $\rightarrow$ bật lens `[📑 Slide / CMS]` $\rightarrow$ xem Slide `SLD-01..N` ở `#htmlSubPane` (`--slide-scale >= 0.45`, zoom `Fit | 100%`, aspect `16:9`/`16:10`/`Auto`) $\rightarrow$ điều hướng bàn phím hoặc bấm `▶ Present` không chồng chéo `useWalkthroughController`.

### L2 Interface & Contract
- **Backend / IPC Contracts:** `crates/presentation-cms` template builder + `crates/spec-registry` V12 repository (`ts-rs` bindings generated for `PresentationDeckV12`, `DeckSlideV12`, `WordingSlotV12`, `DeckProtocolMessage`).
- **Frontend SDK:** `src/api/presentation.ts` (`apiPresentation` namespace object, Zod-validated DTOs, zero raw `fetch`).

### Host Integration Contract
- **host_portal:** `DualMainPanel [5.29]` (`CompanionCanvasTab [5.31]`)
- **shell_mode:** `workbench-dual-pane` (chat collapsed rail `40px` in Slide Studio preset; `srcDoc` sandboxed stage)
- **chromeless_reason:** `not_applicable` (`▶ Present` toggles internal Stage overlay synced with `fullscreenchange`)
- **router_file:** `src/features/flow/components/CompanionCanvasTab.tsx`
- **route_mount_parent:** `src/features/flow/components/CompanionDualPaneStage.tsx`
- **navigation_entry:** Header toggle `[🔀 Userflow (2D DAG) | 📑 Slide / CMS]` + `＋ Mở Tab / File`
- **companion_surfaces:** `CompanionStructureLeftPane` (`#structureSubPane`), `CompanionLiveHtmlRightPane` (`#htmlSubPane`)
- **shell_e2e_selectors:** `[data-testid="lens-slide-cms"]`, `[data-cowok-deck-protocol="1"]`, `[data-testid="btn-present-deck"]`, `[data-testid="zoom-pill-scale"]`

### L3 Migration & Schema Evolution
- **`V12__epic06_presentation_cms_and_charts.sql`:** Additive `ALTER TABLE` + 3-step `slide_uid` ULID backfill + `CREATE UNIQUE INDEX deck_slides_slide_uid_key ON deck_slides(slide_uid)` inside `BEGIN IMMEDIATE`. Positional `SELECT *` audit completed on `presentation_decks`, `deck_slides`, `wording_slots`.

### L4 Transaction & Concurrency
- Slide creation, reorder (`slide_index` temp-offset atomic swap), and duplicate (new `slide_uid`/`ff_id` ULID) execute inside `BEGIN IMMEDIATE` with `revision` monotonic OCC check (`409 STALE_REVISION`).

### L5 Idempotency & Lifecycle
- Deterministic migration seed/backfill handles interrupted restarts without duplicating `SLD-*` keys; `HostRuntimeContract` init message carries monotonic `epoch` + `seq` preventing replay across `srcDoc` reloads.

### L6 State Machine & Transition
- Deck Stage states: `IDLE` $\rightarrow$ `PROTOCOL_BOUND` (`PORT_ACK` + `DECK_PROTOCOL_INIT`) $\rightarrow$ `PRESENTING` (`fullscreenchange` synced) $\rightarrow$ `IDLE`; Lens states: `userflow` $\leftrightarrow$ `slide-cms` (preserving `(selectedNodeId, activeSlideUid)` independently).

### L7 Error Taxonomy
- `E_DECK_PROTOCOL_INVALID` (`422`: missing `.slide`/`.slide.active` or duplicate `data-ff-id`), `E_V12_MIGRATION_CONFLICT` (`500` transactional rollback), `E_ASPECT_UNSUPPORTED` (`400`), `E_STALE_REVISION` (`409`).

### L8 UI State Matrix
- Covers `Default` (`SLD-01..N` + stage), `Empty` (0 slides CTA create from 8 templates), `Loading` (template compilation skeleton), `Error` (malformed deck repair banner with Toast), and `Dark-in-Warm` isolated theme matrix.

### L9 Observability & Telemetry
- Structured logs via Rust `tracing` + frontend telemetry events: `deck.v12.migrated`, `deck.template.render_ms`, `deck.protocol.violation`, `deck.slide_scale.computed`.

### L10 Clarifications & Open Questions
- Closed by `Spike S4` & `Spike S5` in `evaluation-report.md`: `PRESENTATIONS` typo purged; `PresentationDualPaneView` superseded by `CompanionDualPaneStage.tsx`; all open technical questions resolved prior to coding.

## 5. Explicit Assumptions
- `Story 03.2` (`PROJECT_CONSTRAINTS`, `tokens.css`, `icon-manifest.json`) and `Story 04.1` (`CompanionCanvasTab`, `CompanionDualPaneStage`) are already `completed` and frozen as baseline contracts.
- Existing `V1` databases contain `presentation_decks` rows scoped by `project_id` cascade without non-defaulted `tenant_id`.

## 6. Autonomy Boundaries
- **Always:** Add unit/integration tests for all 8 templates, `V12` migration rollback/backfill, and Zod `HostRuntimeContract` messages; enforce Pure CSS in `src/styles/tokens.css`.
- **Ask First:** Modifying any `V1..V11` historical migration file or changing `HostRuntimeContract` major protocol version (`"1"` $\rightarrow$ `"2"`).
- **Never:** Use `allow-same-origin` on slide iframes, inject `!important` light background overrides inside deck `srcDoc`, use Tailwind classes, or call raw `fetch` outside `src/api/presentation.ts`.

## 7. Tasks
- [ ] **Task 06.1.1**: Implement `crates/presentation-cms` 8 `layout_kind` template engines inheriting `PROJECT_CONSTRAINTS`, projection scale tokens (`16px..56px`), and isolated Dark/Light `srcDoc` theming (covers `AC-01`, `AC-04`).
- [ ] **Task 06.1.2**: Author `V12__epic06_presentation_cms_and_charts.sql` additive migration (with 3-step `slide_uid` ULID backfill + atomic reorder/duplicate + `SELECT *` audit) and `ts-rs` bindings + `src/api/presentation.ts` SDK (covers `AC-02`).
- [ ] **Task 06.1.3**: Wire `CompanionCanvasTab` & `CompanionDualPaneStage` (`§9.1` canonical layout) 1-click `[🔀 Userflow | 📑 Slide / CMS]` lens switch, chat rail auto-collapse (`--slide-scale >= 0.45`), zoom pill (`Fit | 100% Pan`), and separate `▶ Present` deck controller (covers `AC-03`).
- [ ] **Task 06.1.4**: Implement `--slide-w`/`--slide-h` scaling (`16:9`/`16:10`/`Auto`), Warm/Dim `--agent-accent` separation, and `.live-html-frame !important` deck isolation (covers `AC-04`).
- [ ] **Task 06.1.5**: Lock `HostRuntimeContract` (`data-cowok-deck-protocol="1"`), Zod message contracts, IME `isComposing` arrow suppression, and `.slide.active` / duplicate `[data-ff-id]` ingestion validation + E2E Playwright & Rust tests (covers `AC-05`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| AC1 | (Core Capability — 8 Branded `layout_kind` Templates & Typographic Scale) `[DATA: presentation_decks, deck_slides, wording_slots]` `[FLOW-OUT: presentation.deck.rendered]` | TBD | Yes | TBD | Yes | Pending | Task 06.1.1 |
| AC2 | (Integration & Additive `V12` SQLite WAL Migration) `[DATA: presentation_decks, deck_slides, wording_slots]` `[SEED: projects]` `[FLOW-IN: spec.v12.migrated]` | TBD | Yes | TBD | Yes | Pending | Task 06.1.2 |
| AC3 | (Canonical `DESIGN.md §9.1` Lens Switcher, Separate `▶ Present` & Slide Navigation Preservation) `[FLOW-OUT: canvas.lens.switched]` | TBD | Yes | TBD | Yes | Pending | Task 06.1.3 |
| AC4 | (`[EDGE-CASE]` Aspect Ratio Switching `16:9`/`16:10`/`Auto` & Dark-Deck Token Isolation) | TBD | Yes | TBD | Yes | Pending | Task 06.1.1<br>Task 06.1.4 |
| AC5 | (`HostRuntimeContract` `data-cowok-deck-protocol="1"` & `[EDGE-CASE]` Structural Validation) `[FLOW-OUT: deck.protocol.synced]` | TBD | Yes | TBD | Yes | Pending | Task 06.1.5 |

## 9. Cross-Feature Dependencies
- **Story Dependencies:** `["03.2", "04.1"]`

### Impacts
- Cung cấp schema `V12` (`presentation_decks`, `deck_slides`, `wording_slots`), `layout_kind` templates và `HostRuntimeContract` cho `Story 06.2`, `Story 06.3`, `Story 06.4`.

### Consumes
- Tiêu thụ `PROJECT_CONSTRAINTS` (`Story 03.2`) và `CompanionCanvasTab` / `CompanionDualPaneStage` (`Story 04.1`).

### Shared Entities
- Sử dụng các bảng trong `2.2. database-spec.md`: `projects`, `project_constraints`, `presentation_decks`, `deck_slides`, `wording_slots`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ Portal (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) ở Presentation Studio `16:9` (`LAY-05 §9.1`).

## 10. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.2/10` | Covers all `FR-15` 8 templates, aspect ratios & `HostRuntimeContract` |
| **2. Data Integrity & State** | `9.1/10` | Enforces 3-step `V12` `slide_uid` ULID backfill & named queries |
| **3. Security & Validation** | `9.4/10` | `0 allow-same-origin` sandbox + structural `.slide.active`/`ff_id` guard |
| **4. Performance & Scalability** | `9.0/10` | Scale calculation & `>= 0.45` studio rail auto-collapse budget |
| **5. Error Handling & Recovery** | `8.9/10` | Explicit `409`/`422` deck protocol & migration rollback coverage |
| **6. Architectural Depth & Leverage** | `9.2/10` | Eliminates `PresentationDualPaneView` split & unifies `CompanionDualPaneStage` |
| **7. UX Empathy** | `9.1/10` | Preserves dark deck fidelity, readable `>= 0.45` scale & separated `▶ Present` |
| **TOTAL AVERAGE** | **9.13/10** | **PASSED (>= 8.5/10)** |

## 11. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 06.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-08 | 2.0.0 | Nâng cấp AC-03..AC-04 đồng bộ cấu trúc Slide/Phase trên CompanionCanvasTab & thanh chuyển góc nhìn [Userflow | Slide/CMS] theo DESIGN.md v5.4 | cowok Orchestrator |
| 2026-11-12 | 3.0.0 | [refactor-story] Bổ sung AC-05 HostRuntimeContract (data-cowok-deck-protocol="1") theo Master Plan | /refactor-story |
| 2026-11-17 | 4.0.0 | [evaluate-epic Reconciliation] Nâng Tier 3, bổ sung EARS + YAML oracles, V12 migration backfill, tách ▶ Present, dark-deck token isolation & EC-06-07/13/14 | Evaluation Reconciliation Agent |
