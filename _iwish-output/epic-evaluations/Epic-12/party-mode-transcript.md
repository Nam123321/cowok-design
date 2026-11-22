# Party Mode Evaluation Transcript — Epic 12 (`VS-05`)

**Panelists:** `PM`, `Architect`, `Review`, `Dev`, `UX` (Deep Mode, 2 Rounds)

## Round 1 — Challenges & Pushbacks
- **Architect & Dev:** Khi đóng gói standalone single-binary (`12.1`), frontend gọi `/api/*` mà không có Vite dev server proxy sẽ bị 404 nếu `harness-daemon` chỉ mount `/api/v1/*`. Cần mount alias `/api/*` ngay trong `harness-daemon` (`0` Vite-proxy 404s).
- **PM & Review:** Ràng buộc `12.2` tiêu thụ `08.5` (Cowok Plugin format) và `12.3` tiêu thụ `07.1` (Pooled Headless Chromium trong container image).

## Round 2 — Consensus
- Chốt `12.1`, `12.2` (`delivery: hybrid`), `12.3` dưới envelope `IMPLEMENT`.
