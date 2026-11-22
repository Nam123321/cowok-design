---
schema: plugin-boundary-review/v1
epic_id: Epic-11
feature_group: "VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution"
applicability: REQUIRED_REVIEW
delivery_strategy: HYBRID_BYOS_RUNNER_AND_OPEN_MCP_GATEWAY
---

# Plugin Boundary Review — Epic 11 (`11.1`, `11.2`)

| Component ID | Story | Trust Ownership | Packaging Category | Hard Veto / Evidence Notes |
| :--- | :---: | :---: | :---: | :--- |
| `posix_byos_runner_and_normalizer` | `11.1` | `KERNEL` | `KERNEL_CORE` | `crates/bot-fleet`: `0o600` FD staging + `-PGID` reaper (`0` orphans `<=2s` after cancel) + Post-Execution Normalizer (`oxc/lol_html -> reconcile_ff_ids -> TriangleLinter -> IconValidator`). |
| `byok_and_cli_provider_adapters` | `11.1` | `EXTENSION` | `EXTENSION_PLUGIN` | External CLI runners (`claude`, `codex`, `agy`, `ollama`...) & BYOK HTTP gateways isolated behind OS Keychain/`0o600` Vault. |
| `bidirectional_open_mcp_gateway` | `11.2` | `KERNEL` | `KERNEL_CORE` | `crates/mcp-gateway` (`stdio` + SSE) enforcing 4 Permission Tiers (`READ_ONLY..HUMAN_ONLY`) & deterministic AST/linter self-healing `ToolError`. |
| `host_mcp_1click_installers` | `11.2` | `EXTENSION` | `EXTENSION_PLUGIN` | Settings `MCP` 1-Click Installer adapters targeting Claude, Cursor, Codex, and Antigravity configs. |
