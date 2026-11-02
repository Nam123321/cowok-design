# Epic 12 FMEA Risk Matrix — 4-Channel Packaging & Distribution Architecture

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E12-01 | Story 12.1 Edge Case | Tự động cấp phát cổng mạng trống (Zero Port Collision) khi khởi chạy nhiều phiên bản uiux-os up trên cùng một máy trạm. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 12.1.3 |
| EC-E12-02 | Story 12.2 Edge Case | Tự động đồng bộ cấu hình kết nối MCP và cặp đôi cowok/advisor khi cài đặt vào không gian làm việc Cowok.ai. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 12.2.3 |
| EC-E12-03 | Story 12.3 Edge Case | Chạy tiến trình bên trong container dưới quyền non-root user với giới hạn bộ nhớ rõ ràng cho Headless Chromium. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 12.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 12 (4-Channel Packaging & Distribution Architecture) | Review Agent & Advisor |
