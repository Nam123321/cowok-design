// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";
import { CONTEXT_LIMITS, type ContextApiError } from "./context-errors";
import {
  ContextAdapterModeSchema,
  ContextModelProfileSchema,
  ContextReasonCodeSchema,
  PortalIdSchema,
  TaskHorizonSchema,
} from "./context-types";

const SHA256_LOWER_HEX_RE = /^[a-f0-9]{64}$/;

export const DomOutlineNodeSchema = z
  .object({
    ffId: z.string().trim().min(1).max(CONTEXT_LIMITS.MAX_DOM_FF_ID_LEN),
    tag: z.string().trim().min(1).max(CONTEXT_LIMITS.MAX_DOM_TAG_LEN),
    role: z.string().trim().max(CONTEXT_LIMITS.MAX_DOM_ROLE_LEN).optional(),
    summary: z.string().trim().max(CONTEXT_LIMITS.MAX_DOM_SUMMARY_LEN).optional(),
  })
  .strict();
export type DomOutlineNode = z.infer<typeof DomOutlineNodeSchema>;

export const ContextAssembleRequestSchema = z
  .object({
    projectId: z.string().uuid(),
    portalId: PortalIdSchema,
    horizon: TaskHorizonSchema,
    modelProfile: ContextModelProfileSchema,
    intent: z
      .string()
      .transform((v) => v.normalize("NFC").trim())
      .refine((v) => [...v].length >= 1 && [...v].length <= CONTEXT_LIMITS.MAX_INTENT_CODE_POINTS, {
        message: "intent must be 1..2000 Unicode code points",
      }),
    requestedImageTokens: z.number().int().min(0).max(CONTEXT_LIMITS.MAX_REQUESTED_IMAGE_TOKENS),
    sourceIds: z
      .array(z.string().uuid())
      .min(1)
      .max(CONTEXT_LIMITS.MAX_SOURCE_IDS)
      .refine((ids) => new Set(ids).size === ids.length, {
        message: "sourceIds entries must be unique",
      })
      .optional(),
    domOutlineNodes: z
      .array(DomOutlineNodeSchema)
      .max(CONTEXT_LIMITS.MAX_DOM_OUTLINE_NODES)
      .optional(),
    inspectOnly: z.boolean().optional(),
  })
  .strict();
export type ContextAssembleRequest = z.infer<typeof ContextAssembleRequestSchema>;

const ContextAssembleDataSchema = z
  .object({
    telemetryId: z.string().uuid(),
    projectId: z.string().uuid(),
    portalId: PortalIdSchema,
    horizon: TaskHorizonSchema,
    modelProfile: ContextModelProfileSchema,
    adapterMode: ContextAdapterModeSchema,
    reasonCode: ContextReasonCodeSchema,
    staticPrefixSha256: z.string().regex(SHA256_LOWER_HEX_RE),
    tokenUsage: z
      .object({
        staticPrefix: z.number().int().min(0),
        dynamicTail: z.number().int().min(0),
        outline: z.number().int().min(0).max(CONTEXT_LIMITS.MAX_OUTLINE_BPE_TOKENS),
        textTotal: z.number().int().min(0).max(CONTEXT_LIMITS.MAX_TEXT_BPE_TOKENS),
        image: z.number().int().min(0),
        ssotDom: z.number().int().min(0),
        nonSsotText: z.number().int().min(0),
        ssotDomRatio: z.number().min(CONTEXT_LIMITS.MIN_SSOT_DOM_RATIO).max(1),
      })
      .strict(),
    sources: z
      .object({
        requested: z.number().int().min(0),
        included: z.number().int().min(0),
        pruned: z.number().int().min(0),
      })
      .strict(),
    latencyMs: z.number().int().min(0),
    assembledAt: z.string().datetime(),
  })
  .strict()
  .refine(
    (d) => d.tokenUsage.textTotal === d.tokenUsage.staticPrefix + d.tokenUsage.dynamicTail + d.tokenUsage.outline,
    { message: "textTotal invariant mismatch" }
  )
  .refine((d) => d.sources.requested === d.sources.included + d.sources.pruned, {
    message: "sources invariant mismatch",
  });
export type ContextAssembleData = z.infer<typeof ContextAssembleDataSchema>;

export const ContextAssembleSuccessSchema = z
  .object({
    ok: z.literal(true),
    data: ContextAssembleDataSchema,
    requestId: z.string().uuid(),
  })
  .strict();
export type ContextAssembleSuccess = z.infer<typeof ContextAssembleSuccessSchema>;

export const ContextTelemetryQuerySchema = z
  .object({
    projectId: z.string().uuid(),
    portalId: PortalIdSchema.optional(),
    horizon: TaskHorizonSchema.optional(),
    modelProfile: ContextModelProfileSchema.optional(),
    cursor: z.string().min(1).max(CONTEXT_LIMITS.MAX_CURSOR_LEN).optional(),
    limit: z
      .preprocess(
        (v) => (v === undefined ? CONTEXT_LIMITS.DEFAULT_TELEMETRY_LIMIT : Number(v)),
        z.number().int().min(1).max(CONTEXT_LIMITS.MAX_TELEMETRY_LIMIT)
      )
      .default(CONTEXT_LIMITS.DEFAULT_TELEMETRY_LIMIT),
  })
  .strict();
export type ContextTelemetryQuery = z.infer<typeof ContextTelemetryQuerySchema>;

const ContextTelemetryItemSchema = z
  .object({
    telemetryId: z.string().uuid(),
    projectId: z.string().uuid(),
    portalId: PortalIdSchema,
    horizon: TaskHorizonSchema,
    modelProfile: ContextModelProfileSchema,
    adapterMode: ContextAdapterModeSchema,
    reasonCode: ContextReasonCodeSchema,
    staticPrefixSha256: z.string().regex(SHA256_LOWER_HEX_RE),
    tokenUsage: z
      .object({
        outline: z.number().int().min(0).max(CONTEXT_LIMITS.MAX_OUTLINE_BPE_TOKENS),
        textTotal: z.number().int().min(0).max(CONTEXT_LIMITS.MAX_TEXT_BPE_TOKENS),
        image: z.number().int().min(0),
        ssotDomRatio: z.number().min(CONTEXT_LIMITS.MIN_SSOT_DOM_RATIO).max(1),
      })
      .strict(),
    latencyMs: z.number().int().min(0),
    status: z.literal("success"),
    createdAt: z.string().datetime(),
  })
  .strict();
export type ContextTelemetryItem = z.infer<typeof ContextTelemetryItemSchema>;

export const ContextTelemetryPageSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        items: z.array(ContextTelemetryItemSchema),
        page: z
          .object({
            limit: z.number().int().min(1).max(CONTEXT_LIMITS.MAX_TELEMETRY_LIMIT),
            nextCursor: z.string().nullable(),
            order: z.literal("created_at_desc_id_desc"),
          })
          .strict(),
      })
      .strict(),
    requestId: z.string().uuid(),
  })
  .strict();
export type ContextTelemetryPage = z.infer<typeof ContextTelemetryPageSchema>;

export type ContextAssembleOutcome = ContextAssembleSuccess | ContextApiError;
export type ContextTelemetryOutcome = ContextTelemetryPage | ContextApiError;
