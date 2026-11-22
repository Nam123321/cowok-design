# ADR-006: Rust `DecisionCascadeClient`, Pooled 1-Chromium CDP, Shared `lol_html` Engine & Plugin-Isolated PPTX (`TDR-006`)

**Status:** ACCEPTED  
**Date:** 2026-11-20  
**Deciders:** System Architect, AI Engineering Lead, Performance & Security Leads  

## 1. Context
Sau khi hợp nhất PR #14 (`Epic 06` hoàn thành trên `master` với `crates/presentation-cms` sử dụng `lol_html + html5ever`) và chốt phương án tích hợp Decision Engine + Multi-Viewport Visual QC + Deck Gen AI, hệ thống phát sinh 3 rủi ro kiến trúc nếu giữ thiết kế cũ:
1. Tách riêng `crates/jev-engine` sẽ nhân đôi DTO serialization và tạo split-brain với `crates/domain` và `crates/bot-fleet`.
2. Sinh mới engine HTML5 sanitizer cho App Preview sẽ trùng lặp logic mXSS với `crates/presentation-cms` (đã có `lol_html + html5ever` ở `TDR-005`).
3. Khởi tạo Chromium mới cho từng viewport/slide gây bùng nổ RAM (`>2 GB`) và dễ vượt SLA `<=2s` cho mỗi lượt kiểm tra `lib-audit` / `slides-check`.

## 2. Decision
1. **Hợp nhất Decision Engine vào `crates/domain` + `crates/bot-fleet` (`~450 LOC` Rust `DecisionCascadeClient`):**
   - Loại bỏ `crates/jev-engine` khỏi danh sách crates độc lập để giữ nguyên cấu trúc 11 crates tối gọn của Cargo Workspace.
   - `crates/domain` định nghĩa `ConfidenceKind`, `VerdictShield` (`0%` raw float leak), và `RubricShield` schema; `crates/bot-fleet` thực thi 3 tầng cascade (`Local FTS5 BM25` $\to$ `gemini-decision-1` / `TypeSafe JEV` $\to$ Offline Deterministic MCDA fallback) cùng `egress_guard` (`4KB/64KB`, `<untrusted_artifact_bytes>`) và `MAX_ATTEMPTS = 3` oscillation breaker.
2. **Dùng chung `lol_html + html5ever` qua `SanitizeProfile::{Slide, AppPreview}` trong `crates/html-forge`:**
   - Chia sẻ lõi patcher/sanitizer từ `crates/presentation-cms::cms_patcher` sang `crates/html-forge`, kết hợp `SurgicalPatcher::reconcile_ff_ids` cho cả TSX và HTML5 AppPreview.
3. **Pooled Single-Chromium Rust CDP:**
   - Duy trì tối đa `<=1` tiến trình Chromium dùng chung cho `1440/1024/768/390` + `320..1440` sweep + `1920x1080` `slides-check`, thực thi `elementsFromPoint` occlusion + RGBA SSIM thật, có `iframe_bridge_fallback` gắn nhãn `UNVERIFIED_NO_BROWSER` nếu host không có Chromium.
4. **Plugin-Isolated `.pptx` Exporter:**
   - Lõi `KERNEL_CORE` xuất self-contained offline `.html` + Vector `1920x1080` `.pdf` qua CDP `Page.printToPDF`; định dạng `.pptx` đóng gói dưới dạng `EXTENSION_PLUGIN` tách biệt.

## 3. Consequences
- Giữ vững số lượng crate gọn nhẹ (`11` crates), không phát sinh crate dư thừa (`crates/jev-engine`).
- Bảo đảm parity 100% giữa lớp phòng thủ mXSS của Slide (`Epic 06`) và App Preview (`Story 05.4`).
- Kiểm soát chặt RAM và thời gian kiểm định `<=2s` cho Visual QC Gate (`Story 07.1`).
