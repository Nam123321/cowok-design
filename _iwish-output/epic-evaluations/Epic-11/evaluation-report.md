# Epic 11 Evaluation & Actionable Implementation Plan (`VS-05`)

**Verdict:** `READY` | **Envelope (`max_phase`):** `IMPLEMENT` | **Edge-Case Guardian:** `APPROVED`

## Plugin Decomposition Strategy
- **HYBRID_BYOS_RUNNER_AND_OPEN_MCP_GATEWAY:** `KERNEL_CORE` (`crates/bot-fleet`, `-PGID` reaper, Post-Execution Normalizer, `crates/mcp-gateway` 4 Tiers) + `EXTENSION_PLUGIN` (CLI/BYOK provider adapters & Host MCP 1-click config adapters).

## Actionable Implementation Plan
1. Refactor `11.1` (retire stubs + Normalizer) & `11.2` (Open MCP Gateway từ `11.3`); mark `11.3` `cancelled`.
2. Đạt gates `<=20s` first visual preview, `0` orphan processes `<=2s` after cancel, `0` stub outputs, `100%` 4-tier auth enforcement.
