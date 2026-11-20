# Epic 05 FMEA Risk Matrix — Option 5 2D Storyboard, Entry-First Live HTML, ChangeProposal & Real GitHub PR

## 1. FMEA Edge Case Registry

| Risk ID | Failure Mode | Potential Impact & Mitigation | S | O | D | RPN | Status |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| EC-E05-01 | Story 05.1 Edge Case | Không tự ý chạy sinh mã/sửa đổi khi `ambiguityScore >= 0.45` (`[AC-03]`) hoặc khi `flow-changes-draft.json` còn xung đột chưa xác nhận (`[AC-04]`). | 5 | 2 | 1 | **10** | Enforced in AC-03, AC-04 & Task 05.1 |
| EC-E05-01B | Story 05.1 Sandbox & Pool | Khử `<script>`/`<svg>`/`'unsafe-inline'` bằng per-render CSP nonce + Web Crypto SHA-256 (`html-sandbox-wrapper.ts`), pool LRU-5 (`port1.close()` khi evict) và Dirty Editor Guard `[EC-STORYBOARD-001]`. | 5 | 2 | 1 | **10** | Enforced in Story 05.1 Data/UI Specs |
| EC-E05-02 | Story 05.2 Edge Case | Khóa nút Apply (`412 UNVERIFIED_GATE_CLAIM`) nếu thiếu HMAC `ProposalGateReceipt` hợp lệ từ daemon, hoặc trả `409 PROPOSAL_STALE_BASE_HASH` khi file gốc thay đổi, kèm AA-forgiving `pixelmatch` + `SSIM`. | 5 | 2 | 1 | **10** | Enforced in Story 05.2 |
| EC-E05-03 | Story 05.3 Edge Case | Khóa `fs_jail` chặn `.env*`/`.npmrc`/`.netrc`/`.git-credentials`/`*.pem`/`*.key` khi export ZIP hoặc mở GitHub PR non-interactive; `VersionSnapshot` rollback hoàn tác cả đĩa và DB `chat_artifact_snapshots`. | 5 | 2 | 1 | **10** | Enforced in Story 05.3 |
| EC-E05-04 | Story 05.1/05.2 Spatial Action Drift & Redundant Controls | Hợp nhất click-component + marquee-drag làm mất thao tác chọn đơn lẻ, không mở được menu hành động bằng chuột phải trong `Interact`, hoặc hiển thị nút thừa (`Link to DAG` khi là Presentation hoặc khi DAG trái đã mở đúng node; `Inspect Property` trên vùng kéo nhiều phần tử). Khắc phục bằng Context-Aware 4-Action Floating Pill (kèm tooltips phím tắt/modifier), `ELEMENT_CONTEXT_MENU` chuột phải trên `Interact`, `Edit Draft` tách đơn lẻ (inline) vs vùng kéo (mở pane trái lọc slot), và tự `resolved` comment pin qua `ChangeProposal.sourceCommentId`. | 8 | 7 | 4 | **224** | Planned in Story 05.1 AC-10 & Story 05.2 AC-08 |

## 2. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-02 | 1.0.0 | Initial FMEA Risk Matrix for Epic 05 | Review Agent & Advisor |
| 2026-11-16 | 2.0.0 | Deep evaluation hardenings (`KU/UU/AS/BS-E05-01..03`): SQLite V9, CSP Nonce + LRU-5, HMAC Gate Receipt, FS Jail Secrets Block | Party Mode Council & Edge Case Guardian |
| 2026-11-19 | 2.1.0 | Added `EC-E05-04` (Context-Aware 4-Action Floating Pill, Interact Right-Click, Single-Inline vs Region-Panel Edit Draft, Read-Only Property Inspector & Comment-to-Proposal Auto-Resolve) | Orchestrator & Edge Case Guardian |
