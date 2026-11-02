# Epic 03 FMEA Risk Matrix — Project-Scoped Configurable Constraint Engine & <1ms data-ff-id Patcher

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E03-01 | Story 03.1 Edge Case | Đảm bảo mỗi dự án áp dụng độc lập cấu hình ràng buộc của riêng mình mà không làm ảnh hưởng tới dự án khác đang mở. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 03.1.3 |
| EC-E03-02 | Story 03.2 Edge Case | Khi Agent gọi tên icon không tồn tại trong icon-manifest.json của dự án, tự động gợi ý 3 icon hợp lệ gần nhất theo ngữ nghĩa. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 03.2.3 |
| EC-E03-03 | Story 03.3 Edge Case | Bảo toàn hoàn toàn cấu trúc cây cú pháp AST và định dạng code xung quanh khi vá nội dung chứa ký tự đặc biệt hoặc đa dòng. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 03.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 03 (Project-Scoped Configurable Constraint Engine & <1ms data-ff-id Patcher) | Review Agent & Advisor |
