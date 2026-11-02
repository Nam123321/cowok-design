# ADR-003: Architecture Decision Record — Project-Scoped Configurable Design Constraint Engine (`crates/ast-guardian` & `crates/jev-engine`)

**Status:** ACCEPTED  
**Date:** 2026-11-02  
**Deciders:** UI/UX Lead, AI Systems Architect, Brand Guardian  

## 1. Context
Trong thực tế phát triển sản phẩm đa dự án, mỗi dự án có một bộ quy chuẩn kỹ thuật và thương hiệu riêng biệt (ví dụ: dự án A cấm tuyệt đối Tailwind CSS và khóa cứng 145 SVG Icons từ `cowok-brand-product-system-v4.3`; dự án B cho phép CSS Modules kết hợp bộ icon riêng của khách hàng; dự án C quy định danh mục component và kiểu chữ đặc thù). Nếu không có một bộ máy thực thi ràng buộc cấu hình động theo từng dự án (Project-Scoped Constraint Engine), AI Agent sẽ tự ý sinh mã vi phạm quy chuẩn của dự án đó (bịa mã màu hex, import sai thư viện icon, dùng sai hệ thống CSS).

## 2. Decision
1. **Project-Scoped Configurable Constraint Profile (`PROJECT_CONSTRAINTS`):** Khi khởi tạo hoặc nạp một dự án vào UI/UX OS, các quy tắc ràng buộc của dự án đó được trích xuất từ bộ 9 Foundational Specs (`02_BRAND_IDENTITY`, `07_COMPONENT_REGISTRY`, `08_TECH_STACK_CONTRACT`, `09_EVAL_SPEC`) thành một cấu hình thực thi xác định:
   - **Styling Policy:** `vanilla-css-tokens-only` (Zero-Tailwind) | `css-modules` | `custom-token-contract`.
   - **Token Registry Contract:** Danh sách biến CSS hợp lệ từ file `tokens.css` của dự án; chặn mã màu hex/rgb cứng ngoài danh mục.
   - **Iconology Manifest Contract:** Danh sách biểu tượng hợp lệ từ `icon-manifest.json` của dự án (với `cowok-brand-product-system-v4.3` gồm 145 SVG Icons là gói cấu hình chuẩn mặc định / Flagship Preset).
   - **Typography, Layout & Viewport Invariants:** Quy tắc thang chữ, lưới bố cục, chuẩn tương phản WCAG 2.2 AA ($\ge 4.5:1$), và tỷ lệ khung trình chiếu `16:9`.
2. **Sub-Millisecond Mechanized Enforcement (`oxc` AST + `Jev Noul` <1ms):** `crates/ast-guardian` và `crates/jev-engine` nạp cấu hình ràng buộc của dự án đang mở để quét và chặn tức thì mọi mã nguồn vi phạm trước khi ghi xuống đĩa hoặc hiển thị lên Preview.
3. **Self-Healing Constraint Feedback:** Khi phát hiện vi phạm, bộ kiểm duyệt trả về vị trí dòng/cột chính xác kèm gợi ý thay thế hợp lệ trong phạm vi dự án để Agent tự động sửa lỗi.

## 3. Consequences
- Đảm bảo tính linh hoạt đa dự án (Multi-Project Flexibility) đồng thời duy trì kỷ luật tuân thủ 100% đối với từng dự án cụ thể.
- Bộ quy chuẩn `cowok-brand-product-system-v4.3` (Zero-Tailwind + 145 Brand Icons) được đóng gói làm Flagship Preset sẵn sàng sử dụng ngay cho toàn bộ hệ sinh thái Cowok.ai.
