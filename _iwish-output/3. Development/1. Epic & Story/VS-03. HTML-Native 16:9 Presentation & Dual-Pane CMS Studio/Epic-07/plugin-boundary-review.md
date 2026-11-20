---
schema: plugin-boundary-review/v1
epic_id: Epic-07
feature_group: "VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio"
applicability: CANDIDATE
delivery_strategy: CORE_CDP_QC_AND_PLUGIN_PPTX_EXPORTER
---

# Plugin Boundary Review — Epic 07 (`07.1`, `07.2`)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `pooled_chromium_visual_qc_and_autofix` | `07.1` | `KERNEL` | `KERNEL_CORE` | Pooled `<=1` Chromium Rust CDP (`lib-audit`, `slides-check`, `elementsFromPoint`, RGBA SSIM) + `iframe_bridge_fallback` (`UNVERIFIED_NO_BROWSER`). |
| `offline_html_and_vector_pdf_exporter` | `07.2` | `KERNEL` | `KERNEL_CORE` | Self-contained 1-file offline `.html` (`crates/html-forge`) + CDP `Page.printToPDF` Vector `1920x1080` `.pdf`. |
| `pptx_deck_exporter_adapter` | `07.2` | `EXTENSION` | `EXTENSION_PLUGIN` | Isolated per `TDR-006`; optional plugin export keeping core binary lightweight. |
