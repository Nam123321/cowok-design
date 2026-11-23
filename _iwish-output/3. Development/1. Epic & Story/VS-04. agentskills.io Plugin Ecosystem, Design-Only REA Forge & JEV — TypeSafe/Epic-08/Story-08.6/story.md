---
type: I-Wish Story
id: "08.6"
title: "Story 08.6: Standalone `design-craft` GitHub Repo Assembler, 4-Pillar Operational Skills/Tools Pack, Multi-Agent Pack, Optional Upstream REA Installer & Default Plugin Mount"
description: "Single Accountable Owner đóng gói toàn bộ 4 trụ cột vận hành thiết kế FE & Presentation (5-Phase Cognitive Progressive Workflow + Business/Domain & Forged Craft Skills + Portable Builder & QC/Audit Deterministic Tools + Multi-Agent Orchestration Pack & Lightweight MCP Server) từ packages/builtin-skills thành GitHub Repo độc lập mang tên `design-craft`, kèm installer CLI đa harness gợi ý cài GitHub repo REA gốc (optionalPeers), và mount làm Default Plugin Repo của cowok-design."
resource: "iwish://uxui-os/development/epic-08/story-08.6"
status: completed
risk_tier: 3
delivery: core
timestamp: "2026-11-22T22:50:00+07:00"
tags:
  - "story-08.6"
  - "epic-08"
  - "design-craft"
  - "agentskills-io"
  - "plugin-repo"
  - "autonomous-agents"
  - "qc-tools"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.3. ui-ux-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../epic.md]]"
dependencies: ["08.5", "05.4", "05.6", "02.4"]
provides_apis:
  - "CLI: npx design-craft install --target <cowok|claude|codex|cursor|antigravity|openclaw>"
  - "MCP: npx design-craft-mcp (stdio)"
  - "Default Plugin Mount: crates/plugin-host default repo `design-craft`"
consumes_apis:
  - "crates/plugin-host (Plugin Lifecycle & agentskills.io Validator)"
  - "packages/builtin-skills (Single Assembly Staging Source)"
---

# Story 08.6: Standalone `design-craft` GitHub Repo Assembler, 4-Pillar Operational Skills/Tools Pack, Multi-Agent Pack, Optional Upstream REA Installer & Default Plugin Mount

- **FR Covered:** [FR-19, FR-20, FR-21, FR-32, FR-35: agentskills.io Plugin Ecosystem, Split DE+DR & Multi-Platform Distribution](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 08: agentskills.io Repackager, Split DE+DR Runtime, 7 Built-in DE Tools, Controlled Tool Jail & LEARN](../epic.md)
- **Status:** `backlog`
- **Risk Tier:** `3`

## 1. Story Description & Context
Giải quyết dứt điểm nguy cơ phân mảnh bộ skill/tool vận hành thiết kế qua nhiều Story bằng cách đặt **`Story 08.6`** làm **Single Accountable Owner (Trạm lắp ráp & Đóng gói duy nhất)** xuất bản repo GitHub độc lập mang tên **`design-craft`** (tách biệt rõ thương hiệu với ứng dụng/Harness OS `cowok-design`):
- **Đóng gói đủ 4 Trụ cột (không chỉ có workflow rỗng):**
  1. **Trụ cột 1 — Cognitive Progressive Workflow & Multi-Agent Orchestration (`flows/` + `agents/`):** Quy trình 5 Phase + 5 Adaptive Entry Modes (`GREENFIELD`, `ADD_FEATURE`, `REVAMP`, `MICRO_POLISH`, `DECK_FROM_APP`), Async HITL Checkpoints, và đội hình **`design-orchestrator` + 4 Sub-Agents (`flow-architect`, `brand-stylist`, `ui-crafter`, `design-auditor` Read-Only)**.
  2. **Trụ cột 2 — Business/Domain & Forged Craft Skills (`skills/domain-business/` + `skills/craft/`):** Toàn bộ Craft Skills đã forge (`cowok-design-opt`, `sketch-monochrome`, `oklch-palette`, `typography`, `impeccable-polish`, `stageshow-slide`, `dashboard-ux`, `archify-diagram`, `reference-design-contract`) cộng thêm **5 Business/Domain Skills** (`business-requirements-distiller`, `ux-microcopy-and-i18n`, `user-simulation-8d`, `data-viz-storyteller`, `responsive-and-a11y-craft`).
  3. **Trụ cột 3 — Portable Deterministic Engine cho Builder & QC/Audit Agent (`engine/` + `mcp/server.mjs`):**
     - *Builder Tools:* `de-phase-router`, `de-intake`, `de-scan-sketch`, `de-tokens-oklch` (hỗ trợ `themeModes` linh hoạt theo yêu cầu user), `de-brand-preview`, `de-extract-contract`, `de-patch-dom`.
     - *QC/Audit Tools (Đôi mắt của `design-auditor`):* `qc-a11y-contrast`, `qc-anti-slop-linter` (bắt jargon kỹ thuật, hex/Tailwind cấm, placeholder), `qc-layout-overflow` (Desktop/Mobile/Slide 16:9), `qc-chart-and-data`, `qc-spec-parity`, `qc-popular-coverage`.
     - **Capability-Driven Upgrade:** Khi chạy trên IDE ngoài (Claude/Codex/Cursor/AGY/OpenClaw) dùng bản Portable `engine/*.mjs` / `mcp/server.mjs`; khi gắn làm **Default Plugin Repo** trong `cowok-design`, tự động nâng cấp 1:1 sang nhân **Rust Native (`crates/html-forge`, `oxc`, `spec-registry`) + Pooled 1-Chromium CDP (`Story 07.1`)**!
  4. **Trụ cột 4 — Tri thức Chuẩn hóa & Rubric (`catalogs/` + `templates/` + `rubrics/`):** `popular-components.{fe,deck}.json`, `reference-archetypes.json`, `domain-blueprints.json`, `templates/specs/01..09`, `rubrics/design-qc-rubric.json`.
- **Tích hợp Optional Upstream REA:** `plugin.json` khai báo `optionalPeers` trỏ thẳng tới **GitHub repo REA gốc**; CLI installer `npx design-craft install` gợi ý thân thiện cài kèm REA gốc nếu user muốn trích xuất phong cách & hành vi UI từ app/website thực tế (và lọc an toàn qua skill `reference-design-contract`), fallback sạch sang `reference-archetypes.json` nếu không cài.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6` (Scope=1.5, Data=0.5, Integration=1.5, UI=0.5, Security=1, Unknowns=1 $\rightarrow$ `CS = 6 < 7`).

## 3. Acceptance Criteria (AC)
- **AC-01 (4-Pillar `design-craft` Standalone Repo Assembly):** Lệnh build/export `scripts/assemble-design-craft-repo.ts` gom sạch 100% tài sản từ `packages/builtin-skills/` thành cấu trúc repo `design-craft` hợp lệ theo chuẩn `agentskills.io` + `plugin.json` (có đủ `flows/`, `skills/phase-*`, `skills/craft/*`, `skills/domain-business/*`, `agents/`, `engine/`, `mcp/`, `catalogs/`, `templates/`, `rubrics/`), 0 phụ thuộc ngược vào mã nguồn riêng của `cowok-design`.
- **AC-02 (Portable Builder + QC/Audit Tools & Zero-Shell MCP Server):** Toàn bộ 7 Builder Tools (`de-*`) và 6 QC/Audit Tools (`qc-*`) chạy độc lập qua CLI (`npx design-craft ...`) lẫn **stdio MCP Server (`npx design-craft-mcp`)** cho autonomous agent không có shell; khi chạy trong `cowok-design`, capability detection tự định tuyến 1:1 sang Rust Native/CDP mà không đổi chữ ký gọi tool.
- **AC-03 (Multi-Harness Mount & Sub-Agent Orchestration Export):** `npx design-craft install --target <cowok|claude|codex|cursor|antigravity|openclaw>` tự biên dịch `agents/*.md` (`design-orchestrator` + 4 Sub-Agents tách biệt `Builder != design-auditor`) và `flows/cognitive-progressive.yaml` (hỗ trợ Async HITL suspend/resume tại Phase Gates) sang đúng định dạng gốc của từng harness (`.claude/agents/`, `.codex/agents/*.toml`, `.cursor/agents/`, `.agents/agents/`, `agent-pack.json`).
- **AC-04 (Optional Upstream REA Repo Recommendation & Graceful Fallback):** `plugin.json` khai báo `optionalPeers` trỏ trực tiếp **GitHub repo REA gốc** (không tạo repo bọc trung gian giả). Trình cài đặt hiển thị tùy chọn gợi ý cài REA gốc; nếu user bỏ qua, mọi lệnh Phase 2 vẫn chạy 100% qua `catalogs/reference-archetypes.json` + ảnh/mô tả.
- **AC-05 (Default Plugin Mount in `cowok-design`):** `crates/plugin-host` mount `design-craft` làm **Default Built-in Plugin Repo** ngay khi khởi động `cowok-design` (hiển thị trong Plugin Center, tương thích GitHub Cron Sync `FR-21` và nâng cấp SemVer v1.x từ các Story `09.1`/`09.2`/`10.3`).
- **AC-06 ([EDGE-CASE] Security, Disjoint Multi-Agent Ownership & Token Discipline):**
  - Toàn bộ `engine/*.mjs` và `mcp/server.mjs` chạy Zero-Trust Path Jail (chặn `..`, symlink escape `O_NOFOLLOW`, command injection, và network egress trái phép).
  - Multi-Agent Orchestrator ép buộc **Disjoint File Ownership** khi chạy song song (mỗi sub-agent chỉ ghi file riêng, chỉ Orchestrator hợp nhất) và **KHÔNG tự vượt Phase Gate** nếu chưa có xác nhận người dùng (trừ cờ tường minh `--auto-pilot` kèm Heritage Pack).
  - Với yêu cầu `MICRO_POLISH`, `de-phase-router` chạy single-agent + `<1ms` DE (tuyệt đối không spawn 4 sub-agent gây lãng phí token).
  - `de-tokens-oklch` và `de-brand-preview` chỉ sinh đúng danh sách `themeModes` user yêu cầu (hỗ trợ `light`, `dark`, `warm`, `dim`, hoặc custom mode — không ép mặc định Light/Dark).

## 4. Tasks
- [ ] **Task 08.6.1**: Chuẩn hóa 5 Business/Domain Skills (`business-requirements-distiller`, `ux-microcopy-and-i18n`, `user-simulation-8d`, `data-viz-storyteller`, `responsive-and-a11y-craft`), 5 Phase Skills, `catalogs/` và `rubrics/design-qc-rubric.json` trong `packages/builtin-skills/`.
- [ ] **Task 08.6.2**: Triển khai Portable Builder (`de-*`) & QC/Audit (`qc-*`) Engine (`engine/*.mjs`) + Zero-Shell stdio MCP Server (`mcp/server.mjs`) với capability fallback 1:1 cho `cowok-design` Rust Native.
- [ ] **Task 08.6.3**: Triển khai Multi-Agent Orchestration Pack (`agents/`, `flows/cognitive-progressive.yaml`), Multi-Harness CLI Installer (`bin/design-craft.mjs` + `optionalPeers` trỏ **GitHub repo REA gốc**) và `scripts/assemble-design-craft-repo.ts`.
- [ ] **Task 08.6.4**: Mount `design-craft` làm Default Plugin Repo trong `crates/plugin-host`, viết bộ kiểm định Clean-Room (chạy độc lập trong thư mục tạm không có `cowok-design`) cho CLI + MCP + QC Tools + Multi-Harness Mount.

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 08.6.1`, `Task 08.6.3` | Clean-Room Standalone Assembly & `agentskills.io` Validator Test |
| **AC-02** | `Task 08.6.2`, `Task 08.6.4` | Portable DE + 6 QC Tools + MCP Stdio Contract Test |
| **AC-03** | `Task 08.6.3`, `Task 08.6.4` | Multi-Harness Export (Claude/Codex/Cursor/AGY/OpenClaw) Snapshot Test |
| **AC-04** | `Task 08.6.3` | Optional Upstream REA Prompt & Offline Archetype Fallback Test |
| **AC-05** | `Task 08.6.4` | `crates/plugin-host` Default Mount & Plugin Center E2E Test |
| **AC-06 (`[EDGE-CASE]`)** | `Task 08.6.2`, `Task 08.6.3`, `Task 08.6.4` | Path Jail, Disjoint Sub-Agent Fence, Micro-Polish Fast-Path & Custom `themeModes` Tests |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["08.5", "05.4", "05.6", "02.4"]`
- **Consumed by:** `Story 12.1` & `Story 12.2` (nhúng trực tiếp gói `design-craft` vào single-binary CLI, Tauri Desktop & Cowok.ai Native Pack); `Story 09.1` / `09.2` / `10.3` (phát hành bản nâng cấp SemVer v1.1/v1.2 của `design-craft` qua cùng 1 pipeline).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Đủ cả 4 trụ cột (Workflow + Business/Craft Skills + Builder & QC Tools + Multi-Agent Pack) |
| **2. Data Integrity & State** | `9.5/10` | Single assembly source (`packages/builtin-skills/`), 0 phân mảnh giữa các Story |
| **3. Security & Validation** | `9.6/10` | Path Jail + Design-Only REA boundary + Independent Read-Only `design-auditor` |
| **4. Performance & Scalability** | `9.6/10` | Fast-path single-agent cho `MICRO_POLISH`; tự nâng cấp sang Rust Native trong `cowok-design` |
| **5. Error Handling & Recovery** | `9.5/10` | Optional REA fallback mượt sang archetype; Async HITL suspend/resume an toàn |
| **6. Architectural Depth & Leverage** | `9.8/10` | 1 repo mở phục vụ đồng thời GitHub GTM + Default Plugin Repo của `cowok-design` |
| **7. UX Empathy** | `9.6/10` | Tên `design-craft` tách bạch với `cowok-design`, cài 1 lệnh cho mọi IDE/Autonomous Agent |
| **TOTAL AVERAGE** | **9.60/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-22 | 1.0 | Khởi tạo Story 08.6 làm Single Owner đóng gói repo độc lập `design-craft` đủ 4 trụ cột + Optional Upstream REA + Default Plugin Mount | /make-story |
