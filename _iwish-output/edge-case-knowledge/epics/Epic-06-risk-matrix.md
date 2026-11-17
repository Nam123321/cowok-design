# Epic 06 FMEA Risk Matrix — Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E06-01 | `Story 06.1` Scaling & Theme Leak | Tự động co giãn `--slide-scale >= 0.45` trên `16:9`/`16:10`/`Auto` qua `--slide-w`/`--slide-h`, giữ projection type scale `16px..56px`, không để `.live-html-frame !important` đè dark deck. | 6 | 4 | 2 | **48** | Enforced in `06.1 AC-04/AC-07` |
| EC-E06-02 | `Story 06.2` Slow `srcDoc` Reload & IME/Cursor Loss | Tách `targetCacheKey` khỏi `PATCH_BY_FF_ID` (`<16ms`), bọc `inert` nửa ẩn `translateX(-50%)`, bảo vệ `isComposing`, Unified Reverse-Patch Undo & crash-draft journal. | 9 | 7 | 7 | **441** | Enforced in `06.2 AC-03/AC-04/AC-06` |
| EC-E06-03 | `Story 06.3` Chart Validation & DoS Under CSP-B | Chỉ dùng 2D zero-CDN SVG/`<canvas>` (`3D widget` deferred), giới hạn `MAX_SERIES=12`, `MAX_POINTS=200`, `<= 1e6`, và áp `\|sum - 100\| <= 0.05` & `>= 0` riêng cho proportional/funnel charts. | 8 | 6 | 7 | **336** | Enforced in `06.3 AC-03` |
| EC-06-01 | `KU-04` Pool Eviction & `one-page` Slot Collision | Ghim Audience + Presenter active + next-slide trong `POOL_MAX_SLOTS`; cô lập `one-page` slide bằng `<section data-slide-uid="SLD-*">` & `[data-ff-id]` scoped. | 9 | 8 | 7 | **504** | Enforced in `06.2 AC-04` (`Spike S2`) |
| EC-06-02 | Partial Disk HTML vs SQLite WAL Rollback | Stage `.tmp` + `fsync` -> SQLite WAL tx (`content_sha256`+`revision`) -> atomic `rename()`, rollback tx & unlink `.tmp` khi lỗi; sweep `.tmp` khi boot. | 9 | 6 | 8 | **432** | Enforced in `06.2 AC-05` (`Spike S1`) |
| EC-06-03 | Dirty Debounce Draft & Flush Failure Loss | `beforeunload`/route guard, `sessionStorage`/`IndexedDB` recovery journal theo `(slide_uid, revision)`, banner `Toast [5.12]` retry/rollback khi `SQLITE_BUSY`/`ENOSPC`. | 9 | 7 | 7 | **441** | Enforced in `06.2 AC-06` (`Spike S2`) |
| EC-06-04 | `BroadcastChannel` Secret Leak / Hijack | Trao đổi `sessionSecret` out-of-band (`window.opener`/`MessageChannel`, cấm `?secret=`), kiểm tra Host `origin` + `deckId` + Zod, `SYNC_FULL_STATE` & Inline Split fallback. | 9 | 6 | 8 | **432** | Enforced in `06.4 AC-01/AC-03` (`Spike S5`) |
| EC-06-05 | SVG `<style>`/`style=""` mXSS & `'unsafe-inline'` | Cấm `<svg><style>`, `<foreignObject>`, `<animate*>`, `xlink:href`, `data:image/svg+xml`, validate token CSS cho cả inline `style=""` + nonce `<style>`, xóa `'unsafe-inline'`. | 9 | 6 | 7 | **378** | Enforced in `06.2 AC-05` (`Spike S1`) |
| EC-06-06 | `vp.slides` vs `APPLY_HTML` Split-Brain | `APPLY_HTML` re-parse `[data-ff-id]` về `wording_slots`; nếu phá cấu trúc slot thì khóa badge `custom_html_override` cho tới khi sync lại hoặc reset. | 8 | 8 | 6 | **384** | Enforced in `06.2 AC-04` (`Spike S2`) |
| EC-06-07 | `V12` `slide_uid` Unique Backfill & Reorder Swap | Migration `V12` tách 3 bước trong 1 tx (nullable `slide_uid` -> ULID `SLD-*` backfill -> `UNIQUE INDEX`); swap index nguyên tử & tái sinh UID khi duplicate. | 8 | 7 | 6 | **336** | Enforced in `06.1 AC-02/AC-06` (`Spike S4`) |
| EC-06-09 | Copilot TOCTOU & Stale Lease Deadlock | Pre-flush dirty CMS trước stream Copilot, lease lock trực quan `60s` TTL tự mở khi crash/abort/SSE drop, khóa read-only `SNAP-*` trên Presenter. | 8 | 8 | 5 | **320** | Enforced in `06.3 AC-04` & `06.4 AC-04` |
| EC-06-11 | `Story 03.3` / `04.3` Contract Extension Drift | Audit & mở rộng `patch_by_ff_id`/`postPatchByFfId` (`chart_json`, bullet arrays, `revision` receipt) và `KeyedParallelCoordinator` cancel/receipt. | 7 | 7 | 5 | **245** | Enforced in `06.2 AC-02` & `06.3 AC-02` |
| EC-06-12 | Pre-`PORT_ACK` Drop & Stale Closure | `channel.port1.onmessage` đọc React `ref`; hàng đợi max `32` frame coalesce theo `(slide_uid, ff_id)`, retry `500ms`×`3`, `rAF` rect throttle. | 6 | 6 | 6 | **216** | Enforced in `06.2 AC-04` (`Spike S2`) |
| EC-06-13 | Malformed `.slide.active` / Duplicate `[data-ff-id]` | Kiểm định cấu trúc `.slide.active` & tính duy nhất `data-ff-id` khi ingest; map unmapped slots sang read-only chip; giữ biểu đồ hợp lệ cũ khi Copilot JSON lỗi. | 6 | 6 | 5 | **180** | Enforced in `06.1 AC-05` & `06.3 AC-03` |
| EC-06-14 | `PresentationDualPaneView` Drift, Key Trap & P10 Telemetry | Bãi bỏ `PresentationDualPaneView` theo `DESIGN.md §9.1`, chặn phím tắt khi `isComposing`, chuyển tiếp `B`/`W`/`F` & single-`Esc`, thu telemetry P10. | 6 | 7 | 4 | **168** | Enforced in `06.1 AC-05` & `06.4 AC-02/AC-04` |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 06 (Dual-Pane 16:9 HTML Presentation & Non-Tech CMS Workbench) | Review Agent & Advisor |
| 2026-11-17 | 2.0.0 | Integrated all 26 Unknowns & Edge Case Guardian EC-06-01..14 mitigations across Stories 06.1..06.4 (Tier 3 upgrade + Presenter split) | Review Agent (Edge Case Guardian) |
