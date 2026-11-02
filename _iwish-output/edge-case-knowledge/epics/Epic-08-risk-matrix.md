# Epic 08 FMEA Risk Matrix — agentskills.io Repackager Engine, 6 Built-in Harness Skills & GitHub Cron Lifecycle

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E08-01 | Story 08.1 Edge Case | Đảm bảo phần YAML frontmatter của mỗi SKILL.md luôn <= 100 tokens để nạp vào danh mục tìm kiếm mà không gây phình ngữ cảnh. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 08.1.3 |
| EC-E08-02 | Story 08.2 Edge Case | Quét bảo mật và chặn mọi script độc hại hoặc lệnh gọi mạng trái phép khi nhập repository từ GitHub bên ngoài. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 08.2.3 |
| EC-E08-03 | Story 08.3 Edge Case | Giữ nguyên phiên bản Active hiện tại và đưa bản cập nhật mới vào vùng Staging nếu phát hiện thay đổi làm gãy schema (Breaking Change). | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 08.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 08 (agentskills.io Repackager Engine, 6 Built-in Harness Skills & GitHub Cron Lifecycle) | Review Agent & Advisor |
