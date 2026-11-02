# Epic 01 Evaluation Report (`evaluation-report.md`)

- **Epic ID:** `Epic-01`
- **Epic Title:** `Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry`
- **Value Stream:** `VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine`
- **Execution Mode:** **Deep Mode** (`Complexity Score CS = 6.0` — Foundational Rust/TS Monorepo, 16-Table SQLite WAL Genesis & 9-Spec Synthesis Daemon)
- **Evaluation Integrity:** `VERIFIED`
- **Epic Readiness:** `READY`
- **Story Creation Envelope:** `max_phase: "IMPLEMENT"`, `allowed_story_ids: ["01.1", "01.2", "01.3"]`

---

## 1. Executive Summary & Architectural Verdict
`Epic 01` là nền móng cấp Kernel (`Stream-1`, Wave 1, In-Degree `0`) của toàn bộ hệ điều hành thiết kế **UI/UX OS (FlowForge Harness OS)**. Qua kiểm chứng **Architecture Coherence Check (`PASS`, 0 conflicts)**, **2 vòng phản biện Party Mode (5 vai trò)** và **4-Quadrant Unknowns Review**, `Epic 01` đạt trạng thái **`READY`** với phạm vi triển khai rõ ràng gồm 3 Stories tuần tự (`01.1` $\rightarrow$ `01.2` $\rightarrow$ `01.3`):
1. **Story 01.1:** Khởi tạo Hybrid Monorepo (11 Rust Crates + pnpm) & `crates/domain` Type Sync (`ts-rs` $\rightarrow$ `packages/shared-types`).
2. **Story 01.2:** Xây dựng `crates/spec-registry` với `rusqlite` SQLite WAL (16 Bảng) & 2-Way File Sync (`notify` watcher + SHA-256 `StaleFile` OCC).
3. **Story 01.3:** Xây dựng `crates/harness-daemon` (Axum + Tokio) & 9-Spec Synthesis Pipeline (`CONFIRMED`/`INFERRED`/`MISSING` provenance + Cross-Project Brand Reuse).

---

## 2. Plugin Decomposition Strategy

- **Plugin Boundary Applicability:** `NOT_APPLICABLE` (`delivery: "core"`)
- **Reference Artifact:** [plugin-boundary-review.md](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-01.%20Rust%20Core%20Harness,%209-Spec%20Registry%20&%20Project%20Constraint%20Engine/Epic-01/plugin-boundary-review.md)
- **Agent Surface Blueprint:** [agent-surface-map.md](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-01.%20Rust%20Core%20Harness,%209-Spec%20Registry%20&%20Project%20Constraint%20Engine/Epic-01/agent-surface-map.md)

| Component ID | Trust Owner | Packaging | Agent Exposure | Decomposition Rationale |
| :--- | :---: | :---: | :---: | :--- |
| `crates/domain` | `KERNEL` | `KERNEL_CORE` | `NOT_NEEDED` | Định nghĩa kiểu dữ liệu gốc và cầu nối `ts-rs` dùng chung cho cả 11 Rust crates và React 19 UI Shell. |
| `crates/spec-registry` | `KERNEL` | `KERNEL_CORE` | `AGENT_FACING` (`TOOL`) | Quản lý cơ sở dữ liệu nhúng 16 bảng SQLite WAL (bao gồm cả bảng `SKILL_PLUGINS` làm nền cho `Epic 08`) và đồng bộ file 2 chiều có kiểm chứng `StaleFile` OCC. |
| `crates/harness-daemon` | `KERNEL` | `KERNEL_CORE` | `AGENT_PROPOSE_ONLY` (`PLAYBOOK`) | Máy chủ điều phối nội bộ (`127.0.0.1:4170`) cung cấp API quản lý dự án và tổng hợp 9 Foundational Specs có gắn nhãn provenance. |

---

## 3. Actionable Implementation Plan (Stories `01.1`, `01.2`, `01.3`)

### 3.1 Story 01.1 — Khởi tạo Hybrid Monorepo (11 Rust Crates + pnpm) & `crates/domain` Type Sync
- **Mục tiêu:** Thiết lập cấu trúc thư mục chuẩn của dự án tại `/Users/hatrang20061988/Desktop/AI Project/uxui-os` gồm Cargo Workspace 11 crates và pnpm workspace (`packages/shared-types`, `packages/ui-shell`), hoàn thiện `crates/domain` và xuất tự động TypeScript interfaces + Zod schemas.
- **Deliverables cốt lõi:**
  - Root `Cargo.toml` khai báo 11 thành viên (`crates/domain`, `crates/spec-registry`, `crates/constraint-engine`, `crates/flow-graph`, `crates/html-forge`, `crates/presentation-cms`, `crates/plugin-host`, `crates/rea-bridge`, `crates/bot-fleet`, `crates/mcp-gateway`, `crates/harness-daemon`).
  - `crates/domain/src/lib.rs` định nghĩa `Project`, `CanonicalSpecKey` (whitelist 9 specs), `ProjectSpec`, `ProvenanceStatus` (`Confirmed`, `Inferred`, `Missing`), `ConstraintProfile`, `HarnessError` (`StaleFile`, `InvalidSpecKey`, `ProjectNotFound`).
  - `packages/shared-types/src/index.ts` đồng bộ kiểu từ `ts-rs` kèm bài kiểm tra round-trip JSON serde $\leftrightarrow$ TypeScript.

### 3.2 Story 01.2 — Xây dựng `crates/spec-registry` với `rusqlite` SQLite WAL (16 Bảng) & 2-Way File Sync
- **Mục tiêu:** Khởi tạo 16 bảng SQLite ở chế độ WAL (`PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;`), cài đặt đồng bộ 2 chiều giữa `product/specs/*.md` và bảng `PROJECT_SPECS` / `SPEC_REVISIONS`, và chặn ghi đè bằng `expected_sha256`.
- **Deliverables cốt lõi:**
  - Migration `V1__initial_16_tables.sql` tạo đủ 16 bảng (`PROJECTS`, `PROJECT_SPECS`, `SPEC_REVISIONS`, `CONSTRAINT_PROFILES`, `CONSTRAINT_RULES`, `FLOW_GRAPHS`, `FLOW_NODES`, `FLOW_EDGES`, `SCREEN_ARTIFACTS`, `PRESENTATION_DECKS`, `DECK_SLIDES`, `WORDING_SLOTS`, `SKILL_PLUGINS`, `MODEL_TOURNAMENTS`, `BOT_JOBS`, `GATE_REPORTS`).
  - Cơ chế **Hash-Gated Watcher Suppression** và **Atomic Temp-File Rename** (`.tmp.<uuid>` $\rightarrow$ `sync_all()` $\rightarrow$ `rename()`) kèm debounce `50ms` chống đọc file rỗng (`0 bytes`) khi trình soạn thảo bên ngoài đang lưu.
  - Kiểm chứng `StaleFile` OCC trả về lỗi `409 Conflict` khi `expected_sha256 != actual_sha256`.

### 3.3 Story 01.3 — Xây dựng `crates/harness-daemon` (Axum + Tokio) & 9-Spec Synthesis Pipeline
- **Mục tiêu:** Dựng Axum HTTP + WebSocket daemon (`127.0.0.1:4170` với hỗ trợ dynamic port discovery `.uxui-os/daemon.lock`), cung cấp REST API quản lý Project & 9 Foundational Specs, hỗ trợ tổng hợp 9 Specs từ Product Brief có gắn nhãn `[CONFIRMED]`/`[INFERRED]`/`[MISSING]` và kế thừa Brand Guideline xuyên dự án (`FR-01`, `FR-11`, `FR-12`).
- **Deliverables cốt lõi:**
  - Endpoints: `GET /api/v1/health`, `POST /api/v1/projects`, `GET /api/v1/projects/:id/specs`, `PUT /api/v1/projects/:id/specs/:spec_key`, `POST /api/v1/projects/:id/specs/synthesize`.
  - Hỗ trợ `inherit_brand_from_project_id` sao chép có truy vết phiên bản (`source_project_id`, `source_revision_sha256`) cho `04-design-direction.md` và `08-brand-guidelines.md`.

---

## 4. Edge-Case Guardian Audit Seal (`Step 6.5`)

| FMEA ID | Failure Mode / Boundary Condition | RPN | Mitigation Bound in Epic 01 | Status |
| :--- | :--- | :---: | :--- | :---: |
| **EC-E01-01** | Infinite `notify` watcher echo loop between `product/specs/*.md` and SQLite `PROJECT_SPECS` | `210` $\rightarrow$ `24` | Hash-Gated Watcher Suppression (`disk_sha256 == db_sha256` $\rightarrow$ `NO-OP`) + Atomic `.tmp` rename | `MITIGATED` |
| **EC-E01-02** | Concurrent Human + Bot write race (`Lost Update`) on the same spec file | `240` $\rightarrow$ `20` | Mandatory `expected_sha256` OCC check returning `409 STALE_FILE` | `MITIGATED` |
| **EC-E01-03** | Path traversal via crafted `spec_key` (`../../etc/passwd`) or symlink escape | `270` $\rightarrow$ `18` | Strict 9-variant `CanonicalSpecKey` enum + `canonicalize()` workspace jail | `MITIGATED` |
| **EC-E01-04** | Transient `0-byte` truncate event from external IDE save wiping SQLite spec content | `180` $\rightarrow$ `20` | `50ms` debounce re-read on sudden `0-byte` file change + explicit delete flag requirement | `MITIGATED` |
| **EC-E01-05** | `SQLITE_BUSY` under multi-bot parallel reads/writes | `160` $\rightarrow$ `16` | Mandatory `PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;` on every connection | `MITIGATED` |

- **Edge-Case Guardian Verdict:** `APPROVED (ZERO_OPEN_HIGH_RISKS)`
