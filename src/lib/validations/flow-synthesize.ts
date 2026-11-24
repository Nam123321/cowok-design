// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (Zod Runtime Contracts for 3-Tier Flow DAG Synthesis & VerdictShield 0% Float Leak)
import { z } from "zod";

const MAX_CLUSTER_NODES = 8;
const MIN_CLUSTER_NODES = 1;
const FULL_COVERAGE_PERCENT = 100;
const ZERO_FLOATS = 0;
const MAX_PROJECT_ID_LEN = 128;
const MAX_BRIEF_SUMMARY_LEN = 4000;
const MAX_SCREENS_COUNT = 64;
const MAX_CANDIDATE_EDGES_COUNT = 256;
const MAX_SCREEN_KEY_LEN = 128;
const MAX_SCREEN_TITLE_LEN = 200;
const MAX_ROUTE_PATH_LEN = 256;
const MAX_PERSONA_DIMENSION_LEN = 128;
const MAX_FMEA_FAILURE_MODE_LEN = 500;
const MAX_EDGE_NODE_REF_LEN = 128;
const MAX_TRIGGER_EVENT_LEN = 128;
const MAX_GUARD_CONDITION_LEN = 512;

export const FlowTierSchema = z.enum(["PRIMARY", "EDGE_CASE"]);
export type FlowTier = z.infer<typeof FlowTierSchema>;

export const FlowSubstateVariantSchema = z.enum(["DEFAULT", "LOADING", "EMPTY", "ERROR"]);
export type FlowSubstateVariant = z.infer<typeof FlowSubstateVariantSchema>;

export const FlowSeverityBandSchema = z.enum(["LOW", "MEDIUM", "HIGH", "CRITICAL"]);
export type FlowSeverityBand = z.infer<typeof FlowSeverityBandSchema>;

export const FlowConfidenceKindSchema = z.enum([
  "ExactMatch",
  "ModelVerdict",
  "DeterministicFallback",
]);
export type FlowConfidenceKind = z.infer<typeof FlowConfidenceKindSchema>;

export const FlowSynthesizeErrorCodeSchema = z.enum([
  "UNAUTHORIZED",
  "FORBIDDEN",
  "INVALID_REQUEST",
  "PAYLOAD_TOO_LARGE",
  "PROJECT_NOT_FOUND",
  "PROJECT_TENANT_MISMATCH",
  "STORAGE_BUSY",
  "INTERNAL_ERROR",
  "VALIDATION_ERROR",
]);

export const ScreenSeedInputSchema = z
  .object({
    screenKey: z.string().trim().min(1).max(MAX_SCREEN_KEY_LEN),
    title: z.string().trim().min(1).max(MAX_SCREEN_TITLE_LEN),
    routePath: z.string().max(MAX_ROUTE_PATH_LEN).optional(),
    personaDimension: z.string().max(MAX_PERSONA_DIMENSION_LEN).optional(),
    fmeaFailureMode: z.string().max(MAX_FMEA_FAILURE_MODE_LEN).optional(),
    severityBand: FlowSeverityBandSchema.optional(),
  })
  .strict();

export const RawEdgeCandidateSchema = z
  .object({
    sourceNodeId: z.string().trim().min(1).max(MAX_EDGE_NODE_REF_LEN),
    targetNodeId: z.string().trim().min(1).max(MAX_EDGE_NODE_REF_LEN),
    flowType: z.enum(["main", "if_else", "if_not", "loopback"]),
    triggerEvent: z.string().trim().min(1).max(MAX_TRIGGER_EVENT_LEN),
    guardCondition: z.string().max(MAX_GUARD_CONDITION_LEN).nullable().optional(),
    flowTier: FlowTierSchema.optional(),
  })
  .strict();

export const FlowSynthesizeRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_PROJECT_ID_LEN),
    briefSummary: z.string().trim().min(1).max(MAX_BRIEF_SUMMARY_LEN),
    maxDefaultClusterNodes: z
      .number()
      .int()
      .min(MIN_CLUSTER_NODES)
      .max(MAX_CLUSTER_NODES)
      .default(MAX_CLUSTER_NODES),
    includeFmeaBranches: z.boolean().default(true),
    screens: z.array(ScreenSeedInputSchema).max(MAX_SCREENS_COUNT).optional().default([]),
    candidateEdges: z
      .array(RawEdgeCandidateSchema)
      .max(MAX_CANDIDATE_EDGES_COUNT)
      .optional()
      .default([]),
  })
  .strict();
export type FlowSynthesizeRequest = z.infer<typeof FlowSynthesizeRequestSchema>;

export const SynthesizedNodeSchema = z
  .object({
    id: z.string().min(1),
    screenKey: z.string().min(1),
    title: z.string().min(1),
    routePath: z.string(),
    nodeType: z.string().min(1),
    flowTier: FlowTierSchema,
    clusterId: z.string().min(1),
    severityBand: FlowSeverityBandSchema,
    mergedScreenKeys: z.array(z.string()),
    posX: z.number().int(),
    posY: z.number().int(),
    defaultFixtureId: z.string().min(1),
  })
  .strict();
export type SynthesizedNode = z.infer<typeof SynthesizedNodeSchema>;

export const SynthesizedEdgeSchema = z
  .object({
    id: z.string().min(1),
    sourceNodeId: z.string().min(1),
    targetNodeId: z.string().min(1),
    flowType: z.enum(["main", "if_else", "if_not", "loopback"]),
    triggerEvent: z.string().min(1),
    guardCondition: z.string().nullable(),
    flowTier: FlowTierSchema,
    severityBand: FlowSeverityBandSchema,
  })
  .strict();
export type SynthesizedEdge = z.infer<typeof SynthesizedEdgeSchema>;

export const SynthesizedSubstateBindingSchema = z
  .object({
    nodeId: z.string().min(1),
    projectId: z.string().min(1),
    substate: FlowSubstateVariantSchema,
    fixtureId: z.string().min(1),
    flowTier: FlowTierSchema,
    clusterId: z.string().min(1),
    severityBand: FlowSeverityBandSchema,
    payloadJson: z.string(),
    updatedAt: z.number().int(),
  })
  .strict();
export type SynthesizedSubstateBinding = z.infer<typeof SynthesizedSubstateBindingSchema>;

export const SynthesizedFlowSummarySchema = z
  .object({
    primaryClusterCount: z.number().int().min(0).max(MAX_CLUSTER_NODES),
    edgeCaseBranchCount: z.number().int().min(0),
    substateCoveragePercent: z.literal(FULL_COVERAGE_PERCENT),
    floatLeakCount: z.literal(ZERO_FLOATS),
    strippedOrphanEdgeCount: z.number().int().min(0),
    rejectedCycleEdgeCount: z.number().int().min(0),
    preservedLoopbackEdgeCount: z.number().int().min(0),
    decisionKind: FlowConfidenceKindSchema,
    highestSeverity: FlowSeverityBandSchema,
    simulatedDimensions: z.array(z.string()),
  })
  .strict();
export type SynthesizedFlowSummary = z.infer<typeof SynthesizedFlowSummarySchema>;

export const SynthesizedFlowPlanSchema = z
  .object({
    projectId: z.string().min(1),
    summary: SynthesizedFlowSummarySchema,
    nodes: z.array(SynthesizedNodeSchema),
    edges: z.array(SynthesizedEdgeSchema),
    substates: z.array(SynthesizedSubstateBindingSchema),
  })
  .strict();
export type SynthesizedFlowPlan = z.infer<typeof SynthesizedFlowPlanSchema>;

export const SynthesizeFlowResponseSchema = z
  .object({
    ok: z.literal(true),
    data: SynthesizedFlowPlanSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type SynthesizeFlowResponse = z.infer<typeof SynthesizeFlowResponseSchema>;

export const StoredFlowSubstateSchema = z
  .object({
    nodeId: z.string().min(1),
    projectId: z.string().min(1),
    substate: FlowSubstateVariantSchema,
    fixtureId: z.string().min(1),
    flowTier: FlowTierSchema,
    clusterId: z.string().min(1),
    severityBand: FlowSeverityBandSchema,
    updatedAt: z.number().int(),
  })
  .strict();
export type StoredFlowSubstate = z.infer<typeof StoredFlowSubstateSchema>;

export const FlowSubstatesQuerySchema = z
  .object({
    projectId: z.string().trim().min(1).max(MAX_PROJECT_ID_LEN),
    flowTier: FlowTierSchema.optional(),
  })
  .strict();
export type FlowSubstatesQuery = z.infer<typeof FlowSubstatesQuerySchema>;

export const FlowSubstatesListResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        projectId: z.string().min(1),
        items: z.array(StoredFlowSubstateSchema),
        totalCount: z.number().int().min(0),
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();
export type FlowSubstatesListResponse = z.infer<typeof FlowSubstatesListResponseSchema>;

export const FlowSynthesizeErrorResponseSchema = z
  .object({
    error: z
      .object({
        code: z.string().min(1),
        message: z.string().min(1),
        field: z.string().optional(),
        requestId: z.string().min(1),
      })
      .strict(),
  })
  .strict();
