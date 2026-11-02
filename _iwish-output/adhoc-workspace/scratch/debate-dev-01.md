# Dev Socratic Debate & Technical Feasibility Report — Epic 01: Cargo Workspace, Domain Contracts (`ts-rs`) & 16-Table SQLite Registry

- **Epic**: Epic 01 — Cargo Workspace, Domain Contracts (`ts-rs`) & 16-Table SQLite Registry
- **Stitch Screen Asset**: `projects/9168334019814804214/screens/223e92c3f6c64c72bfd928cc660710ae` (*FlowForge Harness OS — Epic 01 Control Plane*)
- **Evaluator**: `dev-agent` (Step 3 of `/epic-ux-design`)
- **Stories Covered**: `Story 01.1`, `Story 01.2`, `Story 01.3`

---

## 1. Socratic Evaluation of Stitch Native Consultation & UX Recommendations

1. **Position on 3-Screen State Architecture**:
   - **Challenge**: Stitch rendered a dense single-pane Control Plane (`223e92c3f6c64c72bfd928cc660710ae`) combining workspace crates, SQLite WAL status, and daemon telemetry. If all 3 stories are jammed into a single static view without explicit state-machine routing, developers cannot deterministically test story-isolated failure modes (`ts-rs` drift vs. `rusqlite` OCC `409 STALE_FILE` vs. `Axum` port fallback).
   - **Resolution (`ACCEPT WITH MODIFICATION`)**: Partition `epic-prototype.html` into a **3-Screen Interactive Prototype** switched via top-level `data-ff-id="nav-screen-01-1..01-3"` tabs while preserving the shared top **Daemon Status Pill (`127.0.0.1:4170`, WAL Mode, 16/16 Tables, `.uxui-os/daemon.lock`)** and right-hand **`cowok` & `advisor` (Dr. Eggbot) Consultation Drawer**.

2. **Position on Client-Side Contract & 64-Bit Integer Safety (`Story 01.1`)**:
   - **Challenge**: Rust `i64`/`u64` timestamps and row versions serialized to JSON can silently lose precision in V8 (`> Number.MAX_SAFE_INTEGER`, `9007199254740991`).
   - **Resolution (`MANDATE`)**: All `ts-rs` exported structs in `crates/domain` must use adjacently tagged serde enums `#[serde(tag = "kind", content = "payload")]` and enforce a `MAX_SAFE_INTEGER` guard (`<= 9_007_199_254_740_991`) or serialize 64-bit monotonic counters/hashes as ISO-8601 strings / hex SHA-256 strings.

---

## 2. API & State Machine Binding for the 3-Screen Interactive Prototype

### Screen 1: `Story 01.1` — Workspace & `ts-rs` Contract Inspector
- **State Machine**: `IDLE` -> `EXPORTING_BINDINGS` -> `VERIFIED_ZERO_DRIFT` | `CONTRACT_DRIFT_ERROR` | `UNSAFE_INT64_REJECTED`.
- **Domain Bindings (`crates/domain` -> `packages/shared-types/src/generated/`)**:
  - **11-Crate Cargo Topology Matrix**: `crates/domain`, `crates/spec-registry`, `crates/constraint-engine`, `crates/flow-graph`, `crates/html-forge`, `crates/presentation-cms`, `crates/plugin-host`, `crates/rea-bridge`, `crates/bot-fleet`, `crates/mcp-gateway`, `crates/harness-daemon` + `pnpm` workspace packages (`packages/shared-types`, `packages/ui-shell`).
  - **`CanonicalSpecKey` (9-Key Whitelist)**: `01-product-overview.md`, `02-target-personas-and-jtbd.md`, `03-user-flows-and-portals.md`, `04-design-direction.md`, `05-information-architecture.md`, `06-design-system.md`, `07-component-registry.md`, `08-brand-guidelines.md`, `09-data-and-copy.md`. Any non-whitelisted key triggers `HarnessError { kind: "InvalidSpecKey", payload: { key } }`.
  - **`ProvenanceStatus` Tagged Enum**: `CONFIRMED` | `INFERRED` | `MISSING` serialized via `#[serde(tag = "kind", content = "payload")]`.
  - **`HarnessError` Envelope**: Deterministic mapping to HTTP status codes (`422 INVALID_SPEC_KEY`, `403 PATH_TRAVERSAL_DENIED`, `409 STALE_FILE`, `422 BRIEF_EMPTY`, `503 SQLITE_BUSY`).

### Screen 2: `Story 01.2` — 16-Table SQLite WAL & 2-Way File Sync / 409 OCC Simulator
- **State Machine**: `SYNCED` (`disk_sha256 === db_sha256`) -> `FS_WATCHER_DEBOUNCING_50MS` -> `ATOMIC_TEMP_RENAME` (`.tmp.<uuid>` -> `sync_all()` -> `rename()`) -> `SYNCED` | `OCC_CONFLICT_409`.
- **SQLite WAL & PRAGMA Verification (`crates/spec-registry`)**:
  - Cold-start migration `< 15ms` initializing all **16 canonical tables** (`PROJECTS`, `PROJECT_SPECS`, `SPEC_REVISIONS`, `CONSTRAINT_PROFILES`, `CONSTRAINT_RULES`, `FLOW_GRAPHS`, `FLOW_NODES`, `FLOW_EDGES`, `SCREEN_ARTIFACTS`, `PRESENTATION_DECKS`, `DECK_SLIDES`, `WORDING_SLOTS`, `SKILL_PLUGINS`, `MODEL_TOURNAMENTS`, `BOT_JOBS`, `GATE_REPORTS`).
  - **4 Mandatory PRAGMAs**: `journal_mode = WAL`, `synchronous = NORMAL`, `foreign_keys = ON`, `busy_timeout = 5000`.
- **Hash-Gated Echo Suppression**: When `PUT /api/v1/projects/:id/specs/:spec_key` writes via atomic rename, the `notify` watcher computes `disk_sha256`; if `disk_sha256 == db_sha256`, the event is suppressed (`ECHO_SUPPRESSED_NOOP`) to prevent infinite write loops.

### Screen 3: `Story 01.3` — Axum Daemon Control Plane & 9-Spec Synthesis Studio
- **State Machine**: `BOOTING_4170` -> (`PORT_IN_USE` -> `FALLBACK_4171_LOCK_WRITTEN`) -> `READY` -> `SYNTHESIZING_9_SPECS` -> `PROVENANCE_GATED` | `COMPLETED`.
- **REST API Endpoints (`crates/harness-daemon` on `127.0.0.1:4170..4180`)**:
  - `GET /api/v1/health`: Returns `{ status: "ok", port: 4170, pid, wal_mode: true, tables_ready: 16, cold_start_ms: 8.4, lock_file: ".uxui-os/daemon.lock" }`.
  - `POST /api/v1/projects`: Accepts `{ name, root_path, inherit_brand_from_project_id? }`; records parent-child brand token lineage in `SPEC_REVISIONS`.
  - `GET /api/v1/projects/:id/specs`: Lists all 9 `CanonicalSpecKey` documents with `content_sha256`, `provenance_summary_json`, and `updated_at`.
  - `PUT /api/v1/projects/:id/specs/:spec_key`: Requires `{ content, expected_sha256 }`. Enforces `canonicalize()` jail + SHA-256 OCC check (`409 STALE_FILE` on mismatch).
  - `POST /api/v1/projects/:id/specs/synthesize`: Runs 9-spec synthesis; if brief word count `< 50`, injects `[INFERRED]` and `[MISSING]` provenance markers and blocks downstream compilation until resolved.

---

## 3. Edge-Case Simulation Requirements in `epic-prototype.html` (`EC-E01-01` .. `EC-E01-08`)

`epic-prototype.html` MUST provide interactive fault-injection toggles with deterministic UI feedback:

| Edge Case ID | Story | Interactive Trigger (`data-ff-id`) | Expected State Transition & UI Response |
| :--- | :--- | :--- | :--- |
| **`EC-E01-01`** | `01.2` | `btn-ec-01-echo-suppression` | Triggers daemon `PUT` atomic `.tmp.<uuid>` -> `sync_all()` -> `rename()` -> watcher logs `SKIP: disk_sha256 == db_sha256 (0 echo loops)`. |
| **`EC-E01-02`** | `01.2` | `btn-ec-02-stale-file-409` | Simulates concurrent VSCode/Execution Bot disk edit changing `disk_sha256` before browser `PUT` -> returns HTTP `409 STALE_FILE` and opens `StaleFileConflictBanner` (`Reload Disk Revision` vs `Force Overwrite`). |
| **`EC-E01-03`** | `01.1` / `01.3` | `btn-ec-03-invalid-spec-key` | Attempts write to `../secret.md` (outside 9-key `CanonicalSpecKey` whitelist) -> HTTP `422 INVALID_SPEC_KEY` toast & serde payload preview. |
| **`EC-E01-04`** | `01.2` | `btn-ec-04-zero-byte-save` | Simulates IDE atomic save (`truncate(0)` followed `18ms` later by payload write) -> `50ms` watcher debounce coalesces the transient `0-byte` event and prevents wiping SQLite spec content. |
| **`EC-E01-05`** | `01.2` | `btn-ec-05-path-traversal` | Injects symlink escape in `spec_key`/project path -> `std::fs::canonicalize()` sandbox jail rejects with HTTP `403 PATH_TRAVERSAL_DENIED`. |
| **`EC-E01-06`** | `01.1` | `btn-ec-06-unsafe-int64` | Simulates `u64` payload `9007199254740999` (`> Number.MAX_SAFE_INTEGER`) -> `ts-rs` boundary guard rejects unquoted 64-bit integer. |
| **`EC-E01-07`** | `01.3` | `btn-ec-07-port-fallback` | Simulates `127.0.0.1:4170` `EADDRINUSE` -> daemon probes `4170..4180`, binds `127.0.0.1:4171`, and atomically updates `.uxui-os/daemon.lock` with new PID + port. |
| **`EC-E01-08`** | `01.3` | `btn-ec-08-sparse-brief` | Submits a `< 50`-word brief to `POST /api/v1/projects/:id/specs/synthesize` -> flags synthesized specs with `[INFERRED]` and `[MISSING]` provenance badges and surfaces open questions in `advisor` (Dr. Eggbot). |

---

## 4. Zero-Trust CSP Enforcement & `data-ff-id` Instrumentation Contract

1. **Strict `<meta http-equiv="Content-Security-Policy">` Tag**:
   `epic-prototype.html` must embed the exact Zero-Trust policy in `<head>`:
   ```html
   <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:;">
   ```
   - Zero external JS CDNs (`unpkg`, `cdn.tailwindcss.com`, `jsdelivr`) so the prototype operates deterministically offline and passes zero-trust CSP verification.
2. **Deterministic `data-ff-id` Instrumentation**:
   - Every interactive control, screen tab, edge-case simulator button, spec row, OCC conflict modal action, and telemetry badge must carry a stable `data-ff-id="..."` attribute for automated Playwright/Webwright verification.
