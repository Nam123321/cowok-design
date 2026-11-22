// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";

const MAX_CONTEXT_BODY_BYTES = 262_144;
const MAX_INTENT_CODE_POINTS = 2_000;
const MAX_REQUESTED_IMAGE_TOKENS = 10_000;
const MAX_OUTLINE_BPE_TOKENS = 350;
const MAX_TEXT_BPE_TOKENS = 1_500;
const MIN_SSOT_DOM_RATIO = 0.6;
const MAX_SOURCE_IDS = 100;
const DEFAULT_TELEMETRY_LIMIT = 25;
const MAX_TELEMETRY_LIMIT = 100;
const MAX_CURSOR_LEN = 512;
const MAX_DOM_OUTLINE_NODES = 128;
const MAX_DOM_FF_ID_LEN = 128;
const MAX_DOM_TAG_LEN = 32;
const MAX_DOM_ROLE_LEN = 64;
const MAX_DOM_SUMMARY_LEN = 240;

export const CONTEXT_LIMITS = {
  MAX_CONTEXT_BODY_BYTES,
  MAX_INTENT_CODE_POINTS,
  MAX_REQUESTED_IMAGE_TOKENS,
  MAX_OUTLINE_BPE_TOKENS,
  MAX_TEXT_BPE_TOKENS,
  MIN_SSOT_DOM_RATIO,
  MAX_SOURCE_IDS,
  DEFAULT_TELEMETRY_LIMIT,
  MAX_TELEMETRY_LIMIT,
  MAX_CURSOR_LEN,
  MAX_DOM_OUTLINE_NODES,
  MAX_DOM_FF_ID_LEN,
  MAX_DOM_TAG_LEN,
  MAX_DOM_ROLE_LEN,
  MAX_DOM_SUMMARY_LEN,
} as const;

const UUID_V4_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isUuidV4(val: string): boolean {
  return UUID_V4_RE.test(val.trim());
}

export const ContextApiErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "FORBIDDEN",
  "INVALID_IDEMPOTENCY_KEY",
  "INVALID_CURSOR",
  "PROJECT_TENANT_MISMATCH",
  "VISIBILITY_CLASS_UNRESOLVED",
  "CROSS_PORTAL_LEAK_BLOCKED",
  "IMAGE_TOKEN_BUDGET_EXCEEDED",
  "ATOMIC_BLOCK_OVER_BUDGET",
  "SSOT_DOM_RATIO_BELOW_MINIMUM",
  "TOKENIZER_UNAVAILABLE",
  "IDEMPOTENCY_KEY_REUSED",
  "TELEMETRY_CONFLICT",
  "TELEMETRY_PROJECT_REFERENCE_INVALID",
  "TELEMETRY_CONTRACT_VIOLATION",
  "TELEMETRY_WRITE_BUSY",
  "TELEMETRY_STORAGE_UNAVAILABLE",
  "DAEMON_CONTRACT_VIOLATION",
]);
export type ContextApiErrorCode = z.infer<typeof ContextApiErrorCodeSchema>;

export const ContextApiErrorSchema = z
  .object({
    ok: z.literal(false),
    error: z
      .object({
        code: ContextApiErrorCodeSchema,
        message: z.string().min(1),
        field: z.string().min(1).optional(),
        retryable: z.boolean(),
      })
      .strict(),
    requestId: z.string().uuid(),
  })
  .strict();
export type ContextApiError = z.infer<typeof ContextApiErrorSchema>;
