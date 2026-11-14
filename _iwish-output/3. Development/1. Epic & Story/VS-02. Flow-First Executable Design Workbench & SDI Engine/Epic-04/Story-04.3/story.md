---
type: I-Wish Story
id: "04.3"
title: "Story 04.3: Day-0 Onboarding Wizard, Honest 16+ CLI Doctor (Extension), OS Keychain/0600 Vault & SSRF-Guarded BYOK Verifier"
description: "Triển khai bề mặt mở rộng HYBRID / EXTENSION_PLUGIN và két bảo mật cho Epic 04 (EC-E04-03, EC-E04-04): Day-0 First-Run Onboarding Wizard (<60s) dẫn nhập bằng thiết lập BYOK/provider không biệt ngữ + mục Developer CLI có thể gập gọn; tách bạch Passive Day-0 CLI Scan (PATH + metadata phi-Keychain + extension IDE cline/roo không gây bão modal securityd hay treo FD Electron) khỏi Explicit Per-CLI Active Auth Probe (concurrency <=4, CI=1, GIT_TERMINAL_PROMPT=0, closed FDs, -PGID SIGKILL <=1500ms); thay handle_verify_provider_key prefix mock (legacy_workbench_v6.rs:1525-1651) bằng Live Verifier <3s kèm reqwest Redirect Policy::none() + connect-time SocketAddr SSRF blocklist + sửa lỗi truyền nhầm byosEndpoint thành byosToken trong SettingsGeneralModelsPane.tsx; tự động fallback OS Keychain -> Argon2id+AES-256-GCM .cowok-design/vault.enc (0o600) và CHẶN hạ cấp agent_settings sang ephemeral khi vault.enc ghi được; chuẩn hóa hoàn toàn về /api/v1/{doctor/verify-key,agent-settings,mcp-servers} + SQLite V6/V7 qua src/api/workbench-settings.ts."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.3/story.md"
status: refactored
risk_tier: 3
delivery: hybrid
timestamp: "2026-11-14T08:47:00+07:00"
tags:
  - "story-04.3"
  - "epic-04"
  - "uxui-os"
  - "cli-doctor"
  - "byok-vault"
  - "extension-plugin"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-04/Story-04.3/agent-enablement.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-04/evaluation-report.md"
dependencies: ["04.2"]
provides_apis:
  - "internal:epic-04/story-04.3"
  - "GET /api/v1/doctor/clis"
  - "POST /api/v1/doctor/verify-key"
  - "PUT /api/v1/settings/vault"
  - "GET /api/v1/agent-settings"
  - "PUT /api/v1/agent-settings"
  - "GET /api/v1/mcp-servers"
  - "POST /api/v1/mcp-servers"
  - "DELETE /api/v1/mcp-servers/:server_id"
consumes_apis:
  - "GET /api/v1/projects"
  - "GET /api/v1/health"
---

# Story 04.3: Day-0 Onboarding Wizard, Honest 16+ CLI Doctor (Extension), OS Keychain/0600 Vault & SSRF-Guarded BYOK Verifier

- **FR Covered:** [FR-02: Interactive Multi-Portal Flow Graph, FR-03: State-Hydrated Live Preview, FR-06: Element-Anchored Comments (Day-0 Runtime Doctor & Secure BYOS/BYOK Enabler)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, Onboarding Doctor & Persistent Node Execution](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-04/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Story 04.3 tách riêng toàn bộ miền rủi ro bảo mật `EXTENSION_PLUGIN` (Local CLI Host Probe) và `Credential Vault / BYOK` ra khỏi `Story 04.2` (`CS = 5.5 < 7`), đồng thời khắc phục triệt để các khiếm khuyết trên `master`:
1. **Loại bỏ báo cáo auth giả, bão modal `securityd` & treo FD (`EC-E04-04`):** `src/app/api/doctor/route.ts` từng gộp `--version` thành `authenticated: true` và chạy probe chủ động tràn lan dễ kích hoạt popup macOS `securityd` hoặc treo FD Electron (`cursor`/`windsurf`/`trae`). Story 04.3 dẫn dắt Day-0 Wizard Step 1 bằng thiết lập BYOK/Provider tối giản không biệt ngữ (`<60s`), xếp bảng 16+ Developer CLI vào mục có thể gập gọn; tách đôi **Passive Day-0 Scan** (chỉ quét `PATH`, metadata cấu hình phi-Keychain & phân loại IDE extension như `cline`/`roo`) khỏi **Explicit Per-CLI Active Auth Probe** (`execFile` zero `sh -c`, `concurrency <= 4`, `CI=1`, `GIT_TERMINAL_PROMPT=0`, đóng FDs `/dev/null`, `-PGID` `SIGKILL <= 1500ms`), xóa import server route ở `OnboardingWizardModal.tsx#L6` về canonical SDK **`src/api/workbench-settings.ts` (`apiWorkbenchSettings` / `apiAgentSettings`)**.
2. **Thay prefix mock & vá SSRF + lỗi `byosToken` (`EC-E04-03`):** Thay thế mock kiểm tra tiền tố trong `handle_verify_provider_key` (`legacy_workbench_v6.rs:1525-1651`) và `fetch('/api/doctor')` ở `SettingsGeneralModelsPane.tsx` bằng `POST /api/v1/doctor/verify-key` (`<3s` timeout) gọi provider thật với `reqwest::redirect::Policy::none()`, connect-time `SocketAddr` SSRF blocklist (`127.0.0.0/8`, `::1`, `[::ffff:127.0.0.1]`, RFC1918, `169.254.0.0/16`, `100.64.0.0/10`, metadata hosts), redaction đầy đủ trong log/error, và sửa lỗi `SettingsGeneralModelsPane.tsx` đang truyền nhầm `byosEndpoint` vào trường `apiKey` (`byosToken`).
3. **Keychain -> `0o600` Encrypted Vault Auto-Fallback & Anti-Downgrade (`agent_settings` SQLite V6/V7):** Lưu secrets ưu tiên OS Keychain và **tự động fallback** sang file mã hóa Argon2id + AES-256-GCM `.cowok-design/vault.enc` (`0o600`). Cấm hạ cấp vault qua `PUT /api/v1/agent-settings` xuống `ephemeral` hễ `vault.enc` còn ghi được (`COWOK_VAULT_MODE=ephemeral` chỉ chấp nhận khi cả Keychain lẫn đĩa đều không ghi được); xóa bỏ toàn bộ secrets/cờ bảo mật trong `localStorage` của `useWorkbenchSessionSettings.ts` sang `agent_settings` & `mcp_servers` SQLite V6/V7.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.0, Data=1.0, Integration=1.0, UI=1.0, Security=1.0, Unknowns=0.5 $\rightarrow$ `CS = 5.5 < 7`, đạt ngưỡng an toàn).

## 3. Acceptance Criteria (AC)
- **AC-01 (`[EDGE-CASE]` `EC-E04-04`: Zero-Jargon Day-0 Wizard `<60s`, Passive CLI Scan vs Bounded Active Auth Probe):** Day-0 Onboarding Wizard (`<60s`) đặt thiết lập BYOK/provider dễ hiểu lên Bước 1 (gom 16+ CLI `claude, codex, gemini, opencode, aider, goose, qwen, copilot, cursor, windsurf, kilo, amp, cline, roo, kiro, trae` vào mục Developer CLI gập gọn); mặc định chạy **Passive Day-0 Scan** (kiểm `PATH`, config phi-Keychain, phân loại VS Code extension `cline`/`roo` không kích popup `securityd` hay treo Electron); chỉ khi người dùng kích **Explicit Per-CLI Active Auth Probe** mới thực thi `execFile` (`argv` cố định, zero `sh -c`, `concurrency <= 4`, `CI=1`, `GIT_TERMINAL_PROMPT=0`, closed FDs, `-PGID` `SIGKILL` khi quá `1500ms`) phân biệt rạch ròi `installed` vs `authenticated`; hỗ trợ MCP Fallback qua `/api/v1/mcp-servers` (SQLite V6/V7) và gọi qua `src/api/workbench-settings.ts` (`OnboardingWizardModal.tsx` không import server route).
- **AC-02 (`[EDGE-CASE]` `EC-E04-03`: Live `<3s` BYOK Verifier, `SocketAddr` Connect-Time SSRF Block & `byosToken` Fix):** Loại bỏ hoàn toàn prefix mock ở `handle_verify_provider_key` (`legacy_workbench_v6.rs:1525-1651`) và `fetch('/api/doctor')` trong `SettingsGeneralModelsPane.tsx`; nối qua `src/api/workbench-settings.ts` -> `POST /api/v1/doctor/verify-key` (`<3s` live probe) với `reqwest::redirect::Policy::none()`, connect-time `SocketAddr` SSRF filter chặn `127.0.0.0/8`, `::1`, `[::ffff:127.0.0.1]`, RFC1918, `169.254.0.0/16`, `100.64.0.0/10`, metadata hosts (chỉ allowlist localhost Ollama khi cấu hình explicit); redact 100% API key trong error/tracing; và sửa lỗi truyền nhầm `byosEndpoint` làm `apiKey` (`byosToken`) trong `SettingsGeneralModelsPane.tsx`.
- **AC-03 (`[EDGE-CASE]` Keychain -> `0o600` `.cowok-design/vault.enc` Auto-Fallback, Anti-Downgrade Guard & Zero-`localStorage` Security State):** Lưu credentials theo chuỗi OS Keychain -> tự động fallback `.cowok-design/vault.enc` (Argon2id + AES-256-GCM, `0o600`); **`PUT /api/v1/agent-settings` từ chối mọi yêu cầu hạ cấp `vault_mode` xuống `ephemeral` nếu `.cowok-design/vault.enc` ghi được** (chỉ cho phép `COWOK_VAULT_MODE=ephemeral` khi cả Keychain lẫn vault disk bất khả dụng); lưu 3 chế độ quyền (`Auto-Approve | Ask Every Time | Plan Only`) và `onboarding_done` vào `agent_settings` (SQLite V7) và thanh tẩy mọi secret/cờ bảo mật trong `localStorage` ở `useWorkbenchSessionSettings.ts`.
- **AC-04 (`[EDGE-CASE]` Automatic `X-Cowok-Session-Token` Injection & Fail-Closed Extension Boundary):** Tất cả lời gọi qua `src/api/workbench-settings.ts` (`apiWorkbenchSettings` / `apiAgentSettings`) tự động đính kèm `X-Cowok-Session-Token` qua `daemon-client.ts` / `apiClient` trung tâm; extension probe (`agent-enablement.md`) vận hành `READ_ONLY` với hệ thống máy chủ và chỉ ghi xuống SQLite V6/V7 sau xác nhận người dùng.

## 4. Tasks
- [ ] **Task 04.3.1**: Tách Passive Day-0 Scan vs Explicit Per-CLI Active Auth Probe (`concurrency <= 4`, `CI=1`, `GIT_TERMINAL_PROMPT=0`, closed FDs, `-PGID` `SIGKILL <= 1500ms`, phân loại extension `cline`/`roo`) cho 16+ CLIs, dẫn nhập Day-0 Wizard `<60s` với BYOK step-1 & mục Developer CLI gập gọn cùng `/api/v1/mcp-servers` SQLite V6/V7 (thỏa mãn `AC-01`).
- [ ] **Task 04.3.2**: Thay prefix mock ở `handle_verify_provider_key` (`legacy_workbench_v6.rs:1525-1651`) bằng `POST /api/v1/doctor/verify-key` `<3s` live probe + `reqwest::redirect::Policy::none()` + connect-time `SocketAddr` SSRF blocklist (`[::ffff:127.0.0.1]`, RFC1918, `169.254.0.0/16`, `100.64.0.0/10`) + log redaction; sửa lỗi `byosToken`/`byosEndpoint` trong `SettingsGeneralModelsPane.tsx` và bỏ server import khỏi `OnboardingWizardModal.tsx` sang `src/api/workbench-settings.ts` (thỏa mãn `AC-01`, `AC-02`, `AC-04`).
- [ ] **Task 04.3.3**: Cài đặt OS Keychain -> `.cowok-design/vault.enc` (`0o600`, Argon2id + AES-256-GCM) auto-fallback và anti-downgrade guard chặn `PUT /api/v1/agent-settings` hạ cấp `ephemeral` khi `vault.enc` ghi được (thỏa mãn `AC-03`).
- [ ] **Task 04.3.4**: Di chuyển toàn bộ cờ bảo mật / `permission_mode` / `vault_mode` / `onboarding_done` trong `useWorkbenchSessionSettings.ts` khỏi `localStorage` sang `GET/PUT /api/v1/agent-settings` SQLite V7 (thỏa mãn `AC-03`, `AC-04`).

## 5. AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Verification Method | Status |
| :--- | :--- | :--- | :--- | :--- |
| **AC-01** | [EDGE-CASE] Zero-Jargon Wizard `<60s`, Passive vs Active 16+ CLI Doctor | `Task 04.3.1`, `Task 04.3.2` | Passive No-Popup Scan, Bounded `-PGID` Timeout & MCP SQLite Test | pending |
| **AC-02** | [EDGE-CASE] Live Verifier `<3s`, `SocketAddr` SSRF Block & `byosToken` Fix | `Task 04.3.2` | Redirect `Policy::none()`, `[::ffff:127.0.0.1]` SSRF & Redaction Test | pending |
| **AC-03** | [EDGE-CASE] Keychain/`0o600` Auto-Fallback, Anti-Downgrade & Zero-LS | `Task 04.3.3`, `Task 04.3.4` | Vault Fallback CRYPTO `0o600`, Anti-Ephemeral Guard & Zero-LS Test | pending |
| **AC-04** | [EDGE-CASE] Auto Session Token (`apiWorkbenchSettings`) & Extension Boundary | `Task 04.3.2`, `Task 04.3.4` | Token Header Injection & Read-Only Probe Test | pending |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["04.2"]`

### Impacts
- Cung cấp hạ tầng sẵn sàng kết nối AI (Local Coding CLI + MCP + Live BYOK Vault V6/V7) cho `Story 04.4` (Keyed Execution Coordinator), `Epic 05` (SDI Engine) và `Epic 11` (BYOS/BYOK Sandbox).

### Consumes
- Tiêu thụ bảng `agent_settings` (kể cả cột V7 `permission_mode`, `vault_mode`, `onboarding_done`), `mcp_servers` (SQLite `V6/V7`) và Streaming BFF / `X-Cowok-Session-Token` Kernel từ `Story 04.2`.

### Shared Entities
- Quản lý `AgentSetting` (`agent_settings`) và `McpServer` (`mcp_servers`) trong `V6/V7`; đọc `Project` (`projects`).

### Cross-Portal
- Áp dụng đồng nhất Onboarding & Settings Vault cho toàn Workspace/Portal trong Cowok Design OS.

## 7. QA Simulator Guardian Scorecard
| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.7/10` | Giải quyết đúng passive/active CLI scan, live `<3s` verifier (bỏ prefix mock) và `byosToken` bug |
| **2. Data Integrity & State** | `9.7/10` | `agent_settings` V7 (`permission_mode`, `vault_mode`, `onboarding_done`) & `mcp_servers` trên SQLite |
| **3. Security & Validation** | `9.9/10` | Chống DNS rebinding bằng `SocketAddr` SSRF check + `Policy::none()`, anti-downgrade `vault.enc`, zero `localStorage` |
| **4. Performance & Scalability** | `9.6/10` | Passive Day-0 scan zero popup/zero FD hang; Active probe bounded `concurrency <= 4`, `-PGID <= 1500ms` |
| **5. Error Handling & Recovery** | `9.6/10` | Keychain -> `vault.enc` auto-fallback, từ chối hạ cấp `ephemeral` khi vault khả dụng |
| **6. Architectural Depth & Leverage** | `9.7/10` | Chuẩn hóa về `src/api/workbench-settings.ts` & cô lập `EXTENSION_PLUGIN` (`agent-enablement.md`) |
| **7. UX Empathy** | `9.7/10` | Bước 1 Day-0 BYOK tối giản không biệt ngữ `<60s`, không kẹt hộp thoại Keychain macOS |
| **TOTAL AVERAGE** | **9.70/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log
| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-13 | 1.0.0 | [evaluate-epic Deep Mode] Tách khỏi Story 04.2 thành Story 04.3 chuyên biệt cho Day-0 CLI Doctor (`EXTENSION_PLUGIN`), OS Keychain/`0o600` Vault & SSRF-Guarded BYOK Verifier; nâng cấp AC trung thực `installed` vs `authenticated` và xóa stub `/api/doctor` | Party Mode Council & Security Architect |
| 2026-11-14 | 1.1.0 | [evaluate-epic Report Sync] Chỉnh URI sang `/api/v1/{doctor/verify-key,agent-settings,mcp-servers}` và `src/api/workbench-settings.ts`; tách Passive Day-0 CLI Scan vs Bounded Active Auth Probe (`EC-E04-04`); thay prefix mock `legacy_workbench_v6.rs:1525-1651` bằng `SocketAddr` SSRF + `Policy::none()` + sửa `byosToken`; bắt buộc Keychain -> `0o600` `vault.enc` auto-fallback & anti-downgrade guard (`V7`) | /evaluate-epic Council & Security Architect |
