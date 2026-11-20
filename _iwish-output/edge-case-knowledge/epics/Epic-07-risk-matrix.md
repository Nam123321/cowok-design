# Epic 07 FMEA Risk Matrix — 17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E07-01 | Story 07.1 Edge Case (Baseline) | Phát hiện chính xác hiện tượng tràn khung dọc/ngang (`scrollHeight > clientHeight`) khi người dùng nhập đoạn văn bản quá dài trên slide 16:9 (`lib-audit` + `slides-check`). | 7 | 6 | 3 | **126** | Enforced in 07.1 AC-01, AC-03 |
| EC-E07-02 | Story 07.1 Auto-Fix Loop | Giới hạn vòng lặp Auto-Fix tối đa `MAX_ATTEMPTS = 3` và hiển thị so sánh trước/sau kèm 1-click `Cmd+Z` để tránh làm hỏng ý đồ thiết kế của người dùng (kế thừa từ `07.2` cũ). | 7 | 5 | 4 | **140** | Enforced in 07.1 AC-03 & Task 07.1.3 |
| EC-E07-03 | Story 07.2 Split Export | Đảm bảo file `.html` offline tự chứa (Shared `crates/html-forge`) hiển thị biểu đồ & SVG sắc nét với `0` external CDN requests khi tắt mạng (kế thừa từ `07.3` cũ, đánh dấu `07.3` `cancelled`). | 8 | 5 | 4 | **160** | Enforced in 07.2 AC-01, AC-03, AC-04 |
| EC-E07-04 | Story 07.1 Pooled 1-Chromium CDP | Pooled `<= 1` Chromium CDP leaks tabs/storage across concurrent runs or serves stale `SHA-1` `qc_gate_runs` cache when `PROJECT_CONSTRAINTS` / `fixtureRef` changes. Mitigation: Ephemeral `IncognitoBrowserContext` per run + pool semaphore + composite cache key `(html, constraints, viewport, fixture)` + honest `UNVERIFIED_NO_BROWSER` fallback. | 8 | 6 | 5 | **240** | Enforced in 07.1 AC-01, AC-03 |
| EC-E07-05 | Story 07.1 4-SubState Flow Coverage | Missing `05.5` dependency leaves E2E Flow Completeness check blind to `LOADING`/`EMPTY`/`ERROR` substates. Mitigation: Declared `"05.5"` dependency + structured missing-substate diagnostic. | 7 | 5 | 4 | **140** | Enforced in 07.1 AC-02 & §6 |
| EC-E07-06 | Story 07.2 OOM & P0 QC Bypass | Base64-inlining `>25MB` assets causes OOM, `override_qc_warning` bypassed silently, or missing PPTX plugin throws raw error. Mitigation: `<= 25MB` pre-encode bound, mandatory explicit `override_qc_warning` audit check, graceful PPTX plugin prompt without polluting core binary (`ADR-006`). | 8 | 5 | 4 | **160** | Enforced in 07.2 AC-02, AC-03 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 07 (17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter) | Review Agent & Advisor |
| 2026-11-20 | 2.0.0 | Migrated to canonical 1-10 FMEA scale (`EC-E07-01..06`), aligned consolidated `07.1` (Pooled CDP + Real RGBA SSIM + Overlay) and `07.2` (Split Export + Plugin PPTX; `07.3` cancelled) | Review Agent (`573b2485`) |
