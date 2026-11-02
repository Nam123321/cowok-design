# Epic 11 FMEA Risk Matrix — POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E11-01 | Story 11.1 Edge Case | Dọn sạch 100% tiến trình con và xóa ngay lập tức file tạm 0600 khi tiến trình hoàn tất hoặc bị hủy giữa chừng. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 11.1.3 |
| EC-E11-02 | Story 11.2 Edge Case | Chặn mọi lệnh truy cập mạng ngoài, đọc biến môi trường nhạy cảm hoặc ghi file ngoài thư mục dự án đối với script tự rèn. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 11.2.3 |
| EC-E11-03 | Story 11.3 Edge Case | Từ chối ngay lập tức với mã lỗi JSON-RPC chuẩn nếu một Agent bên ngoài cố gọi các công cụ thuộc tầng HUMAN_ONLY. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 11.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 11 (POSIX BYOS/BYOK Sandbox, Ephemeral Tool Forging & Universal Open MCP Server) | Review Agent & Advisor |
