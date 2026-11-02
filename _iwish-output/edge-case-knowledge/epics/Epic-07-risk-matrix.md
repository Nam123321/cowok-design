# Epic 07 FMEA Risk Matrix — 17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E07-01 | Story 07.1 Edge Case | Phát hiện chính xác hiện tượng tràn khung dọc/ngang (scrollHeight > clientHeight) khi người dùng nhập đoạn văn bản quá dài trên slide 16:9. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 07.1.3 |
| EC-E07-02 | Story 07.2 Edge Case | Giới hạn vòng lặp Auto-Fix tối đa 3 lần và hiển thị so sánh trước/sau để tránh làm hỏng ý đồ thiết kế của người dùng. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 07.2.3 |
| EC-E07-03 | Story 07.3 Edge Case | Đảm bảo các biểu đồ động và SVG icons hiển thị sắc nét 100% khi mở file .html ở chế độ hoàn toàn không có mạng Internet. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 07.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 07 (17-Rule Multi-Viewport Visual Eval Gate & Self-Contained HTML / Vector PDF Exporter) | Review Agent & Advisor |
