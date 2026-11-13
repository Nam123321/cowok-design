---
type: I-Wish Story
id: "04.3"
title: "Story 04.3: Day-0 Onboarding Wizard, Honest 16+ CLI Doctor (Extension), OS Keychain/0600 Vault & SSRF-Guarded BYOK Verifier"
description: "Triển khai bề mặt mở rộng HYBRID / EXTENSION_PLUGIN và két bảo mật cho Epic 04 (GAP-01, GAP-02): Day-0 First-Run Onboarding Wizard (<60s) dò 16+ Local Coding CLI qua execFile an toàn (zero sh -c, timeout <=1500ms) tách bạch trung thực trạng thái installed vs authenticated và MCP Fallback; thay stub /api/doctor trong SettingsGeneralModelsPane bằng Live BYOK Verifier (<3s) có bộ lọc SSRF + redaction; bảo vệ API key qua OS Keychain hoặc Argon2id + AES-256-GCM .cowok-design/vault.enc (0o600) (chỉ ephemeral khi COWOK_VAULT_MODE=ephemeral), cấm lưu secrets/flags trong localStorage, đồng bộ agent_settings SQLite V5 và tự động gắn X-Cowok-Session-Token qua src/api/."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.3/story.md"
status: refactored
risk_tier: 3
delivery: hybrid
timestamp: "2026-11-13T12:10:00+07:00"
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
  - "POST /api/v1/byok/verify"
  - "PUT /api/v1/settings/vault"
  - "GET /api/v1/settings/agent"
  - "PUT /api/v1/settings/agent"
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
Story 04.3 tách riêng toàn bộ miền rủi ro bảo mật `EXTENSION_PLUGIN` (Local CLI Host Probe) và `Credential Vault / BYOK` ra khỏi `Story 04.2` nhằm kiểm soát chặt độ phức tạp (`CS = 5.5 < 7`), đồng thời sửa thẳng các lỗ hổng đã kiểm chứng trong codebase:
1. **Loại bỏ báo cáo auth giả & shell injection:** `src/app/api/doctor/route.ts` hiện gộp `--version` thành `authenticated: true` cho CLI. Story 04.3 yêu cầu dò `16+` allowlisted Local Coding CLIs qua `execFile` (`argv` tách rời, **không** dùng `sh -c`, stdin đóng `/dev/null`, timeout `<=1500ms`) và tách rạch ròi **`installed`** vs **`authenticated`** (chỉ đánh dấu `authenticated: true` khi probe session/auth thật xác nhận!). Xóa import trực tiếp server module `app/api/doctor/route` khỏi client `OnboardingWizardModal.tsx#L6` và đi qua `src/api/doctor.ts`.
2. **Thay stub BYOK giả & thêm SSRF Guard:** `SettingsGeneralModelsPane.tsx#L204` hiện đang gọi `fetch('/api/doctor')` làm nút "Test Connection". Story 04.3 thay thế bằng endpoint `/api/v1/byok/verify` chạy phía daemon (`<3s` timeout), kiểm tra `base_url` qua SSRF filter (HTTPS, chặn IP nội bộ RFC1918/link-local/`169.254.169.254`, chỉ cho phép localhost Ollama khi có cờ allowlist), và ẩn hoàn toàn key trong log.
3. **Két bảo mật Keychain / `0o600` Encrypted Vault & Zero-Secret `localStorage`:** Xóa bỏ lưu key hoặc cờ bảo mật vào `localStorage` (`useWorkbenchSessionSettings` chuyển sang đọc/ghi `agent_settings` trong SQLite V5); lưu secret vào OS Keychain hoặc file mã hóa Argon2id + AES-256-GCM `.cowok-design/vault.enc` (`0o600`), chỉ dùng ephemeral RAM khi người dùng opt-in tường minh `COWOK_VAULT_MODE=ephemeral`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.0, Data=1.0, Integration=1.0, UI=1.0, Security=1.0, Unknowns=0.5 $\rightarrow$ `CS = 5.5 < 7`, đạt ngưỡng an toàn).

## 3. Acceptance Criteria (AC)
- **AC-01 (Day-0 First-Run Onboarding Wizard `<60s` & Honest 16+ Local CLI Doctor via `execFile`):** Cung cấp Day-0 Onboarding Wizard (`<60s`) tự động dò ít nhất 16 Local Coding CLIs allowlisted (`claude`, `codex`, `gemini`, `opencode`, `aider`, `goose`, `qwen`, `copilot`, `cursor`, `windsurf`, `kilo`, `amp`, `cline`, `roo`, `kiro`, `trae`) qua `execFile` (`argv` cố định, zero shell `sh -c`, `/dev/null` stdin, `<=1500ms` timeout); tách bạch **`installed` vs `authenticated`** trung thực (không được xem exit code `0` của `--version` là đã đăng nhập); hỗ trợ khai báo MCP Server fallback lưu vào bảng `mcp_servers` SQLite V5; xóa import server route khỏi `OnboardingWizardModal.tsx` sang `src/api/doctor.ts`.
- **AC-02 (SSRF-Guarded Live BYOK Key Verifier `<3s` & Diagnostics Redaction):** Thay thế hoàn toàn stub `/api/doctor` trong `SettingsGeneralModelsPane.tsx` bằng `src/api/byok.ts` gọi `POST /api/v1/byok/verify` trong daemon (`<3s` HTTP timeout) xác thực key thật tới provider; cài đặt bộ lọc SSRF (ép HTTPS, chặn DNS/IP RFC1918, `127.0.0.0/8`, `169.254.169.254` trừ localhost Ollama allowlisted) và mask/redact mọi fragment của API key khỏi error payload & pino/tracing logs.
- **AC-03 (`[EDGE-CASE]` OS Keychain -> Encrypted `0o600` Daemon Vault `.cowok-design/vault.enc` -> Explicit Ephemeral Opt-In; Zero Secrets in `localStorage`):** Lưu BYOK credentials theo thứ tự ưu tiên OS Keychain -> tệp `.cowok-design/vault.enc` mã hóa Argon2id + AES-256-GCM quyền `0o600`; từ chối fallback âm thầm về bộ nhớ tạm nếu thiếu Keychain (chỉ cho phép khi có `COWOK_VAULT_MODE=ephemeral`); lưu 3 chế độ quyền (`Auto-Approve | Ask Every Time | Plan Only`) và cấu hình model vào bảng `agent_settings` (SQLite V5) — **nghiêm cấm** ghi secrets hoặc security flags xuống `localStorage`.
- **AC-04 (`[EDGE-CASE]` Automatic `X-Cowok-Session-Token` Injection & Fail-Closed Extension Boundary):** Tất cả lời gọi từ `src/api/doctor.ts` và `src/api/byok.ts` tự động đính kèm `X-Cowok-Session-Token` qua `apiClient` trung tâm (không yêu cầu thao tác copy thủ công); extension probe (`agent-enablement.md`) hoạt động ở chế độ đọc (`READ_ONLY`) đối với hệ thống máy chủ và chỉ lưu cấu hình xuống SQLite V5 sau xác nhận tường minh của người dùng.

## 4. Tasks
- [ ] **Task 04.3.1**: Tách và tái cấu trúc CLI Doctor (`EXTENSION`) với allowlist 16+ CLIs qua `execFile` (`<=1500ms`, zero `sh -c`), logic kiểm tra `installed` vs `authenticated` trung thực và cấu hình `mcp_servers` SQLite V5 (thỏa mãn `AC-01`).
- [ ] **Task 04.3.2**: Xây dựng endpoint daemon `POST /api/v1/byok/verify` có SSRF Guard + Log Redactor, cập nhật `SettingsGeneralModelsPane.tsx` & `OnboardingWizardModal.tsx` dùng `src/api/doctor.ts` / `src/api/byok.ts` kèm tự động gắn `X-Cowok-Session-Token` (thỏa mãn `AC-01`, `AC-02`, `AC-04`).
- [ ] **Task 04.3.3**: Cài đặt OS Keychain adapter + Encrypted Vault `.cowok-design/vault.enc` (Argon2id + AES-256-GCM, `0o600`) và opt-in `COWOK_VAULT_MODE=ephemeral`, kèm test kiểm tra không rò rỉ secret (thỏa mãn `AC-03`).
- [ ] **Task 04.3.4**: Di chuyển `useWorkbenchSessionSettings` & cờ security/permission mode (`Auto-Approve | Ask Every Time | Plan Only`) sang `agent_settings` SQLite V5 và cấm ghi xuống `localStorage` (thỏa mãn `AC-03`, `AC-04`).

## 5. AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Verification Method | Status |
| :--- | :--- | :--- | :--- | :--- |
| **AC-01** | Day-0 Wizard & Honest 16+ CLI Doctor (`execFile`) | `Task 04.3.1`, `Task 04.3.2` | CLI Allowlist, Non-Shell & Auth Distinction Test | pending |
| **AC-02** | SSRF-Guarded Live BYOK Verifier (<3s) | `Task 04.3.2` | SSRF Block (`169.254.169.254`/RFC1918) & Timeout Test | pending |
| **AC-03** | [EDGE-CASE] Keychain/0600 Vault & Zero-`localStorage` Secrets | `Task 04.3.3`, `Task 04.3.4` | Vault `0o600` Crypto & Zero-LS AST Audit | pending |
| **AC-04** | [EDGE-CASE] Auto Session Token & Extension Boundary | `Task 04.3.2`, `Task 04.3.4` | Token Header Injection & Read-Only Probe Test | pending |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["04.2"]`

### Impacts
- Cung cấp hạ tầng sẵn sàng kết nối AI (Local Coding CLI + MCP + Live BYOK Vault) cho `Story 04.4` (Keyed Execution Coordinator), `Epic 05` (SDI Engine) và `Epic 11` (BYOS/BYOK Sandbox).

### Consumes
- Tiêu thụ bảng `agent_settings`, `mcp_servers` (SQLite `V5`) và Streaming BFF / `X-Cowok-Session-Token` Kernel từ `Story 04.2`.

### Shared Entities
- Quản lý `AgentSetting` (`agent_settings`) và `McpServer` (`mcp_servers`) trong `V5__epic04_complete_contracts.sql`; đọc `Project` (`projects`).

### Cross-Portal
- Áp dụng đồng nhất Onboarding & Settings Vault cho toàn Workspace/Portal trong Cowok Design OS.

## 7. QA Simulator Guardian Scorecard
| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Dò trung thực `installed` vs `authenticated` cho 16+ CLI và verify BYOK thật `<3s` |
| **2. Data Integrity & State** | `9.6/10` | `agent_settings` & `mcp_servers` trên SQLite V5 thay hoàn toàn cho `localStorage` |
| **3. Security & Validation** | `9.8/10` | Cấm `sh -c`, lọc SSRF `169.254.169.254`/RFC1918, Argon2id+AES-256-GCM `0o600` & zero redaction leak |
| **4. Performance & Scalability** | `9.5/10` | Song song dò CLI với trần `1500ms`/CLI, wizard Day-0 dưới `60s` |
| **5. Error Handling & Recovery** | `9.5/10` | Fail-closed khi thiếu Keychain (bắt opt-in `COWOK_VAULT_MODE=ephemeral`), fallback MCP rõ ràng |
| **6. Architectural Depth & Leverage** | `9.6/10` | Cô lập hoàn toàn bề mặt `EXTENSION_PLUGIN` (theo `agent-enablement.md` v1.1) khỏi Core Kernel |
| **7. UX Empathy** | `9.5/10` | Tháo nút thắt Day-0 non-tech, không báo xanh giả tạo khi CLI chưa đăng nhập |
| **TOTAL AVERAGE** | **9.59/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log
| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-13 | 1.0.0 | [evaluate-epic Deep Mode] Tách khỏi Story 04.2 thành Story 04.3 chuyên biệt cho Day-0 CLI Doctor (`EXTENSION_PLUGIN`), OS Keychain/`0o600` Vault & SSRF-Guarded BYOK Verifier; nâng cấp AC trung thực `installed` vs `authenticated` và xóa stub `/api/doctor` | Party Mode Council & Security Architect |
