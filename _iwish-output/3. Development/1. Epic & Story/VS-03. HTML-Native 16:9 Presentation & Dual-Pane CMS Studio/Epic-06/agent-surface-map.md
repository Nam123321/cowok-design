---
schema: agent-surface-map/v1
epic_id: Epic-06
feature_group: "VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 06 Agent Surface Map (`06.1..06.4` Completed + `06.5`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `06.1..06.4` | `presentation_cms_and_presenter` | `KERNEL` | `HUMAN_ONLY` / `AGENT_FACING` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `crates/presentation-cms` `V15` *(completed PR #14)* | `EXISTING_SCHEMA` | `CONSISTENT` |
| `06.5` | `ai_multi_slide_deck_generator` | `DOMAIN` | `AGENT_PROPOSE_ONLY` | `UI_KIT` / `PLAYBOOK` | `LANE_2_OPERATIONAL_NLP` | `/api/v1/decks/*`, `/api/v1/slides/copilot/*` -> `crates/bot-fleet` + `stageshow-slide-craft` | `EXISTING_SCHEMA` (`V15 presentation_decks, deck_slides`) | `CONSISTENT` |
| `06.5` | `csv_pdf_to_chartspec_v12` | `KERNEL` | `AGENT_FACING` | `TOOL` | `LANE_1_DETERMINISTIC` | `.csv/.tsv/.pdf -> ChartSpecV12` normalizer (`100 ± 0.05` proportional sum + raw labels) | `EXISTING_SCHEMA` (`wording_slots`) | `CONSISTENT` |
