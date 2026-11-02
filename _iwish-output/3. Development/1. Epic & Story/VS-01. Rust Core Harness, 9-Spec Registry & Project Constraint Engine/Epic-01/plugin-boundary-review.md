---
schema: "plugin-evaluate-decision/v1.1"
decision_id: "DEC-EPIC-01-KERNEL-SUBSTRATE"
input_fingerprint: "sha256:epic-01-cargo-workspace-domain-contracts-sqlite-registry"
selected_routes:
  - skill: "plugin-vs-core-classifier"
    mode: "design"
    reason_code: "CORE_ONLY_SCOPE"
prerequisite_verdict: "NOT_APPLICABLE"
blockers: []
evidence_refs:
  - "_iwish-output/2. Product Planning/2.5. architecture.md"
  - "_iwish-output/2. Product Planning/2.2. database-spec.md"
  - "_iwish-output/2. Product Planning/ADRs/ADR-001-Hybrid-Rust-Core-Immediate-Adoption.md"
mutation_boundary:
  authorized: false
  exact_scope_ref: "Epic-01"
next_step: "Proceed with Kernel Core implementation of crates/domain, crates/spec-registry, and crates/harness-daemon."
observability:
  redacted_attempt_id: "att-epic-01-boundary-v1"
  policy_version: "2.1.0"
plugin_delivery:
  delivery: "core"
  adr_ref: "_iwish-output/2. Product Planning/ADRs/ADR-001-Hybrid-Rust-Core-Immediate-Adoption.md"
  components:
    - id: "crates/domain"
      packaging: "KERNEL_CORE"
    - id: "crates/spec-registry"
      packaging: "KERNEL_CORE"
    - id: "crates/harness-daemon"
      packaging: "KERNEL_CORE"
---

# Epic 01 — Plugin Boundary Review (`plugin-boundary-review.md`)

## 1. Classification Summary (`Axis 1: Trust & Ownership`)
- **Epic:** `Epic-01: Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry`
- **Trust Owner:** `KERNEL` (`KERNEL_CORE`)
- **Plugin Delivery Verdict:** `NOT_APPLICABLE` (`delivery: "core"`)
- **Rationale:**
  1. `crates/domain` defines the canonical Rust structs/enums and `ts-rs` TypeScript contracts for the entire Harness OS (`Project`, `ProjectSpec`, `FeatureGraph`, `ConstraintProfile`, `PresentationDeck`, `BotJob`, `GateReport`, `StaleFileError`). It cannot be hot-swapped or externalized as a third-party plugin.
  2. `crates/spec-registry` owns the 16-table embedded `rusqlite` SQLite WAL database (`PRAGMA journal_mode = WAL`) and the 2-way `notify` filesystem watcher over `product/specs/` and `product/features/` with SHA-256 `StaleFile` OCC. As the root state engine (`Stream-1`, In-Degree `0`), it is strictly `KERNEL_CORE`.
  3. `crates/harness-daemon` provides the loopback-bound (`127.0.0.1:4170`) Axum HTTP/WebSocket control plane and the 9 Foundational Specs synthesis pipeline.

## 2. Candidate Parts Matrix

| Component ID | Trust Owner | Packaging | Plugin Applicability | Rationale |
| :--- | :---: | :---: | :---: | :--- |
| `crates/domain` | `KERNEL` | `KERNEL_CORE` | `NOT_APPLICABLE` | Root type system and `ts-rs` codegen bridge shared across all 11 Rust crates and React 19 packages. |
| `crates/spec-registry` | `KERNEL` | `KERNEL_CORE` | `NOT_APPLICABLE` | Owns the 16 SQLite WAL tables (including `SKILL_PLUGINS` schema storage) and SHA-256 `StaleFile` OCC enforcement. |
| `crates/harness-daemon` | `KERNEL` | `KERNEL_CORE` | `NOT_APPLICABLE` | Loopback Axum HTTP + WebSocket daemon orchestrating the 9 Foundational Specs Vault. |

## 3. Downstream Plugin Substrate Enablement
While `Epic 01` itself is 100% `KERNEL_CORE`, `Story 01.2` provisions Table #11 (`SKILL_PLUGINS`) and Table #12 (`MODEL_TOURNAMENTS`) inside the 16-table SQLite WAL schema so that downstream **Epic 08 (`agentskills.io` Repackager & GitHub Cron Updater)** can register, version-pin, and manage external plugins without schema migrations.
