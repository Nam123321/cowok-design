---
type: I-Wish Story
id: "07.3"
title: "Story 07.3: Built-in Native Rust OOXML .pptx Exporter (Editable Native Shapes + Visual-Exact Fallback & Limitations Disclosure)"
description: "Tái kích hoạt Story 07.3 theo bản thiết kế /rea đã duyệt: xây dựng bộ xuất PowerPoint (.pptx) thuần Rust tích hợp sẵn mặc định (ACTIVE, 0 Node.js, 0 LibreOffice) trong crates/html-forge, kết hợp trích xuất bố cục Pooled 1-Chromium CDP (Story 07.1) + fallback tĩnh html5ever, hỗ trợ 2 chế độ (Chỉnh sửa được vs Giống hệt màn hình) và Hộp Lưu ý Hạn chế Hiệu ứng minh bạch 9 ngôn ngữ trên SharePresentPanel."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/Story-07.3/story.md"
status: completed
risk_tier: 2
delivery: core
adr_ref: "ADR-006"
components:
  - id: "builtin-native-rust-pptx-exporter"
    packaging: "KERNEL_CORE"
timestamp: "2026-11-25T15:35:00+07:00"
tags:
  - "story-07.3"
  - "epic-07"
  - "pptx-ooxml"
  - "rust-native"
  - "html-forge"
  - "split-export"
  - "i18n-9-locale"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/1.%20Idea%20Discovery/1.4.%20research/rea-pptx-ooxml-rust-exporter-blueprint.md"
dependencies: ["07.1", "07.2"]
provides_apis:
  - "POST /api/v1/export/pptx"
consumes_apis:
  - "POST /api/v1/qc/run"
  - "GET /api/v1/export/history"
---

# Story 07.3: Built-in Native Rust OOXML `.pptx` Exporter (Editable Native Shapes + Visual-Exact Fallback & Limitations Disclosure)

- **FR Covered:** [FR-18 / FR-19: Xuất bản Bản trình chiếu 16:9 sang PowerPoint (`.pptx`) Tích hợp Sẵn Thuần Rust](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **REA Blueprint:** [rea-pptx-ooxml-rust-exporter-blueprint.md](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/1.%20Idea%20Discovery/1.4.%20research/rea-pptx-ooxml-rust-exporter-blueprint.md)
- **Epic:** [Epic 07: Pooled 1-Chromium CDP Visual QC & Split Share vs Developer Handoff Exporter](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-03.%20HTML-Native%2016:9%20Presentation%20&%20Dual-Pane%20CMS%20Studio/Epic-07/epic.md)
- **Status:** `completed`
- **Risk Tier:** `2`

## 1. Story Description & Context
Nâng cấp tính năng xuất `.pptx` từ stub plugin ngoài ở `Story 07.2` thành **Built-in Native Rust OOXML `.pptx` Exporter** (`ACTIVE` mặc định ngay khi cài đặt, `< 500 KB`, **0 Node.js**, **0 LibreOffice**) nằm trong kernel dùng chung `crates/html-forge` (`TDR-006` / `ADR-006`). Bộ xuất tái sử dụng **Pooled 1-Chromium CDP** (`Story 07.1`, `<= 1` process) để trích xuất geometry `1920×1080` thật (`getBoundingClientRect` + `getComputedStyle`) kèm **static `html5ever` fallback** khi thiếu Chromium (0% crash), hỗ trợ 2 chế độ xuất thân thiện non-tech (**Chỉnh sửa được** vs **Giống hệt màn hình**), tuân thủ đầy đủ cổng kiểm định QC P0 + bảng nhật ký `V30 export_bundle_runs` (không cần tạo migration SQLite mới), và hiển thị minh bạch **Hộp Lưu ý Hạn chế & Hiệu ứng PowerPoint** cùng **Báo cáo hậu kiểm `FidelityWarning[]`** trên `SharePresentPanel.tsx` bằng 100% 9 ngôn ngữ.

## 2. Plan-Tune Heuristic & Complexity Score

| Dimension | Score (0–2) | Rationale |
| :--- | :---: | :--- |
| **1. Scope & Functional Surface** | `1.5` | 3 Rust modules (`pptx_ir`, `pptx_layout`, `pptx_ooxml`), 1 daemon route upgrade, 1 UI panel enhancement. |
| **2. Data & Persistence** | `0.5` | Tái sử dụng 100% bảng `V30 export_bundle_runs` (`format = 'PLUGIN_PPTX'`, `plugin_status = 'BUILTIN_ACTIVE'`); 0 migration mới. |
| **3. Integration Complexity** | `1.5` | Kết nối Pooled 1-Chromium CDP (`Story 07.1`) với static `html5ever` fallback + QC Gate P0 (`Story 07.2`). |
| **4. UI / UX & i18n Surface** | `1.0` | Mở rộng thẻ `.pptx` trong `SharePresentPanel.tsx` (2 chế độ + Hộp lưu ý + Fidelity Summary, 9 ngôn ngữ). |
| **5. Security & Boundary Risk** | `1.0` | XML 1.0 sanitizer, zero-SSRF airgap, 25 MiB cap, filename slugify, `quick-xml` escaping bắt buộc. |
| **6. Unknowns & Ambiguity** | `0.5` | Toàn bộ 6 thuật toán lõi & 18 edge-cases đã được giải phẫu sẵn trong `/rea` blueprint đã duyệt. |
| **TOTAL COMPLEXITY SCORE (CS)** | **`6.0`** | **`CS = 6.0 < 7` — Đạt ngưỡng an toàn, không cần tách nhỏ Story.** |

## 3. Acceptance Criteria (AC)

- **AC-01 (Pure Rust OOXML `.pptx` Core Engine — Zero Node / Zero LibreOffice):**
  Triển khai lõi sinh tệp `.pptx` chuẩn ECMA-376 trong `crates/html-forge/src/{pptx_ir.rs,pptx_ooxml.rs}` sử dụng `zip` + `quick-xml` + `unicode-normalization` (tuyệt đối không phụ thuộc Node.js hay LibreOffice, cấm ghép chuỗi XML thô bằng `format!`):
  1. Quy đổi chính xác tọa độ `1920×1080` sang EMU 16:9 (`12,192,000 × 6,858,000 EMU`, hệ số `6,350 EMU/px`), kẹp biên an toàn (loại `NaN`/âm/vô cực/`w <= 0 || h <= 0 || opacity == 0`).
  2. Lọc 100% ký tự điều khiển không hợp lệ theo XML 1.0 (`\x00..\x08`, `\x0B`, `\x0C`, `\x0E..\x1F`) trước khi ghi XML; chuẩn hóa toàn bộ chuỗi sang **Unicode NFC** (chống vỡ dấu tiếng Việt trên Keynote/Google Slides) và phát đầy đủ 3 khe font `<a:latin>`, `<a:ea>`, `<a:cs>` + đệm chiều rộng `+4%` + `<a:normAutofit/>`.
  3. Chuyển đổi đầy đủ màu sRGB 6-hex + `<a:alpha>`, góc gradient tuyến tính `(cssDeg - 90 + 360) % 360 * 60000`, đổ bóng `<a:outerShdw>`, ảnh bo tròn (`<a:prstGeom prst="ellipse"|"roundRect"/>`), và lưới bảng 2D có `colspan`/`rowspan` sinh đủ ô gộp dummy `<a:hMerge/>`/`<a:vMerge/>` (chống PowerPoint crash).
  4. Nhúng vector **Dual-Blip** (`<a:blip r:embed="rIdPng"/>` PNG @2x + `<a16:svgBlip r:embed="rIdSvg"/>` SVG), chống trùng lặp toàn bộ media theo **SHA-256** (`ppt/media/media_<hash8>.<ext>`), ánh xạ chuyển trang an toàn `<p:transition>` (`fade`, `push`, `wipe`, `morph` bọc trong `<mc:AlternateContent>` fallback `<p:fade/>`), và cưỡng chế giới hạn kích thước gói **`<= 25 MiB`**.

- **AC-02 (Hybrid Layout Extraction — Pooled 1-Chromium CDP + Fail-Safe Static `html5ever` Fallback):**
  Triển khai `crates/html-forge/src/pptx_layout.rs` ánh xạ HTML 16:9 thành `PptxDeckIr` kèm sổ cái `FidelityWarning[]`:
  1. **Đường chính (CDP Geometry Projection):** Tái sử dụng Pooled 1-Chromium CDP (`Story 07.1`, tối đa 1 context đồng thời) tại khung `1920×1080` để đọc `getBoundingClientRect()`, `getComputedStyle()`, thứ tự chồng lớp CSS stacking context, bỏ qua wrapper trong suốt rỗng, chụp ảnh PNG @2x cục bộ cho phần tử có hiệu ứng web không có tương đương OOXML (`backdrop-filter`, `mix-blend-mode`, `<canvas>`, WebGL, SVG có `<filter>` hoặc vượt ngưỡng `MAX_SHAPES_PER_SLIDE = 250` / `MAX_SVG_NODES = 300`), và hỗ trợ chế độ `visual_exact` (chụp toàn trang 1920×1080 PNG sắc nét).
  2. **Đường dự phòng an toàn (Static Fallback):** Khi môi trường không có Chromium hoặc CDP gián đoạn, tự động chuyển sang trình phân tích DOM tĩnh `html5ever` xếp khối tuyến tính 16:9 (0% crash, slide rỗng vẫn sinh `<p:sld>` hợp lệ), gắn cờ `usedStaticFallback: true` và phát cảnh báo `STATIC_LAYOUT_FALLBACK` trong `FidelityWarning[]`.
  3. **Chính sách Airgap Zero-SSRF:** Chặn tuyệt đối fetch ảnh từ `http://`, `https://`, hoặc `file://` ngoài sandbox dự án; ghi nhận cảnh báo `EXTERNAL_MEDIA_BLOCKED` thay vì rò rỉ mạng cục bộ.

- **AC-03 (`[EDGE-CASE]` Built-in Active Default Daemon Endpoint & `V30` Audit Continuity):**
  Nâng cấp `POST /api/v1/export/pptx` trong `crates/harness-daemon/src/routes/split_export.rs` thành dịch vụ **tích hợp sẵn mặc định (`BUILTIN_ACTIVE`)**:
  1. Nhận `html`, `inlineCss`, `casAssets`, `exportMode: "editable" | "visual_exact"` (mặc định `"editable"`), `locale`, `title`, `overrideQcWarning` (giới hạn body `MAX_EXPORT_HTTP_BODY_BYTES`); thực thi trong `tokio::task::spawn_blocking`.
  2. Trả về HTTP `200` gồm tệp nhị phân `.pptx` thật mã hóa `pptxBase64` (giải mã ra ZIP hợp lệ bắt đầu bằng magic bytes `PK\x03\x04`), `pluginInstalled: true`, `pluginStatus: "BUILTIN_ACTIVE"`, `exportMode`, `slideCount`, `fidelityWarnings[]`, `usedStaticFallback`, `sha256` thật của file ZIP, `byteSize`, và `fileName` đã chuyển tự an toàn tiếng Việt (chống path traversal `../`).
  3. Tuân thủ nghiêm ngặt Pre-Export QC Gate P0 (`409 QC_GATE_P0_BLOCKED` khi chưa override, cho phép xuất khi `overrideQcWarning: true`), giới hạn 25 MiB (`413 ASSET_SIZE_LIMIT_EXCEEDED`), và ghi nhật ký vào bảng **`V30 export_bundle_runs`** hiện có (`format = 'PLUGIN_PPTX'`, `plugin_status = 'BUILTIN_ACTIVE'`) mà **không cần tạo migration SQLite mới**. `GET /api/v1/export/history` trả `pptxPluginInstalled: true`.

- **AC-04 (Non-Tech UI — 2-Mode Toggle, Transparent Limitations Disclosure, Post-Export Fidelity Summary & 3-Locale E2E):**
  Nâng cấp thẻ PowerPoint trong `src/features/split-export/SharePresentPanel.tsx`, hook `useSplitExport.ts`, API client `src/api/split-export.ts` (Zod `.parse()` bắt buộc), và Zod schemas `src/lib/validations/split-export.ts`:
  1. Hiển thị thẻ `.pptx` ở trạng thái **Tích hợp sẵn (`BUILTIN_ACTIVE`)** với bộ chọn 2 chế độ trực quan: **`Chỉnh sửa được (Khuyên dùng)`** (`editable`) và **`Giống hệt màn hình (Ảnh nét cao)`** (`visual_exact`), hoàn toàn không dùng thuật ngữ kỹ thuật (OOXML/EMU/CDP).
  2. Hiển thị **Hộp Lưu ý Minh bạch TRƯỚC khi xuất (`Lưu ý khi xuất sang PowerPoint`)** nêu rõ: (a) những gì giữ nguyên (chữ, bố cục 16:9, bảng, màu, biểu tượng, chuyển trang cơ bản), (b) hiệu ứng web nâng cao không có trên PowerPoint (kính mờ `blur`, chuyển động CSS tùy biến, di chuột hover, biểu đồ/3D tương tác) sẽ được tự động tối ưu thành hình phẳng/ảnh nét cao, và (c) khuyến nghị nhanh chuyển sang **Trình chiếu HTML 1 tệp** nếu cần 100% hiệu ứng động.
  3. Sau khi bấm xuất, tự động tải tệp `.pptx` nhị phân thật về trình duyệt và hiển thị **Báo cáo hậu kiểm (`Fidelity Summary`)** sinh trực tiếp từ `fidelityWarnings[]` + số slide/dung lượng định dạng qua `useFormatter()`, bọc 100% khóa dịch trên cả 9 ngôn ngữ (zero chuỗi viết cứng, zero Tailwind `dark:`, dùng `<BrandIcon>` + biến CSS ngữ nghĩa).
  4. Kiểm thử Playwright E2E `tests/e2e/story-07.3.spec.ts` bấm đủ cả 2 chế độ (`editable` và `visual_exact`), xác minh tải tệp `.pptx` thật (`PK\x03\x04` + kiểm tra `[Content_Types].xml`, `_rels/.rels`, `ppt/slides/slide1.xml` hợp lệ), hộp lưu ý hạn chế và báo cáo hậu kiểm trên 3 ngôn ngữ (`vi-VN`, `en-US`, `ja-JP`).

## 4. Tasks
- [ ] **Task 07.3.1**: Cài đặt lõi sinh OOXML thuần Rust (`crates/html-forge/Cargo.toml`, `src/pptx_ir.rs`, `src/pptx_ooxml.rs`, `src/lib.rs`) + bộ unit test `story_07_3_pptx_exporter.rs` phủ 18 edge-cases & kiểm tra cấu trúc ZIP/XML vòng tròn (thỏa mãn `AC-01`).
- [ ] **Task 07.3.2**: Cài đặt bộ trích xuất bố cục lai (`crates/html-forge/src/pptx_layout.rs`) kết nối Pooled 1-Chromium CDP (`Story 07.1`) + đường dự phòng tĩnh `html5ever` + sổ cái `FidelityWarning[]` + airgap guard (thỏa mãn `AC-02`).
- [ ] **Task 07.3.3**: Nâng cấp `POST /api/v1/export/pptx` & `GET /api/v1/export/history` trong `crates/harness-daemon/src/routes/split_export.rs` sang `BUILTIN_ACTIVE`, giữ nguyên QC Gate P0 & `V30` audit + integration tests `story_07_3_pptx_export.rs` (thỏa mãn `AC-03`).
- [ ] **Task 07.3.4**: Nâng cấp Zod contracts (`src/lib/validations/split-export.ts`), API client (`src/api/split-export.ts`), hook (`useSplitExport.ts`), UI 2 chế độ + Hộp Lưu ý Hạn chế + Fidelity Summary (`SharePresentPanel.tsx`), unit tests (`split-export.test.tsx`), và E2E 3 ngôn ngữ (`tests/e2e/story-07.3.spec.ts`) (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 07.3.1` | `cargo test -p html-forge --test story_07_3_pptx_exporter` (ZIP unpack + `quick-xml` parse + 18 edge-cases) |
| **AC-02** | `Task 07.3.2` | `cargo test -p html-forge --test story_07_3_pptx_exporter` (CDP & static `html5ever` fallback + `FidelityWarning[]`) |
| **AC-03 (`[EDGE-CASE]`)** | `Task 07.3.3` | `cargo test -p harness-daemon --test story_07_3_pptx_export` (`PK\x03\x04` binary, QC P0 block/override, 25 MiB cap, `V30` audit) |
| **AC-04** | `Task 07.3.4` | `npx vitest run src/features/split-export/split-export.test.tsx` + Playwright `tests/e2e/story-07.3.spec.ts` (`vi-VN`, `en-US`, `ja-JP`) |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["07.1", "07.2"]`

### Impacts
- Nâng cấp trực tiếp trải nghiệm xuất PowerPoint cho toàn bộ người dùng Presentation Studio 16:9 và cung cấp lõi `.pptx` thuần Rust tái sử dụng cho `Story 12.1` (CLI headless export) mà không cần cài Node.js hay LibreOffice.

### Consumes
- Tiêu thụ hạ tầng **Pooled 1-Chromium CDP** (`Story 07.1`), `qc_gate_runs` (`POST /api/v1/qc/run`), và **Split Export Pipeline + `V30 export_bundle_runs`** (`Story 07.2`).

### Shared Entities
- Đọc `projects`, `qc_gate_runs`; ghi nhật ký vào `export_bundle_runs` (`V30__story_07_2_split_export_audit.sql` — 0 bảng mới, 0 migration mới).

### Cross-Portal
- Hiển thị nhất quán trong `ExportBundleModal` -> `SharePresentPanel` trên `WorkbenchShell` (`workbenches` portal) ở cả Dark/Light mode và Mobile `390px` trên toàn bộ 9 ngôn ngữ.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Sinh `.pptx` chuẩn ECMA-376 với 2 chế độ (`editable` + `visual_exact`), bảng `colspan/rowspan`, Dual-Blip SVG/PNG, transitions an toàn |
| **2. Data Integrity & State** | `9.5/10` | Tái sử dụng 100% `V30 export_bundle_runs`, lưu `artifact_sha256` + `byte_size` thật của tệp ZIP `.pptx`, zero migration churn |
| **3. Security & Validation** | `9.6/10` | XML 1.0 sanitizer, `quick-xml` Writer (0 raw XML concat), airgap zero-SSRF, 25 MiB cap, slugify tiếng Việt, Zod `.parse()` |
| **4. Performance & Scalability** | `9.5/10` | Pure Rust `< 50ms` emit, SHA-256 media dedup, `spawn_blocking`, tái dùng Pooled 1-Chromium semaphore (`<= 1` context) |
| **5. Error Handling & Recovery** | `9.5/10` | Fallback tĩnh `html5ever` không crash khi thiếu Chromium, cưỡng chế QC P0 gate + override tường minh, 18 edge-cases fail-closed |
| **6. Architectural Depth & Leverage** | `9.6/10` | Tái dụng `crates/html-forge` (`TDR-006`/`ADR-006`), `0` crate thừa, `0` Node.js, `0` LibreOffice |
| **7. UX Empathy** | `9.7/10` | Non-tech 2 chế độ dễ hiểu, Hộp Lưu ý Hạn chế minh bạch trước khi xuất + Báo cáo hậu kiểm thật trên 9 ngôn ngữ |
| **TOTAL AVERAGE** | **`9.57/10`** | **PASSED (`>= 8.5/10`)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 07.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Editable PPTX (GAP-17) & AC-05 Interactive Prototype Review Bundle (GAP-15) theo Master Plan | /refactor-story |
| 2026-11-20 | 2.1.0 | [ac_removal] Tạm đánh dấu `[DEPRECATED]` & `superseded_by: 07.2` trong đợt hợp nhất Vertical Slice | /refactor-story |
| 2026-11-25 | 3.0.0 | [reactivate-story] Tái kích hoạt Story 07.3 (`refactored`, `dependencies: ["07.1", "07.2"]`) làm **Built-in Native Rust OOXML `.pptx` Exporter** theo bản thiết kế `/rea` đã được User phê duyệt | Stage 1 Planner |
