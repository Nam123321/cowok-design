// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (Zod Contracts for Two-Store /rea Bridge, Firewall & Honest Proof Exporter)
import { z } from "zod";

export const REA_TIER1A_MAX_TOKENS = 250;
const MAX_SLUG_LEN = 96;
const MAX_URL_LEN = 1_024;
const MAX_LICENSE_LEN = 64;
const MAX_SUMMARY_HINT_LEN = 8_000;
const MAX_PAYLOAD_TEXT_LEN = 524_288;
const MAX_PAYLOAD_B64_LEN = 1_800_000;
const MAX_SEARCH_QUERY_LEN = 256;
const MAX_SEARCH_LIMIT = 100;
const DEFAULT_SEARCH_LIMIT = 20;
const SHA256_HEX_LEN = 64;

export const ReaSourceFormatSchema = z.enum([
  "url",
  "github",
  "fig",
  "clipboard",
  "zip",
  "pen",
  "wire",
]);
export type ReaSourceFormat = z.infer<typeof ReaSourceFormatSchema>;

export const ReaFirewallVerdictSchema = z.enum([
  "ADOPT_PRIOR_ART",
  "REFERENCE_ONLY",
  "REJECT",
]);
export type ReaFirewallVerdict = z.infer<typeof ReaFirewallVerdictSchema>;

export const ReaProofStatusSchema = z.enum([
  "awaiting-verification",
  "proof-verified",
]);
export type ReaProofStatus = z.infer<typeof ReaProofStatusSchema>;

export const ReaSearchTierSchema = z.enum(["1a", "1b"]);
export type ReaSearchTier = z.infer<typeof ReaSearchTierSchema>;

export const ReaIngestRequestSchema = z
  .object({
    slug: z
      .string()
      .trim()
      .min(1)
      .max(MAX_SLUG_LEN)
      .regex(/^[A-Za-z0-9][A-Za-z0-9._-]{0,95}$/, "Invalid reference slug")
      .refine((v) => !v.includes(".."), "Slug cannot contain '..'"),
    sourceUrl: z.string().trim().min(1).max(MAX_URL_LEN),
    sourceFormat: ReaSourceFormatSchema,
    licenseSpdx: z.string().trim().max(MAX_LICENSE_LEN).default("UNKNOWN_PUBLIC"),
    summaryHint: z.string().max(MAX_SUMMARY_HINT_LEN).optional(),
    payloadText: z.string().max(MAX_PAYLOAD_TEXT_LEN).optional(),
    payloadBase64: z.string().max(MAX_PAYLOAD_B64_LEN).optional(),
  })
  .strict();

export type ReaIngestRequest = z.infer<typeof ReaIngestRequestSchema>;

export const ReaFirewallDecisionSchema = z.object({
  verdict: ReaFirewallVerdictSchema,
  normalizedLicense: z.string().min(1),
  reason: z.string(),
  policyRule: z.string().min(1),
});
export type ReaFirewallDecisionDto = z.infer<typeof ReaFirewallDecisionSchema>;

export const ReaReferenceRecordSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string().min(1),
  slug: z.string().min(1),
  sourceUrl: z.string().min(1),
  sourceFormat: ReaSourceFormatSchema,
  licenseSpdx: z.string().min(1),
  firewallVerdict: ReaFirewallVerdictSchema,
  firewallReason: z.string(),
  tier1aSummary: z.string(),
  tier1aTokens: z.number().int().min(0).max(REA_TIER1A_MAX_TOKENS),
  designDnaJson: z.string(),
  rawFieldsJson: z.string().nullable().optional(),
  proofStatus: ReaProofStatusSchema,
  sanitizedSecretsCount: z.number().int().min(0),
  createdAt: z.number().int().min(0),
});
export type ReaReferenceRecordDto = z.infer<typeof ReaReferenceRecordSchema>;

export const ReaIngestOutcomeSchema = z.object({
  record: ReaReferenceRecordSchema,
  firewall: ReaFirewallDecisionSchema,
  strippedCategories: z.array(z.string()),
  extractedFiles: z.array(z.string()),
});
export type ReaIngestOutcomeDto = z.infer<typeof ReaIngestOutcomeSchema>;

export const ReaIngestResponseSchema = z
  .object({
    ok: z.literal(true),
    data: ReaIngestOutcomeSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type ReaIngestResponse = z.infer<typeof ReaIngestResponseSchema>;

export const ReaSearchQuerySchema = z
  .object({
    q: z.string().max(MAX_SEARCH_QUERY_LEN).default(""),
    limit: z.coerce.number().int().min(1).max(MAX_SEARCH_LIMIT).default(DEFAULT_SEARCH_LIMIT),
    tier: ReaSearchTierSchema.default("1a"),
  })
  .strict();
export type ReaSearchQuery = z.infer<typeof ReaSearchQuerySchema>;

export const ReaSearchHitSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string().min(1),
  slug: z.string().min(1),
  sourceUrl: z.string().min(1),
  sourceFormat: ReaSourceFormatSchema,
  licenseSpdx: z.string().min(1),
  firewallVerdict: ReaFirewallVerdictSchema,
  firewallReason: z.string(),
  tier1aSummary: z.string(),
  tier1aTokens: z.number().int().min(0).max(REA_TIER1A_MAX_TOKENS),
  proofStatus: ReaProofStatusSchema,
  sanitizedSecretsCount: z.number().int().min(0),
  createdAt: z.number().int().min(0),
  designDna: z.unknown().optional(),
  rawFields: z.unknown().optional(),
  bm25Rank: z.number().nullable().optional(),
});
export type ReaSearchHitDto = z.infer<typeof ReaSearchHitSchema>;

export const ReaSearchDataSchema = z.object({
  tier: ReaSearchTierSchema,
  query: z.string(),
  sanitizedFtsQuery: z.string(),
  maxTier1aTokens: z.number().int().min(1).max(REA_TIER1A_MAX_TOKENS),
  total: z.number().int().min(0),
  items: z.array(ReaSearchHitSchema),
});
export type ReaSearchDataDto = z.infer<typeof ReaSearchDataSchema>;

export const ReaSearchResponseSchema = z
  .object({
    ok: z.literal(true),
    data: ReaSearchDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type ReaSearchResponse = z.infer<typeof ReaSearchResponseSchema>;

export const ReaExportProofRequestSchema = z
  .object({
    referenceId: z.string().trim().min(1).max(MAX_SLUG_LEN),
    roundtripChecksum: z
      .string()
      .trim()
      .regex(/^[0-9a-fA-F]{64}$/, "roundtripChecksum must be 64-char SHA-256 hex")
      .optional(),
  })
  .strict();
export type ReaExportProofRequest = z.infer<typeof ReaExportProofRequestSchema>;

export const ReaExportProofDataSchema = z.object({
  referenceId: z.string().min(1),
  slug: z.string().min(1),
  proofStatus: ReaProofStatusSchema,
  verified: z.boolean(),
  checksumMatched: z.boolean(),
  computedSha256: z.string().length(SHA256_HEX_LEN),
  proofPayloadJson: z.string(),
  verificationNote: z.string(),
});
export type ReaExportProofDataDto = z.infer<typeof ReaExportProofDataSchema>;

export const ReaExportProofResponseSchema = z
  .object({
    ok: z.literal(true),
    data: ReaExportProofDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type ReaExportProofResponse = z.infer<typeof ReaExportProofResponseSchema>;

const DEFAULT_CATALOG_LIMIT = 5;
const MAX_CATALOG_LIMIT = 20;

export const ReaCatalogQuerySchema = z
  .object({
    q: z.string().max(MAX_SEARCH_QUERY_LEN).default(""),
    limit: z.coerce.number().int().min(1).max(MAX_CATALOG_LIMIT).default(DEFAULT_CATALOG_LIMIT),
  })
  .strict();
export type ReaCatalogQuery = z.infer<typeof ReaCatalogQuerySchema>;

export const ReaCatalogItemSchema = z.object({
  slug: z.string().min(1),
  name: z.string().min(1),
  repoUrl: z.string().url(),
  category: z.string().min(1),
  subcategory: z.string(),
  stars: z.number().int().min(0),
  licenseSpdx: z.string().min(1),
  firewallVerdict: ReaFirewallVerdictSchema,
  lang: z.string(),
  description: z.string(),
  topics: z.array(z.string()),
  layoutArchetype: z.string().min(1),
});
export type ReaCatalogItemDto = z.infer<typeof ReaCatalogItemSchema>;

export const ReaCatalogDataSchema = z.object({
  upstreamRepo: z.string().min(1),
  upstreamCommit: z.string().min(1),
  totalCatalogEntries: z.number().int().min(0),
  query: z.string(),
  sanitizedFtsQuery: z.string(),
  total: z.number().int().min(0),
  items: z.array(ReaCatalogItemSchema),
});
export type ReaCatalogDataDto = z.infer<typeof ReaCatalogDataSchema>;

export const ReaCatalogResponseSchema = z
  .object({
    ok: z.literal(true),
    data: ReaCatalogDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type ReaCatalogResponse = z.infer<typeof ReaCatalogResponseSchema>;

export const ReaErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "INVALID_PAYLOAD",
  "UNAUTHORIZED",
  "AUTH_FORBIDDEN",
  "SSRF_URL_BLOCKED",
  "FIREWALL_REJECTED",
  "ZIP_BOMB_REJECTED",
  "PATH_JAIL_VIOLATION",
  "REFERENCE_NOT_FOUND",
  "PAYLOAD_TOO_LARGE",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
]);
export type ReaErrorCode = z.infer<typeof ReaErrorCodeSchema>;

export const ReaErrorEnvelopeSchema = z
  .object({
    error: z.object({
      code: ReaErrorCodeSchema,
      message: z.string().min(1),
      field: z.string().optional(),
      requestId: z.string().min(1),
      details: z.record(z.unknown()).optional(),
    }),
  })
  .strict();
export type ReaErrorEnvelope = z.infer<typeof ReaErrorEnvelopeSchema>;
