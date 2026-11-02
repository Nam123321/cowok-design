# Epic 01 FMEA Risk Matrix — Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **EC-E01-01** (`UU-01.1`) | Infinite `notify` watcher echo loop between `product/specs/*.md` and SQLite `PROJECT_SPECS` | Hash-Gated Watcher Suppression (`disk_sha256 == db_sha256` $\rightarrow$ `NO-OP`) + Atomic `.tmp.<uuid>` rename (`Story 01.2 AC-03`). | 7 | 2 | 2 | **28** | Enforced in Story 01.2 AC-03 & Task 01.2.3 |
| **EC-E01-02** | Concurrent Human + Bot write race (`Lost Update`) on the same spec file | Mandatory `expected_sha256` OCC check returning `409 STALE_FILE` (`Story 01.2 AC-04`, `Story 01.3 AC-01`). | 8 | 2 | 1 | **16** | Enforced in Story 01.2 AC-04 & Task 01.2.4 |
| **EC-E01-03** (`AS-01.2`) | Path traversal via crafted `spec_key` (`../../etc/passwd`) or symlink escape | Strict 9-variant `CanonicalSpecKey` enum (`Story 01.1 AC-05`) + `canonicalize()` workspace jail (`Story 01.2 AC-05`). | 9 | 2 | 1 | **18** | Enforced in Story 01.1 AC-05 & Story 01.2 AC-05 |
| **EC-E01-04** (`UU-01.2`) | Transient `0-byte` truncate event from external IDE save wiping SQLite spec content | `50ms` debounce re-read on sudden `0-byte` file change + explicit delete flag requirement (`Story 01.2 AC-05`). | 6 | 2 | 2 | **24** | Enforced in Story 01.2 AC-05 & Task 01.2.3 |
| **EC-E01-05** (`KU-01.2`) | `SQLITE_BUSY` under multi-bot parallel reads/writes & cold start latency | Mandatory `PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;` on every connection (`Story 01.2 AC-01`). | 6 | 2 | 1 | **12** | Enforced in Story 01.2 AC-01 & Task 01.2.1 |
| **EC-E01-06** (`KU-01.1`) | `ts-rs` tagged enum & 64-bit integer precision drift across Rust and TypeScript | Explicit `MAX_SAFE_INTEGER` safe number bounds + automated JSON round-trip test (`Story 01.1 AC-04`). | 6 | 2 | 2 | **24** | Enforced in Story 01.1 AC-04 & Task 01.1.4 |
| **EC-E01-07** (`AS-01.1`) | Daemon port `127.0.0.1:4170` collision on startup | Dynamic loopback port fallback + atomic `.uxui-os/daemon.lock` discovery file (`Story 01.3 AC-04`). | 5 | 2 | 2 | **20** | Enforced in Story 01.3 AC-04 & Task 01.3.4 |
| **EC-E01-08** (`BS-01.1`, `BS-01.2`) | Cross-project Brand Guideline drift & hallucinated specs from sparse Product Briefs | Track `source_project_id` + `source_revision_sha256` (`Story 01.3 AC-03`) & enforce `[CONFIRMED]`/`[INFERRED]`/`[MISSING]` tags (`Story 01.3 AC-05`). | 6 | 2 | 2 | **24** | Enforced in Story 01.3 AC-03, AC-05 & Task 01.3.2 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 01 (Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry) | Review Agent & Advisor |
| 2026-11-02 | 1.1.0 | Expanded FMEA Risk Matrix (`EC-E01-01` .. `EC-E01-08`) incorporating Party Mode & 4-Quadrant Unknowns from `/evaluate-epic` | Review Agent & Advisor |
