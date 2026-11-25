---
type: I-Wish Review Report
story_id: "11.2"
epic_id: "Epic-11"
verdict: CLEAN
review_rounds: 3
reviewer: "Stage 3 Plan-Blind Adversarial Reviewer (e64553ac)"
timestamp: "2026-11-25T00:24:00+07:00"
---

# Stage 3 Review Report — Story 11.2: Compiler-Grade Bidirectional Open MCP Server (4 Auth Tiers) & 1-Click Host Installer UI

- **Final Verdict:** **`VERDICT: CLEAN`** (Round 3 Delta-Only Verification — 0 CRITICAL / 0 HIGH / 0 MEDIUM findings remaining; all 8 Round 1 defects + 1 Round 2 UTF-8 parser boundary fix closed)
- **Risk Tier:** `3`

## 1. Verified Implementation Surface
- **Rust Core (`crates/mcp-gateway`):**
  - `src/lib.rs`, `src/rpc_protocol.rs`, `src/tool_registry.rs`, `src/activity_log.rs`: Compiler-grade JSON-RPC 2.0 MCP Server (`stdio` + `SSE` framing, multi-header/UTF-8 safe `parse_stdio_frame`), 4-tier permission matrix (`READ_ONLY`, `INSPECT`, `MUTATE`, `HUMAN_ONLY`), unconditional `-32003 HUMAN_APPROVAL_REQUIRED` block for `ActorKind::McpClient` (`EC-LEARN-001` — no approval-token bypass), distinct `-32004 TOOL_DISABLED` for disabled non-`HUMAN_ONLY` tools, self-healing `ToolError` diagnostics, atomic `apply_batch` rollback + empty-idempotency-key protection, and `elk.js` bounded diagram DoS cap.
  - `src/host_installer.rs`: Atomic JSON & structural TOML (`TomlStructuralParser`, UTF-8 byte-slice safe) merge for `Claude Code`, `Codex CLI`, `Cursor`, `Antigravity` with `O_NOFOLLOW` symlink rejection, advisory lockfile, checksummed backup, and 1-click rollback (including unlink rollback for newly created host configs).
- **Persistence (`V28` SQLite WAL + ADR-091 Prisma Mirror):**
  - `crates/spec-registry/migrations/V28__story_11_2_mcp_external_activity.sql` (`mcp_tool_policies`, `mcp_external_activity_logs`, `mcp_host_install_backups`) mirrored 1:1 in `prisma/schema.prisma`.
- **Daemon, BFF & Frontend:**
  - `crates/harness-daemon/src/routes/mcp_gateway.rs` + `src/app/api/v1/mcp/{install,external-activity,tools/toggle,rpc}/route.ts` with Zod runtime validation (`src/lib/validations/mcp-gateway.ts`).
  - `src/features/mcp-gateway/` mounted in `SettingsModal` MCP Tab (`mcp-host-installer-list`, `mcp-tool-permission-matrix`, `mcp-external-activity-table`) with locked `HUMAN_ONLY` badges across all 9 locales.

## 2. FMEA Edge Cases Enforced
- **`EC-E11-03` (Host Config Corruption):** `O_NOFOLLOW`, advisory locking, structural TOML/JSON AST merge preserving sibling servers and inline comments, checksummed backups, and 1-click rollback.
- **`EC-E11-05` (`EC-LEARN-001` `HUMAN_ONLY` Block, Batch Rollback & DoS Cap):** Unconditional `-32003` rejection for `McpClient`, transactional batch rollback, and bounded diagram node cap.

## 3. Verification Proof
- `cargo fmt --check && cargo clippy -p mcp-gateway --all-targets -- -D warnings` — **0 warnings**
- `cargo test -p mcp-gateway --test story_11_2_mcp_gateway` — **12/12 PASS**
- `cargo test -p harness-daemon --test story_11_2_mcp_gateway` — **6/6 PASS**
- Vitest (`mcp-gateway` 6/6, `prisma/schema.contract` 12/12, `locales.test.ts` 3/3) — **PASS**
- Clean Code Guardian (`--enforce`) — **`APPROVED` (0 unratcheted errors)**
- Stage 4 Live Browser Playwright E2E (`tests/e2e/story-11.2.spec.ts`) — **PASS (`27.5s`, `0` console/page/5xx errors)** + Desktop Light, Desktop Dark & Mobile screenshots visually verified.
