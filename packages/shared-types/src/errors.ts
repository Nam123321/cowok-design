import { z } from "zod";
import { MAX_SAFE_INTEGER, MIN_SAFE_INTEGER } from "./primitives.js";

export const HarnessErrorSchema = z.discriminatedUnion("kind", [
  z
    .object({
      kind: z.literal("INVALID_SPEC_KEY"),
      payload: z.object({ key: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("STALE_FILE"),
      payload: z.object({ path: z.string(), expected_hash: z.string(), actual_hash: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("PROJECT_NOT_FOUND"),
      payload: z.object({ project_id: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("PATH_TRAVERSAL_DENIED"),
      payload: z.object({ path: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("SQLITE_BUSY"),
      payload: z.object({ details: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("UNSAFE_INTEGER"),
      payload: z
        .object({
          value: z
            .string()
            .regex(/^-?\d+$/, "UNSAFE_INTEGER value must be a lossless base-10 integer string")
            .refine((val) => {
              const bi = BigInt(val);
              const i64Min = -9_223_372_036_854_775_808n;
              const i64Max = 9_223_372_036_854_775_807n;
              const safeMin = BigInt(MIN_SAFE_INTEGER);
              const safeMax = BigInt(MAX_SAFE_INTEGER);
              return bi >= i64Min && bi <= i64Max && (bi < safeMin || bi > safeMax);
            }, "UNSAFE_INTEGER value must be a valid i64 outside JavaScript safe integer bounds"),
        })
        .strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("VALIDATION_FAILED"),
      payload: z.object({ reason: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("BOUNDARY_VIOLATION"),
      payload: z.object({ reason: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("PROMPT_BUDGET_EXCEEDED"),
      payload: z
        .object({
          tokens: z.number().int().nonnegative(),
          ceiling: z.number().int().nonnegative(),
        })
        .strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("INVALID_PROMPT_PAYLOAD"),
      payload: z.object({ details: z.string() }).strict(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("MODEL_ADAPTATION_FAILED"),
      payload: z.object({ reason: z.string() }).strict(),
    })
    .strict(),
]);

export type HarnessError = z.infer<typeof HarnessErrorSchema>;

export const HttpErrorBodySchema = z
  .object({
    code: z.string().min(1),
    kind: z.string().optional(),
    message: z.string(),
    details: z.unknown(),
    request_id: z.string(),
  })
  .strict();

export type HttpErrorBody = z.infer<typeof HttpErrorBodySchema>;
