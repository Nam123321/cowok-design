---
type: I-Wish Story
id: "08.3"
title: "Story 08.3: Quản lý Vòng đời Plugin 6 Trạng thái & Vòng Tự học LEARN có Van Duyệt Con Người (crates/plugin-host Domain Core)"
description: "Quản lý vòng đời plugin qua 6 trạng thái SCREAMING_SNAKE_CASE (DISCOVERED -> VALIDATED -> INSTALLED -> ACTIVE -> DEPRECATED -> ARCHIVED), structural breaking-change diffing & staging promotion/discard, và vòng tự học LEARN có ApproverContext HUMAN_ONLY chống đầu độc tri thức (EC-LEARN-001)."
resource: "_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/Story-08.3/story.md"
status: completed
risk_tier: 3
delivery: core
adr_ref: "../../../../2. Product Planning/ADRs/ADR-002-Tech-Stack-Tradeoffs-Rust-Go-vs-Node.md"
timestamp: "2026-11-16T17:47:00+07:00"
tags:
  - "story-08.3"
  - "epic-08"
  - "uxui-os"
  - "lifecycle"
  - "learn-pipeline"
  - "plugin-host"
links_to:
  - "../../../../2. Product Planning/2.1. product-brief-or-prd.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.2. database-spec.md"
dependencies: ["08.2"]
references: ["04.3", "04.4"]
provides_apis:
  - "internal:epic-08/story-08.3"
consumes_apis: []
contract_manifest:
  schema_version: 2
  status: IMPLEMENTED
  mutates: []
  provides:
    - "crates/plugin-host:ManagedPlugin"
    - "crates/plugin-host:ManifestUpdateOutcome"
    - "crates/plugin-host:load_verified_staged_bundle_from_dir"
    - "crates/plugin-host:LearnPipeline"
    - "crates/plugin-host:HumanSessionAuthority"
    - "crates/plugin-host:HumanApproverToken"
    - "crates/plugin-host:detect_breaking_change"
    - "crates/plugin-host:map_storage_constraint_error"
    - "packages/shared-types:Story08_3GeneratedTypes"
    - "packages/builtin-skills:PluginStateMachine"
    - "packages/builtin-skills:LearnPipeline"
    - "packages/builtin-skills:HumanSessionAuthority"
    - "packages/builtin-skills:detectBreakingChange"
    - "packages/builtin-skills:mapStorageConstraintError"
  consumes:
    - "crates/plugin-host:SkillRepackager"
    - "crates/plugin-host:scan_external_script_security"
---

# Story 08.3: Quản lý Vòng đời Plugin 6 Trạng thái & Vòng Tự học LEARN có Van Duyệt Con Người (crates/plugin-host Domain Core)

- **FR Covered:** [FR-21: Quản lý Vòng đời Plugin 6 Trạng thái & Vòng Tự học LEARN có Van Duyệt Con Người](../../../../2.%20Product%20Planning/2.1.%20product-brief-or-prd.md) *(Lưu ý: phần CSDL SQLite WAL, `tokio-cron-scheduler`, `ActorKind` daemon auth và Workbench UI được tách sang [Story 08.4](../Story-08.4/story.md))*
- **Epic:** [Epic 08: agentskills.io Repackager Engine, Design Capability Registry & Triangle Linter](../epic.md)
- **Status:** `completed` *(Stage 4 Plan-Blind OCR Epoch 6 PASS, Codex-Fallback Peer APPROVED 9.5/10, Wave 2 Party-Mode + /audit-360 Hardening APPROVED, Stage 4B Human Completion Gate approved -> Story 08.3 completed)*
- **Risk Tier:** `3`

## 1. Story Description & Context
Story 08.3 hoàn thiện phần nhân logic miền (`crates/plugin-host` theo `D2 Single Core Port`) cho vòng đời plugin và động cơ tự học tiến hóa (tách biệt sạch khỏi hạ tầng DB/Cron/Daemon API/UI ở `Story 08.4` để giữ `CS < 7`):
1. **6-State Plugin Lifecycle Machine + Complete Transitions**:
   - Máy trạng thái quản lý chặt chẽ 6 pha chuẩn `SCREAMING_SNAKE_CASE` (`DISCOVERED -> VALIDATED -> INSTALLED -> ACTIVE -> DEPRECATED -> ARCHIVED`), đóng gói kín các trường nội bộ của `ManagedPlugin` kèm `from_persisted` rehydration & `to_domain_plugin()`, bổ sung đầy đủ nhánh `VALIDATED -> ARCHIVED`, `INSTALLED -> ARCHIVED`, chuyển đổi `promote_staging` / `promote_verified_staging_dir` / `discard_staging` (kiểm tra `O_NOFOLLOW`, `nlink == 1`, `skills-lock.json` qua `load_verified_staged_bundle_from_dir`), và chốt **Quarantined/Revoked** chặn tái kích hoạt sau sự cố bảo mật.
   - Chặn bước nhảy trạng thái trái phép, ép buộc CAS `expected_revision: u64`, và cung cấp bộ mapping tương thích ngược `'ENABLED' -> 'ACTIVE'` từ Story 04.3/04.4.
2. **Breaking Change Isolation into Staging (`[EDGE-CASE] EC-E08-03`)**:
   - Thay thế cờ boolean tin cậy mù quáng từ caller bằng bộ **so khớp cấu trúc xác định (Structural Manifest & SemVer Breaking Diff)** để tự động phát hiện thay đổi phá vỡ (`capabilities_required`, `risk_tier`, union `"type"` JSON Schema `parameters`, major/pre-1.0 SemVer), trả về `Result<ManifestUpdateOutcome, PluginLifecycleError>` nguyên vẹn (tránh rollback trạng thái DB khi cách ly vào `Staging`), giữ nguyên phiên bản `ACTIVE` đang chạy và chặn ghi đè staging chưa xử lý.
3. **Self-Improving LEARN Domain Core with Bound `HumanApproverToken` (`EC-LEARN-001`)**:
   - `propose_card_scoped` & `propose_card_from_gate_report` (`AC-04`) kiểm tra trùng lặp ID/nội dung (`content_sha256`) và `idempotency_key` trong phạm vi `(tenant_id, project_id)`, và quét chặn prompt-injection/C1/bidi/`U+2028`/`U+E0001`/NFKC homoglyph/multiline image-beacons trong `source_evidence`, `title` và `content`.
   - `accept_card_verified` / `reject_card_verified` BẮT BUỘC nhận `HumanApproverToken` ký bởi `HumanSessionAuthority` (`HUMAN_WORKBENCH_SESSION` với khóa zeroize-on-drop / `WeakMap` + CSPRNG fail-closed, xoay vòng `kid` qua `register_authority`, gắn chặt `card_id`, `content_sha256`, `action`, `expected_revision` và TTL `1..=900s`), kiểm tra scope/CAS *trước khi* tiêu hủy `(tenant_id, project_id, nonce)`, hỗ trợ `from_persisted_state`/`export_consumed_nonces`/`prune_expired_nonces`, và cấm chuyển `REJECTED -> ACCEPTED`.
   - `get_active_prompt_injections` & `render_prompt_fenced_cards` chỉ trả về thẻ `ACCEPTED` đúng phạm vi `(tenant_id, project_id)`, sắp xếp xác định và chọn nguyên thẻ theo bộ đếm `cl100k_base` $\le 1,500$ tokens (`FR-14`) đặt trong ranh giới XML thoát ký tự (`<LEARNED_CARDS_DATA>` kèm `&apos;`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (`Scope=1`, `Data=1`, `Integration=1`, `UI=0`, `Security=1.5`, `Unknowns=0.5` $\rightarrow$ `CS = 5 < 7`; phần DB/Cron/Daemon Auth/UI tách sang `Story 08.4`).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability & 6-State Lifecycle):** Triển khai máy trạng thái 6 pha chuẩn mực quản lý vòng đời plugin (`SCREAMING_SNAKE_CASE` serde contract) hỗ trợ kích hoạt, ngừng sử dụng, CAS `revision >= 1`, `from_persisted` và lưu trữ an toàn.
- **AC-02 (D2 Domain Contracts):** Xuất hợp đồng kiểu chuẩn qua `ts-rs` (`crates/plugin-host --features ts` -> `packages/shared-types/src/generated`) tương thích `crates/domain::models::SkillPlugin` (`to_domain_plugin()`, `Story 08.4` nối CSDL `SKILL_PLUGINS`, `KNOWLEDGE_CARDS`, `LEARN_CONSUMED_NONCES`) và giữ `packages/builtin-skills` làm DTO/golden parity check.
- **AC-03 ([EDGE-CASE] Breaking Change Isolation Guard):** Giữ nguyên 100% phiên bản `ACTIVE` hiện tại và cô lập bản cập nhật mới vào `Staging` qua `ManifestUpdateOutcome::StagedForReview` khi phát hiện breaking changes (`EC-E08-03`).
- **AC-04 (Self-Improving LEARN Pipeline & `EC-LEARN-001` Filter):** Tổng hợp lỗi cổng qua `propose_card_from_gate_report` (`idempotency_key` theo scope) thành thẻ tri thức `PROPOSED` và chỉ cho phép đọc các thẻ `ACCEPTED` qua `render_prompt_fenced_cards` / `get_active_prompt_injections`.
- **AC-05 ([EDGE-CASE] EC-E08-03 & EC-E08-10 Complete Transitions, Diff Engine & Quarantine):** Bổ sung nhánh `VALIDATED -> ARCHIVED`, `INSTALLED -> ARCHIVED`, API `promote_staging` / `promote_verified_staging_dir` (kiểm tra `O_NOFOLLOW`, `nlink == 1`, `skills-lock.json`, frontmatter `risk_tier`/`capabilities_required`/`entry_point` và script extension allowlist) và `discard_staging`, cờ `quarantined` cấm `DEPRECATED -> ACTIVE` sau lỗi bảo mật, và bộ so khớp cấu trúc breaking change tự động (gồm union `"type"` & `risk_tier`).
- **AC-06 ([EDGE-CASE] EC-LEARN-001 Bound `HumanApproverToken`, Scope Isolation & Prompt-Injection Defense):** Bắt buộc truyền `HumanApproverToken` (`actor_kind == HUMAN_WORKBENCH_SESSION`) gắn `(kid, card_id, content_sha256, action, expected_revision)` khi gọi `accept_card_verified`/`reject_card_verified`; từ chối `propose_card_scoped` trùng `id`/`content_sha256` hoặc chứa IPI/C1/bidi/`U+2028`/`U+E0001`/NFKC homoglyph/delimiter breakout/`<img>` exfil; cấm nhảy `REJECTED -> ACCEPTED`; và thực thi chọn lọc theo phạm vi `(tenant_id, project_id)` cùng trần $\le 1,500$ BPE tokens (`FR-14`).

## 4. Tasks
- [x] **Task 08.3.1**: Chuẩn hóa `PluginStateMachine` / `ManagedPlugin` sang `SCREAMING_SNAKE_CASE`, bổ sung `VALIDATED|INSTALLED -> ARCHIVED`, `promote_staging`/`discard_staging`, `from_persisted` và `quarantined` guard (`AC-01`, `AC-05`).
- [x] **Task 08.3.2**: Cài đặt bộ Structural Manifest & SemVer Breaking-Change Diff (kể cả union `"type"` & `risk_tier` escalation) cùng `ManifestUpdateOutcome` transactional trong `crates/plugin-host` thay cho boolean thủ công (`AC-03`, `AC-05`).
- [x] **Task 08.3.3**: Gia cố `LearnPipeline` với `HumanSessionAuthority` / `HumanApproverToken` (`HUMAN_WORKBENCH_SESSION`, `kid` rotation, bound token, `from_persisted_state`, `export_consumed_nonces`), chống trùng `id`/`idempotency_key`, `propose_card_from_gate_report`, cấm `REJECTED -> ACCEPTED`, lọc `(tenant_id, project_id)` và ngân sách $\le 1,500$ tokens (`AC-04`, `AC-06`).
- [x] **Task 08.3.4**: Quét khử prompt-injection (`<IMPORTANT>`, `---`, C1/bidi/`U+2028`/`U+E0001`, NFKC homoglyph, multiline ref `<img src>`), kiểm tra `load_verified_staged_bundle_from_dir` (`O_NOFOLLOW`, `nlink == 1`, `skills-lock.json`) và đồng bộ `ts-rs` + DTO + golden fixtures với `packages/shared-types` & `packages/builtin-skills` (`AC-02`, `AC-05`, `AC-06`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| **AC-01** | Core Capability & 6-State Lifecycle (`SCREAMING_SNAKE_CASE`, `from_persisted`, CAS `u64`, `ENABLED` compat) | [Code] Path: [lifecycle-types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-types.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/mod.rs)<br>[Code] Path: [plugin-state-machine.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/plugin-state-machine.ts)<br>[Code] Path: [plugin-persisted-validator.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/plugin-persisted-validator.ts)<br>[Code] Path: [plugin_state_machine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/plugin_state_machine.rs)<br>[Code] Path: [lifecycle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle.ts) | No | [Test] Path: [lifecycle-learn.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/lifecycle-learn.test.ts)<br>[Test] Path: [story_08_3_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_3_parity.rs) | No | Completed | Task 08.3.1 |
| **AC-02** | D2 Domain Contracts (`ts-rs` + `to_domain_plugin()` + Zod + Golden Parity) | [Code] Path: [lifecycle-types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-types.ts)<br>[Code] Path: [learn_pipeline.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/learn_pipeline.rs)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/mod.rs)<br>[Code] Path: [lifecycle-errors.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-errors.ts)<br>[Code] Path: [plugin_state_machine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/plugin_state_machine.rs)<br>[Code] Path: [lifecycle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle.ts) | No | [Test] Path: [export_types.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/export_types.rs)<br>[Test] Path: [lifecycle-learn.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/lifecycle-learn.test.ts)<br>[Test] Path: [story_08_3_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_3_parity.rs) | No | Completed | Task 08.3.4 |
| **AC-03** | Breaking Change Isolation Guard (`ManifestUpdateOutcome`, `EC-E08-03`) | [Code] Path: [lifecycle-types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-types.ts)<br>[Code] Path: [plugin-persisted-validator.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/plugin-persisted-validator.ts)<br>[Code] Path: [breaking_diff.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/breaking_diff.rs)<br>[Code] Path: [lifecycle-errors.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-errors.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/mod.rs)<br>[Code] Path: [plugin-state-machine.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/plugin-state-machine.ts)<br>[Code] Path: [lifecycle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle.ts)<br>[Code] Path: [plugin_state_machine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/plugin_state_machine.rs)<br>[Code] Path: [breaking-diff.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/breaking-diff.ts) | No | [Test] Path: [lifecycle-learn.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/lifecycle-learn.test.ts)<br>[Test] Path: [story_08_3_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_3_parity.rs) | No | Completed | Task 08.3.2 |
| **AC-04** | Self-Improving LEARN Pipeline (`propose_card_from_gate_report`, `idempotency_key`, `EC-LEARN-001`) | [Code] Path: [lifecycle-types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-types.ts)<br>[Code] Path: [learn_pipeline.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/learn_pipeline.rs)<br>[Code] Path: [learn-pipeline.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-pipeline.ts)<br>[Code] Path: [learn-store-helpers.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-store-helpers.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/mod.rs)<br>[Code] Path: [learn-authority.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-authority.ts)<br>[Code] Path: [learn-sanitizer.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-sanitizer.ts)<br>[Code] Path: [lifecycle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle.ts) | No | [Test] Path: [lifecycle-learn.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/lifecycle-learn.test.ts)<br>[Test] Path: [story_08_3_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_3_parity.rs) | No | Completed | Task 08.3.3 |
| **AC-05** | Complete Transitions, Structural Diff, `O_NOFOLLOW` Staging Verify & `quarantined` Lock (`EC-E08-03`, `EC-E08-10`) | [Code] Path: [lifecycle-types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-types.ts)<br>[Code] Path: [plugin-persisted-validator.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/plugin-persisted-validator.ts)<br>[Code] Path: [breaking_diff.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/breaking_diff.rs)<br>[Code] Path: [lifecycle-errors.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-errors.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/mod.rs)<br>[Code] Path: [plugin-state-machine.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/plugin-state-machine.ts)<br>[Code] Path: [lifecycle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle.ts)<br>[Code] Path: [staging-security.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/staging-security.ts)<br>[Code] Path: [plugin_state_machine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/plugin_state_machine.rs)<br>[Code] Path: [breaking-diff.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/breaking-diff.ts) | No | [Test] Path: [lifecycle-learn.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/lifecycle-learn.test.ts)<br>[Test] Path: [story_08_3_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_3_parity.rs) | No | Completed | Task 08.3.1, Task 08.3.2 |
| **AC-06** | Bound `HUMAN_WORKBENCH_SESSION` HMAC Token, Scope Isolation, `<= 1,500` BPE Budget & IPI/NFKC Defense (`EC-LEARN-001`) | [Code] Path: [lifecycle-types.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-types.ts)<br>[Code] Path: [learn_pipeline.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/learn_pipeline.rs)<br>[Code] Path: [lifecycle-errors.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle-errors.ts)<br>[Code] Path: [learn-pipeline.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-pipeline.ts)<br>[Code] Path: [learn-store-helpers.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-store-helpers.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/lifecycle/mod.rs)<br>[Code] Path: [learn-authority.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-authority.ts)<br>[Code] Path: [learn-sanitizer.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/learn-sanitizer.ts)<br>[Code] Path: [lifecycle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/lifecycle.ts) | No | [Test] Path: [lifecycle-learn.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/lifecycle-learn.test.ts)<br>[Test] Path: [story_08_3_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_3_parity.rs) | No | Completed | Task 08.3.3, Task 08.3.4 |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["08.2"]

### Impacts
- Cung cấp nhân domain an toàn cho tầng CSDL/Cron/Daemon Auth/UI của `Story 08.4` và các Vertical Epics tiếp theo (`Epic 09`, `Epic 10`).

### Consumes
- Tiêu thụ các hợp đồng kiểm định và manifest từ: `Story 08.2` (`SkillRepackager` & `security_scanner`).

### Shared Entities
- Chuẩn hóa hợp đồng dữ liệu phục vụ `SKILL_PLUGINS` và `KNOWLEDGE_CARDS` trong `2.2. database-spec.md`.

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Hoàn thiện máy trạng thái 6 pha + structural breaking diff + LEARN core |
| **2. Data Integrity & State** | `9.7/10` | Cô lập `Staging`, chặn tái kích hoạt `quarantined`, cấm nhảy cóc `REJECTED -> ACCEPTED` |
| **3. Security & Validation** | `9.9/10` | Bắt buộc `ApproverContext` `HUMAN_ONLY`, cô lập tenant/project và khử prompt injection |
| **4. Performance & Scalability** | `9.7/10` | Bộ lọc và diff cấu trúc chạy tức thì `O(N)` trong Rust |
| **5. Error Handling & Recovery** | `9.7/10` | `discard_staging` và báo lỗi chuyển trạng thái tường minh |
| **6. Architectural Depth & Leverage** | `9.8/10` | Tách sạch domain (`08.3`) khỏi DB/Cron/HTTP/UI (`08.4`), giữ `CS < 7` |
| **7. UX Empathy** | `9.6/10` | Bảo toàn luồng ổn định cho user trong khi nâng cấp/học từ lỗi an toàn |
| **TOTAL AVERAGE** | **9.71/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 08.3 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | Dựng code baseline in-memory `PluginStateMachine` & `LearnPipeline` (trạng thái `refactored`) | cowok Orchestrator |
| 2026-11-16 | 2.1.0 | Reconciliation sau `/evaluate-epic`: bổ sung `AC-05..AC-06` (`EC-E08-03`, `EC-LEARN-001`), tách tầng SQLite/Cron/Auth/UI sang `Story 08.4` | Antigravity / Epic 08 Evaluation |
| 2026-11-17 | 2.2.0 | Wave 2 Hardening (`/audit-360`, `/party-mode`, `/infra-sync`, `/SSOT Reconciliation Skill`): bound `HumanApproverToken` + redacted zeroize/`WeakMap` + CSPRNG fail-closed, `ManagedPlugin::from_persisted` + `to_domain_plugin()` + transactional `ManifestUpdateOutcome` + `load_verified_staged_bundle_from_dir`, `LearnPipeline::from_persisted_state` + `propose_card_from_gate_report` + `export_consumed_nonces`, `ts-rs` exports, `Story-08.3/data-spec.md`, và đồng bộ `learn_consumed_nonces` / `kid` sang `Story 08.4` | Antigravity / Wave 2 Hardening |
