# ADR-001: Architecture Decision Record — UI/UX OS as an Agent Harness Operating System

**Status:** ACCEPTED  
**Date:** 2026-11-02  
**Deciders:** AI Systems Architect, Product Leadership, Quality Assurance Council  
**Technical Scope:** Core System Architecture, Multi-Agent Coordination, Execution Runtime  

---

## 1. Context and Problem Statement
Trong các dự án kỹ thuật phần mềm phức tạp như Cowok.ai, các LLM Coding Agent thường gặp phải 3 khuyết tật mang tính hệ thống khi được giao nhiệm vụ thiết kế và lập trình giao diện:
1. **Ảo giác & Lệch chuẩn Thiết kế (Design Drift):** Khi chỉ dựa vào một file markdown tĩnh (`DESIGN.md`), Agent tự chế các class CSS, gõ mã hex tùy tiện và sáng tạo ra các cấu trúc thẻ `div` lồng nhau không thể bảo trì.
2. **Context Dilution & Token Bloat:** Việc nhồi nhét tài liệu PRD hàng chục trang và toàn bộ catalog component vào một prompt duy nhất làm tràn context window, giảm sút độ tập trung của mô hình (Lost in the Middle) và tiêu tốn chi phí token khổng lồ.
3. **Sự Đứt gãy giữa Thiết kế Ứng dụng & Trình diễn Nghiệp vụ:** Doanh nghiệp phải duy trì hai luồng riêng biệt: Một luồng code app (React/TypeScript) và một luồng làm slide báo cáo/thuyết trình (PowerPoint/Google Slides). Định dạng PPTX nhị phân không tương thích với Agent, khiến việc tự động hóa cập nhật số liệu và thiết kế báo cáo trở nên bất khả thi.

---

## 2. Decision: Xây dựng UI/UX OS dưới dạng "Agent Harness Operating System"

Chúng tôi quyết định kiến trúc lại UI/UX OS không phải là một công cụ sinh UI thông thường, mà là một **Harness OS độc lập** với 5 trụ cột then chốt:

```
┌─────────────────────────────────────────────────────────────────┐
│                    ADR-001 ARCHITECTURAL PILLARS                │
├─────────────────────────────────────────────────────────────────┤
│ 1. Harness Over Model: "Agent = Model + Harness" (Addy Osmani)  │
│ 2. The 9 Project Context Specs SSOT (Thay thế DESIGN.md đơn lẻ) │
│ 3. Spec Registry & JIT Dynamic Prompt Assembly                  │
│ 4. Native HTML Presentation & Dual-Pane CMS Workbench           │
│ 5. Hybrid BYOK + BYOS CLI Execution Core (0600 Prompt Staging)  │
└─────────────────────────────────────────────────────────────────┘
```

1. **Harness Over Model:** Xác lập nguyên lý "Harness quyết định chất lượng". Mọi sự tương tác của LLM đều bị đóng khung và bảo vệ bởi các rào chắn thực định: Linter quét token, TypeScript compiler, Playwright WCAG assertions, và sandboxed iframe previews.
2. **Bộ 9 Project Context Specs (Thay thế `DESIGN.md` đơn lẻ):**
   - Phân rã ngữ cảnh dự án thành 9 tài liệu đặc tả có cấu trúc máy đọc được: `Project Brief`, `Brand Identity`, `Design System Tokens`, `Portal Topology`, `Feature Map DAG`, `Layout & Responsive Spec`, `Component Registry`, `Tech Stack Contract`, và `Eval Spec`.
3. **Spec Registry & Just-In-Time (JIT) Prompt Assembly:**
   - Đóng gói các đặc tả thành một Registry có versioning.
   - Khi một Agent nhận task, hệ thống chỉ trích xuất đúng lát cắt đặc tả liên quan (ví dụ: chỉ lấy `Portal: Tenant Admin` + `Component: DataTableView` + `Tokens: Surface`), nạp ~1,500 token thay vì 60,000 token dư thừa.
4. **HTML-Native Presentation & Dual-Pane CMS:**
   - Chuẩn hóa toàn bộ bài thuyết trình và báo cáo định kỳ sang định dạng HTML tương tác nguyên bản lưu trữ dưới dạng JSON AST (`presentation.json`).
   - Cung cấp giao diện Dual-Pane: Khung trái là CMS gõ text/chat với Agent cho người dùng phi kỹ thuật; khung phải là slide HTML trình diễn thời gian thực.
5. **Hybrid BYOK + BYOS Engine:**
   - Hỗ trợ BYOK (API key trực tiếp) cho môi trường cloud.
   - Hỗ trợ BYOS (External CLI Delegation sang `codex`, `claude`, `antigravity`) bằng kỹ thuật File Descriptor Prompt Staging (`0600`) để tận dụng thuê bao cố định của người dùng với 0 chi phí token API.

---

## 3. Consequences & Trade-offs

### Positive Consequences (Lợi ích Đạt được)
- **Zero Design Drift:** Toàn bộ mã nguồn sinh ra bắt buộc phải dùng các biến token CSS đã đăng ký và các component chuẩn trong Registry.
- **Tiết kiệm 80% Chi phí Token:** Nhờ cơ chế JIT Prompt Assembly, kích thước context giảm từ 60K token xuống 1.5K token, tốc độ sinh mã nhanh hơn 4 lần.
- **Dễ dùng cho Non-Tech Users:** Cho phép founder, PM, marketer sửa slide và copy trực tiếp trên CMS Pane mà không sợ làm vỡ code.
- **Khả năng Tích hợp Hoàn hảo:** Đóng vai trò là MCP Plugin cho Cowok.ai, cho phép toàn bộ Agent trong hệ sinh thái Cowok.ai gọi các công cụ thiết kế và xuất bản presentation qua chuẩn giao thức MCP.

### Negative Consequences & Mitigation (Thách thức & Biện pháp)
- *Thách thức:* Cần xây dựng và duy trì bộ parser/validator cho 9 tài liệu đặc tả.
  *Biện pháp:* Sử dụng Zod Schemas để tự động kiểm tra tính hợp lệ của từng tài liệu ở cả tầng API lẫn client.
- *Thách thức:* Nguy cơ lệch pha giữa code sửa tay và Spec Registry (Spec-to-Code Drift).
  *Biện pháp:* Tích hợp Reverse AST Scanner tự động quét và đối soát khi có commit code.
