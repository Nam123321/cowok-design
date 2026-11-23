// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (Strict Zod Contracts for Decision Cascade Route & V21 Audit Log — 0% Float Leak)
import { z } from "zod";

const SHA256_HEX_RE = /^[a-fA-F0-9]{64}$/;
const MAX_PROMPT_LEN = 16_384;
const MAX_CONTEXT_LEN = 131_072;
const MAX_VERDICT_LABEL_LEN = 128;
const MAX_CANDIDATE_ID_LEN = 128;
const MAX_CANDIDATE_TEXT_LEN = 8_192;
const MAX_DE_SCORE_BPS = 10_000;
const MAX_RUBRIC_CRITERION_LEN = 256;
const MAX_RUBRIC_CRITERIA = 16;
const MAX_DECISION_ATTEMPTS = 3;
const MAX_PRIOR_VERDICTS = 16;
const MAX_CANDIDATES = 32;
const DEFAULT_AUDIT_LIMIT = 50;
const MAX_AUDIT_LIMIT = 200;

export const CascadeStageSchema = z.enum(["DECISION_1", "DECISION_2", "RUBRIC_SHIELD"]);
export type CascadeStage = z.infer<typeof CascadeStageSchema>;

export const DecisionTierSchema = z.enum([
  "LOCAL_FTS5_BM25",
  "GEMINI_DECISION_1",
  "TYPESAFE_JEV",
  "HEURISTIC_FALLBACK",
]);
export type DecisionTier = z.infer<typeof DecisionTierSchema>;

export const ConfidenceKindSchema = z.enum([
  "ExactMatch",
  "ModelVerdict",
  "DeterministicFallback",
]);
export type ConfidenceKind = z.infer<typeof ConfidenceKindSchema>;

export const PipelineStageNameSchema = z.enum([
  "DE_PRE_FILTER",
  "DECISION_1",
  "DR_DE_EXECUTION",
  "DE_POST_CHECK",
  "DECISION_2",
  "RUBRIC_SHIELD",
  "ESCALATE_HUMAN_ADVISOR",
]);
export type PipelineStageName = z.infer<typeof PipelineStageNameSchema>;

export const DecisionCandidateSchema = z
  .object({
    id: z.string().trim().min(1).max(MAX_CANDIDATE_ID_LEN),
    label: z.string().trim().min(1).max(MAX_VERDICT_LABEL_LEN),
    text: z.string().max(MAX_CANDIDATE_TEXT_LEN),
    deScoreBps: z.number().int().min(0).max(MAX_DE_SCORE_BPS).default(0),
  })
  .strict();
export type DecisionCandidate = z.infer<typeof DecisionCandidateSchema>;

export const RubricPairwiseInputSchema = z
  .object({
    candidateX: DecisionCandidateSchema,
    candidateY: DecisionCandidateSchema,
    rubricCriteria: z
      .array(z.string().trim().min(1).max(MAX_RUBRIC_CRITERION_LEN))
      .max(MAX_RUBRIC_CRITERIA)
      .default([]),
  })
  .strict()
  .refine((pair) => pair.candidateX.id !== pair.candidateY.id, {
    message: "candidateX.id and candidateY.id must be distinct",
    path: ["candidateY", "id"],
  });
export type RubricPairwiseInput = z.infer<typeof RubricPairwiseInputSchema>;

export const RubricPairwiseOutcomeSchema = z
  .object({
    winnerId: z.string().min(1),
    winnerLabel: z.string().min(1),
    forwardWinnerId: z.string().min(1),
    swappedWinnerId: z.string().min(1),
    swapConsistent: z.boolean(),
    tieBreakApplied: z.boolean(),
    tieBreakReason: z.string().optional(),
  })
  .strict();
export type RubricPairwiseOutcome = z.infer<typeof RubricPairwiseOutcomeSchema>;

export const StageTraceEntrySchema = z
  .object({
    stage: PipelineStageNameSchema,
    passed: z.boolean(),
    summary: z.string(),
    durationMs: z.number().int().min(0),
  })
  .strict();
export type StageTraceEntry = z.infer<typeof StageTraceEntrySchema>;

export const DecisionRouteRequestSchema = z
  .object({
    projectId: z.string().uuid(),
    stage: CascadeStageSchema,
    prompt: z.string().max(MAX_PROMPT_LEN).default(""),
    context: z.string().max(MAX_CONTEXT_LEN).default(""),
    untrustedArtifactSha256: z.string().regex(SHA256_HEX_RE, "Expected 64-char hex SHA-256"),
    attemptNumber: z.number().int().min(1).max(MAX_DECISION_ATTEMPTS).default(1),
    priorVerdicts: z
      .array(z.string().trim().min(1).max(MAX_VERDICT_LABEL_LEN))
      .max(MAX_PRIOR_VERDICTS)
      .default([]),
    staleFileSha256: z.string().regex(SHA256_HEX_RE, "Expected 64-char hex SHA-256").optional(),
    currentFileSha256: z.string().regex(SHA256_HEX_RE, "Expected 64-char hex SHA-256").optional(),
    candidates: z.array(DecisionCandidateSchema).max(MAX_CANDIDATES).default([]),
    rubricPair: RubricPairwiseInputSchema.optional(),
    preferExternalTier: z.boolean().default(false),
  })
  .strict();
export type DecisionRouteRequest = z.infer<typeof DecisionRouteRequestSchema>;

export const VerdictShieldOutputSchema = z
  .object({
    verdictLabel: z.string().min(1),
    confidenceKind: ConfidenceKindSchema,
    tierUsed: DecisionTierSchema,
    cascadeStage: CascadeStageSchema,
    attemptNumber: z.number().int().min(1).max(MAX_DECISION_ATTEMPTS),
    breakerTripped: z.boolean(),
    escalatedToAdvisor: z.boolean(),
    rubricSwapConsistent: z.boolean().optional(),
    rubricOutcome: RubricPairwiseOutcomeSchema.optional(),
    egressGuarded: z.boolean(),
    stageTrace: z.array(StageTraceEntrySchema),
    latencyMs: z.number().int().min(0),
  })
  .strict();
export type VerdictShieldOutput = z.infer<typeof VerdictShieldOutputSchema>;

export const DecisionRouteResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        auditId: z.string().min(1),
        output: VerdictShieldOutputSchema,
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();
export type DecisionRouteResponse = z.infer<typeof DecisionRouteResponseSchema>;

export const DecisionAuditQuerySchema = z
  .object({
    projectId: z.string().uuid(),
    limit: z.coerce.number().int().min(1).max(MAX_AUDIT_LIMIT).default(DEFAULT_AUDIT_LIMIT),
    stage: CascadeStageSchema.optional(),
  })
  .strict();
export type DecisionAuditQuery = z.infer<typeof DecisionAuditQuerySchema>;

export const DecisionAuditRecordSchema = z
  .object({
    id: z.string().min(1),
    tenantId: z.string().min(1),
    projectId: z.string().min(1),
    cascadeStage: CascadeStageSchema,
    tierUsed: DecisionTierSchema,
    confidenceKind: ConfidenceKindSchema,
    verdictLabel: z.string().min(1),
    rubricSwapConsistent: z.boolean().optional(),
    attemptNumber: z.number().int().min(1).max(MAX_DECISION_ATTEMPTS),
    breakerTripped: z.boolean(),
    staleFileSha256: z.string().optional(),
    latencyMs: z.number().int().min(0),
    createdAt: z.number().int(),
  })
  .strict();
export type DecisionAuditRecord = z.infer<typeof DecisionAuditRecordSchema>;

export const DecisionAuditResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        projectId: z.string().min(1),
        items: z.array(DecisionAuditRecordSchema),
        totalCount: z.number().int().min(0),
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();
export type DecisionAuditResponse = z.infer<typeof DecisionAuditResponseSchema>;

export const DecisionErrorCodeSchema = z.enum([
  "STALE_FILE_OCC",
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "FORBIDDEN",
  "AUTH_FORBIDDEN",
  "PROJECT_NOT_FOUND",
  "PROJECT_TENANT_MISMATCH",
  "PAYLOAD_TOO_LARGE",
  "STORAGE_BUSY",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
]);

export const DecisionErrorResponseSchema = z
  .object({
    error: z
      .object({
        code: DecisionErrorCodeSchema,
        message: z.string().min(1),
        field: z.string().optional(),
        requestId: z.string().min(1),
        details: z.record(z.unknown()).optional(),
      })
      .strict(),
  })
  .strict();
export type DecisionErrorResponse = z.infer<typeof DecisionErrorResponseSchema>;
