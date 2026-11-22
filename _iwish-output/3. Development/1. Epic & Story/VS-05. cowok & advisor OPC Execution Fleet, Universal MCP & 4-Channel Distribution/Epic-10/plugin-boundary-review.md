---
schema: plugin-boundary-review/v1
epic_id: Epic-10
feature_group: "VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution"
applicability: NOT_APPLICABLE
delivery_strategy: CORE_ORCHESTRATION_FLEET
---

# Plugin Boundary Review — Epic 10 (`10.1`, `10.2`, `10.3`)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `cowok_3lane_and_mpsc_actor` | `10.1` | `KERNEL` | `KERNEL_CORE` | Serialized MPSC SQLite Write Actor (`busy_timeout=5000`) preventing `SQLITE_BUSY` + `<50ms` `$UIUX_LIVE_SPEC` + 1-step `Cmd+Z`. |
| `parallel_worker_leases_and_jury` | `10.2` | `KERNEL` | `KERNEL_CORE` | Disjoint `V17 worker_file_leases` (`fencing_token`, `owned_paths[]`), `429` semaphore clamp `3->1`, independent `Builder != Scorer` QC + Tier-3 `Design Jury`. |
| `cmp07_tournament_and_advisor_loop` | `10.3` | `DOMAIN` | `KERNEL_CORE` | `CMP-07 DesignTournamentArena` (`Blind Wipe`, `Cross-Variant Borrow`, `/crystallize`), Pre-Judge AST Distance `<=0.78`, `advisor` 6-step loop (`V18`). |
