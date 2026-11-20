---
type: I-Wish Story
id: "04.6"
title: "Story 04.6: SQLite V4/V16 Chat Persistence, CAS Binary/CSV Intake, <svg viewBox> Normalizer & In-Chat Zero-Leak Key Vault"
description: "Đưa toàn bộ lịch sử chat về SQLite V4 chat_messages với auto-provisioned session FK thay thế sessionStorage, sửa CAS intake cho .csv/.tsv/.xlsx/.fig + onPaste, thêm <svg viewBox> auto-inference & DESIGN_CDN_CSP toggle, cùng UX nạp key Decision Engine (aistudio.google.com/app/apikey & console.typesafe.ai) an toàn qua 3-Layer Secret Interceptor + daemon redact_secrets vào 0o600/Keychain Vault."
resource: "iwish://uxui-os/development/epic-04/story-04.6"
status: refactored
risk_tier: 3
delivery: core
adr_ref: "ADR-006"
timestamp: "2026-11-20T20:45:00+07:00"
tags:
  - "story-04.6"
  - "epic-04"
  - "cas-intake"
  - "chat-persistence"
  - "zero-leak-vault"
links_to:
  - "[[../../../../2. Product Planning/2.1. product-brief-or-prd.md]]"
  - "[[../../../../2. Product Planning/2.2. database-spec.md]]"
  - "[[../../../../2. Product Planning/2.5. architecture.md]]"
  - "[[../epic.md]]"
dependencies: ["04.2", "04.3"]
provides_apis:
  - "GET /api/v1/chat-sessions/:sid/messages"
  - "POST /api/v1/chat-sessions/:sid/messages"
  - "POST /api/v1/cas/upload"
  - "POST /api/v1/vault/chat-ingest"
consumes_apis:
  - "POST /api/v1/doctor/verify-key"
---

# Story 04.6: SQLite `V4/V16` Chat Persistence, CAS Binary/CSV Intake, `<svg viewBox>` Normalizer & In-Chat Zero-Leak Key Vault

- **FR Covered:** [FR-31: SQLite Chat Persistence, CAS Binary/CSV Intake & Zero-Leak In-Chat Key Vault](file:///Users/hatrang20061988/.gemini/antigravity/worktrees/uxui-os/complete_story_zero_four/_iwish-output/2.%20Product%20Planning/2.1.%20product-brief-or-prd.md)
- **Epic:** [Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, CAS Intake & In-Chat Vault](../epic.md)
- **Status:** `refactored`
- **Risk Tier:** `3`

## 1. Story Description & Context
Khép kín hoàn toàn trải nghiệm khởi đầu trên khung chat cho người dùng non-tech qua lát cắt dọc 5 lớp: giữ bền vững 100% lịch sử hội thoại qua đóng/mở tab (`V4 chat_messages`), tiếp nhận file nhị phân và bảng số liệu `.csv/.tsv/.xlsx/.fig` bằng Content-Addressable Storage (`V16 cas_blobs`), tự động chuẩn hóa `<svg viewBox>` tránh sụp khung hình `0px`, và cung cấp luồng nạp khóa API cho Decision Engine (`Google AI Studio` & `TypeSafe JEV`) ngay trong chat không bao giờ làm lộ secret.

## 2. Plan-Tune Heuristic & Complexity Score
- **Complexity Score (CS):** `CS = 5` (Scope=1, Data=1, Integration=1, UI=1, Security=1, Unknowns=0 -> hợp nhất trọn luồng dọc theo `TDR-006`).
- **Formal-Verify Candidate (`risk_tier: 3`):** Bất biến chặn lưu plaintext secret trước khi vào `V4 chat_messages`/SSE và khóa ghi CAS `.tmp -> sha256` bằng `O_NOFOLLOW`.

## 3. Acceptance Criteria (AC)
- **AC-01 (SQLite `V4` Chat Persistence & Auto-Provisioned Session FK):** Nối `ChatPrimitivesBridge` <-> `GET/POST /api/v1/chat-sessions/:sid/messages` xuống bảng `V4 chat_messages` (tự cấp phát session FK cho phiên mới chưa gán project), thay thế hoàn toàn cơ chế đồng bộ `sessionStorage` (`100%` khôi phục sau đóng/mở tab); API `POST /messages` bắt buộc quét chối/redact key thô phía server phòng trường hợp bypass từ client.
- **AC-02 (CAS Binary/CSV Intake & Drag/Paste Capture):** Mở rộng `ChatPrimitivesBridge` và `LaunchpadPanels` thêm `.csv,.tsv,.xlsx,.fig` vào `<input accept>`; sửa `readSelectedChatFiles` khỏi lỗi đánh rơi file nhị phân bằng stream vào `POST /api/v1/cas/upload` -> `.cowok-design/cas/<sha256>` + bảng `V16a cas_blobs` (sở hữu migration slot `V16a`); kiểm tra magic-byte, giới hạn giải nén zip-bomb cho `.xlsx/.fig`, vô hiệu hóa CSV formula/DDE (`=`, `+`, `-`, `@`), ghi nguyên tử `O_NOFOLLOW` `.tmp -> sha256`, và xử lý `onPaste` (`e.preventDefault()`) không dán base64 vào textarea.
- **AC-03 (`<svg viewBox>` Auto-Inference & SVG mXSS Sanitizer):** `html-sandbox-sanitizer.ts` tự suy diễn `viewBox` từ kích thước/`width`/`height` khi thiếu (kèm fallback `0 0 1200 800` khi giá trị thiếu, `<= 0`, `%` hoặc `calc(...)` — đạt `0` SVG silent drops), lọc sạch SVG mXSS (`<script>`, `<foreignObject>`, `on*`, `xlink:href`), và cung cấp công tắc `DESIGN_CDN_CSP` an toàn theo `TDR-003`.
- **AC-04 (In-Chat Zero-Leak Key Vault UX & Pre-Persistence 3-Layer Secret Interceptor):** Hiển thị `DecisionEngineOnboardingCard` trong chat với liên kết tới `aistudio.google.com/app/apikey` & `console.typesafe.ai` kèm hướng dẫn từng bước; khi người dùng gửi key qua `POST /api/v1/vault/chat-ingest` (kèm `Jev Noul` probes), chạy **Pre-Persistence 3-Layer Secret Interceptor** trước khi lọt vào optimistic React state hay `V4 chat_messages`, đưa thẳng vào `0o600`/OS Keychain Vault, thay thế bằng **`VaultReceiptCard`** (Presenter Mask, pill `429`, nút `[Replace/Revoke]`, cảnh báo high-entropy, `Enterprise Local-Only Lock`) đạt **`100%` secret intercept**.
- **AC-05 ([EDGE-CASE] Concurrent SSE Redaction, Corrupted CAS & Dedupe GC Lock):** Mọi event phát qua SSE hoặc ghi log hệ thống đều chạy qua `redact_secrets` trước khi serialize; file upload vượt quota, sai magic-byte hoặc hỏng header trả lỗi cấu trúc kèm trạng thái thu hồi sạch mà không làm hỏng `V16 cas_blobs`; khóa dedupe chống GC xóa blob đang ghi đồng thời.

## 4. Tasks
- [ ] **Task 04.6.1**: Thêm migration `V16` cho `cas_blobs` và API `GET/POST /api/v1/chat-sessions/:sid/messages`, `/cas/upload`, `/vault/chat-ingest` trong `crates/spec-registry` & `crates/harness-daemon` (thỏa mãn `AC-01`, `AC-02`, `AC-04`).
- [ ] **Task 04.6.2**: Cập nhật `ChatPrimitivesBridge`, `LaunchpadPanels`, `DecisionEngineOnboardingCard`, `VaultReceiptCard` và sửa `onPaste` / `readSelectedChatFiles` + `<input accept>` (thỏa mãn `AC-01`, `AC-02`, `AC-04`).
- [ ] **Task 04.6.3**: Bổ sung `<svg viewBox>` auto-inference & `DESIGN_CDN_CSP` toggle trong `html-sandbox-sanitizer.ts` (thỏa mãn `AC-03`).
- [ ] **Task 04.6.4**: Cài đặt 3-Layer Secret Interceptor + `redact_secrets` trong daemon/SSE + test `[EDGE-CASE]` khôi phục chat sau reload, upload 5 loại file CAS và intercept secret 100% (thỏa mãn `AC-04`, `AC-05`).

## 5. AC-to-Task Traceability Matrix

| Acceptance Criteria | Mapped Implementation Tasks | Verification Method |
| :--- | :--- | :--- |
| **AC-01** | `Task 04.6.1`, `Task 04.6.2` | SQLite & Playwright Tab-Close Persistence Test |
| **AC-02** | `Task 04.6.1`, `Task 04.6.2` | CAS SHA-256 Upload & Paste Integration Test |
| **AC-03** | `Task 04.6.3` | Vitest SVG Sanitizer Regression Test |
| **AC-04** | `Task 04.6.1`, `Task 04.6.2`, `Task 04.6.4` | 100% Secret Intercept & Vault UI Test |
| **AC-05 (`[EDGE-CASE]`)** | `Task 04.6.4` | Redacted SSE & Corrupt Upload Boundary Test |

## 6. Cross-Feature Dependencies
- **Story Dependencies:** ["04.2", "04.3"]

### Impacts
- Cung cấp lịch sử chat bền vững (`V4 chat_messages`), tệp CAS `.csv/.tsv/.pdf/.fig` cho `Story 06.5` và `Story 09.1`, cùng chìa khóa BYOK cho `Story 09.2` Decision Engine.

### Consumes
- Tiêu thụ SQLite pool & streaming BFF từ `Story 04.2` và Keychain/`0o600` Vault core từ `Story 04.3`.

### Shared Entities
- Sử dụng `chat_sessions`, `chat_messages` (`V4`) và `cas_blobs` (`V16`).

### Cross-Portal
- Áp dụng đồng nhất trên mọi Portal & phiên chat không gắn project trước.

## 7. QA Simulator Guardian Scorecard

| Evaluation Axis | Score | Notes |
| :--- | :---: | :--- |
| **1. Functional Correctness** | `9.6/10` | Phủ trọn chat persistence, CAS intake, `<svg viewBox>` và key vault UX |
| **2. Data Integrity & State** | `9.6/10` | Xóa `sessionStorage` volatile sync, dùng `V4 chat_messages` & SHA-256 CAS |
| **3. Security & Validation** | `9.8/10` | 3-Layer Secret Interceptor + `redact_secrets` + `0o600` Vault |
| **4. Performance & Scalability** | `9.5/10` | Stream CAS không giữ base64 trong bộ nhớ UI |
| **5. Error Handling & Recovery** | `9.5/10` | Pill `429`, nút `[Replace/Revoke]`, high-entropy warning rõ ràng |
| **6. Architectural Depth & Leverage** | `9.5/10` | Tái sử dụng trọn vẹn `V4` và Vault `04.3` |
| **7. UX Empathy** | `9.7/10` | Non-tech dán key thẳng vào chat an toàn kèm link `aistudio`/`typesafe` |
| **TOTAL AVERAGE** | **9.60/10** | **PASSED (>= 8.5/10)** |

## 8. Change Log

| Date | Version | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-11-20 | 1.0 | Khởi tạo Story 04.6 theo kế hoạch 20 Vertical Slices Post-PR #14 (`TDR-006`) | /make-story |
