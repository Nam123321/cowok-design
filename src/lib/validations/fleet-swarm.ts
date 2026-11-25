// @story 10.2
// @implements AC-01, AC-02, AC-03, AC-04 (Strict Zod Contracts for V27 worker_file_leases, Parallel Swarm Dispatch & Independent Builder != Scorer Tier-3 Design Jury)
import { z } from "zod";

const SHA256_HEX_RE = /^[a-fA-F0-9]{64}$/;
const SAFE_ID_RE = /^[A-Za-z0-9._:-]{1,128}$/;

export const WorkerRoleSchema = z.enum(["BUILDER", "VERIFIER_QC", "DESIGN_JURY"]);
export type WorkerRole = z.infer<typeof WorkerRoleSchema>;

export const EvidenceTierSchema = z.enum(["E1", "E2", "E3", "E4"]);
export type EvidenceTier = z.infer<typeof EvidenceTierSchema>;

export const JuryLensSchema = z.enum(["VISUAL", "UX_FLOW", "COPY", "BRAND_IDENTITY"]);
export type JuryLens = z.infer<typeof JuryLensSchema>;

export const JuryVerdictStatusSchema = z.enum(["PASS", "FAIL", "ESCALATED_TO_LEDGER"]);
export type JuryVerdictStatus = z.infer<typeof JuryVerdictStatusSchema>;

const SafeIdentifierSchema = z
  .string()
  .trim()
  .regex(SAFE_ID_RE, "Identifier must match ^[A-Za-z0-9._:-]{1,128}$")
  .refine((val) => !val.includes(".."), "Identifier must not contain '..'");

// ---------------------------------------------------------------------------
// POST /api/v1/fleet/leases/acquire
// ---------------------------------------------------------------------------
export const AcquireWorkerLeaseRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    workerId: SafeIdentifierSchema,
    workerRole: WorkerRoleSchema,
    ownedPaths: z.array(z.string().trim().min(1).max(512)).min(1).max(128),
    ttlMs: z.number().int().min(5_000).max(300_000).default(60_000),
    workerPgid: z.number().int().min(2).max(2_147_483_647).optional(),
  })
  .strict();
export type AcquireWorkerLeaseRequest = z.infer<typeof AcquireWorkerLeaseRequestSchema>;

export const AcquireWorkerLeaseDataSchema = z
  .object({
    leaseId: z.string().min(1),
    projectId: z.string().min(1),
    workerId: z.string().min(1),
    workerRole: WorkerRoleSchema,
    fencingToken: z.number().int().min(1),
    ownedPaths: z.array(z.string().min(1)).min(1),
    expiresAt: z.number().int(),
    createdAt: z.number().int(),
    reapedExpiredCount: z.number().int().min(0),
  })
  .strict();
export type AcquireWorkerLeaseData = z.infer<typeof AcquireWorkerLeaseDataSchema>;

export const AcquireWorkerLeaseResponseSchema = z
  .object({
    ok: z.literal(true),
    data: AcquireWorkerLeaseDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type AcquireWorkerLeaseResponse = z.infer<typeof AcquireWorkerLeaseResponseSchema>;

// ---------------------------------------------------------------------------
// POST /api/v1/fleet/dispatch
// ---------------------------------------------------------------------------
export const SwarmWorkerTaskInputSchema = z
  .object({
    workerId: SafeIdentifierSchema,
    workerRole: WorkerRoleSchema,
    oneJob: z.string().trim().min(1).max(4_096),
    neverRules: z.array(z.string().trim().min(1).max(1_024)).min(1).max(32),
    ownedPaths: z.array(z.string().trim().min(1).max(512)).min(1).max(128),
    ttlMs: z.number().int().min(5_000).max(300_000).default(60_000),
    workerPgid: z.number().int().min(2).max(2_147_483_647).optional(),
  })
  .strict();
export type SwarmWorkerTaskInput = z.infer<typeof SwarmWorkerTaskInputSchema>;

export const DispatchFleetSwarmRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    sessionId: SafeIdentifierSchema,
    tasks: z.array(SwarmWorkerTaskInputSchema).min(1).max(32),
    upstreamHttpStatus: z.number().int().min(100).max(599).optional(),
    retryAfterMs: z.number().int().min(0).max(300_000).optional(),
  })
  .strict();
export type DispatchFleetSwarmRequest = z.infer<typeof DispatchFleetSwarmRequestSchema>;

export const DispatchedTaskAssignmentSchema = z
  .object({
    taskId: z.string().min(1),
    leaseId: z.string().min(1),
    workerId: z.string().min(1),
    workerRole: WorkerRoleSchema,
    fencingToken: z.number().int().min(1),
    ownedPaths: z.array(z.string().min(1)).min(1),
    envelopeRelPath: z.string().min(1),
    dispatchState: z.enum(["DISPATCHED", "QUEUED_BY_CLAMP"]),
    expiresAt: z.number().int(),
  })
  .strict();
export type DispatchedTaskAssignment = z.infer<typeof DispatchedTaskAssignmentSchema>;

export const DispatchFleetSwarmDataSchema = z
  .object({
    dispatchId: z.string().min(1),
    projectId: z.string().min(1),
    sessionId: z.string().min(1),
    concurrencyLimit: z.number().int().min(1).max(3),
    clampedOn429: z.boolean(),
    backoffMs: z.number().int().min(0),
    reapedExpiredCount: z.number().int().min(0),
    assignments: z.array(DispatchedTaskAssignmentSchema).min(1),
  })
  .strict();
export type DispatchFleetSwarmData = z.infer<typeof DispatchFleetSwarmDataSchema>;

export const DispatchFleetSwarmResponseSchema = z
  .object({
    ok: z.literal(true),
    data: DispatchFleetSwarmDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type DispatchFleetSwarmResponse = z.infer<typeof DispatchFleetSwarmResponseSchema>;

// ---------------------------------------------------------------------------
// POST /api/v1/fleet/jury/evaluate
// ---------------------------------------------------------------------------
export const QcIssueInputSchema = z
  .object({
    ruleId: z.string().trim().min(1).max(128),
    tier: EvidenceTierSchema,
    severity: z.enum(["CRITICAL", "HIGH", "MEDIUM", "LOW"]),
    selector: z.string().max(256).nullable().optional(),
    message: z.string().trim().min(1).max(2_048),
  })
  .strict();
export type QcIssueInput = z.infer<typeof QcIssueInputSchema>;

export const ContrastMeasurementSchema = z
  .object({
    selector: z.string().trim().min(1).max(256),
    ratio: z.number().min(0).max(21),
    isLargeText: z.boolean().optional(),
  })
  .strict();
export type ContrastMeasurement = z.infer<typeof ContrastMeasurementSchema>;

export const ElementBoundsSchema = z
  .object({
    selector: z.string().trim().min(1).max(256),
    x: z.number(),
    y: z.number(),
    width: z.number().min(0),
    height: z.number().min(0),
    interactive: z.boolean().optional(),
    clipped: z.boolean().optional(),
    viewportWidth: z.number().positive().optional(),
  })
  .strict();
export type ElementBounds = z.infer<typeof ElementBoundsSchema>;

export const EvaluateDesignJuryRequestSchema = z
  .object({
    projectId: z.string().trim().min(1).max(128),
    screenId: z.string().trim().min(1).max(128),
    builderWorkerId: SafeIdentifierSchema,
    scorerWorkerId: SafeIdentifierSchema,
    scorerRole: WorkerRoleSchema,
    generationFence: z.number().int().min(1),
    expectedGenerationFence: z.number().int().min(1).optional(),
    freezeSnapshotSha256: z.string().regex(SHA256_HEX_RE, "Expected 64-char hex SHA-256"),
    frozenHtml: z.string().max(262_144).optional(),
    qcRunId: z.string().trim().min(1).max(128).optional(),
    reportedIssues: z.array(QcIssueInputSchema).max(256).optional(),
    expectedDefectIds: z.array(z.string().trim().min(1).max(128)).max(256).optional(),
    contrastRatios: z.array(ContrastMeasurementSchema).max(256).optional(),
    boundingBoxes: z.array(ElementBoundsSchema).max(256).optional(),
    brandTokensExpected: z.array(z.string().trim().min(1).max(128)).max(128).optional(),
  })
  .strict();
export type EvaluateDesignJuryRequest = z.infer<typeof EvaluateDesignJuryRequestSchema>;

export const JuryLensScoreSchema = z
  .object({
    lens: JuryLensSchema,
    score: z.number().min(0).max(10),
    passed: z.boolean(),
    findings: z.array(z.string()),
  })
  .strict();
export type JuryLensScore = z.infer<typeof JuryLensScoreSchema>;

export const QcViolationItemSchema = z
  .object({
    ruleId: z.string().min(1),
    tier: EvidenceTierSchema,
    severity: z.string().min(1),
    selector: z.string().nullable(),
    message: z.string().min(1),
    fingerprintSha256: z.string().regex(SHA256_HEX_RE),
    recurrenceCount: z.number().int().min(1),
  })
  .strict();
export type QcViolationItem = z.infer<typeof QcViolationItemSchema>;

export const EvaluateDesignJuryDataSchema = z
  .object({
    verdictId: z.string().min(1),
    projectId: z.string().min(1),
    screenId: z.string().min(1),
    builderWorkerId: z.string().min(1),
    scorerWorkerId: z.string().min(1),
    generationFence: z.number().int().min(1),
    freezeSnapshotSha256: z.string().regex(SHA256_HEX_RE),
    qcRunId: z.string().nullable(),
    e1E4CatchRate: z.number().min(0).max(1),
    verdict: JuryVerdictStatusSchema,
    lensScores: z.array(JuryLensScoreSchema).length(4),
    violations: z.array(QcViolationItemSchema),
    recommendedFixes: z.array(z.string()),
    escalatedToLedger: z.boolean(),
    ledgerEntryId: z.string().nullable(),
    approvalPolicy: z.string().nullable(),
  })
  .strict();
export type EvaluateDesignJuryData = z.infer<typeof EvaluateDesignJuryDataSchema>;

export const EvaluateDesignJuryResponseSchema = z
  .object({
    ok: z.literal(true),
    data: EvaluateDesignJuryDataSchema,
    requestId: z.string().min(1),
  })
  .strict();
export type EvaluateDesignJuryResponse = z.infer<typeof EvaluateDesignJuryResponseSchema>;

export const FleetSwarmErrorResponseSchema = z
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
export type FleetSwarmErrorResponse = z.infer<typeof FleetSwarmErrorResponseSchema>;
