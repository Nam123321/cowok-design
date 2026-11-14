---
type: I-Wish Story
id: "04.1"
title: "Story 04.1: Unified DualMainPanel Workbench, Multi-Format CompanionCanvasTab, True 2D SVG Bezier DAG Studio & HMAC/Preview Remediation"
description: "Xây dựng khung Workbench (src/) theo chuẩn tối giản cowok.ai (Zero Amber, 40px Shorten Chat rail PD-02 = A), <BrandIcon /> (143 SVGs), CompanionCanvasTab & Native 2D SVG Cubic Bezier DAG Studio (DagCanvas2D + crates/flow-graph + SQLite V3/V6); giữ kiểm thử hồi quy cho phần đã hạ cánh trên master (legacy_specs.rs VerifiedLegacyIngress + verify_anti_idor_with_caller, singleflight hydration), đồng thời xử lý dứt điểm delta còn lại: giải thể src/features/preview/* trùng lặp về CompanionLiveHtmlRightPane, khử mã hex #f59e0b còn sót trong DagCanvas2D.tsx/DagNodeHeaderAndPorts.tsx, chuẩn hóa enum fixture_ref (DEFAULT|LOADING|EMPTY|ERROR) và thêm cảnh báo Orphan ScreenState trước deploy-draft."
resource: "file:///Users/hatrang20061988/Desktop/AI Project/uxui-os/_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.1/story.md"
status: completed
risk_tier: 3
delivery: core
timestamp: "2026-11-14T08:45:00+07:00"
tags:
  - "story-04.1"
  - "epic-04"
  - "uxui-os"
  - "dag-canvas-2d"
  - "workbench-shell"
  - "security-remediation"
links_to:
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.5.%20architecture.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.2.%20database-spec.md"
  - "file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/epic-evaluations/Epic-04/evaluation-report.md"
dependencies: ["01.3", "03.2"]
provides_apis:
  - "internal:epic-04/story-04.1"
  - "GET /api/v1/projects/:id/flow/snapshot"
  - "POST /api/v1/projects/:id/flow/nodes"
  - "PATCH /api/v1/projects/:id/flow/nodes/positions"
  - "PATCH /api/v1/projects/:id/flow/nodes/:node_id"
  - "POST /api/v1/projects/:id/flow/deploy-draft"
consumes_apis:
  - "GET /api/v1/health"
  - "GET /api/v1/projects/:id/constraints"
---

# Story 04.1: Unified DualMainPanel Workbench, Multi-Format CompanionCanvasTab, True 2D SVG Bezier DAG Studio & HMAC/Preview Remediation

- **FR Covered:** [FR-02: Interactive Multi-Portal Flow Graph (ScreenState & Transition)](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, Onboarding Doctor & Persistent Node Execution](file:///Users/hatrang20061988/Desktop/AI%20Project/uxui-os/_iwish-output/3.%20Development/1.%20Epic%20&%20Story/VS-02.%20Flow-First%20Executable%20Design%20Workbench%20&%20SDI%20Engine/Epic-04/epic.md)
- **Status:** `completed`
- **Risk Tier:** `3`

## 1. Story Description & Context
Story 04.1 thiết lập bề mặt Workbench tối giản theo thiết kế `cowok.ai` và bộ máy đồ thị Native 2D SVG Cubic Bezier `DagCanvas2D` nối thẳng với Rust `crates/flow-graph` & `crates/spec-registry` (`V3__flow_workbench_dag.sql` + mở rộng `V6/V7`). Theo kết quả rà soát đối chiếu mã nguồn trên `master`, phần vá HMAC trong `crates/harness-daemon/src/routes/legacy_specs.rs#L527-L553` (`VerifiedLegacyIngress` + `verify_anti_idor_with_caller`) và singleflight hydration (`data-flow-hydrated="true"`) đã hạ cánh làm baseline hồi quy; Story 04.1 chốt phần delta còn lại để khép kín hoàn chỉnh:
1. **Bảo toàn & kiểm thử hồi quy HMAC / Singleflight baseline:** Đưa kiểm thử hồi quy fail-closed cho `legacy_specs.rs` (`VerifiedLegacyIngress` + `verify_anti_idor_with_caller`) và singleflight hydration (`data-flow-hydrated="true"`) vào bộ test Story 04.1 thay cho công việc cài mới trùng lặp.
2. **Giải thể `src/features/preview/*` trùng lặp:** Xóa bỏ hoàn toàn các component trùng lặp ở `src/features/preview/*` (`SandboxedLivePreview.tsx`, `PreviewToolbar.tsx` dùng Tailwind) và quy về bề mặt duy nhất `CompanionLiveHtmlRightPane.tsx` (việc gia cố `DOMParser` CSP zero-network & CSPRNG `bootSecret` runtime được thi công thống nhất tại `Story 04.5`).
3. **Thanh tẩy hex cứng còn sót & chuẩn hóa SDK:** Xóa nốt mã màu `#f59e0b` còn sót trong `DagCanvas2D.tsx` (`L128`, `L168`) và `DagNodeHeaderAndPorts.tsx` (`L124`) cùng bất kỳ `#4ade80`/`#f87171` nào sang `src/styles/tokens.css`; dọn sạch `fetch()` thô trong workbench sang `src/api/flow-workbench.ts` / `src/api/constraints.ts`.
4. **Chuẩn hóa enum & Orphan `ScreenState` guard:** Thống nhất `fixture_ref` viết hoa **`DEFAULT | LOADING | EMPTY | ERROR`** (khớp bảng `screen_fixtures` và Zod `FixtureVariantSchema`, kèm bộ chuẩn hóa Rust chấp nhận dữ liệu cũ `'Default'`) và `viewport_preset` (`desktop-1440 | tablet-768 | mobile-390`), đồng thời thêm cảnh báo node `ScreenState` mồ côi trước khi `deploy-draft`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5.0` (Scope=1.0, Data=1.0, Integration=1.0, UI=1.0, Security=0.5, Unknowns=0.5 $\rightarrow$ `CS = 5.0 < 7`, đạt ngưỡng an toàn, không tách nhỏ story).

## 3. Acceptance Criteria (AC)
- **AC-01 (Unified DualMainPanel Workbench, 40px Shorten Rail & Zero-Amber CSS Tokens):** Xây dựng `WorkbenchShell` với `MainSidebar` thu gọn/mở rộng (`28x28px`), thanh `Shorten Chat` `40px` (`PD-02 = A`), bộ 4 Theme chuẩn (`Warm Paper`, `Focus Dim`, `Night Workspace`, `Monochrome Minimal`), Reactive i18n Store không tải lại trang, component `<BrandIcon />` (143 SVGs từ Story 03.2); đồng thời thay thế 100% mã màu cứng còn sót — bao gồm `#f59e0b` trong `DagCanvas2D.tsx` (`L128, L168`) & `DagNodeHeaderAndPorts.tsx` (`L124`) và mọi `#4ade80`/`#f87171` — bằng biến ngữ nghĩa trong `src/styles/tokens.css`.
- **AC-02 (Multi-Format `CompanionCanvasTab` & Native 2D SVG Cubic Bezier `DagCanvas2D` Studio):** Cài đặt `DagCanvas2D` vẽ trên mặt phẳng `(x, y)` bằng SVG Bezier bậc 3 với cổng `●` Input/Output, Cụm khung (`Cluster Frames`), 3 loại luồng (`Main`, `IF / ELSE`, `IF NOT / Fallback`), tách bạch lưu bố cục tự do (`● Saved` qua `layout_version`) với chỉnh sửa cấu trúc tạo `DRAFT` (`+ Node`, nối/đổi dây, đảo chiều `⇄`, sửa `Condition`/`Note` + `Undo/Redo`), thanh công cụ `.dag-right-vertical-dock` (Zoom `+`/`-`/`Reset`, `Fit View`, `Minimap`, pinch/Ctrl+Scroll/Space+Drag), đồng bộ OCC qua `GET /api/v1/projects/:id/flow/snapshot` + `crates/flow-graph` + SQLite `V3__flow_workbench_dag.sql`, và bảo đảm hồi quy singleflight hydration (`data-flow-hydrated="true"`).
- **AC-03 (`[EDGE-CASE]` Orphan ScreenState Guard, Uppercase Enum Normalization & Canonical `src/api/` Client SDK):** Phát hiện và cảnh báo nút `ScreenState` mồ côi (`0` cạnh nối vào/ra) trên `DagNodeCard` và `#dagDraftBar` trước khi `deploy-draft` (kèm 3-color DFS chặn chu trình vô hạn); chuẩn hóa enum `fixture_ref` sang dạng viết hoa **`DEFAULT | LOADING | EMPTY | ERROR`** (khớp `screen_fixtures` & Zod `FixtureVariantSchema`, kèm Rust normalizer chấp nhận an toàn các bản ghi `'Default'` cũ) và `viewport_preset` (`desktop-1440 | tablet-768 | mobile-390`) ở cả Rust/Zod/UI; và loại bỏ mọi `fetch()` thô trong `src/features/**`/`src/pages/**` về `src/api/flow-workbench.ts` & `src/api/constraints.ts`.
- **AC-04 (`[EDGE-CASE]` `EC-E04-03 / EC-E04-04`: Verified Legacy Ingress Regression & Physical Purge of Duplicate `src/features/preview/*`):** Bắt buộc có regression integration test xác nhận toàn bộ endpoint `legacy_specs.rs#L527-L553` (`/specs`, `/promote`, `/path-token`, `/surgical-patch`) thực thi `VerifiedLegacyIngress` + `verify_anti_idor_with_caller` fail-closed khi thiếu HMAC hoặc sai caller; đồng thời xóa bỏ vật lý 100% mã trùng lặp & Tailwind ở `src/features/preview/*` để `CompanionLiveHtmlRightPane.tsx` là bề mặt preview duy nhất (`sandbox="allow-scripts allow-forms"`, zero `allow-same-origin`, còn gia cố `DOMParser` CSP zero-network & `bootSecret` thực hiện tiếp ở `Story 04.5`).

## 4. Tasks
- [x] **Task 04.1.1**: Tinh chỉnh `WorkbenchShell`, `DualMainPanel`, `40px` Shorten Rail, 4 Theme Tokens & `<BrandIcon />` và thanh tẩy 100% raw hex `#f59e0b` (trong `DagCanvas2D.tsx`, `DagNodeHeaderAndPorts.tsx`) / `#4ade80` / `#f87171` về `tokens.css` (thỏa mãn `AC-01`).
- [x] **Task 04.1.2**: Hoàn thiện `CompanionCanvasTab` và Native 2D SVG Bezier `DagCanvas2D` (`GET /api/v1/projects/:id/flow/snapshot`, Cluster Frames, Layout vs `DRAFT` separation, `.dag-right-vertical-dock`, OCC persistence trên `V3__flow_workbench_dag.sql` và regression singleflight hydration `data-flow-hydrated="true"`) (thỏa mãn `AC-02`).
- [x] **Task 04.1.3**: Cài đặt cảnh báo Orphan ScreenState + 3-color DFS, chuẩn hóa enum `fixture_ref` (`DEFAULT|LOADING|EMPTY|ERROR` + normalizer ngược cho `'Default'`) / `viewport_preset` và di chuyển toàn bộ `fetch()` thô trong workbench/flow/chat sang `src/api/flow-workbench.ts` & `src/api/constraints.ts` (thỏa mãn `AC-03`).
- [x] **Task 04.1.4**: Bổ sung regression suite fail-closed xác minh `crates/harness-daemon/src/routes/legacy_specs.rs` (`VerifiedLegacyIngress` + `verify_anti_idor_with_caller` trên `master`) không bị thoái lui (thỏa mãn `AC-04`).
- [x] **Task 04.1.5**: Xóa bỏ vật lý và thanh tẩy 100% `src/features/preview/*` trùng lặp, hợp nhất đường dẫn xem trước duy nhất tại `CompanionLiveHtmlRightPane.tsx` (thỏa mãn `AC-04`).

## 5. AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Verification Method | Status |
| :--- | :--- | :--- | :--- | :--- |
| **AC-01** | Unified Workbench, 40px Rail & Zero-Amber CSS Tokens (`DagCanvas2D`/`DagNodeHeaderAndPorts`) | `Task 04.1.1` | CSS Token Linter (`zero #f59e0b`) & Component Test | completed |
| **AC-02** | CompanionCanvasTab, Snapshot Singleflight & Native 2D SVG Bezier `DagCanvas2D` | `Task 04.1.2` | SVG Bezier, Singleflight Hydration & V3 OCC Test | completed |
| **AC-03** | [EDGE-CASE] Orphan Node Guard, Uppercase Enums & `src/api/` SDK | `Task 04.1.3` | DFS Cycle/Orphan, Uppercase Enum & SDK AST Test | completed |
| **AC-04** | [EDGE-CASE] `legacy_specs.rs` Ingress Regression & `src/features/preview/*` Purge | `Task 04.1.4`, `Task 04.1.5` | Rust Ingress Regression Test & Physical Purge Check | completed |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["01.3", "03.2"]`

### Impacts
- Cung cấp vỏ Workbench chuẩn (`MainSidebar`, `DualMainPanel`, `CompanionCanvasTab`, `CompanionLiveHtmlRightPane`) và hệ thống DAG `DagCanvas2D` + `crates/harness-daemon` xác thực HMAC chuẩn cho `Story 04.2..04.5` và `Epic 05`, `Epic 06`.

### Consumes
- Tiêu thụ Runtime Harness & Spec Registry từ `Story 01.3` và 143+ `<BrandIcon />` SVG + `tokens.css` từ `Story 03.2`.

### Shared Entities
- Quản lý `FlowGraph`, `FlowNode`, `FlowTransition`, `FlowDraftDeployment` (`flow_graphs`, `flow_nodes`, `flow_edges`, `flow_draft_deployments` trong `V3__flow_workbench_dag.sql`) và đọc `Project`, `ProjectSpec`.

### Cross-Portal
- Hiển thị đa cổng (`PortalGroup` tabs / filter) trên đồ thị luồng liên cổng (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`).

## 7. QA Simulator Guardian Scorecard
| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Thay thế hoàn chỉnh `@xyflow/react` ảo bằng `DagCanvas2D` thực tế đang chạy cùng 3-layer SSOT |
| **2. Data Integrity & State** | `9.6/10` | `V3__flow_workbench_dag.sql` 12-Step Rebuild, OCC `version`/`layout_version` và chuẩn hóa enum |
| **3. Security & Validation** | `9.7/10` | Vá dứt điểm lỗ hổng `legacy_specs.rs` HMAC bypass và cô lập sandbox `allow-scripts allow-forms` |
| **4. Performance & Scalability** | `9.5/10` | Native SVG Bezier DAG 60fps, không thư viện nặng, `deploy-draft` idempotent checksum |
| **5. Error Handling & Recovery** | `9.4/10` | Chặn chu trình DFS 3 màu, phát hiện nút mồ côi, hoàn tác Undo/Redo độc lập Zoom/Pan |
| **6. Architectural Depth & Leverage** | `9.6/10` | Thanh tẩy duplicate `src/features/preview/*` và ép chuẩn `src/api/` canonical SDK |
| **7. UX Empathy** | `9.5/10` | Zero Amber tokens, 40px Shorten Rail, touchpad pinch-to-zoom mượt mà |
| **TOTAL AVERAGE** | **9.54/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log
| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 04.1 với @xyflow/react v12 và Canvas ScreenState | cowok Orchestrator |
| 2026-11-11 | 2.0.0 | Bổ sung DualMainPanel, Native 2D SVG Bezier DAG Canvas (v5.4) & V3 12-Step Rebuild | /refactor-story |
| 2026-11-13 | 3.0.0 | [evaluate-epic Deep Mode] Loại bỏ toàn bộ dư âm `@xyflow/react v12`; bổ sung vá bảo mật `legacy_specs.rs` HMAC, giải thể `src/features/preview/*`, chuẩn hóa enum & `src/api/` | Party Mode Council & Architect |
| 2026-11-14 | 3.1.0 | [evaluate-epic Report Sync] Đổi `/flow` -> `/flow/snapshot`, khóa regression `legacy_specs.rs` (`VerifiedLegacyIngress`) & singleflight trên `master`, thống nhất enum `DEFAULT|LOADING|EMPTY|ERROR` và dọn nốt `#f59e0b` (`DagCanvas2D`, `DagNodeHeaderAndPorts`) | /evaluate-epic Council & Architect |
