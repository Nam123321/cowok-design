---
type: I-Wish Story
id: '08.2'
title: 'Story 08.2: Động cơ Đóng gói lại Repo GitHub, Discovery Stubs & Frame-Cited
  Video Intake (crates/plugin-host & packages/builtin-skills)'
description: Tự động phân tích mã nguồn/skill ngoài, bóc tách thành SKILL.md + scripts/
  + references/ + assets/ chuẩn agentskills.io, quét bảo mật Zero-Trust chống command
  injection/eval/IPI/binary spoofing, sinh Discovery Stubs fail-closed <= 1,536B và
  xử lý Frame-Cited Video Intake.
resource: _iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin
  Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/Story-08.2/story.md
status: completed
risk_tier: 3
delivery: core
adr_ref: ../../../../2. Product Planning/ADRs/ADR-002-Tech-Stack-Tradeoffs-Rust-Go-vs-Node.md
timestamp: '2026-11-16T17:46:00+07:00'
tags:
- story-08.2
- epic-08
- uxui-os
- repackager
- video-intake
- zero-trust
links_to:
- ../../../../2. Product Planning/2.1. product-brief-or-prd.md
- ../../../../2. Product Planning/2.5. architecture.md
- ../../../../2. Product Planning/2.2. database-spec.md
dependencies:
- '08.1'
provides_apis:
- internal:epic-08/story-08.2
consumes_apis: []
contract_manifest:
  schema_version: 2
  status: IMPLEMENTED
  mutates: []
  provides:
  - crates/plugin-host:SkillRepackager
  - crates/plugin-host:verify_repackaged_bundle
  - crates/plugin-host:commit_staged_bundle_atomic
  - crates/plugin-host:plugin_host_repackager
  - crates/plugin-host:scan_external_script_security
  - crates/plugin-host:VideoIntakeProcessor
  - packages/builtin-skills:repackageDirectory
  - packages/builtin-skills:BuiltinSkillsRegistry
  consumes: []
---

# Story 08.2: Động cơ Đóng gói lại Repo GitHub, Discovery Stubs & Frame-Cited Video Intake (crates/plugin-host & packages/builtin-skills)

- **FR Covered:** [FR-20: Động cơ Đóng gói lại Repo GitHub, Discovery Stubs & Video Intake](../../../../2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 08: agentskills.io Repackager Engine, Design Capability Registry & Triangle Linter](../epic.md)
- **Status:** `completed` *(Wave 7 Plan-Blind OCR 9.8/10 & Codex-Fallback 10.0/10 APPROVED; Stage 4B Human Completion Gate approved -> Story 08.2 completed)*
- **Risk Tier:** `3`

## 1. Story Description & Context
Story 08.2 xây dựng động cơ đóng gói lại mã nguồn bên ngoài (External Repackager Engine) và tiếp nhận video tham chiếu chuyển động (Frame-Cited Video Reference Intake), chạy thẩm quyền trên **`crates/plugin-host`** (`D2 Single Core Port`):
1. **Skill Repackager Engine**: Tự động phân tích thư mục mã nguồn ngoài, bóc tách và phân loại tệp thành cấu trúc chuẩn mực `agentskills.io`:
   - `SKILL.md` (tuần tự hóa YAML an toàn qua `serde_yaml`, frontmatter $\le 100$ tokens & $\le 1,024$ bytes).
   - `plugin.json` (manifest định danh, phiên bản, rủi ro, quyền hạn khớp 1:1 với `SKILL.md`).
   - `scripts/` (chỉ cho phép mã kịch bản văn bản UTF-8 đã qua kiểm duyệt tĩnh), `references/`, `assets/` (từ chối binary thực thi/magic-byte giả mạo).
   - Ghim toàn vẹn bằng `skills-lock.json` (`package_sha256`).
2. **Zero-Trust Security Scanner (`[EDGE-CASE] EC-E08-02` & `EC-E08-07`)**:
   - **Authoritative Single Scanner:** Chặn triệt để kỹ thuật né substring thường gặp (chuẩn hóa khoảng trắng/nối dòng, kiểm tra `.mjs/.cjs/.tsx/.jsx/.sh/.bash/.zsh/.rb/.pl/.svg/.html`, chặn `base64 -d`, `atob`, `Buffer.from`, chuỗi ghép `eval`/`Function`, `std::process::Command`, `Deno.Command`, `Bun.spawn`, `fetch`/`requests`/`urllib`, path traversal 1–2 cấp `../`, `..\`, `~/.ssh`, `/proc/self`).
   - **Indirect Prompt Injection & Exfiltration Defense:** Quét `SKILL.md` và `references/` chặn `<IMPORTANT>`, `<SYSTEM>`, YAML delimiter breakout `---`, HTML comments, bidi Unicode ẩn, và Markdown/HTML image beacons (`![...](http...)`, `<img src>`).
   - **Binary & Traversal Jail (`EC-E08-05`, `EC-E08-06`):** Khóa `skill_name` bằng `validate_safe_segment` trước khi nối `dest_dir`, từ chối symlink + hardlink (`nlink > 1`), giới hạn độ sâu/số file/tổng dung lượng chống zip-bomb, và từ chối non-UTF-8 hoặc executable magic bytes (`ELF`, `Mach-O`, `PE`, `Wasm`, `.so`, `.dylib`) thay vì rơi vào `from_utf8_lossy`.
3. **Upstream Discovery Stubs (Fail-Closed $\le 1,536$ bytes — target `~1.2KB`)**:
   - Sinh thẻ khám phá `discovery-stub.json` có chữ ký SHA-256 của gói và khóa cứng trần fail-closed ($\le 1,536$ bytes tổng thể kèm giới hạn từng trường — tuyệt đối không cắt xén âm thầm).
4. **Frame-Cited Video Reference Intake**:
   - `VideoIntakeProcessor` phân tích storyboard `[frame: mm:ss.xx] -> Action: Intent`, xác thực chặt chẽ định dạng timestamp hợp lệ và từ chối khung hỏng/rỗng trước khi xuất `VideoStoryboardReceipt` (`frame_id`).

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (`Scope=1`, `Data=0.5`, `Integration=1`, `UI=0`, `Security=2.0`, `Unknowns=0.5` $\rightarrow$ `CS = 5 < 7`, đạt ngưỡng an toàn; phần fetch mạng từ GitHub/SQLite/UI thuộc `Story 08.4`).

## 3. Acceptance Criteria (AC)
- **AC-01 (Core Capability & Repackager Layout):** Động cơ repackager phân rã và sắp xếp tệp thành công sang layout chuẩn `agentskills.io` (`SKILL.md`, `plugin.json`, `scripts/`, `references/`, `assets/`).
- **AC-02 (D2 Integration & Contracts):** Tương thích với kiểu dữ liệu `domain::models::SkillPlugin` và xuất DTO JSON chuẩn sang `packages/builtin-skills` (nối tuyến persistent `skill_plugins` ở `Story 08.4`).
- **AC-03 ([EDGE-CASE] Zero-Trust Command Injection Baseline):** Chặn đứng 100% mọi kịch bản chứa `curl | bash`, `eval`, `child_process.exec`, và duyệt thư mục trái phép (`EC-E08-02`).
- **AC-04 (Discovery Stubs & Frame-Cited Video Intake):** Tự động sinh `discovery-stub.json` (`~1.2KB`) và bóc tách storyboard video có trích dẫn mã `frame_id` theo từng mốc thời gian (`VideoIntakeProcessor`).
- **AC-05 ([EDGE-CASE] EC-E08-02 & EC-E08-06 Normalized Scanner, IPI & Binary Spoofing Guard):** Chuẩn hóa khoảng trắng/nối dòng trên mọi phần mở rộng kịch bản & markup; chặn `base64 -d`/`atob`/`Command` ẩn, `<IMPORTANT>`/delimiter breakout/image beacons trong `SKILL.md`; và **từ chối tuyệt đối** file non-UTF-8 hoặc magic bytes thực thi (`ELF`, `Mach-O`, `PE`, `Wasm`) thay vì `from_utf8_lossy` cho rơi vào `assets/`. Đồng bộ bộ test golden malicious fixtures giữa Rust & TS (`crates/plugin-host` làm SSOT).
- **AC-06 ([EDGE-CASE] EC-E08-04 Fail-Closed `discovery-stub.json` <= 1,536 Bytes):** Áp trần cứng fail-closed `stub_size_bytes <= 1,536` bytes cùng giới hạn từng trường (`name <= 64`, `version <= 32`, `upstream_url <= 256`, `capabilities <= 16 x 48`, `description <= 320` bytes) — từ chối tạo stub nếu vượt ngưỡng (tuyệt đối không silent truncation).
- **AC-07 ([EDGE-CASE] EC-E08-05 & EC-E08-07 Safe YAML, `skill_name` Path-Jail, Hardlink/Quota & Timeline Guard):** Xác thực `skill_name` qua segment allowlist trước `dest_dir.join(...)`; serialize YAML frontmatter bằng `serde_yaml` (chặn `format!()` delimiter breakout); từ chối hardlink (`nlink > 1`) và pre-existing symlinks tại `dest_dir`; áp quota số tệp, độ sâu, tổng dung lượng giải nén; và xác thực định dạng `[frame: mm:ss.xx]` nghiêm ngặt trong `VideoIntakeProcessor` + ghi biên nhận `skills-lock.json` (`package_sha256`).

## 4. Tasks
- [x] **Task 08.2.1**: Củng cố `SkillRepackager` và `VideoIntakeProcessor` trong `crates/plugin-host` (chuẩn layout `agentskills.io`, `skills-lock.json`, timeline validation) (`AC-01`, `AC-04`).
- [x] **Task 08.2.2**: Hợp nhất DTO/Contract giữa `crates/plugin-host` (`domain::models`) và `packages/builtin-skills` theo `D2 Single Core Port` (`AC-02`).
- [x] **Task 08.2.3**: Nâng cấp `scan_external_script_security` chặn đa hình/ẩn mã hóa, quét Indirect Prompt Injection trong `SKILL.md` và loại bỏ lỗ hổng `from_utf8_lossy` binary smuggling (`AC-03`, `AC-05`).
- [x] **Task 08.2.4**: Áp trần fail-closed $\le 1,536$ bytes cho `discovery-stub.json` và giới hạn từng trường metadata (`AC-06`).
- [x] **Task 08.2.5**: Áp `validate_safe_segment` cho `skill_name`, `serde_yaml` serialization, hardlink/quota guard và chạy golden test suite đồng thời trên Cargo + Vitest (`AC-05`, `AC-07`).

## AC-to-Task Traceability Matrix
| AC ID | Description | Mapped Implementation Tasks | Missing Implementation? | Test File Path | Missing Test? | Status | Mapped Tasks |
|---|---|---|---|---|---|---|---|
| **AC-01** | Core Capability & Repackager Layout (`SKILL.md`, `plugin.json`, `scripts/`, `references/`, `assets/`, `skills-lock.json`) | [Code] Path: `crates/plugin-host/src/repackager/repackager_engine.rs`<br>[Code] Path: [repackager.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager.ts)<br>[Code] Path: [repackager_engine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/repackager_engine.rs)<br>[Code] Path: [plugin_host_repackager.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/bin/plugin_host_repackager.rs)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs)<br>[Code] Path: [repackager-bundle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-bundle.ts) | No | [Test] Path: `crates/plugin-host/tests/story_08_2_parity.rs`<br>[Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Task 08.2.1 |
| **AC-02** | D2 Integration & Zod Contracts (`crates/plugin-host` & `packages/builtin-skills`) | [Code] Path: `packages/builtin-skills/src/types.ts`<br>[Code] Path: [repackager.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager.ts)<br>[Code] Path: [repackager_engine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/repackager_engine.rs)<br>[Code] Path: [plugin_host_repackager.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/bin/plugin_host_repackager.rs)<br>[Code] Path: [bridge-pin.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/bridge-pin.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs)<br>[Code] Path: [repackager-bundle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-bundle.ts) | No | [Test] Path: `packages/builtin-skills/tests/repackager.test.ts`<br>[Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Task 08.2.2 |
| **AC-03** | Zero-Trust Command Injection Baseline (`curl \ | bash`, `eval`, traversal)<br>[Code] Path: [repackager-assets.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-assets.ts)<br>[Code] Path: [repackager-security.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-security.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs)<br>[Code] Path: [security_scanner.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/security_scanner.rs) | No | [Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Completed | Task 08.2.3 |
| **AC-04** | Discovery Stubs (`~1.2KB`) & Frame-Cited Video Intake (`VideoIntakeProcessor`) | [Code] Path: `crates/plugin-host/src/repackager/video_intake.rs`<br>[Code] Path: [video_intake.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/video_intake.rs)<br>[Code] Path: [video-intake.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/video-intake.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs) | No | [Test] Path: `crates/plugin-host/tests/story_08_2_parity.rs`<br>[Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Task 08.2.1 |
| **AC-05** | Normalized Evasion Scanner, IPI `<IMPORTANT>`, & Binary Spoofing Guard (`EC-E08-02`, `EC-E08-06`) | [Code] Path: `crates/plugin-host/src/repackager/security_scanner.rs`<br>[Code] Path: [repackager_engine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/repackager_engine.rs)<br>[Code] Path: [plugin_host_repackager.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/bin/plugin_host_repackager.rs)<br>[Code] Path: [repackager-security.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-security.ts)<br>[Code] Path: [repackager-assets.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-assets.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs)<br>[Code] Path: [security_scanner.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/security_scanner.rs) | No | [Test] Path: `crates/plugin-host/tests/story_08_2_parity.rs`<br>[Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Task 08.2.3, Task 08.2.5 |
| **AC-06** | Fail-Closed `discovery-stub.json <= 1,536B` & Field Limits (`EC-E08-04`) | [Code] Path: `crates/plugin-host/src/repackager/repackager_engine.rs`<br>[Code] Path: [repackager.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager.ts)<br>[Code] Path: [repackager_engine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/repackager_engine.rs)<br>[Code] Path: [plugin_host_repackager.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/bin/plugin_host_repackager.rs)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs)<br>[Code] Path: [repackager-bundle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-bundle.ts) | No | [Test] Path: `packages/builtin-skills/tests/repackager.test.ts`<br>[Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Task 08.2.4 |
| **AC-07** | Safe YAML, FD-Relative `O_NOFOLLOW` Path-Jail, `nlink == 1`, Quotas & Strict Video Grammar (`EC-E08-05`, `EC-E08-07`) | [Code] Path: `crates/plugin-host/src/repackager/repackager_engine.rs`<br>[Code] Path: [repackager_engine.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/repackager_engine.rs)<br>[Code] Path: [repackager-security.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-security.ts)<br>[Code] Path: [video-intake.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/video-intake.ts)<br>[Code] Path: [repackager-bundle.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-bundle.ts)<br>[Code] Path: [video_intake.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/video_intake.rs)<br>[Code] Path: [plugin_host_repackager.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/bin/plugin_host_repackager.rs)<br>[Code] Path: [repackager.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager.ts)<br>[Code] Path: [bridge-pin.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/bridge-pin.ts)<br>[Code] Path: [repackager-assets.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/src/repackager-assets.ts)<br>[Code] Path: [mod.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/mod.rs)<br>[Code] Path: [security_scanner.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/src/repackager/security_scanner.rs) | No | [Test] Path: `crates/plugin-host/tests/story_08_2_parity.rs`<br>[Test] Path: [story_08_2_parity.rs](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/crates/plugin-host/tests/story_08_2_parity.rs)<br>[Test] Path: [repackager.test.ts](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/packages/builtin-skills/tests/repackager.test.ts) | No | Completed | Task 08.2.5 |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["08.1"]

### Impacts
- Cung cấp năng lực đóng gói, băm khóa SHA-256 và kiểm duyệt an toàn plugin cho `Story 08.3` và tầng tích hợp mạng/CSDL `Story 08.4`.

### Consumes
- Tiêu thụ các hợp đồng nền tảng từ: `Story 08.1` (`SKILL.md` YAML parser & Triangle Linter).

### Shared Entities
- Sử dụng các bảng được phân vùng trong `2.2. database-spec.md` (`SKILL_PLUGINS`, `PROJECTS`, `SPEC_DOCUMENTS`).

### Cross-Portal
- Áp dụng thống nhất trên toàn bộ các Portal của dự án và chế độ Presentation Studio 16:9.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Đáp ứng `FR-20`, `skills-lock.json`, fail-closed discovery stub và video intake |
| **2. Data Integrity & State** | `9.7/10` | Khóa SHA-256 toàn vẹn và loại bỏ cắt xén âm thầm metadata |
| **3. Security & Validation** | `9.9/10` | Chặn né scanner đa hình, IPI, image beacons, magic-byte spoofing, `skill_name` escape & hardlinks |
| **4. Performance & Scalability** | `9.6/10` | Quét tĩnh nhanh trong Rust với giới hạn quota chống DoS |
| **5. Error Handling & Recovery** | `9.7/10` | Fail-closed tường minh cho stub overflow, symlink và timeline hỏng |
| **6. Architectural Depth & Leverage** | `9.7/10` | Thống nhất `crates/plugin-host` SSOT và tái sử dụng `validator.rs` |
| **7. UX Empathy** | `9.5/10` | Bảo vệ người dùng khỏi plugin độc hại mà không mất thông tin rủi ro |
| **TOTAL AVERAGE** | **9.67/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Khởi tạo Story 08.2 đồng bộ với PRD v2.0 và Kiến trúc cowok/advisor | cowok Orchestrator |
| 2026-11-10 | 1.1.0 | Đưa các repo đã đánh giá (awesome-design-md, screenshot-to-code, xstate-test) vào pipeline đóng gói Plugin | cowok Orchestrator |
| 2026-11-12 | 2.0.0 | Dựng code baseline trong `crates/plugin-host` và `packages/builtin-skills` (trạng thái `refactored`, chưa đủ edge-case guards) | cowok Orchestrator |
| 2026-11-16 | 2.1.0 | Reconciliation sau `/evaluate-epic`: bổ sung `AC-05..AC-07` (`EC-E08-02`, `04`, `05`, `06`, `07`), khóa fail-closed stub `<= 1,536B`, IPI & binary spoofing guard | Antigravity / Epic 08 Evaluation |
