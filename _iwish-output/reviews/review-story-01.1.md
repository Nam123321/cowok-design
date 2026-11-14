# Edge Case Guardian & Adversarial Review Report — Story 01.1

- **Story:** Story 01.1 — Khởi tạo Hybrid Monorepo (11 Rust Crates + pnpm) & crates/domain Type Sync
- **Epic:** Epic 01 (Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry)
- **Reviewer:** Review Agent (Edge Case Guardian Skill)
- **Verdict:** APPROVED (PASS)

## 1. FMEA & Boundary Evaluation
- **Identified Edge Cases:**
  1. `KU-01.1` / `EC-E01-01`: Ngăn chặn lệch kiểu dữ liệu (Type Drift), lỗi tràn số nguyên 64-bit (`u64`/`i64` vs JS `MAX_SAFE_INTEGER`) và khác biệt biểu diễn enum phân biệt (`#[serde(tag = "kind", content = "payload")]`) giữa Rust `serde_json` và TypeScript Zod.
  2. `EC-E01-03`: Khóa chặt whitelist `CanonicalSpecKey` (đúng 9 biến thể từ `01-product-overview` đến `09-data-and-copy`) ngay tại tầng giải tuần tự hóa để chặn mọi chuỗi chứa `../`, `/`, `\`, null byte hoặc tên file tùy ý.
  3. `BS-01.2`: Định nghĩa tường minh `ProvenanceStatus` (`CONFIRMED`, `INFERRED`, `MISSING`) trong `crates/domain` để phân biệt thông tin đã xác nhận và thông tin suy luận.
  4. `EC-E01-10` (`AU-01.2`): Thống nhất `CanonicalSpecKey` (`01-product-overview.md` .. `09-data-and-copy.md`), chuẩn hóa `ConstraintSeverity`, hỗ trợ cả `sha256:` và `sha256-`, phủ 100% Zod schema cho toàn bộ `ts-rs` exports với `SafeJsInteger`, đồng bộ `@cowok-design/shared-types`, và cấm symlink route directory.
- **Mitigation Verified:** Embedded into `[EDGE-CASE]` Acceptance Criteria `AC-03`, `AC-04`, and `AC-05`, mapped to `Task 01.1.3` and `Task 01.1.4`.
- **Complexity Score (CS):** `4.5/10` (< 7 threshold — No story split required).

## 2. Final Verdict
**VERDICT: PASS / APPROVED** (Reconciled 2026-11-13 via `/audit-360` + `/party-mode`)
