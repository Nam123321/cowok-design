---
type: I-Wish Story
id: '08.1'
title: 'Story 08.1: Xây dựng Hệ thống Năng lực Mở Đa Trục & Bộ Kỹ năng Built-in chuẩn
  agentskills.io (packages/builtin-skills & crates/plugin-host)'
description: Xây dựng Kiến trúc Đăng ký & Khám phá Năng lực Mở Đa Trục (craft/, skills/,
  design-templates/, design-systems/, scenarios/, atoms/, prompt-templates/), Per-Run
  Staging bảo toàn KV Prompt Cache, Động cơ Triangle Linter quét Anti-Slop/Geometric/WCAG
  < 25ms, và bộ 6 Kỹ năng Lõi FR-19 + 3 Kỹ năng Hạt giống chuẩn agentskills.io dưới
  runtime thẩm quyền D2 crates/plugin-host.
resource: _iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin
  Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/Story-08.1/story.md
status: fixing
risk_tier: 2
delivery: core
adr_ref: ../../../../2. Product Planning/ADRs/ADR-002-Tech-Stack-Tradeoffs-Rust-Go-vs-Node.md
timestamp: '2026-11-16T17:45:00+07:00'
tags:
- story-08.1
- epic-08
- uxui-os
- agentskills
- triangle-linter
- plugin-host
links_to:
- ../../../../2. Product Planning/2.1. product-brief-or-prd.md
- ../../../../2. Product Planning/2.5. architecture.md
- ../../../../2. Product Planning/2.2. database-spec.md
dependencies:
- '01.1'
- '01.2'
provides_apis:
- internal:epic-08/story-08.1
consumes_apis: []
contract_manifest:
  schema_version: 2
  status: IMPLEMENTED
  mutates: []
  provides:
  - crates/plugin-host:SkillRegistry
  - crates/plugin-host:PerRunStager
  - crates/plugin-host:TriangleLinter
  consumes: []
---

# Story 08.1: Xây dựng Hệ thống Năng lực Mở Đa Trục & Bộ Kỹ năng Built-in chuẩn agentskills.io (packages/builtin-skills & crates/plugin-host)

- **FR Covered:** [FR-19: Xây dựng Bộ Kỹ năng Built-in và Năng lực Mở Đa Trục chuẩn agentskills.io](../../../../2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 08: agentskills.io Repackager Engine, Design Capability Registry & Triangle Linter](../epic.md)
- **Status:** `completed` *(Stage 4 Codex Round 4 APPROVED 9.0/10; Stage 4B Human Completion Gate approved -> Story 08.1 completed)*
- **Risk Tier:** `2`

## 1. Story Description & Context
Thay vì khóa cứng một con số kỹ năng cố định, Story 08.1 thiết lập **Kiến trúc Đăng ký & Khám phá Năng lực Mở Đa Trục (Open-Ended Multi-Axis Design Capability Registry)** trong `packages/builtin-skills` kết hợp với **runtime thẩm quyền duy nhất (`D2 Single Core Port`) tại `crates/plugin-host`**:
1. **Phân định Thẩm quyền `D2` (`crates/plugin-host` SSOT vs `packages/builtin-skills` Seed Assets):**
   - `crates/plugin-host` là runtime duy nhất có thẩm quyền phân tích YAML frontmatter, tính `stable_prompt_hash`, staging `.cowok-skills/<run_id>/`, và thực thi Triangle Linter.
   - `packages/builtin-skills` lưu trữ tài nguyên tĩnh chuẩn (`craft/`, `skills/`, `design-templates/`, `design-systems/`, `scenarios/`, `atoms/`, `prompt-templates/`), DTO `ts-rs`/Zod và bộ kiểm thử golden fixtures đối chiếu với Rust (không duy trì logic bảo mật/validation độc lập gây lệch chuẩn).
2. **Kiến trúc Đa Trục & Bộ `6` Kỹ năng Lõi `FR-19` + `3` Kỹ năng Hạt giống Bổ sung (`9` Built-in Skills):**
   - **6 Kỹ năng vận hành lõi (`FR-19`):** `intent-clarifier-sdi`, `project-spec-synthesizer`, `flow-state-completer`, `design-taste-and-ux-guard`, `html-presentation-storyteller`, `self-healing-eval-gate`.
   - **3 Kỹ năng hạt giống mở rộng:** `reference-design-contract` (`observed | provided | inferred` + `Keep | Change | Do-Not-Copy`), `impeccable-design-polish` (24 động từ điều hướng thiết kế), `research-decision-room` (chống bịa đặt nguồn, xuất ADR).
   - Các trục bổ trợ: `craft/`, `design-templates/`, `design-systems/v1` (W3C DTCG), `scenarios/`, `atoms/`, `prompt-templates/`.
3. **Per-Run Staging & KV Prompt Cache Preservation:** Cơ chế phân vùng thư mục chạy nguyên tử `.cowok-skills/<run_id>/` tính toán hàm băm xác định `stable_prompt_hash` (SHA-256) trên toàn bộ trường ảnh hưởng prompt (`name`, `version`, `description`, sorted `capabilities_required`, `prompt_overview`, normalized `parameters`, `risk_tier`, `tags`, `SKILL.md` SHA-256) giúp giữ nguyên 100% KV Prompt Cache của các LLM Gateway.
4. **Triangle Deterministic Linter & Single-Pass Auto-Fix (`crates/plugin-host`):**
   - Quét tells & Anti-AI-Slop AST + Security Hygiene (`<script`, `dangerouslySetInnerHTML`, `javascript:` URI, external image beacons), 25+ luật hình học/WCAG 2.2 AA/concentric radii trong $< 25\text{ms}$ với `0` token LLM.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (`Scope=1`, `Data=1`, `Integration=1`, `UI=0.5`, `Security=1.0`, `Unknowns=0.5` $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn, không cần tách nhỏ thêm; phần SQLite/Daemon/UI thuộc `Story 08.4`).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability & Multi-Axis Registry):** Triển khai đầy đủ Kiến trúc Đăng ký & Khám phá Năng lực Mở Đa Trục (`craft/`, `skills/`, `design-templates/`, `design-systems/`, `scenarios/`, `atoms/`, `prompt-templates/`), bộ `6` kỹ năng lõi `FR-19` + `3` hạt giống mở rộng, và sắp xếp `SkillRegistry::discover` xác định theo thứ tự từ điển.
- **AC-02 (D2 Single Core Port & Domain Contracts):** Khóa `crates/plugin-host` là runtime thẩm quyền duy nhất tương thích kiểu `domain::models::SkillPlugin` (`Story 08.4` mở rộng đồng bộ `ts-rs`); `packages/builtin-skills` cung cấp assets tĩnh, Zod/DTO contracts và golden-fixture parity tests.
- **AC-03** (`[EDGE-CASE]` Resilience & Boundary Guard — Frontmatter <= 100 Tokens): Đảm bảo YAML frontmatter của mỗi `SKILL.md` luôn $\le 100$ tokens để nạp danh mục tìm kiếm mà không gây phình ngữ cảnh.
- **AC-04 (Triangle Deterministic Linter & Auto-Fix):** Cung cấp công cụ quét tells AI slop, 25+ luật hình học/WCAG và `lint_fix` 1 bước trong $< 25\text{ms}$.
- **AC-05 (Reference Design Contract & Deep-Research Seed Suite):** Triển khai trọn vẹn `reference-design-contract` (`Keep | Change | Do-Not-Copy`), `impeccable-design-polish`, `research-decision-room` cùng 67 styles, 25 chart types, 84 brand DNA presets.
- **AC-06** (`[EDGE-CASE]` `EC-E08-01` Strictly Delimited YAML Parser & Manifest Parity): `validate_skill_frontmatter` BẮT BUỘC phân tích bằng `serde_yaml` với ranh giới mở đầu `---` và đóng `\n---` nghiêm ngặt (loại bỏ fallback `else { trimmed }` và lỗi `find("---")` cắt sớm khi string YAML chứa `---`, từ chối anchors/aliases/custom tags); hiệu chỉnh bộ đếm token đa ngữ + non-ASCII/ký tự đặc biệt kèm trần cứng $\le 1,024$ bytes; và đối soát khớp 1:1 `name`, `version`, `description`, `risk_tier` giữa `SKILL.md` và `plugin.json`.
- **AC-07** (`[EDGE-CASE]` `EC-E08-08` Canonical KV-Cache Hash & Atomic Read-Only Staging): `compute_stable_prompt_hash` BẮT BUỘC băm theo khung có độ dài tiền tố (length-prefixed) trên đầy đủ `prompt_overview`, `parameters`, `risk_tier`, `tags`, `SKILL.md` SHA-256 và `capabilities_required` đã sắp xếp; `stage_run` tạo `.cowok-skills/<run_id>/` nguyên tử bằng `create_dir` (từ chối thư mục/symlink đã tồn tại) và khóa quyền chỉ đọc trong phiên; chặn `<IMPORTANT>`, `<SYSTEM>`, HTML comments, Unicode ẩn/bidi (`U+200B..U+200F`, `U+202A..U+202E`) và Markdown/HTML image beacons (`![]()`, `<img>`) trong `PROMPT.md`.
- **AC-08 (Triangle Linter Security Hygiene Rules):** Mở rộng `slop_scanner` (vẫn bảo đảm $< 25\text{ms}$) để phát hiện `<script`, `dangerouslySetInnerHTML`, `javascript:` URI và `![...](http...)` external image beacons trong đầu ra thiết kế/skill.

## 4. Tasks
- [x] **Task 08.1.1**: Chuẩn hóa Kiến trúc Năng lực Mở Đa Trục (`6` kỹ năng lõi `FR-19` + `3` hạt giống mở rộng) trong `packages/builtin-skills` dưới vai trò Asset + DTO + Golden Fixture SSOT (`AC-01`, `AC-05`).
- [x] **Task 08.1.2**: Khóa `crates/plugin-host` làm `D2 Single Core Port`, sắp xếp `SkillRegistry::discover` xác định và hoàn thiện Triangle Linter $< 25\text{ms}$ (`AC-02`, `AC-04`, `AC-08`).
- [x] **Task 08.1.3**: Cài đặt `[EDGE-CASE] EC-E08-01` (`serde_yaml`, strict `---` delimiters, multilingual token + `1,024B` cap, `SKILL.md <-> plugin.json` parity) trong `validator.rs` (`AC-03`, `AC-06`).
- [x] **Task 08.1.4**: Cài đặt `[EDGE-CASE] EC-E08-08` (length-prefixed `compute_stable_prompt_hash` đủ trường, atomic `create_dir` read-only staging, chặn `<IMPORTANT>`/bidi/`<img>` trong `PROMPT.md`) tại `staging.rs` (`AC-07`).
- [x] **Task 08.1.5**: Bổ sung golden-fixture regression suite xác minh chéo `crates/plugin-host` (`cargo test -p plugin-host`) và `packages/builtin-skills` (`vitest run`) (`AC-02`, `AC-06`, `AC-07`, `AC-08`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| **AC-01** | Core Capability & Multi-Axis Registry (6 FR-19 + 3 seed skills, sorted discovery) | [Code] Path: [registry.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/registry.ts)<br>[Code] Path: [staging.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/staging.rs)<br>[Code] Path: [staging.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/staging.ts)<br>[Code] Path: [registry.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/registry.rs)<br>[Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.1, Task 08.1.2 |
| **AC-02** | D2 Single Core Port (`crates/plugin-host`) & `packages/builtin-skills` Golden Fixture Parity | [Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs)<br>[Code] Path: [registry.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/registry.ts)<br>[Code] Path: [registry.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/registry.rs)<br>[Code] Path: [types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/types.ts)<br>[Code] Path: [validator.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/validator.ts) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.2, Task 08.1.5 |
| **AC-03** | `[EDGE-CASE]` Resilience & Boundary Guard — Frontmatter <= 100 Tokens | [Code] Path: [validator.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/validator.ts)<br>[Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs)<br>[Code] Path: [validator.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/validator.rs) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.3 |
| **AC-04** | Triangle Deterministic Linter & Single-Pass Auto-Fix (`< 25ms`) | [Code] Path: [slop_scanner.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/triangle_linter/slop_scanner.rs)<br>[Code] Path: [triangle-linter.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/triangle-linter.ts)<br>[Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs)<br>[Code] Path: [geometric_checker.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/triangle_linter/geometric_checker.rs) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.2 |
| **AC-05** | Reference Design Contract & Deep-Research Seed Suite (67 styles, 25 charts, 84 brand presets) | [Code] Path: [registry.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/registry.ts)<br>[Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs)<br>[Code] Path: [registry.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/registry.rs) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.1 |
| **AC-06** | `[EDGE-CASE]` `EC-E08-01` Strict `serde_yaml` Delimiters, BPE Multilingual + 1,024B Cap & Parity | [Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs)<br>[Code] Path: [registry.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/registry.ts)<br>[Code] Path: [registry.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/registry.rs)<br>[Code] Path: [validator.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/validator.rs)<br>[Code] Path: [types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/types.ts)<br>[Code] Path: [validator.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/validator.ts) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.3, Task 08.1.5 |
| **AC-07** | `[EDGE-CASE]` `EC-E08-08` Length-Prefixed KV-Cache Hash & Atomic Read-Only Staging | [Code] Path: [staging.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/staging.rs)<br>[Code] Path: [staging.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/staging.ts)<br>[Code] Path: [lib.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lib.rs) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.4, Task 08.1.5 |
| **AC-08** | Triangle Linter Security Hygiene Rules (`<script`, `dangerouslySetInnerHTML`, `javascript:`, beacons) | [Code] Path: [slop_scanner.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/triangle_linter/slop_scanner.rs)<br>[Code] Path: [triangle-linter.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/triangle-linter.ts) | No | [Test] Path: [builtin-skills.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/builtin-skills.test.ts)<br>[Test] Path: [story_08_1_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_1_parity.rs) | No | Completed | Task 08.1.2, Task 08.1.5 |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["01.1", "01.2"]

### Impacts
- Cung cấp toàn bộ hệ thống năng lực thiết kế mở và động cơ kiểm định Triangle Linter cho `Story 08.2`, `Story 08.3`, và tầng tích hợp `Story 08.4`.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: `Story 01.1` (`crates/domain`), `Story 01.2` (`crates/spec-registry`), `Story 03.1` (`crates/constraint-engine`), và tuân thủ trần ngân sách ngữ cảnh `FR-14` (`Epic 02` / `Story 02.1` soft architectural alignment).

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`SKILL_PLUGINS`, `PROJECTS`, `SPEC_DOCUMENTS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án (`SuperAdmin`, `TenantAdmin`, `SalesApp`, `MarketingWeb`, `DeveloperPortal`) và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Đồng bộ 6 lõi `FR-19` + 3 hạt giống mở rộng và Triangle Linter |
| **2. Data Integrity & State** | `9.7/10` | Khóa `crates/plugin-host` SSOT (`D2`), `SKILL.md <-> plugin.json` parity & `stable_prompt_hash` |
| **3. Security & Validation** | `9.8/10` | Vá YAML `---` delimiter breakout, BPE đa ngữ, atomic staging & Linter XSS/beacon hygiene |
| **4. Performance & Scalability** | `9.8/10` | Triangle Linter chạy `< 25ms` với 0 token LLM, bảo toàn KV prompt cache |
| **5. Error Handling & Recovery** | `9.5/10` | Fail-closed YAML/symlink/encoding guards rõ ràng |
| **6. Architectural Depth & Leverage** | `9.7/10` | Tái sử dụng `crates/domain` + `constraint-engine` theo chuẩn `agentskills.io` |
| **7. UX Empathy** | `9.6/10` | Giữ ngữ cảnh nhỏ gọn, triệt tiêu AI slop cho người dùng non-tech |
| **TOTAL AVERAGE** | **9.67/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 08.1 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-10 | 1.1.0 | Ánh xạ các repo đã hấp thụ (Hallmark, Impeccable, Stageshow, UI-UX-Pro-Max, Awesome-Design-MD, XState-Test) | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | Dựng code baseline trong `packages/builtin-skills` và `crates/plugin-host` (trạng thái `refactored`, chưa chốt ACs cứng) | cowok Orchestrator |
| 2026-11-16 | 2.1.0 | Reconciliation sau `/evaluate-epic`: chốt D2 `crates/plugin-host` SSOT, 6 lõi + 3 seed skills (`FR-19`), bổ sung `AC-06..AC-08` (`EC-E08-01`, `EC-E08-08`) | Antigravity / Epic 08 Evaluation |
| 2026-11-16 | 2.2.0 | Hoàn tất Stage 3B (`/pi-code-agent` + `antigravity`), Stage 4 Tournament & Codex Round 4 (`APPROVED`, `9.0/10`, `F-05 LOW` deferred) và chốt Stage 4B `completed` | Antigravity + Codex |
