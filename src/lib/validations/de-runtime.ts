// @story 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";

const MAX_ID_LEN = 128;
const MAX_PATH_LEN = 512;
const MAX_URL_LEN = 512;
const MAX_SLUG_LEN = 64;
const MAX_DESC_LEN = 2_000;
const MAX_MARKDOWN_LEN = 65_536;
const MAX_SCRIPT_LEN = 65_536;
const MAX_CONTENT_OVERRIDE_LEN = 524_288;
export const MAX_HEADER_TOKENS = 100;
export const MIN_TOOL_TIMEOUT_MS = 100;
export const MAX_TOOL_TIMEOUT_MS = 5_000;
export const DEFAULT_TOOL_TIMEOUT_MS = 3_000;
const MAX_STAGED_FILES = 16;
const MAX_ATTACHED_SCRIPTS = 8;
const MAX_TOKEN_PAIRS = 32;
const SHA256_HEX_LEN = 64;
export const LEARN_MIN_HIT_COUNT = 3;
export const LEARN_PROMOTE_SLPR = 0.95;
export const LEARN_RETIRE_SLPR = 0.85;

export const BuiltInDeToolSchema = z.enum([
  "scan-ui",
  "intake",
  "tokens",
  "nfa-packs",
  "archify",
  "get_artifact",
  "notes-check",
]);

export type BuiltInDeTool = z.infer<typeof BuiltInDeToolSchema>;

export const DeFindingSeveritySchema = z.enum(["ERROR", "WARNING", "INFO"]);

export const DeFindingSchema = z.object({
  ruleId: z.string().min(1),
  pack: z.string().min(1),
  severity: DeFindingSeveritySchema,
  message: z.string(),
  line: z.number().int().min(1),
  snippet: z.string(),
});

export type DeFindingDto = z.infer<typeof DeFindingSchema>;

export const TokenPairInputSchema = z
  .object({
    tokenName: z.string().trim().min(1).max(MAX_SLUG_LEN),
    fgHex: z.string().trim().regex(/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/),
    bgHex: z.string().trim().regex(/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/),
  })
  .strict();

export const RunBuiltInDeRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    tools: z.array(BuiltInDeToolSchema).min(1),
    targetArtifactPath: z.string().trim().min(1).max(MAX_PATH_LEN),
    contentOverride: z.string().max(MAX_CONTENT_OVERRIDE_LEN).optional(),
    tokenPairs: z.array(TokenPairInputSchema).max(MAX_TOKEN_PAIRS).optional(),
  })
  .strict();

export type RunBuiltInDeRequest = z.infer<typeof RunBuiltInDeRequestSchema>;

export const DeToolResultSchema = z.object({
  tool: BuiltInDeToolSchema,
  passed: z.boolean(),
  durationMs: z.number().int().min(0),
  durationMicros: z.number().int().min(0),
  findings: z.array(DeFindingSchema),
  metadata: z.unknown().optional(),
});

export type DeToolResultDto = z.infer<typeof DeToolResultSchema>;

export const DeRunReportSchema = z.object({
  targetArtifactPath: z.string().min(1),
  passed: z.boolean(),
  costUsd: z.number().min(0),
  totalDurationMs: z.number().int().min(0),
  withinSla: z.boolean(),
  results: z.array(DeToolResultSchema),
});

export type DeRunReportDto = z.infer<typeof DeRunReportSchema>;

export const RunBuiltInDeResponseSchema = z
  .object({
    ok: z.literal(true),
    data: DeRunReportSchema,
    requestId: z.string().min(1),
  })
  .strict();

export type RunBuiltInDeResponse = z.infer<typeof RunBuiltInDeResponseSchema>;

export const SecurityScanVerdictSchema = z.enum(["PASS", "QUARANTINED", "REJECT"]);
export const LicenseClassificationSchema = z.enum([
  "ADOPT_PRIOR_ART",
  "REFERENCE_ONLY",
  "REJECT",
]);

export const ImportUserSkillRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN).optional(),
    skillSlug: z
      .string()
      .trim()
      .min(1)
      .max(MAX_SLUG_LEN)
      .regex(/^[a-z0-9][a-z0-9._-]*$/i, "Skill slug must use alphanumeric, dot, dash, or underscore"),
    repoSourceUrl: z.string().trim().url().max(MAX_URL_LEN),
    licenseSpdx: z.string().trim().min(1).max(MAX_SLUG_LEN),
    description: z.string().trim().min(1).max(MAX_DESC_LEN),
    bodyMarkdown: z.string().trim().min(1).max(MAX_MARKDOWN_LEN),
    attachedScripts: z.array(z.string().max(MAX_SCRIPT_LEN)).max(MAX_ATTACHED_SCRIPTS).optional(),
  })
  .strict();

export type ImportUserSkillRequest = z.infer<typeof ImportUserSkillRequestSchema>;

export const PackagedUserSkillReceiptSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string().min(1),
  projectId: z.string().nullable(),
  skillSlug: z.string().min(1),
  repoSourceUrl: z.string(),
  headerTokens: z.number().int().min(0).max(MAX_HEADER_TOKENS),
  securityScanVerdict: SecurityScanVerdictSchema,
  licenseClassification: LicenseClassificationSchema,
  skillMdPath: z.string().min(1),
  skillMdPreview: z.string(),
  createdAt: z.number().int().min(0),
});

export type PackagedUserSkillReceiptDto = z.infer<typeof PackagedUserSkillReceiptSchema>;

export const ImportUserSkillResponseSchema = z
  .object({
    ok: z.literal(true),
    data: PackagedUserSkillReceiptSchema,
    requestId: z.string().min(1),
  })
  .strict();

export type ImportUserSkillResponse = z.infer<typeof ImportUserSkillResponseSchema>;

export const ListUserSkillsResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z.object({ items: z.array(PackagedUserSkillReceiptSchema) }),
    requestId: z.string().min(1),
  })
  .strict();

export type ListUserSkillsResponse = z.infer<typeof ListUserSkillsResponseSchema>;

export const EphemeralRuntimeSchema = z.enum(["sh", "python3", "node"]);
export type EphemeralRuntime = z.infer<typeof EphemeralRuntimeSchema>;

export const StagedJailFileSchema = z
  .object({
    relativePath: z.string().trim().min(1).max(MAX_PATH_LEN),
    content: z.string().max(MAX_SCRIPT_LEN),
  })
  .strict();

export const ExecuteEphemeralToolRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    toolName: z.string().trim().min(1).max(MAX_SLUG_LEN),
    runtime: EphemeralRuntimeSchema,
    scriptSource: z.string().min(1).max(MAX_SCRIPT_LEN),
    timeoutMs: z
      .number()
      .int()
      .min(MIN_TOOL_TIMEOUT_MS)
      .max(MAX_TOOL_TIMEOUT_MS)
      .default(DEFAULT_TOOL_TIMEOUT_MS),
    stagedFiles: z.array(StagedJailFileSchema).max(MAX_STAGED_FILES).optional(),
  })
  .strict();

export type ExecuteEphemeralToolRequest = z.infer<typeof ExecuteEphemeralToolRequestSchema>;

export const ToolJailAuditReceiptSchema = z.object({
  executionId: z.string().min(1),
  toolName: z.string().min(1),
  runtime: EphemeralRuntimeSchema,
  actorKind: z.string().min(1),
  scriptSha256: z.string().length(SHA256_HEX_LEN),
  exitCode: z.number().int(),
  timedOut: z.boolean(),
  killedPgid: z.number().int().nullable(),
  durationMs: z.number().int().min(0),
  stdout: z.string(),
  stderr: z.string(),
  envCleared: z.boolean(),
  networkIsolated: z.boolean(),
  networkProxySinkholed: z.boolean().optional(),
  nofollowVerified: z.boolean(),
  rlimitAsBytes: z.number().int().min(0),
  rlimitNproc: z.number().int().min(0),
});

export type ToolJailAuditReceiptDto = z.infer<typeof ToolJailAuditReceiptSchema>;

export const ExecuteEphemeralToolResponseSchema = z
  .object({
    ok: z.literal(true),
    data: ToolJailAuditReceiptSchema,
    requestId: z.string().min(1),
  })
  .strict();

export type ExecuteEphemeralToolResponse = z.infer<typeof ExecuteEphemeralToolResponseSchema>;

export const LedgerStatusSchema = z.enum(["ACCUMULATING", "PROPOSED", "PROMOTED", "RETIRED"]);
export type LedgerStatus = z.infer<typeof LedgerStatusSchema>;

export const GateFailureLedgerEntrySchema = z.object({
  id: z.string().min(1),
  tenantId: z.string().min(1),
  projectId: z.string().min(1),
  ruleId: z.string().min(1),
  fingerprintSha256: z.string().length(SHA256_HEX_LEN),
  sampleMessage: z.string(),
  hitCount: z.number().int().min(1),
  slprScore: z.number().min(0).max(1),
  promotedCardId: z.string().nullable(),
  status: LedgerStatusSchema,
  lastSeenAt: z.number().int().min(0),
  qualifiesForProposal: z.boolean(),
});

export type GateFailureLedgerEntryDto = z.infer<typeof GateFailureLedgerEntrySchema>;

export const LearnLedgerListResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z.object({
      projectId: z.string().min(1),
      entries: z.array(GateFailureLedgerEntrySchema),
      minHitCount: z.number().int().min(1),
      promoteSlprThreshold: z.number().min(0).max(1),
      retireSlprThreshold: z.number().min(0).max(1),
    }),
    requestId: z.string().min(1),
  })
  .strict();

export type LearnLedgerListResponse = z.infer<typeof LearnLedgerListResponseSchema>;

export const RecordLearnLedgerRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_ID_LEN),
    ruleId: z.string().trim().min(1).max(MAX_SLUG_LEN),
    failureMessage: z.string().trim().min(1).max(MAX_DESC_LEN),
    slprScore: z.number().min(0).max(1),
    sourceEvidence: z.string().trim().max(MAX_PATH_LEN).optional(),
  })
  .strict();

export type RecordLearnLedgerRequest = z.infer<typeof RecordLearnLedgerRequestSchema>;

export const RecordLearnLedgerResponseSchema = z
  .object({
    ok: z.literal(true),
    data: GateFailureLedgerEntrySchema,
    requestId: z.string().min(1),
  })
  .strict();

export type RecordLearnLedgerResponse = z.infer<typeof RecordLearnLedgerResponseSchema>;

export const DeRuntimeErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "AUTH_FORBIDDEN",
  "HUMAN_APPROVAL_REQUIRED",
  "AST_CAPABILITY_BLOCKED",
  "JAIL_TIMEOUT_KILLED",
  "PATH_JAIL_VIOLATION",
  "SSRF_URL_BLOCKED",
  "LICENSE_REJECTED",
  "SECURITY_SCAN_REJECTED",
  "LEARN_SMUGGLING_BLOCKED",
  "LEARN_SCOPE_CAPACITY_EXCEEDED",
  "PROJECT_NOT_FOUND",
  "PAYLOAD_TOO_LARGE",
  "STORAGE_BUSY",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
]);

export type DeRuntimeErrorCode = z.infer<typeof DeRuntimeErrorCodeSchema>;

export const DeRuntimeErrorEnvelopeSchema = z
  .object({
    error: z.object({
      code: DeRuntimeErrorCodeSchema,
      message: z.string().min(1),
      field: z.string().optional(),
      requestId: z.string().min(1),
      details: z.record(z.unknown()).optional(),
    }),
  })
  .strict();

export type DeRuntimeErrorEnvelope = z.infer<typeof DeRuntimeErrorEnvelopeSchema>;
