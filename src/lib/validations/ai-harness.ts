// @story 08.7
// @implements AC-01, AC-03, AC-04 (Boundary Zod Contracts for UnifiedSkillCatalog, Dynamic Model Registry & Bridge 1 Studio Corrections)
import { z } from "zod";

const SHA256_HEX_RE = /^[0-9a-f]{64}$/;
const SIGNATURE_KEY_RE = /^[a-z0-9:_-]+$/;
const SAFE_ZERO_PII_TOKEN_RE = /^[a-z0-9:_-]+$/;

export const SkillSourceBucketSchema = z.enum([
  "design_craft",
  "host_skills",
  "upstream_verbatim",
  "sqlite_plugin",
  "learn_card",
]);
export type SkillSourceBucket = z.infer<typeof SkillSourceBucketSchema>;

export const UnifiedSkillEntrySchema = z.object({
  skillId: z.string().min(1).max(128),
  name: z.string().min(1).max(128),
  version: z.string().min(1).max(32),
  bucket: SkillSourceBucketSchema,
  description: z.string().min(1).max(600),
  tags: z.array(z.string().min(1).max(48)).max(24),
  referencePaths: z.array(z.string().min(1).max(256)).max(32),
  tier1TokenEstimate: z.number().int().min(1).max(100),
  contentSha256: z.string().regex(SHA256_HEX_RE),
});
export type UnifiedSkillEntry = z.infer<typeof UnifiedSkillEntrySchema>;

export const SkillCatalogResponseSchema = z.object({
  catalogSha256: z.string().regex(SHA256_HEX_RE),
  tier1IndexSha256: z.string().regex(SHA256_HEX_RE),
  totalSkills: z.number().int().min(0),
  bucketCounts: z.record(SkillSourceBucketSchema, z.number().int().min(0)),
  skills: z.array(UnifiedSkillEntrySchema),
  warnings: z.array(z.string()).default([]),
});
export type SkillCatalogResponse = z.infer<typeof SkillCatalogResponseSchema>;

export const SkillCatalogEnvelopeSchema = z.object({
  ok: z.literal(true),
  data: SkillCatalogResponseSchema,
  requestId: z.string().min(1),
});
export type SkillCatalogEnvelope = z.infer<typeof SkillCatalogEnvelopeSchema>;

export const ModelRegistryEntrySchema = z.object({
  id: z.string().min(1).max(128),
  providerId: z.string().min(1).max(32),
  modelId: z.string().min(1).max(128),
  displayName: z.string().min(1).max(128),
  tier: z.enum(["tier1_frontier", "tier2_fast", "tier3_local"]),
  contextWindow: z.number().int().min(4096).max(2_000_000),
  supportsPromptCache: z.boolean(),
  cacheStrategy: z.enum([
    "anthropic_ephemeral",
    "gemini_cached_content",
    "openai_cache_key",
    "none",
  ]),
  isDefaultForProvider: z.boolean(),
  enabled: z.boolean(),
});
export type ModelRegistryEntry = z.infer<typeof ModelRegistryEntrySchema>;

export const ModelsRegistryResponseSchema = z.object({
  models: z.array(ModelRegistryEntrySchema),
});
export type ModelsRegistryResponse = z.infer<typeof ModelsRegistryResponseSchema>;

export const ModelsRegistryEnvelopeSchema = z.object({
  ok: z.literal(true),
  data: ModelsRegistryResponseSchema,
  requestId: z.string().min(1),
});
export type ModelsRegistryEnvelope = z.infer<typeof ModelsRegistryEnvelopeSchema>;

export const StudioCorrectionKindSchema = z.enum([
  "chart_type_override",
  "qa_autofix_click",
  "palette_contrast_fix",
  "density_mode_override",
  "callout_placement_fix",
]);
export type StudioCorrectionKind = z.infer<typeof StudioCorrectionKindSchema>;

export const RecordStudioCorrectionRequestSchema = z
  .object({
    projectId: z.string().min(1).max(128),
    correctionKind: StudioCorrectionKindSchema,
    signatureKey: z.string().min(3).max(160).regex(SIGNATURE_KEY_RE),
    fromValue: z.string().min(1).max(64).regex(SAFE_ZERO_PII_TOKEN_RE).optional(),
    toValue: z.string().min(1).max(64).regex(SAFE_ZERO_PII_TOKEN_RE).optional(),
    skillId: z.string().min(1).max(128).regex(SAFE_ZERO_PII_TOKEN_RE).optional(),
  })
  .strict();
export type RecordStudioCorrectionRequest = z.infer<typeof RecordStudioCorrectionRequestSchema>;

export const RecordStudioCorrectionResponseSchema = z.object({
  signatureKey: z.string(),
  hitCount: z.number().int().min(0),
  escalatedToLedger: z.boolean(),
  ledgerEntryId: z.string().nullable(),
  optOutSkipped: z.boolean().default(false),
});
export type RecordStudioCorrectionResponse = z.infer<typeof RecordStudioCorrectionResponseSchema>;

export const RecordStudioCorrectionEnvelopeSchema = z.object({
  ok: z.literal(true),
  data: RecordStudioCorrectionResponseSchema,
  requestId: z.string().min(1),
});
export type RecordStudioCorrectionEnvelope = z.infer<typeof RecordStudioCorrectionEnvelopeSchema>;

export const AiHarnessErrorEnvelopeSchema = z.object({
  error: z.object({
    code: z.string().min(1),
    message: z.string().min(1),
    field: z.string().optional(),
    requestId: z.string().optional(),
  }),
});
export type AiHarnessErrorEnvelope = z.infer<typeof AiHarnessErrorEnvelopeSchema>;
