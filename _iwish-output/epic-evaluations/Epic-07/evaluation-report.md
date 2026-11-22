# Epic 07 Evaluation & Actionable Implementation Plan (`VS-03`)

**Verdict:** `READY` | **Envelope (`max_phase`):** `IMPLEMENT` | **Edge-Case Guardian:** `APPROVED`

## Plugin Decomposition Strategy
- **KERNEL (`Story 07.1`, `07.2`):** Pooled 1-Chromium CDP QC, `VisualQcOverlay`, 1-Click Auto-Fix, `V17 qc_gate_runs` SHA-1 cache, `crates/html-forge` 1-file offline HTML & CDP Vector PDF.
- **EXTENSION (`Story 07.2`):** Optional `.pptx` Deck Exporter plugin (`TDR-006`).

## Actionable Implementation Plan
1. Refactor `Story 07.1` (gộp `07.2 + 09.3` occlusion) và `Story 07.2` (nhận Export scope từ `07.3` cũ); mark `07.3` `cancelled`.
2. Đạt gates `<=2s` check, `<=1` Chromium process, `>=85%` post-fix pass rate, `0` external 404 offline `file://`, selectable vector PDF text.
