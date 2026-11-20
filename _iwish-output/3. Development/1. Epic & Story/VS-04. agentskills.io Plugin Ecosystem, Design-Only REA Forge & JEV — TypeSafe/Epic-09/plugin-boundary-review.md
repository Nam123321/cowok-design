---
schema: plugin-boundary-review/v1
epic_id: Epic-09
feature_group: "VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe"
applicability: NOT_APPLICABLE
delivery_strategy: CORE_REA_AND_DECISION_CASCADE
---

# Plugin Boundary Review — Epic 09 (`09.1`, `09.2`)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `two_store_rea_and_license_firewall` | `09.1` | `KERNEL` | `KERNEL_CORE` | `crates/rea-bridge`: `Tier-1A <=250 tok` + `Tier-1B` FTS5 `.cowok-design/cache/rea-reference.sqlite3` + `License/ToS Firewall` (100% GPL/AGPL clone block, `ADR-007`). |
| `decision_cascade_and_rubric_shield` | `09.2` | `KERNEL` | `KERNEL_CORE` | ~450 LOC Rust `DecisionCascadeClient` inside `crates/domain` + `crates/bot-fleet` (no `crates/jev-engine`), `ConfidenceKind`, `egress_guard` 4KB/64KB, `<untrusted_artifact_bytes>`, `VerdictShield` 0% float leak (`ADR-006`). |
