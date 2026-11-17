// @story 08.3
// @implements AC-02, AC-03, AC-05, AC-06
import { z } from "zod";
import type {
  ConsumedNonceRecord as RustConsumedNonceRecord,
  DomainStorageAction as RustDomainStorageAction,
  ManifestUpdateOutcome as RustManifestUpdateOutcome,
  StorageConstraintError as RustStorageConstraintError,
  StorageConstraintTarget as RustStorageConstraintTarget,
} from "@cowok-design/shared-types";

export type ManifestUpdateOutcome = RustManifestUpdateOutcome;
export type ConsumedNonceRecord = RustConsumedNonceRecord;
export type DomainStorageAction = RustDomainStorageAction;

export type PluginLifecycleErrorCode =
  | "UnknownPluginState"
  | "InvalidTransition"
  | "QuarantinedReactivationForbidden"
  | "RevisionConflict"
  | "PendingStagingMustBeResolved"
  | "NonActiveBreakingUpdateRejected"
  | "NoStagedCandidate"
  | "StagingPromotionRejected"
  | "InvalidQuarantineReason"
  | "InvalidManifestField"
  | "InvalidPersistedState"
  | "InvalidSemVer";

/** Typed lifecycle error carrying a machine-readable discriminant code for callers. */
export class PluginLifecycleError extends Error {
  public readonly code: PluginLifecycleErrorCode;
  public readonly meta?: Record<string, unknown>;
  constructor(code: PluginLifecycleErrorCode, message: string, meta?: Record<string, unknown>) {
    super(`${code}: ${message}`);
    this.name = "PluginLifecycleError";
    this.code = code;
    this.meta = meta;
  }
}

export type LearnErrorCode =
  | "InvalidCardInput"
  | "PromptInjectionBlocked"
  | "DuplicateCardId"
  | "DuplicateActiveContent"
  | "IdempotentReplay"
  | "IdempotencyPayloadMismatch"
  | "CardNotFound"
  | "UnauthorizedApprover"
  | "ReplayedApprovalNonce"
  | "InvalidCardTransition"
  | "RevisionConflict"
  | "CapacityExceeded";

/** Typed LEARN pipeline error carrying a machine-readable discriminant code for callers. */
export class LearnError extends Error {
  public readonly code: LearnErrorCode;
  public readonly meta?: Record<string, unknown>;
  constructor(code: LearnErrorCode, message: string, meta?: Record<string, unknown>) {
    super(`${code}: ${message}`);
    this.name = "LearnError";
    this.code = code;
    this.meta = meta;
  }
}

export const StorageConstraintTargetSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("PLUGIN_ACTIVE_SLUG") }),
  z.object({ kind: z.literal("KNOWLEDGE_CARD_PRIMARY_KEY") }),
  z.object({ kind: z.literal("KNOWLEDGE_CARD_CONTENT_HASH") }),
  z.object({ kind: z.literal("CRON_SCHEDULE_LEASE") }),
  z.object({ kind: z.literal("OTHER"), name: z.string() }),
]) satisfies z.ZodType<RustStorageConstraintTarget>;
export type StorageConstraintTarget = z.infer<typeof StorageConstraintTargetSchema>;

export const StorageConstraintErrorSchema = z.object({
  code: z.string(),
  target_kind: StorageConstraintTargetSchema,
  detail: z.string(),
}) satisfies z.ZodType<RustStorageConstraintError>;
export type StorageConstraintError = z.infer<typeof StorageConstraintErrorSchema>;

/** Deterministic Prisma/SQLite storage error mapper (AC-05, EC-E08-05). */
export function mapStorageConstraintError(err: StorageConstraintError): DomainStorageAction {
  const code = err.code.toUpperCase();
  if (code === "P2002" || code === "SQLITE_CONSTRAINT_UNIQUE") {
    const k = err.target_kind.kind;
    if (k === "KNOWLEDGE_CARD_CONTENT_HASH" || k === "KNOWLEDGE_CARD_PRIMARY_KEY") {
      return { action: "IDEMPOTENT_DUPLICATE", error: "DuplicateKnowledgeCard" };
    }
    if (k === "PLUGIN_ACTIVE_SLUG" || k === "CRON_SCHEDULE_LEASE") {
      return { action: "CONFLICT_RETRYABLE", error: "ActivePluginConflict", retry_hint: "Re-read active revision & isolate breaking candidate into Staging" };
    }
    return { action: "UNMAPPED_FATAL", error: `Unclassified unique constraint on '${err.target_kind.name}': ${err.detail}` };
  }
  if (code === "P2003" || code === "SQLITE_CONSTRAINT_FOREIGNKEY") return { action: "PARENT_NOT_FOUND", error: "ProjectOrPluginNotFound" };
  if (code === "SQLITE_BUSY" || code === "P2034") return { action: "RETRY_WITH_BOUNDED_BACKOFF", max_attempts: 3, backoff_ms: [10, 25, 50] };
  return { action: "UNMAPPED_FATAL", error: err.detail };
}
