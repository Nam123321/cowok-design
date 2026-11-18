// @story 08.4
// @implements AC-01, AC-02
import { z } from "zod";
import type { PluginSummaryDto as RustPluginSummaryDto } from "../../../packages/shared-types/src/generated/PluginSummaryDto.ts";
import { type AssertExactKeys, safeNonGithubSourceUrl, safeRelativePath, safeRemoteHttpUrl, utf8BytesMax } from "./plugins-byte-limit";

const MIN_DECISION_RATIONALE_LEN = 4;
const MAX_DECISION_RATIONALE_LEN = 500;
const MAX_SKILL_NAME_BYTES = 64;
const MAX_SKILL_VERSION_BYTES = 32;
const MAX_SKILL_DESC_BYTES = 320;
const MAX_ENTRY_POINT_LEN = 256;
const MAX_CAPABILITY_LEN = 64;
const MAX_CAPABILITIES_COUNT = 32;
const MAX_TAGS_COUNT = 32;
const MAX_TAG_LEN = 64;
const MAX_PROMPT_OVERVIEW_LEN = 4096;
const MAX_SKILL_MD_LEN = 65_536;
const MIN_RISK_TIER = 1;
const MAX_RISK_TIER = 3;
const MAX_SLUG_BYTES = 64;
const MAX_URL_LEN = 1024;
const MAX_REASON_LEN = 1000;
const SAFE_SEGMENT_RE = /^[A-Za-z0-9._-]+$/;

export const PluginStateEnum = z
  .enum([
    "DISCOVERED",
    "VALIDATED",
    "INSTALLED",
    "ACTIVE",
    "ENABLED",
    "DEPRECATED",
    "ARCHIVED",
  ])
  .transform((state) => (state === "ENABLED" ? "ACTIVE" : state));

export const SourceTypeEnum = z.enum(["builtin", "local", "github", "upload"]);

export const SkillManifestInputSchema = z.object({
  name: utf8BytesMax(MAX_SKILL_NAME_BYTES, "name"),
  version: utf8BytesMax(MAX_SKILL_VERSION_BYTES, "version"),
  description: utf8BytesMax(MAX_SKILL_DESC_BYTES, "description"),
  entry_point: safeRelativePath(MAX_ENTRY_POINT_LEN).optional().default("SKILL.md"),
  capabilities_required: z.array(z.string().min(1).max(MAX_CAPABILITY_LEN)).max(MAX_CAPABILITIES_COUNT).default([]),
  risk_tier: z.number().int().min(MIN_RISK_TIER).max(MAX_RISK_TIER).default(MIN_RISK_TIER),
  parameters: z.record(z.unknown()).or(z.boolean()).nullable().optional(),
  tags: z.array(z.string().min(1).max(MAX_TAG_LEN)).max(MAX_TAGS_COUNT).default([]),
  prompt_overview: z.string().max(MAX_PROMPT_OVERVIEW_LEN).nullable().optional(),
});

export const PluginSummaryDtoSchema = z.object({
  id: z.string().min(1),
  tenantId: z.string().default("local-default"),
  projectId: z.string(),
  pluginSlug: z.string().min(1),
  version: z.string().min(1),
  lifecycleState: PluginStateEnum,
  enabled: z.boolean().default(false),
  sourceType: SourceTypeEnum.default("local"),
  sourceRepoUrl: z.string().default(""),
  upstreamRepoUrl: z.string().nullable().default(null),
  upstreamCommitSha: z.string().nullable().default(null),
  cronSchedule: z.string().nullable().default(null),
  revision: z.number().int().nonnegative().default(1),
  breakingChangeIsolated: z.boolean().default(false),
  stagingVersion: z.string().nullable().default(null),
  stagingDiffReasons: z.array(z.string()).default([]),
  quarantined: z.boolean().default(false),
  quarantineReason: z.string().nullable().default(null),
  manifestJson: z.record(z.unknown()).default({}),
  updatedAt: z.string().default(""),
}) satisfies z.ZodType<RustPluginSummaryDto, z.ZodTypeDef, unknown>;

export const DiscoverOrInstallPluginRequestSchema = z
  .object({
    projectId: z.string().min(1),
    pluginSlug: utf8BytesMax(MAX_SLUG_BYTES, "pluginSlug")
      .refine((v) => v !== "." && v !== ".." && SAFE_SEGMENT_RE.test(v), {
        message: "pluginSlug must match [A-Za-z0-9._-]+ and not be '.' or '..'",
      }),
    sourceType: SourceTypeEnum.optional(),
    sourceRepoUrl: z.string().min(1).max(MAX_URL_LEN),
    manifest: SkillManifestInputSchema,
    skillMd: z.string().max(MAX_SKILL_MD_LEN).optional(),
  })
  .superRefine((val, ctx) => {
    if (val.sourceType === "github") {
      if (!safeRemoteHttpUrl(MAX_URL_LEN).safeParse(val.sourceRepoUrl).success) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["sourceRepoUrl"],
          message: "GitHub sourceRepoUrl must be a valid public https:// URL",
        });
      }
      return;
    }
    if (!safeNonGithubSourceUrl(val.sourceRepoUrl)) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["sourceRepoUrl"],
        message: "sourceRepoUrl contains a disallowed URI scheme, traversal, or private address",
      });
    }
  });

export const TransitionPluginRequestSchema = z.object({
  projectId: z.string().min(1),
  targetState: PluginStateEnum,
  expectedRevision: z.number().int().nonnegative(),
  reason: z.string().max(MAX_REASON_LEN).optional(),
});

export const QuarantinePluginRequestSchema = z.object({
  projectId: z.string().min(1),
  expectedRevision: z.number().int().nonnegative(),
  reason: z.string().trim().min(MIN_DECISION_RATIONALE_LEN).max(MAX_DECISION_RATIONALE_LEN),
});

export const StagingDecisionRequestSchema = z.object({
  projectId: z.string().min(1),
  expectedRevision: z.number().int().nonnegative(),
  reason: z.string().min(MIN_DECISION_RATIONALE_LEN).max(MAX_DECISION_RATIONALE_LEN),
});

export type PluginLifecycleTarget = "VALIDATED" | "INSTALLED" | "ACTIVE" | "DEPRECATED" | "ARCHIVED";
export type PluginSummaryDto = z.infer<typeof PluginSummaryDtoSchema>;
const _pluginSummaryBidirectionalKeyCheck: AssertExactKeys<PluginSummaryDto, RustPluginSummaryDto> = true;
void _pluginSummaryBidirectionalKeyCheck;
export type DiscoverOrInstallPluginRequest = z.input<typeof DiscoverOrInstallPluginRequestSchema>;
export type TransitionPluginRequest = z.input<typeof TransitionPluginRequestSchema>;
export type QuarantinePluginRequest = z.input<typeof QuarantinePluginRequestSchema>;
export type StagingDecisionRequest = z.input<typeof StagingDecisionRequestSchema>;
