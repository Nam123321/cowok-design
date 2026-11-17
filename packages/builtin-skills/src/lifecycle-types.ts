// @story 08.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { z } from "zod";
import type {
  ActorKind as RustActorKind,
  BreakingDiffReport as RustBreakingDiffReport,
  CardDecisionAction as RustCardDecisionAction,
  CardStatus as RustCardStatus,
  HumanApproverToken as RustHumanApproverToken,
  KnowledgeCard as RustKnowledgeCard,
  LearnCardCategory as RustLearnCardCategory,
  ManagedPluginRecord as RustManagedPluginRecord,
  PluginState as RustPluginState,
  StagedPluginBundle as RustStagedPluginBundle,
} from "@cowok-design/shared-types";
import { SkillManifestSchema } from "./types";

const PLUGIN_STATE_MAP: Record<string, RustPluginState> = {
  DISCOVERED: "DISCOVERED",
  VALIDATED: "VALIDATED",
  INSTALLED: "INSTALLED",
  ACTIVE: "ACTIVE",
  ENABLED: "ACTIVE",
  DEPRECATED: "DEPRECATED",
  ARCHIVED: "ARCHIVED",
};

const LifecycleSkillManifestSchema = SkillManifestSchema.extend({
  parameters: z.union([z.record(z.unknown()), z.boolean()]).nullable().default(null),
  prompt_overview: z.string().nullable().default(null),
});

export const PluginStateSchema = z.string().transform((raw, ctx) => {
  const mapped = PLUGIN_STATE_MAP[raw.trim().toUpperCase()];
  if (!mapped) {
    ctx.addIssue({ code: z.ZodIssueCode.custom, message: `Unknown plugin state '${raw}'` });
    return z.NEVER;
  }
  return mapped;
});
export type PluginState = z.infer<typeof PluginStateSchema>;

const CATEGORY_MAP: Record<string, RustLearnCardCategory> = {
  RULE: "RULE",
  PATTERN: "PATTERN",
  ANTI_PATTERN: "ANTI_PATTERN",
  ANTIPATTERN: "ANTI_PATTERN",
  LESSON: "LESSON",
  FLOW: "FLOW",
};

export const LearnCardCategorySchema = z.string().transform((raw, ctx) => {
  const mapped = CATEGORY_MAP[raw.trim().toUpperCase()];
  if (!mapped) {
    ctx.addIssue({ code: z.ZodIssueCode.custom, message: `Unknown LearnCardCategory '${raw}'` });
    return z.NEVER;
  }
  return mapped;
});
export type LearnCardCategory = z.infer<typeof LearnCardCategorySchema>;

export const CardStatusSchema = z.string().transform((raw, ctx) => {
  const upper = raw.trim().toUpperCase();
  if (upper === "PROPOSED" || upper === "ACCEPTED" || upper === "REJECTED") {
    return upper as RustCardStatus;
  }
  ctx.addIssue({ code: z.ZodIssueCode.custom, message: `Unknown CardStatus '${raw}'` });
  return z.NEVER;
});
export type CardStatus = z.infer<typeof CardStatusSchema>;

export const CardDecisionActionSchema = z.enum(["ACCEPT", "REJECT"]) satisfies z.ZodType<RustCardDecisionAction>;
export type CardDecisionAction = z.infer<typeof CardDecisionActionSchema>;

export const ActorKindSchema = z.enum([
  "HUMAN_WORKBENCH_SESSION",
  "AGENT_HMAC",
  "MCP_CLIENT",
  "SUPERADMIN_CLI",
]) satisfies z.ZodType<RustActorKind>;
export type ActorKind = z.infer<typeof ActorKindSchema>;

export const HumanApproverTokenSchema = z.object({
  kid: z.string().min(1),
  actor_kind: ActorKindSchema,
  actor_id: z.string().min(1),
  tenant_id: z.string().min(1),
  project_id: z.string().min(1),
  card_id: z.string().min(1),
  content_sha256: z.string().regex(/^[0-9a-f]{64}$/),
  action: CardDecisionActionSchema,
  expected_revision: z.number().int().min(1),
  reason: z.string().min(4),
  issued_at_s: z.number().int(),
  expires_at_s: z.number().int(),
  nonce: z.string().min(1),
  hmac_sha256: z.string().regex(/^[0-9a-f]{64}$/),
}) satisfies z.ZodType<RustHumanApproverToken>;
export type HumanApproverToken = z.infer<typeof HumanApproverTokenSchema>;

export const ManagedPluginRecordSchema = z.object({
  id: z.string().min(1),
  tenant_id: z.string().min(1).default("local-default"),
  project_id: z.string().min(1).default("local-default"),
  plugin_slug: z.string().min(1),
  name: z.string().min(1),
  version: z.string().min(1),
  state: PluginStateSchema,
  revision: z.number().int().min(1),
  breaking_change_isolated: z.boolean(),
  staging_version: z.string().nullable(),
  staging_manifest: LifecycleSkillManifestSchema.nullable(),
  staging_diff_reasons: z.array(z.string()).default([]),
  manifest: LifecycleSkillManifestSchema,
  quarantined: z.boolean(),
  quarantine_reason: z.string().nullable(),
  updated_at: z.number().int(),
}) satisfies z.ZodType<RustManagedPluginRecord, z.ZodTypeDef, unknown>;
export type ManagedPluginRecord = z.infer<typeof ManagedPluginRecordSchema>;

export const BreakingDiffReportSchema = z.object({
  is_breaking: z.boolean(),
  reasons: z.array(z.string()),
}) satisfies z.ZodType<RustBreakingDiffReport, z.ZodTypeDef, unknown>;
export type BreakingDiffReport = z.infer<typeof BreakingDiffReportSchema>;

export const StagedPluginBundleSchema = z.object({
  manifest: LifecycleSkillManifestSchema,
  skill_md: z.string(),
  scripts: z.array(z.object({ rel_path: z.string(), content: z.string() })).default([]),
}) satisfies z.ZodType<RustStagedPluginBundle, z.ZodTypeDef, unknown>;
export type StagedPluginBundle = z.infer<typeof StagedPluginBundleSchema>;

export const KnowledgeCardSchema = z.object({
  id: z.string().min(1),
  tenant_id: z.string().min(1),
  project_id: z.string().min(1),
  idempotency_key: z.string().nullable(),
  category: LearnCardCategorySchema,
  title: z.string().min(1),
  content: z.string().min(1),
  content_sha256: z.string().regex(/^[0-9a-f]{64}$/),
  status: CardStatusSchema,
  source_evidence: z.string().min(1),
  revision: z.number().int().min(1),
  reviewed_by: z.string().nullable(),
  decision_reason: z.string().nullable(),
  created_at: z.number().int(),
  accepted_at: z.number().int().nullable(),
}) satisfies z.ZodType<RustKnowledgeCard, z.ZodTypeDef, unknown>;
export type KnowledgeCard = z.infer<typeof KnowledgeCardSchema>;
