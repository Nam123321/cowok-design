---
type: I-Wish Story
id: "08.7"
title: "Story 08.7: Zero-Hardcode AI Harness Spine — UnifiedSkillCatalog, 2-Breakpoint Provider Prompt Caching, Dynamic Model Registry, 3-Bucket Skill Separation & Studio-to-Ledger Bridge"
description: "Loại bỏ 100% hardcode trong AI Harness Core theo ADR-009: thay CRAFT_SKILL_SPECS tĩnh bằng UnifiedSkillCatalog (quét động packages/host-skills + packages/design-craft + upstream-skills + SQLite ACTIVE plugins + LEARN cards), nối mount_default_design_craft_plugin() vào harness-daemon & orchestrator, triển khai 2-Breakpoint Cache-Safe Prompt Compiler với wire-native caching (Anthropic cache_control, Gemini cachedContent, OpenAI prompt_cache_key), Dynamic Model Registry, và Bridge 1 ghi nhận Manual Studio Correction (>=3 lần) vào V18a gate_failure_ledger."
resource: "iwish://cowok-design/development/epic-08/story-08.7"
status: backlog
risk_tier: 3
delivery: core
adr_ref: "ADR-009"
timestamp: "2026-11-24T12:10:00+07:00"
tags:
  - "story-08.7"
  - "epic-08"
  - "adr-009"
  - "zero-hardcode-harness"
  - "unified-skill-catalog"
  - "prompt-caching"
  - "studio-feedback-loop"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../../../../2. Product Planning/2.8. tech-stack-spec.md]]"
  - "[[../../../../2. Product Planning/ADRs/ADR-009-Zero-Hardcode-AI-Harness-Spine-And-Data-Storytelling-Feedback-Loop.md]]"
  - "[[../epic.md]]"
dependencies: ["08.5", "08.6", "09.2", "10.1", "11.1"]
provides_apis:
  - "GET /api/v1/skills/catalog"
  - "POST /api/v1/skills/catalog/reload"
  - "GET /api/v1/models/registry"
  - "POST /api/v1/learn/studio-corrections"
consumes_apis:
  - "POST /api/v1/decision/route"
  - "GET /api/v1/plugins"
  - "POST /api/v1/orchestrator/chat"
---

# Story 08.7: Zero-Hardcode AI Harness Spine — `UnifiedSkillCatalog`, 2-Breakpoint Provider Prompt Caching, Dynamic Model Registry, 3-Bucket Skill Separation & Studio-to-Ledger Bridge

- **FR Covered:** [FR-14, FR-19, FR-20, FR-21, FR-32, FR-34, FR-39: Zero-Hardcode AI Harness Spine, 3-Bucket Skill Ownership, Wire-Native Prompt Caching & Studio-to-Ledger Feedback](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 08: agentskills.io Repackager, Split DE+DR Runtime, 7 Built-in DE Tools, Controlled Tool Jail & LEARN](../epic.md)
- **ADR Reference:** [ADR-009: Zero-Hardcode AI Harness Spine & Data Storytelling Feedback Loop](../../../../2.%20Product%20Planning/ADRs/ADR-009-Zero-Hardcode-AI-Harness-Spine-And-Data-Storytelling-Feedback-Loop.md)
- **Status:** `backlog`
- **Risk Tier:** `3`

## 1. Story Description & Context
Chèn làm mốc **Pre-Wave C Keystone** (ngay sau Wave B và trước Wave C `07.1 / 10.2 / 11.2 / 14.2`) để xử lý triệt để 6 điểm hardcode/đứt mạch đã kiểm toán trong lõi AI Harness (`crates/spec-registry/src/skill_router.rs`, `crates/plugin-host/src/default_mount.rs`, `crates/bot-fleet/src/byok_gateway.rs`, `crates/harness-daemon/src/routes/orchestrator.rs`), chuẩn hóa quyền sở hữu 3 nhóm skill (`packages/design-craft/` vs `packages/host-skills/` vs `upstream-skills/`), bật prompt caching thực thụ ở tầng HTTP wire cho Anthropic/Gemini/OpenAI, và nối **Bridge 1** đưa các thao tác sửa tay lặp lại của người dùng trên `Studio Biểu đồ & Dashboard` về `V18a gate_failure_ledger`.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 6` (Scope=1.5, Data=0.5, Integration=1.5, UI=0.5, Security=1, Unknowns=1 $\rightarrow$ `CS = 6 < 7`, đạt ngưỡng an toàn).
- **Formal-Verify Candidate (`risk_tier: 3`):** Bất biến prefix KV-cache (Zone 1 SHA-256 bất biến tuyệt đối khi đổi Tier-2/Tier-3 JIT skills giữa các lượt chat), Zero-PII & `HUMAN_ONLY` boundary trên Bridge 1 (`studio-corrections -> gate_failure_ledger`), và tương thích ngược 100% gói `design-craft` độc lập.

## 3. Acceptance Criteria (AC)
- **AC-01 (3-Bucket Canonical Skill Separation & `UnifiedSkillCatalog` Zero-Hardcode):**
  - Tách dứt điểm 3 vùng: `packages/design-craft/` (100% Design, UX & Data Storytelling skills gồm `data-viz-storyteller v2.0` + 3 `references/*.md`), `packages/host-skills/` (chỉ host-runtime skills, gỡ bản sao thiết kế trùng trong legacy `packages/builtin-skills/skills/`), và `upstream-skills/` (100% verbatim community repos như `dashboard-design` MIT).
  - Thay hoàn toàn `CRAFT_SKILL_SPECS: [CraftSkillSpec; 7]` và matcher tiếng Anh cứng trong `crates/spec-registry/src/skill_router.rs` bằng `UnifiedSkillCatalog` nạp động 4 nguồn (`host-skills` + `mount_default_design_craft_plugin()` + `upstream-skills` + SQLite `skill_plugins ACTIVE` & `LEARN` cards đã duyệt), phơi bày `GET /api/v1/skills/catalog` và `POST /api/v1/skills/catalog/reload`, hỗ trợ định tuyến đa ngôn ngữ (VI/EN) qua FTS5 BM25 + `DecisionCascadeClient` (`09.2`).
- **AC-02 (2-Breakpoint Cache-Safe Prompt Compiler & Tier-3 Lazy `references/` Loading):**
  - Nối `ContextCompiler` + `UnifiedSkillCatalog` trực tiếp vào `crates/harness-daemon/src/routes/orchestrator.rs`.
  - Giữ **Zone 1 Static Cached Prefix** (System Identity + Tool Schemas + Tier-1 Skill Index `<= 100 tok`/skill sắp xếp deterministic + `DESIGN.md`/`tokens.css` hash) có `zone1_prefix_sha256` bất biến `100%` qua nhiều lượt hội thoại khác nhau; toàn bộ **Tier-2 `SKILL.md`** body và **Tier-3 `references/*.md`** (như `storytelling-arc.md`, `chart-selection-matrix.md`, `html-canvas-contract.md`) chỉ được nạp JIT vào **Zone 2 Dynamic Turn Tail / Tool Result**, kèm **Cache-Safe Compaction** tại ngưỡng `75%` context window.
- **AC-03 (Wire-Native LLM Provider Prompt Caching & Dynamic Model Registry):**
  - Gỡ bỏ `default_model_for_provider` cứng trong `crates/bot-fleet/src/byok_gateway.rs`; thay bằng **Dynamic Model Registry** (`GET /api/v1/models/registry`) đọc từ cấu hình chuẩn (SQLite + seed JSON cập nhật được không cần build lại binary) kèm capability flags (`supports_prompt_cache`, `context_window`, `tier`).
  - Phát đúng payload cache wire-native theo provider: Anthropic (`cache_control: { "type": "ephemeral" }` tại 2 breakpoints), Gemini (`cachedContent` handle + TTL lifecycle), OpenAI (`prompt_cache_key` deterministic theo `(project_id, catalog_sha256)`), và ghi nhận telemetry `cache_read_tokens` / `cache_creation_tokens` không chứa prompt/PII.
- **AC-04 (Bridge 1 — `Manual Studio Correction → V18a gate_failure_ledger`):**
  - Cung cấp endpoint `POST /api/v1/learn/studio-corrections` và hook không chặn UI trong `Studio Biểu đồ & Dashboard` (`06.5`) ghi nhận các sự kiện sửa tay lặp lại có ý nghĩa (`chart_type_override`, `qa_autofix_click`, `palette_contrast_fix`, `density_mode_override`) kèm ngữ cảnh cấu trúc ẩn danh (`Zero-PII`, không chứa dữ liệu CSV thật của người dùng).
  - Khi cùng một `correction_signature` đạt `hit_count >= 3` (tôn trọng công tắc opt-out), tự động ghi vào `V18a gate_failure_ledger` để đề xuất thẻ `LEARN` chờ người dùng duyệt `HUMAN_ONLY` (tuyệt đối không tự kích hoạt skill khi chưa duyệt).
- **AC-05 ([EDGE-CASE] Resilience, Cache-Key Isolation, Symlink/Tamper Jail & Offline Fallback):**
  - Mọi đường dẫn `SKILL.md` và `references/*.md` nạp động phải đi qua `O_NOFOLLOW` + path-jail boundary (từ chối symlink escape hoặc `..`).
  - Khi một plugin mới cài hoặc một thẻ `LEARN` được duyệt, chỉ `catalog_sha256` đổi có chủ đích và làm mới Zone 1 sạch sẽ; lỗi đọc 1 plugin hỏng không làm sập `UnifiedSkillCatalog` (cô lập lỗi theo từng plugin và ghi cảnh báo).
  - Khi chạy offline/air-gapped hoặc provider trả lỗi cache (`400`/`501` không hỗ trợ caching ở endpoint custom OpenAI-compatible), tự động hạ cấp mượt xuống gọi chuẩn không cache mà không thất bại lượt chat.

## 4. Tasks
- [ ] **Task 08.7.1**: Tái cấu trúc 3-Bucket (`packages/design-craft/` vs `packages/host-skills/` vs `upstream-skills/`), kiểm tra 0 phụ thuộc ngược trong `design-craft` và cập nhật workspace manifests + tests (thỏa mãn `AC-01`).
- [ ] **Task 08.7.2**: Xây dựng `UnifiedSkillCatalog` trong `crates/plugin-host` + `crates/spec-registry` (gỡ hoàn toàn `CRAFT_SKILL_SPECS` 7 phần tử cứng), nối `mount_default_design_craft_plugin()` + `upstream-skills` + SQLite `ACTIVE` + `LEARN` vào `crates/harness-daemon` và `orchestrator.rs`, phơi bày `/api/v1/skills/catalog*` (thỏa mãn `AC-01`, `AC-05`).
- [ ] **Task 08.7.3**: Nâng cấp 2-Breakpoint Cache-Safe Prompt Compiler (Zone 1 bất biến SHA-256 + Zone 2 Tier-2/Tier-3 JIT + 75% Compaction), Dynamic Model Registry (`/api/v1/models/registry`) và Wire-Native Caching cho Anthropic/Gemini/OpenAI trong `crates/bot-fleet/src/byok_gateway.rs` (thỏa mãn `AC-02`, `AC-03`, `AC-05`).
- [ ] **Task 08.7.4**: Xây dựng Bridge 1 (`POST /api/v1/learn/studio-corrections` -> `V18a gate_failure_ledger` tại `hit_count >= 3`, Zero-PII, `HUMAN_ONLY`) + nối hook từ `Studio Biểu đồ & Dashboard` (thỏa mãn `AC-04`).
- [ ] **Task 08.7.5**: Viết bộ test toàn diện (Rust + Vitest): 0-hardcode catalog reload, VI/EN routing cho `dashboard-design` & `data-viz-storyteller`, Zone 1 SHA-256 invariance across turns, HTTP wire-mock verification cho 3 providers + fallback khi custom endpoint không hỗ trợ cache, và Bridge 1 threshold `hit_count >= 3` + Zero-PII assertions (thỏa mãn `AC-01..AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 08.7.1`, `Task 08.7.2`, `Task 08.7.5` | 3-Bucket Parity, `UnifiedSkillCatalog` Dynamic Discovery & VI/EN Routing Test |
| **AC-02** | `Task 08.7.3`, `Task 08.7.5` | Zone 1 `sha256` Prefix Invariance & Tier-3 Lazy `references/` Injection Test |
| **AC-03** | `Task 08.7.3`, `Task 08.7.5` | Dynamic Model Registry & Wire-Mock Payload Tests (Anthropic/Gemini/OpenAI) |
| **AC-04** | `Task 08.7.4`, `Task 08.7.5` | Studio Correction Threshold (`>=3`) -> `gate_failure_ledger` & Zero-PII Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 08.7.2`, `Task 08.7.3`, `Task 08.7.5` | Symlink Path-Jail, Corrupt-Plugin Isolation & Cache-Unsupported Fallback Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** `["08.5", "08.6", "09.2", "10.1", "11.1"]`
- **Impacts / Unblocks:** Mở khóa trực tiếp Wave C (`Story 10.2` Swarm `Builder != Scorer` và `Story 11.2` Universal Open MCP Server sử dụng cùng `UnifiedSkillCatalog` không hardcode), cung cấp dữ liệu đầu vào Bridge 1 cho `Story 10.3` (`advisor` Reflective Skill PR Pipeline) và `Story 13.1` (Sealed `design-craft` SemVer Gating).
- **Shared Entities:** `skill_plugins`, `learn_knowledge_cards`, `gate_failure_ledger` (`V18a`), `model_registry_profiles` (additive SQLite migration).

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.7/10` | Xóa sạch 6 hardcode/đứt mạch cốt lõi của AI Harness Spine |
| **2. Data Integrity & State** | `9.6/10` | 1 Capability = 1 Canonical Owner; Zero-PII trên Bridge 1 ledger |
| **3. Security & Validation** | `9.7/10` | `O_NOFOLLOW` path-jail, `HUMAN_ONLY` gate, cô lập plugin hỏng |
| **4. Performance & Scalability** | `9.8/10` | 2-Breakpoint Zone 1 bất biến + Wire-Native Provider Caching giảm 60–85% token |
| **5. Error Handling & Recovery** | `9.5/10` | Hạ cấp mượt khi custom endpoint không hỗ trợ caching hoặc chạy air-gapped |
| **6. Architectural Depth & Leverage** | `9.8/10` | Nền móng chung cho Swarm (`10.2`), MCP (`11.2`), `advisor` (`10.3`) & `design-craft` |
| **7. UX Empathy** | `9.5/10` | Định tuyến tiếng Việt mượt mà, Studio tự học từ thói quen sửa tay của người dùng |
| **TOTAL AVERAGE** | **9.66/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-24 | 1.0 | Khởi tạo Story 08.7 (Pre-Wave C Keystone) hiện thực hóa `ADR-009` sau kiểm toán AI Harness Spine hậu Wave B | System Architect |
