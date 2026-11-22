---
type: I-Wish Retrospective
epic_id: "Epic-01"
status: completed
date: "2026-11-20"
---

# Epic 01 Retrospective — Cargo Workspace, Domain Contracts (`ts-rs`) & 16-Table SQLite Registry

## 1. Outcome Summary (`3/3` Stories `completed` in `sprint-status.yaml`)
- **Story 01.1:** Khởi tạo Cargo workspace (11 Rust crates) & `pnpm-workspace.yaml` (`packages/shared-types`, `packages/ui-shell`), định nghĩa domain structs (`DesignIntentV1`, `CanonicalSpecKey`, `ProvenanceStatus`, `ProjectConstraintProfile`, `ScreenState`, `Transition`, `PresentationDeck`, `SkillPlugin`, `FleetTask`, `HarnessError`) kèm xuất `.d.ts` tự động qua `ts-rs`.
- **Story 01.2:** Xây dựng `crates/spec-registry` trên `rusqlite` SQLite WAL (16 bảng ban đầu) với đồng bộ 2 chiều `notify` + Hash-Gated Watcher Suppression + `StaleFile` SHA-256 OCC.
- **Story 01.3:** Xây dựng `crates/harness-daemon` (Axum + Tokio) trên `127.0.0.1` với dynamic port discovery (`.cowok-design/daemon.lock`), REST API quản lý Project & 9 Foundational Specs, cùng In-Process Guarded Vite BFF & SDK.

## 2. Verification Note
- Tài liệu tổng kết này đồng bộ trạng thái `completed` đã được xác nhận cho `Epic-01` (`Story 01.1`, `Story 01.2`, `Story 01.3`) trong `sprint-status.yaml` và `epic.md`.
