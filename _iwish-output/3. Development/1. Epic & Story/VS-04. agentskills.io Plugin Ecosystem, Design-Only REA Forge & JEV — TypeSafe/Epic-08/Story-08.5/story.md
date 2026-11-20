---
type: I-Wish Story
id: "08.5"
title: "Story 08.5: Split DE+DR Plugin Runtime, 7 Built-in DE Tools, Controlled Ephemeral Tool Jail & System-1 LEARN Loop"
description: "Hợp nhất 08.5 + 13.1-split + 11.2 cũ thành lát cắt dọc Split DE+DR hoàn chỉnh trên nền crates/plugin-host: Plugin Center Repo Import UI + Ephemeral Tool Audit Drawer + LEARN Queue + CLI uiux-os de-check <-> 7 Built-in DE Tools (scan-ui, intake KIND_CAP, tokens OKLCH auto-contrast, linear-NFA tells/impeccable/hallmark/stageshow <=25ms, archify, get_artifact BFS, notes-check) + HUMAN_ONLY Ephemeral Tool Jail (env_clear, O_NOFOLLOW) + System-1 LEARN filter (hit_count>=3, dedupe noul, SLPR>=0.95, auto-retire) -> V18 gate_failure_ledger."
resource: "iwish://uxui-os/development/epic-08/story-08.5"
status: refactored
risk_tier: 3
delivery: hybrid
adr_ref: "ADR-007"
components:
  - id: "de-dr-plugin-runtime"
    packaging: "KERNEL_CORE"
  - id: "user-skill-plugin-pack"
    packaging: "EXTENSION_PLUGIN"
timestamp: "2026-11-20T20:45:00+07:00"
tags:
  - "story-08.5"
  - "epic-08"
  - "split-de-dr"
  - "tool-jail"
  - "learn-loop"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.2. database-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../epic.md]]"
dependencies: ["02.4", "08.1", "08.2", "08.3", "08.4"]
provides_apis:
  - "POST /api/v1/de/run"
  - "POST /api/v1/plugins/import-repo"
  - "POST /api/v1/plugins/ephemeral-tools/execute"
  - "GET /api/v1/learn/ledger"
consumes_apis:
  - "POST /api/v1/projects/:id/learn/cards"
---

# Story 08.5: Split DE+DR Plugin Runtime, 7 Built-in DE Tools, Controlled Ephemeral Tool Jail & System-1 `LEARN` Loop

- **FR Covered:** [FR-35: Split DE+DR Plugin Runtime, 7 Built-in DE Tools, Controlled Ephemeral Tool Jail & System-1 LEARN](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 08: agentskills.io Repackager, Split DE+DR Runtime, 7 Built-in DE Tools, Controlled Tool Jail & LEARN](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Chuyển toàn bộ sức mạnh "Deterministic Engineering (DE) + Controlled Skill/Tool Extension" thành lát cắt vận hành thực sự theo `ADR-007`: tách cứng **7 công cụ DE siêu nhanh `$0` (`<=25ms`)** khỏi **Skill DR `SKILL.md`**, cho phép user nạp repo/skill tự tạo qua quét bảo mật & Progressive Disclosure, rèn/chạy script tạm trong `HUMAN_ONLY` Jail không thể RCE, và tự tiến hóa System-1 `LEARN` dựa trên `V18 gate_failure_ledger` có lọc thống kê chặt (`hit_count >= 3`, `SLPR >= 0.95`, `auto-retire`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6` (Scope=1.5, Data=1, Integration=1, UI=1, Security=1, Unknowns=0.5 $\rightarrow$ `CS = 6 < 7`, hợp nhất Tool Jail từ `11.2` cũ và Built-in DE + `LEARN` trên nền `crates/plugin-host` vừa hoàn thiện tại `08.3/08.4`).
- **Formal-Verify Candidate (`risk_tier: 3`):** Quy trình kill nhóm tiến trình Tool Jail (`setpgid`/`kill(-pgid, SIGKILL)` + `RLIMIT_AS`/`RLIMIT_NPROC`) và máy trạng thái hysteresis `V18a gate_failure_ledger -> LEARN Queue`.

## 3. Acceptance Criteria (AC)
- **AC-01 (7 Built-in DE Tools & CLI `uiux-os de-check`):** Đóng gói và phơi bày qua `/api/v1/de/*` + `uiux-os de-check`: (1) `scan-ui`, (2) `intake` (`KIND_CAP`), (3) `tokens` (OKLCH auto-contrast), (4) linear-NFA `tells/impeccable/hallmark/stageshow` rule packs (`<=25ms`), (5) `archify` (JSON-IR->SVG), (6) `get_artifact` (BFS bundler có `visited` cycle guard, `MAX_DEPTH`, `MAX_FILES`, `O_NOFOLLOW`), và (7) `notes-check`, đạt **DE Precision `>=95%`, Recall `>=95%`, FP `<=5%`**.
- **AC-02 (`HUMAN_ONLY` Controlled Ephemeral Tool Jail):** Hấp thụ `Story 11.2` cũ về `crates/plugin-host`: mọi script tạm hoặc repo bên ngoài user thêm vào qua `Plugin Center Repo Import UI` & `Ephemeral Tool Audit Drawer` phải chạy dưới nhóm tiến trình cô lập `setpgid(0,0)` + `kill(-pgid, SIGKILL)`, giới hạn `RLIMIT_NPROC`/`RLIMIT_AS`, cô lập mạng ngoài (network-isolated), `env_clear()`, `O_NOFOLLOW`, AST capability scan và **tuyệt đối chỉ chạy bằng phiên `ActorKind::HumanWorkbenchSession` (`HUMAN_ONLY`)** (`100%` chặn RCE/injection/traversal).
- **AC-03 (Auto-Package User Skills into JIT Progressive Disclosure):** Tự động quét bảo mật, đóng gói kỹ năng người dùng/repo mới thành `SKILL.md` (`<=100` tok metadata) + đăng ký vào **`V16c user_skill_registry`** (migration slot tách khỏi `04.6` `V16a` và `02.4` `V16b`) để nạp đúng lúc qua `CachePreservingSkillRouter` (`02.4`).
- **AC-04 (System-1 `LEARN` Loop & `V18a gate_failure_ledger`):** Lưu vết lỗi vào `V18a gate_failure_ledger`; chỉ đề xuất thẻ ở trạng thái `PROPOSED` vào `knowledge_cards` (`LEARN Queue` — `POST /api/v1/projects/:id/learn/cards`, tối đa `1,000` thẻ/scope, tuyệt đối phải qua duyệt `HUMAN_ONLY` của `08.4`) khi `hit_count >= 3`, dedupe bằng `Jev Noul`, áp ngưỡng hysteresis **`promote >= 0.95`, `auto-retire < 0.85`** theo trailing window chống rung lắc thống kê.
- **AC-05 ([EDGE-CASE] Injection Smuggling & Adversarial Tool Timeout):** Cô lập và tiêu diệt process chạy vượt timeout (`<=5s`), từ chối symlink/hardlink trong workspace giải nén repo và vô hiệu hóa mọi thẻ `LEARN` chứa chỉ thị thao túng `HUMAN_ONLY` hoặc `tests/eval/sealed/`.

## 4. Tasks
- [ ] **Task 08.5.1**: Hiện thực hóa **7 Built-in DE Tools** + linear-NFA rule packs (`<=25ms`) và CLI `uiux-os de-check` + `/api/v1/de/*` (thỏa mãn `AC-01`).
- [ ] **Task 08.5.2**: Xây dựng Controlled Ephemeral Tool Jail (`env_clear`, `O_NOFOLLOW`, `HUMAN_ONLY` auth gate) + Auto-Packaging vào `V16 user_skill_registry` (thỏa mãn `AC-02`, `AC-03`, `AC-05`).
- [ ] **Task 08.5.3**: Triển khai `V18 gate_failure_ledger` & bộ lọc thống kê System-1 `LEARN` (`hit_count>=3`, dedupe `noul`, `SLPR>=0.95`, `auto-retire`) + Plugin Center Import UI / Ephemeral Tool Audit Drawer / `LEARN` Queue (thỏa mãn `AC-04`).
- [ ] **Task 08.5.4**: Chạy bộ test Precision/Recall (`>=95%`, `FP<=5%`) và security regression suite (`100%` RCE/injection block) (thỏa mãn `AC-01..05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 08.5.1`, `Task 08.5.4` | Precision/Recall `>=95%` & `<=25ms` Benchmark |
| **AC-02** | `Task 08.5.2`, `Task 08.5.4` | `env_clear` + `O_NOFOLLOW` + RCE Pen-Test |
| **AC-03** | `Task 08.5.2` | Repo Import -> Progressive Disclosure Registration Test |
| **AC-04** | `Task 08.5.3` | `V18` Ledger & `SLPR>=0.95` Filter / Auto-Retire Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 08.5.2`, `Task 08.5.4` | Symlink/Timeout/Injection Security Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["02.4", "08.1", "08.2", "08.3", "08.4"]

### Impacts
- Cấp bộ 7 DE Tools & `LEARN` filter cho `Story 09.1`, `Story 09.2`, `Story 10.2`, `Story 11.2`, `Story 12.2`, và `Story 13.1`.

### Consumes
- Xây dựng trực tiếp trên `CachePreservingSkillRouter` (`02.4`), `crates/plugin-host` & `HumanApproverToken` (`08.1..08.4`).

### Shared Entities
- `skill_plugins`, `knowledge_cards`, `user_skill_registry` (`V16`), `gate_failure_ledger` (`V18`).

### Cross-Portal
- Áp dụng trên toàn bộ dự án & hệ plugin Cowok.ai.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Phủ trọn 7 DE Tools, User Skill Ingestion, Tool Jail & `LEARN` |
| **2. Data Integrity & State** | `9.5/10` | `V16` + `V18` tách bạch ledger và thẻ đã duyệt |
| **3. Security & Validation** | `9.8/10` | `HUMAN_ONLY` Jail (`env_clear`, `O_NOFOLLOW`) chặn 100% RCE |
| **4. Performance & Scalability** | `9.6/10` | Linear-NFA `<=25ms` ($0) trước khi gọi Decision Engine |
| **5. Error Handling & Recovery** | `9.5/10` | Auto-retire thẻ xuống phong độ & timeout kill sạch |
| **6. Architectural Depth & Leverage** | `9.7/10` | Dùng lại 100% `crates/plugin-host` thay vì trùng `11.2` |
| **7. UX Empathy** | `9.4/10` | Repo Import UI & Ephemeral Audit Drawer minh bạch quyền hạn |
| **TOTAL AVERAGE** | **9.59/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0 | Khởi tạo Story 08.5 hợp nhất Split DE+DR, Tool Jail (`11.2` cũ) & `LEARN` (`TDR-007`) | /make-story |
