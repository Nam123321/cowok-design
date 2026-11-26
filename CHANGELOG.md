# Changelog — `cowok-design`

All notable changes to `cowok-design` (FlowForge Harness OS) are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0-rc.1] — 2026-11-26

First Release Candidate delivering **100% of the v1.0 Roadmap** across all **7 Value Streams (`VS-01`–`VS-07`)**, **14 Epics**, and **47 active Stories** (`PR #1`–`#25`), verified by Stage 3 Tier-3 Plan-Blind Reviews, Stage 4 Live Browser 3-Locale E2E, and 9/9 CI gates.

### Core Platform & Persistence (`VS-01` — Epics 01–03)
- 12-crate Rust Cargo Workspace (`domain`, `spec-registry`, `context-compiler`, `constraint-engine`, `flow-graph`, `html-forge`, `presentation-cms`, `plugin-host`, `rea-bridge`, `bot-fleet`, `mcp-gateway`, `harness-daemon`) with `ts-rs` contract parity.
- SQLite WAL **V33** (`94` tables) with 2-way spec file sync, `oxc` AST constraint linter, 143-icon Flagship Registry, and `<1ms` surgical `data-ff-id` patcher.

### Flow-First Design Workbench & Brand Studio (`VS-02` — Epics 04–05)
- Unified DualMainPanel Workbench with 2D SVG Bezier DAG Studio, Option 5 Action-Linked Storyboard, Cognitive Sketch-to-Polish, Multi-Mode Theme Studio (`light`/`dark`/`warm`/`dim`), and real GitHub PR handoff.
- Day-0 Onboarding Wizard, OS Keychain / `0600` zero-leak vault, and `/simulate-user` (8D) + FMEA Edge-Case synthesis.

### 16:9 HTML Presentation Studio, CDP Visual QC & Export (`VS-03` — Epics 06–07)
- Dual-Pane 16:9 HTML Presentation Engine + CMS Workbench (`<16ms` sync, Dual-Screen Presenter Mode, Chart & Dashboard Studio v2, Layer 1 Data Storytelling).
- Pooled 1-Chromium Rust CDP Visual QC (`lib-audit` + `slides-check`), Split Share vs Developer Handoff Export, and Built-in Native Rust OOXML `.pptx` Exporter (`Story 07.3`).

### Plugin Ecosystem, REA Forge & Decision Cascade (`VS-04` — Epics 08–09)
- `agentskills.io/v1` Plugin Runtime, standalone `design-craft` pack (`scripts/assemble-design-craft-repo.ts`), Zero-Hardcode AI Harness Spine (`ADR-009`), Two-Store `/rea` + Awesome-RE DNA Forge, and 5-Stage `DecisionCascadeClient` with `RubricShield`.

### Execution Fleet, Open MCP & 4-Channel Packaging (`VS-05` — Epics 10–12)
- `cowok` Master Orchestrator, MPSC SQLite Write Actor, disjoint `worker_file_leases`, Anti-Slop Design Tournament (`CMP-07`) with Visual Zone Picker & Cross-Variant Borrow (`Story 10.3`).
- POSIX BYOS/BYOK Sandbox (`0600` FD + `-PGID`), Bidirectional Open MCP Gateway, Single-Binary CLI (`cowok-design`), Tauri v2 Desktop Host (`.dmg`/`.msi`/`.AppImage`), Universal 5-Host Plugin Pack, and Multi-Arch Hardened OCI Image (`linux/amd64`, `linux/arm64`).

### Sealed Anti-Goodhart Benchmark & 25-Case M1–M5 Certification (`VS-06` — Epic 13)
- Out-of-band pinned SHA-256 + `O_NOFOLLOW | O_NONBLOCK` + FD `0444` Sealed Eval Runner (`422 SEALED_EVAL_TAMPER_ERROR`), Paired Ablation (14 briefs), strict SemVer 2.0.0 `design-craft` gate, 3-provider calibration, 22-probe fault injection, and 25 live `M1..M5` probes.
- Resizable `CertificationDrawer` (`420px–960px`), expandable `M1–M5` domain guide, and full 9-locale parity (`vi`, `en`, `ja`, `zh`, `ko`, `fr`, `es`, `pt`, `it`).

### Cloud Control Plane, Support Desk & Distribution Ops (`VS-07` — Epic 14)
- In-App Non-Tech Support Drawer with Zero-PII Scrubber, Cloud Ingestion API & SuperAdmin Portal MVP, Canary Rollout, Ed25519 Signed Updater, Skill Quarantine Advisory, and Offline Licensing.

### Release Engineering (`Phase 1` — `v1.0.0-rc.1`)
- Synchronized all 6 release manifests to `1.0.0-rc.1` via `scripts/release-version-sync.mjs`.
- Upgraded `.github/workflows/release.yml` to build & verify all 4 distribution channels with Story 13.1 `--tier release` certification gate, OCI & Tauri contract verifiers, SHA-256 checksums, graceful RC unsigned mode / GA strict Ed25519 signing, and standalone `design-craft` Model C bundle packaging.
