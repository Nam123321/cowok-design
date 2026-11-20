---
type: I-Wish Story
id: "06.4"
title: "Story 06.4: Dual-Screen Presenter Mode, Out-of-Band BroadcastChannel Security, Pace Timer & Fallback Modes"
description: "Triển khai chế độ Presenter Screen Mode tách biệt khỏi Story 06.3: đồng bộ 2 màn hình qua Host-mediated BroadcastChannel dùng sessionSecret trao đổi out-of-band (cấm ?secret=), kiểm tra origin + deckId + Zod, SYNC_FULL_STATE khi refresh, Tauri multi-webview IPC parity, Inline Split Presenter Night fallback khi popup bị chặn, hiển thị speaker_notes, đồng hồ tabular-nums, next-slide preview, phím blackout B/W + F, single-Esc fullscreenchange, khóa read-only SNAP-* và telemetry P10."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.4/story.md"
status: completed
risk_tier: 3
delivery: core
timestamp: "2026-11-17T23:30:00+07:00"
tags:
  - "story-06.4"
  - "epic-06"
  - "uxui-os"
  - "rust-day-1"
  - "ui"
  - "presenter"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/DESIGN.md"
dependencies: ["06.1", "06.2"]
provides_apis:
  - "internal:epic-06/story-06.4"
  - "internal:epic-06/presenter-broadcast-sync"
consumes_apis:
  - "internal:epic-06/story-06.1"
  - "internal:epic-06/story-06.2"
---

# Story 06.4: Dual-Screen Presenter Mode, Out-of-Band `BroadcastChannel` Security, Pace Timer & Fallback Modes

- **FR Covered:** [FR-15: Dual-Screen Presenter Mode & Broadcast Sync Engine](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 06: Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-03.%20HTML-Native%2016:9%20Presentation%20&%20Dual-Pane%20CMS%20Studio/Epic-06/epic.md)
- **Evaluation Report:** [Epic 06 Evaluation & Actionable Plan](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-06/evaluation-report.md)
- **Status:** `completed` *(Stage 3B, Stage 4, and Stage 5A verified; formally promoted with explicit user Stage 5B approval)*
- **Risk Tier:** `3`

## 1. Story Description & Context
Chế độ **Presenter Screen Mode** trước đây nằm trong tiêu đề `Story 06.3` nhưng thiếu toàn bộ ACs/Tasks (`BS-01`). Theo đồng thuận Party Mode (`Story 06.3 -> 06.3 + 06.4`), `Story 06.4` quản lý toàn bộ luồng thuyết trình 2 màn hình trên nền tảng `HostRuntimeContract` (`Story 06.1`) và `speaker_notes` (`Story 06.2`):
1. **Out-of-Band `BroadcastChannel` & Tauri Parity (`KU-02`, `EC-06-04`):** Vì sandboxed Audience `srcDoc` iframe có opaque origin (`0 allow-same-origin`), chỉ Host React mới làm trung gian nối `Audience MessageChannel <-> Host BroadcastChannel (scoped sessionSecret) <-> Presenter Window` (hoặc Tauri multi-webview IPC). `sessionSecret` bắt buộc trao đổi out-of-band qua `window.opener`/`MessageChannel` (cấm `?secret=` trên URL), kiểm tra Host `origin`, hằng-thời-gian `sessionSecret`, `deckId` và Zod schema trên cả 2 đầu; hỗ trợ `SYNC_FULL_STATE` khi refresh Presenter và `channel.close()` khi đóng.
2. **Presenter UI Console & Popup-Blocked Inline Fallback (`UX Round 2`, `EC-06-04`):** Bấm mở Presenter phải gọi `window.open(...)` **đồng bộ bên trong handler `onClick`** (không chờ `await` IPC). Hiển thị Slide hiện tại, Slide kế tiếp (slot đã pin trong `POOL_MAX_SLOTS`), `speaker_notes` đọc plain-text an toàn, và đồng hồ pace timer (`tabular-nums`, `Start` / `Pause` / `Reset`). Nếu popup bị trình duyệt chặn (`window.open === null`), hiện `.ff-toast-warning` (`[5.12]`) kèm nút **"Mở lại Presenter Window"** và bật **Inline Split Presenter Fallback** dùng Night tokens (`#0B1220` / `#172033` / `#F1F5F9`) tránh lóa ánh sáng sân khấu.
3. **Zero Key-Trap Exit, Blackout `B`/`W`, Snapshot Read-Only Guard & P10 Telemetry (`BS-04`, `BS-07`, `EC-06-14`):** Đồng bộ React `Present` với `fullscreenchange` để nhấn `Esc` 1 lần thoát ngay trên Audience stage, còn nhấn `Esc` trong Presenter Window chỉ đóng/ngắt cửa sổ phụ mà không tắt màn hình Audience. Chuyển tiếp `B` (Black screen) / `W` (White screen) / `F` từ `HostRuntimeContract`, khóa read-only khi trình chiếu snapshot lịch sử `SNAP-*`, và thu thập telemetry P10.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.0, Data=0.5, Integration=1.5, UI=1.5, Security=1.0, Unknowns=0 [Spike S5 Spec] $\rightarrow$ `CS = 5.5 < 7`, tách sạch khỏi `06.3` để thực thi độc lập).

## 3. Acceptance Criteria (AC)

### AC-01 (Host-Mediated Presenter Window, Synchronous `onClick` Launch & Out-of-Band `BroadcastChannel` Security) `[FLOW-IN: deck.protocol.synced]` `[FLOW-OUT: presenter.window.synced]`
WHEN the user activates Presenter View from Stage controls THE Host SHALL invoke `window.open(...)` synchronously within the `onClick` gesture (avoiding async IPC blocking), provision a per-session `sessionSecret` strictly out-of-band via `window.opener` / `MessageChannel` handshake (prohibiting `?secret=` query parameters) or Tauri multi-webview IPC, validate same-origin `event.origin`, constant-time `sessionSecret`, `deckId`, and Zod schema on both Host and Presenter `BroadcastChannel` listeners, synchronize active slide transitions bi-directionally with the Audience `srcDoc` `MessageChannel`, support `SYNC_FULL_STATE` rehydration on reload, and execute `channel.close()` on unload.
```yaml
oracle:
  given: "Mounted Audience deck stage with active MessageChannel and user clicking Presenter button"
  when: "Presenter window opens, sends handshake, navigates slides, reloads once, and closes"
  then: "URL contains zero ?secret= parameter, same-origin listener validates sessionSecret/deckId/Zod, slide indices stay synchronized after reload, and close() frees channel resources"
  invariants:
    - "INV-06.4-ORIGIN: Opaque-origin srcDoc iframe never gains allow-same-origin or direct BroadcastChannel access; unauthenticated same-origin tabs are rejected"
```

### AC-02 (Speaker Notes Plain-Text Viewer, `tabular-nums` Pace Timer & Pinned Next-Slide Preview) `[DATA: deck_slides]` `[FLOW-IN: cms.ast.committed]`
WHILE the Presenter View is active THE Presenter console SHALL render safe plain-text `speaker_notes` (synchronized from `Story 06.2` / `V12`), a monospace `tabular-nums` pace stopwatch supporting `Start`, `Pause`, and `Reset` transitions accurate within 1 second over 10-minute sessions, and a pinned next-slide preview (`SLD-(i+1)`) sourced from the `POOL_MAX_SLOTS` pinned slot without flickering or evicting the Audience stage frame.
```yaml
oracle:
  given: "Multi-slide deck containing HTML-like plain text in speaker_notes and populated slides SLD-01..SLD-06"
  when: "Presenter advances from SLD-01 to SLD-02 while stopwatch runs and pauses"
  then: "speaker_notes render escaped text safely with zero script execution, timer uses Geist Mono tabular-nums accurately, and next-slide preview updates to SLD-03 without reloading Audience iframe"
  invariants:
    - "INV-06.4-LOCK: speaker_notes escaping blocks HTML injection; Audience slot stays pinned under pool LOCK"
```

### AC-03 (`[EDGE-CASE]` Popup-Blocked Inline Split Presenter Fallback)
IF `window.open(...)` returns `null` due to popup-blocker restrictions THEN THE Stage SHALL display a `Toast [5.12]` `.ff-toast-warning` with a 1-click **"Mở lại Presenter Window"** action and activate an **Inline Split Presenter Fallback** panel rendered in Night surface tokens (`#0B1220` / `#172033` / `#F1F5F9`) showing `speaker_notes`, pace timer, and next-slide preview alongside the Stage (`EC-06-04`).
```yaml
oracle:
  given: "Browser context configured to block window.open popups"
  when: "User clicks Open Presenter View button"
  then: "Zero unhandled null-window exceptions occur, warning Toast offers immediate reopen action, and Night-themed Inline Split Presenter displays notes, timer, and next slide"
  invariants:
    - "INV-06.4-FALLBACK: Secondary presenter fallback preserves notes and timer access when OS popups fail"
```

### AC-04 (`[EDGE-CASE]` Single-`Esc` Focus Safety, Blackout `B`/`W`/`F` Routing, `SNAP-*` Read-Only Lock & P10 Telemetry)
WHEN keyboard events occur across Audience `iframe`, Host stage overlay (`Present`), and Presenter Window THE system SHALL bind Host overlay state to `fullscreenchange` for **single-`Esc` exit** on Audience stage while confining `Esc` inside the secondary Presenter Window to closing/unlinking only the Presenter Window, forward `B` (blackout), `W` (whiteout), and `F` keys via `HostRuntimeContract` (suppressed when `isComposing` or inside inputs), enforce `guardReadOnlyAction` on historical `SNAP-*` snapshots in Presenter View, and record P10 telemetry (`presenter.sync.latency_ms`, `presenter.popup_blocked.count`, `presenter.channel_rejected.count`) (`EC-06-09`, `EC-06-14`).
```yaml
oracle:
  given: "Active Audience stage fullscreen paired with Presenter Window plus historical SNAP-* read-only preview test case"
  when: "Presenter presses B/W blackout and Esc inside Presenter Window, then Host presses Esc once on Stage"
  then: "B/W toggles Audience blackout screen, Esc inside Presenter closes only Presenter while Audience keeps projecting, single Esc exits Audience fullscreen, and SNAP-* prevents mutation"
  invariants:
    - "INV-06.4-LOCK: Zero double-Esc traps; accidental Esc in secondary Presenter window never blanks live Audience projection and SNAP-* read-only LOCK blocks writes"
```

## 4. Specification Layers (`L0`–`L10`, Tier 3 Compliance)

### L0 Context & Metadata
- **Story Key:** `06.4` | **Epic:** `Epic-06` (`VS-03`) | **Risk Tier:** `3` | **Delivery:** `core` (`src/features/flow`).
- **ADR References:** `ADR-002`, `ADR-003`, `TDR-003`, `TDR-005` (`2.5. architecture.md`).

### L1 User Experience
- Popup Presenter chạy giao diện tối chống lóa (`#0B1220` / `#172033`), gồm Current Slide (`16:9`), Next Slide mini-preview, `speaker_notes` phóng to rõ nét, và đồng hồ `tabular-nums`; nếu trình duyệt chặn popup thì trượt mượt vào Inline Split Fallback ngay trên màn hình hiện tại.

### L2 Interface & Contract
- Typed Zod protocol: `PresenterHandshakeSchema` (`window.opener` / `MessageChannel`), `PresenterBroadcastEventSchema` (`SYNC_FULL_STATE`, `SLIDE_NAV`, `BLACKOUT_TOGGLE`, `HANDSHAKE_ACK`), và `TauriPresenterBridge` adapter.

### Host Integration Contract
- **host_portal:** `DualMainPanel [5.29]` / Secondary Same-Origin Presenter Route (`/presenter/:deckId`)
- **shell_mode:** `workbench-dual-pane` (stage fullscreen + secondary presenter popout / inline split fallback)
- **chromeless_reason:** Secondary Presenter Window & Fullscreen projection strip workbench navigation chrome intentionally to prevent audience distraction.
- **router_file:** `src/App.jsx` (`/presenter/:deckId`) & `src/features/flow/components/CompanionCanvasTab.tsx`
- **route_mount_parent:** `src/features/flow/components/CompanionDualPaneStage.tsx`
- **navigation_entry:** Stage toolbar **`Trình chiếu`** (`btn-open-presenter`, chỉ hiển thị khi `isSlideLens === true`, `onClick` synchronous `window.open`)
- **companion_surfaces:** `CompanionLiveHtmlRightPane` (Audience Stage), Presenter Popout / Inline Split Night Panel
- **shell_e2e_selectors:** `[data-testid="btn-open-presenter"]`, `[data-testid="presenter-inline-fallback"]`, `[data-testid="presenter-speaker-notes"]`, `[data-testid="presenter-timer"]`, `[data-testid="audience-blackout-overlay"]`

### L3 Migration & Schema Evolution
- Đọc `deck_slides.speaker_notes`, `slide_uid`, `slide_index`, và `presentation_decks.aspect_ratio` từ `V12` (`Story 06.1`). Không yêu cầu bảng DB mới riêng cho timer session.

### L4 Transaction & Concurrency
- Host giữ vai trò Single Source of Truth cho `activeSlideIndex` & `blackoutMode` (`none | black | white`); chỉ Host điều phối chuyển tiếp sang Audience `MessageChannel` (`epoch` + `seq`), tránh xung đột khi nhiều cửa sổ cùng gửi lệnh điều hướng.

### L5 Idempotency & Lifecycle
- Khi Presenter Window `F5`/reload, gửi `REQUEST_SYNC_FULL_STATE` lấy snapshot trạng thái hiện tại ngay sau `sessionSecret` re-handshake; mọi listener giải phóng khi `beforeunload` gọi `channel.close()`.

### L6 State Machine & Transition
- Presenter Connection States: `CLOSED` $\rightarrow$ `OPENING_POPUP` $\rightarrow$ (`AUTHENTICATED` | `POPUP_BLOCKED_INLINE_FALLBACK`) $\rightarrow$ `CLOSED`.
- Projection Blackout States: `VISIBLE` $\leftrightarrow$ `BLACKOUT (B)` $\leftrightarrow$ `WHITEOUT (W)`.

### L7 Error Taxonomy
- `E_PRESENTER_POPUP_BLOCKED` (chuyển Inline Split Fallback), `E_CHANNEL_SECRET_MISMATCH` (reject + telemetry), `E_INVALID_BROADCAST_ORIGIN` (drop), `E_HISTORICAL_READONLY_LOCK` (chặn mutate trên `SNAP-*`).

### L8 UI State Matrix
- `Connected Dual-Window`, `Popup Blocked Inline Fallback`, `Audience Blackout (B)`, `Audience Whiteout (W)`, `Paused Stopwatch`, `End-of-Deck Placeholder` (khi `SLD-last` không có Next Slide), `Historical SNAP-* Read-Only`.

### L9 Observability & Telemetry
- Emit `presenter.sync.latency_ms`, `presenter.popup_blocked.count`, `presenter.channel_rejected.count`, `presenter.rehydrate.count`.

### L10 Clarifications & Open Questions
- Closed by `Spike S5` & Party Mode Round 2: tách khỏi `06.3`, giữ `speaker_notes` input tại `06.2`, toàn bộ câu hỏi kỹ thuật đã được khóa.

## 5. Explicit Assumptions
- `Story 06.1` (`HostRuntimeContract` + `V12` `aspect_ratio`) và `Story 06.2` (`speaker_notes` persistence + `POOL_MAX_SLOTS` pinning) đã sẵn sàng làm nền tảng.
- Tauri Desktop target có API mở cửa sổ webview phụ cùng event bus IPC tương đương `BroadcastChannel` + `sessionSecret`.

## 6. Autonomy Boundaries
- **Always:** Viết Playwright test mở Presenter popup (mô phỏng đồng bộ 2 trang), test chặn popup (`window.open -> null`) kiểm chứng Inline Split Night fallback, và test single-`Esc` + `B`/`W`.
- **Ask First:** Thay đổi tuyến route `/presenter/:deckId` trong App Router hoặc thêm điều khiển WebRTC remote presentation.
- **Never:** Truyền `sessionSecret` qua URL query string `?secret=`, cấp `allow-same-origin` cho Audience slide iframe để nó tự đọc `BroadcastChannel`, hoặc cho phép `Esc` ở Presenter Window tắt nhầm Audience fullscreen.

## 7. Tasks
- [x] **Task 06.4.1**: Implement synchronous `onClick` `window.open` Presenter launcher, out-of-band `sessionSecret` handshake (`window.opener`/`MessageChannel`, zero URL secret), Origin/deckId/Zod validated Host-mediated `BroadcastChannel` relay, `SYNC_FULL_STATE`, and `channel.close()` cleanup + Tauri IPC adapter (covers `AC-01`).
- [x] **Task 06.4.2**: Build Presenter Night UI Console rendering escaped plain-text `speaker_notes`, `tabular-nums` pace stopwatch (`Start`/`Pause`/`Reset`), and pinned `POOL_MAX_SLOTS` next-slide preview (covers `AC-02`).
- [x] **Task 06.4.3**: Build Popup-Blocked `Toast [5.12]` `.ff-toast-warning` + **"Mở lại Presenter Window"** + Night-tokenized **Inline Split Presenter Fallback** (`#0B1220`/`#172033`/`#F1F5F9`) (covers `AC-03`).
- [x] **Task 06.4.4**: Wire single-`Esc` `fullscreenchange` synchronization, `B`/`W` blackout & `F` key forwarding, `guardReadOnlyAction` for `SNAP-*`, P10 telemetry, and full unit + multi-context Playwright E2E tests (covers `AC-04`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| AC1 | (Host-Mediated Presenter Window, Synchronous `onClick` Launch & Out-of-Band `BroadcastChannel` Security) `[FLOW-IN: deck.protocol.synced]` `[FLOW-OUT: presenter.window.synced]` | [Code] Path: [presenter-broadcast.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-broadcast.ts)<br>[Code] Path: [presenter-host-helpers.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-host-helpers.ts)<br>[Code] Path: [usePresenterHostTransport.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterHostTransport.ts)<br>[Code] Path: [usePresenterClientTransport.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterClientTransport.ts)<br>[Code] Path: [usePresenterSessionController.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterSessionController.ts)<br>[Code] Path: [usePresenterWindowClient.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterWindowClient.ts)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/validations/presentation-presenter.ts)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/api/presentation-presenter.ts)<br>[Code] Path: [route.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/app/api/decks/%5BdeckId%5D/presenter/handshake/route.ts)<br>[Code] Path: [route.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/app/api/projects/%5Bid%5D/decks/%5BdeckId%5D/presenter/handshake/route.ts)<br>[Code] Path: [api-guard-session.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/api-guard-session.ts)<br>[Code] Path: [api-guard.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/api-guard.ts)<br>[Code] Path: [vite-bff-routes.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/vite-bff-routes.ts)<br>[Code] Path: [presentation_presenter.rs](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/crates/harness-daemon/src/routes/presentation_presenter.rs)<br>[Code] Path: [PresenterCockpitView.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitView.tsx)<br>[Code] Path: [PresenterConsoleWindow.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterConsoleWindow.tsx)<br>[Code] Path: [App.jsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/App.jsx)<br>[Code] Path: [PresenterCockpitCards.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitCards.tsx) | No | [Test] Path: [story_06_4_presenter.rs](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/crates/harness-daemon/tests/story_06_4_presenter.rs)<br>[Test] Path: [presenter-mode-06.4.test.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/tests/presenter-mode-06.4.test.tsx)<br>[Test] Path: [presenter-mode-06.4.spec.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/tests/e2e/presenter-mode-06.4.spec.ts) | No | Completed | Task 06.4.1 |
| AC2 | (Speaker Notes Plain-Text Viewer, `tabular-nums` Pace Timer & Pinned Next-Slide Preview) `[DATA: deck_slides]` `[FLOW-IN: cms.ast.committed]` | [Code] Path: [PresenterConsoleWindow.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterConsoleWindow.tsx)<br>[Code] Path: [PresenterCockpitView.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitView.tsx)<br>[Code] Path: [PresenterCockpitCards.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitCards.tsx)<br>[Code] Path: [presenter-timer.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-timer.ts)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/api/presentation-presenter.ts)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/validations/presentation-presenter.ts)<br>[Code] Path: [usePresenterClientTransport.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterClientTransport.ts)<br>[Code] Path: [usePresenterWindowClient.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterWindowClient.ts)<br>[Code] Path: [presentation_presenter.rs](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/crates/harness-daemon/src/routes/presentation_presenter.rs)<br>[Code] Path: [usePresenterSessionController.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterSessionController.ts)<br>[Code] Path: [presenter-host-helpers.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-host-helpers.ts) | No | [Test] Path: [presenter-mode-06.4.test.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/tests/presenter-mode-06.4.test.tsx)<br>[Test] Path: [presenter-mode-06.4.spec.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/tests/e2e/presenter-mode-06.4.spec.ts)<br>[Test] Path: [story_06_4_presenter.rs](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/crates/harness-daemon/tests/story_06_4_presenter.rs) | No | Completed | Task 06.4.2 |
| AC3 | (`[EDGE-CASE]` Popup-Blocked Inline Split Presenter Fallback) | [Code] Path: [PresenterInlineSplitFallback.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterInlineSplitFallback.tsx)<br>[Code] Path: [PresenterCockpitView.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitView.tsx)<br>[Code] Path: [CompanionDualPaneStage.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/CompanionDualPaneStage.tsx)<br>[Code] Path: [usePresenterSessionController.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterSessionController.ts)<br>[Code] Path: [presenter-broadcast.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-broadcast.ts)<br>[Code] Path: [presenter-timer.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-timer.ts)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/validations/presentation-presenter.ts)<br>[Code] Path: [usePresenterHostTransport.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterHostTransport.ts)<br>[Code] Path: [PresenterCockpitCards.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitCards.tsx)<br>[Code] Path: [presenter-host-helpers.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-host-helpers.ts) | No | [Test] Path: [presenter-mode-06.4.test.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/tests/presenter-mode-06.4.test.tsx)<br>[Test] Path: [presenter-mode-06.4.spec.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/tests/e2e/presenter-mode-06.4.spec.ts) | No | Completed | Task 06.4.3 |
| AC4 | (`[EDGE-CASE]` Single-`Esc` Focus Safety, Blackout `B`/`W`/`F` Routing, `SNAP-*` Read-Only Lock & P10 Telemetry) | [Code] Path: [CompanionDualPaneStage.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/CompanionDualPaneStage.tsx)<br>[Code] Path: [CompanionCanvasTab.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/CompanionCanvasTab.tsx)<br>[Code] Path: [usePresenterSessionController.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterSessionController.ts)<br>[Code] Path: [usePresenterWindowClient.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterWindowClient.ts)<br>[Code] Path: [PresenterCockpitCards.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitCards.tsx)<br>[Code] Path: [PresenterCockpitView.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterCockpitView.tsx)<br>[Code] Path: [route.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/app/api/projects/%5Bid%5D/decks/%5BdeckId%5D/presenter/handshake/route.ts)<br>[Code] Path: [presenter-broadcast.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-broadcast.ts)<br>[Code] Path: [PresenterConsoleWindow.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/components/PresenterConsoleWindow.tsx)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/api/presentation-presenter.ts)<br>[Code] Path: [presentation-presenter.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/lib/validations/presentation-presenter.ts)<br>[Code] Path: [usePresenterHostTransport.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterHostTransport.ts)<br>[Code] Path: [usePresenterClientTransport.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/hooks/usePresenterClientTransport.ts)<br>[Code] Path: [presentation_presenter.rs](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/crates/harness-daemon/src/routes/presentation_presenter.rs)<br>[Code] Path: [presenter-host-helpers.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/utils/presenter-host-helpers.ts)<br>[Code] Path: [route.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/app/api/decks/%5BdeckId%5D/presenter/handshake/route.ts) | No | [Test] Path: [story_06_4_presenter.rs](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/crates/harness-daemon/tests/story_06_4_presenter.rs)<br>[Test] Path: [presenter-mode-06.4.test.tsx](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/src/features/flow/tests/presenter-mode-06.4.test.tsx)<br>[Test] Path: [presenter-mode-06.4.spec.ts](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/tests/e2e/presenter-mode-06.4.spec.ts) | No | Completed | Task 06.4.4 |

## 9. Cross-Feature Dependencies
- **Story Dependencies:** `["06.1", "06.2"]`

### Impacts
- Hoàn thiện trải nghiệm trình chiếu điều hành cho toàn bộ `VS-03` (`Epic 06`) độc lập với Copilot/2D Charts (`Story 06.3`).

### Consumes
- Tiêu thụ `Story 06.1` (`HostRuntimeContract` `data-cowok-deck-protocol="1"`, `V12` `deck_slides`) và `Story 06.2` (`speaker_notes`, `POOL_MAX_SLOTS` pinning).

### Shared Entities
- Sử dụng các bảng trong `2.2. database-spec.md`: `presentation_decks`, `deck_slides`, `version_snapshots`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ Portal (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`).

## 10. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.3/10` | Fully closes `BS-01` missing Presenter ACs/Tasks with Timer, Notes & Next Slide |
| **2. Data Integrity & State** | `9.1/10` | Host SSOT arbitration + `SYNC_FULL_STATE` rehydration & `SNAP-*` lock |
| **3. Security & Validation** | `9.5/10` | Out-of-band `sessionSecret` (no `?secret=`), strict origin/Zod & `0 allow-same-origin` |
| **4. Performance & Scalability** | `9.0/10` | Pinned next-slide slot eliminates iframe teardown lag on slide advance |
| **5. Error Handling & Recovery** | `9.4/10` | Popup-blocker Inline Split Night fallback ensures zero dead presenter launches |
| **6. Architectural Depth & Leverage** | `9.1/10` | Reuses `HostRuntimeContract`, `MessageChannel` & `Toast [5.12]` cleanly |
| **7. UX Empathy** | `9.3/10` | Eliminates double-`Esc` traps & protects live stage when closing Presenter popout |
| **TOTAL AVERAGE** | **9.24/10** | **PASSED (>= 8.5/10)** |

## 11. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-17 | 1.0.0 | Khởi tạo Story 06.4 (tách Presenter Screen Mode từ Story 06.3 theo Party Mode Round 2 & Evaluation Report, Tier 3, EARS + YAML oracles, EC-06-04/09/14) | Evaluation Reconciliation Agent |
| 2026-11-20 | 1.1.0 | Stage 5B approved promotion to `completed`; populated full AC-to-Task Traceability Matrix with verified implementation & test paths | Antigravity |
| 2026-11-20 | 1.2.0 | [SSOT & Party-Mode Audit Fix] Đổi nhãn nút `Diễn giả` -> `Trình chiếu` (`isSlideLens`-only trên Presentation Deck), bổ sung `usePresenterInlineCollapse` tự thu gọn cả `MainSidebar` & `#structureSubPane` (có unmount cleanup & guard `>= 1024px`), hoàn tất luồng Fork Session FE (`useSessionForkSync`, nút `Rẽ nhánh từ đây` trên chat, `cowok:project-state-invalidated` & fix `copyBranchMessages`), và chuẩn hóa error surfacing trong `usePresenterHostTransport` | Antigravity |
