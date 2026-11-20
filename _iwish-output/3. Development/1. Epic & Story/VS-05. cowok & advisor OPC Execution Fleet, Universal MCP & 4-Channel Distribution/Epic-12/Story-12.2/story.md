---
type: I-Wish Story
id: "12.2"
title: "Story 12.2: Kênh 2 & 3 — Ứng dụng Desktop Tauri v2 (.dmg/.msi/.AppImage) & Gói Native Plugin cho Cowok.ai"
description: "Đóng gói ứng dụng Desktop gốc nhẹ (~25MB) cho người dùng non-tech và xuất bản gói cài đặt 1-click Native Plugin & Skill Pack cho Cowok.ai."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-12/Story-12.2/story.md"
status: refactored
risk_tier: 3
delivery: hybrid
adr_ref: "ADR-007"
components:
  - id: "tauri-desktop-host"
    packaging: "KERNEL_CORE"
  - id: "cowok-native-plugin-pack"
    packaging: "EXTENSION_PLUGIN"
timestamp: "2026-11-20T18:00:00+07:00"
tags:
  - "story-12.2"
  - "epic-12"
  - "uxui-os"
  - "tauri-v2"
  - "cowok-plugin"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
dependencies: ["12.1", "08.5"]
provides_apis:
  - "internal:epic-12/story-12.2"
consumes_apis:
  - "POST /api/v1/mcp/install"
  - "POST /api/v1/de/run"
---

# Story 12.2: Kênh 2 & 3 — Ứng dụng Desktop Tauri v2 (`.dmg/.msi/.AppImage`) & Gói Native Plugin cho Cowok.ai

- **FR Covered:** [FR-30: Kênh 2 & 3 — Ứng dụng Desktop Tauri v2 (.dmg/.msi/.AppImage) & Gói Native Plugin cho Cowok.ai](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 12: 4-Channel Packaging & Distribution Architecture](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-12/epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Đóng gói ứng dụng Desktop gốc nhẹ (~25MB) trên **Tauri v2** (`.dmg/.msi/.AppImage`) cho người dùng non-tech dùng chung sidecar binary từ `Story 12.1`, và xuất bản **Native Plugin & Skill Pack** cài đặt 1-click cho `Cowok.ai` (mang trọn bộ `cowok-design-opt`, Split DE+DR runtime `Story 08.5`, Decision Cascade & MCP hooks vào không gian Cowok).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=0.5, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Ranh giới HMAC capability token từ WebView, xác thực chữ ký updater (`O_NOFOLLOW` + `nlink == 1`) và bảo toàn khóa `HUMAN_ONLY` khi đồng bộ plugin Cowok.ai.

## 3. Acceptance Criteria (AC)
- **AC-01 (Tauri v2 Desktop App `.dmg/.msi/.AppImage` ~25MB):** Đóng gói React 19 Workbench cùng sidecar `uiux-os` từ `Story 12.1` thành app desktop bản địa không đòi người dùng mở Terminal, hỗ trợ drag-and-drop workspace/CSV/Brand Kit thẳng vào khung chat (kết hợp Windows Job Object cleanup ở `11.1`).
- **AC-02 (1-Click Native Plugin & Skill Pack cho `Cowok.ai`):** Xuất bản manifest plugin chuẩn tương thích Cowok.ai đóng gói Split DE+DR Skill Pack (`Story 08.5`), tự động đồng bộ cấu hình MCP (`Story 11.2`) và cặp đôi `cowok` / `advisor` ngay sau khi cài, đồng thời giữ nguyên rào chắn `HUMAN_ONLY` không bị nới quyền.
- **AC-03 ([EDGE-CASE] Auto-Sync Drift Repair & Symlink/Hardlink Extraction Rejection):** Khi người dùng nâng cấp hoặc thay đổi thư mục workspace trong `Cowok.ai`, plugin tự phát hiện cấu hình MCP cũ và khôi phục an toàn mà không ghi đè khóa bí mật; giải nén plugin và updater bắt buộc kiểm tra `O_NOFOLLOW` + `nlink == 1` và từ chối mọi symlink thoát sandbox workspace.
- **AC-04 (Tauri v2 HMAC Preload Trust Boundary & Native Module / Updater Integrity Harness):** Khoanh vùng bảo mật chọn thư mục bằng **HMAC capability token** ở tiến trình Rust main (tuyệt đối không lộ đường dẫn filesystem thô xuống WebView untrusted) và chạy harness kiểm tra chữ ký SHA-256 của mọi native sidecar + auto-updater trước khi đóng gói phát hành.

## 4. Tasks
- [ ] **Task 12.2.1**: Cấu hình Tauri v2 bundle (`.dmg/.msi/.AppImage`) nhúng sidecar `uiux-os` và native workspace drag-and-drop cho non-tech (thỏa mãn `AC-01`).
- [ ] **Task 12.2.2**: Đóng gói Gói Native Plugin & Skill Pack (`hybrid` extension) cho `Cowok.ai` đồng bộ DE+DR + MCP + `cowok`/`advisor` (thỏa mãn `AC-02`, `AC-03`).
- [ ] **Task 12.2.3**: Cài đặt Tauri v2 HMAC Token Preload Trust Boundary (không lộ raw fs path tới WebView) và harness kiểm tra chữ ký Native Sidecar + Auto-updater Integrity (thỏa mãn `AC-03`, `AC-04`).
- [ ] **Task 12.2.4**: Viết kiểm thử bảo mật HMAC token forgery, updater signature mismatch và Cowok.ai plugin manifest validation (thỏa mãn `AC-01..AC-04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 12.2.1`, `Task 12.2.4` | Tauri v2 Desktop Bundle & Drag-Drop Smoke Test |
| **AC-02** | `Task 12.2.2`, `Task 12.2.4` | Cowok.ai Native Plugin Manifest & MCP Auto-Wire Test |
| **AC-03 (`[EDGE-CASE]`)** | `Task 12.2.2`, `Task 12.2.3` | Sync Drift Repair & Symlink Traversal Rejection Test |
| **AC-04** | `Task 12.2.3`, `Task 12.2.4` | HMAC Preload Security & Updater Integrity Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["12.1", "08.5"]

### Impacts
- Hoàn thiện trọn vẹn 4 kênh phân phối của `Epic 12` và đóng gói bàn giao cuối cùng trong `Story 13.1` (`M5`).

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: ["12.1", "08.5"] (`uiux-os` sidecar binary, Split DE+DR Skill Pack & Tool Jail).

### Shared Entities
- Đọc/Ghi `PROJECTS`, `SKILL_PLUGINS`, `user_skill_registry` (`V16`) trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ đủ App Desktop Tauri v2 lẫn Native Plugin cho Cowok.ai |
| **2. Data Integrity & State** | `9.4/10` | Ký checksum sidecar/updater & đồng bộ không ghi đè secret |
| **3. Security & Validation** | `9.6/10` | HMAC Capability Token ngăn WebView RCE/path disclosure |
| **4. Performance & Scalability** | `9.5/10` | Gói cài đặt nhẹ (~25MB) dùng lại sidecar Rust |
| **5. Error Handling & Recovery** | `9.4/10` | Tự sửa cấu hình MCP drift và chặn symlink độc hại |
| **6. Architectural Depth & Leverage** | `9.5/10` | `hybrid` packaging theo ADR-007 |
| **7. UX Empathy** | `9.6/10` | Mở app dùng ngay trên macOS/Windows/Linux không cần CLI |
| **TOTAL AVERAGE** | **9.50/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 12.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | [refactor-story] Bổ sung AC-04 Tauri v2 HMAC Preload Trust Boundary & Packaged Native Module / Updater Harness theo Master Plan | /refactor-story |
| 2026-11-20 | 3.0.0 | [refactor-story] Nâng cấp hybrid packaging theo ADR-007 (nhúng Split DE+DR pack từ 08.5); cập nhật dependencies ["12.1", "08.5"] | /refactor-story |
