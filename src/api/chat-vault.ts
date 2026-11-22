// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05 (Canonical Typed Client SDK for Chat, CAS Upload & Zero-Leak Vault)
import {
  CasAttachmentDtoSchema,
  ChatMessageDtoSchema,
  ChatMessagesResponseSchema,
  ChatVaultIngestRequestSchema,
  CreateChatMessageRequestSchema,
  IdempotencyKeySchema,
  ReplaceVaultReceiptRequestSchema,
  RevokeVaultReceiptResponseSchema,
  VaultReceiptDtoSchema,
  type CasAttachmentDto,
  type ChatMessageDto,
  type ChatMessagesResponse,
  type ChatVaultIngestRequest,
  type CreateChatMessageRequest,
  type ReplaceVaultReceiptRequest,
  type RevokeVaultReceiptResponse,
  type VaultReceiptDto,
} from "../lib/validations/chat-vault";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonApiError,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export { HarnessDaemonApiError as ChatVaultApiError };

const HEX_RADIX = 16;
const SLICE_START = 2;
const SLICE_END = 10;

function nextIdempotencyKey(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  return `idem-${Date.now()}-${Math.random().toString(HEX_RADIX).slice(SLICE_START, SLICE_END)}`;
}

export class ChatVaultClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  getMessages(sessionId: string): Promise<ChatMessagesResponse> {
    const sid = encodeURIComponent(sessionId);
    return this.request(`/api/v1/chat-sessions/${sid}/messages`, ChatMessagesResponseSchema, {
      method: "GET",
    });
  }

  appendMessage(
    sessionId: string,
    input: CreateChatMessageRequest,
    options?: { idempotencyKey?: string }
  ): Promise<ChatMessageDto> {
    const validated = CreateChatMessageRequestSchema.parse(input);
    const sid = encodeURIComponent(sessionId);
    const headers: Record<string, string> = {};
    if (options?.idempotencyKey) {
      headers["X-Idempotency-Key"] = IdempotencyKeySchema.parse(options.idempotencyKey);
    }
    return this.request(`/api/v1/chat-sessions/${sid}/messages`, ChatMessageDtoSchema, {
      method: "POST",
      headers,
      body: JSON.stringify(validated),
    });
  }

  uploadAttachment(params: {
    sessionId: string;
    file: File | Blob;
    filename?: string;
    projectId?: string | null;
    idempotencyKey?: string;
  }): Promise<CasAttachmentDto> {
    const key = IdempotencyKeySchema.parse(params.idempotencyKey || nextIdempotencyKey());
    const form = new FormData();
    form.append("sessionId", params.sessionId);
    if (params.projectId) form.append("projectId", params.projectId);
    form.append("idempotencyKey", key);
    const resolvedName =
      params.filename || (params.file instanceof File ? params.file.name : "attachment.txt");
    form.append("file", params.file, resolvedName);
    return this.request("/api/v1/cas/upload", CasAttachmentDtoSchema, {
      method: "POST",
      headers: { "X-Idempotency-Key": key },
      body: form,
    });
  }

  ingestVaultSecret(
    input: ChatVaultIngestRequest,
    options?: { idempotencyKey?: string }
  ): Promise<VaultReceiptDto> {
    const validated = ChatVaultIngestRequestSchema.parse(input);
    const key = IdempotencyKeySchema.parse(
      options?.idempotencyKey || validated.idempotencyKey || nextIdempotencyKey()
    );
    return this.request("/api/v1/vault/chat-ingest", VaultReceiptDtoSchema, {
      method: "POST",
      headers: { "X-Idempotency-Key": key },
      body: JSON.stringify({ ...validated, idempotencyKey: key }),
    });
  }

  replaceVaultReceipt(
    receiptId: string,
    input: ReplaceVaultReceiptRequest
  ): Promise<VaultReceiptDto> {
    const validated = ReplaceVaultReceiptRequestSchema.parse(input);
    const rid = encodeURIComponent(receiptId);
    return this.request(`/api/v1/vault/receipts/${rid}`, VaultReceiptDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  revokeVaultReceipt(receiptId: string): Promise<RevokeVaultReceiptResponse> {
    const rid = encodeURIComponent(receiptId);
    return this.request(`/api/v1/vault/receipts/${rid}`, RevokeVaultReceiptResponseSchema, {
      method: "DELETE",
    });
  }
}

export const apiChatVault = new ChatVaultClient();
export default apiChatVault;
