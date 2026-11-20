---
schema: plugin-boundary-review/v1
epic_id: Epic-06
feature_group: "VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio"
applicability: NOT_APPLICABLE
delivery_strategy: CORE_PRESENTATION_ENGINE_ONLY
---

# Plugin Boundary Review — Epic 06 (`06.1..06.4` Completed + `06.5` AI Deck Gen)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `slide_stage_cms_charts_presenter` | `06.1..06.4` | `KERNEL` | `KERNEL_CORE` | Completed baseline (PR #14 `292f2ff`): `16:9`/`16:10`/`Auto`, `lol_html` + `html5ever`, 2D Charts CSP-B, Presenter `BroadcastChannel`. |
| `ai_deck_gen_and_csv_pdf_normalizer` | `06.5` | `DOMAIN` | `KERNEL_CORE` | Retires `presentation_copilot.rs:1767-1782` echo stub; wires through `crates/bot-fleet` + 5-Stage Sandwich (`09.2`) + `stageshow` + `.csv/.tsv/.pdf -> ChartSpecV12` (`ADR-006`). |
