---
type: I-Wish Implementation Plan
story_id: "11.2"
epic_id: "11"
risk_tier: 3
migration_slot: "V28__story_11_2_mcp_external_activity.sql"
parallel_wave: "Wave-C"
status: pending_approval
target_files:
  - crates/spec-registry/migrations/V28__story_11_2_mcp_external_activity.sql
  - crates/mcp-gateway/Cargo.toml
  - crates/mcp-gateway/src/lib.rs
  - crates/mcp-gateway/src/protocol.rs
  - crates/mcp-gateway/src/solvers.rs
  - crates/mcp-gateway/src/host_installer.rs
  - crates/mcp-gateway/tests/story_11_2_mcp_gateway.rs
  - crates/harness-daemon/src/routes/mcp_gateway.rs
  - crates/harness-daemon/tests/story_11_2_mcp_gateway.rs
  - src/lib/validations/mcp-gateway.ts
  - src/api/mcp-gateway.ts
  - src/app/api/v1/mcp/install/route.ts
  - src/app/api/v1/mcp/external-activity/route.ts
  - src/app/api/v1/mcp/tools/toggle/route.ts
  - src/app/api/v1/mcp/rpc/route.ts
  - src/features/mcp-gateway/HostInstallerList.tsx
  - src/features/mcp-gateway/McpToolPermissionTable.tsx
  - src/features/mcp-gateway/ExternalActivityTable.tsx
  - src/features/mcp-gateway/useMcpGatewayController.ts
  - src/features/mcp-gateway/mcp-gateway.test.tsx
  - src/pages/workbench/components/SettingsMcpAutomationsPane.tsx
  - tests/e2e/story-11.2.spec.ts
coordinator_shared_wiring:
  - crates/spec-registry/src/pool.rs
  - crates/harness-daemon/Cargo.toml
  - crates/harness-daemon/src/routes/mod.rs
  - crates/harness-daemon/src/routes/legacy_router.rs
  - prisma/schema.prisma
  - prisma/schema.contract.test.ts
  - src/lib/vite-bff-routes.ts
  - src/i18n/locales/*.json
---

# Implementation Plan — Story 11.2 (Compiler-Grade Bidirectional Open MCP Server, Run-Scoped Tool Bundles & 1-Click Host Installer UI)

## 1. Architectural Strategy
1. **Additive SQLite WAL Migration (`V28`):** Create `mcp_tool_policies` (with DB-enforced `CHECK (permission_tier <> 'HUMAN_ONLY' OR mcp_callable = 0)`), `mcp_external_activity_logs`, and `mcp_host_install_backups`. Mirror all 3 into `prisma/schema.prisma`.
2. **Rust Bidirectional MCP Engine (`crates/mcp-gateway`):**
   - `protocol.rs`: JSON-RPC 2.0 dispatcher supporting both `stdio` framing and Streamable HTTP/SSE. Implements 4-tier permissions (`READ_ONLY`, `INSPECT`, `REVISE_PROPOSE`, `HUMAN_ONLY` hidden & unconditionally rejected for `ActorKind::McpClient` with `-32003` + Workbench deep-link per `EC-LEARN-001`).
   - Core compiler-grade tools: `get_app_state` (`<500ms`), `apply_batch` (`@alias` resolution, `dry_run`, `idempotency_key`, `diff_image` summary, atomic rollback of steps `1..k` on step `k` failure), predicate query `has-problem('text-truncated')`, and `Self-Healing ToolError` (structured `recovery_hint` + `valid_examples`).
   - Dynamic resources: `cowok-design://rules`, `cowok://skills/*` (backed by `UnifiedSkillCatalog` from `08.7`), `ui://cowok-design/brief-and-preview.html`.
   - `solvers.rs`: Run-Scoped Outbound MCP Tool Bundles (`resolve_run_scoped_bundle`) + deterministic embedded Rust solvers: `Banner Solver G1–G15` (15-grid constraint solver) and `Grounded Diagram` Sugiyama/ELK-compatible layered layout solver (`MAX_NODES = 256` DoS cap, zero external Node runtime required).
   - `host_installer.rs`: Safe 1-Click Host Config Installer (`cowok_ai`, `antigravity`, `claude_code`, `codex`, `cursor`, `opencode`) enforcing `O_NOFOLLOW`, sibling-preserving JSON/TOML AST merge, SHA-256 pre-write backup, and 1-click rollback.
3. **Daemon & BFF Routes (`crates/harness-daemon/src/routes/mcp_gateway.rs` + `src/app/api/v1/mcp/*`):**
   - Wire `POST /api/v1/mcp/rpc`, `POST /api/v1/mcp/install`, `POST /api/v1/mcp/tools/toggle`, `GET /api/v1/mcp/external-activity` with strict BFF validation (`parseJsonBodyStrict`, `rawBinary: true`, fail-closed role).
4. **Frontend UI (`src/features/mcp-gateway/` + `SettingsMcpAutomationsPane.tsx`):**
   - Extend `SettingsMcpAutomationsPane.tsx` with `HostInstallerList` (`[data-ff-id='mcp-host-installer-list']`), `McpToolPermissionTable` (`[data-ff-id='mcp-tool-permission-matrix']` showing locked `HUMAN_ONLY` rows), and `ExternalActivityTable` (`[data-ff-id='mcp-external-activity-table']`).

## 2. Verification & TDD Gates
- Rust: `cargo test -p mcp-gateway --test story_11_2_mcp_gateway` & `cargo test -p harness-daemon --test story_11_2_mcp_gateway`
- Frontend: `npx vitest run src/features/mcp-gateway/mcp-gateway.test.tsx`
- Stage 3: Dual-Engine Plan-Blind Adversarial Review (`risk_tier: 3`)
- Stage 4: Live Browser Playwright E2E `tests/e2e/story-11.2.spec.ts`
