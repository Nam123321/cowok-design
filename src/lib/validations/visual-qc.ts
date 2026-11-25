// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";

export const MAX_AUTOFIX_ATTEMPTS = 3;
const MAX_FF_ID_LEN = 128;
const MAX_TAG_LEN = 64;
const MAX_ROLE_LEN = 64;
const MAX_TARGET_ID_LEN = 160;
const MAX_HTML_CHARS = 524_288;
const MAX_CONSTRAINTS_CHARS = 65_536;
const MIN_VIEWPORT_PX = 320;
const MAX_VIEWPORT_PX = 1920;
const MAX_VIEWPORT_COUNT = 12;
const MAX_FIXTURE_REF_LEN = 128;
const MAX_GEOMETRY_NODES = 500;
const MAX_FLOW_NODES = 200;
const MAX_FLOW_EDGES = 500;
const MAX_RGBA_CHANNEL = 255;
const LAYOUT_SHA1_HEX_LEN = 40;

export const QcTargetKindSchema = z.enum([
  "APP_SCREEN",
  "PRESENTATION_SLIDE",
  "FULL_FLOW",
]);

export const QcVerdictSchema = z.enum([
  "PASS",
  "FAIL",
  "UNVERIFIED_NO_BROWSER",
]);

export const QcSeveritySchema = z.enum(["P0", "P1", "P2"]);

export const QcBoundingBoxSchema = z.object({
  x: z.number(),
  y: z.number(),
  width: z.number().nonnegative(),
  height: z.number().nonnegative(),
});

export const DomGeometryNodeSchema = z.object({
  ffId: z.string().min(1).max(MAX_FF_ID_LEN),
  tag: z.string().min(1).max(MAX_TAG_LEN),
  role: z.string().max(MAX_ROLE_LEN).nullable().optional(),
  bbox: QcBoundingBoxSchema,
  zIndex: z.number().int().optional(),
  scrollWidth: z.number().nonnegative().optional(),
  clientWidth: z.number().nonnegative().optional(),
  scrollHeight: z.number().nonnegative().optional(),
  clientHeight: z.number().nonnegative().optional(),
  fontSizePx: z.number().positive().optional(),
  lineHeightRatio: z.number().positive().optional(),
  fgHex: z.string().nullable().optional(),
  bgHex: z.string().nullable().optional(),
  borderRadiusPx: z.number().nonnegative().nullable().optional(),
  parentRadiusPx: z.number().nonnegative().nullable().optional(),
  parentPaddingPx: z.number().nonnegative().nullable().optional(),
  isInteractive: z.boolean().optional(),
  isVisible: z.boolean().optional(),
  pointerEventsNone: z.boolean().optional(),
  textLen: z.number().int().nonnegative().optional(),
  hasAriaLabel: z.boolean().optional(),
  hasFocusRing: z.boolean().optional(),
});

export const QcIssueSchema = z.object({
  id: z.string().min(1),
  ruleId: z.string().min(1),
  category: z.string().min(1),
  severity: QcSeveritySchema,
  message: z.string().min(1),
  viewportWidth: z.number().int().positive(),
  ffId: z.string().nullable().optional(),
  bbox: QcBoundingBoxSchema.nullable().optional(),
  deterministicFixable: z.boolean(),
  suggestedFix: z.string().nullable().optional(),
  targetNodeId: z.string().nullable().optional(),
});

export const FlowCtaNodeSchema = z.object({
  ctaId: z.string().min(1),
  label: z.string().min(1),
  targetScreenId: z.string().nullable().optional(),
});

export const FlowAuditNodeSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  isEntry: z.boolean().optional(),
  subStates: z.array(z.string()).default([]),
  ctas: z.array(FlowCtaNodeSchema).default([]),
});

export const FlowAuditEdgeSchema = z.object({
  source: z.string().min(1),
  target: z.string().min(1),
  triggerCtaId: z.string().nullable().optional(),
});

export const FlowCompletenessReportSchema = z.object({
  deadEndCtas: z.array(z.string()),
  orphanScreens: z.array(z.string()),
  missingSubstates: z.record(z.string(), z.array(z.string())),
  issues: z.array(QcIssueSchema),
  isComplete: z.boolean(),
});

export const CdpBreakerStatusSchema = z.object({
  isOpen: z.boolean(),
  consecutiveCrashes: z.number().int().nonnegative(),
  totalCrashes: z.number().int().nonnegative(),
  totalRestarts: z.number().int().nonnegative(),
  lastCrashReason: z.string().nullable().optional(),
});

export const QcRunRequestSchema = z.object({
  projectId: z.string().min(1),
  targetKind: QcTargetKindSchema.default("APP_SCREEN"),
  targetId: z.string().min(1).max(MAX_TARGET_ID_LEN),
  html: z.string().max(MAX_HTML_CHARS).default(""),
  constraintsJson: z.string().max(MAX_CONSTRAINTS_CHARS).optional(),
  viewports: z.array(z.number().int().min(MIN_VIEWPORT_PX).max(MAX_VIEWPORT_PX)).min(1).max(MAX_VIEWPORT_COUNT).optional(),
  fixtureRef: z.string().max(MAX_FIXTURE_REF_LEN).optional(),
  geometryNodes: z.array(DomGeometryNodeSchema).max(MAX_GEOMETRY_NODES).optional(),
  flowNodes: z.array(FlowAuditNodeSchema).max(MAX_FLOW_NODES).optional(),
  flowEdges: z.array(FlowAuditEdgeSchema).max(MAX_FLOW_EDGES).optional(),
  referenceRgba: z.array(z.number().int().min(0).max(MAX_RGBA_CHANNEL)).optional(),
  candidateRgba: z.array(z.number().int().min(0).max(MAX_RGBA_CHANNEL)).optional(),
  frameWidth: z.number().int().nonnegative().optional(),
  frameHeight: z.number().int().nonnegative().optional(),
  ssimThreshold: z.number().min(0).max(1).optional(),
  forceRefresh: z.boolean().optional(),
  forceNoBrowser: z.boolean().optional(),
});

export const QcRunResponseDataSchema = z.object({
  qcRunId: z.string().min(1),
  projectId: z.string().min(1),
  targetKind: QcTargetKindSchema,
  targetId: z.string().min(1),
  layoutSha1: z.string().length(LAYOUT_SHA1_HEX_LEN),
  viewports: z.array(z.number().int().positive()),
  verdict: QcVerdictSchema,
  p0Count: z.number().int().nonnegative(),
  p1Count: z.number().int().nonnegative(),
  ssimScore: z.number().min(0).max(1).nullable(),
  issues: z.array(QcIssueSchema),
  autofixAttempt: z.number().int().min(0).max(MAX_AUTOFIX_ATTEMPTS),
  maxAutofixAttempts: z.number().int().positive(),
  cacheHit: z.boolean(),
  usedFallback: z.boolean(),
  fallbackMode: z.string().nullable().optional(),
  flowCompleteness: FlowCompletenessReportSchema.nullable().optional(),
  rulesEvaluated: z.number().int().nonnegative(),
  breakerStatus: CdpBreakerStatusSchema,
  createdAt: z.number().int(),
});

export const QcRunResponseSchema = z.object({
  ok: z.literal(true),
  data: QcRunResponseDataSchema,
  requestId: z.string(),
});

export const QcAutoFixModeSchema = z.enum(["DETERMINISTIC_LINT_FIX", "AGENT_PATCH"]);

export const QcAutoFixRequestSchema = z.object({
  projectId: z.string().min(1),
  qcRunId: z.string().min(1),
  issueIds: z.array(z.string().min(1)).min(1),
  mode: QcAutoFixModeSchema,
  attemptNumber: z.number().int().min(1).max(MAX_AUTOFIX_ATTEMPTS),
  currentHtml: z.string().max(MAX_HTML_CHARS).optional(),
});

export const QcAutoFixResponseDataSchema = z.object({
  qcRunId: z.string().min(1),
  projectId: z.string().min(1),
  mode: QcAutoFixModeSchema,
  attemptNumber: z.number().int().min(0).max(MAX_AUTOFIX_ATTEMPTS),
  maxAutofixAttempts: z.number().int().positive(),
  remainingAttempts: z.number().int().nonnegative(),
  patchedHtml: z.string(),
  previousHtml: z.string(),
  fixedIssueIds: z.array(z.string()),
  unfixableIssueIds: z.array(z.string()),
  elapsedMs: z.number().nonnegative(),
  llmTokensUsed: z.number().int().nonnegative(),
  diffSummary: z.array(z.string()),
  rollbackSupported: z.boolean(),
  requiresAgentPatch: z.boolean(),
});

export const QcAutoFixResponseSchema = z.object({
  ok: z.literal(true),
  data: QcAutoFixResponseDataSchema,
  requestId: z.string(),
});

export const VisualQcErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "FORBIDDEN",
  "PROJECT_NOT_FOUND",
  "PROJECT_TENANT_MISMATCH",
  "PAYLOAD_TOO_LARGE",
  "QC_RUN_NOT_FOUND",
  "QC_EVALUATION_FAILED",
  "AUTOFIX_MAX_ATTEMPTS_EXCEEDED",
  "STORAGE_BUSY",
  "INTERNAL_ERROR",
]);

export const VisualQcErrorResponseSchema = z.object({
  error: z.object({
    code: VisualQcErrorCodeSchema,
    message: z.string(),
    field: z.string().optional(),
    requestId: z.string(),
    details: z.record(z.unknown()).optional(),
  }),
});

export type QcTargetKind = z.infer<typeof QcTargetKindSchema>;
export type QcVerdict = z.infer<typeof QcVerdictSchema>;
export type QcSeverity = z.infer<typeof QcSeveritySchema>;
export type QcBoundingBox = z.infer<typeof QcBoundingBoxSchema>;
export type DomGeometryNode = z.infer<typeof DomGeometryNodeSchema>;
export type QcIssue = z.infer<typeof QcIssueSchema>;
export type FlowCtaNode = z.infer<typeof FlowCtaNodeSchema>;
export type FlowAuditNode = z.infer<typeof FlowAuditNodeSchema>;
export type FlowAuditEdge = z.infer<typeof FlowAuditEdgeSchema>;
export type FlowCompletenessReport = z.infer<typeof FlowCompletenessReportSchema>;
export type CdpBreakerStatus = z.infer<typeof CdpBreakerStatusSchema>;
export type QcRunRequest = z.infer<typeof QcRunRequestSchema>;
export type QcRunResponseData = z.infer<typeof QcRunResponseDataSchema>;
export type QcRunResponse = z.infer<typeof QcRunResponseSchema>;
export type QcAutoFixMode = z.infer<typeof QcAutoFixModeSchema>;
export type QcAutoFixRequest = z.infer<typeof QcAutoFixRequestSchema>;
export type QcAutoFixResponseData = z.infer<typeof QcAutoFixResponseDataSchema>;
export type QcAutoFixResponse = z.infer<typeof QcAutoFixResponseSchema>;
export type VisualQcErrorResponse = z.infer<typeof VisualQcErrorResponseSchema>;
