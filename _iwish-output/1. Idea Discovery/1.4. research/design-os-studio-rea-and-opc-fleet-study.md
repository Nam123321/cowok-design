---
type: I-Wish Research Study
title: "Deep REA v6.8.0 Study: design:os Studio v0.7.6, OPC Autonomous Execution Fleet (cowok & advisor), agentskills.io & JEV — TypeSafe"
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/1. Idea Discovery/1.4. research/design-os-studio-rea-and-opc-fleet-study.md"
tags: ["rea", "design-os-studio", "cowok-orchestrator", "advisor-agent", "agentskills-io", "jev-typesafe", "opc-fleet", "rust-binaries"]
timestamp: "2026-11-02T13:15:00+07:00"
status: completed
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/2. Product Planning/2.1. product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/2. Product Planning/2.5. architecture.md"
---

# Deep REA v6.8.0 Study: `design:os studio v0.7.6`, OPC Autonomous Execution Fleet (`cowok` & `advisor`), `agentskills.io` & `JEV — TypeSafe`

## 1. Mục tiêu Nghiên cứu & Giải phẫu Ngược (`/rea` v6.8.0)

Báo cáo này ghi lại kết quả giải phẫu ngược nhị phân thực tế (`/rea` `inspect-desktop-application`, `inspect-plist`, `inspect-rust-binary --architecture arm64`) trên ứng dụng **`/Applications/designos.app` (`design:os studio v0.7.6`, bundle ID `os.design.studio`)** kết hợp với nghiên cứu mô hình vận hành **One-Person Company (OPC) Execution Agent Fleet**, chuẩn đóng gói mở **`https://agentskills.io/home`**, và kiến trúc thực thi siêu tốc **`JEV — TypeSafe`**.

Đồng thời, theo quyết định đồng bộ hóa hệ sinh thái với **Cowok.ai**:
- **Master Orchestrator Agent (`Chief-of-Staff` / tương đương `"J"` trong `design:os`):** Được định danh chính thức là **`cowok`** (`Cowok Orchestrator`).
- **Meta-Agent / Skill & Routine Spawner (tương đương `"Dr. Eggbot"` trong `design:os`):** Được định danh chính thức là **`advisor`** (`Advisor Agent`).

---

## 2. Kết quả Giải phẫu Ngược Nhị phân `/Applications/designos.app` (`v0.7.6`)

### 2.1. Cấu trúc 4 Universal Binaries & 9 Internal Rust Crates
Qua công cụ `rea.mjs inspect-desktop-application` và `inspect-rust-binary --architecture arm64`, bộ cài `designos.app/Contents/MacOS/` chứa 4 binary Mach-O Universal (`x86_64` + `arm64`) biên dịch bằng `rustc 1.93.1`:

| Binary Name | Kích thước | Vai trò Kiến trúc |
| :--- | :---: | :--- |
| `designos-studio` | `35.1 MB` | Ứng dụng Desktop GUI chính (`Tauri v2` + `wry` WebView + `portable-pty` Agent Terminal Hub + Live Spec Watcher). |
| `designos` | `13.4 MB` | CLI headless phục vụ các Execution Agents (`designos validate`, `export`, `tokens`, `import-figma`, `capture-url`). |
| `designos-mcp` | `10.1 MB` | Máy chủ Model Context Protocol (`stdio`) cung cấp 22+ công cụ đọc/ghi tài liệu thiết kế, auto-layout và kiểm lỗi. |
| `designos-diff` | `9.2 MB` | Công cụ so sánh trực quan (Visual & Semantic AST Diff) giữa 2 phiên bản thiết kế hoặc giữa bản vẽ và DOM thực tế. |

Bản đồ **9 Crates nội bộ** được trích xuất từ symbol table:
1. `designos-doc`: Quản lý tài liệu `.designos`, schema validation, `normalize_spec`, và vá tài liệu (`patch.rs`).
2. `designos-mcp`: Giao thức JSON-RPC 2.0 MCP, bao gồm `loop_guard.rs` chống lặp vô hạn khi Agent gọi tool liên tiếp.
3. `designos-render`: Bộ kết xuất 2D/Canvas và đo đạc hộp giới hạn (bounding box).
4. `designos-export`: Xuất bản thiết kế ra mã nguồn và tài sản đồ họa.
5. `designos-fig`: Bộ đọc/chuyển đổi định dạng Figma sang cấu trúc chuẩn của OS.
6. `designos-svg`: Chuẩn hóa đường dẫn vector SVG và biểu tượng.
7. `designos-webcapture`: Chụp cấu trúc DOM và style từ trang web thực tế (`capture_url_to_spec`).
8. `designos-diff`: So sánh sai lệch cấu trúc và điểm ảnh giữa hai bản thiết kế.
9. `designos-app`: Quản lý trạng thái cửa sổ, `live_spec.rs`, và `agent_pty.rs`.

### 2.2. Cơ chế `$DESIGNOS_LIVE_SPEC` (`<50ms` Hot-Apply) & Embedded PTY Agent Hub (`agent_pty.rs`)
- Trong `crates/designos-app/src/agent_pty.rs` và `live_spec.rs`, khi khởi chạy một Execution Agent trong terminal tích hợp (`portable-pty`), hệ thống tự động tiêm các biến môi trường:
  - `$DESIGNOS_LIVE_SPEC`: Đường dẫn tới file JSON đặc tả đang mở trên màn hình.
  - `$DESIGNOS_WORKSPACE`: Thư mục gốc của dự án.
  - `$DESIGNOS_CLI`: Đường dẫn tuyệt đối tới binary CLI.
- Khi Agent (như `Claude Code`, `Codex`, `Gemini`, `Cursor`) ghi đè hoặc patch file `$DESIGNOS_LIVE_SPEC`, file-watcher của Rust đọc, kiểm tra hợp lệ và cập nhật thẳng lên Canvas trong **<50ms** kèm khả năng hoàn tác 1 bước (`Cmd+Z`), loại bỏ hoàn toàn bước copy-paste thủ công.

### 2.3. Bộ 17 Quy tắc Kiểm định Thiết kế Có Giải nghiệm (17-Rule Solver-Backed Design Linter)
Trích xuất từ `crates/designos-mcp/src/main.rs` cho thấy 17 quy tắc kiểm lỗi tự động được chia làm 3 nhóm:
- **Layout & Geometry:** `child-overflow`, `sibling-overlap`, `offscreen-layer`, `zero-size-layer`, `clipped-text`, `auto-layout-Inconsistency`.
- **Accessibility & Touch:** `small-touch-target` (<44x44px), `low-contrast-text` (WCAG AA), `tiny-text` (<11px), `missing-alt-or-label`.
- **System Hygiene & Tokens:** `hardcoded-color` (chưa gắn token), `unaligned-pixel`, `deep-nesting`, `duplicate-layer-id`, `empty-group`, `missing-font-fallback`, `broken-component-ref`.

---

## 3. Thiết kế Hệ thống OPC Execution Agent Fleet cho UI/UX OS (`cowok` & `advisor`)

Kế thừa triết lý **One-Person Company (OPC)** và nâng cấp toàn diện cho **UI/UX OS (FlowForge)**, hệ thống thiết lập môi trường tự hành cho đa dạng các Execution Agents (`Claude Code`, `Codex`, `Antigravity`, `Grok Bot`, `Muse`, `Duet`, `Dot`, `$SHELL`) dưới sự chỉ huy của 2 Agent lõi đồng bộ với **Cowok.ai**:

### 3.1. Hai Agent Chỉ huy Cốt lõi (`cowok` & `advisor`)
1. **`cowok` (Master Orchestrator Agent / Chief-of-Staff):**
   - Giao diện điều phối duy nhất mà con người (System Architect / Founder / PM) tương tác.
   - Nhận ý định cấp cao, phân rã thành các gói công việc nguyên tử, chọn bộ ngữ cảnh từ 9 Foundational Specs, giao việc cho các Execution Agents qua Mailbox hoặc PTY, và tổng hợp báo cáo trình duyệt.
2. **`advisor` (Meta-Agent / Skill & Routine Spawner):**
   - Chuyên trách thiết kế tổ chức Agent, kiểm toán lỗ hổng quy trình, tạo mới hoặc nâng cấp các bộ kỹ năng chuẩn `agentskills.io` (`SKILL.md`), và thiết lập lịch trình tự động (`Cron Routines`).
   - Khi một Execution Agent mắc lỗi lặp lại, `advisor` thực thi **Vòng lặp Tự tiến hóa 6 Bước (6-Step Self-Evolution Hardening Loop)**:
     `Fail` $\rightarrow$ `Root Cause` $\rightarrow$ `Patch SKILL.md / Rule` $\rightarrow$ `Patch AST / Eval Check` $\rightarrow$ `Regression Test` $\rightarrow$ `Close`.

### 3.2. Quy ước Hợp đồng Thực thi (`ONE_JOB` + `NEVER`) & Thang Kiểm chứng `E1..E4`
- **Hợp đồng `ONE_JOB` & `NEVER`:** Mỗi Execution Agent trong đội hình chỉ được giao đúng 1 nhiệm vụ cụ thể (`ONE_JOB`) và danh sách các hành vi bị cấm tuyệt đối (`NEVER`, ví dụ: *NEVER tự ý sửa `02-brand-guideline.md`*, *NEVER tự chấm điểm sản phẩm của chính mình*).
- **Nguyên tắc `"Builder != Scorer"` (Độc lập Kiểm định):** Agent sinh ra giao diện/slide (`Builder`) tuyệt đối không được quyền tự đánh dấu `PASS`. Một Agent độc lập (`Verifier` / `Visual QA Gate`) chấm điểm dựa trên bộ tiêu chí mù (Blind Rubric).
- **Thang Bằng chứng 4 Cấp (`E1..E4 Verify Ladder`):**
  - `E1 (Static & Contract Check)`: Đạt schema JSON, `ts-rs` types và ràng buộc Project Constraint Engine.
  - `E2 (Deterministic Build & AST Gate)`: Vượt qua `oxc` AST Linter (<1ms) và kiểm tra liên kết `data-ff-id` / `fixtureRef`.
  - `E3 (Headless Render & Multi-Viewport Geometry)`: Render thật trên Chromium ở cả 3 khung nhìn (`Desktop 1440`, `Tablet 768`, `Mobile 390`) hoặc khung `16:9 (1920x1080)`, đạt WCAG 2.2 AA và không tràn khung.
  - `E4 (Independent Rubric Score & Visual Diff)`: Đạt điểm thẩm mỹ & nghiệp vụ từ `Verifier Agent` độc lập trước khi trình Human duyệt cuối.

---

## 4. Tích hợp Chuẩn Mở `agentskills.io` & `JEV — TypeSafe` vào Nhân Rust

### 4.1. Bộ Đóng gói Lại Kỹ năng theo Chuẩn `https://agentskills.io/home` (`crates/skill-repackager`)
- Mọi repository GitHub, workflow hoặc skill rời rạc khi nạp vào UI/UX OS đều được chuẩn hóa về cấu trúc thư mục mở của `agentskills.io`:
  - `SKILL.md` (tuân thủ cơ chế **Progressive Disclosure 3 tầng**: Tầng 1 Metadata YAML `~100 tokens` nạp sẵn; Tầng 2 Hướng dẫn thực thi `<5,000 tokens` nạp khi kích hoạt; Tầng 3 Tài nguyên `scripts/`, `references/`, `assets/` nạp theo yêu cầu).
  - Bọc ngoài bởi `plugin.json` đồng bộ 100% với kiến trúc Plugin của **Cowok.ai**.
- Kèm theo **Bộ Quản lý Vòng đời Plugin 6 Trạng thái** (`Discovered` $\rightarrow$ `Validated` $\rightarrow$ `Installed` $\rightarrow$ `Active` $\rightarrow$ `Deprecated` $\rightarrow$ `Archived`) và **Cron Job Scheduler** tự động quét cập nhật từ GitHub upstream.

### 4.2. Sáu Nguyên hàm Tăng tốc `JEV — TypeSafe` (`crates/jev-engine`)
1. **`Jev Choice` (<2ms Routing):** Chấm điểm logprobs xác định để chọn đúng Portal, Layout, Component và Skill mà không tốn vòng suy luận CoT dài dòng.
2. **`Jev Noul` (<1ms Pre-Filter):** Quét vi phạm ràng buộc dự án (Tailwind/Hex/Icon/Forbidden Imports) ngay trên luồng sinh mã trong dưới 1ms.
3. **`Speculative Multi-Head Ambiguity Scorer`:** Đánh giá độ mơ hồ của yêu cầu (`Structured Design Intent - SDI`) để kích hoạt **Visual Clarification (2–3 phương án trực quan)** khi cần thiết.
4. **`window.__jevFast` + `elementFromPoint` Occlusion Guard:** Chụp bản đồ tọa độ DOM nguyên tử trong 1 lệnh IPC và kiểm tra che khuất điểm ảnh tâm phần tử, đảm bảo 0% click nhầm khi kiểm thử tự động.
5. **`StalePage` / `StaleFile` Fingerprint OCC Guard:** Kiểm soát đồng thời lạc quan (Optimistic Concurrency Control) bằng mã băm SHA-256 khi nhiều Execution Agents cùng thao tác trên một dự án.
6. **`3-Strike Monotonic Dead-Loop Circuit Breaker`:** Tự động ngắt mạch nếu một Agent thất bại 3 lần liên tiếp trên cùng một mã lỗi, chuyển quyền xử lý về cho `cowok` và `advisor`.
