# Epic 04 FMEA Risk Matrix — Flow-First 2D SVG Bezier DAG Studio, State-Hydrated Preview & Element-Anchored Comments

## 1. FMEA Edge Case Registry

| Risk ID | Story | Failure Mode | Deterministic Mitigation & Verification | S | O | D | Residual RPN | Status |
| :--- | :---: | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **EC-E04-01** | `04.1` | `V3__flow_workbench_dag.sql` migration upgrade & `schema_migrations` checksum gate (`P4`) | Track applied migrations + SHA-256 in `schema_migrations` (`pool.rs`), rebuild `V1` `flow_graphs`/`flow_nodes`/`flow_edges` inside `BEGIN IMMEDIATE` preserving rows & `screen_artifacts` FKs (`IT-MIG-04`). | 9 | 2 | 2 | **36** | Mitigated in Story 04.1 Plan |
| **EC-E04-02** | `04.1` | Concurrent edge cycle race & partial `deploy-draft` batch commit (`P3`) | `BEGIN IMMEDIATE` serialization + `flow_graphs.version` CAS + 3-color DFS on post-batch graph + `flow_draft_deployments(project_id, draft_id)` idempotency (`IT-FLOW-03`). | 9 | 2 | 3 | **54** | Mitigated in Story 04.1 Plan |
| **EC-E04-03** | `04.1` | Cross-tenant/project IDOR, `api-guard.ts` implicit `admin` fallback & `<iframe sandbox>` escape/theme bleed (`P6`) | Remove default `admin` role, enforce `(tenant_id, project_id)` + composite SQLite FKs, and isolate user HTML in `<iframe sandbox="allow-scripts allow-forms" srcDoc>` with CSP + `MessageChannel` capability nonce (`SEC-FLOW-06`, `SEC-IFRAME-06`). | 10 | 2 | 2 | **40** | Mitigated in Story 04.1 Plan |
| **EC-E04-04** | `04.1` | God-file complexity (`8` FE files + `routes.rs` `3,340 LOC`), circular imports, Amber brand violations & `Shorten Chat` `250px` drift (`P8`) | Decompose all God files (`<= 250 LOC` module, `<= 50 LOC` fn, `0` cycles), enforce Cowok Minimalist Zero-Amber tokens, and collapse `Shorten Chat` to `40px` rail (`PD-02 = A`) (`UT-UI-08`). | 7 | 2 | 3 | **42** | Mitigated in Story 04.1 Plan |
| **EC-E04-05** | `04.2` | SQLite V4 multi-project concurrent write locking causing `sqlite3_busy` | Dedicated single-writer MPSC queue in Rust daemon + `spawn_blocking` with 5000ms timeout budget and exponential jitter retry (25/50/100ms). | 4 | 2 | 2 | **16** | Mitigated in Story 04.2 Plan |
| **EC-E04-06** | `04.2` | Path traversal attack via forged folder picker path (`../`) or symlink | Preload/Daemon HMAC-SHA256 signature verification (`EC-PATH-HMAC-002`); reject any unsigned folder paths before file system access. | 5 | 1 | 1 | **5** | Mitigated in Story 04.2 Plan |
| **EC-E04-07** | `04.3` | KeyedParallelCoordinator cross-node resource contamination | Enforce `COWOK_OWNED_NODES` lease token with monotonic fencing token and `stale_after_s = 120s`. | 5 | 2 | 2 | **20** | Mitigated in Story 04.3 Plan |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 04 | Review Agent & Advisor |
| 2026-11-11 | 2.1.0 | Synchronized `PA 1C` Dual-Ingress Loopback Auth + `guardedRoute({ authMode: "strict-loopback" })`, `PA 2B` Orthogonal Layout/Structural OCC + Snapshot 3-Way Entity Diff, `Option CSP-B` 2-Tier Read-Only Design CDN Allowlist, `V3` 12-Step SQLite Rebuild (`25 -> 27` tables), and `0700`/`0600` Filesystem Hardening for Story 04.1 | Edge Case Guardian & Party-Mode Council |
