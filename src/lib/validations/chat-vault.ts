// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05 (Canonical Zod Contracts for Chat, CAS & Zero-Leak Vault)
import { z } from "zod";

const MAX_IDEMPOTENCY_KEY_LEN = 128;
const MAX_CHAT_PROSE_LEN = 200_000;
const MAX_ATTACHMENT_FILENAME_LEN = 255;
const MIN_SECRET_ENVELOPE_LEN = 8;
const MAX_SECRET_ENVELOPE_LEN = 512;

export const ApiErrorDtoSchema = z
  .object({
    code: z.string().min(1),
    message: z.string().min(1),
    retryable: z.boolean(),
    limit: z.string().min(1).optional(),
  })
  .strict();
export type ApiErrorDto = z.infer<typeof ApiErrorDtoSchema>;

export const IdempotencyKeySchema = z
  .string()
  .min(1)
  .max(MAX_IDEMPOTENCY_KEY_LEN)
  .regex(/^[\x21-\x7e]+$/, "Idempotency key must be visible ASCII without whitespace");

export const VaultReceiptDtoSchema = z.object({
  receiptId: z.string().min(1),
  provider: z.string().min(1),
  presenterMask: z.string().regex(/^.{2,6}\.\.\..{2,4}$/),
  status: z.enum(["VERIFIED", "RATE_LIMITED_429", "INVALID", "REVOKED"]),
  localOnly: z.boolean(),
  highEntropyWarning: z.boolean(),
});
export type VaultReceiptDto = z.infer<typeof VaultReceiptDtoSchema>;

export const ChatMessageDtoSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  projectId: z.string().min(1),
  role: z.enum(["user", "assistant", "system"]),
  proseMarkdown: z.string(),
  seqNo: z.number().int().nonnegative(),
  vaultReceipt: VaultReceiptDtoSchema.nullable().optional(),
  createdAt: z.number().int().nonnegative(),
});
export type ChatMessageDto = z.infer<typeof ChatMessageDtoSchema>;

export const ChatMessagesResponseSchema = z.object({
  sessionId: z.string().min(1),
  projectId: z.string().min(1),
  messages: z.array(ChatMessageDtoSchema),
});
export type ChatMessagesResponse = z.infer<typeof ChatMessagesResponseSchema>;

export const CreateChatMessageRequestSchema = z.object({
  role: z.enum(["user", "assistant", "system"]).default("user"),
  proseMarkdown: z.string().min(1).max(MAX_CHAT_PROSE_LEN),
  projectId: z.string().min(1).nullable().optional(),
  vaultReceipt: VaultReceiptDtoSchema.optional(),
});
export type CreateChatMessageRequest = z.infer<typeof CreateChatMessageRequestSchema>;

export const CasUploadMetadataSchema = z.object({
  sessionId: z.string().min(1),
  projectId: z.string().min(1).nullable().optional(),
  filename: z.string().min(1).max(MAX_ATTACHMENT_FILENAME_LEN),
  idempotencyKey: IdempotencyKeySchema.optional(),
});
export type CasUploadMetadata = z.infer<typeof CasUploadMetadataSchema>;

export const CasAttachmentDtoSchema = z.object({
  id: z.string().min(1),
  blobSha256: z.string().regex(/^[a-f0-9]{64}$/),
  sessionId: z.string().min(1),
  projectId: z.string().nullable().optional(),
  filename: z.string().min(1),
  fileExt: z.enum(["csv", "tsv", "xlsx", "fig", "pdf", "png", "svg", "txt"]),
  mimeType: z.string().min(1),
  byteSize: z.number().int().positive(),
  storageRelpath: z.string().min(1),
  createdAt: z.number().int().nonnegative(),
  replayed: z.boolean().default(false),
});
export type CasAttachmentDto = z.infer<typeof CasAttachmentDtoSchema>;

export const CasQuotaExceededErrorSchema = z.object({
  code: z.literal("CAS_QUOTA_EXCEEDED"),
  message: z.string().min(1),
  limit: z.enum([
    "REQUEST_BYTES",
    "TENANT_PHYSICAL_BYTES",
    "TENANT_INGRESS_24H",
    "SESSION_ATTACHMENT_COUNT",
  ]),
  retryable: z.literal(false),
});
export type CasQuotaExceededError = z.infer<typeof CasQuotaExceededErrorSchema>;

export const ChatVaultIngestRequestSchema = z.object({
  sessionId: z.string().min(1),
  projectId: z.string().min(1).nullable().optional(),
  provider: z.enum(["google_ai_studio", "typesafe_jev", "anthropic", "openai", "custom"]),
  rawSecretEnvelope: z.string().min(MIN_SECRET_ENVELOPE_LEN).max(MAX_SECRET_ENVELOPE_LEN),
  enterpriseLocalOnlyLock: z.boolean().default(false),
  idempotencyKey: IdempotencyKeySchema.optional(),
});
export type ChatVaultIngestRequest = z.infer<typeof ChatVaultIngestRequestSchema>;

export const ReplaceVaultReceiptRequestSchema = z.object({
  provider: z.enum(["google_ai_studio", "typesafe_jev", "anthropic", "openai", "custom"]).default("custom"),
  rawSecretEnvelope: z.string().min(MIN_SECRET_ENVELOPE_LEN).max(MAX_SECRET_ENVELOPE_LEN),
  enterpriseLocalOnlyLock: z.boolean().default(false),
});
export type ReplaceVaultReceiptRequest = z.infer<typeof ReplaceVaultReceiptRequestSchema>;

export const RevokeVaultReceiptResponseSchema = z.object({
  receiptId: z.string().min(1),
  status: z.literal("REVOKED"),
});
export type RevokeVaultReceiptResponse = z.infer<typeof RevokeVaultReceiptResponseSchema>;
