# Edge Case Guardian & Adversarial Review Report — Story 01.3

- **Story:** Story 01.3 — Xây dựng crates/harness-daemon (Axum + Tokio) & 9-Spec Synthesis Pipeline
- **Epic:** Epic 01 (Cargo Workspace, Domain Contracts (ts-rs) & 16-Table SQLite Registry)
- **Reviewer:** Review Agent (Edge Case Guardian Skill)
- **Verdict:** APPROVED (PASS)

## 1. FMEA & Boundary Evaluation
- **Identified Edge Cases:**
  1. `AS-01.1`: Xử lý xung đột cổng mặc định `127.0.0.1:4170` bằng cơ chế tự động chọn cổng loopback rảnh và ghi nguyên tử thông tin kết nối vào `.cowok-design/daemon.lock` (ràng buộc trực tiếp vào `DaemonAuthState`).
  2. `BS-01.1`: Phát hiện lệch phiên bản khi kế thừa Brand Guideline xuyên dự án (`inherit_brand_from_project_id`) cho `03-ui-ux-spec.md` & `09-data-and-copy.md` (cấm tuyệt đối sao chép `08-security-tenancy.md`) thông qua `source_project_id` và `source_revision_sha256`.
  3. `BS-01.2` & `EC-E01-03`: Tổng hợp cấu trúc theo từng `spec_key`; không được phép bịa thông tin dưới nhãn `[CONFIRMED]` khi thiếu dữ liệu ngành; gắn nhãn `[MISSING]` / `[INFERRED]` kèm `open_questions` và kiểm soát chặt `seed_repo_path`.
  4. `EC-E01-09` & `EC-E01-14`: Xóa bỏ backdoor `is_loopback_hmac` và `forwardDirect`; bảo vệ toàn bộ routes (`/api/v1/projects`, `/auth/local-session`, `legacy_*`); loại bỏ RAM-only shadow stores (`proposals`, `sessions`, 22-byte mock ZIP) và symlink `src/app/api/v1/projects`; bổ sung `src/api/harness-daemon.ts`, `src/lib/validations/harness-daemon.ts` và bootstrap `X-Cowok-Session-Token` ở React Workbench.
- **Mitigation Verified:** Embedded into Acceptance Criteria `AC-03`..`AC-07`, mapped to `Task 01.3.1`..`Task 01.3.6`.
- **Complexity Score (CS):** `5.0/10` (< 7 threshold — No story split required).

## 2. Final Verdict
**VERDICT: PASS / APPROVED** (Reconciled 2026-11-13 via `/audit-360` + `/party-mode`)
