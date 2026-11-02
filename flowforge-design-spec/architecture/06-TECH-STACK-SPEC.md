# Technical Specification: Monorepo Architecture & Tech Stack Contract

**Document:** `06-TECH-STACK-SPEC.md`  
**Parent Workspace:** `/Users/hatrang20061988/Desktop/AI Project/uxui-os/flowforge-design-spec`  
**Status:** Approved Technical Architecture  
**Version:** 2.0  
**Target:** Monorepo Workspace `/Users/hatrang20061988/Desktop/AI Project/uxui-os/`  

---

## 1. Triết lý Công nghệ & Nguyên tắc Không Phân mảnh (Anti-Fragmentation)

Để tránh tình trạng "phân mảnh kiến trúc" (Architectural Split-Brain), UI/UX OS thiết lập các quy tắc bất biến:
1. **Frontend Bất biến:** 100% là **Vite v8 + React 19 SPA** (Tuyệt đối không dùng Next.js, tránh các lỗi liên quan đến Server Components / SSR Hydration trong môi trường Sandbox preview).
2. **Backend Daemon Bất biến:** **Fastify v5 (TypeScript)** – tốc độ cao, hỗ trợ SSE thời gian thực, tích hợp SQLite (WAL mode) để quản lý Spec Registry cục bộ.
3. **Giao tiếp Chuẩn hóa:** Sử dụng **Model Context Protocol (MCP)** làm cầu nối duy nhất giữa UI/UX OS và Cowok.ai.

---

## 2. Cấu trúc Monorepo (pnpm Workspaces)

```
uxui-os/
├── apps/
│   ├── web/                        # React 19 SPA Frontend
│   │   ├── src/
│   │   │   ├── components/         # Canvas, Preview Iframe, CMS Texting Pane
│   │   │   ├── pages/              # FlowEditor, PresentationStudio, SpecManager
│   │   │   ├── store/              # Zustand state store
│   │   │   └── App.tsx
│   │   └── vite.config.ts
│   │
│   └── api/                        # Fastify v5 Harness Daemon
│       ├── src/
│       │   ├── routes/             # REST & SSE endpoints
│       │   ├── byos/               # CLI Process Manager & 0600 Prompt Stager
│       │   └── server.ts
│       └── package.json
│
├── packages/
│   ├── domain/                     # Zod Schemas cho 9 Specs, Presentation & Flows
│   ├── spec-registry/              # Lưu trữ, Versioning & Query Engine cho Specs
│   ├── prompt-assembler/           # JIT Dynamic Prompt Assembly Engine
│   ├── flow-engine/                # Máy trạng thái hữu hạn & Fixture Data Hydration
│   ├── presentation-engine/        # Slide Renderers, HTML5 View Transitions, CMS Sync
│   ├── eval-harness/               # Playwright, Token Linter, WCAG Assertions
│   ├── mcp-server/                 # MCP Server (Stdio + SSE) cung cấp tools cho Cowok.ai
│   └── template-starter/           # Template Vite React 19 chuẩn tích hợp Cowok Tokens
│
├── pnpm-workspace.yaml
├── package.json
└── tsconfig.base.json
```

---

## 3. Chi tiết Tech Stack cho Từng Phân hệ

| Phân hệ | Công nghệ Lựa chọn | Lý do Lựa chọn & Ưu thế |
|---|---|---|
| **Giao diện Người dùng (UI)** | **React 19 + Vite v8** | Khởi động tức thì (HMR < 100ms), tương thích hoàn hảo với React Compiler và iframe sandbox. |
| **Sơ đồ Máy Trạng thái** | **@xyflow/react (v12)** | Thư viện node-graph tốt nhất cho React, quản lý luồng màn hình `ScreenState` và `Transition` edges. |
| **Hệ thống Style & Tokens** | **Tailwind CSS v4** | Hỗ trợ CSS variables bản địa không cần file config phức tạp, biên dịch trực tiếp từ `DESIGN.md`. |
| **Trình diễn Slide HTML** | **HTML5 View Transitions + GSAP** | Chuyển trang mượt mà như video, tỷ lệ chuẩn 16:9, dễ dàng nhúng biểu đồ và code interactive. |
| **Trực quan hóa Dữ liệu** | **Recharts + Lucide Icons** | Biểu đồ động có thể bind data sống, phù hợp cho cả dashboard ứng dụng và slide báo cáo. |
| **Backend Daemon** | **Fastify v5 + Zod** | Xử lý high-throughput, latency dưới 5ms, type-safety từ schema Zod sang TypeScript. |
| **Cơ sở Dữ liệu Cục bộ** | **better-sqlite3 (WAL mode)** | Không cần setup server db phức tạp, đọc/ghi siêu nhanh, lưu trữ Spec Registry và lịch sử phiên bản. |
| **MCP Integration** | **@modelcontextprotocol/sdk** | Chuẩn kết nối mở được Anthropic, Google, và Cowok.ai hỗ trợ toàn diện qua `stdio` và `SSE`. |
| **BYOS Process Engine** | **Node.js `child_process` (POSIX)** | Quản lý tiến trình độc lập (`detached: true`), File descriptor prompt staging (`0600`), diệt Zombie process. |
| **Kiểm định Chất lượng** | **Playwright + TypeScript AST** | Kiểm thử giao diện thực định không qua mắt thường: Assert DOM, contrast WCAG 2.2, kiểm tra lỗi console. |

---

## 4. Đặc tả Hợp đồng Giao tiếp MCP (MCP Tool Contract cho Cowok.ai)

UI/UX OS đăng ký 6 MCP Tools chính để Cowok.ai có thể điều khiển:

1. `design.create_project`: Khởi tạo dự án thiết kế mới với bộ tokens từ Brand Guideline.
2. `design.synthesize_specs`: Nhận PRD từ Cowok.ai và tự động sinh đủ Bộ 9 Project Context Specs.
3. `design.generate_flow`: Tạo luồng trạng thái đa màn hình trên XYFlow Canvas.
4. `design.inspect_screen`: Chụp ảnh, trích xuất HTML hydrate và cây DOM của màn hình đang chọn.
5. `design.create_presentation`: Tạo bài thuyết trình HTML chuẩn từ PRD hoặc đề cương báo cáo.
6. `design.update_slide_content`: Cập nhật câu chữ, wording hoặc số liệu của slide từ prompt hoặc CMS Pane.
7. `design.approve_and_export`: Xuất mã nguồn sạch, tạo Git Pull Request hoặc tải trọn gói file nén.
