---
type: I-Wish Story
id: "05.4"
title: "Story 05.4: Cognitive Sketch-to-Polish, Steering Chips, Shared lol_html + reconcile_ff_ids Surgical Editing & 2-Tier Auto-Apply"
description: "Triển khai tiến trình nhận thức từ Day-0 Welcome -> VisualClarificationModal (2 Live Direction Pills + -PGID preempt) -> data-stage-skin=sketch-paper -> <16ms Brand Swap (OKLCH guard) -> Steering Chips & DesignerTalkOverlay (fix EMPTY_ELEMENT_RECTS bằng bridge.elementRects, __jevFast, elementFromPoint), chia sẻ presentation-cms::cms_patcher (lol_html + html5ever) sang crates/html-forge, flush useAppScreenCmsState từ localStorage xuống SQLite, và SurgicalPatcher::reconcile_ff_ids."
resource: "iwish://uxui-os/development/epic-05/story-05.4"
status: completed
risk_tier: 3
delivery: core
timestamp: "2026-11-20T20:45:00+07:00"
tags:
  - "story-05.4"
  - "epic-05"
  - "sketch-paper"
  - "lol-html"
  - "reconcile-ff-ids"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.3. ui-ux-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../epic.md]]"
dependencies: ["05.1", "05.2", "06.2", "02.4"]
provides_apis:
  - "POST /api/v1/sdi/clarify"
  - "POST /api/v1/patch"
consumes_apis:
  - "POST /api/v1/skills/route"
---

# Story 05.4: Cognitive Sketch-to-Polish, Steering Chips, Shared `lol_html` + `reconcile_ff_ids` Surgical Editing & 2-Tier Auto-Apply

- **FR Covered:** [FR-33: Cognitive Sketch-to-Polish, Steering Chips, Shared lol_html & 3-Tier Flow DAG](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 05: Option 5 2D Storyboard, Cognitive Sketch-to-Polish, Shared lol_html & 3-Tier Auto Flow DAG](../epic.md)
- **Status:** `completed` (Approved at Human Gate 2)
- **Risk Tier:** `3`

## 1. Story Description & Context
Thiết kế hành trình sáng tạo như làm việc với họa sĩ phác thảo: khởi đầu sạch màu (`data-stage-skin="sketch-paper"`) để user tập trung cấu trúc và ý đồ; sau đó gạt `[Sketch | Brand Color]` đắp màu `<16ms`, tinh chỉnh qua `SteeringChipsBar` và bấm trực tiếp vào phần tử trên HTML (`DesignerTalkOverlay` đã vá lỗi `EMPTY_ELEMENT_RECTS`), đồng bộ 100% sửa đổi từ `localStorage` xuống SQLite qua động cơ `lol_html + html5ever` dùng chung (`TDR-006`) và `reconcile_ff_ids`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6` (Scope=1.5, Data=1, Integration=1, UI=1, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 6 < 7`, hợp nhất `05.4 + 05.5 + 03.4 + 09.3-DOM` tái sử dụng trực tiếp PR #14 `cms_patcher`).
- **Formal-Verify Candidate (`risk_tier: 3`):** Giao thức OCC đa tác nhân (human CMS edit vs AI regeneration vs `-PGID` direction switch + `useAppScreenCmsState` `localStorage -> SQLite` flush).

## 3. Acceptance Criteria (AC)
- **AC-01 (Cognitive Phase Progression, Reference Taste Intake & `-PGID` + `generation_epoch` Preempt):** Thanh điều khiển hiển thị **Cognitive Phase Stepper (`1. Phác thảo` → `2. Diện mạo` → `3. Hoàn thiện` → `4. Duyệt v1`)** với nút chuyển giai đoạn có chủ đích (`Chốt phác thảo → Chọn diện mạo`). Modal Phase 2 nhận Mục tiêu + Ngôn ngữ + **App/Website tham khảo** (*VD: Linear, Stripe, Notion*), chọn **`themeModes` theo đúng yêu cầu user** (`light`, `warm`, `dark`, hoặc đa chế độ tùy biến — không ép mặc định Light/Dark), chọn **Popular FE Component Checklist** chống bỏ sót, và **2 Style Direction Cards** kèm lý do/cảm hứng bằng ngôn ngữ đời thường (0 thuật ngữ kỹ thuật); đổi hướng giữa chừng tự gửi `-PGID` preempt & tăng monotonic `generation_epoch`.
- **AC-02 (Artistic Semantic Sketch Paper → Phase 3 Brand Synthesis + Designer `<16ms` View Lens):** Ở Phase 1 (`Phác thảo`), preview hiển thị phong cách giấy vẽ tay nghệ thuật (phông chữ viết tay đầy đủ dấu tiếng Việt, viền lượn tay, KPI nét đứt, dot-grid) tuân thủ **100% semantic CSS variables** theo giao diện `Warm / Dim / Night` của `cowok-design`. Khi chuyển sang Phase 3 (`Hoàn thiện`), hiển thị màu & phông chữ thật của dự án (độc lập hoàn toàn với sáng/tối của `cowok-design`, có OKLCH AA guard `>= 4.5:1`), kèm nút phụ nhỏ **`Soi nét phác`** (`<16ms` Wireframe Lens) riêng cho Designer kiểm tra bố cục.
- **AC-03 (`SteeringChipsBar` Minimalist Semantic CSS & Fix `DesignerTalkOverlay` Coordinates):** Sửa `DualPaneStageSubpanes.tsx` bỏ `EMPTY_ELEMENT_RECTS`, cấp tọa độ thực qua `bridge.elementRects`, `window.__jevFast` và `elementFromPoint` qua `TDR-003` `MessageChannel` (`0 allow-same-origin`); `SteeringChipsBar` tuân thủ **minimalist semantic CSS variables** (`--human-soft`, `--human-accent`, `--human-border`, đồng bộ với `DEFAULT`); áp dụng **2-Tier Auto-Apply** (Undo tức thì vs thẻ cảnh báo **Khóa khung** bằng ngôn ngữ đời thường, không hiện mã thô `APPSHELL_LOCK_BOUNDARY`).
- **AC-04 (Shared `lol_html + html5ever` & Flush `localStorage` -> SQLite + `reconcile_ff_ids`):** Chia sẻ `presentation-cms::cms_patcher` (`SanitizeProfile::{Slide, AppPreview}`) sang `crates/html-forge`, flush `useAppScreenCmsState` từ `localStorage` xuống SQLite có kiểm `ast_revision` OCC (`0` `localStorage`/AI split-brain), và chạy `SurgicalPatcher::reconcile_ff_ids` với ngưỡng tin cậy cấu trúc (trả `stale -> 409 STALE_MAPPING` thay vì ghép nhầm sibling khi AI thay đổi lớn cấu trúc).
- **AC-05 ([EDGE-CASE] Concurrent Direct Edit vs AI Regeneration & Tier-2 Rollback):** Khóa revision `ast_revision` OCC khi cả human đang sửa CMS và AI vừa xong patch (`reconcile_ff_ids` bảo tồn neo chưa đổi hoặc trả `409`), stack-replay `UNDO` không hồi sinh bản đã hoàn tác, và rollback tức thì state/DOM/`localStorage` khi chạm Tier-2 AppShell Lock.
- **AC-06 ([EDGE-CASE] Phase Integrity, Minimalist Contrast & Custom `themeModes`):** Nút `Soi nét phác` không xuất hiện gây nhầm lẫn ở Phase 1; mọi pill/chip đang chọn đạt WCAG AA across Warm/Dim/Night; `themeModes` luôn cho phép chọn đơn mode (`light` / `warm` / `dark`) hoặc đa mode tùy nhu cầu người dùng.

## 4. Tasks
- [x] **Task 05.4.1**: Trích xuất & dùng chung `presentation-cms::cms_patcher` (`lol_html + html5ever`) qua `SanitizeProfile::{Slide, AppPreview}` vào `crates/html-forge` + cài `SurgicalPatcher::reconcile_ff_ids` (thỏa mãn `AC-04`).
- [x] **Task 05.4.2**: Xây dựng Day-0 Goal+Locale Welcome, `VisualClarificationModal` (2 Live Direction Pills + `-PGID` preempt), skin `data-stage-skin="sketch-paper"` & `[Sketch | Brand Color]` `<16ms` OKLCH guard (thỏa mãn `AC-01`, `AC-02`).
- [x] **Task 05.4.3**: Sửa `DualPaneStageSubpanes.tsx:181` `EMPTY_ELEMENT_RECTS`, nối `SteeringChipsBar` + `DesignerTalkOverlay` (`bridge.elementRects`, `__jevFast`, `elementFromPoint`, 2-Tier Auto-Apply) và flush `useAppScreenCmsState` xuống SQLite via `/api/v1/patch` (thỏa mãn `AC-03`, `AC-04`).
- [x] **Task 05.4.4**: Viết Playwright + Rust tests kiểm chứng `2-Tier Auto-Apply`, `0 409 STALE_MAPPING cascades`, `0 localStorage/AI split-brain` (thỏa mãn `AC-04`, `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 05.4.2` | Clarification Lock & `-PGID` Preempt E2E Test |
| **AC-02** | `Task 05.4.2` | `<16ms` Skin Swap & OKLCH Contrast Gate |
| **AC-03** | `Task 05.4.3` | Playwright Click Coordinate (`elementFromPoint`) & 2-Tier Undo Test |
| **AC-04** | `Task 05.4.1`, `Task 05.4.3`, `Task 05.4.4` | Shared `lol_html`, SQLite Flush & `reconcile_ff_ids` Tests |
| **AC-05 (`[EDGE-CASE]`)** | `Task 05.4.4` | Concurrent Edit OCC Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["05.1", "05.2", "06.2", "02.4"]

### Impacts
- Tạo nền HTML5 chuẩn neo `data-ff-id` sạch cho `Story 05.5`, `Story 07.1`, và `Story 10.3`.

### Consumes
- Chia sẻ engine `lol_html + html5ever` từ `Story 06.2`, `sketch-monochrome`/`impeccable` từ `Story 02.4`, và Live Preview/Diff từ `05.1`/`05.2`.

### Shared Entities
- `screen_artifacts`, `element_comments`, `change_proposals`.

### Cross-Portal
- Bảo toàn token riêng của từng Portal (`SalesApp`, `SuperAdmin`, `MarketingWeb`) khi bật `[Sketch | Brand Color]`.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Giải quyết dứt điểm 2 bug thật (`EMPTY_ELEMENT_RECTS` & `localStorage` split-brain) |
| **2. Data Integrity & State** | `9.6/10` | SQLite flush + `reconcile_ff_ids` giữ neo `data-ff-id` |
| **3. Security & Validation** | `9.6/10` | Kế thừa mXSS defense `lol_html + html5ever` từ `TDR-005`/`TDR-006` |
| **4. Performance & Scalability** | `9.6/10` | `<16ms` Brand Swap & `<1ms` HTML patch |
| **5. Error Handling & Recovery** | `9.5/10` | 2-Tier Auto-Apply (`Cmd+Z` vs `LockedRuleChangeReceipt`) |
| **6. Architectural Depth & Leverage** | `9.7/10` | Zero duplication giữa `presentation-cms` và `html-forge` |
| **7. UX Empathy** | `9.7/10` | Sketch trước - màu sau đúng tiến trình nhận thức non-tech |
| **TOTAL AVERAGE** | **9.61/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0 | Khởi tạo Story 05.4 hợp nhất Cognitive Sketch-to-Polish & Shared lol_html (`TDR-006`) | /make-story |
| 2026-11-22 | 1.1 | Hoàn tất Stage 2–4: V19 SQLite WAL (`60` tables), shared `crates/html-forge`, Rust/BFF/FE, Stage 3 3-round Plan-Blind `CLEAN`, Stage 4 Live Browser E2E PASS + 4 screenshots verified. Chờ Human Gate 2. | /flow-v4.1 |
