# UX Socratic Debate & Consultation Evaluation — Epic 01: Cargo Workspace, Domain Contracts (`ts-rs`) & 16-Table SQLite Registry

- **Epic**: Epic 01 — Cargo Workspace, Domain Contracts (`ts-rs`) & 16-Table SQLite Registry
- **Stitch Screen Asset**: `projects/9168334019814804214/screens/223e92c3f6c64c72bfd928cc660710ae` (*FlowForge Harness OS — Epic 01 Control Plane*)
- **Design System Asset**: `assets/8dea0cc19e8c48d193f6550232507a0f`
- **Evaluator**: `ux-agent` (Step 3 of `/epic-ux-design`)
- **Governance Constraints**: `ADR-003` (Zero-Tailwind / 100% Pure Vanilla CSS Custom Properties from `tokens.css`), `2.11. component-registry.md`, `layer-epic-fe.md`

---

## 1. Stitch Native AI Consultation Assessment & Non-Tech Clarity Augmentation

### 1.1 Native AI Consultation Status
Request for consultation was sent to Native AI alongside the generation of Screen `projects/9168334019814804214/screens/223e92c3f6c64c72bfd928cc660710ae`. Stitch returned structural design notes (`outputComponents[2].text`) and 3 interactive follow-up suggestions (`Inspect Domain Contracts (ts-rs)`, `Review Axum Daemon Telemetry Logs`, `Run Full Hash Verification Suite`), which were synthesized alongside our internal UX evaluation and Socratic Debate.

### 1.2 Non-Tech Clarity Augmentation (`409 STALE_FILE` & Provenance Badges)
While Stitch established a clean 12-column control-plane hierarchy (`CMP-02 <AppShell />`, `Metric Line [5.10]`, `DataTableView [5.21]`, and `RightDrawer [5.25]`), raw infrastructure telemetry (`SHA-256` hashes, `WAL` checkpoints, `409 STALE_FILE` OCC exceptions) risks overwhelming non-technical product owners and designers. `ux-agent` mandates the following human-centric translations:

1. **Human-Readable OCC Conflict Banner (`StaleFileConflictBanner` / `Toast [5.12]`)**:
   - **Problem**: Displaying raw `HTTP 409 STALE_FILE: expected_sha256 != current_sha256` causes decision paralysis and accidental data loss.
   - **UX Augmentation**: Present a dual-layer message:
     - *Primary Human Headline*: **"External Edit Detected in IDE/Disk — Your workspace draft is out of sync with the file on disk."**
     - *Secondary Technical Proof (Collapsible/Monospace Pill)*: Side-by-side truncated SHA-256 pills (`Workspace Base: 8f3a…91c2` vs `Disk Revision: e41b…07d9`) with timestamp delta.
     - *Explicit Action Affordances*: Primary safe action **`Reload Disk Revision`** (`--human-accent: #4B7FFF`, preserves external IDE edits) vs secondary destructive outline action **`Force Overwrite Disk`** (`--status-danger`, requires confirmation tooltip).
2. **Semantic Provenance Badges (`SpecProvenanceBadge`: `CONFIRMED` / `INFERRED` / `MISSING`)**:
   - **Problem**: Users confuse AI-inferred sections (`[INFERRED]`) with human-verified ground truth (`[CONFIRMED]`) or overlook blockers (`[MISSING]`).
   - **UX Augmentation**:
     - `CONFIRMED`: Solid/subtle emerald pill + checkmark SVG (`"Verified in Brief"`).
     - `INFERRED`: Warm amber/ochre pill (`--agent-accent: #E89A35`) + spark/agent SVG (`"AI Synthesized — Needs Review"`).
     - `MISSING`: High-contrast coral/crimson pill + alert triangle SVG (`"Action Required — Ask Dr. Eggbot"`), clicking which auto-focuses the corresponding open question in `CMP-13 <CowokOrchestratorBar />` / `RightDrawer [5.25]`.

---

## 2. Critique of Stitch Raw Output (`ADR-003` Compliance & Theme Architecture)

| Dimension | Stitch Raw HTML Export Critique | Mandatory UI/UX OS Remediation (`epic-prototype.html` & Production) |
| :--- | :--- | :--- |
| **Styling Engine (`ADR-003`)** | Stitch exports `<script src="https://cdn.tailwindcss.com">` and utility classes (`bg-slate-50`, `text-blue-600`, `border-gray-200`). **REJECTED.** | Strip 100% of Tailwind CDN scripts and utility classes. Enforce semantic BEM/module classes bound strictly to `Brand-guideline/cowok-brand-product-system-v4.3/foundations/css/tokens.css` (`--bg-canvas: #F7F5F0`, `--text-primary: #081E3A`, `--human-accent: #4B7FFF`, `--agent-accent: #E89A35`). |
| **Multi-Theme Parity** | Hardcoded light-mode hex values prevent runtime switching across studio lighting conditions. | Bind root `data-theme="warm|dim|night|night-low"` switcher inside `CMP-02 <AppShell />` header so all surfaces, borders, and status tokens cascade zero-latency via CSS Custom Properties. |
| **Iconography & Craft** | Uses external icon fonts (Material Symbols) which violate offline/self-contained Tauri/local-first constraints. | Replace all icon fonts with inline `<svg>` elements enforcing `stroke-width="1.85"`, `stroke-linecap="round"`, `stroke-linejoin="round"`, and `currentColor` inheritance. |
| **DOM Instrumentation** | Interactive nodes lack deterministic test/harness hooks. | Attach mandatory `data-ff-id="..."` attributes on every interactive control, KPI metric, table row, badge, and drawer action to support Playwright E2E and Harness OS inspection. |

---

## 3. UX Resolution Across Epic 01 Stories

### 3.1 Story 01.1 — Cargo Workspace & Domain Contract (`ts-rs`) Inspector
- **Registered Components**: `CMP-02 <AppShell />`, `Metric Line [5.10]` (KPI Card #2: *`ts-rs` Type Parity: 100% / 16 Core Structs*), Contract Inspector Tab inside `DataTableView [5.21]`.
- **UX Resolution**:
  - Provide a split-preview drawer/modal showing the canonical Rust struct (`crates/domain/src/lib.rs`) alongside its generated TypeScript binding (`packages/shared-types/src/generated/index.ts`).
  - Include a `data-ff-id="btn-verify-ts-rs-parity"` action that visually confirms zero schema drift across all 16 domain models (`Project`, `ProjectSpec`, `ConstraintProfile`, `FlowGraph`, etc.) without requiring the user to open a terminal.

### 3.2 Story 01.2 — 16-Table SQLite WAL Registry, `409 STALE_FILE` OCC & `0-byte` IDE Save Debounce
- **Registered Components**: `Metric Line [5.10]` (KPI Card #3: *16-Table SQLite WAL Sync*), `StaleFileConflictBanner` / `Toast [5.12]`, `CMP-04 <SpecTreeExplorer />`, `DataTableView [5.21]`.
- **UX Resolution**:
  1. **16-Table SQLite WAL Status**: Surface live WAL health (`PRAGMA journal_mode=WAL`, `foreign_keys=ON`, active table count `16/16`) in KPI Card #3 (`data-ff-id="kpi-sqlite-wal-status"`) with a subtle pulse indicator when file-watcher sync completes.
  2. **`0-byte` Atomic Save / IDE Truncation Debounce Indicator**: Modern IDEs (VS Code, Cursor, Neovim) often truncate a file to `0 bytes` for `10–50ms` during atomic save. Instead of flashing a false-positive error or wiping the preview, display a calm inline micro-state badge (`data-ff-id="badge-fs-debounce-state"`): **`"Waiting for IDE atomic write (50ms debounce)..."`** in `--agent-accent` before committing the SHA-256 hash update to SQLite.
  3. **Optimistic Concurrency Control (`409 STALE_FILE`)**: Render `StaleFileConflictBanner` (`data-ff-id="banner-stale-file-conflict"`) at the top of the active spec workspace whenever `expected_sha256` diverges from disk, offering one-click **`Reload Disk Revision`** (`data-ff-id="btn-conflict-reload-disk"`) and **`Force Overwrite`** (`data-ff-id="btn-conflict-force-overwrite"`).

### 3.3 Story 01.3 — 9-Spec Synthesis Wizard, Provenance Badges & Cross-Project Brand Inheritance
- **Registered Components**: `CMP-04 <SpecTreeExplorer />`, `DataTableView [5.21]`, `SpecProvenanceBadge`, `CMP-13 <CowokOrchestratorBar />` / `RightDrawer [5.25]`, `Metric Line [5.10]` (KPI Card #1: *9 Foundational Specs*, KPI Card #4: *Cross-Project Brand Lineage*).
- **UX Resolution**:
  1. **9-Spec Registry Matrix (`DataTableView [5.21]`)**: List all 9 foundational markdown specifications (`01-product-overview.md` through `09-data-and-copy.md`) with real-time SHA-256 checksums, section completion counters, and clickable `SpecProvenanceBadge` pills (`CONFIRMED`, `INFERRED`, `MISSING`).
  2. **Dr. Eggbot (`advisor`) Clarification Loop (`RightDrawer [5.25]`)**: When any of the 9 specs contains `[MISSING]` sections, `CMP-13 <CowokOrchestratorBar />` highlights the exact open clarification questions from `advisor` (Dr. Eggbot). Answering a question inline immediately upgrades the section badge from `MISSING` -> `CONFIRMED` and updates the SQLite SHA-256 ledger.
  3. **Cross-Project Brand Inheritance (`inherit_brand_from_project_id`)**: Inside the Synthesis Wizard & `RightDrawer [5.25]`, provide a dedicated Brand Lineage Selector (`data-ff-id="select-inherit-brand-project"`). Selecting an upstream project (e.g., *Cowok Brand System v4.3*) previews inherited `tokens.css` swatches (`#F7F5F0`, `#081E3A`, `#4B7FFF`, `#E89A35`) and links the read-only foreign project brand reference without duplicating or mutating upstream asset files.
