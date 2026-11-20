// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04 (Slide Copilot & Bridge Frame Zod Contracts)
import { z } from "zod";
import { CommentRegionRectSchema } from "./element-comment";
import {
  CHART_MAX_ARIA_LABEL_CHARS,
  CHART_MAX_SVG_PRIMITIVES,
  ChartKind2DSchema,
  ChartSpecV12Schema,
} from "./presentation-chart-specs";

export const SLIDE_COPILOT_LEASE_TTL_MS = 60_000;

const CANONICAL_SLIDE_UID_RE = /^SLD-[0-9A-HJKMNP-TV-Z]{26}$/;
const CANONICAL_FF_ID_RE = /^[a-zA-Z0-9_-]{1,64}$/;
const SHA256_HEX_RE = /^[0-9a-f]{64}$/;
const MIN_SECRET_LEN = 16;
const MAX_PROMPT_LEN = 4_000;
const MAX_SESSION_ID_LEN = 128;
const MAX_CANCEL_REASON_LEN = 240;

const LocalSlideUidSchema = z.string().regex(CANONICAL_SLIDE_UID_RE, {
  message: "Invalid canonical slideUid format (expected 30-char SLD- ULID)",
});

const LocalFfIdSchema = z.string().regex(CANONICAL_FF_ID_RE, {
  message: "Invalid canonical data-ff-id",
});

const SlideCopilotStructuredCommentSchema = z
  .object({
    commentId: z.string().min(1),
    projectId: z.string().optional(),
    nodeId: z.string().optional(),
    slideUid: z.string().optional(),
    targetType: z.enum(["html_element", "spatial_region"]).optional(),
    elementFfId: LocalFfIdSchema,
    coveredFfIds: z.array(z.string().min(1)).optional(),
    regionRect: CommentRegionRectSchema.optional(),
    domHash: z.string().default(""),
    artifactSha256: z.string().default(""),
    commentBody: z.string().min(1).max(MAX_PROMPT_LEN),
    targetLabel: z.string().optional(),
  })
  .strict();

export const SlideCopilotDispatchRequestSchema = z
  .object({
    sessionId: z.string().min(1).max(MAX_SESSION_ID_LEN),
    targetFfId: LocalFfIdSchema,
    prompt: z.string().min(1).max(MAX_PROMPT_LEN),
    expectedRevision: z.number().int().nonnegative(),
    preFlushedSha256: z.string().regex(SHA256_HEX_RE),
    snapshotReadOnly: z.literal(false).default(false),
    requestedChartKind: ChartKind2DSchema.optional(),
    structuredComment: SlideCopilotStructuredCommentSchema.optional(),
  })
  .strict();

export const SlideCopilotLeaseDtoSchema = z
  .object({
    jobId: z.string().min(1),
    slideUid: LocalSlideUidSchema,
    targetFfId: z.string().max(64).default(""),
    compositeScopeKey: z.string().min(1),
    status: z.enum(["QUEUED", "RUNNING", "COMPLETED", "CANCELLED", "FAILED", "INTERRUPTED"]),
    fencingToken: z.number().int().positive(),
    queuePosition: z.number().int().nonnegative(),
    leaseTtlMs: z.literal(SLIDE_COPILOT_LEASE_TTL_MS),
    leaseExpiresAtMs: z.number().int().nonnegative().nullable(),
    activeGlobalSlots: z.number().int().nonnegative(),
  })
  .strict();

export const SlideCopilotCancelRequestSchema = z
  .object({
    jobId: z.string().min(1),
    expectedFencingToken: z.number().int().positive(),
    reason: z.string().max(MAX_CANCEL_REASON_LEN).optional(),
  })
  .strict();

export const RenderChart2DFrameSchema = z
  .object({
    type: z.literal("RENDER_2D_CHART"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecret: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    slideUid: LocalSlideUidSchema,
    ffId: LocalFfIdSchema,
    spec: ChartSpecV12Schema,
    ariaLabel: z.string().max(CHART_MAX_ARIA_LABEL_CHARS),
  })
  .strict();

export const ChartRenderAckFrameSchema = z
  .object({
    type: z.literal("CHART_RENDER_ACK"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecretAck: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    slideUid: LocalSlideUidSchema,
    ffId: LocalFfIdSchema,
    svgPrimitiveCount: z.number().int().nonnegative().max(CHART_MAX_SVG_PRIMITIVES),
    renderTimeMs: z.number().nonnegative(),
    boundsRect: z
      .object({
        x: z.number().finite(),
        y: z.number().finite(),
        width: z.number().nonnegative(),
        height: z.number().nonnegative(),
      })
      .strict(),
  })
  .strict();

export const StageMatClickFrameSchema = z
  .object({
    type: z.literal("STAGE_MAT_CLICK"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecretAck: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    slideUid: LocalSlideUidSchema.optional(),
  })
  .strict();

export type SlideCopilotStructuredComment = z.infer<typeof SlideCopilotStructuredCommentSchema>;
export type SlideCopilotDispatchRequest = z.infer<typeof SlideCopilotDispatchRequestSchema>;
export type SlideCopilotLeaseDto = z.infer<typeof SlideCopilotLeaseDtoSchema>;
export type SlideCopilotCancelRequest = z.infer<typeof SlideCopilotCancelRequestSchema>;
export type RenderChart2DFrame = z.infer<typeof RenderChart2DFrameSchema>;
export type ChartRenderAckFrame = z.infer<typeof ChartRenderAckFrameSchema>;
export type StageMatClickFrame = z.infer<typeof StageMatClickFrameSchema>;
