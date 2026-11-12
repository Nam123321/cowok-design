/**
 * Canonical entrypoint for @cowok-design/shared-types
 */

export * from "./primitives.js";
export * from "./errors.js";
export * from "./specs.js";
export * from "./constraints.js";
export * from "./generated/index.js";
export type { HarnessError } from "./errors.js";
export { HarnessErrorSchema } from "./errors.js";

// Canonical Deprecated Aliases for backward compatibility
import type { ConstraintProfile } from "./generated/ConstraintProfile.js";
import type { BotJob } from "./generated/BotJob.js";

export type ProjectConstraintProfile = ConstraintProfile;
export type FleetTask = BotJob;
