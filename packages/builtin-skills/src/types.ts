import { z } from "zod";

export const SkillManifestSchema = z.object({
  name: z.string(),
  version: z.string(),
  description: z.string(),
  entry_point: z.string().default("SKILL.md"),
  capabilities_required: z.array(z.string()).default([]),
  risk_tier: z.number().int().min(1).max(3).default(1),
  parameters: z.record(z.unknown()).optional(),
  tags: z.array(z.string()).default([]),
  prompt_overview: z.string().optional(),
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
