---
epic_id: "Epic-04"
value_stream: "VS-02. Flow-First Executable Design Workbench & SDI Engine"
layer: api
status: active
cs: 5.0
endpoints_count: 5
---

# Layer-Epic-API: Epic 04: Flow-First 2D SVG Bezier DAG Studio, Security Kernel, CAS Intake & In-Chat Vault

## 1. API Architecture Overview & Protocols
- **Authoritative Runtime (`crates/harness-daemon`):** Axum + Tokio backend mounted on loopback with `legacy_project_auth_middleware` / `daemon_auth`, calling `crates/spec-registry` SQLite WAL and `crates/harness-daemon/src/vault.rs` (`0o600` Argon2id/AES-256-GCM + OS Keychain).
- **Zero-Leak Secret Invariant:** `POST /api/v1/chat-sessions/:sid/messages`, SSE broadcast (`sse_tx`), and daemon tracing run through `redact_secrets` before persistence or broadcast. `POST /api/v1/vault/chat-ingest` extracts raw Decision Engine API keys (`google_ai_studio`, `typesafe_jev`, etc.) directly into Vault custody and stores only masked receipt metadata (`VaultReceiptDto`).
- **CAS Intake Hardening:** `POST /api/v1/cas/upload` validates magic bytes, enforces `.xlsx`/`.fig` decompression caps against zip-bombs, neutralizes CSV/TSV formula prefixes (`=`, `+`, `-`, `@`), writes atomically via `O_NOFOLLOW` `.tmp -> .cowok-design/cas/<sha256>`, and guards concurrent writes with a dedupe lock on `CasBlob`.

## 2. Endpoint Inventory Table

| Method | URI | Auth / Guard | Bound Model(s) | Status | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `GET` | `/api/v1/chat-sessions/:sid/messages` | Authenticated Loopback Session | `ChatSession`, `ChatMessage` | New (`04.6`) | Fetch persisted SQLite `V4` chat messages sorted chronologically with `VaultReceiptCard` metadata. |
| `POST` | `/api/v1/chat-sessions/:sid/messages` | Authenticated Loopback Session | `ChatSession`, `ChatMessage` | New (`04.6`) | Auto-provision `ChatSession` FK if unattached, enforce server-side `redact_secrets`, persist `ChatMessage`. |
| `POST` | `/api/v1/cas/upload` | Authenticated Loopback Session | `CasBlob` | New (`04.6`) | Multipart binary/CSV intake (`.csv`, `.tsv`, `.xlsx`, `.fig`, `.pdf`, `.png`, `.svg`, `.txt`), atomic `O_NOFOLLOW` CAS write. |
| `POST` | `/api/v1/vault/chat-ingest` | Authenticated Loopback Session | `ChatSession`, `ChatMessage` | New (`04.6`) | 3-Layer Secret Interceptor + JEV Noul / `doctor/verify-key` verification -> `0o600` Vault -> returns `VaultReceiptDto`. |
| `POST` | `/api/v1/doctor/verify-key` | Authenticated Loopback Session | `AgentSetting` | Reused (`04.3`) | Upstream SSRF-safe live key probe consumed by in-chat vault ingestion. |
| POST | `/api/v1/vault/receipts/:receipt_id` | `Bearer (Member, Admin)` | `CasBlob` | Active | API contract declared in 04.6 |
| DELETE | `/api/v1/vault/receipts/:receipt_id` | `Bearer (Member, Admin)` | `CasBlob` | Active | API contract declared in 04.6 |

## 3. Request & Response Schemas (Zod Contracts)

```typescript
import { z } from "zod";

export const ChatMessageCreateRequestSchema = z.object({
  id: z.string().min(1).optional(),
  projectId: z.string().optional().nullable(),
  role: z.enum(["user", "assistant", "system"]),
  proseMarkdown: z.string().min(1).max(65536),
  parentMessageId: z.string().optional().nullable(),
  turnReceiptJson: z.string().optional().nullable(),
});

export const ChatVaultIngestRequestSchema = z.object({
  sessionId: z.string().min(1),
  projectId: z.string().optional().nullable(),
  provider: z.enum(["google_ai_studio", "typesafe_jev", "anthropic", "openai", "custom"]),
  rawSecretEnvelope: z.string().min(8).max(512),
  enterpriseLocalOnlyLock: z.boolean().default(false),
});

export const VaultReceiptDtoSchema = z.object({
  receiptId: z.string().min(1),
  provider: z.string().min(1),
  presenterMask: z.string().regex(/^.{2,6}\.\.\..{2,4}$/),
  status: z.enum(["VERIFIED", "RATE_LIMITED_429", "INVALID", "REVOKED"]),
  highEntropyWarning: z.boolean(),
});

export const CasBlobDtoSchema = z.object({
  sha256: z.string().length(64),
  sessionId: z.string().min(1),
  filename: z.string().min(1),
  mimeType: z.string().min(1),
  fileExt: z.enum(["csv", "tsv", "xlsx", "fig", "pdf", "png", "svg", "txt"]),
  byteSize: z.number().int().positive(),
  storageRelpath: z.string().min(1),
  createdAt: z.number().int(),
});
```

## 4. Error Handling & HTTP Status Codes

| Status Code | Error Code | Trigger Condition |
| :--- | :--- | :--- |
| `400 Bad Request` | `INVALID_CAS_FORMAT` / `CORRUPT_HEADER` | Unsupported extension, magic-byte mismatch, corrupt ZIP header, or invalid multipart payload (`CasBlob` stays clean). |
| `413 Payload Too Large` | `CAS_QUOTA_EXCEEDED` | File exceeds `25MB` upload limit or `.xlsx`/`.fig` exceeds zip-bomb expansion bound. |
| `429 Too Many Requests` | `PROVIDER_RATE_LIMITED` | Upstream key probe returns HTTP `429`; recorded on `VaultReceiptCard` with `429` status pill. |
| `422 Unprocessable Entity` | `ENTERPRISE_LOCAL_LOCK_ACTIVE` | External provider verification attempted while `enterpriseLocalOnlyLock` is enabled. |
| `500 Internal Server Error` | `VAULT_PERSISTENCE_FAILED` | Vault encryption or SQLite WAL transaction error after safe `.tmp` rollback. |
