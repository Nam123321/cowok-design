// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Strict Zod Contracts for V31 Anti-Slop Design Tournament, Cross-Variant Borrow, /crystallize & advisor 6-Step GEPA Loop)
import { z } from "zod";

const SHA256_HEX_RE = /^[a-fA-F0-9]{64}$/;
const SAFE_ID_RE = /^[A-Za-z0-9._-]{1,128}$/;

export const DesignAxisSchema = z.enum([
  "INFORMATION_DENSITY",
  "NAVIGATION_TOPOLOGY",
  "VISUAL_METAPHOR",
  "INTERACTION_RHYTHM",
]);
export type DesignAxis = z.infer<typeof DesignAxisSchema>;

export const CrystallizePackageKindSchema = z.enum([
  "STARTER_TEMPLATE",
  "BRAND_KIT",
  "SKILL_PLUGIN",
]);
export type CrystallizePackageKind = z.infer<typeof CrystallizePackageKindSchema>;

export const AdvisorStepKindSchema = z.enum([
  "1_SIGNAL_COLLECT",
  "2_ROOT_CAUSE",
  "3_PATCH_SKILL_DRAFT",
  "4_SEALED_EVAL_PRECHECK",
  "5_REGRESSION_CHECK",
  "6_STAGE_DESIGN_CRAFT_PR",
]);
export type AdvisorStepKind = z.infer<typeof AdvisorStepKindSchema>;

const SafeTournamentIdSchema = z
  .string()
  .trim()
  .regex(SAFE_ID_RE, "Identifier must match ^[A-Za-z0-9._-]{1,128}$")
  .refine((val) => val !== "." && !val.includes(".."), "Identifier must not be '.' or contain '..'");

// ---------------------------------------------------------------------------
// POST /api/v1/tournament/start
// ---------------------------------------------------------------------------
export const VariantBranchOverrideSchema = z
  .object({
    axis: DesignAxisSchema,
    initialHtml: z.string().max(262_144).optional(),
    rerollHtmls: z.array(z.string().max(262_144)).max(2).optional(),
    simulatedLatencyMs: z.number().int().min(0).max(60_000).optional(),
  })
  .strict();

export const StartTournamentRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    prompt: z.string().trim().min(1).max(4_096),
    axes: z.array(DesignAxisSchema).min(2).max(4).optional(),
    budgetMs: z.number().int().min(500).max(15_000).optional(),
    branchOverrides: z.array(VariantBranchOverrideSchema).max(4).optional(),
    rubricCriteria: z.array(z.string().trim().min(1).max(160)).max(12).optional(),
  })
  .strict();
export type StartTournamentRequest = z.infer<typeof StartTournamentRequestSchema>;

export const GeneratedVariantSchema = z
  .object({
    id: SafeTournamentIdSchema,
    displayLabel: z.string().min(1).max(64),
    axis: DesignAxisSchema,
    axisProfile: z.string().min(1),
    leasedPath: z.string().min(1),
    html: z.string().min(1),
    artifactSha256: z.string().regex(SHA256_HEX_RE),
    astSimilarity: z.number().min(0).max(1),
    cloneDistancePassed: z.boolean(),
    lowDiversityWarning: z.boolean(),
    rerollAttempts: z.number().int().min(0).max(2),
    rubricVerdict: z.string().min(1),
    rubricWins: z.number().int().min(0),
    deScoreBps: z.number().int().min(0).max(10_000),
    elapsedMs: z.number().int().min(0),
    archived: z.boolean(),
  })
  .strict();
export type GeneratedVariant = z.infer<typeof GeneratedVariantSchema>;

export const RubricPairwiseOutcomeSchema = z
  .object({
    winnerId: z.string().min(1),
    winnerLabel: z.string().min(1),
    forwardWinnerId: z.string().min(1),
    swappedWinnerId: z.string().min(1),
    swapConsistent: z.boolean(),
    tieBreakApplied: z.boolean(),
    tieBreakReason: z.string().nullable().optional(),
  })
  .strict();

export const PairwiseBlindComparisonSchema = z
  .object({
    variantAId: z.string().min(1),
    variantALabel: z.string().min(1),
    variantBId: z.string().min(1),
    variantBLabel: z.string().min(1),
    outcome: RubricPairwiseOutcomeSchema,
  })
  .strict();

export const BlindRubricSummarySchema = z
  .object({
    recommendedVariantId: z.string().min(1),
    recommendedDisplayLabel: z.string().min(1),
    allPairsSwapConsistent: z.boolean(),
    pairwiseComparisons: z.array(PairwiseBlindComparisonSchema),
  })
  .strict();
export type BlindRubricSummary = z.infer<typeof BlindRubricSummarySchema>;

export const StartTournamentDataSchema = z
  .object({
    runId: SafeTournamentIdSchema,
    projectId: z.string().min(1),
    prompt: z.string().min(1),
    axes: z.array(DesignAxisSchema).min(2).max(4),
    generationEpoch: z.number().int().min(1),
    budgetMs: z.number().int().min(1),
    elapsedMs: z.number().int().min(0),
    degradedMinVariants: z.boolean(),
    timedOutAxes: z.array(DesignAxisSchema),
    lowDiversityWarning: z.boolean(),
    variants: z.array(GeneratedVariantSchema).min(2).max(4),
    rubricSummary: BlindRubricSummarySchema,
    createdAt: z.number().int(),
  })
  .strict();
export type StartTournamentData = z.infer<typeof StartTournamentDataSchema>;

export const StartTournamentResponseSchema = z
  .object({
    ok: z.literal(true),
    data: StartTournamentDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type StartTournamentResponse = z.infer<typeof StartTournamentResponseSchema>;

// ---------------------------------------------------------------------------
// POST /api/v1/tournament/select
// ---------------------------------------------------------------------------
export const SelectTournamentVariantRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    runId: SafeTournamentIdSchema,
    winningVariantId: SafeTournamentIdSchema,
    generationEpoch: z.number().int().min(1),
    borrowFromVariantId: SafeTournamentIdSchema.optional(),
    borrowSelectorOrInstruction: z.string().trim().max(400).optional(),
  })
  .strict();
export type SelectTournamentVariantRequest = z.infer<typeof SelectTournamentVariantRequestSchema>;

export const SelectTournamentVariantDataSchema = z
  .object({
    runId: SafeTournamentIdSchema,
    projectId: z.string().min(1),
    winningVariantId: SafeTournamentIdSchema,
    generationEpoch: z.number().int().min(1),
    mergedHtml: z.string().min(1),
    mergedSha256: z.string().regex(SHA256_HEX_RE),
    borrowedFromVariantId: z.string().nullable(),
    borrowedFfId: z.string().nullable(),
    extractedCssRulesCount: z.number().int().min(0),
    reconciledTotalFfIds: z.number().int().min(0),
    duplicateFfIdsAfterMerge: z.number().int().min(0),
    archivedVariantIds: z.array(z.string()),
    memoryEventIds: z.array(z.string()).min(1),
  })
  .strict();
export type SelectTournamentVariantData = z.infer<typeof SelectTournamentVariantDataSchema>;

export const SelectTournamentVariantResponseSchema = z
  .object({
    ok: z.literal(true),
    data: SelectTournamentVariantDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type SelectTournamentVariantResponse = z.infer<typeof SelectTournamentVariantResponseSchema>;

// ---------------------------------------------------------------------------
// GET /api/v1/tournament/runs
// ---------------------------------------------------------------------------
export const PersistedVariantRowSchema = z
  .object({
    id: z.string().min(1),
    displayLabel: z.string().min(1),
    axisProfile: z.string().min(1),
    artifactSha256: z.string().regex(SHA256_HEX_RE),
    astSimilarity: z.number().min(0).max(1),
    cloneDistancePassed: z.boolean(),
    rubricVerdict: z.string().nullable(),
    archived: z.boolean(),
  })
  .strict();

export const PersistedTournamentRunSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    axes: z.array(z.string()),
    winningVariantId: z.string().nullable(),
    borrowedFromVariantId: z.string().nullable(),
    borrowedFfId: z.string().nullable(),
    generationEpoch: z.number().int().min(1),
    createdAt: z.number().int(),
    variants: z.array(PersistedVariantRowSchema),
  })
  .strict();
export type PersistedTournamentRun = z.infer<typeof PersistedTournamentRunSchema>;

export const ListTournamentRunsResponseSchema = z
  .object({
    ok: z.literal(true),
    data: z
      .object({
        projectId: z.string().min(1),
        runs: z.array(PersistedTournamentRunSchema),
      })
      .strict(),
    requestId: z.string().min(1),
  })
  .strict();
export type ListTournamentRunsResponse = z.infer<typeof ListTournamentRunsResponseSchema>;

// ---------------------------------------------------------------------------
// POST /api/v1/crystallize
// ---------------------------------------------------------------------------
export const CrystallizeRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    runId: SafeTournamentIdSchema.optional(),
    variantId: SafeTournamentIdSchema.optional(),
    packageKind: CrystallizePackageKindSchema,
    name: z.string().trim().min(1).max(160),
    description: z.string().trim().max(1_000).optional(),
    htmlSnapshot: z.string().max(262_144).optional(),
  })
  .strict();
export type CrystallizeRequest = z.infer<typeof CrystallizeRequestSchema>;

export const CrystallizeDataSchema = z
  .object({
    packageId: z.string().min(1),
    projectId: z.string().min(1),
    packageKind: CrystallizePackageKindSchema,
    name: z.string().min(1),
    artifactRelPath: z.string().min(1),
    artifactSha256: z.string().regex(SHA256_HEX_RE),
    memoryEventId: z.string().min(1),
    createdAt: z.number().int(),
  })
  .strict();
export type CrystallizeData = z.infer<typeof CrystallizeDataSchema>;

export const CrystallizeResponseSchema = z
  .object({
    ok: z.literal(true),
    data: CrystallizeDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type CrystallizeResponse = z.infer<typeof CrystallizeResponseSchema>;

// ---------------------------------------------------------------------------
// POST /api/v1/advisor/evolve
// ---------------------------------------------------------------------------
export const AdvisorStepReceiptSchema = z
  .object({
    step: AdvisorStepKindSchema,
    passed: z.boolean(),
    summary: z.string().min(1),
  })
  .strict();

export const AdvisorEvolveRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    targetSkillPath: z.string().trim().min(1).max(512),
    referencePaths: z.array(z.string().trim().min(1).max(512)).max(16).optional(),
    promoteDirectly: z.boolean().optional(),
    targetChannel: z.string().trim().min(1).max(64).optional(),
    minStudioCorrectionHits: z.number().int().min(3).max(100).optional(),
  })
  .strict();
export type AdvisorEvolveRequest = z.infer<typeof AdvisorEvolveRequestSchema>;

export const AdvisorEvolveDataSchema = z
  .object({
    patchId: z.string().min(1),
    projectId: z.string().min(1),
    targetSkillPath: z.string().min(1),
    stagingStatus: z.string().min(1),
    approvalPolicy: z.string().min(1),
    promotedToProduction: z.boolean(),
    sealedEvalUntouched: z.boolean(),
    awaitingStory131BlindEval: z.boolean(),
    regressionPassed: z.boolean(),
    constraintsPreserved: z.boolean(),
    signalsConsumed: z.number().int().min(0),
    studioCorrectionsConsumed: z.number().int().min(0),
    draftPatchRelPath: z.string().min(1),
    draftPatchMarkdown: z.string().min(1),
    steps: z.array(AdvisorStepReceiptSchema).length(6),
    memoryEventId: z.string().min(1),
    createdAt: z.number().int(),
  })
  .strict();
export type AdvisorEvolveData = z.infer<typeof AdvisorEvolveDataSchema>;

export const AdvisorEvolveResponseSchema = z
  .object({
    ok: z.literal(true),
    data: AdvisorEvolveDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type AdvisorEvolveResponse = z.infer<typeof AdvisorEvolveResponseSchema>;

export const TournamentErrorResponseSchema = z
  .object({
    error: z
      .object({
        code: z.string().min(1),
        message: z.string().min(1),
        field: z.string().optional(),
        requestId: z.string().min(1),
        details: z.record(z.unknown()).optional(),
      })
      .strict(),
  })
  .strict();
export type TournamentErrorResponse = z.infer<typeof TournamentErrorResponseSchema>;
