# Epic 02 FMEA Risk Matrix — JIT Prompt Assembler & 3-Profile Model Capability Adapter

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E02-01 | Story 02.1 Cross-Portal Guard | Ngăn chặn rò rỉ ngữ cảnh của Portal SuperAdmin sang bài thuyết trình Marketing công khai. | 9 | 5 | 3 | **135** | Enforced in 02.1 AC-03 & Task 02.1.1 |
| EC-E02-02 | Story 02.1 Deterministic Pruner (`02.2` absorbed) | Tự động nén danh sách component/icon theo độ liên quan ngữ nghĩa (`atomic_complete`) nếu lát cắt vượt `1,500` BPE tokens mà không cắt mù giữa thẻ HTML/JSON. | 8 | 5 | 4 | **160** | Enforced in 02.1 AC-02, AC-03 (`02.2` cancelled) |
| EC-E02-03 | Story 02.1 3-Profile Downgrade (`02.3` absorbed) | Tự động hạ cấp xuống Profile B với micro-tools định sẵn hoặc Profile C offline FTS5 nếu model nhỏ gặp lỗi cú pháp khi gọi tool phức tạp. | 8 | 5 | 4 | **160** | Enforced in 02.1 AC-04 (`02.3` cancelled) |
| EC-E02-04 | Story 02.1 Body Split-Brain Remediation | Synced consolidated Story 02.1 body (`02.1+02.2+02.3`), `status: refactored`, `dependencies: ["01.2","01.3"]`, Context Meter & `--inspect-prompt`. | 9 | 2 | 2 | **36** | Mitigated in Story 02.1 v2.0.0 |
| EC-E02-05 | Story 02.1 Prefix Byte-Stability | Non-deterministic token eviction breaks KV-cache static prefix across turns or leaks `max_image_tokens_per_turn`. Mitigation: `ReasonCode` deterministic pruner & strict Static-Prefix/Dynamic-Tail boundary. | 8 | 6 | 5 | **240** | Enforced in 02.1 AC-02..AC-04 |
| EC-E02-06 | Story 02.4 `app_shell_locks` & `oxc` Echo Loop | Concurrent agent overwrites `app_shell_locks`, races lease timeout takeover, or `oxc` reverse-sync overwrites locked `AppShell` / triggers `SpecWatcher` echo loop. Mitigation: Single Lock Owner monotonic `fencing_token` CAS, `quarantined_drift` isolation, hash-gated `SpecWatcher`, `V18` slot (`58` tables). | 9 | 6 | 5 | **270** | Enforced in 02.4 AC-01, AC-04, AC-05 |
| EC-E02-07 | Story 02.4 Router Order & Skill Slug Drift | 3-Zone `CachePreservingSkillRouter` non-deterministic order or `cowok-design-opt > 250` BPE tokens busts prefix cache; slug mismatch (`stageshow-slide`). Mitigation: Canonical lexicographical sorting + length-prefixed hash, BPE `<=250` cap, unified slugs (`stageshow-slide`, `impeccable-polish`). | 7 | 6 | 5 | **210** | Enforced in 02.4 AC-02, AC-03 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 02 (JIT Prompt Assembler & 3-Profile Model Capability Adapter) | Review Agent & Advisor |
| 2026-11-20 | 2.0.0 | Migrated to 1-10 FMEA scale (`EC-E02-01..07`), reconciled Story 02.1 (`02.2/02.3` cancelled) and Story 02.4 (`V18`, Single Lock Owner `fencing_token`, 7 Craft Skills) | Review Agent (`573b2485`) |
