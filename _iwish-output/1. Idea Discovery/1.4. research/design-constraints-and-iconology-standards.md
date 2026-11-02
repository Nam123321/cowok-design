# Specification: Mandatory Design Constraints, Zero-Tailwind Invariant & Brand Iconology Standards

**Document:** `_iwish-output/1. Idea Discovery/1.4. research/design-constraints-and-iconology-standards.md`  
**Status:** Approved Architectural Standard  
**Category:** Zero-Trust Category A Design Governance  
**Author:** Lead UI/UX Systems Architect & Quality Assurance Council  
**Date:** 2026-11-02  

---

## 1. Mục đích & Nguyên tắc Bất biến (The Non-Negotiable Invariants)

Để ngăn chặn triệt để tình trạng hỗn loạn giao diện, mất tính nhất quán thương hiệu và ảo giác mã nguồn của các AI Coding Agent, UI/UX OS thiết lập **3 Rào cản Thiết kế Bắt buộc (Mandatory Design Constraints)**. Mọi đoạn mã do Agent hay lập trình viên viết ra đều phải vượt qua bộ lọc linter trước khi được đưa vào codebase.

---

## 2. Rào cản 1: Điều luật Cấm Tailwind CSS (The Zero-Tailwind Invariant)

### 2.1 Tại sao CẤM Tuyệt đối Tailwind CSS trong UI/UX OS?
1. **Làm Bẩn Cây Cú pháp (AST Pollution):**
   - Khi Agent viết Tailwind, các thẻ HTML bị nhồi nhét hàng chục utility classes:
     ```html
     <!-- BỊ CẤM: Rác cú pháp, làm rối loạn diff và khiến Agent ảo giác -->
     <div class="flex flex-col items-start justify-between p-6 bg-slate-900 border border-slate-800 rounded-2xl shadow-xl hover:border-blue-500 transition-all duration-300">
     ```
   - Điều này làm kích thước context tăng vọt gấp 3 lần, làm giảm khả năng suy luận logic nghiệp vụ của LLM và khiến việc review code diff của con người trở thành ác mộng.
2. **Ảo giác Mã màu & Lệch chuẩn Thương hiệu (Brand Drift):**
   - LLM có xu hướng tự phát minh ra các class tùy tiện (`bg-blue-550`, `text-neutral-950`, `p-[18px]`, `bg-[#1a202c]`). Hệ quả là ứng dụng bị phân mảnh hàng trăm mã màu khác nhau ngoài danh mục cho phép.
3. **Bất khả thi khi Rebranding & Đổi Theme:**
   - Nếu dùng Tailwind, khi doanh nghiệp muốn đổi màu thương hiệu hoặc điều chỉnh độ bo góc từ 8px sang 12px, lập trình viên phải tìm và thay thế hàng triệu class rải rác ở hàng nghìn file.

### 2.2 Chuẩn Mực Thay thế: Pure Vanilla CSS Custom Properties (Tokens CSS)
Toàn bộ hệ thống giao diện và bài thuyết trình của UI/UX OS sử dụng **100% CSS Variables thuần** được biên dịch từ `cowok-brand-product-system-v4.3/foundations/css/tokens.css` kết hợp với **Semantic Class Names**:

```html
<!-- CHUẨN MỰC: Tinh gọn, ngữ nghĩa rõ ràng, diff sạch sẽ 100% -->
<div class="metric-card" data-ff-id="kpi-arr-growth">
  <span class="metric-label">ARR Growth</span>
  <div class="metric-value">$4.8M</div>
  <span class="metric-trend positive">+120% YoY</span>
</div>
```

```css
/* Tệp style chuẩn tắc sử dụng biến CSS thương hiệu */
.metric-card {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg, 16px);
  padding: var(--spacing-6, 24px);
  box-shadow: var(--shadow-sm);
  transition: border-color var(--transition-normal);
}

.metric-card:hover {
  border-color: var(--human-accent);
}

.metric-value {
  color: var(--text-primary);
  font-family: var(--font-display);
  font-size: var(--text-2xl);
  font-weight: 700;
}
```

### 2.3 Cơ chế Linter Kiểm duyệt Tự động (Automated Anti-Tailwind Linter Gate)
Script linter tích hợp trong Harness sẽ quét cây AST của mọi file `.html`, `.jsx`, `.tsx`, `.vue`:
- Quét tìm các chuỗi class phổ biến của Tailwind (`flex`, `grid-cols-`, `p-`, `m-`, `bg-`, `text-slate-`, `rounded-`, `@apply`).
- Quét tìm mã hex cứng dạng `#xxxxxx` hoặc hàm `rgb(...)`.
- **Hành động:** Nếu phát hiện bất kỳ vi phạm nào, Harness đánh **FAIL lập tức (Exit code 1)** và nạp thông báo lỗi ngược lại cho Agent tự sửa trước khi render preview.

---

## 3. Rào cản 2: Chuẩn mực Iconology Nghiêm ngặt (Strict Brand Iconography)

### 3.1 Cấm Tuyệt đối Việc Import Icon Tự do
- Agent bị **CẤM HOÀN TOÀN** việc tự ý cài đặt hoặc import các thư viện icon bên ngoài như: `lucide-react` (nếu chưa qua kiểm duyệt), `font-awesome`, `@heroicons`, `react-icons`, `@mui/icons-material`.
- Lý do: Việc pha trộn nhiều bộ icon khác nhau tạo ra sự cẩu thả về độ dày nét (stroke-width), góc bo và tỷ lệ khung hình, phá vỡ tính chuyên nghiệp của thương hiệu.

### 3.2 Bộ Icon Chuẩn Tắc (The Canonical 145 SVG Manifest)
UI/UX OS thừa hưởng và khóa cứng danh mục **145 Brand Icons** chuẩn mực từ `Brand-guideline/cowok-brand-product-system-v4.3/iconography/icon-manifest.json`.

Mọi icon trên ứng dụng hoặc slide trình chiếu bắt buộc phải được gọi thông qua Component chuẩn:

```tsx
import { BrandIcon } from "@/components/ui/brand-icon";

// ĐÚNG CHUẨN: Chỉ dùng tên icon có trong icon-manifest.json
<BrandIcon name="chart-bar" size={20} className="text-accent" />
<BrandIcon name="shield-check" size={16} />
<BrandIcon name="arrow-up-right" size={14} />
```

Hoặc trong HTML tĩnh bằng SVG Sprite:
```html
<svg class="brand-icon" width="20" height="20">
  <use href="#icon-chart-bar"></use>
</svg>
```

---

## 4. Rào cản 3: Chuẩn Mực Khung Hình 16:9 & Trợ năng (Accessibility)

1. **Khung Hình Bài Thuyết trình (Presentation Viewport Invariant):**
   - Mọi slide thuyết trình phải tuân thủ tỷ lệ vàng **16:9** (`aspect-ratio: 16 / 9`).
   - Sử dụng CSS Container Queries và kỹ thuật co giãn tỷ lệ tự động (`transform: scale(...)`) để hiển thị toàn vẹn trên mọi màn hình máy chiếu hoặc laptop mà không bao giờ bị tràn thanh cuộn ngang (Zero Horizontal Scroll).
2. **Quy chuẩn Trợ năng WCAG 2.2 AA:**
   - Tỷ lệ tương phản chữ trên nền tối thiểu phải đạt **4.5:1** cho văn bản thông thường và **3:1** cho tiêu đề lớn.
   - Playwright Runner sẽ tự động tính toán ma trận màu sắc của slide/màn hình; nếu không đạt tỷ lệ tương phản, bản thiết kế sẽ bị từ chối xuất bản.
