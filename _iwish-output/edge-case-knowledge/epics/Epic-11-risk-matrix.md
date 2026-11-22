# Epic 11 FMEA Risk Matrix — POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E11-01 | Story 11.1 `0o600` FD & Zombie Child Cleanup | Dọn sạch 100% tiến trình con (`-PGID` reaper / Windows Job Object) và xóa ngay file tạm `0o600` qua RAII Drop guard khi hoàn tất hoặc hủy giữa chừng. | 9 | 5 | 3 | **135** | Enforced in 11.1 AC-01, AC-03 |
| EC-E11-02 | Story 08.5 Ephemeral Tool Jail (`11.2` cũ absorbed) | Toàn bộ phạm vi Ephemeral Tool Forging & Sandbox Jailing đã chuyển về `Story 08.5` (`EC-E08-11`, `env_clear()`, `-PGID`, `RLIMIT`, `O_NOFOLLOW`, `HUMAN_ONLY`). | 9 | 4 | 4 | **144** | Enforced in 08.5 AC-02 & AC-05 |
| EC-E11-03 | Story 11.2 Host Config Corruption (`11.3` cũ absorbed) | Khi `POST /api/v1/mcp/install` ghi cấu hình vào host ngoài (`Claude/Codex/Cursor/Antigravity`), tạo checksummed backup + `O_NOFOLLOW` + atomic JSON/TOML AST merge để rollback 1-click (`11.3` marked `cancelled`). | 8 | 5 | 4 | **160** | Enforced in 11.2 AC-03, AC-04 |
| EC-E11-04 | Story 11.1 `ps aux` Secret Leak & `$COWOK_SESSION_DIR` Symlink Trap | API keys/prompts leak via `/proc/<pid>/cmdline` (`ps aux`), child processes survive cancel/Windows exit, or `$COWOK_SESSION_DIR` follows symlink into `~/.claude`. Mitigation: `0o600` FD staging, `setpgid`/`-PGID` + Windows Job Object parity, `O_NOFOLLOW` `$COWOK_SESSION_DIR`, `429` cooldown rotation. | 10 | 5 | 4 | **200** | Enforced in 11.1 AC-01, AC-03, AC-04 |
| EC-E11-05 | Story 11.2 `EC-LEARN-001` `HUMAN_ONLY` MCP Block, Batch Rollback & `elk.js` DoS | External `McpClient` invokes `HUMAN_ONLY` / Ephemeral Tool Jail bypassing `08.4 EC-LEARN-001`, partial `apply_batch` failure leaks half-written state, or `elk.js` OOMs/requires missing Node. Mitigation: Unconditional `ActorKind::McpClient` `-32003` / `403 HUMAN_APPROVAL_REQUIRED` block + Workbench deep-link, transactional `1..k` rollback, embedded WASM `elk.js` + `MAX_NODES` DoS cap. | 10 | 5 | 5 | **250** | Enforced in 11.2 AC-01..AC-03 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 11 (POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server) | Review Agent & Advisor |
| 2026-11-20 | 2.0.0 | Migrated to 1-10 FMEA scale (`EC-E11-01..05`), harmonized unconditional `EC-LEARN-001` `McpClient` `HUMAN_ONLY` block (`11.2`), `11.3` cancelled, Tool Jail moved to `08.5` | Review Agent (`573b2485`) |
