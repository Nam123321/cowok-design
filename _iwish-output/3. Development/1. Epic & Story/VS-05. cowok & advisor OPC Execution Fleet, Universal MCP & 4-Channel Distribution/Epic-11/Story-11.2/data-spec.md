---
type: I-Wish Data & Contract Spec
story_id: "11.2"
title: "Data & V28 Bidirectional Open MCP Server, Tool Policies & Host Install Backup Contract: Story 11.2"
status: active
adr_ref: "../../../../2. Product Planning/ADRs/ADR-007-Hybrid-Core-and-Extension-Packaging.md"
data_decision_records:
  - model: McpToolPolicy
    action: NEW_AGGREGATE
    bounded_context: mcp-gateway
    justification: "V28 mcp_tool_policies persists per-tool enablement and enforces immutable 4-tier permission isolation (FULL_CREATE / HUMAN_ONLY permanently locked offline from ActorKind::McpClient per EC-LEARN-001)"
  - model: McpExternalActivityLog
    action: NEW_AGGREGATE
    bounded_context: mcp-gateway
    justification: "V28 mcp_external_activity_logs captures live JSON-RPC tool/resource calls, Self-Healing ToolError recoveries, and rejected HUMAN_ONLY attempts"
  - model: McpHostInstallBackup
    action: NEW_AGGREGATE
    bounded_context: mcp-gateway
    justification: "V28 mcp_host_install_backups records pre-write host config SHA-256 backups for 1-click safe rollback"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "./story.md"
  - "./ui-spec.md"
---

# Data & Contract Specification — Story 11.2 (`V28`)

## 1. Additive SQLite WAL DDL (`V28__story_11_2_mcp_external_activity.sql`)

```sql
CREATE TABLE IF NOT EXISTS mcp_tool_policies (
    tool_name TEXT PRIMARY KEY NOT NULL,
    permission_tier TEXT NOT NULL CHECK (permission_tier IN ('READ_ONLY','INSPECT','REVISE_PROPOSE','HUMAN_ONLY')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    mcp_callable INTEGER NOT NULL DEFAULT 1 CHECK (mcp_callable IN (0, 1)),
    updated_at BIGINT NOT NULL,
    CHECK (permission_tier <> 'HUMAN_ONLY' OR mcp_callable = 0)
);

CREATE TABLE IF NOT EXISTS mcp_external_activity_logs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('MCP_CLIENT','HUMAN_WORKBENCH_SESSION')),
    host_client TEXT NOT NULL,
    method_name TEXT NOT NULL,
    tool_name TEXT,
    permission_tier TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('OK','SELF_HEALING_ERROR','DENIED_HUMAN_ONLY','VALIDATION_ERROR')),
    jsonrpc_code INTEGER,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    idempotency_key TEXT,
    summary TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mcp_activity_created ON mcp_external_activity_logs(created_at DESC);

CREATE TABLE IF NOT EXISTS mcp_host_install_backups (
    id TEXT PRIMARY KEY NOT NULL,
    host_id TEXT NOT NULL,
    config_path TEXT NOT NULL,
    backup_path TEXT NOT NULL,
    before_sha256 TEXT NOT NULL,
    after_sha256 TEXT NOT NULL,
    rolled_back INTEGER NOT NULL DEFAULT 0 CHECK (rolled_back IN (0, 1)),
    created_at BIGINT NOT NULL
);
```

## 2. Boundary Zod & API Contracts (`src/lib/validations/mcp-gateway.ts`)
- `POST /api/v1/mcp/rpc`: Streamable HTTP JSON-RPC 2.0 endpoint exposing `tools/list` (hides `HUMAN_ONLY` tools from `ActorKind::McpClient`), `tools/call` (`get_app_state`, `apply_batch`, `query_predicate`, `banner_solver`, `grounded_diagram_elk`, plus `HUMAN_ONLY` unconditional `-32003` rejection with Workbench deep-link), `resources/list` & `resources/read` (`cowok-design://rules`, `cowok://skills/*`, `ui://cowok-design/brief-and-preview.html`).
- `POST /api/v1/mcp/install`: 1-Click Host Installer (`cowok_ai`, `antigravity`, `claude_code`, `codex`, `cursor`, `opencode`) with `O_NOFOLLOW`, atomic JSON/TOML AST merge, checksum backup, and rollback (`action: "install" | "rollback"`).
- `POST /api/v1/mcp/tools/toggle`: Toggle `READ_ONLY | INSPECT | REVISE_PROPOSE` tools (`HUMAN_ONLY` always locked & rejects `mcp_callable=true` with `403 HUMAN_APPROVAL_REQUIRED`).
- `GET /api/v1/mcp/external-activity`: Returns live activity audit log entries & tool policy matrix.
