# Specification: HTML-Native Presentation & Dual-Pane CMS Workbench

**Document:** `07-HTML-PRESENTATION-CMS-SPEC.md`  
**Parent Workspace:** `/Users/hatrang20061988/Desktop/AI Project/uxui-os/flowforge-design-spec`  
**Status:** Approved Specification  
**Version:** 1.0  
**Target:** Multi-Team Presentation & Reporting Subsystem  

---

## 1. Mục đích & Nguyên lý Thiết kế

Tài liệu này định nghĩa chi tiết phân hệ **HTML-Native Presentation & Reporting Workbench** của UI/UX OS. 

Phân hệ này giải quyết triệt để sự đứt gãy giữa:
- **Người dùng phi kỹ thuật (Non-tech Founder, Marketer, Sales, PM):** Muốn sửa nội dung, câu chữ (wording), số liệu báo cáo một cách đơn giản, trực quan như gõ tin nhắn/CMS mà không phải nhìn thấy code HTML/React.
- **AI Coding Agent:** Cần một cấu trúc dữ liệu tường minh (JSON AST / Clean HTML) để có thể đọc hiểu, cập nhật số liệu, diff và kiểm thử tự động mà không làm hỏng cấu trúc file như file nhị phân PPTX.
- **Tính Nhất quán Thương hiệu (Cross-Team Brand Consistency):** Đảm bảo cả trang web app, slide thuyết trình bán hàng và tài liệu giới thiệu đều dùng chung 1 hệ thống Token màu sắc, font chữ và phong cách từ Brand Guideline.

---

## 2. Kiến trúc Giao diện Dual-Pane (CMS Texting Pane + Live Presentation)

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ UI/UX OS - PRESENTATION & REPORTING WORKBENCH                                         │
├──────────────────────────────────┬─────────────────────────────────────────────────────┤
│ KHUNG TRÁI: CMS & TEXTING PANE   │ KHUNG PHẢI: LIVE INTERACTIVE PRESENTATION           │
│ (Dành cho Human & Agent Chat)   │ (Render Iframe 16:9 - Chuẩn HTML & Biểu đồ Động)   │
├──────────────────────────────────┼─────────────────────────────────────────────────────┤
│ 📑 Slide 03 / 12: Business Model │                                                     │
│                                  │   ┌─────────────────────────────────────────────┐   │
│ [Slide Title]                    │   │  OUR BUSINESS MODEL & UNIT ECONOMICS        │   │
│ Enterprise B2B SaaS             │   │  Scalable Subscription + Usage Hybrid       │   │
│                                  │   │                                             │   │
│ [Subtitle / Hook]                │   │  ┌──────────────┐      ┌─────────────────┐  │   │
│ Hybrid Platform + Add-ons        │   │  │   ARR Growth │      │ Retention (NDR) │  │   │
│                                  │   │  │    $4.8M     │      │     135%        │  │   │
│ [Key Metric 1] [Value]           │   │  │   +120% YoY  │      │  Enterprise Net │  │   │
│ ARR Growth   | $4.8M (+120%)     │   │  └──────────────┘      └─────────────────┘  │   │
│                                  │   │                                             │   │
│ [Key Metric 2] [Value]           │   │  • Tier 1: Platform Fee ($2,000/mo)         │   │
│ Retention    | 135% NDR          │   │  • Tier 2: Consumption-based Compute        │   │
│                                  │   │  • Enterprise SLA & Custom Deployment       │   │
│ [Bullet Points]                  │   │                                             │   │
│ • Tier 1: Platform Fee ($2,000)  │   │                                             │   │
│ • Tier 2: Consumption Compute    │   │                                             │   │
│ • Enterprise SLA Support         │   │                                             │   │
│                                  │   │  [Live Chart: Monthly Revenue Run-Rate]     │   │
│ ──────────────────────────────── │   └─────────────────────────────────────────────┘   │
│ 🤖 AGENT COPILOT CHAT BOX:       │                                                     │
│ [User]: "Đổi số liệu ARR lên     │   ◀ Slide trước (Left)  |  Slide tiếp (Right) ▶     │
│ $5.2M và làm ngắn lại 3 bullet"  │   [Phím Space: Next] [F11: Toàn màn hình] [PDF]     │
│ [Agent]: "Đã cập nhật slide 03!" │                                                     │
└──────────────────────────────────┴─────────────────────────────────────────────────────┘
```

### 2.1 Chi tiết Khung Trái (CMS & Texting Pane)
1. **Trình sửa Đổi Thuộc tính theo Form (Structured Field Editor):**
   - Không bắt user gõ thẻ `<h1>` hay `<div>`.
   - Cung cấp các trường nhập liệu thân thiện: `Slide Title`, `Subtitle`, `Badges`, `Metrics List`, `Body Bullets`, `Call-to-Action Text`, `Speaker Notes`.
   - Mỗi thay đổi trên form lập tức phát sự kiện `onChange` bắn sang iframe thông qua `postMessage`.
2. **Hộp thoại Agent Copilot Tích hợp:**
   - Cho phép người dùng nhập yêu cầu bằng ngôn ngữ tự nhiên:
     - *"Chuyển slide này sang tone giọng trang trọng cho nhà đầu tư"*
     - *"Rút gọn 5 gạch đầu dòng này thành 3 điểm cốt lõi ngắn gọn"*
     - *"Thay thế biểu đồ đường bằng bảng so sánh 3 cột giữa chúng ta và đối thủ"*
   - Agent nhận yêu cầu, cập nhật file dữ liệu JSON của slide, và iframe tự động chuyển đổi giao diện trong chưa đầy 1 giây.

### 2.2 Chi tiết Khung Phải (Live HTML Presentation Iframe)
1. **Chuẩn Hiển thị Tương tác:**
   - Tỷ lệ khung hình chuẩn 16:9 (1920x1080), tự động scale theo kích thước màn hình mà không bị méo font chữ.
   - Hỗ trợ đầy đủ phím điều hướng: Mũi tên trái/phải, Phím Space, Phím PageUp/PageDown, Phím `F` (Fullscreen).
2. **Khả năng Nhúng Dữ liệu Sống (Live Interactive Widgets):**
   - Nhúng trực tiếp các component React thật: Bảng dữ liệu có thể lọc, biểu đồ Chart.js / Recharts có animation, mô hình 3D tương tác (`Three.js`), và code preview có syntax highlighting.
3. **Presenter Mode (Chế độ Người Thuyết trình):**
   - Hỗ trợ mở cửa sổ phụ thứ hai dành cho người thuyết trình: Hiển thị slide hiện tại, slide kế tiếp, đồng hồ bấm giờ và **Speaker Notes**.

---

## 3. Cấu trúc Dữ liệu Chuẩn hóa (Slide JSON AST Schema)

Để Agent và CMS Pane có thể giao tiếp hai chiều hoàn hảo, toàn bộ bài thuyết trình được mô hình hóa thành một file JSON chuẩn mực (`presentation.json`):

```json
{
  "$schema": "https://flowforge.design/schemas/presentation-v1.json",
  "meta": {
    "title": "Cowok.ai Q4 Strategic Roadmap",
    "theme": "cowok-brand-dark",
    "aspectRatio": "16:9",
    "author": "Product Strategy Team",
    "version": "1.2.0"
  },
  "slides": [
    {
      "id": "slide-01",
      "layout": "hero-cover",
      "title": "Cowok.ai: The Autonomous Enterprise",
      "subtitle": "Unifying Agents, Workflows, and Zero-Trust SDLC",
      "presenterNotes": "Chào mừng các lãnh đạo và nhà đầu tư tham dự buổi chia sẻ chiến lược Q4.",
      "elements": {
        "badge": "Confidential • Internal Only",
        "date": "November 2026",
        "cta": "Explore the Architecture"
      }
    },
    {
      "id": "slide-03",
      "layout": "metrics-and-bullets",
      "title": "Business Model & Unit Economics",
      "subtitle": "Scalable Subscription + Compute Hybrid",
      "presenterNotes": "Nhấn mạnh vào tỷ lệ Net Dollar Retention đạt 135% của nhóm khách hàng Enterprise.",
      "metrics": [
        { "label": "ARR Growth", "value": "$4.8M", "sub": "+120% YoY", "trend": "positive" },
        { "label": "Retention", "value": "135%", "sub": "Enterprise NDR", "trend": "positive" }
      ],
      "bullets": [
        "Tier 1: Fixed Platform Subscription ($2,000/month)",
        "Tier 2: Consumption-based Compute & Model Execution",
        "Tier 3: Dedicated Private Cloud & Enterprise SLA"
      ],
      "chart": {
        "type": "bar",
        "dataSource": "fixtures/revenue-growth-q3.json"
      }
    }
  ]
}
```

---

## 4. Quản lý Đa Ngữ Cảnh & Đa Dự Án (Context Orchestration Matrix)

UI/UX OS thiết lập cơ chế phân vùng ngữ cảnh theo thời gian và phạm vi dự án:

| Thời lượng Tác vụ | Ví dụ Điển hình | Nguồn Context Cần Lấy | Ngân sách Token |
|---|---|---|:---:|
| **Tác vụ Ngắn (Short-Task)** | - Sửa câu chữ trong slide 03<br>- Đổi màu nút bấm sang primary<br>- Cập nhật 1 con số KPI | Chỉ nạp schema của đúng slide đó + danh mục token màu ngắn gọn | **~1,000 – 1,500 tokens** |
| **Tác vụ Trung bình (Mid-Task)** | - Thêm 1 slide mới về Đối thủ<br>- Tạo form nhập liệu mới trong Portal<br>- Chuyển đổi biểu đồ sang đồ thị tròn | Nạp layout spec tương ứng + danh mục component chart + slide JSON hiện tại | **~3,000 – 5,000 tokens** |
| **Tác vụ Dài (Long-Task)** | - Khởi tạo toàn bộ bộ slide 15 trang từ PRD<br>- Dựng luồng onboarding 5 màn hình<br>- Tái cấu trúc Brand Guideline toàn bộ | Nạp đầy đủ PRD, Brand Guideline, Portal Topology, và kích hoạt Multi-Agent Planning | **~15,000 – 25,000 tokens** |

### Nguyên tắc Cô lập & Chống Trộn lẫn (Multi-Project Isolation):
- Mỗi dự án (ví dụ: `Project Cowok-ai` vs `Project Distro` vs `Marketing Campaign 2026`) sở hữu một **Context Vault** riêng biệt với UUID độc lập.
- Khi làm việc trên bài thuyết trình của Marketing, Agent bị cấm truy cập vào database schema của backend hay các story kỹ thuật của engineering, tránh tuyệt đối hiện tượng rò rỉ bối cảnh và sinh nhầm mã nguồn.
