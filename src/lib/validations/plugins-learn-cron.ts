// @story 08.4
// @implements AC-03, AC-04, AC-05
import { z } from "zod";
import type { CompiledPromptSnapshotDto as RustCompiledPromptSnapshotDto } from "../../../packages/shared-types/src/generated/CompiledPromptSnapshotDto.ts";
import type { CronScheduleStatusDto as RustCronScheduleStatusDto } from "../../../packages/shared-types/src/generated/CronScheduleStatusDto.ts";
import type { HumanOnlyDecisionReceiptDto as RustHumanOnlyDecisionReceiptDto } from "../../../packages/shared-types/src/generated/HumanOnlyDecisionReceiptDto.ts";
import type { KnowledgeCardDto as RustKnowledgeCardDto } from "../../../packages/shared-types/src/generated/KnowledgeCardDto.ts";
import { type AssertExactKeys, safeRemoteHttpUrl, utf8BytesMax } from "./plugins-byte-limit";

const MIN_DECISION_RATIONALE_LEN = 4;
const MAX_DECISION_RATIONALE_LEN = 500;
const MAX_URL_LEN = 1024;
const MAX_RUN_ID_LEN = 128;
export const MAX_LEARN_TOKENS = 1500;
const MAX_SELECTED_SKILLS = 100;
const MAX_SKILL_NAME_LEN = 64;
const MAX_CARD_TITLE_BYTES = 160;
const MAX_CARD_CONTENT_BYTES = 4096;
const MAX_CARD_EVIDENCE_BYTES = 512;
export const MIN_CRON_EXPR_LEN = 5;
export const MAX_CRON_EXPR_LEN = 120;

export const LearnCardCategoryEnum = z.enum([
  "RULE",
  "PATTERN",
  "ANTI_PATTERN",
  "LESSON",
  "FLOW",
]);

const CardStatusEnum = z.enum(["PROPOSED", "ACCEPTED", "REJECTED"]);

const CronSyncStatusEnum = z.enum([
  "IDLE",
  "RUNNING",
  "BACKOFF",
  "SUCCEEDED",
  "AUTH_REQUIRED",
  "FAILED",
]);

const HttpActorKindEnum = z.enum([
  "HUMAN_WORKBENCH_SESSION",
  "SUPERADMIN_CLI",
  "AGENT_HMAC",
  "MCP_CLIENT",
]);

export const CompilePromptBundleRequestSchema = z.object({
  runId: z.string().min(1).max(MAX_RUN_ID_LEN),
  selectedSkillNames: z.array(z.string().min(1).max(MAX_SKILL_NAME_LEN)).max(MAX_SELECTED_SKILLS).default([]),
  maxLearnTokens: z.number().int().positive().max(MAX_LEARN_TOKENS).optional(),
});

export const CompiledPromptSnapshotDtoSchema = z.object({
  runId: z.string(),
  stagingDir: z.string(),
  stablePromptHash: z.string(),
  learnSnapshotSha256: z.string(),
  learnCardIds: z.array(z.string()),
  learnTotalTokens: z.number().int().nonnegative(),
  fencedPromptXml: z.string(),
}) satisfies z.ZodType<RustCompiledPromptSnapshotDto, z.ZodTypeDef, unknown>;

export const KnowledgeCardDtoSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string(),
  projectId: z.string(),
  idempotencyKey: z.string().nullable().default(null),
  category: LearnCardCategoryEnum,
  title: z.string(),
  content: z.string(),
  contentSha256: z.string(),
  sourceEvidence: z.string(),
  status: CardStatusEnum,
  revision: z.number().int().nonnegative(),
  reviewedBy: z.string().nullable().default(null),
  decisionReason: z.string().nullable().default(null),
  createdAt: z.number().int(),
  acceptedAt: z.number().int().nullable().default(null),
}) satisfies z.ZodType<RustKnowledgeCardDto, z.ZodTypeDef, unknown>;

export const ProposeLearnCardRequestSchema = z.object({
  idempotencyKey: z.string().min(1).max(MAX_RUN_ID_LEN).optional(),
  category: LearnCardCategoryEnum,
  title: utf8BytesMax(MAX_CARD_TITLE_BYTES, "title"),
  content: utf8BytesMax(MAX_CARD_CONTENT_BYTES, "content"),
  sourceEvidence: utf8BytesMax(MAX_CARD_EVIDENCE_BYTES, "sourceEvidence"),
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
}) satisfies z.ZodType<RustHumanOnlyDecisionReceiptDto, z.ZodTypeDef, unknown>;

export const CronScheduleStatusDtoSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string(),
  pluginId: z.string(),
  cronExpression: z.string(),
  upstreamRepoUrl: z.string(),
  lastCommitSha: z.string().nullable().default(null),
  syncStatus: CronSyncStatusEnum,
  consecutiveFailures: z.number().int().nonnegative(),
  nextRetryAt: z.number().int().nullable().default(null),
  lastErrorCode: z.string().nullable().default(null),
  lastErrorMessage: z.string().nullable().default(null),
  updatedAt: z.string(),
}) satisfies z.ZodType<RustCronScheduleStatusDto, z.ZodTypeDef, unknown>;

export const UpsertCronScheduleRequestSchema = z.object({
  projectId: z.string().min(1),
  cronExpression: z.string().trim().min(MIN_CRON_EXPR_LEN).max(MAX_CRON_EXPR_LEN),
  upstreamRepoUrl: safeRemoteHttpUrl(MAX_URL_LEN),
});

export const TriggerSyncRequestSchema = z.object({
  projectId: z.string().min(1).optional(),
  refreshAuth: z.boolean().optional(),
});

export type CompilePromptBundleRequest = z.input<typeof CompilePromptBundleRequestSchema>;
export type CompiledPromptSnapshotDto = z.infer<typeof CompiledPromptSnapshotDtoSchema>;
export type KnowledgeCardDto = z.infer<typeof KnowledgeCardDtoSchema>;
export type ProposeLearnCardRequest = z.input<typeof ProposeLearnCardRequestSchema>;
export type LearnCardDecisionRequest = z.input<typeof LearnCardDecisionRequestSchema>;
export type HumanOnlyDecisionReceiptDto = z.infer<typeof HumanOnlyDecisionReceiptDtoSchema>;
export type CronScheduleStatusDto = z.infer<typeof CronScheduleStatusDtoSchema>;
export type UpsertCronScheduleRequest = z.input<typeof UpsertCronScheduleRequestSchema>;
export type TriggerSyncRequest = z.input<typeof TriggerSyncRequestSchema>;

const _compiledPromptBidirectionalCheck: [RustCompiledPromptSnapshotDto] extends [CompiledPromptSnapshotDto]
  ? AssertExactKeys<CompiledPromptSnapshotDto, RustCompiledPromptSnapshotDto>
  : never = true;
const _knowledgeCardKeyCheck: AssertExactKeys<KnowledgeCardDto, RustKnowledgeCardDto> = true;
const _decisionReceiptKeyCheck: AssertExactKeys<HumanOnlyDecisionReceiptDto, RustHumanOnlyDecisionReceiptDto> = true;
const _cronScheduleKeyCheck: AssertExactKeys<CronScheduleStatusDto, RustCronScheduleStatusDto> = true;
void [_compiledPromptBidirectionalCheck, _knowledgeCardKeyCheck, _decisionReceiptKeyCheck, _cronScheduleKeyCheck];
