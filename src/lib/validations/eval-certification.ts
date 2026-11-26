// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Zod Runtime Boundary Contracts for Release Certification & Anti-Goodhart Eval)
import { z } from "zod";

export const EvalTierSchema = z.enum(["pr", "stage", "release"]);
export type EvalTier = z.infer<typeof EvalTierSchema>;

export const EvalVerdictSchema = z.enum(["PASS", "FAIL", "SEALED_EVAL_TAMPER_ERROR"]);
export type EvalVerdict = z.infer<typeof EvalVerdictSchema>;

export const EvalMilestoneIdSchema = z.enum(["M1", "M2", "M3", "M4", "M5"]);
export type EvalMilestoneId = z.infer<typeof EvalMilestoneIdSchema>;

export const SimCaseStatusSchema = z.enum(["PASS", "FAIL", "BLOCKED", "GAP"]);
export type SimCaseStatus = z.infer<typeof SimCaseStatusSchema>;

export const ManifestPinSourceSchema = z.enum(["compiled_binary", "ci_env_override"]);
export type ManifestPinSource = z.infer<typeof ManifestPinSourceSchema>;

export const DecisionProviderIdSchema = z.enum([
  "local_fts5",
  "gemini_decision_jev",
  "deterministic_mcda",
]);
export type DecisionProviderId = z.infer<typeof DecisionProviderIdSchema>;

export const EvalRunRequestSchema = z.object({
  tier: EvalTierSchema,
  projectId: z.string().trim().min(1).max(128).optional(),
  designCraftCandidateDir: z.string().trim().min(1).max(512).optional(),
  runSimulatedUsers: z.boolean().default(true),
});
export type EvalRunRequest = z.input<typeof EvalRunRequestSchema>;

export const EvalHistoryQuerySchema = z.object({
  limit: z.number().int().min(1).max(100).optional(),
  tier: EvalTierSchema.optional(),
  projectId: z.string().trim().min(1).max(128).optional(),
});
export type EvalHistoryQuery = z.infer<typeof EvalHistoryQuerySchema>;

export const SimCaseResultSchema = z.object({
  caseId: z.string().regex(/^UC-(0[1-9]|1[0-9]|2[0-5])$/),
  milestone: EvalMilestoneIdSchema,
  titleKey: z.string(),
  mandatorySecurityGate: z.boolean(),
  securityGateKey: z.string().nullable(),
  status: SimCaseStatusSchema,
  durationMs: z.number().int().nonnegative(),
  evidenceSummary: z.string(),
});
export type SimCaseResult = z.infer<typeof SimCaseResultSchema>;
export const EvalUseCaseResultSchema = SimCaseResultSchema;
export type EvalUseCaseResult = SimCaseResult;

export const MilestoneSummarySchema = z.object({
  milestone: EvalMilestoneIdSchema,
  passed: z.number().int().nonnegative(),
  total: z.number().int().positive(),
  blocked: z.number().int().nonnegative(),
  gap: z.number().int().nonnegative(),
  securityGatePassed: z.boolean(),
  status: z.enum(["PASS", "FAIL"]),
});
export type MilestoneSummary = z.infer<typeof MilestoneSummarySchema>;
export const EvalMilestoneSummarySchema = MilestoneSummarySchema;
export type EvalMilestoneSummary = MilestoneSummary;

export const PairedAblationSummarySchema = z.object({
  briefsEvaluated: z.number().int().nonnegative(),
  dataStorytellingBriefsCount: z.number().int().nonnegative(),
  baselineMeanScore: z.number(),
  withSkillMeanScore: z.number(),
  liftPercent: z.number(),
  regressionsCount: z.number().int().nonnegative(),
  designCraftSemverPassed: z.boolean(),
  designCraftVersion: z.string(),
});
export type PairedAblationSummary = z.infer<typeof PairedAblationSummarySchema>;

export const DecisionProviderCalibrationSchema = z.object({
  providerId: DecisionProviderIdSchema,
  casesEvaluated: z.number().int().nonnegative(),
  accuracyPercent: z.number().min(0).max(100),
  floatLeaksCount: z.number().int().nonnegative(),
  passed: z.boolean(),
});
export type DecisionProviderCalibration = z.infer<typeof DecisionProviderCalibrationSchema>;

export const DecisionCalibrationSummarySchema = z.object({
  providers: z.array(DecisionProviderCalibrationSchema),
  totalFloatLeaks: z.number().int().nonnegative(),
  passed: z.boolean(),
});
export type DecisionCalibrationSummary = z.infer<typeof DecisionCalibrationSummarySchema>;

export const FaultInjectionSummarySchema = z.object({
  injectedCount: z.number().int().nonnegative(),
  caughtCount: z.number().int().nonnegative(),
  catchRatePercent: z.number().min(0).max(100),
  builderScorerIsolated: z.boolean(),
  passed: z.boolean(),
});
export type FaultInjectionSummary = z.infer<typeof FaultInjectionSummarySchema>;

export const DesignCraftGateSummarySchema = z.object({
  designCraftSemverPassed: z.boolean(),
  designCraftVersion: z.string(),
  liftPercent: z.number(),
  regressionsCount: z.number().int().nonnegative(),
});
export type DesignCraftGateSummary = z.infer<typeof DesignCraftGateSummarySchema>;

export const EvalBenchmarkRunDtoSchema = z.object({
  runId: z.string(),
  projectId: z.string().nullable(),
  tier: EvalTierSchema,
  offlineEnforced: z.boolean(),
  sealedManifestSha256: z.string(),
  manifestPinSource: ManifestPinSourceSchema,
  tamperDetected: z.boolean(),
  tamperReason: z.string().nullable(),
  verdict: EvalVerdictSchema,
  durationMs: z.number().int().nonnegative(),
  estimatedCostUsd: z.number().nonnegative(),
  simCasesPassed: z.number().int().nonnegative(),
  simCasesBlocked: z.number().int().nonnegative(),
  simCasesGap: z.number().int().nonnegative(),
  simCasesTotal: z.number().int().positive(),
  securityGatesPassed: z.boolean(),
  securityBlockers: z.array(z.string()),
  milestones: z.array(MilestoneSummarySchema),
  simCases: z.array(SimCaseResultSchema),
  ablation: PairedAblationSummarySchema,
  calibration: DecisionCalibrationSummarySchema,
  faultInjection: FaultInjectionSummarySchema,
  ledgerPath: z.string(),
  triggeredBy: z.string(),
  createdAt: z.number().int().nonnegative(),
});
export type EvalBenchmarkRunDto = z.infer<typeof EvalBenchmarkRunDtoSchema>;
export const EvalBenchmarkRunSchema = EvalBenchmarkRunDtoSchema;
export type EvalBenchmarkRun = EvalBenchmarkRunDto;

export const EvalRunResponseSchema = z.object({
  ok: z.literal(true),
  data: EvalBenchmarkRunDtoSchema,
  requestId: z.string(),
});
export type EvalRunResponse = z.infer<typeof EvalRunResponseSchema>;

export const EvalHistoryDataSchema = z.object({
  sealedIntegrityOk: z.boolean(),
  sealedManifestSha256: z.string(),
  manifestPinSource: ManifestPinSourceSchema,
  tamperReason: z.string().nullable(),
  latestRun: EvalBenchmarkRunDtoSchema.nullable(),
  runs: z.array(EvalBenchmarkRunDtoSchema),
  totalRuns: z.number().int().nonnegative(),
});
export type EvalHistoryData = z.infer<typeof EvalHistoryDataSchema>;

export const EvalHistoryResponseSchema = z.object({
  ok: z.literal(true),
  data: EvalHistoryDataSchema,
  requestId: z.string(),
});
export type EvalHistoryResponse = z.infer<typeof EvalHistoryResponseSchema>;

export const EvalErrorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    field: z.string().optional(),
    requestId: z.string().optional(),
    details: z.record(z.unknown()).optional(),
  }),
});
export type EvalErrorResponse = z.infer<typeof EvalErrorResponseSchema>;
