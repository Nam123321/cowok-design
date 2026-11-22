# ADR-007: Split DE+DR Plugin Runtime, 7 Built-in DE Tools, Controlled Ephemeral Tool Jail & Sealed Anti-Goodhart Eval (`TDR-007`)

**Status:** ACCEPTED  
**Date:** 2026-11-20  
**Deciders:** System Architect, Security Lead, AI Evaluation Lead  

## 1. Context
Để `cowok-design` vượt trội các nền tảng AI native trong cả ứng dụng web lẫn bài thuyết trình HTML trên dự án dài kỳ, agent cần vừa có **Kỹ năng chuyên môn (DR — Progressive Disclosure Skills)**, vừa có **Công cụ cơ khí xác định (DE — Deterministic Engineering Tools & AST/NFA Scanners)**, đồng thời cho phép người dùng tự thêm repo GitHub/tạo skill mà vẫn chống RCE, prompt injection và tự gian lận điểm (`Goodhart's Law`) trong quá trình `advisor` tự tiến hóa (`LEARN`).

## 2. Decision
1. **Kiến trúc Split DE+DR Plugin Runtime & 7 Built-in DE Tools (`Story 08.5`):**
   - Tách rạch ròi lớp **DR** (`SKILL.md` Progressive Disclosure 3 tầng, tối ưu token `cowok-design-opt <=250 tok` + `<15ms` 3-Zone `CachePreservingSkillRouter`) với lớp **DE** (các tool Rust/TS xác định hoàn toàn không gọi LLM).
   - Đóng gói sẵn **7 Built-in DE Tools:** `scan-ui`, `intake` (`KIND_CAP`), `tokens` (OKLCH auto-contrast), linear-NFA rule packs `<=25ms` (`tells/impeccable/hallmark/stageshow`), `archify` (JSON-IR $\to$ SVG), `get_artifact` (BFS bundler), và `notes-check`.
2. **Controlled Ephemeral Tool Jail (`HUMAN_ONLY` Auth Tier):**
   - Chuyển phần rèn công cụ tạm thời (trước đây dự kiến ở `11.2`) hợp nhất thẳng vào `Story 08.5` trên nền hạ tầng `crates/plugin-host` (`Story 08.3`/`08.4`), bắt buộc thực thi trong jail cô lập: `env_clear()`, `O_NOFOLLOW`, kiểm tra hash, và xác nhận `HUMAN_ONLY` trước khi kích hoạt script/skill bên ngoài.
3. **System-1 `LEARN` Loop có Cổng Thống kê & Sealed Anti-Goodhart Benchmark (`Story 13.1`):**
   - Ghi nhận lỗi vào `V18 gate_failure_ledger`; chỉ đề xuất `knowledge_cards` khi `hit_count >= 3`, dedupe bằng `Jev Noul`, `SLPR >= 0.95` và tự động thoái lui (`auto-retire`) khi tụt chuẩn.
   - Đóng băng bộ chấm điểm chuẩn trong `tests/eval/sealed/` (`chmod 444` + kiểm tra SHA-256 trước mỗi lượt `uiux-os eval run`), kích hoạt ngay **`100% SEALED_EVAL_TAMPER_ERROR`** nếu phát hiện chỉnh sửa thước đo.

## 3. Consequences
- Bảo đảm độ chính xác `>=95%` và tỷ lệ cảnh báo giả `<=5%` cho bộ kiểm tra DE với chi phí `$0`.
- Cô lập rủi ro RCE/Prompt Injection khi mở rộng kho kỹ năng bên ngoài.
- Đảm bảo điểm tiến bộ giữa `with_skill` và `baseline` là thực chất, có thể kiểm chứng qua bộ 25 ca `/simulate-user` (`M1..M5`).
