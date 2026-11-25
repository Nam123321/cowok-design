---
type: I-Wish Retrospective
epic_id: "Epic-11"
title: "Epic 11 Retrospective — POSIX BYOS/BYOK Runner (0o600 FD + -PGID), Cross-CLI Normalizer & Universal Open MCP Gateway (11.1–11.2)"
status: completed
timestamp: "2026-11-25T10:22:00+07:00"
---

# Epic 11 Retrospective (`11.1` – `11.2`; `11.3` cancelled)

## 1. Executive Summary & Gate Verification
- **Stories Completed (`2/2` active, `1` superseded):** `11.1` (`completed`), `11.2` (`completed`), `11.3` (`cancelled`, superseded by `11.2` + `08.5` per `ADR-007`).
- **Step 0 (`update-pending-retro-fixes.py --check`):** **`PASS`** (`No pending risks.`, exit `0`).
- **Step 5a (`evaluate-epic-drift.py --epic 11`):** **`PASS`** (`Architecture Coherence Check (Compiled): PASS` — `7` ADRs checked, `3` stories scanned, `0` CRITICAL/HIGH conflicts, `0` MEDIUM warnings).
- **Step 5b (`Unknowns Analyst` `4d04e311` + Anti-Bias Gate):** `has_code_errors: false`, `has_scope_gaps: false`, `pending_fixes: []`.
- **Step 5d (`ecc-gate.py --epic 11`):** **`PASS` (`exit 0`)** — `Story 11.1`, `Story 11.2`, and `Epic 11` validated cleanly under iWish v4.1 deterministic hygiene (`require_sig_receipts: false`); `Story 11.3` skipped (`cancelled`).
- **Verification Proof (`Story 11.2`):** Stage 3 Plan-Blind Round 3 **`VERDICT: CLEAN`** (closed 8 Round 1 + 1 Round 2 UTF-8 parser boundary defect), `mcp-gateway` **12/12 PASS**, `harness-daemon` **6/6 PASS**, Vitest **6/6 PASS**, Prisma mirror **12/12 PASS**, 9-locale tests **3/3 PASS**, Clean Code Guardian **`APPROVED` (`0` findings)**, Stage 4 Live Browser Playwright E2E on `/plugins` Chip-Grid + compact `Settings` popup **PASS (`0` console/page/5xx errors)** across Desktop Light/Dark & Mobile.

## 2. What Went Well
1. **Defense-in-Depth `EC-LEARN-001` (`HUMAN_ONLY`) Enforcement:** Layered enforcement across SQLite `V28` `CHECK` constraints (`permission_tier <> 'HUMAN_ONLY' OR mcp_callable = 0`), unconditional Rust `ActorKind::McpClient` `-32003 HUMAN_APPROVAL_REQUIRED` rejection (zero approval-token bypass), and a single localized UI lock chip across all 9 languages.
2. **Hardened 1-Click External Host Installer:** Atomic JSON + structural UTF-8-safe TOML AST merging (`TomlStructuralParser`) preserving sibling servers and inline comments, `O_NOFOLLOW` symlink rejection, advisory lockfile, checksummed backups, and 1-click unlink/restore rollback across `Cowok.ai`, `Antigravity`, `Claude Code`, `Codex`, `Cursor`, and `OpenCode`.
3. **Production POSIX BYOS/BYOK Execution Kernel (`11.1`):** Replaced all stubs with `crates/bot-fleet` (`0o600` FD staging + `O_CLOEXEC` + RAII zeroize/unlink, `setpgid`/`-PGID` reaper, DNS-pinned SSRF-protected BYOK gateway with `429` cooldown rotation, and CLI-agnostic post-execution normalizer).
4. **UX & Product-Language Alignment (`/plugins` Chip-Grid):** Moved the heavy MCP Permission Matrix (redesigned as a 3-column Chip-Grid matching `PluginRegistryPanel`) and spacious Live Activity Audit Table onto full-page `/plugins`, kept `Settings` popup compact with a quick-connect summary, standardized uniform host card heights, and purged internal dev jargon (`Story 08.x`, `EC-LEARN-001`, `V20`, `HUMAN_ONLY` English leaks) across all 9 locales.

## 3. Recurring Defect Trends & Carry-Forward Items
1. **UTF-8 Char-Boundary Slicing in Custom Rust Parsers:** Byte-indexed string parsers (`self.src[self.pos..]`) panic on multi-byte UTF-8 unless comparisons operate on `&[u8]` (`self.bytes[self.pos..]`) or check `is_ascii()` before fixed-index slicing.
2. **Internal Dev Jargon Leaking into End-User UI:** Backend descriptions and legacy components (`V20`, `Story 08.x`, `HUMAN_ONLY`, `Deterministic Engineering`) can surface directly in UI unless every user-facing label is routed through 9-language i18n keys.
3. **Cross-OS Parity Carry-Forward for Epic 12 (`Story 12.2`):** Monitor Windows Job Object process-group cleanup parity alongside POSIX `-PGID` when packaging Tauri v2 `.msi` installers in Epic 12.

## 4. Durable Prevention Rules
- **`R-RUST-UTF8-BYTE-SLICE-11`:** In custom Rust text/TOML/frame parsers, compare prefixes against `bytes[pos..]` (`&[u8]`) rather than `src[pos..]` and guard fixed byte-index slices with `token.is_ascii()` + explicit multi-byte UTF-8 regression tests.
- **`R-UI-ZERO-INTERNAL-JARGON-11`:** Never render Story IDs, ADR numbers, DB table/version names (`V20`, `SQLite WAL`), or raw enum codes (`HUMAN_ONLY`, `UNVERIFIED_NO_BROWSER`) in end-user UI; route all user-facing titles, badges, and tool descriptions through all 9 locale files.
