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
    justification: "V16 cas_blobs tracks content-addressed uploads (.csv/.tsv/.xlsx/.fig/.pdf) at .cowok-design/cas/<sha256>"
  - model: ChatMessage
    action: EXTENDS_AGGREGATE
    bounded_context: chat-persistence
    justification: "Story 04.6 binds V4 chat_messages to SQLite persistence with auto-provisioned session FK and zero raw secret storage"
links_to:
  - "../../../../2. Product Planning/2.2. database-spec.md"
  - "../../../../2. Product Planning/2.5. architecture.md"
  - "../../../../2. Product Planning/2.12. api-registry.md"
  - "./story.md"
---

# Data & Security Vault Specification — Story 04.6

## 1. Bounded Context & Zero-Leak Vault Invariants

Story `04.6` eliminates `sessionStorage` chat drift by persisting messages directly into `V4 chat_messages`, stores binary/tabular attachments in `.cowok-design/cas/<sha256>` indexed by `V16 cas_blobs`, normalizes `<svg viewBox>` in `html-sandbox-sanitizer.ts`, and routes API keys submitted in chat through the **3-Layer Secret Interceptor** + Rust daemon `redact_secrets` into OS Keychain / `0o600` Vault.

## 2. Additive SQLite WAL DDL (`V16 cas_blobs`)

```sql
CREATE TABLE IF NOT EXISTS cas_blobs (
    sha256 TEXT PRIMARY KEY NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    session_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    file_ext TEXT NOT NULL CHECK (file_ext IN ('csv','tsv','xlsx','fig','pdf','png','svg','txt')),
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    storage_relpath TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cas_blobs_session ON cas_blobs(session_id, created_at);
```

## 3. Prisma Schema Parity

```prisma
model CasBlob {
  sha256         String  @id
  projectId      String? @map("project_id")
  sessionId      String  @map("session_id")
  filename       String
  mimeType       String  @map("mime_type")
  fileExt        String  @map("file_ext")
  byteSize       BigInt  @map("byte_size")
  storageRelpath String  @map("storage_relpath")
  createdAt      BigInt  @map("created_at")

  @@index([sessionId, createdAt])
  @@map("cas_blobs")
}
```

## 4. API Endpoints & Zod Contracts

- GET `/api/v1/chat-sessions/:sid/messages`: Returns persisted `V4` chat messages sorted by `seq_no` with `VaultReceiptCard` metadata (`masked_prefix`, `key_status`).
- POST `/api/v1/chat-sessions/:sid/messages`: Auto-provisions `chat_sessions` FK if unattached to a project, runs `redact_secrets`, persists message in SQLite WAL.
- POST `/api/v1/cas/upload`: Streams multipart upload to `.cowok-design/cas/<sha256>` with extension/size validation and inserts `V16 cas_blobs`.
- POST `/api/v1/vault/chat-ingest`: Intercepts raw Decision Engine (`google_ai_studio`, `typesafe_jev`) keys via `noul` probes + 3-Layer Secret Interceptor -> verifies via POST `/api/v1/doctor/verify-key` -> writes Keychain/`0o600` Vault without persisting plaintext to SQLite or SSE logs.

```typescript
import { z } from "zod";

export const ChatVaultIngestRequestSchema = z.object({
  sessionId: z.string().min(1),
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
```
