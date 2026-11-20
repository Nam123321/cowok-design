---
schema: agent-surface-map/v1
epic_id: Epic-04
feature_group: "VS-02. Flow-First Executable Design Workbench & SDI Engine"
default_execution_lane: LANE_1_DETERMINISTIC
---

# Epic 04 Agent Surface Map (`04.1..04.5` Baseline + `04.6`)

| Story ID | Component ID | Trust | Exposure Tier | Capability Form | Execution Lane | Single Core Port / Symbol | Persistence | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| `04.1..04.5` | `flow_workbench_baseline` | `KERNEL` | `HUMAN_ONLY` / `AGENT_FACING` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `crates/flow-graph` + `crates/spec-registry` `V7` *(completed)* | `EXISTING_SCHEMA` | `CONSISTENT` |
| `04.6` | `chat_session_sqlite_persistence` | `KERNEL` | `HUMAN_ONLY` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `GET/POST /chat-sessions/:sid/messages` -> `V4 chat_messages` | `EXISTING_SCHEMA` | `CONSISTENT` |
| `04.6` | `cas_binary_and_svg_normalizer` | `KERNEL` | `AGENT_FACING` | `TOOL` | `LANE_1_DETERMINISTIC` | `/cas/upload` + `html-sandbox-sanitizer.ts` `<svg viewBox>` | `NEW_SCHEMA` (`V16 cas_blobs`) | `CONSISTENT` |
| `04.6` | `in_chat_zero_leak_vault` | `KERNEL` | `HUMAN_ONLY` | `UI_KIT` / `TOOL` | `LANE_1_DETERMINISTIC` | `/vault/chat-ingest` + `redact_secrets` + `VaultReceiptCard` | `0o600` Vault / Keychain | `CONSISTENT` |
