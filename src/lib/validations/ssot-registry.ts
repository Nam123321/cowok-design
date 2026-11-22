// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";

const MAX_ID_LEN = 128;
const MAX_CURSOR_LEN = 256;
const MAX_LIMIT = 100;
const DEFAULT_LIMIT = 50;
const MAX_SLOT_NAME_LEN = 64;
const MAX_LOCKED_SLOTS = 32;
const MIN_LOCK_TTL_MS = 5000;
const MAX_LOCK_TTL_MS = 3600000;
const DEFAULT_LOCK_TTL_MS = 30000;
const MAX_TRANSFER_REASON_LEN = 256;
const MAX_EXPORT_FORMATS = 2;
const MAX_INTENT_LEN = 1000;
const MAX_CRAFT_SKILLS = 7;
const MAX_SPINE_TOKENS = 250;
const SHA256_HEX_LEN = 64;

export const SsotPortalIdSchema = z.enum([
  "SuperAdmin",
  "TenantAdmin",
  "SalesApp",
  "MarketingWeb",
  "DeveloperPortal",
  "Presentation",
]);

export type SsotPortalId = z.infer<typeof SsotPortalIdSchema>;

export const CraftSkillSlugSchema = z.enum([
  "archify-diagram",
  "dashboard-ux",
  "impeccable-polish",
  "oklch-palette",
  "sketch-monochrome",
  "stageshow-slide",
  "typography",
]);

export type CraftSkillSlug = z.infer<typeof CraftSkillSlugSchema>;

export const SsotComponentsQuerySchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    portalId: SsotPortalIdSchema,
    cursor: z.string().trim().max(MAX_CURSOR_LEN).optional(),
    limit: z.coerce.number().int().min(1).max(MAX_LIMIT).default(DEFAULT_LIMIT),
  })
  .strict();

export type SsotComponentsQuery = z.infer<typeof SsotComponentsQuerySchema>;

export const AppShellLockRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    portalId: SsotPortalIdSchema,
    action: z.enum(["ACQUIRE", "RENEW", "RELEASE", "TRANSFER"]),
    lockOwnerId: z.string().trim().min(1).max(MAX_ID_LEN).optional(),
    expectedRevision: z.number().int().min(0),
    expectedFencingToken: z.number().int().min(0).nullable().default(null),
    lockedSlots: z.array(z.string().trim().min(1).max(MAX_SLOT_NAME_LEN)).max(MAX_LOCKED_SLOTS).default([]),
    ttlMs: z.number().int().min(MIN_LOCK_TTL_MS).max(MAX_LOCK_TTL_MS).default(DEFAULT_LOCK_TTL_MS),
    forceTransferReason: z.string().trim().min(1).max(MAX_TRANSFER_REASON_LEN).optional(),
  })
  .strict();

export type AppShellLockRequest = z.infer<typeof AppShellLockRequestSchema>;

export const SsotExportRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    portalId: SsotPortalIdSchema,
    formats: z
      .array(z.enum(["markdown", "json"]))
      .min(1)
      .max(MAX_EXPORT_FORMATS)
      .default(["markdown", "json"]),
    includeQuarantined: z.boolean().default(false),
  })
  .strict();

export type SsotExportRequest = z.infer<typeof SsotExportRequestSchema>;

export const SkillRouteRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    intentSummary: z.string().trim().min(1).max(MAX_INTENT_LEN),
    portalId: SsotPortalIdSchema,
    requestedSlugs: z.array(CraftSkillSlugSchema).max(MAX_CRAFT_SKILLS).optional(),
    maxSpineTokens: z.number().int().min(1).max(MAX_SPINE_TOKENS).default(MAX_SPINE_TOKENS),
  })
  .strict();

export type SkillRouteRequest = z.infer<typeof SkillRouteRequestSchema>;

export const ComponentPatternDtoSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    portalId: SsotPortalIdSchema,
    patternSlug: z.string().min(1),
    componentName: z.string().min(1),
    sourcePath: z.string().nullable(),
    astSignatureSha256: z
      .string()
      .refine((v) => v.length === 0 || v.length === SHA256_HEX_LEN, {
        message: "astSignatureSha256 must be 64 hex chars or empty when quarantined before first sync",
      }),
    dedupMarkerSha256: z.string(),
    sourceRevision: z.number().int().min(0),
    lastSyncSource: z.enum(["oxc_reverse_sync", "manual", "export_sync"]),
    tokensSnapshotJson: z.string(),
    docMarkdown: z.string(),
    driftStatus: z.enum(["synced", "quarantined_drift"]),
    quarantinedDiffJson: z.string(),
    revision: z.number().int().min(1),
    createdAt: z.number().int().min(0),
    updatedAt: z.number().int().min(0),
  })
  .strict()
  .refine(
    (d) => d.astSignatureSha256.length === SHA256_HEX_LEN || d.driftStatus === "quarantined_drift",
    { message: "Synced component patterns must include a 64-char astSignatureSha256" }
  );

export type ComponentPatternDto = z.infer<typeof ComponentPatternDtoSchema>;

export const AppShellLockDtoSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    portalId: SsotPortalIdSchema,
    lockOwnerId: z.string(),
    fencingToken: z.number().int().min(1),
    lockedSlots: z.array(z.string()),
    expiresAt: z.number().int().min(0),
    isExpired: z.boolean(),
    revision: z.number().int().min(1),
    createdAt: z.number().int().min(0),
    updatedAt: z.number().int().min(0),
  })
  .strict();

export type AppShellLockDto = z.infer<typeof AppShellLockDtoSchema>;

export const AppShellLockConflictDetailsSchema = z
  .object({
    currentRevision: z.number().int().min(0),
    currentFencingToken: z.number().int().min(0),
    expiresAt: z.number().int().min(0),
    isExpired: z.boolean(),
  })
  .strict();

export type AppShellLockConflictDetails = z.infer<typeof AppShellLockConflictDetailsSchema>;

export const SsotComponentsResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        patterns: z.array(ComponentPatternDtoSchema),
        lock: AppShellLockDtoSchema.nullable(),
        nextCursor: z.string().nullable(),
        quarantinedCount: z.number().int().min(0),
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();

export type SsotComponentsResponse = z.infer<typeof SsotComponentsResponseSchema>;

export const AppShellLockMutationResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        lock: AppShellLockDtoSchema,
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();

export type AppShellLockMutationResponse = z.infer<typeof AppShellLockMutationResponseSchema>;

export const SsotExportResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        exportId: z.string().min(1),
        revision: z.number().int().min(1),
        portalId: SsotPortalIdSchema,
        generatedAt: z.number().int().min(0),
        markdownPath: z.string(),
        markdownSha256: z
          .string()
          .refine((v) => v.length === 0 || v.length === SHA256_HEX_LEN, {
            message: "markdownSha256 must be empty or 64 hex chars",
          }),
        jsonPath: z.string(),
        jsonSha256: z
          .string()
          .refine((v) => v.length === 0 || v.length === SHA256_HEX_LEN, {
            message: "jsonSha256 must be empty or 64 hex chars",
          }),
      })
      .strict()
      .refine((d) => d.markdownPath.length > 0 || d.jsonPath.length > 0, {
        message: "At least one export format artifact must be generated",
      }),
    requestId: z.string().min(1),
  })
  .strict();

export type SsotExportResponse = z.infer<typeof SsotExportResponseSchema>;

export const SkillRouteResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        portalId: SsotPortalIdSchema,
        spineTokens: z.number().int().min(1).max(MAX_SPINE_TOKENS),
        selectedSlugs: z.array(CraftSkillSlugSchema),
        prefixSha256: z.string().length(SHA256_HEX_LEN),
        zone1Spine: z.string().min(1),
        zone2CraftPrefix: z.string(),
        zone3IntentTail: z.string().min(1),
        latencyMs: z.number().min(0),
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();

export type SkillRouteResponse = z.infer<typeof SkillRouteResponseSchema>;

export const SsotApiErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "AUTH_FORBIDDEN",
  "CROSS_PORTAL_LEAK_BLOCKED",
  "PROJECT_TENANT_MISMATCH",
  "PROJECT_NOT_FOUND",
  "LOCK_FENCING_CONFLICT",
  "LOCK_TRANSFER_FORBIDDEN",
  "COMPONENT_PATTERN_CONFLICT",
  "COMPONENT_PATTERN_UNIQUE_CONFLICT",
  "PATH_JAIL_VIOLATION",
  "SPINE_TOKEN_BUDGET_EXCEEDED",
  "STORAGE_BUSY",
  "EXPORT_SUPERSEDED",
  "EXPORT_MANIFEST_INVALID",
  "STALE_MAPPING",
  "STALE_GENERATION_EPOCH",
  "E_MXSS_BLOCKED",
  "PAYLOAD_TOO_LARGE",
  "FORBIDDEN",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
]);

export type SsotApiErrorCode = z.infer<typeof SsotApiErrorCodeSchema>;

export const SsotApiErrorSchema = z
  .object({
    error: z
      .object({
        code: SsotApiErrorCodeSchema,
        message: z.string().min(1),
        field: z.string().optional(),
        requestId: z.string().min(1),
        details: z.union([AppShellLockConflictDetailsSchema, z.record(z.unknown())]).optional(),
      })
      .strict(),
  })
  .strict();

export type SsotApiError = z.infer<typeof SsotApiErrorSchema>;
