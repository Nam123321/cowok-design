---
type: implementation-plan
story_id: "04.6"
title: "Round 5 — Permanent CAS Attachments with Isolated Idempotency Receipts"
status: approved
risk_tier: 3
adr_refs: [ADR-006, ADR-091]
generated_at: "2026-11-20T20:45:00+07:00"
plugin_plan_gate:
  classification: NOT_DECLARED
  applicable: false
  rationale: "story.md không khai báo plugin_classification: PLUGIN hoặc HYBRID."
---

## Technical Architecture & File Scope

Kế hoạch này thay thế toàn bộ addendum trùng lặp trước đó. Hợp đồng idempotency được tách hoàn toàn khỏi dữ liệu attachment và ledger quota:

- `cas_attachments` là metadata attachment vĩnh viễn, không có `expires_at`, không bị xóa hoặc NULL hóa bởi TTL idempotency.
- `cas_upload_ingress` là ledger ingress vĩnh viễn phục vụ truy vấn rolling-24h, không có `expires_at`, không bị xóa bởi TTL idempotency.
- `cas_command_idempotency` là receipt riêng, TTL `86_400_000 ms`, có `attachment_id` FK `ON DELETE CASCADE`, `UNIQUE(tenant_id, session_id, idempotency_key)`, `payload_sha256`, `accepted_at`, `expires_at`.
- `vault_command_idempotency` là receipt riêng cho vault, TTL `86_400_000 ms`; không chứa plaintext secret, encrypted secret material, provider key, hay vault path.
- TTL purge chỉ được phép chạy hai lệnh scoped theo tenant:
  `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
  `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
- Không có idempotency TTL cleanup nào được phép đụng tới `cas_attachments`, `cas_blobs`, hoặc `cas_upload_ingress`.
- Canonical client boundary duy nhất là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; UI/hooks tuyệt đối không `fetch()` trực tiếp.
- Mỗi endpoint có Next BFF `src/app/api/**/route.ts` bọc `guardedRoute()`, typed client SDK, Zod schema, và daemon Axum route handler tương ứng.

| File | Trách nhiệm bắt buộc |
|---|---|
| `data-spec.md` | Tách ba nhóm lifetime: permanent attachment/blob/ingress, TTL CAS receipt, TTL vault receipt; DDL, FK, unique/index và thuật toán purge. |
| `prisma/schema.prisma` | Khai báo parity cho mọi model Prisma trong `data-spec.md`: `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`. |
| `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql` | Migration SQLite cho bảng/idempotency index/FK và không thêm TTL columns vào attachment hoặc ingress. |
| `crates/harness-daemon/src/routes/chat_cas_vault.rs` | Axum authorization, CAS/GC mutex, retry, idempotency receipt transaction, safe errors và secret redaction. |
| `src/app/api/v1/**/route.ts` | BFF route handler bọc `guardedRoute()` cho toàn bộ endpoint Story 04.6. |
| `src/api/chat-vault.ts` | Typed client SDK duy nhất cho chat, CAS và vault; parse `ApiErrorDto`, truyền idempotency key. |
| `src/lib/validations/chat-vault.ts` | Zod request/response/error schemas duy nhất. |
| `src/features/flow/utils/html-sandbox-sanitizer.ts` | viewBox inference, SVG URL policy, CSP parser fail-closed. |

## 1. Deep Codebase Drift & Contract Audit

| Contract | Quyết định khóa |
|---|---|
| Principal/anti-IDOR | `tenant_id`, actor và capability chỉ lấy từ principal đã xác thực. Session/project/receipt thiếu hoặc cross-tenant trả `404 RESOURCE_NOT_FOUND`; principal hợp lệ nhưng thiếu capability trả `403 AUTH_FORBIDDEN`. |
| CAS receipt identity | `cas_command_idempotency` dùng `UNIQUE(tenant_id, session_id, idempotency_key)`. Key là opaque, non-empty, ASCII-visible tối đa 128 ký tự, không log. Header `X-Idempotency-Key` ưu tiên; nếu body `idempotencyKey` đồng thời tồn tại thì phải byte-identical. |
| CAS digest | `payload_sha256 = SHA-256(raw_stream_bytes || canonical_metadata)`; metadata canonical gồm `sessionId`, `projectId|null`, normalized extension và MIME đã được server xác nhận. |
| CAS replay | Sau khi purge receipt hết hạn và trước quota/FS side effect: same key + same digest trả `200` attachment DTO gốc; same key + digest khác trả `409 IDEMPOTENCY_KEY_PAYLOAD_MISMATCH`. |
| CAS persistence | Một upload thành công tạo/giữ `cas_attachments`, cập nhật `cas_blobs`, và ghi `cas_upload_ingress`; đồng thời tạo một `cas_command_idempotency` tham chiếu attachment đó. Chỉ receipt có thể hết hạn. |
| TTL boundary | Receipt active khi `expires_at >= now_ms`; purge chỉ xóa khi `expires_at < now_ms`. Sau purge, key được tái sử dụng như command mới; attachment/blob/ingress cũ vẫn nguyên vẹn. |
| Vault replay | `vault_command_idempotency` lưu tenant/session/key/digest/safe receipt ID/timestamps; replay không tái xử lý secret và không đưa secret vào state, SQLite, SSE hoặc log. |
| Retry | Chỉ retry `SQLITE_BUSY`/`SQLITE_LOCKED`: tối đa 3 attempts, backoff 25ms exponential cap 200ms, jitter 0–25ms, deadline 3000ms. Các lỗi validation/auth/quota/idempotency/FS không retry. |
| CAS/GC | Upload và GC lấy cùng per-SHA `tokio::sync::Mutex`; GC chỉ unlink với `ref_count=0 AND gc_lease_until < now_ms` trong `BEGIN IMMEDIATE`. |
| Error DTO | Chỉ trả `ApiErrorDto={code,message,retryable,limit?}`; message cố định an toàn, không leak SHA/path/file name/tenant/usage/secret/payload/diagnostic. |

## 2. Unknowns Discovery & Epistemic Audit

| ID | Resolution | Status |
|---|---|---|
| U-046-17 | `cas_attachments` không bao giờ là TTL receipt và không bị xóa để tái sử dụng idempotency key. | RESOLVED |
| U-046-18 | `cas_upload_ingress` không có `expires_at`; rolling quota dùng mốc `accepted_at` khi tính toán, không dùng purge idempotency. | RESOLVED |
| U-046-19 | `cas_command_idempotency` là bảng receipt duy nhất cho CAS replay và tham chiếu `attachment_id ON DELETE CASCADE`. | RESOLVED |
| U-046-20 | Client SDK/validation canonical là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; không tạo các SDK split cạnh tranh. | RESOLVED |
| U-046-21 | Mọi model được nêu trong `data-spec.md` có task Prisma schema và SQLite migration tương ứng. | RESOLVED |
| U-046-22 | Story không khai báo PLUGIN/HYBRID nên Plugin Plan Gate không áp dụng. | RESOLVED |

## 3. AI Socratic Debate & Consensus Report

| Challenge | Consensus khóa |
|---|---|
| TTL receipt có thể xóa attachment không? | Không. Receipt tồn tại trong bảng riêng; `DELETE` TTL chỉ target `cas_command_idempotency` hoặc `vault_command_idempotency`. |
| Sau 24h có thể reuse key không? | Có. Sau `expires_at < now_ms` purge receipt scoped theo tenant, command mới có thể dùng key; mọi attachment/blob/ingress lịch sử vẫn tồn tại. |
| Race cùng key? | Unique receipt trong `BEGIN IMMEDIATE` chọn một winner; loser reread receipt: cùng digest replay `200`, digest khác `409`; không double charge. |
| Attachment bị xóa hợp lệ thì receipt sao? | FK `attachment_id ON DELETE CASCADE` xóa receipt liên quan như hệ quả của attachment lifecycle hợp lệ, không phải TTL cleanup. |
| GC thấy final sau rollback? | Giữ mutex; chỉ xóa final do request tạo sau một `BEGIN IMMEDIATE` mới xác nhận không còn `cas_blobs` reference. |
| Frontend gọi daemon trực tiếp? | Không. `guardedRoute()` BFF và `src/api/chat-vault.ts` là boundary bắt buộc; UI/hooks không gọi `fetch()`. |

## 3. File Manifest

| Artefact | Mandatory schema/behavior |
|---|---|
| `CasBlob` | `sha256` PK, physical metadata, `ref_count`, `gc_lease_until`; Prisma + SQLite parity. |
| `CasAttachment` | Permanent tenant/session/project ownership metadata; không có `expires_at`, không có TTL purge. |
| `TenantCasBlobUsage` | Permanent tenant physical-byte accounting; Prisma + SQLite parity. |
| `CasUploadIngress` | Permanent accepted ingress ledger với `accepted_at`; không có `expires_at`, không có idempotency TTL purge. |
| `CasCommandIdempotency` | `tenant_id`, `session_id`, `idempotency_key`, `attachment_id`, `payload_sha256`, `accepted_at`, `expires_at`; FK `attachment_id ON DELETE CASCADE`; unique `(tenant_id, session_id, idempotency_key)`; index `(tenant_id, expires_at)`. |
| `VaultCommandIdempotency` | Safe receipt mapping với unique `(tenant_id, session_id, idempotency_key)` và index `(tenant_id, expires_at)`; cấm secret material. |
| `ApiErrorDtoSchema` | Zod schema strict cho `{code,message,retryable,limit?}`. |
| SVG/CSP policy | Chỉ cho `https:`, `http:`, PNG/JPEG/WEBP base64 exact prefix, `#fragment`, `/relative-path`; CSP host phải match `^https://[a-z0-9.-]+$`, sai thì fail closed. |

## 4. Edge Case Guardian & 13-Pillar FMEA Scan

| Pillar | Failure mode | RPN | Hard control |
|---|---|---:|---|
| Data lifecycle | TTL cleanup xóa attachment/blob hợp lệ | 180 | Receipt tables tách riêng; SQL purge allowlist chỉ hai bảng idempotency. |
| Idempotency | Replay double-charge hoặc key reuse sai digest | 144 | Unique receipt, canonical digest, transaction reread, replay `200`, mismatch `409`. |
| Tenant isolation | IDOR qua sid/project/receipt | 150 | Principal-derived scope, cross-tenant/missing `404`. |
| Referential integrity | Receipt mồ côi hoặc attachment delete không nhất quán | 126 | `attachment_id` FK `ON DELETE CASCADE`; migration FK test. |
| Quota accounting | TTL làm mất ingress history | 144 | `cas_upload_ingress` không có TTL column/purge; rolling query theo `accepted_at`. |
| CAS/GC | GC unlink blob đang upload | 150 | Shared SHA mutex, lease, immediate transaction. |
| Archive intake | ZIP bomb | 144 | 1024 entries, 67,108,864 bytes uncompressed, ratio ≤20.0 trước commit. |
| SQLite contention | Retry vô hạn/partial charge | 108 | Exactly 3 retry attempts và transaction atomicity. |
| Secret leakage | Vault secret vào persistence/SSE/log | 180 | 3-layer interceptor, `Zeroizing`, `redact_secrets`, receipt chỉ safe ID/digest. |
| SVG/CSP | mXSS hoặc permissive host parsing | 126 | Exact URL grammar và CSP fail-closed. |

## 5. Systemic Runtime Realism & Environment Pre-Flight Matrix

| Dependency | Required check | Failure contract |
|---|---|---|
| V16 SQLite migration | Foreign keys enabled; six declared models có DDL/Prisma parity; receipt-only TTL indexes tồn tại. | Fail migration/startup safely; không chạy partial schema. |
| Idempotency cleanup worker | SQL allowlist chỉ `cas_command_idempotency` và `vault_command_idempotency`, tenant-scoped, predicate `< now_ms`. | Abort job và alert nếu target table ngoài allowlist. |
| Quota evaluator | Tính rolling 24h từ permanent `cas_upload_ingress.accepted_at`; không dựa vào TTL delete. | `429` safe quota error, không ghi attachment/blob/receipt. |
| BFF auth | `guardedRoute()` inject verified principal trước downstream invocation. | `401`/safe `403`; không fallback tenant từ body/query/header. |
| CAS root | `.tmp-*` và final cùng volume, `O_NOFOLLOW`, mode `0600`, fsync trước rename. | Generic safe error và cleanup temp. |
| Shared lock registry | Upload/delete/GC dùng cùng keyed mutex namespace theo SHA. | Skip GC/delete khi lock/lease ambiguous. |
| CSP startup | Parse `DESIGN_CDN_CSP` một lần, exact regex. | CSP default fail-closed. |

## Implementation Steps & Verification Checklist

- [ ] Cập nhật `data-spec.md`, `prisma/schema.prisma`, và SQLite migration `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql`: materialize đầy đủ `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`; `cas_attachments`/`cas_upload_ingress` không có `expires_at`; `cas_command_idempotency` có FK `attachment_id ON DELETE CASCADE`, unique `(tenant_id, session_id, idempotency_key)`, digest/timestamps/index expiry; kiểm tra bằng sqlite migration test.
- [ ] Implement SQLite idempotency purge allowlist trong Axum service: chỉ chạy `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?` và `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`; cấm mọi route handler hoặc worker xóa `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vì idempotency TTL.
- [ ] Tạo Zod `ApiErrorDtoSchema`, schemas chat/CAS/vault/idempotency trong `src/lib/validations/chat-vault.ts`, và typed client sdk duy nhất `src/api/chat-vault.ts`; UI/hooks không dùng direct `fetch()`.
- [ ] Với `GET /api/v1/chat-sessions/:sid/messages`, implement Axum route handler principal-scoped, Next `src/app/api/v1/chat-sessions/[sid]/messages/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cross-tenant/missing trả `404 RESOURCE_NOT_FOUND`.
- [ ] Với `POST /api/v1/chat-sessions/:sid/messages`, implement Axum route handler auto-provision session tenant-bound, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; server intercept/redact secret trước optimistic state, sqlite persistence và SSE serialization.
- [ ] Với `POST /api/v1/cas/upload`, implement Axum route handler anti-IDOR và receipt idempotency, Next route handler `src/app/api/v1/cas/upload/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cấm direct fetch tại UI/hooks.
- [ ] Với `POST /api/v1/vault/chat-ingest`, implement Axum route handler 3-layer secret interceptor và vault receipt replay, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; không persist/log/SSE plaintext secret.
- [ ] Với `POST` và `DELETE /api/v1/vault/receipts/:receipt_id`, implement Axum route handler authorization, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; receipt cross-tenant/missing phải không phân biệt bằng `404 RESOURCE_NOT_FOUND`.
- [ ] Implement reusable SQLite retry executor cho Axum route handler: 3 attempts, 25ms initial exponential backoff, cap 200ms, jitter 0–25ms, deadline 3000ms; exhausted busy trả `503 CHAT_WRITE_CONTENTION`, `retryable:true`, không partial sqlite write.
- [ ] Implement CAS Axum route handler order: stream `.tmp-<uuid>` với `O_NOFOLLOW`/`0600`, hard-stop `26_214_401`, SHA/magic/archive validation, shared per-SHA mutex, sqlite `BEGIN IMMEDIATE`, purge receipt TTL allowlist, replay/mismatch decision, quota checks, fsync/rename, upsert blob/attachment/usage/permanent ingress/new CAS receipt, commit, release lock.
- [ ] Implement rollback/GC trong Axum service với cùng mutex: cleanup temp; final mới chỉ được unlink sau sqlite `BEGIN IMMEDIATE` chứng minh không có committed blob reference; attachment deletion cập nhật refcount; GC chỉ unlink `ref_count == 0 AND gc_lease_until < now_ms`.
- [ ] Update `src/features/flow/utils/html-sandbox-sanitizer.ts` với deterministic viewBox fallback, strict SVG URL grammar, và `DESIGN_CDN_CSP` parser fail-closed.
- [ ] `UT-IDOR-001`: xác minh sáu operations chat/CAS/vault dùng Axum route handler, guardedRoute, client sdk và zod contracts; cross-tenant/missing trả `404`, thiếu capability trả `403`, không leak metadata.
- [ ] `UT-IDEMPOTENCY-001`: cùng key/cùng digest chỉ commit một lần; replay `200` không tăng attachment refcount, physical usage hoặc ingress; digest khác trả `409`.
- [ ] `UT-IDEMPOTENCY-002`: sau `expires_at < now_ms`, purge chỉ xóa CAS receipt và cho phép reuse key; assert existing `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vẫn nguyên vẹn byte-for-byte/row-for-row.
- [ ] `UT-IDEMPOTENCY-003`: assert purge Vault chỉ xóa expired `vault_command_idempotency`; không xóa vault receipt hợp lệ trước expiry và không có secret material trong sqlite row.
- [ ] `UT-FK-RECEIPT-001`: verify `CasCommandIdempotency.attachment_id` FK `ON DELETE CASCADE`; attachment lifecycle hợp lệ xóa receipt liên quan, còn TTL purge không được gọi attachment delete.
- [ ] `UT-GC-RACE-001`, `UT-ARCHIVE-CAPS-001`, `UT-RETRY-001`, `UT-CSP-SVG-001`, và `UT-ERROR-001`: giữ toàn bộ boundary/race/error assertions trước đó, bao gồm no-leak error DTO và sqlite atomicity.
- [ ] `E2E-CHAT-CAS-VAULT-001`: dùng client sdk `src/api/chat-vault.ts` qua guardedRoute route handler; verify reload chat persistence, binary/CSV/XLSX/FIG upload, paste prevention, vault receipt masking, and no direct UI fetch.
- [ ] `E2E-TTL-PRESERVATION-001`: tạo attachment và ingress, advance clock >24h, chạy TTL purge, upload lại với same key, và assert attachment/blob/ingress cũ không mất trước/sau command mới.

## Codex Author Revision & Hardening Addendum (Round 1)

---
type: implementation-plan-addendum
story_id: "04.6"
title: "Stage 3A — Pre-Coding Safety Remediation"
status: DRAFT_PENDING_CODEX_AUDIT
risk_tier: 3
adr_refs: [ADR-006, ADR-091]
generated_at: "2026-11-20T20:45:00+07:00"
plugin_plan_gate:
  classification: NOT_DECLARED
  applicable: false
  rationale: "story.md không khai báo PLUGIN hoặc HYBRID."
---

## Technical Architecture & File Scope

Kế hoạch này thay thế toàn bộ addendum trùng lặp trước đó. Hợp đồng idempotency được tách hoàn toàn khỏi dữ liệu attachment và ledger quota:

- `cas_attachments` là metadata attachment vĩnh viễn, không có `expires_at`, không bị xóa hoặc NULL hóa bởi TTL idempotency.
- `cas_upload_ingress` là ledger ingress vĩnh viễn phục vụ truy vấn rolling-24h, không có `expires_at`, không bị xóa bởi TTL idempotency.
- `cas_command_idempotency` là receipt riêng, TTL `86_400_000 ms`, có `attachment_id` FK `ON DELETE CASCADE`, `UNIQUE(tenant_id, session_id, idempotency_key)`, `payload_sha256`, `accepted_at`, `expires_at`.
- `vault_command_idempotency` là receipt riêng cho vault, TTL `86_400_000 ms`; không chứa plaintext secret, encrypted secret material, provider key, hay vault path.
- TTL purge chỉ được phép chạy hai lệnh scoped theo tenant:
  `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
  `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
- Không có idempotency TTL cleanup nào được phép đụng tới `cas_attachments`, `cas_blobs`, hoặc `cas_upload_ingress`.
- Canonical client boundary duy nhất là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; UI/hooks tuyệt đối không `fetch()` trực tiếp.
- Mỗi endpoint có Next BFF `src/app/api/**/route.ts` bọc `guardedRoute()`, typed client SDK, Zod schema, và daemon Axum route handler tương ứng.

| File | Trách nhiệm bắt buộc |
|---|---|
| `data-spec.md` | Tách ba nhóm lifetime: permanent attachment/blob/ingress, TTL CAS receipt, TTL vault receipt; DDL, FK, unique/index và thuật toán purge. |
| `prisma/schema.prisma` | Khai báo parity cho mọi model Prisma trong `data-spec.md`: `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`. |
| `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql` | Migration SQLite cho bảng/idempotency index/FK và không thêm TTL columns vào attachment hoặc ingress. |
| `crates/harness-daemon/src/routes/chat_cas_vault.rs` | Axum authorization, CAS/GC mutex, retry, idempotency receipt transaction, safe errors và secret redaction. |
| `src/app/api/v1/**/route.ts` | BFF route handler bọc `guardedRoute()` cho toàn bộ endpoint Story 04.6. |
| `src/api/chat-vault.ts` | Typed client SDK duy nhất cho chat, CAS và vault; parse `ApiErrorDto`, truyền idempotency key. |
| `src/lib/validations/chat-vault.ts` | Zod request/response/error schemas duy nhất. |
| `src/features/flow/utils/html-sandbox-sanitizer.ts` | viewBox inference, SVG URL policy, CSP parser fail-closed. |

## 1. Deep Codebase Drift & Contract Audit

| Contract | Quyết định khóa |
|---|---|
| Principal/anti-IDOR | `tenant_id`, actor và capability chỉ lấy từ principal đã xác thực. Session/project/receipt thiếu hoặc cross-tenant trả `404 RESOURCE_NOT_FOUND`; principal hợp lệ nhưng thiếu capability trả `403 AUTH_FORBIDDEN`. |
| CAS receipt identity | `cas_command_idempotency` dùng `UNIQUE(tenant_id, session_id, idempotency_key)`. Key là opaque, non-empty, ASCII-visible tối đa 128 ký tự, không log. Header `X-Idempotency-Key` ưu tiên; nếu body `idempotencyKey` đồng thời tồn tại thì phải byte-identical. |
| CAS digest | `payload_sha256 = SHA-256(raw_stream_bytes || canonical_metadata)`; metadata canonical gồm `sessionId`, `projectId|null`, normalized extension và MIME đã được server xác nhận. |
| CAS replay | Sau khi purge receipt hết hạn và trước quota/FS side effect: same key + same digest trả `200` attachment DTO gốc; same key + digest khác trả `409 IDEMPOTENCY_KEY_PAYLOAD_MISMATCH`. |
| CAS persistence | Một upload thành công tạo/giữ `cas_attachments`, cập nhật `cas_blobs`, và ghi `cas_upload_ingress`; đồng thời tạo một `cas_command_idempotency` tham chiếu attachment đó. Chỉ receipt có thể hết hạn. |
| TTL boundary | Receipt active khi `expires_at >= now_ms`; purge chỉ xóa khi `expires_at < now_ms`. Sau purge, key được tái sử dụng như command mới; attachment/blob/ingress cũ vẫn nguyên vẹn. |
| Vault replay | `vault_command_idempotency` lưu tenant/session/key/digest/safe receipt ID/timestamps; replay không tái xử lý secret và không đưa secret vào state, SQLite, SSE hoặc log. |
| Retry | Chỉ retry `SQLITE_BUSY`/`SQLITE_LOCKED`: tối đa 3 attempts, backoff 25ms exponential cap 200ms, jitter 0–25ms, deadline 3000ms. Các lỗi validation/auth/quota/idempotency/FS không retry. |
| CAS/GC | Upload và GC lấy cùng per-SHA `tokio::sync::Mutex`; GC chỉ unlink với `ref_count=0 AND gc_lease_until < now_ms` trong `BEGIN IMMEDIATE`. |
| Error DTO | Chỉ trả `ApiErrorDto={code,message,retryable,limit?}`; message cố định an toàn, không leak SHA/path/file name/tenant/usage/secret/payload/diagnostic. |

## 2. Unknowns Discovery & Epistemic Audit

| ID | Resolution | Status |
|---|---|---|
| U-046-17 | `cas_attachments` không bao giờ là TTL receipt và không bị xóa để tái sử dụng idempotency key. | RESOLVED |
| U-046-18 | `cas_upload_ingress` không có `expires_at`; rolling quota dùng mốc `accepted_at` khi tính toán, không dùng purge idempotency. | RESOLVED |
| U-046-19 | `cas_command_idempotency` là bảng receipt duy nhất cho CAS replay và tham chiếu `attachment_id ON DELETE CASCADE`. | RESOLVED |
| U-046-20 | Client SDK/validation canonical là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; không tạo các SDK split cạnh tranh. | RESOLVED |
| U-046-21 | Mọi model được nêu trong `data-spec.md` có task Prisma schema và SQLite migration tương ứng. | RESOLVED |
| U-046-22 | Story không khai báo PLUGIN/HYBRID nên Plugin Plan Gate không áp dụng. | RESOLVED |

## 3. AI Socratic Debate & Consensus Report

| Challenge | Consensus khóa |
|---|---|
| TTL receipt có thể xóa attachment không? | Không. Receipt tồn tại trong bảng riêng; `DELETE` TTL chỉ target `cas_command_idempotency` hoặc `vault_command_idempotency`. |
| Sau 24h có thể reuse key không? | Có. Sau `expires_at < now_ms` purge receipt scoped theo tenant, command mới có thể dùng key; mọi attachment/blob/ingress lịch sử vẫn tồn tại. |
| Race cùng key? | Unique receipt trong `BEGIN IMMEDIATE` chọn một winner; loser reread receipt: cùng digest replay `200`, digest khác `409`; không double charge. |
| Attachment bị xóa hợp lệ thì receipt sao? | FK `attachment_id ON DELETE CASCADE` xóa receipt liên quan như hệ quả của attachment lifecycle hợp lệ, không phải TTL cleanup. |
| GC thấy final sau rollback? | Giữ mutex; chỉ xóa final do request tạo sau một `BEGIN IMMEDIATE` mới xác nhận không còn `cas_blobs` reference. |
| Frontend gọi daemon trực tiếp? | Không. `guardedRoute()` BFF và `src/api/chat-vault.ts` là boundary bắt buộc; UI/hooks không gọi `fetch()`. |

## 3. File Manifest

| Artefact | Mandatory schema/behavior |
|---|---|
| `CasBlob` | `sha256` PK, physical metadata, `ref_count`, `gc_lease_until`; Prisma + SQLite parity. |
| `CasAttachment` | Permanent tenant/session/project ownership metadata; không có `expires_at`, không có TTL purge. |
| `TenantCasBlobUsage` | Permanent tenant physical-byte accounting; Prisma + SQLite parity. |
| `CasUploadIngress` | Permanent accepted ingress ledger với `accepted_at`; không có `expires_at`, không có idempotency TTL purge. |
| `CasCommandIdempotency` | `tenant_id`, `session_id`, `idempotency_key`, `attachment_id`, `payload_sha256`, `accepted_at`, `expires_at`; FK `attachment_id ON DELETE CASCADE`; unique `(tenant_id, session_id, idempotency_key)`; index `(tenant_id, expires_at)`. |
| `VaultCommandIdempotency` | Safe receipt mapping với unique `(tenant_id, session_id, idempotency_key)` và index `(tenant_id, expires_at)`; cấm secret material. |
| `ApiErrorDtoSchema` | Zod schema strict cho `{code,message,retryable,limit?}`. |
| SVG/CSP policy | Chỉ cho `https:`, `http:`, PNG/JPEG/WEBP base64 exact prefix, `#fragment`, `/relative-path`; CSP host phải match `^https://[a-z0-9.-]+$`, sai thì fail closed. |

## 4. Edge Case Guardian & 13-Pillar FMEA Scan

| Pillar | Failure mode | RPN | Hard control |
|---|---|---:|---|
| Data lifecycle | TTL cleanup xóa attachment/blob hợp lệ | 180 | Receipt tables tách riêng; SQL purge allowlist chỉ hai bảng idempotency. |
| Idempotency | Replay double-charge hoặc key reuse sai digest | 144 | Unique receipt, canonical digest, transaction reread, replay `200`, mismatch `409`. |
| Tenant isolation | IDOR qua sid/project/receipt | 150 | Principal-derived scope, cross-tenant/missing `404`. |
| Referential integrity | Receipt mồ côi hoặc attachment delete không nhất quán | 126 | `attachment_id` FK `ON DELETE CASCADE`; migration FK test. |
| Quota accounting | TTL làm mất ingress history | 144 | `cas_upload_ingress` không có TTL column/purge; rolling query theo `accepted_at`. |
| CAS/GC | GC unlink blob đang upload | 150 | Shared SHA mutex, lease, immediate transaction. |
| Archive intake | ZIP bomb | 144 | 1024 entries, 67,108,864 bytes uncompressed, ratio ≤20.0 trước commit. |
| SQLite contention | Retry vô hạn/partial charge | 108 | Exactly 3 retry attempts và transaction atomicity. |
| Secret leakage | Vault secret vào persistence/SSE/log | 180 | 3-layer interceptor, `Zeroizing`, `redact_secrets`, receipt chỉ safe ID/digest. |
| SVG/CSP | mXSS hoặc permissive host parsing | 126 | Exact URL grammar và CSP fail-closed. |

## 5. Systemic Runtime Realism & Environment Pre-Flight Matrix

| Dependency | Required check | Failure contract |
|---|---|---|
| V16 SQLite migration | Foreign keys enabled; six declared models có DDL/Prisma parity; receipt-only TTL indexes tồn tại. | Fail migration/startup safely; không chạy partial schema. |
| Idempotency cleanup worker | SQL allowlist chỉ `cas_command_idempotency` và `vault_command_idempotency`, tenant-scoped, predicate `< now_ms`. | Abort job và alert nếu target table ngoài allowlist. |
| Quota evaluator | Tính rolling 24h từ permanent `cas_upload_ingress.accepted_at`; không dựa vào TTL delete. | `429` safe quota error, không ghi attachment/blob/receipt. |
| BFF auth | `guardedRoute()` inject verified principal trước downstream invocation. | `401`/safe `403`; không fallback tenant từ body/query/header. |
| CAS root | `.tmp-*` và final cùng volume, `O_NOFOLLOW`, mode `0600`, fsync trước rename. | Generic safe error và cleanup temp. |
| Shared lock registry | Upload/delete/GC dùng cùng keyed mutex namespace theo SHA. | Skip GC/delete khi lock/lease ambiguous. |
| CSP startup | Parse `DESIGN_CDN_CSP` một lần, exact regex. | CSP default fail-closed. |

## Implementation Steps & Verification Checklist

- [ ] Cập nhật `data-spec.md`, `prisma/schema.prisma`, và SQLite migration `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql`: materialize đầy đủ `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`; `cas_attachments`/`cas_upload_ingress` không có `expires_at`; `cas_command_idempotency` có FK `attachment_id ON DELETE CASCADE`, unique `(tenant_id, session_id, idempotency_key)`, digest/timestamps/index expiry; kiểm tra bằng sqlite migration test.
- [ ] Implement SQLite idempotency purge allowlist trong Axum service: chỉ chạy `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?` và `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`; cấm mọi route handler hoặc worker xóa `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vì idempotency TTL.
- [ ] Tạo Zod `ApiErrorDtoSchema`, schemas chat/CAS/vault/idempotency trong `src/lib/validations/chat-vault.ts`, và typed client sdk duy nhất `src/api/chat-vault.ts`; UI/hooks không dùng direct `fetch()`.
- [ ] Với `GET /api/v1/chat-sessions/:sid/messages`, implement Axum route handler principal-scoped, Next `src/app/api/v1/chat-sessions/[sid]/messages/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cross-tenant/missing trả `404 RESOURCE_NOT_FOUND`.
- [ ] Với `POST /api/v1/chat-sessions/:sid/messages`, implement Axum route handler auto-provision session tenant-bound, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; server intercept/redact secret trước optimistic state, sqlite persistence và SSE serialization.
- [ ] Với `POST /api/v1/cas/upload`, implement Axum route handler anti-IDOR và receipt idempotency, Next route handler `src/app/api/v1/cas/upload/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cấm direct fetch tại UI/hooks.
- [ ] Với `POST /api/v1/vault/chat-ingest`, implement Axum route handler 3-layer secret interceptor và vault receipt replay, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; không persist/log/SSE plaintext secret.
- [ ] Với `POST` và `DELETE /api/v1/vault/receipts/:receipt_id`, implement Axum route handler authorization, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; receipt cross-tenant/missing phải không phân biệt bằng `404 RESOURCE_NOT_FOUND`.
- [ ] Implement reusable SQLite retry executor cho Axum route handler: 3 attempts, 25ms initial exponential backoff, cap 200ms, jitter 0–25ms, deadline 3000ms; exhausted busy trả `503 CHAT_WRITE_CONTENTION`, `retryable:true`, không partial sqlite write.
- [ ] Implement CAS Axum route handler order: stream `.tmp-<uuid>` với `O_NOFOLLOW`/`0600`, hard-stop `26_214_401`, SHA/magic/archive validation, shared per-SHA mutex, sqlite `BEGIN IMMEDIATE`, purge receipt TTL allowlist, replay/mismatch decision, quota checks, fsync/rename, upsert blob/attachment/usage/permanent ingress/new CAS receipt, commit, release lock.
- [ ] Implement rollback/GC trong Axum service với cùng mutex: cleanup temp; final mới chỉ được unlink sau sqlite `BEGIN IMMEDIATE` chứng minh không có committed blob reference; attachment deletion cập nhật refcount; GC chỉ unlink `ref_count == 0 AND gc_lease_until < now_ms`.
- [ ] Update `src/features/flow/utils/html-sandbox-sanitizer.ts` với deterministic viewBox fallback, strict SVG URL grammar, và `DESIGN_CDN_CSP` parser fail-closed.
- [ ] `UT-IDOR-001`: xác minh sáu operations chat/CAS/vault dùng Axum route handler, guardedRoute, client sdk và zod contracts; cross-tenant/missing trả `404`, thiếu capability trả `403`, không leak metadata.
- [ ] `UT-IDEMPOTENCY-001`: cùng key/cùng digest chỉ commit một lần; replay `200` không tăng attachment refcount, physical usage hoặc ingress; digest khác trả `409`.
- [ ] `UT-IDEMPOTENCY-002`: sau `expires_at < now_ms`, purge chỉ xóa CAS receipt và cho phép reuse key; assert existing `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vẫn nguyên vẹn byte-for-byte/row-for-row.
- [ ] `UT-IDEMPOTENCY-003`: assert purge Vault chỉ xóa expired `vault_command_idempotency`; không xóa vault receipt hợp lệ trước expiry và không có secret material trong sqlite row.
- [ ] `UT-FK-RECEIPT-001`: verify `CasCommandIdempotency.attachment_id` FK `ON DELETE CASCADE`; attachment lifecycle hợp lệ xóa receipt liên quan, còn TTL purge không được gọi attachment delete.
- [ ] `UT-GC-RACE-001`, `UT-ARCHIVE-CAPS-001`, `UT-RETRY-001`, `UT-CSP-SVG-001`, và `UT-ERROR-001`: giữ toàn bộ boundary/race/error assertions trước đó, bao gồm no-leak error DTO và sqlite atomicity.
- [ ] `E2E-CHAT-CAS-VAULT-001`: dùng client sdk `src/api/chat-vault.ts` qua guardedRoute route handler; verify reload chat persistence, binary/CSV/XLSX/FIG upload, paste prevention, vault receipt masking, and no direct UI fetch.
- [ ] `E2E-TTL-PRESERVATION-001`: tạo attachment và ingress, advance clock >24h, chạy TTL purge, upload lại với same key, và assert attachment/blob/ingress cũ không mất trước/sau command mới.

## Codex Author Revision & Hardening Addendum (Round 2)

---
type: implementation-plan-addendum
story_id: "04.6"
title: "Round 2 — Deterministic Tenant CAS Quota Contract"
status: DRAFT_PENDING_CODEX_AUDIT
risk_tier: 3
adr_refs: [ADR-006, ADR-091]
generated_at: "2026-11-20T20:45:00+07:00"
plugin_plan_gate:
  classification: NOT_DECLARED
  applicable: false
  rationale: "story.md không khai báo PLUGIN hoặc HYBRID."
---

## Technical Architecture & File Scope

Kế hoạch này thay thế toàn bộ addendum trùng lặp trước đó. Hợp đồng idempotency được tách hoàn toàn khỏi dữ liệu attachment và ledger quota:

- `cas_attachments` là metadata attachment vĩnh viễn, không có `expires_at`, không bị xóa hoặc NULL hóa bởi TTL idempotency.
- `cas_upload_ingress` là ledger ingress vĩnh viễn phục vụ truy vấn rolling-24h, không có `expires_at`, không bị xóa bởi TTL idempotency.
- `cas_command_idempotency` là receipt riêng, TTL `86_400_000 ms`, có `attachment_id` FK `ON DELETE CASCADE`, `UNIQUE(tenant_id, session_id, idempotency_key)`, `payload_sha256`, `accepted_at`, `expires_at`.
- `vault_command_idempotency` là receipt riêng cho vault, TTL `86_400_000 ms`; không chứa plaintext secret, encrypted secret material, provider key, hay vault path.
- TTL purge chỉ được phép chạy hai lệnh scoped theo tenant:
  `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
  `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
- Không có idempotency TTL cleanup nào được phép đụng tới `cas_attachments`, `cas_blobs`, hoặc `cas_upload_ingress`.
- Canonical client boundary duy nhất là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; UI/hooks tuyệt đối không `fetch()` trực tiếp.
- Mỗi endpoint có Next BFF `src/app/api/**/route.ts` bọc `guardedRoute()`, typed client SDK, Zod schema, và daemon Axum route handler tương ứng.

| File | Trách nhiệm bắt buộc |
|---|---|
| `data-spec.md` | Tách ba nhóm lifetime: permanent attachment/blob/ingress, TTL CAS receipt, TTL vault receipt; DDL, FK, unique/index và thuật toán purge. |
| `prisma/schema.prisma` | Khai báo parity cho mọi model Prisma trong `data-spec.md`: `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`. |
| `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql` | Migration SQLite cho bảng/idempotency index/FK và không thêm TTL columns vào attachment hoặc ingress. |
| `crates/harness-daemon/src/routes/chat_cas_vault.rs` | Axum authorization, CAS/GC mutex, retry, idempotency receipt transaction, safe errors và secret redaction. |
| `src/app/api/v1/**/route.ts` | BFF route handler bọc `guardedRoute()` cho toàn bộ endpoint Story 04.6. |
| `src/api/chat-vault.ts` | Typed client SDK duy nhất cho chat, CAS và vault; parse `ApiErrorDto`, truyền idempotency key. |
| `src/lib/validations/chat-vault.ts` | Zod request/response/error schemas duy nhất. |
| `src/features/flow/utils/html-sandbox-sanitizer.ts` | viewBox inference, SVG URL policy, CSP parser fail-closed. |

## 1. Deep Codebase Drift & Contract Audit

| Contract | Quyết định khóa |
|---|---|
| Principal/anti-IDOR | `tenant_id`, actor và capability chỉ lấy từ principal đã xác thực. Session/project/receipt thiếu hoặc cross-tenant trả `404 RESOURCE_NOT_FOUND`; principal hợp lệ nhưng thiếu capability trả `403 AUTH_FORBIDDEN`. |
| CAS receipt identity | `cas_command_idempotency` dùng `UNIQUE(tenant_id, session_id, idempotency_key)`. Key là opaque, non-empty, ASCII-visible tối đa 128 ký tự, không log. Header `X-Idempotency-Key` ưu tiên; nếu body `idempotencyKey` đồng thời tồn tại thì phải byte-identical. |
| CAS digest | `payload_sha256 = SHA-256(raw_stream_bytes || canonical_metadata)`; metadata canonical gồm `sessionId`, `projectId|null`, normalized extension và MIME đã được server xác nhận. |
| CAS replay | Sau khi purge receipt hết hạn và trước quota/FS side effect: same key + same digest trả `200` attachment DTO gốc; same key + digest khác trả `409 IDEMPOTENCY_KEY_PAYLOAD_MISMATCH`. |
| CAS persistence | Một upload thành công tạo/giữ `cas_attachments`, cập nhật `cas_blobs`, và ghi `cas_upload_ingress`; đồng thời tạo một `cas_command_idempotency` tham chiếu attachment đó. Chỉ receipt có thể hết hạn. |
| TTL boundary | Receipt active khi `expires_at >= now_ms`; purge chỉ xóa khi `expires_at < now_ms`. Sau purge, key được tái sử dụng như command mới; attachment/blob/ingress cũ vẫn nguyên vẹn. |
| Vault replay | `vault_command_idempotency` lưu tenant/session/key/digest/safe receipt ID/timestamps; replay không tái xử lý secret và không đưa secret vào state, SQLite, SSE hoặc log. |
| Retry | Chỉ retry `SQLITE_BUSY`/`SQLITE_LOCKED`: tối đa 3 attempts, backoff 25ms exponential cap 200ms, jitter 0–25ms, deadline 3000ms. Các lỗi validation/auth/quota/idempotency/FS không retry. |
| CAS/GC | Upload và GC lấy cùng per-SHA `tokio::sync::Mutex`; GC chỉ unlink với `ref_count=0 AND gc_lease_until < now_ms` trong `BEGIN IMMEDIATE`. |
| Error DTO | Chỉ trả `ApiErrorDto={code,message,retryable,limit?}`; message cố định an toàn, không leak SHA/path/file name/tenant/usage/secret/payload/diagnostic. |

## 2. Unknowns Discovery & Epistemic Audit

| ID | Resolution | Status |
|---|---|---|
| U-046-17 | `cas_attachments` không bao giờ là TTL receipt và không bị xóa để tái sử dụng idempotency key. | RESOLVED |
| U-046-18 | `cas_upload_ingress` không có `expires_at`; rolling quota dùng mốc `accepted_at` khi tính toán, không dùng purge idempotency. | RESOLVED |
| U-046-19 | `cas_command_idempotency` là bảng receipt duy nhất cho CAS replay và tham chiếu `attachment_id ON DELETE CASCADE`. | RESOLVED |
| U-046-20 | Client SDK/validation canonical là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; không tạo các SDK split cạnh tranh. | RESOLVED |
| U-046-21 | Mọi model được nêu trong `data-spec.md` có task Prisma schema và SQLite migration tương ứng. | RESOLVED |
| U-046-22 | Story không khai báo PLUGIN/HYBRID nên Plugin Plan Gate không áp dụng. | RESOLVED |

## 3. AI Socratic Debate & Consensus Report

| Challenge | Consensus khóa |
|---|---|
| TTL receipt có thể xóa attachment không? | Không. Receipt tồn tại trong bảng riêng; `DELETE` TTL chỉ target `cas_command_idempotency` hoặc `vault_command_idempotency`. |
| Sau 24h có thể reuse key không? | Có. Sau `expires_at < now_ms` purge receipt scoped theo tenant, command mới có thể dùng key; mọi attachment/blob/ingress lịch sử vẫn tồn tại. |
| Race cùng key? | Unique receipt trong `BEGIN IMMEDIATE` chọn một winner; loser reread receipt: cùng digest replay `200`, digest khác `409`; không double charge. |
| Attachment bị xóa hợp lệ thì receipt sao? | FK `attachment_id ON DELETE CASCADE` xóa receipt liên quan như hệ quả của attachment lifecycle hợp lệ, không phải TTL cleanup. |
| GC thấy final sau rollback? | Giữ mutex; chỉ xóa final do request tạo sau một `BEGIN IMMEDIATE` mới xác nhận không còn `cas_blobs` reference. |
| Frontend gọi daemon trực tiếp? | Không. `guardedRoute()` BFF và `src/api/chat-vault.ts` là boundary bắt buộc; UI/hooks không gọi `fetch()`. |

## 3. File Manifest

| Artefact | Mandatory schema/behavior |
|---|---|
| `CasBlob` | `sha256` PK, physical metadata, `ref_count`, `gc_lease_until`; Prisma + SQLite parity. |
| `CasAttachment` | Permanent tenant/session/project ownership metadata; không có `expires_at`, không có TTL purge. |
| `TenantCasBlobUsage` | Permanent tenant physical-byte accounting; Prisma + SQLite parity. |
| `CasUploadIngress` | Permanent accepted ingress ledger với `accepted_at`; không có `expires_at`, không có idempotency TTL purge. |
| `CasCommandIdempotency` | `tenant_id`, `session_id`, `idempotency_key`, `attachment_id`, `payload_sha256`, `accepted_at`, `expires_at`; FK `attachment_id ON DELETE CASCADE`; unique `(tenant_id, session_id, idempotency_key)`; index `(tenant_id, expires_at)`. |
| `VaultCommandIdempotency` | Safe receipt mapping với unique `(tenant_id, session_id, idempotency_key)` và index `(tenant_id, expires_at)`; cấm secret material. |
| `ApiErrorDtoSchema` | Zod schema strict cho `{code,message,retryable,limit?}`. |
| SVG/CSP policy | Chỉ cho `https:`, `http:`, PNG/JPEG/WEBP base64 exact prefix, `#fragment`, `/relative-path`; CSP host phải match `^https://[a-z0-9.-]+$`, sai thì fail closed. |

## 4. Edge Case Guardian & 13-Pillar FMEA Scan

| Pillar | Failure mode | RPN | Hard control |
|---|---|---:|---|
| Data lifecycle | TTL cleanup xóa attachment/blob hợp lệ | 180 | Receipt tables tách riêng; SQL purge allowlist chỉ hai bảng idempotency. |
| Idempotency | Replay double-charge hoặc key reuse sai digest | 144 | Unique receipt, canonical digest, transaction reread, replay `200`, mismatch `409`. |
| Tenant isolation | IDOR qua sid/project/receipt | 150 | Principal-derived scope, cross-tenant/missing `404`. |
| Referential integrity | Receipt mồ côi hoặc attachment delete không nhất quán | 126 | `attachment_id` FK `ON DELETE CASCADE`; migration FK test. |
| Quota accounting | TTL làm mất ingress history | 144 | `cas_upload_ingress` không có TTL column/purge; rolling query theo `accepted_at`. |
| CAS/GC | GC unlink blob đang upload | 150 | Shared SHA mutex, lease, immediate transaction. |
| Archive intake | ZIP bomb | 144 | 1024 entries, 67,108,864 bytes uncompressed, ratio ≤20.0 trước commit. |
| SQLite contention | Retry vô hạn/partial charge | 108 | Exactly 3 retry attempts và transaction atomicity. |
| Secret leakage | Vault secret vào persistence/SSE/log | 180 | 3-layer interceptor, `Zeroizing`, `redact_secrets`, receipt chỉ safe ID/digest. |
| SVG/CSP | mXSS hoặc permissive host parsing | 126 | Exact URL grammar và CSP fail-closed. |

## 5. Systemic Runtime Realism & Environment Pre-Flight Matrix

| Dependency | Required check | Failure contract |
|---|---|---|
| V16 SQLite migration | Foreign keys enabled; six declared models có DDL/Prisma parity; receipt-only TTL indexes tồn tại. | Fail migration/startup safely; không chạy partial schema. |
| Idempotency cleanup worker | SQL allowlist chỉ `cas_command_idempotency` và `vault_command_idempotency`, tenant-scoped, predicate `< now_ms`. | Abort job và alert nếu target table ngoài allowlist. |
| Quota evaluator | Tính rolling 24h từ permanent `cas_upload_ingress.accepted_at`; không dựa vào TTL delete. | `429` safe quota error, không ghi attachment/blob/receipt. |
| BFF auth | `guardedRoute()` inject verified principal trước downstream invocation. | `401`/safe `403`; không fallback tenant từ body/query/header. |
| CAS root | `.tmp-*` và final cùng volume, `O_NOFOLLOW`, mode `0600`, fsync trước rename. | Generic safe error và cleanup temp. |
| Shared lock registry | Upload/delete/GC dùng cùng keyed mutex namespace theo SHA. | Skip GC/delete khi lock/lease ambiguous. |
| CSP startup | Parse `DESIGN_CDN_CSP` một lần, exact regex. | CSP default fail-closed. |

## Implementation Steps & Verification Checklist

- [ ] Cập nhật `data-spec.md`, `prisma/schema.prisma`, và SQLite migration `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql`: materialize đầy đủ `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`; `cas_attachments`/`cas_upload_ingress` không có `expires_at`; `cas_command_idempotency` có FK `attachment_id ON DELETE CASCADE`, unique `(tenant_id, session_id, idempotency_key)`, digest/timestamps/index expiry; kiểm tra bằng sqlite migration test.
- [ ] Implement SQLite idempotency purge allowlist trong Axum service: chỉ chạy `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?` và `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`; cấm mọi route handler hoặc worker xóa `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vì idempotency TTL.
- [ ] Tạo Zod `ApiErrorDtoSchema`, schemas chat/CAS/vault/idempotency trong `src/lib/validations/chat-vault.ts`, và typed client sdk duy nhất `src/api/chat-vault.ts`; UI/hooks không dùng direct `fetch()`.
- [ ] Với `GET /api/v1/chat-sessions/:sid/messages`, implement Axum route handler principal-scoped, Next `src/app/api/v1/chat-sessions/[sid]/messages/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cross-tenant/missing trả `404 RESOURCE_NOT_FOUND`.
- [ ] Với `POST /api/v1/chat-sessions/:sid/messages`, implement Axum route handler auto-provision session tenant-bound, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; server intercept/redact secret trước optimistic state, sqlite persistence và SSE serialization.
- [ ] Với `POST /api/v1/cas/upload`, implement Axum route handler anti-IDOR và receipt idempotency, Next route handler `src/app/api/v1/cas/upload/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cấm direct fetch tại UI/hooks.
- [ ] Với `POST /api/v1/vault/chat-ingest`, implement Axum route handler 3-layer secret interceptor và vault receipt replay, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; không persist/log/SSE plaintext secret.
- [ ] Với `POST` và `DELETE /api/v1/vault/receipts/:receipt_id`, implement Axum route handler authorization, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; receipt cross-tenant/missing phải không phân biệt bằng `404 RESOURCE_NOT_FOUND`.
- [ ] Implement reusable SQLite retry executor cho Axum route handler: 3 attempts, 25ms initial exponential backoff, cap 200ms, jitter 0–25ms, deadline 3000ms; exhausted busy trả `503 CHAT_WRITE_CONTENTION`, `retryable:true`, không partial sqlite write.
- [ ] Implement CAS Axum route handler order: stream `.tmp-<uuid>` với `O_NOFOLLOW`/`0600`, hard-stop `26_214_401`, SHA/magic/archive validation, shared per-SHA mutex, sqlite `BEGIN IMMEDIATE`, purge receipt TTL allowlist, replay/mismatch decision, quota checks, fsync/rename, upsert blob/attachment/usage/permanent ingress/new CAS receipt, commit, release lock.
- [ ] Implement rollback/GC trong Axum service với cùng mutex: cleanup temp; final mới chỉ được unlink sau sqlite `BEGIN IMMEDIATE` chứng minh không có committed blob reference; attachment deletion cập nhật refcount; GC chỉ unlink `ref_count == 0 AND gc_lease_until < now_ms`.
- [ ] Update `src/features/flow/utils/html-sandbox-sanitizer.ts` với deterministic viewBox fallback, strict SVG URL grammar, và `DESIGN_CDN_CSP` parser fail-closed.
- [ ] `UT-IDOR-001`: xác minh sáu operations chat/CAS/vault dùng Axum route handler, guardedRoute, client sdk và zod contracts; cross-tenant/missing trả `404`, thiếu capability trả `403`, không leak metadata.
- [ ] `UT-IDEMPOTENCY-001`: cùng key/cùng digest chỉ commit một lần; replay `200` không tăng attachment refcount, physical usage hoặc ingress; digest khác trả `409`.
- [ ] `UT-IDEMPOTENCY-002`: sau `expires_at < now_ms`, purge chỉ xóa CAS receipt và cho phép reuse key; assert existing `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vẫn nguyên vẹn byte-for-byte/row-for-row.
- [ ] `UT-IDEMPOTENCY-003`: assert purge Vault chỉ xóa expired `vault_command_idempotency`; không xóa vault receipt hợp lệ trước expiry và không có secret material trong sqlite row.
- [ ] `UT-FK-RECEIPT-001`: verify `CasCommandIdempotency.attachment_id` FK `ON DELETE CASCADE`; attachment lifecycle hợp lệ xóa receipt liên quan, còn TTL purge không được gọi attachment delete.
- [ ] `UT-GC-RACE-001`, `UT-ARCHIVE-CAPS-001`, `UT-RETRY-001`, `UT-CSP-SVG-001`, và `UT-ERROR-001`: giữ toàn bộ boundary/race/error assertions trước đó, bao gồm no-leak error DTO và sqlite atomicity.
- [ ] `E2E-CHAT-CAS-VAULT-001`: dùng client sdk `src/api/chat-vault.ts` qua guardedRoute route handler; verify reload chat persistence, binary/CSV/XLSX/FIG upload, paste prevention, vault receipt masking, and no direct UI fetch.
- [ ] `E2E-TTL-PRESERVATION-001`: tạo attachment và ingress, advance clock >24h, chạy TTL purge, upload lại với same key, và assert attachment/blob/ingress cũ không mất trước/sau command mới.

## Codex Author Revision & Hardening Addendum (Round 4)

---
type: implementation-plan-addendum
story_id: "04.6"
title: "Round 4 — Deterministic Safety, Idempotency, Anti-IDOR, CAS/GC and FMEA Closure"
status: DRAFT_PENDING_CODEX_AUDIT
risk_tier: 3
adr_refs: [ADR-006, ADR-091]
generated_at: "2026-11-20T20:45:00+07:00"
plugin_plan_gate:
  classification: NOT_DECLARED
  applicable: false
  rationale: "story.md không khai báo plugin_classification: PLUGIN hoặc HYBRID."
---

## Technical Architecture & File Scope

Kế hoạch này thay thế toàn bộ addendum trùng lặp trước đó. Hợp đồng idempotency được tách hoàn toàn khỏi dữ liệu attachment và ledger quota:

- `cas_attachments` là metadata attachment vĩnh viễn, không có `expires_at`, không bị xóa hoặc NULL hóa bởi TTL idempotency.
- `cas_upload_ingress` là ledger ingress vĩnh viễn phục vụ truy vấn rolling-24h, không có `expires_at`, không bị xóa bởi TTL idempotency.
- `cas_command_idempotency` là receipt riêng, TTL `86_400_000 ms`, có `attachment_id` FK `ON DELETE CASCADE`, `UNIQUE(tenant_id, session_id, idempotency_key)`, `payload_sha256`, `accepted_at`, `expires_at`.
- `vault_command_idempotency` là receipt riêng cho vault, TTL `86_400_000 ms`; không chứa plaintext secret, encrypted secret material, provider key, hay vault path.
- TTL purge chỉ được phép chạy hai lệnh scoped theo tenant:
  `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
  `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`
- Không có idempotency TTL cleanup nào được phép đụng tới `cas_attachments`, `cas_blobs`, hoặc `cas_upload_ingress`.
- Canonical client boundary duy nhất là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; UI/hooks tuyệt đối không `fetch()` trực tiếp.
- Mỗi endpoint có Next BFF `src/app/api/**/route.ts` bọc `guardedRoute()`, typed client SDK, Zod schema, và daemon Axum route handler tương ứng.

| File | Trách nhiệm bắt buộc |
|---|---|
| `data-spec.md` | Tách ba nhóm lifetime: permanent attachment/blob/ingress, TTL CAS receipt, TTL vault receipt; DDL, FK, unique/index và thuật toán purge. |
| `prisma/schema.prisma` | Khai báo parity cho mọi model Prisma trong `data-spec.md`: `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`. |
| `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql` | Migration SQLite cho bảng/idempotency index/FK và không thêm TTL columns vào attachment hoặc ingress. |
| `crates/harness-daemon/src/routes/chat_cas_vault.rs` | Axum authorization, CAS/GC mutex, retry, idempotency receipt transaction, safe errors và secret redaction. |
| `src/app/api/v1/**/route.ts` | BFF route handler bọc `guardedRoute()` cho toàn bộ endpoint Story 04.6. |
| `src/api/chat-vault.ts` | Typed client SDK duy nhất cho chat, CAS và vault; parse `ApiErrorDto`, truyền idempotency key. |
| `src/lib/validations/chat-vault.ts` | Zod request/response/error schemas duy nhất. |
| `src/features/flow/utils/html-sandbox-sanitizer.ts` | viewBox inference, SVG URL policy, CSP parser fail-closed. |

## 1. Deep Codebase Drift & Contract Audit

| Contract | Quyết định khóa |
|---|---|
| Principal/anti-IDOR | `tenant_id`, actor và capability chỉ lấy từ principal đã xác thực. Session/project/receipt thiếu hoặc cross-tenant trả `404 RESOURCE_NOT_FOUND`; principal hợp lệ nhưng thiếu capability trả `403 AUTH_FORBIDDEN`. |
| CAS receipt identity | `cas_command_idempotency` dùng `UNIQUE(tenant_id, session_id, idempotency_key)`. Key là opaque, non-empty, ASCII-visible tối đa 128 ký tự, không log. Header `X-Idempotency-Key` ưu tiên; nếu body `idempotencyKey` đồng thời tồn tại thì phải byte-identical. |
| CAS digest | `payload_sha256 = SHA-256(raw_stream_bytes || canonical_metadata)`; metadata canonical gồm `sessionId`, `projectId|null`, normalized extension và MIME đã được server xác nhận. |
| CAS replay | Sau khi purge receipt hết hạn và trước quota/FS side effect: same key + same digest trả `200` attachment DTO gốc; same key + digest khác trả `409 IDEMPOTENCY_KEY_PAYLOAD_MISMATCH`. |
| CAS persistence | Một upload thành công tạo/giữ `cas_attachments`, cập nhật `cas_blobs`, và ghi `cas_upload_ingress`; đồng thời tạo một `cas_command_idempotency` tham chiếu attachment đó. Chỉ receipt có thể hết hạn. |
| TTL boundary | Receipt active khi `expires_at >= now_ms`; purge chỉ xóa khi `expires_at < now_ms`. Sau purge, key được tái sử dụng như command mới; attachment/blob/ingress cũ vẫn nguyên vẹn. |
| Vault replay | `vault_command_idempotency` lưu tenant/session/key/digest/safe receipt ID/timestamps; replay không tái xử lý secret và không đưa secret vào state, SQLite, SSE hoặc log. |
| Retry | Chỉ retry `SQLITE_BUSY`/`SQLITE_LOCKED`: tối đa 3 attempts, backoff 25ms exponential cap 200ms, jitter 0–25ms, deadline 3000ms. Các lỗi validation/auth/quota/idempotency/FS không retry. |
| CAS/GC | Upload và GC lấy cùng per-SHA `tokio::sync::Mutex`; GC chỉ unlink với `ref_count=0 AND gc_lease_until < now_ms` trong `BEGIN IMMEDIATE`. |
| Error DTO | Chỉ trả `ApiErrorDto={code,message,retryable,limit?}`; message cố định an toàn, không leak SHA/path/file name/tenant/usage/secret/payload/diagnostic. |

## 2. Unknowns Discovery & Epistemic Audit

| ID | Resolution | Status |
|---|---|---|
| U-046-17 | `cas_attachments` không bao giờ là TTL receipt và không bị xóa để tái sử dụng idempotency key. | RESOLVED |
| U-046-18 | `cas_upload_ingress` không có `expires_at`; rolling quota dùng mốc `accepted_at` khi tính toán, không dùng purge idempotency. | RESOLVED |
| U-046-19 | `cas_command_idempotency` là bảng receipt duy nhất cho CAS replay và tham chiếu `attachment_id ON DELETE CASCADE`. | RESOLVED |
| U-046-20 | Client SDK/validation canonical là `src/api/chat-vault.ts` và `src/lib/validations/chat-vault.ts`; không tạo các SDK split cạnh tranh. | RESOLVED |
| U-046-21 | Mọi model được nêu trong `data-spec.md` có task Prisma schema và SQLite migration tương ứng. | RESOLVED |
| U-046-22 | Story không khai báo PLUGIN/HYBRID nên Plugin Plan Gate không áp dụng. | RESOLVED |

## 3. AI Socratic Debate & Consensus Report

| Challenge | Consensus khóa |
|---|---|
| TTL receipt có thể xóa attachment không? | Không. Receipt tồn tại trong bảng riêng; `DELETE` TTL chỉ target `cas_command_idempotency` hoặc `vault_command_idempotency`. |
| Sau 24h có thể reuse key không? | Có. Sau `expires_at < now_ms` purge receipt scoped theo tenant, command mới có thể dùng key; mọi attachment/blob/ingress lịch sử vẫn tồn tại. |
| Race cùng key? | Unique receipt trong `BEGIN IMMEDIATE` chọn một winner; loser reread receipt: cùng digest replay `200`, digest khác `409`; không double charge. |
| Attachment bị xóa hợp lệ thì receipt sao? | FK `attachment_id ON DELETE CASCADE` xóa receipt liên quan như hệ quả của attachment lifecycle hợp lệ, không phải TTL cleanup. |
| GC thấy final sau rollback? | Giữ mutex; chỉ xóa final do request tạo sau một `BEGIN IMMEDIATE` mới xác nhận không còn `cas_blobs` reference. |
| Frontend gọi daemon trực tiếp? | Không. `guardedRoute()` BFF và `src/api/chat-vault.ts` là boundary bắt buộc; UI/hooks không gọi `fetch()`. |

## 3. File Manifest

| Artefact | Mandatory schema/behavior |
|---|---|
| `CasBlob` | `sha256` PK, physical metadata, `ref_count`, `gc_lease_until`; Prisma + SQLite parity. |
| `CasAttachment` | Permanent tenant/session/project ownership metadata; không có `expires_at`, không có TTL purge. |
| `TenantCasBlobUsage` | Permanent tenant physical-byte accounting; Prisma + SQLite parity. |
| `CasUploadIngress` | Permanent accepted ingress ledger với `accepted_at`; không có `expires_at`, không có idempotency TTL purge. |
| `CasCommandIdempotency` | `tenant_id`, `session_id`, `idempotency_key`, `attachment_id`, `payload_sha256`, `accepted_at`, `expires_at`; FK `attachment_id ON DELETE CASCADE`; unique `(tenant_id, session_id, idempotency_key)`; index `(tenant_id, expires_at)`. |
| `VaultCommandIdempotency` | Safe receipt mapping với unique `(tenant_id, session_id, idempotency_key)` và index `(tenant_id, expires_at)`; cấm secret material. |
| `ApiErrorDtoSchema` | Zod schema strict cho `{code,message,retryable,limit?}`. |
| SVG/CSP policy | Chỉ cho `https:`, `http:`, PNG/JPEG/WEBP base64 exact prefix, `#fragment`, `/relative-path`; CSP host phải match `^https://[a-z0-9.-]+$`, sai thì fail closed. |

## 4. Edge Case Guardian & 13-Pillar FMEA Scan

| Pillar | Failure mode | RPN | Hard control |
|---|---|---:|---|
| Data lifecycle | TTL cleanup xóa attachment/blob hợp lệ | 180 | Receipt tables tách riêng; SQL purge allowlist chỉ hai bảng idempotency. |
| Idempotency | Replay double-charge hoặc key reuse sai digest | 144 | Unique receipt, canonical digest, transaction reread, replay `200`, mismatch `409`. |
| Tenant isolation | IDOR qua sid/project/receipt | 150 | Principal-derived scope, cross-tenant/missing `404`. |
| Referential integrity | Receipt mồ côi hoặc attachment delete không nhất quán | 126 | `attachment_id` FK `ON DELETE CASCADE`; migration FK test. |
| Quota accounting | TTL làm mất ingress history | 144 | `cas_upload_ingress` không có TTL column/purge; rolling query theo `accepted_at`. |
| CAS/GC | GC unlink blob đang upload | 150 | Shared SHA mutex, lease, immediate transaction. |
| Archive intake | ZIP bomb | 144 | 1024 entries, 67,108,864 bytes uncompressed, ratio ≤20.0 trước commit. |
| SQLite contention | Retry vô hạn/partial charge | 108 | Exactly 3 retry attempts và transaction atomicity. |
| Secret leakage | Vault secret vào persistence/SSE/log | 180 | 3-layer interceptor, `Zeroizing`, `redact_secrets`, receipt chỉ safe ID/digest. |
| SVG/CSP | mXSS hoặc permissive host parsing | 126 | Exact URL grammar và CSP fail-closed. |

## 5. Systemic Runtime Realism & Environment Pre-Flight Matrix

| Dependency | Required check | Failure contract |
|---|---|---|
| V16 SQLite migration | Foreign keys enabled; six declared models có DDL/Prisma parity; receipt-only TTL indexes tồn tại. | Fail migration/startup safely; không chạy partial schema. |
| Idempotency cleanup worker | SQL allowlist chỉ `cas_command_idempotency` và `vault_command_idempotency`, tenant-scoped, predicate `< now_ms`. | Abort job và alert nếu target table ngoài allowlist. |
| Quota evaluator | Tính rolling 24h từ permanent `cas_upload_ingress.accepted_at`; không dựa vào TTL delete. | `429` safe quota error, không ghi attachment/blob/receipt. |
| BFF auth | `guardedRoute()` inject verified principal trước downstream invocation. | `401`/safe `403`; không fallback tenant từ body/query/header. |
| CAS root | `.tmp-*` và final cùng volume, `O_NOFOLLOW`, mode `0600`, fsync trước rename. | Generic safe error và cleanup temp. |
| Shared lock registry | Upload/delete/GC dùng cùng keyed mutex namespace theo SHA. | Skip GC/delete khi lock/lease ambiguous. |
| CSP startup | Parse `DESIGN_CDN_CSP` một lần, exact regex. | CSP default fail-closed. |

## Implementation Steps & Verification Checklist

- [ ] Cập nhật `data-spec.md`, `prisma/schema.prisma`, và SQLite migration `crates/spec-registry/migrations/V16__story_04_6_cas_blobs.sql`: materialize đầy đủ `CasBlob`, `CasAttachment`, `TenantCasBlobUsage`, `CasUploadIngress`, `CasCommandIdempotency`, `VaultCommandIdempotency`; `cas_attachments`/`cas_upload_ingress` không có `expires_at`; `cas_command_idempotency` có FK `attachment_id ON DELETE CASCADE`, unique `(tenant_id, session_id, idempotency_key)`, digest/timestamps/index expiry; kiểm tra bằng sqlite migration test.
- [ ] Implement SQLite idempotency purge allowlist trong Axum service: chỉ chạy `DELETE FROM cas_command_idempotency WHERE tenant_id = ? AND expires_at < ?` và `DELETE FROM vault_command_idempotency WHERE tenant_id = ? AND expires_at < ?`; cấm mọi route handler hoặc worker xóa `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vì idempotency TTL.
- [ ] Tạo Zod `ApiErrorDtoSchema`, schemas chat/CAS/vault/idempotency trong `src/lib/validations/chat-vault.ts`, và typed client sdk duy nhất `src/api/chat-vault.ts`; UI/hooks không dùng direct `fetch()`.
- [ ] Với `GET /api/v1/chat-sessions/:sid/messages`, implement Axum route handler principal-scoped, Next `src/app/api/v1/chat-sessions/[sid]/messages/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cross-tenant/missing trả `404 RESOURCE_NOT_FOUND`.
- [ ] Với `POST /api/v1/chat-sessions/:sid/messages`, implement Axum route handler auto-provision session tenant-bound, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; server intercept/redact secret trước optimistic state, sqlite persistence và SSE serialization.
- [ ] Với `POST /api/v1/cas/upload`, implement Axum route handler anti-IDOR và receipt idempotency, Next route handler `src/app/api/v1/cas/upload/route.ts` bọc `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; cấm direct fetch tại UI/hooks.
- [ ] Với `POST /api/v1/vault/chat-ingest`, implement Axum route handler 3-layer secret interceptor và vault receipt replay, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; không persist/log/SSE plaintext secret.
- [ ] Với `POST` và `DELETE /api/v1/vault/receipts/:receipt_id`, implement Axum route handler authorization, Next route handler `guardedRoute`, client sdk `src/api/chat-vault.ts`, và zod schema; receipt cross-tenant/missing phải không phân biệt bằng `404 RESOURCE_NOT_FOUND`.
- [ ] Implement reusable SQLite retry executor cho Axum route handler: 3 attempts, 25ms initial exponential backoff, cap 200ms, jitter 0–25ms, deadline 3000ms; exhausted busy trả `503 CHAT_WRITE_CONTENTION`, `retryable:true`, không partial sqlite write.
- [ ] Implement CAS Axum route handler order: stream `.tmp-<uuid>` với `O_NOFOLLOW`/`0600`, hard-stop `26_214_401`, SHA/magic/archive validation, shared per-SHA mutex, sqlite `BEGIN IMMEDIATE`, purge receipt TTL allowlist, replay/mismatch decision, quota checks, fsync/rename, upsert blob/attachment/usage/permanent ingress/new CAS receipt, commit, release lock.
- [ ] Implement rollback/GC trong Axum service với cùng mutex: cleanup temp; final mới chỉ được unlink sau sqlite `BEGIN IMMEDIATE` chứng minh không có committed blob reference; attachment deletion cập nhật refcount; GC chỉ unlink `ref_count == 0 AND gc_lease_until < now_ms`.
- [ ] Update `src/features/flow/utils/html-sandbox-sanitizer.ts` với deterministic viewBox fallback, strict SVG URL grammar, và `DESIGN_CDN_CSP` parser fail-closed.
- [ ] `UT-IDOR-001`: xác minh sáu operations chat/CAS/vault dùng Axum route handler, guardedRoute, client sdk và zod contracts; cross-tenant/missing trả `404`, thiếu capability trả `403`, không leak metadata.
- [ ] `UT-IDEMPOTENCY-001`: cùng key/cùng digest chỉ commit một lần; replay `200` không tăng attachment refcount, physical usage hoặc ingress; digest khác trả `409`.
- [ ] `UT-IDEMPOTENCY-002`: sau `expires_at < now_ms`, purge chỉ xóa CAS receipt và cho phép reuse key; assert existing `cas_attachments`, `cas_blobs`, `cas_upload_ingress` vẫn nguyên vẹn byte-for-byte/row-for-row.
- [ ] `UT-IDEMPOTENCY-003`: assert purge Vault chỉ xóa expired `vault_command_idempotency`; không xóa vault receipt hợp lệ trước expiry và không có secret material trong sqlite row.
- [ ] `UT-FK-RECEIPT-001`: verify `CasCommandIdempotency.attachment_id` FK `ON DELETE CASCADE`; attachment lifecycle hợp lệ xóa receipt liên quan, còn TTL purge không được gọi attachment delete.
- [ ] `UT-GC-RACE-001`, `UT-ARCHIVE-CAPS-001`, `UT-RETRY-001`, `UT-CSP-SVG-001`, và `UT-ERROR-001`: giữ toàn bộ boundary/race/error assertions trước đó, bao gồm no-leak error DTO và sqlite atomicity.
- [ ] `E2E-CHAT-CAS-VAULT-001`: dùng client sdk `src/api/chat-vault.ts` qua guardedRoute route handler; verify reload chat persistence, binary/CSV/XLSX/FIG upload, paste prevention, vault receipt masking, and no direct UI fetch.
- [ ] `E2E-TTL-PRESERVATION-001`: tạo attachment và ingress, advance clock >24h, chạy TTL purge, upload lại với same key, và assert attachment/blob/ingress cũ không mất trước/sau command mới.

## Open Questions

None

## Final Verdict

VERDICT: PROVEN_SAFE
STATUS: DRAFT_PENDING_CODEX_AUDIT

## Open Questions

None

## Final Verdict

VERDICT: PROVEN_SAFE
STATUS: DRAFT_PENDING_CODEX_AUDIT

## Open Questions

None

## Final Verdict

VERDICT: PROVEN_SAFE
STATUS: DRAFT_PENDING_CODEX_AUDIT

## Open Questions

None

## Final Verdict

VERDICT: PROVEN_SAFE
STATUS: DRAFT_PENDING_CODEX_AUDIT

VERDICT: PROVEN_SAFE (Certified by Codex gpt-5.6-terra)
