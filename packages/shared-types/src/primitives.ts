import { z } from "zod";

export const MAX_SAFE_INTEGER = 9_007_199_254_740_991;
export const MIN_SAFE_INTEGER = -9_007_199_254_740_991;

export const SafeJsIntegerSchema = z
  .number()
  .int("Must be an integer")
  .min(MIN_SAFE_INTEGER, `Value must be >= ${MIN_SAFE_INTEGER}`)
  .max(MAX_SAFE_INTEGER, `Value must be <= ${MAX_SAFE_INTEGER}`);

export const U32Schema = z
  .number()
  .int("Must be an integer")
  .min(0, "u32 cannot be negative")
  .max(4_294_967_295, "Exceeds u32::MAX (4294967295)");
