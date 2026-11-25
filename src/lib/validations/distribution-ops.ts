// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Strict Zod Schemas for 4-Channel Distribution Ops, Plugin Catalog, A/B Experiments, Global Quarantine, Offline Licenses & Audit Logs)
import { z } from "zod";

const MAX_SHORT_ID = 128;
const MAX_TITLE_LEN = 160;
const MAX_URL_LEN = 512;
const MAX_REASON_LEN = 1000;
const MIN_REASON_LEN = 5;
const SHA256_HEX_LEN = 64;
const MAX_SEATS_LIMIT = 100_000;
const MAX_VALID_DAYS = 3650;
const MAX_ROLLOUT_PCT = 100;
const MAX_GITHUB_SEG_LEN = 100;

export const ASCII_IDENTIFIER_REGEX = /^[a-zA-Z0-9._-]{2,80}$/;
export const SHA256_HEX_REGEX = /^[0-9a-fA-F]{64}$/;
export const COMMIT_SHA40_REGEX = /^[0-9a-fA-F]{40}$/;
export const SEMVER_RELEASE_TAG_REGEX = /^v?[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$/;
const GITHUB_SEGMENT_REGEX = /^[a-zA-Z0-9._-]+$/;
const HOSTNAME_REGEX = /^[a-zA-Z0-9.-]+$/;

const FORBIDDEN_MUTABLE_GITHUB_REFS = new Set([
  "main",
  "master",
  "head",
  "develop",
  "dev",
  "latest",
  "canary",
  "nightly",
]);

export function isValidGithubRepoUrl(raw: string): boolean {
  const trimmed = raw.trim();
  if (!trimmed || /[\s\u0000-\u001f\u007f]/.test(trimmed)) return false;
  if (!trimmed.startsWith("https://github.com/")) return false;
  const rest = trimmed.slice("https://github.com/".length);
  const clean = rest.endsWith("/") ? rest.slice(0, -1) : rest;
  const parts = clean.split("/");
  if (parts.length !== 2) return false;
  return parts.every(
    (seg) =>
      seg.length > 0 &&
      seg.length <= MAX_GITHUB_SEG_LEN &&
      seg !== "." &&
      seg !== ".." &&
      !seg.includes("..") &&
      GITHUB_SEGMENT_REGEX.test(seg)
  );
}

export function isValidPinnedCommitOrTag(raw: string): boolean {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.length > SHA256_HEX_LEN) return false;
  if (FORBIDDEN_MUTABLE_GITHUB_REFS.has(trimmed.toLowerCase())) return false;
  if (COMMIT_SHA40_REGEX.test(trimmed)) return true;
  if (!SEMVER_RELEASE_TAG_REGEX.test(trimmed)) return false;
  const dashIdx = trimmed.indexOf("-");
  if (dashIdx !== -1) {
    const pre = trimmed.slice(dashIdx + 1);
    if (pre.startsWith(".") || pre.endsWith(".") || pre.includes("..")) return false;
  }
  return true;
}

export function isValidHttpsDownloadUrl(raw: string): boolean {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.length > MAX_URL_LEN || /[\s\u0000-\u001f\u007f]/.test(trimmed)) {
    return false;
  }
  if (!trimmed.startsWith("https://")) return false;
  const rest = trimmed.slice("https://".length);
  const authority = rest.split(/[/?#]/, 1)[0] ?? "";
  if (!authority || authority.includes("@")) return false;
  const colonIdx = authority.indexOf(":");
  const host = colonIdx === -1 ? authority : authority.slice(0, colonIdx);
  const portStr = colonIdx === -1 ? null : authority.slice(colonIdx + 1);
  if (
    !host ||
    host.startsWith(".") ||
    host.endsWith(".") ||
    host.includes("..") ||
    !HOSTNAME_REGEX.test(host)
  ) {
    return false;
  }
  if (portStr !== null) {
    const port = Number(portStr);
    if (!/^[0-9]+$/.test(portStr) || !Number.isInteger(port) || port < 1 || port > 65535) {
      return false;
    }
  }
  return true;
}

export const DistributionChannelSchema = z.enum(["cli", "desktop", "plugin", "docker"]);
export type DistributionChannel = z.infer<typeof DistributionChannelSchema>;

export const PluginSourceKindSchema = z.enum(["DEFAULT_BUILTIN", "RECOMMENDED_GITHUB"]);
export type PluginSourceKind = z.infer<typeof PluginSourceKindSchema>;

export const PluginLifecycleStatusSchema = z.enum([
  "ACTIVE",
  "INACTIVE",
  "DEPRECATED",
  "QUARANTINED",
  "PROCESSING",
  "PENDING_APPROVAL",
]);
export type PluginLifecycleStatus = z.infer<typeof PluginLifecycleStatusSchema>;

export const ExperimentTargetSurfaceSchema = z.enum([
  "ONBOARDING_DEFAULTS",
  "PLUGIN_RANKING",
  "UX_COPY_LAYOUT",
]);
export type ExperimentTargetSurface = z.infer<typeof ExperimentTargetSurfaceSchema>;

export const ExperimentStatusSchema = z.enum([
  "DRAFT",
  "RUNNING",
  "PAUSED",
  "KILLED",
  "COMPLETED",
]);
export type ExperimentStatus = z.infer<typeof ExperimentStatusSchema>;

export const AdvisorySeveritySchema = z.enum(["CRITICAL", "HIGH", "MEDIUM"]);
export type AdvisorySeverity = z.infer<typeof AdvisorySeveritySchema>;

export const AdvisoryActionSchema = z.enum(["QUARANTINE", "WARN", "REVOKE_QUARANTINE"]);
export type AdvisoryAction = z.infer<typeof AdvisoryActionSchema>;

export const OfflineLicenseTierSchema = z.enum(["FREE", "PRO", "TEAM", "ENTERPRISE"]);
export type OfflineLicenseTier = z.infer<typeof OfflineLicenseTierSchema>;

export function createSignedEnvelopeSchema<T extends z.ZodTypeAny>(payloadSchema: T) {
  return z.object({
    schemaVersion: z.number().int().positive(),
    manifestKind: z.string().min(1),
    manifestSeq: z.number().int().min(1),
    issuedAt: z.number().int(),
    expiresAt: z.number().int(),
    keyId: z.string().min(1),
    publicKeyBase64: z.string().min(1),
    payloadCanonicalJson: z.string().min(1),
    signatureBase64: z.string().min(1),
    payload: payloadSchema,
  });
}

// ---------- Canary Rollout ----------

export const ReleaseRolloutSchema = z.object({
  id: z.string().min(1),
  channel: DistributionChannelSchema,
  version: z.string().min(1),
  versionSeq: z.number().int().min(1),
  rolloutPercent: z.number().int().min(0).max(MAX_ROLLOUT_PCT),
  killSwitch: z.boolean(),
  announcement: z.record(z.unknown()).default({}),
  downloadUrl: z.string().min(1),
  artifactSha256: z.string().min(1),
  notes: z.string().default(""),
  publishedBy: z.string().min(1),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type ReleaseRollout = z.infer<typeof ReleaseRolloutSchema>;

export const LatestReleasePayloadSchema = z.object({
  channel: DistributionChannelSchema,
  version: z.string().min(1),
  versionSeq: z.number().int().min(1),
  rolloutPercent: z.number().int().min(0).max(MAX_ROLLOUT_PCT),
  killSwitch: z.boolean(),
  canaryEligible: z.boolean(),
  downloadUrl: z.string().min(1),
  artifactSha256: z.string().min(1),
  announcement: z.record(z.unknown()).default({}),
  notes: z.string().default(""),
  channels: z.array(ReleaseRolloutSchema).default([]),
});
export type LatestReleasePayload = z.infer<typeof LatestReleasePayloadSchema>;

export const SignedLatestReleaseManifestSchema = createSignedEnvelopeSchema(LatestReleasePayloadSchema);
export type SignedLatestReleaseManifest = z.infer<typeof SignedLatestReleaseManifestSchema>;

export const PublishRolloutRequestSchema = z.object({
  channel: DistributionChannelSchema,
  version: z.string().trim().min(1).max(SHA256_HEX_LEN),
  rolloutPercent: z.number().int().min(0).max(MAX_ROLLOUT_PCT),
  killSwitch: z.boolean().default(false),
  announcement: z.record(z.unknown()).optional(),
  downloadUrl: z
    .string()
    .trim()
    .max(MAX_URL_LEN)
    .refine(isValidHttpsDownloadUrl, "downloadUrl must be a valid https:// URL with non-empty host"),
  artifactSha256: z
    .string()
    .trim()
    .regex(SHA256_HEX_REGEX, "artifactSha256 must be a 64-character hex SHA-256 digest"),
  notes: z.string().trim().max(MAX_REASON_LEN).optional(),
  reason: z.string().trim().max(MAX_REASON_LEN).optional(),
});
export type PublishRolloutRequest = z.input<typeof PublishRolloutRequestSchema>;

export const ListRolloutsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    rollouts: z.array(ReleaseRolloutSchema),
    latestSignedManifest: SignedLatestReleaseManifestSchema,
  }),
  requestId: z.string().min(1),
});
export type ListRolloutsResponse = z.infer<typeof ListRolloutsResponseSchema>;

export const PublishRolloutResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    rollout: ReleaseRolloutSchema,
    signedManifest: SignedLatestReleaseManifestSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type PublishRolloutResponse = z.infer<typeof PublishRolloutResponseSchema>;

// ---------- Plugin Catalog ----------

export const CatalogPluginSchema = z.object({
  pluginId: z.string().min(1),
  sourceKind: PluginSourceKindSchema,
  lifecycleStatus: PluginLifecycleStatusSchema,
  repoUrl: z.string().nullable().optional(),
  pinnedCommitSha: z.string().nullable().optional(),
  minAppVersion: z.string().min(1),
  defaultEnabled: z.boolean(),
  defaultConfig: z.record(z.unknown()).default({}),
  titleI18n: z.record(z.unknown()).default({}),
  summaryI18n: z.record(z.unknown()).default({}),
  manifestSeq: z.number().int().min(1),
  updatedBy: z.string().min(1),
  updatedAt: z.number().int(),
});
export type CatalogPlugin = z.infer<typeof CatalogPluginSchema>;

export const SignedCatalogManifestSchema = createSignedEnvelopeSchema(
  z.object({ plugins: z.array(CatalogPluginSchema) })
);
export type SignedCatalogManifest = z.infer<typeof SignedCatalogManifestSchema>;

const MAX_BULK_PLUGIN_IDS = 200;

export const UpsertCatalogPluginRequestSchema = z
  .object({
    pluginId: z.string().trim().regex(ASCII_IDENTIFIER_REGEX, "pluginId must match ^[a-zA-Z0-9._-]{2,80}$"),
    sourceKind: PluginSourceKindSchema,
    lifecycleStatus: PluginLifecycleStatusSchema,
    asyncSecurityScan: z.boolean().optional(),
    repoUrl: z.string().trim().max(MAX_URL_LEN).optional().nullable(),
    pinnedCommitSha: z.string().trim().max(MAX_SHORT_ID).optional().nullable(),
    minAppVersion: z.string().trim().min(1).max(SHA256_HEX_LEN).default("0.1.0"),
    defaultEnabled: z.boolean().default(true),
    defaultConfig: z.record(z.unknown()).optional(),
    titleI18n: z.record(z.unknown()).optional(),
    summaryI18n: z.record(z.unknown()).optional(),
    reason: z.string().trim().min(MIN_REASON_LEN).max(MAX_REASON_LEN),
  })
  .superRefine((val, ctx) => {
    if (val.sourceKind === "RECOMMENDED_GITHUB") {
      if (!val.repoUrl || !isValidGithubRepoUrl(val.repoUrl)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["repoUrl"],
          message: "RECOMMENDED_GITHUB requires a valid https://github.com/<owner>/<repo> URL without path traversal",
        });
      }
      if (!val.pinnedCommitSha || !isValidPinnedCommitOrTag(val.pinnedCommitSha)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["pinnedCommitSha"],
          message:
            "RECOMMENDED_GITHUB requires a pinned 40-char hex commit SHA or strict semver release tag (mutable branches are forbidden)",
        });
      }
    }
  })
  .transform((val) =>
    val.sourceKind === "DEFAULT_BUILTIN"
      ? { ...val, repoUrl: null, pinnedCommitSha: null }
      : val
  );
export type UpsertCatalogPluginRequest = z.input<typeof UpsertCatalogPluginRequestSchema>;

export const BulkUpdateCatalogRequestSchema = z.object({
  pluginIds: z
    .array(
      z.string().trim().regex(ASCII_IDENTIFIER_REGEX, "pluginId must match ^[a-zA-Z0-9._-]{2,80}$")
    )
    .min(1, "Select at least one plugin")
    .max(MAX_BULK_PLUGIN_IDS),
  lifecycleStatus: PluginLifecycleStatusSchema,
  reason: z.string().trim().min(MIN_REASON_LEN).max(MAX_REASON_LEN),
});
export type BulkUpdateCatalogRequest = z.input<typeof BulkUpdateCatalogRequestSchema>;

export const ListCatalogResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    plugins: z.array(CatalogPluginSchema),
    signedManifest: SignedCatalogManifestSchema,
  }),
  requestId: z.string().min(1),
});
export type ListCatalogResponse = z.infer<typeof ListCatalogResponseSchema>;

export const UpsertCatalogPluginResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    plugin: CatalogPluginSchema,
    signedManifest: SignedCatalogManifestSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type UpsertCatalogPluginResponse = z.infer<typeof UpsertCatalogPluginResponseSchema>;

export const BulkUpdateCatalogResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    updatedIds: z.array(z.string().min(1)),
    updatedCount: z.number().int().min(0),
    lifecycleStatus: PluginLifecycleStatusSchema,
    plugins: z.array(CatalogPluginSchema),
    signedManifest: SignedCatalogManifestSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type BulkUpdateCatalogResponse = z.infer<typeof BulkUpdateCatalogResponseSchema>;

// ---------- A/B Experiments ----------

export const AbVariantWeightSchema = z.object({
  key: z.string().trim().regex(ASCII_IDENTIFIER_REGEX, "variant key must match ^[a-zA-Z0-9._-]{2,80}$"),
  weight: z.number().int().min(0).max(MAX_ROLLOUT_PCT),
  config: z.record(z.unknown()).default({}),
});
export type AbVariantWeight = z.infer<typeof AbVariantWeightSchema>;

export const ExperimentVariantAttributionSchema = z.object({
  variantKey: z.string().min(1),
  installs: z.number().int().min(0),
  funnelDay0: z.number().int().min(0),
  funnelExport: z.number().int().min(0),
  conversionRatePct: z.number(),
  crashEvents: z.number().int().min(0),
  crashRatePct: z.number(),
});
export type ExperimentVariantAttribution = z.infer<typeof ExperimentVariantAttributionSchema>;

export const AbExperimentSchema = z.object({
  experimentKey: z.string().min(1),
  title: z.string().min(1),
  targetSurface: ExperimentTargetSurfaceSchema,
  status: ExperimentStatusSchema,
  variants: z.array(AbVariantWeightSchema),
  killSwitch: z.boolean(),
  manifestSeq: z.number().int().min(1),
  updatedBy: z.string().min(1),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
  attribution: z.array(ExperimentVariantAttributionSchema).default([]),
});
export type AbExperiment = z.infer<typeof AbExperimentSchema>;

export const SignedExperimentsManifestSchema = createSignedEnvelopeSchema(
  z.object({ experiments: z.array(AbExperimentSchema) })
);
export type SignedExperimentsManifest = z.infer<typeof SignedExperimentsManifestSchema>;

export const UpsertExperimentRequestSchema = z
  .object({
    experimentKey: z
      .string()
      .trim()
      .regex(ASCII_IDENTIFIER_REGEX, "experimentKey must match ^[a-zA-Z0-9._-]{2,80}$"),
    title: z.string().trim().min(3).max(MAX_TITLE_LEN),
    targetSurface: ExperimentTargetSurfaceSchema,
    status: ExperimentStatusSchema,
    variants: z.array(AbVariantWeightSchema).min(2),
    killSwitch: z.boolean().default(false),
    reason: z.string().trim().min(MIN_REASON_LEN).max(MAX_REASON_LEN),
  })
  .superRefine((val, ctx) => {
    const seen = new Set<string>();
    let totalWeight = 0;
    for (const v of val.variants) {
      totalWeight += v.weight;
      if (seen.has(v.key)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["variants"],
          message: `Duplicate variant key '${v.key}' is forbidden`,
        });
      }
      seen.add(v.key);
    }
    if (totalWeight !== MAX_ROLLOUT_PCT) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["variants"],
        message: `Variant weights must sum to 100 (got ${totalWeight})`,
      });
    }
  });
export type UpsertExperimentRequest = z.input<typeof UpsertExperimentRequestSchema>;

export const ListExperimentsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    experiments: z.array(AbExperimentSchema),
    signedManifest: SignedExperimentsManifestSchema,
  }),
  requestId: z.string().min(1),
});
export type ListExperimentsResponse = z.infer<typeof ListExperimentsResponseSchema>;

export const UpsertExperimentResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    experiment: AbExperimentSchema,
    signedManifest: SignedExperimentsManifestSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type UpsertExperimentResponse = z.infer<typeof UpsertExperimentResponseSchema>;

// ---------- Global Skill Quarantine ----------

export const SkillAdvisorySchema = z.object({
  id: z.string().min(1),
  skillOrPluginId: z.string().min(1),
  severity: AdvisorySeveritySchema,
  action: AdvisoryActionSchema,
  reason: z.string().min(1),
  affectedSha256: z.string().nullable().optional(),
  advisorySeq: z.number().int().min(1),
  issuedBy: z.string().min(1),
  createdAt: z.number().int(),
});
export type SkillAdvisory = z.infer<typeof SkillAdvisorySchema>;

export const SignedAdvisoriesManifestSchema = createSignedEnvelopeSchema(
  z.object({ advisories: z.array(SkillAdvisorySchema) })
);
export type SignedAdvisoriesManifest = z.infer<typeof SignedAdvisoriesManifestSchema>;

export const IssueQuarantineRequestSchema = z.object({
  skillOrPluginId: z.string().trim().min(1).max(MAX_SHORT_ID),
  severity: AdvisorySeveritySchema,
  action: AdvisoryActionSchema.default("QUARANTINE"),
  reason: z.string().trim().min(MIN_REASON_LEN).max(MAX_REASON_LEN),
  affectedSha256: z.string().trim().max(MAX_SHORT_ID).optional().nullable(),
});
export type IssueQuarantineRequest = z.input<typeof IssueQuarantineRequestSchema>;

export const ListQuarantinesResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    advisories: z.array(SkillAdvisorySchema),
    signedManifest: SignedAdvisoriesManifestSchema,
  }),
  requestId: z.string().min(1),
});
export type ListQuarantinesResponse = z.infer<typeof ListQuarantinesResponseSchema>;

export const IssueQuarantineResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    advisory: SkillAdvisorySchema,
    signedManifest: SignedAdvisoriesManifestSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type IssueQuarantineResponse = z.infer<typeof IssueQuarantineResponseSchema>;

// ---------- Offline Licenses & Cowok.ai Federation ----------

export const OfflineLicensePayloadSchema = z.object({
  licenseId: z.string().min(1),
  tier: OfflineLicenseTierSchema,
  subjectOrg: z.string().min(1),
  cowokOrgId: z.string().nullable().optional(),
  maxSeats: z.number().int().min(1),
  features: z.array(z.string()),
  issuedAt: z.number().int(),
  expiresAt: z.number().int(),
});
export type OfflineLicensePayload = z.infer<typeof OfflineLicensePayloadSchema>;

export const VerifiedOfflineLicenseSchema = z.object({
  payload: OfflineLicensePayloadSchema,
  keyId: z.string().min(1),
  publicKeyBase64: z.string().min(1),
  offlineVerified: z.boolean(),
});
export type VerifiedOfflineLicense = z.infer<typeof VerifiedOfflineLicenseSchema>;

export const OfflineLicenseRecordSchema = z.object({
  licenseId: z.string().min(1),
  tier: OfflineLicenseTierSchema,
  subjectOrg: z.string().min(1),
  cowokOrgId: z.string().nullable().optional(),
  maxSeats: z.number().int().min(1),
  issuedAt: z.number().int(),
  expiresAt: z.number().int(),
  signedToken: z.string().min(1),
  revoked: z.boolean(),
  issuedBy: z.string().min(1),
  offlineVerified: z.boolean(),
});
export type OfflineLicenseRecord = z.infer<typeof OfflineLicenseRecordSchema>;

export const IssueLicenseRequestSchema = z.object({
  tier: OfflineLicenseTierSchema,
  subjectOrg: z.string().trim().min(2).max(MAX_TITLE_LEN),
  cowokOrgId: z.string().trim().max(MAX_SHORT_ID).optional().nullable(),
  maxSeats: z.number().int().min(1).max(MAX_SEATS_LIMIT).default(1),
  validDays: z.number().int().min(1).max(MAX_VALID_DAYS).default(365),
});
export type IssueLicenseRequest = z.input<typeof IssueLicenseRequestSchema>;

export const ListLicensesResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    licenses: z.array(OfflineLicenseRecordSchema),
    keyId: z.string().min(1),
    publicKeyBase64: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type ListLicensesResponse = z.infer<typeof ListLicensesResponseSchema>;

export const IssueLicenseResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    license: OfflineLicenseRecordSchema,
    verification: VerifiedOfflineLicenseSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type IssueLicenseResponse = z.infer<typeof IssueLicenseResponseSchema>;

// ---------- Audit Logs ----------

export const DistributionAuditLogSchema = z.object({
  id: z.string().min(1),
  actorSubject: z.string().min(1),
  actorRole: z.string().min(1),
  mfaVerified: z.boolean(),
  action: z.string().min(1),
  targetResource: z.string().min(1),
  details: z.record(z.unknown()).default({}),
  createdAt: z.number().int(),
});
export type DistributionAuditLog = z.infer<typeof DistributionAuditLogSchema>;

export const ListDistributionAuditLogsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    logs: z.array(DistributionAuditLogSchema),
  }),
  requestId: z.string().min(1),
});
export type ListDistributionAuditLogsResponse = z.infer<
  typeof ListDistributionAuditLogsResponseSchema
>;

// ---------- Client Edge & Cowok.ai Federation ----------

export const LocalPluginOverrideSchema = z.object({
  pluginId: z.string().min(1),
  enabled: z.boolean(),
  pinnedVersion: z.string().nullable().optional(),
});

export const EffectivePluginStateSchema = z.object({
  pluginId: z.string().min(1),
  sourceKind: z.string().min(1),
  lifecycleStatus: z.string().min(1),
  effectiveEnabled: z.boolean(),
  quarantineLocked: z.boolean(),
  resolutionSource: z.string().min(1),
  advisoryReason: z.string().nullable().optional(),
  advisorySeverity: z.string().nullable().optional(),
});

export const ClientEdgeSyncRequestSchema = z.object({
  installId: z.string().trim().min(1).max(MAX_SHORT_ID),
  channel: DistributionChannelSchema.default("desktop"),
  appVersion: z.string().trim().min(1).max(SHA256_HEX_LEN).default("0.1.0"),
  localOverrides: z.array(LocalPluginOverrideSchema).default([]),
  lastSeenReleaseSeq: z.number().int().min(0).default(0),
  lastSeenCatalogSeq: z.number().int().min(0).default(0),
  lastSeenAdvisoriesSeq: z.number().int().min(0).default(0),
  lastSeenExperimentsSeq: z.number().int().min(0).default(0),
  offlineLicenseToken: z.string().optional(),
  latestReleaseManifest: SignedLatestReleaseManifestSchema.optional(),
  catalogManifest: SignedCatalogManifestSchema.optional(),
  advisoriesManifest: SignedAdvisoriesManifestSchema.optional(),
  experimentsManifest: SignedExperimentsManifestSchema.optional(),
});
export type ClientEdgeSyncRequest = z.input<typeof ClientEdgeSyncRequestSchema>;

export const ClientEdgeSyncResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    installId: z.string().min(1),
    verifiedManifestSeqs: z.record(z.unknown()),
    effectivePlugins: z.array(EffectivePluginStateSchema),
    experimentAssignments: z.array(
      z.object({
        experimentKey: z.string().min(1),
        variantKey: z.string().min(1),
        bucket: z.number().int().min(0).max(MAX_ROLLOUT_PCT),
        telemetryTag: z.string().min(1),
        config: z.record(z.unknown()).default({}),
      })
    ),
    experimentVariants: z.array(z.string()),
    licenseVerification: VerifiedOfflineLicenseSchema.nullable().optional(),
  }),
  requestId: z.string().min(1),
});
export type ClientEdgeSyncResponse = z.infer<typeof ClientEdgeSyncResponseSchema>;

export const CowokOrgFederationRequestSchema = z.object({
  cowokOrgId: z.string().trim().min(2).max(MAX_SHORT_ID),
  installId: z.string().trim().min(1).max(MAX_SHORT_ID),
  licenseToken: z.string().trim().min(1),
});
export type CowokOrgFederationRequest = z.input<typeof CowokOrgFederationRequestSchema>;

export const CowokOrgFederationResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    federated: z.boolean(),
    cowokOrgId: z.string().min(1),
    installId: z.string().min(1),
    licenseId: z.string().min(1),
    tier: OfflineLicenseTierSchema,
    subjectOrg: z.string().min(1),
    maxSeats: z.number().int().min(1),
    expiresAt: z.number().int(),
    offlineVerified: z.boolean(),
  }),
  requestId: z.string().min(1),
});
export type CowokOrgFederationResponse = z.infer<typeof CowokOrgFederationResponseSchema>;
