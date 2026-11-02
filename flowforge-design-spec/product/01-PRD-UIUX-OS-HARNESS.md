# PRD — UI/UX OS: Enterprise Agent Harness Operating System & HTML Presentation Workbench

**Document:** `01-PRD-UIUX-OS-HARNESS.md`  
**Version:** 2.0 (Comprehensive Architecture & Workbench Expansion)  
**Status:** Approved by Architecture Council  
**Owner:** Product Leadership & AI Systems Architecture  
**Parent Workspace:** `/Users/hatrang20061988/Desktop/AI Project/uxui-os/flowforge-design-spec`  
**Target Delivery:** Standalone OS Application + First-Class MCP Plugin for Cowok.ai  

---

## 1. Executive Summary & Vision

UI/UX OS (FlowForge) không đơn thuần là một công cụ vẽ giao diện (design tool) hay một công cụ sinh mã HTML cho Agent. UI/UX OS được định vị là một **Agent Harness Operating System** toàn diện cho toàn bộ vòng đời thiết kế, trình diễn và kỹ thuật giao diện số của doanh nghiệp:
1. **Về Nền tảng:** Áp dụng nguyên lý Harness Engineering (*"Agent = Model + Harness"*), cung cấp bộ giàn giáo thực định (Deterministic Scaffolding) đóng khung quanh LLM để quản lý Context Window, Tool Registry, Plugin Framework, Deterministic Evals, và Dynamic Prompt Assembly.
2. **Về Nghiệp vụ Đa Chiều:** Mở rộng từ thiết kế ứng dụng phần mềm (Transactional Web/Mobile Apps) sang **HTML-Native Presentation & Reporting Workbench**. Thay thế hoàn toàn PowerPoint / Google Slides bằng HTML tương tác nguyên bản – định dạng duy nhất mà AI Agent có thể đọc, AST diff, sửa đổi số liệu, kiểm thử tự động và version control pixel-perfect.
3. **Về Trải nghiệm Đa Đối tượng:** Cung cấp giao diện **Dual-Pane (CMS Texting Pane + Live Presentation/Preview)** cho phép người dùng phi kỹ thuật (Non-tech Founder, Marketer, Sales, PM) chỉnh sửa câu chữ, wording, số liệu KPI trực quan hoặc ra lệnh cho Agent thay đổi nội dung mà không phải chạm vào một dòng mã code nào.

---

## 2. Bài toán Thị trường & Khuyết tật của Mô hình Truyền thống

### 2.1 Điểm yếu Chết người của Slide Truyền thống (PPTX / Google Slides)
- **Agent Blindness:** File PPTX là các archive nhị phân XML lồng nhau (`.pptx` là file zip chứa hàng trăm file XML phân mảnh). Khi LLM cố gắng sửa đổi, tỷ lệ làm hỏng file (XML corrupt), nhảy layout hoặc mất định dạng font chữ lên tới hơn 60%.
- **Không có Version Control & Reusability:** Không thể git diff hoặc code review nội dung slide. Mỗi lần cập nhật Brand Guideline, toàn bộ slide của các team phải sửa tay thủ công.
- **Tĩnh và Vô hồn:** Không nhúng được biểu đồ sống (live charts), dữ liệu từ API thời gian thực, hoặc các linh kiện tương tác (interactive widgets).

### 2.2 Điểm yếu của Prompt-to-Code & Design Tool Hiện nay
- **Ảo giác & Lệch chuẩn (Context Drift):** Chỉ dựa vào `DESIGN.md` sơ sài dẫn đến việc Agent tự bịa mã màu hex, tự chế component riêng cho từng trang, khiến hệ thống đa portal (Superadmin vs Tenant Admin vs Sales Web) trở nên hỗn loạn.
- **Token Bloat & Lãng phí Chi phí:** Nhồi toàn bộ tài liệu PRD và Design System vào prompt gây hiện tượng "Lost in the Middle", làm Agent giảm 50% độ chính xác suy luận và tốn kém chi phí token không cần thiết.

---

## 3. Hệ Chân dung Người dùng & Use Cases Đa Nhóm (Cross-Team JTBD)

Cùng một tài sản thương hiệu gốc (**Brand Guideline & Design Tokens**), UI/UX OS phục vụ 3 nhóm đối tượng nghiệp vụ với các luồng công việc đan xen:

```
                  ┌────────────────────────────────────────────────────────┐
                  │    CENTRAL BRAND GUIDELINE & DESIGN TOKEN SSOT        │
                  │    (Colors, Typography, Radii, Shadows, Iconography)   │
                  └──────────────────────────┬─────────────────────────────┘
                                             │
         ┌───────────────────────────────────┼───────────────────────────────────┐
         │                                   │                                   │
         ▼                                   ▼                                   ▼
┌───────────────────────┐         ┌───────────────────────┐         ┌───────────────────────┐
│     UI/UX & DEV       │         │       MARKETING       │         │   EXECUTIVE & SALES   │
│  - App Portals        │         │  - HTML Landing Pages │         │  - HTML Presentations │
│  - Multi-Screen Flows │         │  - Interactive Tour   │         │  - KPI Report Decks   │
│  - State Machine DAG  │         │  - Customer Docs HTML │         │  - Stakeholder Pitches│
│  - Code PR / Export   │         │  - Social Visuals     │         │  - Dual-Pane CMS Text │
└───────────────────────┘         └───────────────────────┘         └───────────────────────┘
```

1. **Nhóm UI/UX & Kỹ thuật (Application Engineering):**
   - *Job:* Thiết kế luồng đa màn hình (Flow-first), xây dựng giao diện đa portal (Superadmin, Tenant Admin, Mobile Van Sales, Sales Web) với tính năng xác minh code TypeScript và build test tự động.
2. **Nhóm Marketing & Growth (Customer-Facing Collateral):**
   - *Job:* Xây dựng các trang giới thiệu sản phẩm bằng HTML tương tác, tài liệu hướng dẫn sử dụng (Customer Knowledge Base HTML), landing page sự kiện với 100% đúng chuẩn nhận diện thương hiệu.
3. **Nhóm Executive, Sales & Operations (Interactive Presentation & Reporting):**
   - *Job:* Dựng slide thuyết trình bán hàng (Sales Pitch Decks), báo cáo định kỳ cho ban giám đốc (Executive KPI Dashboards) bằng HTML.
   - *Hành vi:* Dùng giao diện **Dual-Pane CMS** để gõ chỉnh sửa nội dung văn bản bên trái, nhìn thấy slide đổi ngay bên phải; hoặc gõ prompt: *"Cập nhật doanh thu tháng 10 lên 4.5 tỷ và tóm tắt 3 thách thức chính"* để Agent tự cập nhật số liệu và biểu đồ.

---

## 4. Danh mục Yêu cầu Tính năng (Functional Requirements - FR)

### Nhóm A: Kiến trúc Harness & Context Management
* **FR-H01 (Token Budget Governor):** Tự động phân bổ ngân sách token theo pha (Discovery, Specification, Code Generation, Verification). Không bao giờ vượt quá ngân sách đã định.
* **FR-H02 (Progressive Disclosure Menu):** Chỉ cung cấp danh mục tóm tắt (Component Catalog Menu). Chỉ nạp interface chi tiết của component khi Agent phát tín hiệu cần dùng.
* **FR-H03 (Dual Context Management):** Quản lý độc lập *Durable Project State* (Git-tracked Specs, Brand Guidelines, Schemas) và *Ephemeral Preview State* (mã nguồn tạm, iframe cache).

### Nhóm B: Chuẩn hóa Bộ 9 Project Context Specs
* **FR-S01 (Ingestion & Spec Synthesis Engine):** Tiếp nhận PRD, User Stories, hoặc tài liệu thô từ Cowok.ai; tự động phân rã và sinh đủ Bộ 9 Specs chuẩn hóa:
  - `01_PROJECT_BRIEF.md`, `02_BRAND_IDENTITY.md`, `03_DESIGN_SYSTEM.md`, `04_PORTAL_TOPOLOGY.md`, `05_FEATURE_MAP_DAG.md`, `06_LAYOUT_RESPONSIVE.md`, `07_COMPONENT_REGISTRY.md`, `08_TECH_STACK_CONTRACT.md`, `09_EVAL_SPEC.md`.
* **FR-S02 (Design Spec Registry):** Lưu trữ, quản lý phiên bản (semver `v1.0.0`) và gắn thẻ (tagging) cho từng bộ spec, cho phép tái sử dụng giữa các dự án.
* **FR-S03 (Just-In-Time Dynamic Prompt Assembly):** Khi một Agent thực thi một task cụ thể, hệ thống chỉ trích xuất đúng lát cắt đặc tả liên quan (Portal + Component + Color Token), ghép thành prompt tinh gọn ~1,500 token (tiết kiệm 80% chi phí).

### Nhóm C: HTML Presentation & Reporting Workbench (Nghiệp vụ Mới)
* **FR-P01 (HTML Presentation Engine):** Hỗ trợ render bài thuyết trình chuẩn HTML với hiệu ứng chuyển trang mượt mà (GSAP / View Transitions API), hỗ trợ tỷ lệ 16:9, toàn màn hình (F11), điều khiển bằng phím mũi tên/spacebar.
* **FR-P02 (Dual-Pane CMS Interface):**
  - Khung trái (CMS Texting Pane): Giao diện thân thiện cho non-tech users chứa các trường tiêu đề, nội dung bullet points, số liệu metrics, ghi chú người thuyết trình (Speaker Notes), cùng ô chat chỉ thị Agent.
  - Khung phải (Live Presentation Iframe): Render slide trực tiếp theo thời gian thực (Hot-reload dưới 300ms).
* **FR-P03 (Interactive Reporting & Live Data Binding):** Hỗ trợ nhúng biểu đồ động (Chart.js / Vega-Lite / Recharts) cho phép hiển thị số liệu kinh doanh sống, lọc theo khoảng thời gian ngay trong lúc trình chiếu.
* **FR-P04 (Headless Deterministic Slide Export):** Xuất bản slide thành PDF chất lượng cao (in ấn vector) hoặc HTML độc lập chạy offline (Self-contained HTML package).

### Nhóm D: Tích hợp MCP Plugin cho Cowok.ai
* **FR-M01 (MCP Server Standard):** Cung cấp giao thức `stdio` (cục bộ) và `SSE/HTTP` (mạng nội bộ) hỗ trợ các tools cấp cao:
  - `design.create_project`, `design.generate_flow`, `design.inspect_screen`, `design.propose_diff`, `design.create_presentation`, `design.update_slide_content`, `design.export_artifact`.
* **FR-M02 (BYOS CLI Relay):** Tích hợp bộ chuyển tiếp tiến trình con cho `codex`, `claude`, `antigravity` với cơ chế File Descriptor Staging (`0600`) và Zombie Reaper tự động.

---

## 5. Tiêu chuẩn Kỹ thuật Phi chức năng (NFR & SLO)

1. **Hiệu năng Render Live Preview:** Thời gian cập nhật khi sửa text trên CMS Pane (p95) < 300ms. Thời gian khởi động lại iframe sau khi thay đổi component (p95) < 1.5s.
2. **Độ an toàn Mã nguồn (Sandbox Isolation):** Toàn bộ mã nguồn preview chạy trong sandbox iframe có thuộc tính `sandbox="allow-scripts allow-forms allow-same-origin"`. Không có quyền truy cập vào cookie hay localStorage của host app.
3. **Tuân thủ Chuẩn Trợ năng (Accessibility):** 100% template và slide xuất bản phải đạt chuẩn WCAG 2.2 AA (độ tương phản chữ/nền tối thiểu 4.5:1).
4. **Tính Bền vững Dữ liệu (Zero Code-Corrupt):** Các thao tác sửa đổi từ CMS Pane tuyệt đối không phá vỡ cây AST của file mã nguồn gốc.
