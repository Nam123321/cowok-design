---
type: I-Wish Epic
id: "Epic-14"
title: "Epic 14: Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal (cowok-design)"
status: in-progress
track: "Stream-6"
wave: "Wave-B..Wave-D"
provides:
  - "In-App Non-Tech Support & Request Drawer (No-GitHub) + Zero-PII Scrubber + SQLite Telemetry Outbox (14.1)"
  - "Cloud Ingestion API & SuperAdmin Portal MVP (OIDC/MFA/RBAC, 2-Way Helpdesk, Feature Clustering, Funnel & Crash Center) (14.2)"
  - "4-Channel Ed25519 Signed Rollout, Global Skill Quarantine Advisory, Offline Licenses & Cowok.ai Federation (14.3)"
consumes: ["Epic 04", "Epic 08", "Epic 11"]
links_to:
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
---

# Epic 14: Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal (`cowok-design`)

## 1. Overview / Executive Summary
- **Value Stream:** `VS-07. Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal`
- **FR Covered:** `FR-37`, `FR-38`
- **Architecture Rule (`ADR-008`):** Strict 2-Plane Isolation — client binaries (`cowok-design` CLI, Tauri Desktop, 5-Host Plugin Packs, Docker) ship ONLY the lightweight offline-safe **Client Edge** (`Story 14.1`), while the **Cloud Control Plane & SuperAdmin Portal** (`admin.cowok.design`, `Story 14.2` & `14.3`) deploys separately with OIDC + MFA + 4-role RBAC and Ed25519 downward signing.

## 2. Stories in Epic 14

| Story ID | Title | Status | Risk Tier | Dependencies |
| :--- | :--- | :---: | :---: | :--- |
| [Story-14.1](./Story-14.1/story.md) | Client Edge — In-App Non-Tech Support/Request Drawer (No-GitHub), Zero-PII Diagnostic Scrubber & Multi-Channel Telemetry Outbox | `completed` | `2` | `['04.6', '08.5']` |
| [Story-14.2](./Story-14.2/story.md) | Cloud Ingestion API & SuperAdmin Portal MVP — OIDC/MFA/RBAC, 2-Way Non-Tech Helpdesk, Feature Request Clustering, Activation Funnel & Crash Center | `backlog` | `3` | `['14.1']` |
| [Story-14.3](./Story-14.3/story.md) | 4-Channel Distribution Ops — Canary Rollout, Ed25519 Signed Updater & Skill Quarantine Advisory, Offline Licenses & Cowok.ai Federation | `backlog` | `3` | `['14.2', '08.6']` |

## 3. QA Simulator Guardian Scorecard (7-Row Hybrid Scorecard)

| Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Phủ đủ Non-Tech Helpdesk 2 chiều, Activation Funnel, Crash Center & Distribution Ops |
| **2. Data Integrity & State** | `9.5/10` | SQLite Outbox chống mất sự kiện khi offline; Immutable Audit Log trên SuperAdmin |
| **3. Security & Validation** | `9.7/10` | Tách cứng 2 mặt phẳng, Zero-PII Scrubber phía client, OIDC+MFA+RBAC và chữ ký Ed25519 |
| **4. Performance & Scalability** | `9.5/10` | Batch flush bất đồng bộ không block UI local; tái sử dụng PostHog/Sentry cho analytics/crash |
| **5. Error Handling & Recovery** | `9.4/10` | Graceful offline degradation & Air-Gapped Enterprise mode mặc định tắt outbound |
| **6. Architectural Depth & Leverage** | `9.6/10` | Phục vụ đồng bộ cả 4 kênh phân phối + 5 nền tảng Plugin (`Cowok.ai`, `Claude`, `Codex`, `Antigravity`, `Cursor`) |
| **7. UX Empathy** | `9.7/10` | Non-tech báo lỗi/gửi yêu cầu trong 1 chạm ngay trên Workbench, hoàn toàn không cần GitHub |
| **TOTAL AVERAGE** | **9.56/10** | **PASSED (>= 8.5/10)** |
