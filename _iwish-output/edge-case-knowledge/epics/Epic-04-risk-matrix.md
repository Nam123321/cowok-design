# Epic 04 FMEA Risk Matrix — Flow-First XYFlow Canvas, State-Hydrated Preview & Element-Anchored Comments

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E04-01 | Story 04.1 Edge Case | Phát hiện và cảnh báo các nút ScreenState mồ côi (không có cạnh Transition nối vào hoặc ra) trên sơ đồ luồng. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 04.1.3 |
| EC-E04-02 | Story 04.2 Edge Case | Cách ly hoàn toàn sandbox iframe không cho phép mã nguồn preview truy cập cookie/localStorage của Workbench mẹ. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 04.2.3 |
| EC-E04-03 | Story 04.3 Edge Case | Khi phần tử có data-ff-id bị xóa hoặc di chuyển sang màn hình khác, tự động chuyển bình luận sang trạng thái stale_mapping kèm gợi ý gắn lại neo. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 04.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 04 (Flow-First XYFlow Canvas, State-Hydrated Preview & Element-Anchored Comments) | Review Agent & Advisor |
