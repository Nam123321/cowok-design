---
type: I-Wish Data & Contract Spec
story_id: "04.6"
title: "Data, Persistence, CAS Intake & Zero-Leak Vault Specification: Story 04.6"
status: refactored
adr_ref: "../../../../2. Product Planning/ADRs/ADR-006-Decision-Cascade-Pooled-CDP-and-Shared-Html-Forge.md"
data_decision_records:
  - model: CasBlob
    action: NEW_AGGREGATE
    bounded_context: cas-storage
    justification: "V16 cas_blobs stores deduplicated physical binary/tabular uploads (.csv/.tsv/.xlsx/.fig/.pdf/.png/.svg/.txt) at .cowok-design/cas/<sha256> with gc_lease_until & ref_count."
  - model: CasAttachment
    action: CHILD_ENTITY
    parent_aggregate: CasBlob
    bounded_context: cas-storage
    justification: "V16 cas_attachments isolates per-tenant/session/project ownership & authorization over deduplicated physical cas_blobs."
  - model: TenantCasBlobUsage
    action: CHILD_ENTITY
    parent_aggregate: CasBlob
    bounded_context: cas-storage
    justification: "V16 tenant_cas_blob_usage tracks distinct physical SHA-256 bytes billed to each tenant against the 512 MiB physical cap."
  - model: CasUploadIngress
    action: NEW_AGGREGATE
    bounded_context: cas-storage
    justification: "V16 cas_upload_ingress records accepted UTC epoch ms byte ingress for rolling 24h 1 GiB/tenant quota enforcement."
  - model: CasCommandIdempotency
    action: CHILD_ENTITY
    parent_aggregate: CasBlob
    bounded_context: cas-storage
    justification: "V16 cas_command_idempotency stores 24h TTL X-Idempotency-Key replay records decoupled from permanent cas_attachments & cas_upload_ingress."
  - model: VaultCommandIdempotency
    action: NEW_AGGREGATE
    bounded_context: zero-leak-vault
    justification: "V16 vault_command_idempotency stores 24h TTL safe vault receipt replay metadata without plaintext or encrypted secret material."
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Security Vault Specification — Story 04.6

## 1. Bounded Context & Zero-Leak Vault Invariants

Story `04.6` eliminates `sessionStorage` chat drift by persisting messages directly into `V4 chat_messages` (with server-assigned monotonic `seq_no` inside `TransactionBehavior::Immediate` and auto-provisioned `__workbench_default__` project + `chat_sessions` FK), stores physical binary/tabular blobs deduplicated at `.cowok-design/cas/<sha256>` (`cas_blobs`) with tenant/session-scoped `cas_attachments`, `tenant_cas_blob_usage`, and `cas_upload_ingress` quota ledgers (`26,214,400` B/request, `536,870,912` B physical/tenant, `1,073,741,824` B rolling 24h ingress/tenant, `500` attachments/session), normalizes `<svg viewBox>` in `html-sandbox-sanitizer.ts`, and routes API keys submitted in chat through the **3-Layer Secret Interceptor** (`Zeroizing<String>`) + Rust daemon `redact_secrets` into OS Keychain / `0o600` Vault.

## 2. Additive SQLite WAL DDL (`V16` Chat Seq + Two-Table CAS & Quota Ledgers)

```sql
ALTER TABLE chat_messages ADD COLUMN seq_no BIGINT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_chat_messages_session_seq ON chat_messages(session_id, seq_no) WHERE seq_no IS NOT NULL;

CREATE TABLE IF NOT EXISTS cas_blobs (
    sha256 TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    mime_type TEXT NOT NULL,
    file_ext TEXT NOT NULL CHECK (file_ext IN ('csv','tsv','xlsx','fig','pdf','png','svg','txt')),
    storage_relpath TEXT NOT NULL,
    ref_count INTEGER NOT NULL DEFAULT 1 CHECK (ref_count >= 0),
    gc_lease_until BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cas_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    blob_sha256 TEXT NOT NULL REFERENCES cas_blobs(sha256) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS tenant_cas_blob_usage (
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    blob_sha256 TEXT NOT NULL REFERENCES cas_blobs(sha256) ON DELETE CASCADE,
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    first_attached_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, blob_sha256)
);

CREATE TABLE IF NOT EXISTS cas_upload_ingress (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    session_id TEXT NOT NULL,
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    accepted_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS cas_command_idempotency (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL,
    attachment_id TEXT NOT NULL REFERENCES cas_attachments(id) ON DELETE CASCADE,
    accepted_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    UNIQUE(tenant_id, session_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS vault_command_idempotency (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'local-default',
    session_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL,
    receipt_id TEXT NOT NULL,
    receipt_json TEXT NOT NULL,
    accepted_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    UNIQUE(tenant_id, session_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_cas_blobs_gc ON cas_blobs(gc_lease_until, ref_count);
CREATE INDEX IF NOT EXISTS idx_cas_attachments_tenant_session ON cas_attachments(tenant_id, session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_cas_attachments_blob ON cas_attachments(blob_sha256);
CREATE INDEX IF NOT EXISTS idx_cas_upload_ingress_window ON cas_upload_ingress(tenant_id, accepted_at);
CREATE INDEX IF NOT EXISTS idx_cas_cmd_idempotency_exp ON cas_command_idempotency(tenant_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_vault_cmd_idempotency_exp ON vault_command_idempotency(tenant_id, expires_at);
```

## 3. Prisma Schema Parity

```prisma
model CasBlob {
  sha256         String   @id
  tenantId       String   @default("local-default") @map("tenant_id")
  byteSize       BigInt   @map("byte_size")
  mimeType       String   @map("mime_type")
  fileExt        String   @map("file_ext")
  storageRelpath String   @map("storage_relpath")
  refCount       Int      @default(1) @map("ref_count")
  gcLeaseUntil   BigInt   @default(0) @map("gc_lease_until")
  createdAt      BigInt   @map("created_at")

  attachments    CasAttachment[]
  tenantUsages   TenantCasBlobUsage[]

  @@index([gcLeaseUntil, refCount], map: "idx_cas_blobs_gc")
  @@map("cas_blobs")
}

model CasAttachment {
  id         String  @id @default(uuid())
  blobSha256 String  @map("blob_sha256")
  tenantId   String  @default("local-default") @map("tenant_id")
  projectId  String? @map("project_id")
  sessionId  String  @map("session_id")
  filename   String
  createdAt  BigInt  @map("created_at")

  blob              CasBlob                @relation(fields: [blobSha256], references: [sha256], onDelete: Cascade)
  idempotencyClaims CasCommandIdempotency[]

  @@index([tenantId, sessionId, createdAt], map: "idx_cas_attachments_tenant_session")
  @@index([blobSha256], map: "idx_cas_attachments_blob")
  @@map("cas_attachments")
}

model TenantCasBlobUsage {
  tenantId        String @default("local-default") @map("tenant_id")
  blobSha256      String @map("blob_sha256")
  byteSize        BigInt @map("byte_size")
  firstAttachedAt BigInt @map("first_attached_at")

  blob CasBlob @relation(fields: [blobSha256], references: [sha256], onDelete: Cascade)

  @@id([tenantId, blobSha256])
  @@map("tenant_cas_blob_usage")
}

model CasUploadIngress {
  id         String @id @default(uuid())
  tenantId   String @default("local-default") @map("tenant_id")
  sessionId  String @map("session_id")
  byteSize   BigInt @map("byte_size")
  acceptedAt BigInt @map("accepted_at")

  @@index([tenantId, acceptedAt], map: "idx_cas_upload_ingress_window")
  @@map("cas_upload_ingress")
}

model CasCommandIdempotency {
  id             String @id @default(uuid())
  tenantId       String @default("local-default") @map("tenant_id")
  sessionId      String @map("session_id")
  idempotencyKey String @map("idempotency_key")
  payloadSha256  String @map("payload_sha256")
  attachmentId   String @map("attachment_id")
  acceptedAt     BigInt @map("accepted_at")
  expiresAt      BigInt @map("expires_at")

  attachment CasAttachment @relation(fields: [attachmentId], references: [id], onDelete: Cascade)

  @@unique([tenantId, sessionId, idempotencyKey])
  @@index([tenantId, expiresAt], map: "idx_cas_cmd_idempotency_exp")
  @@map("cas_command_idempotency")
}

model VaultCommandIdempotency {
  id             String @id @default(uuid())
  tenantId       String @default("local-default") @map("tenant_id")
  sessionId      String @map("session_id")
  idempotencyKey String @map("idempotency_key")
  payloadSha256  String @map("payload_sha256")
  receiptId      String @map("receipt_id")
  receiptJson    String @map("receipt_json")
  acceptedAt     BigInt @map("accepted_at")
  expiresAt      BigInt @map("expires_at")

  @@unique([tenantId, sessionId, idempotencyKey])
  @@index([tenantId, expiresAt], map: "idx_vault_cmd_idempotency_exp")
  @@map("vault_command_idempotency")
}
```

## 4. API Endpoints & Zod Contracts

- GET `/api/v1/chat-sessions/:sid/messages`: Returns persisted `V4` chat messages sorted by `(seq_no ASC, created_at ASC)` with `VaultReceiptCard` metadata (`presenterMask`, `status`, `localOnly`, `highEntropyWarning`).
- POST `/api/v1/chat-sessions/:sid/messages`: Auto-provisions `chat_sessions` FK (under `__workbench_default__` internal project if unattached), assigns monotonic `seq_no` inside `TransactionBehavior::Immediate`, runs `redact_secrets`, persists message in SQLite WAL.
- POST `/api/v1/cas/upload`: Streams multipart upload (`<= 26,214,400` B/request, magic-byte check, zip-bomb cap `<= 1024` entries / `<= 64 MiB` / `<= 20:1`, `512 MiB` physical/tenant, `1 GiB` 24h ingress, `500` attachments/session) -> `.cowok-design/cas/<sha256>` -> inserts `V16 cas_blobs` + `cas_attachments`.
- POST `/api/v1/vault/chat-ingest`: Intercepts raw Decision Engine (`google_ai_studio`, `typesafe_jev`) keys via `Zeroizing<String>` + `noul` probes -> verifies via POST `/api/v1/doctor/verify-key` -> writes Keychain/`0o600` Vault -> returns `VaultReceiptDto`.
- POST `/api/v1/vault/receipts/:receipt_id`: Replaces an existing vault secret cleanly by receipt ID without rehydrating old raw secret.
- DELETE `/api/v1/vault/receipts/:receipt_id`: Revokes a vault receipt and deletes its stored credential custody entry.

```typescript
import { z } from "zod";

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
  localOnly: z.boolean(),
  highEntropyWarning: z.boolean(),
});

export const CasQuotaExceededErrorSchema = z.object({
  code: z.literal("CAS_QUOTA_EXCEEDED"),
  limit: z.enum([
    "REQUEST_BYTES",
    "TENANT_PHYSICAL_BYTES",
    "TENANT_INGRESS_24H",
    "SESSION_ATTACHMENT_COUNT",
  ]),
  retryable: z.literal(false),
});
```
