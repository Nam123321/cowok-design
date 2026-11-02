# ADR-002: Architecture Decision Record — Tech Stack Decision: Day-1 Rust Core Daemon + Vite React 19 Hybrid Architecture

**Status:** ACCEPTED (Updated per Founder Directive: Day-1 Rust Execution)  
**Date:** 2026-11-02  
**Deciders:** Product Founder, AI Systems Architect, Tech Lead  

---

## 1. Context
UI/UX OS cần một hạ tầng thực thi siêu nhẹ, tốc độ phân tích AST tức thì (< 2ms), quản lý tiến trình BYOS (`codex`, `claude`, `antigravity`) chuẩn POSIX không rò rỉ zombie process, đồng thời phân phối dưới dạng một binary độc lập kiêm MCP Server cho Cowok.ai.

## 2. Decision (Khóa Kiến trúc Hybrid Triển khai Rust Ngay Từ Ngày Đầu - Day 1)
Thay vì dùng Node.js tạm thời ở giai đoạn 1, chúng tôi quyết định **triển khai trực tiếp Core Daemon, AST Engine, Spec Registry và MCP Server bằng Rust ngay từ đầu (Day 1)**, kết hợp với **Vite v8 + React 19 SPA (Vanilla CSS)** ở tầng giao diện:

1. **Core Harness Daemon & API (`crates/harness-daemon` - Rust):**
   - Framework: **Axum + Tokio** (Async Rust runtime, xử lý REST API, WebSocket và SSE streaming siêu tốc, tiêu thụ RAM nhàn rỗi chỉ ~15MB).
   - Database: **`rusqlite` / `sqlx` (SQLite WAL mode)** nhúng trực tiếp trong binary Rust, quản lý Spec Registry, lịch sử phiên bản và `presentation.json`.
2. **AST Parser, Linter & Patcher (`crates/ast-guardian` - Rust):**
   - Sử dụng **`oxc` (Oxidation Compiler) / `swc_core`** và **`tree-sitter`** gốc trên Rust để quét AST dưới 1ms.
   - Chặn đứng 100% Tailwind classes, mã màu hex cứng và icon ngoài danh mục; thực hiện Target-Aware Patching qua thuộc tính `data-ff-id`.
3. **BYOS Process Manager (`crates/byos-sandbox` - Rust):**
   - Quản lý tiến trình con bằng POSIX syscalls (`nix` / `tokio::process`), tạo process group độc lập (`setpgid`), truyền prompt qua file descriptor `0600` chống nghẽn kernel buffer 64KB, và tự động gặt (reap) toàn bộ tiến trình mồ côi khi tắt.
4. **MCP Server Gateway (`crates/mcp-server` - Rust):**
   - Triển khai chuẩn JSON-RPC 2.0 qua `stdio` và `SSE` bằng Rust, đóng gói chung vào một file thực thi tĩnh duy nhất (`uiux-os-daemon`).
5. **Frontend Workbench (`apps/web` - TypeScript / React 19):**
   - **Vite v8 + React 19 SPA**, `@xyflow/react` v12 cho State Machine Canvas, Dual-Pane Presentation CMS, **100% Pure Vanilla CSS Custom Properties** (`tokens.css`) và **145 Brand SVG Icons**.
   - Đồng bộ kiểu dữ liệu tự động từ Rust sang TypeScript thông qua **`specta` / `ts-rs`**.

## 3. Consequences
- **Hiệu năng Tuyệt đối ngay từ MVP:** Không tốn công viết lại từ Node sang Rust ở giai đoạn 2 (Zero Rewrite Debt).
- **Single Binary Distribution:** Toàn bộ backend, AST linter, SQLite và MCP server nằm gọn trong 1 binary ~15MB, khởi động < 10ms.
- **Type-Safety Xuyên Biên Giới:** Rust Structs tự động xuất ra TypeScript interfaces và JSON Schemas thông qua `ts-rs` + `schemars`.
