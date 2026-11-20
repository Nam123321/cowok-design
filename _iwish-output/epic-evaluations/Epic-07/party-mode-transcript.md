# Party Mode Evaluation Transcript — Epic 07 (`VS-03`)

**Panelists:** `PM`, `Architect`, `Review`, `Dev`, `UX` (Deep Mode, 2 Rounds)

## Round 1 — Challenges & Pushbacks
- **Architect:** Hợp nhất `07.1 + 07.2 + 09.3 occlusion` thành **`Story 07.1`** dùng chung 1 Chromium CDP pool (`<=1` process, `<=2s`, real RGBA SSIM thay `drawWireframeCard`), và chuyển scope xuất bản từ `07.3` cũ thành **`Story 07.2`** (`07.3` -> `cancelled`). Cô lập `.pptx` thành plugin theo `TDR-006`.
- **UX:** Non-tech mở Export modal không muốn lẫn khái niệm Git PR/ZIP vào nút gửi sếp/khách hàng -> bắt buộc chia đôi `ExportBundleModal` thành **Share & Present** (mặc định) và **Developer Handoff**.
- **Review:** Bắt buộc `iframe_bridge_fallback` đánh dấu `UNVERIFIED_NO_BROWSER` thay vì fake green khi thiếu Chromium.

## Round 2 — Consensus
- Chốt `Story 07.1` (`refactored`) + `Story 07.2` (`refactored`, `delivery: hybrid`) + `07.3` (`cancelled`) dưới envelope `IMPLEMENT`.
