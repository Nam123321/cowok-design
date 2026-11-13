---
type: I-Wish Story
id: "01.1"
title: "Story 01.1: Khởi tạo Hybrid Monorepo (11 Rust Crates + pnpm) & crates/domain Type Sync"
description: "Thiết lập Cargo.toml workspace (11 crates) và pnpm-workspace.yaml; định nghĩa structs cho DesignIntentV1, CanonicalSpecKey (9 Foundational Specs), ProvenanceStatus, ProjectConstraintProfile, ScreenState, Transition, PresentationDeck, SkillPlugin, FleetTask, HarnessError kèm tự động xuất .d.ts qua ts-rs và kiểm chứng round-trip JSON."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/Story-01.1/story.md"
status: completed
risk_tier: 2
delivery: core
timestamp: "2026-11-02T14:22:00+07:00"
tags:
  - "story-01.1"
  - "epic-01"
  - "uxui-os"
  - "rust-day-1"
  - "ts-rs"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-01/evaluation-report.md"
dependencies: []
provides_apis:
  - "internal:epic-01/story-01.1"
  - "crate:domain"
  - "pkg:@cowok-design/shared-types"
consumes_apis: []
---

# Story 01.1: Khởi tạo Hybrid Monorepo (11 Rust Crates + pnpm) & crates/domain Type Sync

- **FR Covered:** [FR-01: Project Bootstrap from Product Brief or Seeded Repo](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md), [FR-11: 9 Foundational Project Context Specs Synthesis](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 01: Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry](file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-01/epic.md)
- **Status:** `completed`
- **Risk Tier:** `2`

## 1. Story Description & Context
Thiết lập cấu trúc monorepo Hybrid Rust + React 19 ngay từ Ngày 1 theo **ADR-001 / TDR-004** gồm `Cargo.toml` workspace (khai báo đầy đủ 11 Rust crates: `crates/domain`, `crates/spec-registry`, `crates/constraint-engine`, `crates/flow-graph`, `crates/html-forge`, `crates/presentation-cms`, `crates/plugin-host`, `crates/rea-bridge`, `crates/bot-fleet`, `crates/mcp-gateway`, `crates/harness-daemon`) và `pnpm-workspace.yaml` (`packages/shared-types`, `packages/ui-shell`, `packages/builtin-skills`). Định nghĩa toàn bộ hợp đồng kiểu dữ liệu cốt lõi trong `crates/domain` kèm cơ chế xuất tự động 100% sang TypeScript (`@cowok-design/shared-types`) và Zod schemas đối xứng qua `ts-rs`, nghiêm cấm symlink route alias và lệch casing/regex giữa Rust và TypeScript.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4.5` (Scope=1, Data=1, Integration=1, UI=0.5, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 4.5 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ Story).

## 3. Acceptance Criteria (AC)
- **AC-01 (11-Crate Cargo Workspace & pnpm Monorepo Topology):** Khởi tạo thành công `Cargo.toml` workspace với đầy đủ 11 crate manifests tuân thủ đồ thị phụ thuộc DAG (trong đó `crates/domain` ở tầng đáy không phụ thuộc crate nội bộ nào; các crate chuẩn hóa tối thiểu interface/health contract thực, không dùng symlink route directory) cùng `pnpm-workspace.yaml` kết nối `packages/shared-types`, `packages/ui-shell` và `packages/builtin-skills`.
- **AC-02 (`crates/domain` Canonical Structs, `CanonicalSpecKey` & Provenance Contract):** Định nghĩa đầy đủ các struct/enum cốt lõi (`Project`, `CanonicalSpecKey` cố định đúng 9 khóa `01-product-overview.md` .. `09-data-and-copy.md`, `ProjectSpec`, `ProvenanceStatus` wire `CONFIRMED`/`INFERRED`/`MISSING`, `SpecProvenanceSummary`, `ConstraintProfile`, `FlowGraph`, `PresentationDeck`, `SkillPlugin`, `BotJob`, `GateReport`, và `HarnessError` chuẩn hóa `STALE_FILE`, `INVALID_SPEC_KEY`, `PROJECT_NOT_FOUND`, dùng thống nhất `SafeJsInteger` / `Uuid`).
- **AC-03 (`[EDGE-CASE]` `EC-E01-10` — `ts-rs` Deterministic Codegen, 100% Zod Parity & Single Casing/Hash Regex):** Tự động sinh toàn bộ `.ts` bindings từ `crates/domain` sang `packages/shared-types/src/generated/` (bao gồm `TaskHorizon`, `ContextSlice`, `TokenBudget` và `ModelAdapter` domain types), cung cấp Zod schemas đối xứng cho **100% `ts-rs` exported types** (không shadow `HarnessError`, thống nhất `ConstraintSeverity` casing, `SpecRevision.content_hash` chấp nhận cả `sha256:` và `sha256-`, đồng bộ `dist/` build sạch, và đồng bộ `prisma/schema.prisma` DDR mirror).
- **AC-04 (`[EDGE-CASE]` `EC-E01-01` & `KU-01.1` — `ts-rs` Tagged Enum & `MAX_SAFE_INTEGER` Round-Trip Guard):** Ngăn chặn lệch kiểu (Type Drift) và lỗi tràn số nguyên 64-bit (`u64`/`i64`/`usize` vs JS `MAX_SAFE_INTEGER`) hoặc sai khác định dạng enum phân biệt (`#[serde(tag = "kind", content = "payload")]`) giữa Rust `serde_json` và TypeScript Zod (`SafeJsIntegerSchema`) thông qua bài kiểm thử round-trip JSON tự động.
- **AC-05 (`[EDGE-CASE]` `EC-E01-03` — Strict 9-Spec Key Whitelist Deserialization Guard):** Từ chối ngay ở tầng giải tuần tự hóa (`serde` / `FromStr` / Zod) mọi khóa `spec_key` nằm ngoài 9 giá trị của `CanonicalSpecKey` (bao gồm các chuỗi chứa `../`, `/`, `\`, null byte hoặc tên file tùy ý), trả về lỗi tường minh `HarnessError::InvalidSpecKey`.

## 4. Tasks
- [x] **Task 01.1.1**: Khởi tạo `Cargo.toml` workspace (11 crates) và `pnpm-workspace.yaml` (`packages/shared-types`, `packages/ui-shell`, `packages/builtin-skills`) đúng cấu trúc kiến trúc (thỏa mãn `AC-01`).
- [x] **Task 01.1.2**: Cài đặt các domain structs/enums cốt lõi, `CanonicalSpecKey` (`01-product-overview.md` .. `09-data-and-copy.md`), `ProvenanceStatus` (`CONFIRMED`/`INFERRED`/`MISSING`), `ConstraintSeverity` thống nhất và `HarnessError` trong `crates/domain` (thỏa mãn `AC-02`).
- [x] **Task 01.1.3**: Tích hợp `ts-rs` xuất toàn bộ kiểu (`export_types.rs` đầy đủ) sang `@cowok-design/shared-types`, phủ 100% Zod schemas đối xứng (chấp nhận `sha256:`/`sha256-`, `SafeJsIntegerSchema` upper bounds, build `dist/`), và đồng bộ `prisma/schema.prisma` (thỏa mãn `AC-03`).
- [x] **Task 01.1.4**: Viết bộ kiểm thử biên `[EDGE-CASE]` kiểm chứng round-trip JSON cho tagged enums, giới hạn `MAX_SAFE_INTEGER` (`KU-01.1`, `EC-E01-10`), và chặn mọi `spec_key` ngoài whitelist (`EC-E01-03`) (thỏa mãn `AC-04`, `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 01.1.1` | `cargo check --workspace && pnpm -r typecheck` |
| **AC-02** | `Task 01.1.2` | Unit Tests in `crates/domain` (`cargo test -p domain`) |
| **AC-03 (`[EDGE-CASE]`)** | `Task 01.1.3` | `cargo test -p domain --features ts` + `@cowok-design/shared-types` 100% Zod parity & build |
| **AC-04 (`[EDGE-CASE]`)** | `Task 01.1.4` | Cross-runtime JSON round-trip test (`KU-01.1`, `EC-E01-10`) |
| **AC-05 (`[EDGE-CASE]`)** | `Task 01.1.4` | Path traversal & invalid `spec_key` rejection test (`EC-E01-03`) |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `[]`

### Impacts
- Cung cấp hợp đồng kiểu `crates/domain` và `@cowok-design/shared-types` cho `Story 01.2`, `Story 01.3` và toàn bộ 11 Epics hạ nguồn.

### Consumes
- Tiêu thụ các đặc tả kiến trúc nền tảng từ `2.1. product-brief-or-prd.md`, `2.2. database-spec.md`, và `2.5. architecture.md`.

### Shared Entities
- Định nghĩa cấu trúc Rust/TS ánh xạ trực tiếp tới 16 bảng trong `2.2. database-spec.md` (`PROJECTS`, `PROJECT_SPECS`, `SPEC_REVISIONS`, `CONSTRAINT_PROFILES`, `CONSTRAINT_RULES`, `FLOW_GRAPHS`, `FLOW_NODES`, `FLOW_EDGES`, `SCREEN_ARTIFACTS`, `PRESENTATION_DECKS`, `DECK_SLIDES`, `WORDING_SLOTS`, `SKILL_PLUGINS`, `MODEL_TOURNAMENTS`, `BOT_JOBS`, `GATE_REPORTS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các không gian làm việc của dự án (`Product UI/UX Workspace`, `Marketing HTML Deck Studio`, `Executive Report Builder`, và `Agent Fleet Console`).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Đáp ứng đầy đủ `FR-01, FR-11` và kết quả Party Mode Epic 01 |
| **2. Data Integrity & State** | `9.4/10` | Chuẩn hóa `ProvenanceStatus` (`CONFIRMED`/`INFERRED`/`MISSING`) và `StaleFile` contract |
| **3. Security & Validation** | `9.6/10` | Khóa cứng `CanonicalSpecKey` (9 biến thể) chặn Path Traversal từ gốc |
| **4. Performance & Scalability** | `9.5/10` | Zero-cost Rust abstractions & deterministic `ts-rs` codegen |
| **5. Error Handling & Recovery** | `9.4/10` | Chuẩn hóa `HarnessError` có cấu trúc tường minh |
| **6. Architectural Depth & Leverage** | `9.5/10` | Thiết lập ranh giới 11 crates sạch sẽ ngay từ Ngày 1 |
| **7. UX Empathy** | `9.4/10` | Cung cấp provenance metadata rõ ràng cho cả Human và `cowok` / `advisor` |
| **TOTAL AVERAGE** | **9.47/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 01.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-02 | 1.1.0 | Bổ sung AC-04, AC-05 và các Edge Cases (`KU-01.1`, `EC-E01-03`, `BS-01.2`) từ kết quả `/evaluate-epic` Epic 01 | cowok Orchestrator |
| 2026-11-13 | 1.2.0 | Reconciled package names (`@cowok-design/shared-types`), 9 canonical keys, unified `ConstraintSeverity`, `sha256:`/`sha256-`, 100% Zod coverage (`EC-E01-10`), and TDR-004 | cowok Orchestrator |
