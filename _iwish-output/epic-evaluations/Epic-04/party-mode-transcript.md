# Party Mode Evaluation Transcript — Epic 04 (`VS-02`)

**Panelists:** `PM`, `Architect`, `Review`, `Dev`, `UX` (Deep Mode, 2 Rounds)

## Round 1 — Challenges & Pushbacks
- **PM & UX:** Người dùng non-tech kéo `.csv`/`.fig` hoặc dán API key (`Google AI Studio`, `TypeSafe JEV`) thẳng vào khung chat hiện bị mất dữ liệu hoặc không được hướng dẫn rõ ràng; đóng tab làm mất chat vì `sessionStorage`.
- **Architect:** Cảnh báo tuyệt đối không giữ bản sao API key chưa redact trong SQLite `chat_messages` hoặc SSE event replay; đồng thời ghi nhận cảnh báo `coherence-report-Epic-04.json` đối với `Story-04.3` chữ `"Electron"` chỉ là mô tả tránh treo IDE chứ không dùng Electron.
- **Review:** Bắt buộc nối `Story 04.6` qua `/cas/upload` (`V16 cas_blobs`), `/vault/chat-ingest` (`POST` + `noul` probes), và `<svg viewBox>` auto-inference + `DESIGN_CDN_CSP` toggle.

## Round 2 — Consensus
- Bảo toàn 100% `Story 04.1..04.5` ở trạng thái `completed`, bổ sung duy nhất `Story 04.6` dưới envelope `IMPLEMENT`.
