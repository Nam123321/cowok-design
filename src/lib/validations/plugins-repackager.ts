// @story 08.4
// @implements AC-02, AC-03
import { z } from "zod";
import type { DiscoveryStub as RustDiscoveryStub } from "../../../packages/shared-types/src/generated/DiscoveryStub.ts";
import type { RepackageReceipt as RustRepackageReceipt } from "../../../packages/shared-types/src/generated/RepackageReceipt.ts";
import type { TriangleLintReport as RustTriangleLintReport } from "../../../packages/shared-types/src/generated/TriangleLintReport.ts";
import type { VideoStoryboardReceipt as RustVideoStoryboardReceipt } from "../../../packages/shared-types/src/generated/VideoStoryboardReceipt.ts";
import { type AssertExactKeys, safeRelativePath, safeRemoteHttpUrl, utf8BytesMax } from "./plugins-byte-limit";

const MAX_SKILL_NAME_BYTES = 64;
const MAX_PATH_LEN = 512;
const MAX_REPACKAGE_DESC_BYTES = 320;
const MAX_CAPABILITY_BYTES = 48;
const MAX_CAPABILITIES_COUNT = 16;
const MAX_UPSTREAM_URL_BYTES = 256;
const MAX_STORYBOARD_BYTES = 65_536;
const MAX_LINT_CONTENT_LEN = 100_000;

const DiscoveryStubDtoSchema = z.object({
  name: z.string().min(1),
  version: z.string().min(1),
  description: z.string().min(1),
  upstream_url: z.string().nullish().transform((v) => v ?? undefined),
  capabilities: z.array(z.string()).default([]),
  risk_tier: z.number().int().default(1),
  package_sha256: z.string().default(""),
  stub_size_bytes: z.number().int().nonnegative().default(0),
}) satisfies z.ZodType<RustDiscoveryStub, z.ZodTypeDef, unknown>;

const SkillPluginDtoShapeSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  version: z.string().min(1),
  entry_point: z.string().min(1),
  permissions: z.array(z.string()).default([]),
});

const VideoCheckpointDtoSchema = z.object({
  frame_id: z.string().min(1),
  timestamp_s: z.number().nonnegative(),
  action_label: z.string().min(1),
  ui_intent: z.string().min(1),
  coordinate_bounds: z
    .tuple([z.number(), z.number(), z.number(), z.number()])
    .nullish()
    .transform((v) => v ?? undefined),
});

const TriangleLintIssueDtoSchema = z.object({
  rule_id: z.string().min(1),
  category: z.enum(["AntiAiSlop", "GeometricLayout", "WcagContrast", "SyntaxIntegrity"]),
  severity: z.enum(["Error", "Warning", "Info"]),
  message: z.string().min(1),
  line: z.number().int().positive().nullish(),
  column: z.number().int().positive().nullish(),
  snippet: z.string().nullish().transform((v) => v ?? undefined),
  auto_fixable: z.boolean().default(false),
});

export const RepackageBundleRequestSchema = z.object({
  projectId: z.string().min(1),
  sourceDir: safeRelativePath(MAX_PATH_LEN),
  skillName: utf8BytesMax(MAX_SKILL_NAME_BYTES, "skillName")
    .refine((v) => v !== "." && v !== ".." && /^[A-Za-z0-9._-]+$/.test(v), {
      message: "skillName must match [A-Za-z0-9._-]+ and not be a traversal segment",
    }),
  description: utf8BytesMax(MAX_REPACKAGE_DESC_BYTES, "description").refine(
    (v) => !v.replace(/\r\n?/g, "\n").split("\n").some((line) => line.trim() === "---"),
    { message: "description must not contain YAML '---' delimiter lines" }
  ),
  capabilitiesRequired: z
    .array(utf8BytesMax(MAX_CAPABILITY_BYTES, "capability"))
    .max(MAX_CAPABILITIES_COUNT)
    .default([]),
  upstreamUrl: safeRemoteHttpUrl(MAX_UPSTREAM_URL_BYTES).optional(),
});

export const RepackageReceiptSchema = z.object({
  skill_name: z.string(),
  version: z.string(),
  target_dir: z.string(),
  files_repackaged: z.number().int().nonnegative(),
  discovery_stub: DiscoveryStubDtoSchema,
  skill_plugin: SkillPluginDtoShapeSchema,
  created_at: z.number().int(),
}) satisfies z.ZodType<RustRepackageReceipt, z.ZodTypeDef, unknown>;

export const DiscoveryDescriptorRequestSchema = z.object({
  projectId: z.string().min(1).optional(),
  bundleDir: safeRelativePath(MAX_PATH_LEN),
});

export const DiscoveryDescriptorResponseSchema = z.object({
  discoveryStub: DiscoveryStubDtoSchema,
  skillPlugin: SkillPluginDtoShapeSchema,
});

export const VideoIntakeRequestSchema = z.object({
  skillName: z.string().min(1).max(MAX_SKILL_NAME_BYTES),
  storyboardMarkdown: z.string().min(1).max(MAX_STORYBOARD_BYTES),
  expectedDurationSecs: z.number().finite().positive().optional(),
});

export const VideoStoryboardReceiptSchema = z.object({
  video_source: z.string(),
  duration_s: z.number(),
  total_checkpoints: z.number().int().nonnegative(),
  checkpoints: z.array(VideoCheckpointDtoSchema),
  generated_at: z.number().int(),
}) satisfies z.ZodType<RustVideoStoryboardReceipt, z.ZodTypeDef, unknown>;

export const TriangleLintRequestSchema = z.object({
  content: z.string().min(1).max(MAX_LINT_CONTENT_LEN),
  autoFix: z.boolean().optional(),
});

export const TriangleLintReportSchema = z.object({
  total_issues: z.number().int().nonnegative(),
  new_in_this_call: z.number().int().nonnegative(),
  fixed_issues: z.number().int().nonnegative(),
  passed: z.boolean(),
  duration_ms: z.number().int().nonnegative(),
  issues: z.array(TriangleLintIssueDtoSchema),
  fixed_content: z.string().nullish().transform((v) => v ?? undefined),
}) satisfies z.ZodType<RustTriangleLintReport, z.ZodTypeDef, unknown>;

export type RepackageBundleRequest = z.input<typeof RepackageBundleRequestSchema>;
export type RepackageReceipt = z.infer<typeof RepackageReceiptSchema>;
export type DiscoveryDescriptorRequest = z.input<typeof DiscoveryDescriptorRequestSchema>;
export type DiscoveryDescriptorResponse = z.infer<typeof DiscoveryDescriptorResponseSchema>;
export type VideoIntakeRequest = z.input<typeof VideoIntakeRequestSchema>;
export type VideoStoryboardReceipt = z.infer<typeof VideoStoryboardReceiptSchema>;
export type TriangleLintRequest = z.input<typeof TriangleLintRequestSchema>;
export type TriangleLintReport = z.infer<typeof TriangleLintReportSchema>;

const _discoveryStubKeyCheck: AssertExactKeys<z.infer<typeof DiscoveryStubDtoSchema>, RustDiscoveryStub> = true;
const _repackageReceiptKeyCheck: AssertExactKeys<RepackageReceipt, RustRepackageReceipt> = true;
const _videoReceiptKeyCheck: AssertExactKeys<VideoStoryboardReceipt, RustVideoStoryboardReceipt> = true;
const _lintReportKeyCheck: AssertExactKeys<TriangleLintReport, RustTriangleLintReport> = true;
void [_discoveryStubKeyCheck, _repackageReceiptKeyCheck, _videoReceiptKeyCheck, _lintReportKeyCheck];
