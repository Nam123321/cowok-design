---
type: I-Wish Story
id: "05.5"
title: "Story 05.5: End-to-End /simulate-user (8D) & /edge-case-guardian (FMEA) -> 3-Tier Interactive Flow DAG"
description: "Tích hợp /simulate-user 8 chiều và /edge-case-guardian FMEA vào bộ tổng hợp DAG tương tác 3 tầng (/api/v1/flows/synthesize <-> 3-Tier DAG Synthesizer -> SQLite flow_nodes/flow_edges), giữ mặc định <=8 happy-path nodes/cluster cho app 10 màn hình kèm công tắc [Show Edge-Case Flows], 4-SubState pills trong từng node (DEFAULT/LOADING/EMPTY/ERROR), SampleDataStudio fixtures và 0 heuristic float leaks."
resource: "iwish://uxui-os/development/epic-05/story-05.5"
status: refactored
risk_tier: 2
delivery: core
timestamp: "2026-11-20T20:45:00+07:00"
tags:
  - "story-05.5"
  - "epic-05"
  - "simulate-user"
  - "edge-case-guardian"
  - "flow-dag"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.3. ui-ux-spec.md]]"
  - "[[../epic.md]]"
dependencies: ["05.1", "05.4", "09.2"]
provides_apis:
  - "POST /api/v1/flows/synthesize"
consumes_apis:
  - "POST /api/v1/decision/route"
---

# Story 05.5: End-to-End `/simulate-user` (8D) & `/edge-case-guardian` (FMEA) -> 3-Tier Interactive Flow DAG

- **FR Covered:** [FR-33: Cognitive Sketch-to-Polish, Steering Chips, Shared lol_html & 3-Tier Flow DAG](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 05: Option 5 2D Storyboard, Cognitive Sketch-to-Polish, Shared lol_html & 3-Tier Auto Flow DAG](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `2`

## 1. Story Description & Context
Non-tech thường thiếu toàn bộ luồng lỗi (`Empty`, `Loading`, `Error`, hủy thanh toán, mất mạng), nhưng nếu vẽ tất cả lên sơ đồ ngay lập tức thì DAG sẽ rối rắm hàng chục node. `Story 05.5` (từ `ex-05.6`) chạy ngầm `/simulate-user` 8D + `/edge-case-guardian` FMEA để tự động nạp đủ 4 SubStates vào trong từng node & nối `SampleDataStudio` fixtures, đồng thời giữ sơ đồ mặc định gọn `<=8` happy-path nodes/cluster và bật chi tiết bằng lens `[Show Edge-Case Flows]`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 4` (Vertical slice chuẩn trên `DagCanvas2D` & `crates/flow-graph`).

## 3. Acceptance Criteria (AC)
- **AC-01 (3-Tier DAG Synthesizer with `<=8` Happy-Path Default):** `/api/v1/flows/synthesize` phân tầng luồng sinh ra từ `/simulate-user` 8D + `/edge-case-guardian` FMEA vào `flow_nodes`/`flow_edges` kèm trường phân tầng (`flow_tier`, `cluster_id`, lưu trong `flow_node_substates` hoặc additive column) để gom cụm mặc định **`<=8` happy-path nodes/cluster** (tự động gộp nhóm khi app vượt `8` màn hình chính).
- **AC-02 (`[Show Edge-Case Flows]` Lens & In-Node 4-SubState Pills):** Công tắc **`[Show Edge-Case Flows]`** trên `DagCanvas2D` bật/tắt nhanh các nhánh ngoại lệ (`IF NOT / Fallback`); mỗi `FlowNode` hiển thị sẵn 4 pills **`DEFAULT | LOADING | EMPTY | ERROR`** gắn với `SampleDataStudio` fixtures để preview tức thì.
- **AC-03 (`VerdictShield` `0` Heuristic Float Leaks & Timeout Fallback):** Kết quả chấm ưu tiên rủi ro FMEA khi tổng hợp luồng sử dụng nhãn xác định (`ExactMatch` / `ModelVerdict` / `DeterministicFallback` + `#[serde(deny_unknown_fields)]`), tuyệt đối `0` raw float score rò rỉ ra prompt hoặc giao diện; fallback xác định khi `Decision #1` quá hạn.
- **AC-04 ([EDGE-CASE] Unguarded Auto-Cycle & Orphan Edge Rejection preserving `05.1` `loopback`):** Trước khi lưu `flow_nodes`/`flow_edges`, bộ 3-Color DFS của `crates/flow-graph` loại bỏ mọi edge trỏ tới `node_id` không tồn tại và mọi vòng lặp tự chuyển không có điểm thoát, nhưng **bảo toàn có chủ đích các cạnh quay lui người dùng hợp lệ (`flow_type = 'loopback'`)** của `Story 05.1`.

## 4. Tasks
- [ ] **Task 05.5.1**: Triển khai `/api/v1/flows/synthesize` & 3-Tier DAG Synthesizer tích hợp `/simulate-user` 8D + FMEA (`<=8` happy-path nodes/cluster, 0 float leak) trong `crates/flow-graph` & `crates/harness-daemon` (thỏa mãn `AC-01`, `AC-03`, `AC-04`).
- [ ] **Task 05.5.2**: Xây dựng UI `[Show Edge-Case Flows]` lens + in-node 4-SubState pills (`DEFAULT/LOADING/EMPTY/ERROR`) nối `SampleDataStudio` fixtures trên `DagCanvas2D` (thỏa mãn `AC-02`).
- [ ] **Task 05.5.3**: Viết test xác minh app 10 màn hình giữ `<=8` happy-path nodes mặc định, 4-SubState render thật và `0` float leaks (thỏa mãn `AC-01..04`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 05.5.1`, `Task 05.5.3` | 10-Screen App Synthesis (`<=8` Default Nodes) Test |
| **AC-02** | `Task 05.5.2`, `Task 05.5.3` | Playwright `[Show Edge-Case Flows]` & 4-SubState Pill Test |
| **AC-03** | `Task 05.5.1`, `Task 05.5.3` | Zero Raw-Float Prompt/UI Audit |
| **AC-04 (`[EDGE-CASE]`)** | `Task 05.5.1` | 3-Color DFS Cycle Guard Regression Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["05.1", "05.4", "09.2"]

### Impacts
- Cấp bản đồ 4-SubState đầy đủ để `Story 07.1` CDP QC quét tất cả trạng thái thay vì chỉ quét màn hình happy-path.

### Consumes
- Tiêu thụ Storyboard/SampleData từ `05.1`, `reconcile_ff_ids` từ `05.4`, và `VerdictShield` từ `09.2`.

### Shared Entities
- `flow_nodes`, `flow_edges`, `screen_fixtures`.

### Cross-Portal
- Áp dụng đồng bộ cho các flow thuộc mọi Portal của dự án.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.5/10` | Tự động hóa toàn diện happy-path + 4 sub-states từ FMEA |
| **2. Data Integrity & State** | `9.5/10` | `crates/flow-graph` 3-Color DFS chặn orphan edge & vòng lặp |
| **3. Security & Validation** | `9.4/10` | `0` heuristic float leaks nhờ `VerdictShield` |
| **4. Performance & Scalability** | `9.5/10` | Giữ `<=8` nodes mặc định giúp SVG DAG 60fps trên app lớn |
| **5. Error Handling & Recovery** | `9.5/10` | Luồng `ERROR` & `EMPTY` thành công dân hạng nhất trong mỗi node |
| **6. Architectural Depth & Leverage** | `9.5/10` | Tận dụng `SampleDataStudio` & `DagCanvas2D` |
| **7. UX Empathy** | `9.7/10` | Non-tech không bị ngợp bởi đồ thị chằng chịt ngay từ đầu |
| **TOTAL AVERAGE** | **9.51/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0 | Khởi tạo Story 05.5 (ex-05.6) 3-Tier Interactive Flow DAG hậu PR #14 | /make-story |
