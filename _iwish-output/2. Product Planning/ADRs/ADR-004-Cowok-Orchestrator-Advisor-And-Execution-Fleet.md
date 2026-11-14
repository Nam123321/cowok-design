# ADR-004: Architecture Decision Record — `cowok` Master Orchestrator, `advisor` Meta-Agent, OPC Execution Fleet & `agentskills.io` / `JEV — TypeSafe` Integration

**Status:** ACCEPTED  
**Date:** 2026-11-02  
**Deciders:** System Architect, AI Engineering Lead, Cowok.ai Platform Team  

## 1. Context
Hệ sinh thái AI Agent đang phát triển mạnh với sự xuất hiện của nhiều Execution Agents chuyên biệt (`Claude Code`, `Codex`, `Antigravity`, `Grok Bot`, `Muse`, `Duet`, `Dot`...) cùng kho kỹ năng mã nguồn mở phong phú trên GitHub. Người dùng (đặc biệt là non-tech founders, PM, designers, vibe coders) không muốn phải tự mình gọi tay từng agent hay quản lý hàng chục công cụ rời rạc. Đồng thời, qua giải phẫu ngược (`/rea`) ứng dụng `design:os studio v0.7.6`, mô hình vận hành **One-Person Company (OPC)** chứng minh hiệu quả vượt trội khi có 1 Agent điều phối trung tâm, 1 Meta-Agent chuyên tạo/nâng cấp kỹ năng và luật chơi, cùng cơ chế kiểm định độc lập.

## 2. Decision
1. **Chuẩn hóa Định danh Agent Đồng bộ với Cowok.ai:**
   - **`cowok` (`Cowok Orchestrator`):** Đóng vai trò **Master Orchestrator / Chief-of-Staff duy nhất** giao tiếp với người dùng. Người dùng chỉ cần giao tiếp với `cowok`; `cowok` tự động phân rã công việc, lắp ráp ngữ cảnh JIT và điều phối toàn bộ đội ngũ sub-agents/execution agents mà không cần con người can thiệp thủ công.
   - **`advisor` (`Advisor Agent`):** Đóng vai trò **Meta-Agent / Skill & Routine Spawner** (tương đương vai trò kiến trúc sư hệ thống bot). `advisor` chịu trách nhiệm đóng gói kỹ năng mới theo chuẩn `agentskills.io`, thiết lập lịch trình tự động (`Cron Routines`), và vận hành **Vòng lặp Tự tiến hóa 6 Bước (6-Step Self-Evolution Hardening Loop)** khi phát hiện lỗi lặp lại.
2. **Môi trường Tự hành cho Execution Agent Fleet (`crates/agent-fleet`):**
   - Hỗ trợ đa kênh thực thi thông qua **Embedded Native PTY (`portable-pty`)** tiêm tự động biến môi trường `$UIUX_LIVE_SPEC` (cập nhật trực tiếp lên Canvas trong `<50ms` kèm hoàn tác `Cmd+Z`) và **File-Backed Coordination Mailbox (`.cowok-design/coordination/`)**.
   - Áp dụng hợp đồng **`ONE_JOB` + `NEVER`**, nguyên tắc **`"Builder != Scorer"` (Độc lập Kiểm định)**, và thang bằng chứng **`E1..E4 Verify Ladder`**.
3. **Chuẩn hóa Plugin & Kỹ năng theo `https://agentskills.io/home` (`crates/skill-repackager` & `crates/plugin-lifecycle`):**
   - Mọi repo GitHub, workflow hoặc skill bên ngoài khi tích hợp vào UI/UX OS đều được chuẩn hóa sang cấu trúc `SKILL.md` (3 tầng Progressive Disclosure) + `plugin.json` tương thích Cowok.ai, kèm **Cron Job Scheduler** tự động quét và cập nhật phiên bản mới từ GitHub.
4. **Tích hợp `Design-Only REA Forge` & `JEV — TypeSafe` (`crates/design-rea-forge` & `crates/jev-engine`):**
   - Cho phép phân tích ngược các ứng dụng/website mẫu **giới hạn nghiêm ngặt ở hành vi thiết kế (Design-Only Boundary)** để trích xuất Design DNA thành skill.
   - Tích hợp 6 nguyên hàm `JEV — TypeSafe` (`Jev Choice` <2ms, `Jev Noul` <1ms, `Speculative Multi-Head`, `window.__jevFast` + `elementFromPoint`, `StaleFile` OCC, `3-Strike Breaker`) cùng cơ chế **Model Capability Adapter (3 Profiles)** và **Design Tournament Mode (đấu loại 2–3 phương án song song)**.

## 3. Consequences
- Đồng bộ 100% ngôn ngữ kiến trúc và trải nghiệm điều phối giữa **UI/UX OS** và **Cowok.ai** thông qua cặp đôi `cowok` (Orchestrator) và `advisor` (Meta-Agent).
- Cho phép bất kỳ Execution Agent thế hệ mới nào (`Grok`, `Muse`, `Duet`, `Dot`, `Claude`, `Codex`, `AGY`) tham gia vào dây chuyền sản xuất giao diện và bài thuyết trình HTML dưới sự kiểm soát chất lượng cơ khí hóa.
