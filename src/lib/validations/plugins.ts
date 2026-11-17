// @story 08.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";

const MIN_DECISION_RATIONALE_LEN = 4;
const MAX_DECISION_RATIONALE_LEN = 500;

export const PluginStateEnum = z.enum([
  "DISCOVERED",
  "VALIDATED",
  "INSTALLED",
  "ACTIVE",
  "ENABLED",
  "DEPRECATED",
  "ARCHIVED",
]);

export const SourceTypeEnum = z.enum(["builtin", "local", "github", "upload"]);

export const LearnCardCategoryEnum = z.enum([
  "RULE",
  "PATTERN",
  "ANTI_PATTERN",
  "LESSON",
]);

export const CardStatusEnum = z.enum(["PROPOSED", "ACCEPTED", "REJECTED"]);

export const HttpActorKindEnum = z.enum([
  "HUMAN_WORKBENCH_SESSION",
  "SUPERADMIN_CLI",
  "AGENT_HMAC",
  "MCP_CLIENT",
]);

export const SkillManifestInputSchema = z.object({
  name: z.string().min(1).max(160),
  version: z.string().min(1).max(64),
  description: z.string().min(1).max(2000),
  entry_point: z.string().min(1).optional().default("SKILL.md"),
  capabilities_required: z.array(z.string()).default([]),
  risk_tier: z.number().int().min(1).max(4).default(1),
  ui_capabilities: z.record(z.unknown()).or(z.boolean()).nullable().optional(),
});

export const PluginSummaryDtoSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string().default("local-default"),
  projectId: z.string().min(1),
  pluginSlug: z.string().min(1),
  version: z.string().min(1),
  lifecycleState: z.string().min(1),
  enabled: z.boolean().default(false),
  sourceType: z.string().default("local"),
  sourceRepoUrl: z.string().default(""),
  upstreamCommitSha: z.string().nullable().optional(),
  cronSchedule: z.string().nullable().optional(),
  revision: z.number().int().nonnegative().default(1),
  breakingChangeIsolated: z.boolean().default(false),
  stagingVersion: z.string().nullable().optional(),
  stagingDiffReasons: z.array(z.string()).default([]),
  quarantined: z.boolean().default(false),
  quarantineReason: z.string().nullable().optional(),
  manifestJson: z.record(z.unknown()).default({}),
  updatedAt: z.string().default(""),
});

export const DiscoverOrInstallPluginRequestSchema = z.object({
  projectId: z.string().min(1),
  pluginSlug: z.string().min(1).max(120),
  sourceType: SourceTypeEnum.optional(),
  sourceRepoUrl: z.string().min(1).max(1024),
  manifest: SkillManifestInputSchema,
  skillMd: z.string().optional(),
});

export const TransitionPluginRequestSchema = z.object({
  projectId: z.string().min(1),
  targetState: PluginStateEnum,
  expectedRevision: z.number().int().nonnegative(),
  reason: z.string().max(1000).optional(),
});

export const StagingDecisionRequestSchema = z.object({
  projectId: z.string().min(1),
  expectedRevision: z.number().int().nonnegative(),
  reason: z.string().min(MIN_DECISION_RATIONALE_LEN).max(MAX_DECISION_RATIONALE_LEN),
});

export const RepackageBundleRequestSchema = z.object({
  projectId: z.string().min(1),
  sourceDir: z.string().min(1).max(512),
  skillName: z.string().min(1).max(120),
  description: z.string().min(1).max(2000),
  capabilitiesRequired: z.array(z.string()).default([]),
  upstreamUrl: z.string().url().optional(),
});

export const RepackageReceiptSchema = z.object({
  skill_name: z.string(),
  version: z.string(),
  target_dir: z.string(),
  files_repackaged: z.number().int().nonnegative(),
  discovery_stub: z.record(z.unknown()),
  skill_plugin: z.record(z.unknown()),
  created_at: z.number().int(),
});

export const DiscoveryDescriptorRequestSchema = z.object({
  bundleDir: z.string().min(1).max(512),
});

export const DiscoveryDescriptorResponseSchema = z.record(z.unknown());

export const VideoIntakeRequestSchema = z.object({
  skillName: z.string().min(1).max(120),
  storyboardMarkdown: z.string().min(1).max(65_536),
  expectedDurationSecs: z.number().positive().optional(),
});

export const VideoStoryboardReceiptSchema = z.object({
  video_source: z.string(),
  duration_s: z.number(),
  total_checkpoints: z.number().int().nonnegative(),
  checkpoints: z.array(z.record(z.unknown())),
  generated_at: z.number().int(),
});

export const TriangleLintRequestSchema = z.object({
  content: z.string().min(1).max(100_000),
});

export const TriangleLintReportSchema = z.object({
  total_issues: z.number().int().nonnegative(),
  new_in_this_call: z.number().int().nonnegative(),
  fixed_issues: z.number().int().nonnegative(),
  passed: z.boolean(),
  duration_ms: z.number().int().nonnegative(),
  issues: z.array(z.record(z.unknown())),
});

export const CompilePromptBundleRequestSchema = z.object({
  runId: z.string().min(1).max(128),
  selectedSkillNames: z.array(z.string()).default([]),
  maxLearnTokens: z.number().int().positive().max(1500).optional(),
});

export const CompiledPromptSnapshotDtoSchema = z.object({
  runId: z.string(),
  stagingDir: z.string(),
  stablePromptHash: z.string(),
  learnSnapshotSha256: z.string(),
  learnCardIds: z.array(z.string()),
  learnTotalTokens: z.number().int().nonnegative(),
  fencedPromptXml: z.string(),
});

export const KnowledgeCardDtoSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string(),
  projectId: z.string(),
  idempotencyKey: z.string().nullable().optional(),
  category: LearnCardCategoryEnum,
  title: z.string(),
  content: z.string(),
  contentSha256: z.string(),
  sourceEvidence: z.string(),
  status: CardStatusEnum,
  revision: z.number().int().nonnegative(),
  reviewedBy: z.string().nullable().optional(),
  decisionReason: z.string().nullable().optional(),
  createdAt: z.number().int(),
  acceptedAt: z.number().int().nullable().optional(),
});

export const ProposeLearnCardRequestSchema = z.object({
  idempotencyKey: z.string().min(1).max(128).optional(),
  category: LearnCardCategoryEnum,
  title: z.string().min(1).max(200),
  content: z.string().min(1).max(8000),
  sourceEvidence: z.string().min(1).max(1000),
});

export const LearnCardDecisionRequestSchema = z.object({
  reason: z.string().min(MIN_DECISION_RATIONALE_LEN).max(MAX_DECISION_RATIONALE_LEN),
  expectedRevision: z.number().int().nonnegative(),
});

export const HumanOnlyDecisionReceiptDtoSchema = z.object({
  card: KnowledgeCardDtoSchema,
  kid: z.string(),
  actorKind: HttpActorKindEnum,
  evidenceHash: z.string(),
});

export const CronScheduleStatusDtoSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string(),
  pluginId: z.string(),
  cronExpression: z.string(),
  upstreamRepoUrl: z.string(),
  lastCommitSha: z.string().nullable().optional(),
  syncStatus: z.string(),
  consecutiveFailures: z.number().int().nonnegative(),
  nextRetryAt: z.number().int().nullable().optional(),
  lastErrorCode: z.string().nullable().optional(),
  lastErrorMessage: z.string().nullable().optional(),
  updatedAt: z.string(),
});

export const UpsertCronScheduleRequestSchema = z.object({
  projectId: z.string().min(1),
  cronExpression: z.string().min(5).max(120),
  upstreamRepoUrl: z.string().url().max(1024),
});

export const TriggerSyncRequestSchema = z.object({
  projectId: z.string().min(1).optional(),
  refreshAuth: z.boolean().optional(),
});

export type PluginSummaryDto = z.infer<typeof PluginSummaryDtoSchema>;
export type DiscoverOrInstallPluginRequest = z.input<typeof DiscoverOrInstallPluginRequestSchema>;
export type TransitionPluginRequest = z.input<typeof TransitionPluginRequestSchema>;
export type StagingDecisionRequest = z.input<typeof StagingDecisionRequestSchema>;
export type RepackageBundleRequest = z.input<typeof RepackageBundleRequestSchema>;
export type RepackageReceipt = z.infer<typeof RepackageReceiptSchema>;
export type DiscoveryDescriptorRequest = z.input<typeof DiscoveryDescriptorRequestSchema>;
export type VideoIntakeRequest = z.input<typeof VideoIntakeRequestSchema>;
export type VideoStoryboardReceipt = z.infer<typeof VideoStoryboardReceiptSchema>;
export type TriangleLintRequest = z.input<typeof TriangleLintRequestSchema>;
export type TriangleLintReport = z.infer<typeof TriangleLintReportSchema>;
export type CompilePromptBundleRequest = z.input<typeof CompilePromptBundleRequestSchema>;
export type CompiledPromptSnapshotDto = z.infer<typeof CompiledPromptSnapshotDtoSchema>;
export type KnowledgeCardDto = z.infer<typeof KnowledgeCardDtoSchema>;
export type ProposeLearnCardRequest = z.input<typeof ProposeLearnCardRequestSchema>;
export type LearnCardDecisionRequest = z.input<typeof LearnCardDecisionRequestSchema>;
export type HumanOnlyDecisionReceiptDto = z.infer<typeof HumanOnlyDecisionReceiptDtoSchema>;
export type CronScheduleStatusDto = z.infer<typeof CronScheduleStatusDtoSchema>;
export type UpsertCronScheduleRequest = z.input<typeof UpsertCronScheduleRequestSchema>;
export type TriggerSyncRequest = z.input<typeof TriggerSyncRequestSchema>;
