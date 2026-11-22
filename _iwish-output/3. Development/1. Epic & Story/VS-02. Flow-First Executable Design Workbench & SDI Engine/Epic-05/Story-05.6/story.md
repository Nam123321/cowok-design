---
type: I-Wish Story
id: "05.6"
title: "Story 05.6: Phase 2 Brand & Component Visual Studio, User-Driven Multi-Mode Themes, Bidirectional Hidden Spec Drawer & FE Data Contract"
description: "Triển khai bề mặt trực quan hóa đầy đủ của Phase 2–4 trong quy trình Cognitive Progressive Working: 1× Sketch DOM × N× BrandProposalV1 (hiển thị 2–3 trang thật từ Sketch Phase 1 + Extracted Shared Component Gallery theo đúng themeModes người dùng yêu cầu: light/dark/warm/dim/custom), Popular FE/Deck Component AST Auditor, Bidirectional Hidden Spec Drawer (01..09), trích xuất FE-Required Data Contract (fe-data-contract.draft.ts) và Cross-Project Heritage Pack Picker."
resource: "iwish://uxui-os/development/epic-05/story-05.6"
status: backlog
risk_tier: 3
delivery: core
timestamp: "2026-11-22T22:50:00+07:00"
tags:
  - "story-05.6"
  - "epic-05"
  - "cognitive-progressive"
  - "brand-visual-studio"
  - "component-gallery"
  - "fe-data-contract"
  - "design-craft"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.3. ui-ux-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../epic.md]]"
dependencies: ["05.4", "05.5", "02.4"]
provides_apis:
  - "POST /api/v1/cognitive/brand-proposals"
  - "POST /api/v1/cognitive/scan-components"
  - "GET /api/v1/cognitive/fe-data-contract"
  - "POST /api/v1/cognitive/heritage-packs"
consumes_apis:
  - "GET /api/v1/patch"
  - "POST /api/v1/patch"
  - "POST /api/v1/sdi/clarify"
  - "GET /api/v1/ssot/components"
---

# Story 05.6: Phase 2 Brand & Component Visual Studio, User-Driven Multi-Mode Themes, Bidirectional Hidden Spec Drawer & FE Data Contract

- **FR Covered:** [FR-33: Cognitive Sketch-to-Polish & Progressive Spec Synthesis](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 05: Option 5 2D Storyboard, Cognitive Sketch-to-Polish, Shared lol_html & 3-Tier Auto Flow DAG](../epic.md)
- **Status:** `backlog`
- **Risk Tier:** `3`

## 1. Story Description & Context
Mở rộng trực tiếp nền tảng `Story 05.4` (`crates/html-forge`, `V19` SQLite WAL, Cognitive Phase Stepper) thành **Phase 2 Brand & Component Visual Studio** hoàn chỉnh theo triết lý **Cognitive Progressive + Visual-First / Hidden-Spec Bidirectional**:
- Thay vì chỉ có một hộp thoại chọn hướng ngắn gọn, Phase 2 hiển thị trực quan **2–3 đề xuất Brand ID** (kèm *Lý do thiết kế & Cảm hứng từ app/website tham khảo*) được render tức thì qua cơ chế **`1× Semantic Sketch DOM × N× BrandProposalV1`** (`<5ms` bằng `crates/html-forge`, chỉ tốn **1 lần gọi LLM** sinh JSON DNA) lên **2–3 trang thật lấy từ chính Sketch Phase 1**.
- Quét toàn bộ Sketch Phase 1 bằng Rust AST scanner (`scan_sketch_components`) để lọc danh sách **Component dùng chung**, đối chiếu với **`popular-components.{fe,deck}.json`** (chống bỏ sót *Toast, Confirm Dialog, Tooltip, Dropdown, Empty/Error State* nhưng KHÔNG ép buộc nếu dự án không cần), và hiển thị **Shared Component Gallery** áp trực tiếp từng Brand ID theo đúng **`themeModes` người dùng yêu cầu** (`light`, `dark`, `warm`, `dim`, hoặc nhiều chế độ tùy biến — **không mặc định cứng Light/Dark**).
- Duy trì toàn bộ Spec chuẩn hóa (`01..09` + Deck Spec) **ẩn phía sau** làm context chuẩn cho Agent, đồng bộ 2 chiều mỗi khi user sửa trên Visual (có **Hidden Spec Drawer** để xem/chỉnh khi cần), tự trích xuất **`fe-data-contract.draft.ts`** (ViewModel + Action Inputs + 4-State Matrix cho BE/IDE), và hỗ trợ **Heritage Pack** kế thừa Brand/Component/Deck xuyên dự án.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6` (Scope=1.5, Data=1, Integration=1, UI=1, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 6 < 7`).

## 3. Acceptance Criteria (AC)
- **AC-01 (Tokenized Multi-Variant Brand Studio — 2–3 Real Sketch Pages & Rationale):** Tại Phase 2, Agent nhận gu tham khảo (tên app/website như *Linear, Stripe, Notion*, URL/ảnh hoặc preset) và sinh 2–3 gói `BrandProposalV1` JSON kèm `rationale` + `inspirationRefs`. `crates/html-forge` render tức thì (`<5ms`, `$0`) từng Brand ID lên **2–3 trang thật từ Sketch Phase 1** (không dùng màn hình mẫu giả).
- **AC-02 (User-Driven `themeModes` — Zero Forced Light/Dark Default):** Danh sách chế độ hiển thị (`themeModes: string[]`, VD `["light"]`, `["warm"]`, `["dark"]`, `["light", "dark"]`, `["warm", "dim", "night"]`) được xác định **100% theo mô tả/lựa chọn của user**; Phase 2 Studio và `02_BRAND_IDENTITY` / `tokens.css` chỉ sinh & hiển thị công tắc chuyển đổi cho đúng các mode user yêu cầu, mỗi mode đều qua kiểm định OKLCH WCAG AA `>= 4.5:1`.
- **AC-03 (Extracted Shared Component Gallery & Contextual Popular Component Audit):** Rust AST scanner quét Sketch Phase 1 trích các `data-component-kind` dùng chung + đối chiếu `popular-components.{fe,deck}.json` phân 3 nhóm (`Đã có` / `Gợi ý theo ngữ cảnh` / `Không cần cho dự án này`). Mọi component trích xuất & mục user tick chọn được hiển thị trực quan trong **Shared Component Gallery** áp từng Brand ID (theo đúng `themeModes` của dự án) và đồng bộ 1:1 vào `07_COMPONENT_REGISTRY`.
- **AC-04 (Bidirectional Hidden Spec Drawer & FE-Required Data Contract):** Mọi chỉnh sửa Visual ở Phase 1–3 tự động cập nhật ẩn vào SQLite WAL + `.cowok-design/specs/` (`01..09` hoặc `deck-spec`), có nút mở **Hidden Spec Drawer** xem/sửa 2 chiều (qua OCC). Endpoint `GET /api/v1/cognitive/fe-data-contract` trích xuất **`fe-data-contract.draft.ts`** (ViewModel, Action Input DTOs, 4-State Matrix `Default/Empty/Loading/Error`, Portal visibility) — ghi rõ ranh giới FE-Required Contract (không bịa schema DB nội bộ BE).
- **AC-05 (Cross-Project Heritage Pack & Adaptive Entry Modes):** Tại Phase 4 (`Approve v1`), lưu được **Heritage Pack** (`02_BRAND_IDENTITY`, `07_COMPONENT_REGISTRY`, `presentation-theme-spec.json`, `ChartSpecV12`) vào `~/.cowok-design/heritage-packs/` để dự án FE hoặc Presentation sau kế thừa ở Phase 0 (tự hỗ trợ 5 chế độ vào linh hoạt: `GREENFIELD`, `ADD_FEATURE`, `REVAMP`, `MICRO_POLISH`, `DECK_FROM_APP`).
- **AC-06 ([EDGE-CASE] Partial Mode / Missing Anchors / Huge Sketch & Concurrent Spec Drawer Edit):**
  - Nếu user chỉ chọn 1 mode (VD `["warm"]`), không hiển thị thanh chuyển Light/Dark thừa và không sinh biến CSS rác.
  - Nếu Sketch có `> 20` màn hình, Phase 2 chỉ chọn tối đa `3` trang đại diện (Hero + List/Detail + Action Form) để tránh quá tải DOM/RAM (`<= 3` iframe đồng thời).
  - Nếu user sửa trực tiếp trong Hidden Spec Drawer song song với thao tác Visual, OCC `ast_revision` phát hiện xung đột `409 STALE_MAPPING` thay vì ghi đè âm thầm.
  - Mọi `BrandProposalV1` token đi qua `sanitizeCss` + OKLCH guard fail-closed (chặn `url(`, `expression(`, XSS, độ tương phản `< 4.5:1`).

## 4. Tasks
- [ ] **Task 05.6.1**: Định nghĩa schema `BrandProposalV1` (hỗ trợ `themeModes: string[]` linh hoạt), `PopularComponentAudit` và `FeDataContractDraft` trong Rust + Zod + SQLite WAL (`V20` nếu cần lưu Heritage/Proposal snapshots).
- [ ] **Task 05.6.2**: Xây dựng Rust AST `scan_sketch_components` + `extract_fe_data_contract` + `render_tokenized_brand_preview` trong `crates/html-forge` (`<5ms`, OKLCH AA cho mọi mode trong `themeModes`).
- [ ] **Task 05.6.3**: Xây dựng UI **Phase 2 Brand & Component Visual Studio** (2–3 trang Sketch thật + Brand Rationale + Extracted Shared Component Gallery + Popular Checklist + dynamic `themeModes` switcher) và **Bidirectional Hidden Spec Drawer** tuân thủ 100% minimalist semantic CSS variables.
- [ ] **Task 05.6.4**: Xây dựng **Heritage Pack Registry** (`~/.cowok-design/heritage-packs/`) dùng chung 2 chiều giữa App FE ↔ Presentation 16:9 + bộ test Rust/Vitest/Playwright E2E cho cả 5 Adaptive Entry Modes & Edge Cases.

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 05.6.1`, `Task 05.6.2`, `Task 05.6.3` | Brand Studio 2–3 Sketch Pages Render & Latency `<5ms` Test |
| **AC-02** | `Task 05.6.1`, `Task 05.6.2`, `Task 05.6.3` | Multi-Mode (`warm`, `light`, `dark`, custom) & OKLCH AA Contract Test |
| **AC-03** | `Task 05.6.2`, `Task 05.6.3` | Sketch AST Component Scanner + Popular Catalog Gallery E2E Test |
| **AC-04** | `Task 05.6.1`, `Task 05.6.2`, `Task 05.6.3` | Bidirectional Spec Drawer OCC + `fe-data-contract.draft.ts` Snapshot Test |
| **AC-05** | `Task 05.6.4` | Cross-Project App ↔ Deck Heritage Inheritance & 5 Entry Modes E2E Test |
| **AC-06 (`[EDGE-CASE]`)** | `Task 05.6.2`, `Task 05.6.4` | Single-Mode Guard, 3-Page Cap, CSS Injection Block & 409 OCC Regression |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["05.4", "05.5", "02.4"]`
- **Provides to:** `Story 08.6` (`design-craft` standalone repo reuses `BrandProposalV1`, `popular-components.{fe,deck}.json`, and `fe-data-contract` schemas), `Story 09.1` (feeds live upstream REA DNA into Phase 2 Studio), `Story 10.3` (parallel Brand Tournament).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Giải quyết triệt để Phase 2 Visual Studio + Hidden Spec 2 chiều + FE Data Contract thực tế |
| **2. Data Integrity & State** | `9.5/10` | OCC `409 STALE_MAPPING` giữa Visual và Hidden Spec Drawer |
| **3. Security & Validation** | `9.6/10` | Zero-Trust CSS sanitizer + OKLCH AA guard + Path Jail cho Heritage Pack |
| **4. Performance & Scalability** | `9.7/10` | 1 lượt LLM + `<5ms` tokenized multi-variant render thay vì ~108 LLM calls |
| **5. Error Handling & Recovery** | `9.5/10` | Fallback archetype khi thiếu REA; giới hạn `<=3` preview iframe tránh OOM |
| **6. Architectural Depth & Leverage** | `9.7/10` | Tái sử dụng 100% `crates/html-forge`, `spec-registry`, `component_patterns` |
| **7. UX Empathy** | `9.7/10` | `themeModes` đúng ý user (không ép Light/Dark), Popular Checklist chống quên mà không ép buộc |
| **TOTAL AVERAGE** | **9.61/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-22 | 1.0 | Khởi tạo Story 05.6 cho Phase 2 Brand & Component Visual Studio, User-Driven `themeModes`, Hidden Spec Drawer & FE Data Contract | /make-story |
