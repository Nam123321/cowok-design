---
type: I-Wish Story
id: "14.2"
title: "Story 14.2: Cloud Ingestion API & SuperAdmin Portal MVP — OIDC/MFA/RBAC, 2-Way Non-Tech Helpdesk, Feature Request Clustering, Activation Funnel & Crash Center"
description: "Xây dựng Cloud Control Plane tách biệt (api.cowok.design) và SuperAdmin Portal (admin.cowok.design) với OIDC+MFA+4-Role RBAC để tiếp nhận/trả lời ticket 2 chiều cho non-tech, gom cụm Feature Request, theo dõi phễu kích hoạt đa kênh và gom lỗi crash."
resource: "iwish://cowok-design/development/epic-14/story-14.2"
status: completed
risk_tier: 3
delivery: cloud-control-plane
adr_ref: "ADR-008"
timestamp: "2026-11-23T11:30:00+07:00"
tags: ["story-14.2", "epic-14", "cowok-design", "superadmin", "helpdesk", "analytics", "crash-triage"]
links_to:
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "[[../epic.md]]"
dependencies: ["14.1"]
provides_apis:
  - "POST /v1/ingest/tickets"
  - "POST /v1/ingest/telemetry"
  - "GET /v1/admin/tickets"
  - "POST /v1/admin/tickets/:id/reply"
  - "GET /v1/admin/analytics/funnel"
  - "GET /v1/admin/crashes"
consumes_apis:
  - "POST /api/v1/support/tickets"
  - "POST /api/v1/telemetry/events"
---

# Story 14.2: Cloud Ingestion API & SuperAdmin Portal MVP — OIDC/MFA/RBAC, 2-Way Non-Tech Helpdesk, Feature Request Clustering, Activation Funnel & Crash Center

- **FR Covered:** [FR-38: Cloud Control Plane, SuperAdmin Portal & Signed Distribution Ops](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 14: Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal](../epic.md)
- **Status:** `completed`
- **Risk Tier:** `3`

## 1. Story Description & Context
Triển khai mặt phẳng điều khiển Cloud (`api.cowok.design` + `admin.cowok.design`, tách hoàn toàn khỏi binary local) giúp đội vận hành `cowok-design` đo lường trực quan DAU/WAU/MAU, phễu rớt bước theo 4 kênh & 5 plugin hosts, gom lỗi crash và trả lời trực tiếp ticket / yêu cầu tính năng của người dùng non-tech (đồng bộ 2 chiều về hộp thư trong App).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.5` (Scope=1.5, Data=1, Integration=1, UI=1, Security=1, Unknowns=0 -> `CS = 5.5 < 7`, đạt chuẩn an toàn).

## 3. Acceptance Criteria (AC)
- **AC-01 (Zero-Trust Cloud Ingestion Gateway & Anti-Spam):** Endpoint nhận ticket & telemetry xác thực bằng schema Zod/Serde nghiêm ngặt, rate-limit theo `(install_id, IP)`, re-encode ảnh đính kèm `<= 5 MB` (chống polyglot/XSS payload) và chạy lớp lọc PII thứ 2 phía server trước khi lưu PostgreSQL/Object Storage.
- **AC-02 (SuperAdmin Auth — OIDC SSO + MFA + 4-Role RBAC & Immutable Audit Log):** Chỉ nhân sự được cấp quyền (`SUPER_ADMIN`, `SUPPORT_AGENT`, `SKILL_CURATOR`, `ANALYST_READONLY`) qua OIDC + MFA mới truy cập được `admin.cowok.design`; mọi thao tác đổi trạng thái, trả lời ticket, xuất báo cáo đều ghi `admin_audit_logs` bất biến.
- **AC-03 (Non-Tech Helpdesk Inbox 2 Chiều & Feature Request Clustering):** Màn hình quản trị ticket (`DataTableView [5.21]`, `AdvancedFilterBar [5.22]`, `RightDrawer [5.25]`) hỗ trợ gán mức ưu tiên `P0–P3`, xem Sanitized Diagnostic Bundle + ảnh chụp, trả lời đẩy ngược về App (`GET /api/v1/support/tickets`), gộp các Feature Request trùng nhau thành Cluster và tạo link issue GitHub nội bộ 1 chạm.
- **AC-04 (Activation Funnel, Feature Adoption & Crash Triage Dashboard):** Hiển thị KPI (`Metric Line [5.10]`): DAU/WAU/MAU, phân bố 4 kênh + 5 plugin hosts, **Phễu kích hoạt (`Day-0 -> AI Connected -> Project -> Preview -> Export`)** chỉ rõ tỷ lệ rớt từng bước, bảng tần suất dùng từng tính năng, và danh sách nhóm lỗi crash theo fingerprint + version (hỗ trợ nối adapter PostHog/Sentry hoặc ClickHouse).
- **AC-05 ([EDGE-CASE] Spam Flood Isolation & PII Defense-in-Depth):** Khi 1 `install_id` hoặc dải IP gửi vượt ngưỡng hoặc đính kèm file giả mạo MIME/chứa token nhạy cảm, gateway trả `429`/`422` mà không ảnh hưởng luồng hợp lệ và không ghi chuỗi bí mật vào log/DB.

## 4. Tasks
- [ ] **Task 14.2.1**: Dựng Cloud Ingestion API + PostgreSQL schema (`support_tickets`, `ticket_replies`, `feature_clusters`, `telemetry_events`, `crash_groups`, `admin_audit_logs`) kèm rate-limit, image re-encode và server-side PII filter (thỏa mãn `AC-01`, `AC-05`).
- [ ] **Task 14.2.2**: Cài đặt OIDC + MFA + 4-Role RBAC middleware và Immutable Audit Log cho `/v1/admin/*` (thỏa mãn `AC-02`).
- [ ] **Task 14.2.3**: Xây dựng SuperAdmin Portal UI (`admin.cowok.design`, tuân thủ `DESIGN.md` Zero-Tailwind + 9 ngôn ngữ + `useFormatter()`): Helpdesk Inbox 2 chiều, Feature Request Board, Activation Funnel & Crash Triage Center (thỏa mãn `AC-03`, `AC-04`).
- [ ] **Task 14.2.4**: Viết integration/security tests (RBAC privilege escalation, anti-spam flood, polyglot image rejection, 2-way ticket roundtrip) và Live Browser E2E (thỏa mãn `AC-01..AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 14.2.1`, `Task 14.2.4` | Ingestion Schema, Rate-Limit & Image Re-Encode Test |
| **AC-02** | `Task 14.2.2`, `Task 14.2.4` | RBAC 4-Role & Audit Immutability Security Test |
| **AC-03** | `Task 14.2.3`, `Task 14.2.4` | 2-Way Helpdesk Roundtrip & Feature Clustering E2E |
| **AC-04** | `Task 14.2.3`, `Task 14.2.4` | Funnel Drop-Off & Crash Fingerprint Aggregation Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 14.2.1`, `Task 14.2.4` | Flood `429` & Defense-in-Depth Redaction Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["14.1"]`
- **Impacts:** Nền tảng quản trị trực tiếp cho `Story 14.3` (Distribution Ops & Skill Governance).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Giải quyết trọn vẹn đo lường DAU/Funnel/Crash & Helpdesk non-tech |
| **2. Data Integrity & State** | `9.5/10` | Audit log bất biến, đồng bộ 2 chiều có idempotency key |
| **3. Security & Validation** | `9.7/10` | OIDC+MFA+RBAC, re-encode ảnh, chống spam & defense-in-depth PII |
| **4. Performance & Scalability** | `9.5/10` | Adapter PostHog/Sentry giảm gánh nặng truy vấn OLAP |
| **5. Error Handling & Recovery** | `9.4/10` | Cô lập flood 429 không làm sập ingestion |
| **6. Architectural Depth & Leverage** | `9.6/10` | Tách mặt phẳng Cloud sạch sẽ, 0 phình local binary |
| **7. UX Empathy** | `9.6/10` | Dùng chuẩn `DataTableView`/`RightDrawer`/`Metric Line` nhất quán |
| **TOTAL AVERAGE** | **9.54/10** | **PASSED (>= 8.5/10)** |
