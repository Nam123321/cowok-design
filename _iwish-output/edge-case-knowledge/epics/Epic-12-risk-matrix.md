# Epic 12 FMEA Risk Matrix — 4-Channel Packaging & Distribution Architecture

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E12-01 | Story 12.1 Zero Port Collision & `SIGINT` Reaper | Cấp phát cổng mạng qua bound `TcpListener` fd (0 TOCTOU race) và thu hồi toàn bộ tiến trình con khi nhận `SIGINT`. | 7 | 5 | 3 | **105** | Enforced in 12.1 AC-03 |
| EC-E12-02 | Story 12.2 Cowok.ai Drift Repair | Tự động phát hiện cấu hình MCP cũ và khôi phục an toàn không ghi đè khóa bí mật khi nâng cấp plugin trên Cowok.ai. | 8 | 5 | 4 | **160** | Enforced in 12.2 AC-03 |
| EC-E12-03 | Story 12.3 Container OOM & Non-Root | Chạy container dưới user non-root `uid 10001` kèm `--max-memory-mb` và fallback `--disable-dev-shm-usage` cho Pooled Chromium. | 8 | 5 | 4 | **160** | Enforced in 12.3 AC-03 |
| EC-E12-04 | Story 12.1 Standalone `/api/*` Parity & Read-Only FS | Standalone `uiux-os up` SPA hits 404 on `/api/*` without Vite (`TDR-002 Phase 2`), or fails under read-only `/opt/homebrew`. Mitigation: Mount Axum `/api/* -> /api/v1/*` alias directly, pass bound `TcpListener` fd, route cache/db to XDG dir. | 9 | 7 | 3 | **189** | Enforced in 12.1 AC-01, AC-03 |
| EC-E12-05 | Story 12.2 Tauri HMAC Preload & Updater Extraction (`Risk Tier 3`) | Untrusted WebView forges raw FS path, updater executes tampered native binary, or extraction follows symlink/hardlink (`Risk Tier` mismatch). Mitigation: Rust-main HMAC capability token, SHA-256 updater harness, `O_NOFOLLOW` + `nlink == 1`, preserve `HUMAN_ONLY`, aligned `risk_tier: 3`. | 10 | 4 | 5 | **200** | Enforced in 12.2 AC-02..AC-04 |
| EC-E12-06 | Story 12.3 Host Bind-Mount UID & Circular `12.3 <-> 13.1` Dep | Linux Docker host bind-mount UID mismatch breaks SQLite WAL locking, or `consumes_apis: /api/v1/eval/run` creates circular dependency with `Epic 13`. Mitigation: UID/staging WAL handling + `SIGTERM` reaper, removed `13.1` reverse API dep from `12.3` (`13.1` consumes `12.3`). | 8 | 6 | 4 | **192** | Enforced in 12.3 AC-02, AC-03 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 12 (4-Channel Packaging & Distribution Architecture) | Review Agent & Advisor |
| 2026-11-20 | 2.0.0 | Migrated to 1-10 FMEA scale (`EC-E12-01..06`), added Axum `/api/*` parity (`12.1`), `risk_tier: 3` + `O_NOFOLLOW` (`12.2`), and broke circular `12.3 <-> 13.1` dependency | Review Agent (`573b2485`) |
