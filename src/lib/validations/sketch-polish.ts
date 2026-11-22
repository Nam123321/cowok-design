// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";

const MAX_PROJECT_ID_LEN = 128;
const MAX_SCREEN_ID_LEN = 128;
const MAX_GOAL_PROMPT_LEN = 4_000;
const MAX_FF_ID_LEN = 64;
const MAX_DIRECTION_ID_LEN = 64;
const MAX_DIRECTION_LABEL_LEN = 160;
const MAX_DIRECTION_SUMMARY_LEN = 600;
const MAX_I32_POSITIVE = 2_147_483_647;
const MIN_LIVE_DIRECTIONS = 2;
const SHA256_HEX_LEN = 64;
const MAX_HTML_SNAPSHOT_CHARS = 524_288;

export const SSOT_PORTAL_ENUM = z.enum([
  "SuperAdmin",
  "TenantAdmin",
  "SalesApp",
  "MarketingWeb",
  "DeveloperPortal",
  "Presentation",
]);

export const ClarifyLocaleSchema = z.enum(["vi", "en"]);

export const LiveDirectionOptionSchema = z.object({
  id: z.string().min(1).max(MAX_DIRECTION_ID_LEN),
  label: z.string().min(1).max(MAX_DIRECTION_LABEL_LEN),
  summary: z.string().min(1).max(MAX_DIRECTION_SUMMARY_LEN),
  accentHex: z.string().regex(/^#[0-9A-Fa-f]{6}$/),
  surfaceHex: z.string().regex(/^#[0-9A-Fa-f]{6}$/),
  skillSlug: z.string().min(1).max(MAX_DIRECTION_ID_LEN),
});

export type LiveDirectionOption = z.infer<typeof LiveDirectionOptionSchema>;

export const ClarifyDirectionRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_PROJECT_ID_LEN),
    goalPrompt: z.string().trim().min(1).max(MAX_GOAL_PROMPT_LEN),
    locale: ClarifyLocaleSchema,
    activeDirectionId: z.string().trim().min(1).max(MAX_DIRECTION_ID_LEN).optional(),
    preemptPgid: z.number().int().positive().max(MAX_I32_POSITIVE).optional(),
    expectedGenerationEpoch: z.number().int().positive().optional(),
    portalId: SSOT_PORTAL_ENUM.optional(),
  })
  .strict();

export type ClarifyDirectionRequest = z.infer<typeof ClarifyDirectionRequestSchema>;

export const ClarifySessionDataSchema = z.object({
  sessionId: z.string().min(1),
  projectId: z.string().min(1),
  status: z.enum(["clarifying", "locked", "preempted"]),
  goalPrompt: z.string().min(1),
  locale: ClarifyLocaleSchema,
  activeDirectionId: z.string().min(1),
  directions: z.array(LiveDirectionOptionSchema).min(MIN_LIVE_DIRECTIONS),
  generationEpoch: z.number().int().positive(),
  preemptedWorkers: z.number().int().nonnegative(),
  routedSkills: z.array(z.string()),
  prefixSha256: z.string(),
  updatedAt: z.number().int(),
});

export type ClarifySessionData = z.infer<typeof ClarifySessionDataSchema>;

export const ClarifyDirectionResponseSchema = z.object({
  ok: z.literal(true),
  data: ClarifySessionDataSchema,
  requestId: z.string().min(1),
});

export type ClarifyDirectionResponse = z.infer<typeof ClarifyDirectionResponseSchema>;

export const GetClarifySessionResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    session: ClarifySessionDataSchema.nullable(),
  }),
  requestId: z.string().min(1),
});

export type GetClarifySessionResponse = z.infer<typeof GetClarifySessionResponseSchema>;

export const PatchOpKindSchema = z.enum([
  "TEXT_EDIT",
  "TOKEN_SWAP",
  "STEERING_CHIP",
  "LOCKED_RULE_CHANGE",
  "UNDO",
]);

export type PatchOpKind = z.infer<typeof PatchOpKindSchema>;

export const ApplyTierSchema = z.enum(["INSTANT_UNDO", "LOCKED_RULE_RECEIPT"]);

export type ApplyTier = z.infer<typeof ApplyTierSchema>;

export const OklchContrastReportSchema = z.object({
  originalFgHex: z.string(),
  bgHex: z.string(),
  adjustedFgHex: z.string(),
  originalRatio: z.number(),
  adjustedRatio: z.number(),
  minRatio: z.number(),
  clamped: z.boolean(),
  warning: z.string().nullable(),
});

export type OklchContrastReport = z.infer<typeof OklchContrastReportSchema>;

export const LockedRuleChangeReceiptSchema = z.object({
  ruleCode: z.string().min(1),
  lockOwnerId: z.string().min(1),
  fencingToken: z.number().int().nonnegative(),
  affectedFfId: z.string().min(1),
  summary: z.string().min(1),
  requiresReview: z.boolean(),
  applied: z.boolean(),
});

export type LockedRuleChangeReceipt = z.infer<typeof LockedRuleChangeReceiptSchema>;

export const SurgicalPatchRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_PROJECT_ID_LEN),
    screenId: z.string().trim().min(1).max(MAX_SCREEN_ID_LEN),
    baseArtifactSha256: z.string().regex(/^[0-9a-fA-F]{64}$/),
    ffId: z.string().regex(/^[a-zA-Z0-9_-]{1,64}$/).max(MAX_FF_ID_LEN),
    opKind: PatchOpKindSchema,
    payloadJson: z.record(z.unknown()),
    expectedAstRevision: z.number().int().nonnegative().optional(),
    baseHtml: z.string().max(MAX_HTML_SNAPSHOT_CHARS).optional(),
    nextHtml: z.string().max(MAX_HTML_SNAPSHOT_CHARS).optional(),
    portalId: SSOT_PORTAL_ENUM.optional(),
    confirmLockedRuleOverride: z.boolean().optional(),
  })
  .strict();

export type SurgicalPatchRequest = z.infer<typeof SurgicalPatchRequestSchema>;

export const SurgicalPatchResultDataSchema = z.object({
  patchId: z.string().min(1),
  projectId: z.string().min(1),
  screenId: z.string().min(1),
  astRevision: z.number().int().positive(),
  baseArtifactSha256: z.string().length(SHA256_HEX_LEN),
  resultArtifactSha256: z.string().length(SHA256_HEX_LEN),
  ffId: z.string().min(1),
  opKind: PatchOpKindSchema,
  applyTier: ApplyTierSchema,
  reconciledFfIds: z.number().int().nonnegative(),
  htmlSnapshot: z.string(),
  oklchGuard: OklchContrastReportSchema.nullable(),
  lockedRuleReceipt: LockedRuleChangeReceiptSchema.nullable(),
  undoAvailable: z.boolean(),
  executionTimeMicros: z.number().int().nonnegative(),
  createdAt: z.number().int(),
});

export type SurgicalPatchResultData = z.infer<typeof SurgicalPatchResultDataSchema>;

export const SurgicalPatchResponseSchema = z.object({
  ok: z.literal(true),
  data: SurgicalPatchResultDataSchema,
  requestId: z.string().min(1),
});

export type SurgicalPatchResponse = z.infer<typeof SurgicalPatchResponseSchema>;

export const ScreenPatchRecordSchema = z.object({
  patchId: z.string().min(1),
  astRevision: z.number().int().positive(),
  baseArtifactSha256: z.string().length(SHA256_HEX_LEN),
  resultArtifactSha256: z.string().length(SHA256_HEX_LEN),
  ffId: z.string().min(1),
  opKind: PatchOpKindSchema,
  applyTier: ApplyTierSchema,
  reconciledFfIds: z.number().int().nonnegative(),
  payloadJson: z.record(z.unknown()),
  lockedRuleReceipt: LockedRuleChangeReceiptSchema.nullable(),
  createdAt: z.number().int(),
});

export type ScreenPatchRecord = z.infer<typeof ScreenPatchRecordSchema>;

export const ScreenPatchHistoryDataSchema = z.object({
  projectId: z.string().min(1),
  screenId: z.string().min(1),
  astRevision: z.number().int().nonnegative(),
  currentArtifactSha256: z.string().length(SHA256_HEX_LEN),
  htmlSnapshot: z.string(),
  patches: z.array(ScreenPatchRecordSchema),
});

export type ScreenPatchHistoryData = z.infer<typeof ScreenPatchHistoryDataSchema>;

export const ScreenPatchHistoryResponseSchema = z.object({
  ok: z.literal(true),
  data: ScreenPatchHistoryDataSchema,
  requestId: z.string().min(1),
});

export type ScreenPatchHistoryResponse = z.infer<typeof ScreenPatchHistoryResponseSchema>;
