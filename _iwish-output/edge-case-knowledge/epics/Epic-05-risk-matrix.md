# Epic 05 FMEA Risk Matrix — Structured Design Intent (SDI), Visual Clarification Variants & ChangeProposal Gate

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E05-01 | Story 05.1 Edge Case | Không tự ý sửa đổi hàng loạt màn hình khi ambiguityScore >= 0.45 mà bắt buộc dừng lại hiển thị Visual Clarification. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 05.1.3 |
| EC-E05-02 | Story 05.2 Edge Case | Khóa nút Apply nếu bất kỳ cổng kiểm tra bắt buộc nào (AST Constraint, Typecheck, Build) báo lỗi. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 05.2.3 |
| EC-E05-03 | Story 05.3 Edge Case | Đảm bảo gói ZIP xuất ra luôn bao gồm đầy đủ tokens.css, icon-manifest.json, fixtures và chạy được lệnh pnpm install && pnpm build ngay lập tức. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 05.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 05 (Structured Design Intent (SDI), Visual Clarification Variants & ChangeProposal Gate) | Review Agent & Advisor |
