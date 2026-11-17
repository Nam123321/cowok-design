// @story 08.1, 08.2
// @implements AC-02, AC-06
import { z } from "zod";

export const SkillManifestSchema = z.object({
  name: z.string(),
  version: z.string(),
  description: z.string(),
  entry_point: z.string().default("SKILL.md"),
  capabilities_required: z.array(z.string()).default([]),
  risk_tier: z.number().int().min(1).max(3).default(1),
  parameters: z.union([z.record(z.unknown()), z.boolean()]).nullish(),
  tags: z.array(z.string()).default([]),
  prompt_overview: z.string().nullish(),
});

export type SkillManifest = z.infer<typeof SkillManifestSchema>;

export interface CraftRule {
  id: string;
  title: string;
  category: "typography" | "spacing" | "color" | "hierarchy" | "interactions";
  summary: string;
  content: string;
}

export interface DesignTemplate {
  id: string;
  name: string;
  category: string;
  description: string;
  sections: Array<{
    id: string;
    type: string;
    title: string;
  }>;
}

export interface DesignSystemV1 {
  version: "v1";
  name: string;
  tokens: Record<string, unknown>;
  mapping: Record<string, string>;
}

export interface ScenarioStep {
  step: string;
  skill: string;
  repeat_until?: string;
  max_attempts?: number;
}

export interface ScenarioDef {
  id: string;
  name: string;
  description: string;
  pipeline: ScenarioStep[];
}

export interface AtomDef {
  id: string;
  name: string;
  category: string;
  markup_template: string;
}

export interface StagingRunReceipt {
  run_id: string;
  staged_skills: string[];
  stable_prompt_hash: string;
  staged_dir: string;
  timestamp: number;
}

export type LintSeverity = "Error" | "Warning" | "Info";
export type LintIssueCategory = "AntiAiSlop" | "GeometricLayout" | "WcagContrast" | "SyntaxIntegrity";

export interface LintIssue {
  rule_id: string;
  category: LintIssueCategory;
  severity: LintSeverity;
  message: string;
  line?: number;
  column?: number;
  snippet?: string;
  auto_fixable: boolean;
}

export interface TriangleLintReport {
  total_issues: usize | number;
  new_in_this_call: usize | number;
  fixed_issues: usize | number;
  passed: boolean;
  duration_ms: number;
  issues: LintIssue[];
}

type usize = number;

export const VideoFrameCheckpointSchema = z.object({
  frame_id: z.string(),
  timestamp_s: z.number().nonnegative(),
  action_label: z.string().min(1),
  ui_intent: z.string().min(1),
  coordinate_bounds: z.tuple([z.number(), z.number(), z.number(), z.number()]).nullable().optional(),
});

export const VideoStoryboardReceiptSchema = z.object({
  video_source: z.string().min(1),
  duration_s: z.number().nonnegative(),
  total_checkpoints: z.number().int().nonnegative(),
  checkpoints: z.array(VideoFrameCheckpointSchema),
  generated_at: z.number().int(),
});

export const DiscoveryStubSchema = z.object({
  name: z.string().min(1).max(64),
  version: z.string().min(1).max(32),
  description: z.string().max(320),
  upstream_url: z.string().max(256).optional(),
  capabilities: z.array(z.string().max(48)).max(16),
  risk_tier: z.number().int().min(1).max(3),
  package_sha256: z.string().regex(/^[a-f0-9]{64}$/),
  stub_size_bytes: z.number().int().positive().max(1536),
});

export const SkillsLockEntrySchema = z.object({
  path: z.string().min(1),
  packaged_path: z.string().min(1),
  sha256: z.string().regex(/^[a-f0-9]{64}$/),
  bytes: z.number().int().nonnegative(),
});

export const SkillsLockSchema = z.object({
  schema_version: z.literal(1),
  skill_name: z.string().min(1).max(64),
  version: z.string().min(1).max(32),
  package_sha256: z.string().regex(/^[a-f0-9]{64}$/),
  source_files: z.array(SkillsLockEntrySchema),
  generated_files: z.array(SkillsLockEntrySchema),
});

export const SkillPluginDtoSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  version: z.string().min(1),
  entry_point: z.string().min(1),
  permissions: z.array(z.string()),
});

export const RepackageReceiptSchema = z.object({
  skill_name: z.string().min(1),
  version: z.string().min(1),
  target_dir: z.string().min(1),
  files_repackaged: z.number().int().nonnegative(),
  discovery_stub: DiscoveryStubSchema,
  skill_plugin: SkillPluginDtoSchema,
  created_at: z.number().int(),
});

export const StagedBundleReceiptSchema = z.object({
  tmp_dir: z.string().min(1),
  skill_name: z.string().min(1),
  package_sha256: z.string().regex(/^[a-f0-9]{64}$/),
  staging_token: z.string().regex(/^[a-f0-9]{64}$/),
});

export type SkillPluginDto = z.infer<typeof SkillPluginDtoSchema>;
export type VideoFrameCheckpoint = z.infer<typeof VideoFrameCheckpointSchema>;
export type VideoStoryboardReceipt = z.infer<typeof VideoStoryboardReceiptSchema>;
export type DiscoveryStub = z.infer<typeof DiscoveryStubSchema>;
export type LockedSkillEntry = z.infer<typeof SkillsLockEntrySchema>;
export type SkillsLockFile = z.infer<typeof SkillsLockSchema>;
export type RepackageReceipt = z.infer<typeof RepackageReceiptSchema>;
