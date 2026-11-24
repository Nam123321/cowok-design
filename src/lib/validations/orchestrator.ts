// @story 10.1
// @implements AC-01, AC-02, AC-03, AC-04 (Strict Zod Contracts for cowok 3-Lane Orchestrator, $UIUX_LIVE_SPEC Live Patch & CAS Provenance Backlinks)
import { z } from "zod";

const SHA256_HEX_RE = /^[a-fA-F0-9]{64}$/;
const SAFE_SESSION_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;
const MAX_MESSAGE_LEN = 16_384;
export const MAX_LIVE_SPEC_BYTES = 524_288;
const MAX_SCREEN_ID_LEN = 128;
const MAX_NODE_ID_LEN = 128;
const MAX_LIVE_SPEC_PATH_LEN = 512;

const utf8Encoder = new TextEncoder();

export function utf8ByteLength(value: string): number {
  return utf8Encoder.encode(value).byteLength;
}

const BoundedLiveSpecStringSchema = z.string().refine(
  (val) => utf8ByteLength(val) <= MAX_LIVE_SPEC_BYTES,
  {
    message: `LIVE_SPEC_PAYLOAD_TOO_LARGE: payload exceeds ${MAX_LIVE_SPEC_BYTES} UTF-8 bytes`,
  }
);

export const OrchestratorLaneSchema = z.enum([
  "LANE_0_DETERMINISTIC",
  "LANE_1_DECISION_CASCADE",
  "LANE_2_OPC_FLEET",
]);
export type OrchestratorLane = z.infer<typeof OrchestratorLaneSchema>;

export const OrchestratorCommandKindSchema = z.enum([
  "auto",
  "lint_fix",
  "token_update",
  "resize",
  "undo",
  "apply_live_spec",
]);
export type OrchestratorCommandKind = z.infer<typeof OrchestratorCommandKindSchema>;

export const OrchestratorChatRequestSchema = z
  .object({
    projectId: z.string().uuid(),
    sessionId: z
      .string()
      .regex(SAFE_SESSION_ID_RE, "sessionId must match ^[A-Za-z0-9._-]+$")
      .refine((s) => !s.includes(".."), "sessionId must not contain '..'"),
    nodeId: z.string().min(1).max(MAX_NODE_ID_LEN).optional(),
    message: z.string().trim().min(1).max(MAX_MESSAGE_LEN),
    commandKind: OrchestratorCommandKindSchema.default("auto"),
    liveSpecPayload: BoundedLiveSpecStringSchema.optional(),
  })
  .strict();
export type OrchestratorChatRequest = z.infer<typeof OrchestratorChatRequestSchema>;

export const ProvenanceBacklinkSchema = z
  .object({
    contentHash: z.string().regex(SHA256_HEX_RE, "Expected 64-char hex SHA-256"),
    sessionId: z.string().min(1),
    nodeId: z.string().nullable(),
    patchId: z.string().nullable(),
    mimeType: z.string().min(1),
    createdAt: z.number().int(),
  })
  .strict();
export type ProvenanceBacklink = z.infer<typeof ProvenanceBacklinkSchema>;

export const OrchestratorChatDataSchema = z
  .object({
    lane: OrchestratorLaneSchema,
    latencyMs: z.number().int().min(0),
    tokenCost: z.number().int().min(0),
    reply: z.string(),
    appliedPatchId: z.string().nullable(),
    canUndo: z.boolean(),
    liveSpecPath: z.string().nullable(),
    provenanceBacklinks: z.array(ProvenanceBacklinkSchema),
    actorQueueDepth: z.number().int().min(0),
  })
  .strict();
export type OrchestratorChatData = z.infer<typeof OrchestratorChatDataSchema>;

export const OrchestratorChatResponseSchema = z
  .object({
    ok: z.literal(true),
    data: OrchestratorChatDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type OrchestratorChatResponse = z.infer<typeof OrchestratorChatResponseSchema>;

export const OrchestratorLivePatchActionSchema = z.enum(["apply", "undo"]);
export type OrchestratorLivePatchAction = z.infer<typeof OrchestratorLivePatchActionSchema>;

export const OrchestratorLivePatchRequestSchema = z
  .object({
    projectId: z.string().uuid(),
    sessionId: z
      .string()
      .regex(SAFE_SESSION_ID_RE, "sessionId must match ^[A-Za-z0-9._-]+$")
      .refine((s) => !s.includes(".."), "sessionId must not contain '..'"),
    nodeId: z.string().min(1).max(MAX_NODE_ID_LEN).optional(),
    screenId: z.string().trim().min(1).max(MAX_SCREEN_ID_LEN).optional(),
    action: OrchestratorLivePatchActionSchema.default("apply"),
    liveSpecPayload: BoundedLiveSpecStringSchema.optional(),
    liveSpecFilePath: z.string().trim().min(1).max(MAX_LIVE_SPEC_PATH_LEN).optional(),
    baseHtml: BoundedLiveSpecStringSchema.optional(),
  })
  .strict();
export type OrchestratorLivePatchRequest = z.infer<typeof OrchestratorLivePatchRequestSchema>;

export const OrchestratorLivePatchDataSchema = z
  .object({
    patchId: z.string().min(1),
    projectId: z.string().min(1),
    screenId: z.string().min(1),
    astRevision: z.number().int().min(1),
    action: OrchestratorLivePatchActionSchema,
    canvasSha256: z.string().regex(SHA256_HEX_RE, "Expected 64-char hex SHA-256"),
    htmlSnapshot: z.string(),
    canUndo: z.boolean(),
    latencyMs: z.number().int().min(0),
    appliedLines: z.number().int().min(0),
    provenanceBacklinks: z.array(ProvenanceBacklinkSchema),
    actorQueueDepth: z.number().int().min(0),
  })
  .strict();
export type OrchestratorLivePatchData = z.infer<typeof OrchestratorLivePatchDataSchema>;

export const OrchestratorLivePatchResponseSchema = z
  .object({
    ok: z.literal(true),
    data: OrchestratorLivePatchDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type OrchestratorLivePatchResponse = z.infer<typeof OrchestratorLivePatchResponseSchema>;

export const OrchestratorAssetsQuerySchema = z
  .object({
    projectId: z.string().uuid(),
    sessionId: z
      .string()
      .regex(SAFE_SESSION_ID_RE, "sessionId must match ^[A-Za-z0-9._-]+$")
      .refine((s) => !s.includes(".."), "sessionId must not contain '..'")
      .optional(),
  })
  .strict();
export type OrchestratorAssetsQuery = z.infer<typeof OrchestratorAssetsQuerySchema>;

export const OrchestratorAssetsDataSchema = z
  .object({
    projectId: z.string().min(1),
    items: z.array(ProvenanceBacklinkSchema),
    totalCount: z.number().int().min(0),
  })
  .strict();
export type OrchestratorAssetsData = z.infer<typeof OrchestratorAssetsDataSchema>;

export const OrchestratorAssetsResponseSchema = z
  .object({
    ok: z.literal(true),
    data: OrchestratorAssetsDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type OrchestratorAssetsResponse = z.infer<typeof OrchestratorAssetsResponseSchema>;

export const OrchestratorErrorCodeSchema = z.enum([
  "ACTOR_BACKPRESSURE",
  "SYMLINK_VIOLATION",
  "PATH_TRAVERSAL",
  "LIVE_SPEC_NOT_FOUND",
  "LIVE_SPEC_PAYLOAD_TOO_LARGE",
  "INVALID_LIVE_SPEC_PATCH",
  "E_MXSS_BLOCKED",
  "NOTHING_TO_UNDO",
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "FORBIDDEN",
  "AUTH_FORBIDDEN",
  "PROJECT_NOT_FOUND",
  "PROJECT_TENANT_MISMATCH",
  "PAYLOAD_TOO_LARGE",
  "STORAGE_BUSY",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
]);
export type OrchestratorErrorCode = z.infer<typeof OrchestratorErrorCodeSchema>;

export const OrchestratorErrorResponseSchema = z
  .object({
    error: z
      .object({
        code: OrchestratorErrorCodeSchema,
        message: z.string().min(1),
        field: z.string().optional(),
        requestId: z.string().min(1),
        details: z.record(z.unknown()).optional(),
      })
      .strict(),
  })
  .strict();
export type OrchestratorErrorResponse = z.infer<typeof OrchestratorErrorResponseSchema>;
