// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (Zod Contracts for Bot Fleet Execute & Live Provider Status)
import { z } from "zod";

const MAX_SESSION_ID_LEN = 128;
const MAX_NODE_ID_LEN = 128;
const MAX_PROVIDER_ID_LEN = 64;
const MAX_MODEL_ID_LEN = 128;
const MAX_PROMPT_LEN = 120_000;
const MIN_TIMEOUT_MS = 500;
const MAX_TIMEOUT_MS = 120_000;
const DEFAULT_TIMEOUT_MS = 30_000;
const SAFE_SESSION_ID_RE = /^[A-Za-z0-9._-]+$/;
const SAFE_MODEL_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._:@/-]{0,127}$/;

export const BotFleetModeSchema = z.enum(["byos", "byok", "auto"]);
export type BotFleetMode = z.infer<typeof BotFleetModeSchema>;

export const BotFleetExecuteRequestSchema = z
  .object({
    projectId: z.string().uuid(),
    sessionId: z
      .string()
      .trim()
      .min(1)
      .max(MAX_SESSION_ID_LEN)
      .regex(SAFE_SESSION_ID_RE, "sessionId must contain only [A-Za-z0-9._-]")
      .refine((v) => !v.includes(".."), "sessionId must not contain '..'"),
    nodeId: z.string().trim().min(1).max(MAX_NODE_ID_LEN).optional(),
    mode: BotFleetModeSchema.default("auto"),
    providerId: z.string().trim().min(1).max(MAX_PROVIDER_ID_LEN).optional(),
    adapterId: z.string().trim().min(1).max(MAX_PROVIDER_ID_LEN).optional(),
    modelId: z
      .string()
      .trim()
      .min(1)
      .max(MAX_MODEL_ID_LEN)
      .regex(SAFE_MODEL_ID_RE, "modelId contains unsupported characters")
      .refine((v) => !v.includes(".."), "modelId must not contain '..'")
      .optional(),
    prompt: z.string().trim().min(1).max(MAX_PROMPT_LEN),
    timeoutMs: z
      .number()
      .int()
      .min(MIN_TIMEOUT_MS)
      .max(MAX_TIMEOUT_MS)
      .default(DEFAULT_TIMEOUT_MS),
  })
  .strict();

export type BotFleetExecuteRequest = z.infer<typeof BotFleetExecuteRequestSchema>;

export const BotFleetExecuteDataSchema = z
  .object({
    executionId: z.string().min(1),
    mode: z.enum(["byos", "byok"]),
    adapterId: z.string().nullable(),
    providerId: z.string().nullable(),
    modelId: z.string().nullable(),
    normalizedContent: z.string(),
    extractedJson: z.unknown().nullable().optional(),
    ansiSequencesStripped: z.number().int().min(0),
    jsonBlocksExtracted: z.number().int().min(0),
    svgViewboxesInjected: z.number().int().min(0),
    cooldownApplied: z.boolean(),
    failoverProviders: z.array(z.string()),
    pgidReaped: z.boolean(),
    timedOut: z.boolean(),
    durationMs: z.number().int().min(0),
    executedAt: z.number().int().min(0),
  })
  .strict();

export type BotFleetExecuteData = z.infer<typeof BotFleetExecuteDataSchema>;

export const BotFleetExecuteResponseSchema = z
  .object({
    ok: z.literal(true),
    data: BotFleetExecuteDataSchema,
    requestId: z.string().min(1),
  })
  .strict();

export type BotFleetExecuteResponse = z.infer<typeof BotFleetExecuteResponseSchema>;

export const CliAdapterStatusSchema = z
  .object({
    id: z.string().min(1),
    name: z.string().min(1),
    command: z.string().min(1),
    category: z.string().min(1),
    promptTransport: z.string().min(1),
    installed: z.boolean(),
    authenticated: z.boolean(),
    sandboxReady: z.boolean(),
    supportsMcpConfig: z.boolean(),
    defaultModel: z.string().min(1),
    status: z.string().min(1),
    authReason: z.string().optional(),
  })
  .strict();

export type CliAdapterStatus = z.infer<typeof CliAdapterStatusSchema>;

export const ByokProviderCooldownSchema = z
  .object({
    providerId: z.string().min(1),
    configured: z.boolean(),
    inCooldown: z.boolean(),
    retryAfterSecs: z.number().int().min(0),
    cooldownUntilMs: z.number().int().optional(),
    consecutive429s: z.number().int().min(0),
    lastError: z.string().optional(),
  })
  .strict();

export type ByokProviderCooldown = z.infer<typeof ByokProviderCooldownSchema>;

export const BotFleetProvidersStatusDataSchema = z
  .object({
    sandboxProtocol: z.string().min(1),
    sessionDirIsolated: z.boolean(),
    adapterCount: z.number().int().min(0),
    readyAdapterCount: z.number().int().min(0),
    adapters: z.array(CliAdapterStatusSchema),
    byokProviders: z.array(ByokProviderCooldownSchema),
    checkedAt: z.string().min(1),
  })
  .strict();

export type BotFleetProvidersStatusData = z.infer<typeof BotFleetProvidersStatusDataSchema>;

export const BotFleetProvidersStatusResponseSchema = z
  .object({
    ok: z.literal(true),
    data: BotFleetProvidersStatusDataSchema,
    requestId: z.string().min(1),
  })
  .strict();

export type BotFleetProvidersStatusResponse = z.infer<typeof BotFleetProvidersStatusResponseSchema>;
