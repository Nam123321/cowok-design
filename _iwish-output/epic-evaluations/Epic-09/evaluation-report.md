# Epic 09 Evaluation & Actionable Implementation Plan (`VS-04`)

**Verdict:** `READY` | **Envelope (`max_phase`):** `IMPLEMENT` | **Edge-Case Guardian:** `APPROVED`

## Plugin Decomposition Strategy
- **CORE_REA_AND_DECISION_CASCADE:** `crates/rea-bridge` (Two-Store `/rea`, License/ToS Firewall) và `crates/domain` + `crates/bot-fleet` (`DecisionCascadeClient`, 5-Stage Flow, `RubricShield`, `V17 decision_audit_log`).

## Actionable Implementation Plan
1. Refactor `09.1` (Two-Store + Firewall) & `09.2` (5-Stage Decision Engine + `09.3` breaker/OCC); mark `09.3` `cancelled`.
2. Đạt gates `100%` GPL/AGPL clone block, `0%` float leak, `>=90%` swap consistency, `100%` OWASP LLM01 quarantine.
