# Epic 02 FMEA Risk Matrix — JIT Prompt Assembler & 3-Profile Model Capability Adapter

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E02-01 | Story 02.1 Edge Case | Ngăn chặn rò rỉ ngữ cảnh của Portal SuperAdmin sang bài thuyết trình Marketing công khai. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 02.1.3 |
| EC-E02-02 | Story 02.2 Edge Case | Tự động nén danh sách component/icon theo độ liên quan ngữ nghĩa nếu lát cắt ban đầu vượt quá 1,500 tokens. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 02.2.3 |
| EC-E02-03 | Story 02.3 Edge Case | Tự động hạ cấp xuống Profile B với micro-tools định sẵn nếu model nhỏ gặp lỗi cú pháp khi gọi tool phức tạp. | 4 | 4 | 2 | **32** | Enforced in AC-03 & Task 02.3.3 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 02 (JIT Prompt Assembler & 3-Profile Model Capability Adapter) | Review Agent & Advisor |
