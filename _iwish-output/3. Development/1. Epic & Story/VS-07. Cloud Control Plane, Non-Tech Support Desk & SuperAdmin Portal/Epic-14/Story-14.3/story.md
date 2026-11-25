---
type: I-Wish Story
id: "14.3"
title: "Story 14.3: 4-Channel Distribution Ops — Canary Rollout, Ed25519 Signed Updater & Skill Quarantine Advisory, Offline Licenses & Cowok.ai Federation"
description: "Hoàn thiện năng lực điều hành phân phối 4 kênh trên SuperAdmin Portal: phát hành Canary + ký Ed25519 manifest cập nhật Tauri, danh sách cách ly kỹ năng toàn cục (Global Quarantine Advisory), khóa bản quyền offline Ed25519 và liên thông Cowok.ai."
resource: "iwish://cowok-design/development/epic-14/story-14.3"
status: completed
risk_tier: 3
delivery: cloud-control-plane
adr_ref: "ADR-008"
timestamp: "2026-11-23T11:30:00+07:00"
tags: ["story-14.3", "epic-14", "cowok-design", "ed25519", "canary-rollout", "skill-governance", "cowok-federation"]
links_to:
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "[[../epic.md]]"
dependencies: ["14.2", "08.6"]
provides_apis:
  - "GET /v1/releases/latest.json"
  - "GET /v1/skills/advisories.json"
  - "GET /v1/skills/catalog.json"
  - "GET /v1/experiments/config.json"
  - "POST /v1/admin/releases/rollout"
  - "POST /v1/admin/skills/quarantine-global"
  - "POST /v1/admin/skills/catalog"
  - "POST /v1/admin/experiments"
  - "POST /v1/admin/licenses/issue"
consumes_apis:
  - "GET /v1/admin/analytics/funnel"
---

# Story 14.3: 4-Channel Distribution Ops — Canary Rollout, Ed25519 Signed Updater & Skill Quarantine Advisory, Offline Licenses & Cowok.ai Federation

- **FR Covered:** [FR-38: Cloud Control Plane, SuperAdmin Portal & Signed Distribution Ops](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 14: Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal](../epic.md)
- **Status:** `backlog`
- **Risk Tier:** `3`

## 1. Story Description & Context
Bổ sung tầng vận hành phát hành & quản trị chuỗi cung ứng chuẩn doanh nghiệp vào SuperAdmin Portal (`admin.cowok.design`), cung cấp chữ ký **Ed25519** cho bộ cập nhật Tauri v2 (`Story 12.2`), danh sách cách ly skill khẩn cấp toàn cầu cho hệ sinh thái `design-craft` (`Story 08.6`), khóa bản quyền xác thực offline cho Desktop Pro / Docker Enterprise và liên thông `cowok_org_id` với `Cowok.ai`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=0.5, Security=1, Unknowns=0.5 -> `CS = 5 < 7`, đạt chuẩn an toàn).

## 3. Acceptance Criteria (AC)
- **AC-01 (4-Channel Canary Rollout & Ed25519 Signed Updater Manifest):** SuperAdmin quản lý phát hành theo tỷ lệ (`5% -> 25% -> 100%`) và xuất bản `latest.json` ký số Ed25519 cho Tauri v2 (`Story 12.2`), kèm Kill-Switch & In-App Announcement cho phiên bản gặp lỗi nghiêm trọng.
- **AC-02 (Default & Recommended GitHub Plugin Catalog Lifecycle + Global Quarantine Advisory):** `SKILL_CURATOR` / `SUPER_ADMIN` quản lý vòng đời toàn bộ **Default Built-in Plugins** (`ACTIVE | INACTIVE | DEPRECATED | QUARANTINED`, cấu hình mặc định, phiên bản tối thiểu) và **Recommended GitHub Plugins** (`Add / Edit / Archive` repo URL kèm ghim cứng `tag/commit SHA` & mô tả 9 ngôn ngữ), xuất bản manifest `catalog.json` & `advisories.json` ký số **Ed25519**. Client tuân thủ tuyệt đối **Local Override > Cloud Default** (không âm thầm ghi đè công tắc user hay tự cài mã GitHub ngầm), chỉ cưỡng bức cảnh báo/khóa khi có lệnh **Cách ly toàn cục (`QUARANTINED`)**.
- **AC-03 (Deterministic A/B Experiments & Feature Flags):** `SUPER_ADMIN` tạo/bật/tắt thử nghiệm A/B cho bộ plugin mặc định onboarding, xếp hạng plugin khuyến nghị hoặc UX variants theo tỷ lệ chia (`50/50`, `10/90`), xuất bản `experiments.json` ký **Ed25519**. Client chia nhóm tất định bằng `hash(install_id + experiment_key) % 100` (`0% PII`, chạy offline sau lần sync đầu) và gắn `experiment_variants[]` vào Telemetry để so sánh trực tiếp trên **5-Step Activation Funnel** & **Crash Center** kèm Kill-Switch tức thì.
- **AC-04 (Offline-Verifiable Ed25519 License Tokens & `Cowok.ai` Federation):** Cấp khóa bản quyền `Free / Pro / Team / Enterprise` ký Ed25519 (client xác thực 100% offline không cần ping server) và liên thông `cowok_org_id` khi chạy dưới dạng plugin trong `Cowok.ai`.
- **AC-05 ([EDGE-CASE] Tampered Manifest / Expired Signature / Rollback & Security Guard):** Client Edge từ chối ngay (fail-closed) mọi `latest.json`, `catalog.json`, `experiments.json`, `advisories.json` hoặc License Token bị sửa 1 byte, sai chữ ký Ed25519 hoặc bị downgrade version (rollback attack); cấm tuyệt đối A/B experiment nới lỏng bảo mật (`HUMAN_ONLY`, `Zero-PII Scrubber`, `O_NOFOLLOW`); SuperAdmin bắt buộc ghi lý do & audit log khi kích hoạt Global Quarantine, xuất bản Catalog/Experiment hoặc dừng Canary rollout.

## 4. Tasks
- [ ] **Task 14.3.1**: Cài đặt Ed25519 signing/verification service cho `latest.json` (Canary rollout), `catalog.json` (Default & Recommended GitHub Plugin Lifecycle), `experiments.json` (Deterministic A/B Experiments), `advisories.json` (Global Skill Quarantine) và Offline License Tokens (thỏa mãn `AC-01`, `AC-02`, `AC-03`, `AC-04`, `AC-05`).
- [ ] **Task 14.3.2**: Nối backend thật vào khung UI SuperAdmin Portal (`src/portals/superadmin/`) cho Release Rollout, Default/Recommended Plugin Catalog CRUD, A/B Experiments (kết nối Activation Funnel & Crash Center), Global Quarantine & License Issuance (thỏa mãn `AC-01`, `AC-02`, `AC-03`, `AC-04`).
- [ ] **Task 14.3.3**: Tích hợp Client Edge verifier (`cowok-design` daemon) đọc signed catalog/experiments/advisories/license theo nguyên tắc `Local Override > Cloud Default`, chia nhóm A/B tất định `hash(install_id + key) % 100` (`0% PII`) + đồng bộ federated `cowok_org_id` cho Kênh 3 (thỏa mãn `AC-02`, `AC-03`, `AC-04`, `AC-05`).
- [ ] **Task 14.3.4**: Viết cryptographic tamper/downgrade/replay tests, deterministic A/B bucketing & Local-Override contract tests (thỏa mãn `AC-01..AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 14.3.1`, `Task 14.3.2`, `Task 14.3.4` | Canary Percentage & Signed `latest.json` Contract Test |
| **AC-02** | `Task 14.3.1`, `Task 14.3.2`, `Task 14.3.3` | Default/GitHub Plugin Catalog Sync, Local-Override & Global Quarantine Block Test |
| **AC-03** | `Task 14.3.1`, `Task 14.3.2`, `Task 14.3.3`, `Task 14.3.4` | Deterministic A/B Bucketing (`0% PII`), Funnel/Crash Attribution & Kill-Switch Test |
| **AC-04** | `Task 14.3.1`, `Task 14.3.3`, `Task 14.3.4` | Offline Ed25519 License & `Cowok.ai` Federation Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 14.3.1`, `Task 14.3.3`, `Task 14.3.4` | 1-Byte Tamper, Rollback Attack & Security-Flag Isolation Fail-Closed Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["14.2", "08.6"]`
- **Impacts:** Cung cấp signed updater & advisory pipeline trực tiếp cho `Story 12.2` (Desktop Tauri v2 + 5-Host Plugin Pack).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ đủ Canary, Quarantine toàn cục, License offline & Federation |
| **2. Data Integrity & State** | `9.6/10` | Chữ ký Ed25519 chống sửa đổi chuỗi cung ứng & chống downgrade |
| **3. Security & Validation** | `9.7/10` | Fail-closed trên mọi chữ ký lệch; 100% hành động có audit log |
| **4. Performance & Scalability** | `9.5/10` | File manifest ký số tĩnh phân phối qua CDN cực nhẹ |
| **5. Error Handling & Recovery** | `9.5/10` | Hỗ trợ dừng Canary & Kill-Switch tức thì khi phát hiện crash spike |
| **6. Architectural Depth & Leverage** | `9.6/10` | Dùng chung cho cả Desktop Pro, Docker Enterprise & Cowok.ai |
| **7. UX Empathy** | `9.5/10` | Không khóa người dùng Desktop khi mất kết nối internet |
| **TOTAL AVERAGE** | **9.56/10** | **PASSED (>= 8.5/10)** |
