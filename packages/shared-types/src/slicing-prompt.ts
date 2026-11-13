import { z } from "zod";
import { SafeJsIntegerSchema, U32Schema } from "./primitives.js";
import { CanonicalSpecKeySchema } from "./specs.js";

export const TaskHorizonSchema = z.enum(["short", "mid", "long"]);

export const DepartmentSchema = z.enum([
  "product_ui",
  "marketing_deck",
  "executive_report",
]);

export const PortalSchema = z.enum([
  "super_admin",
  "tenant_admin",
  "sales_app",
  "marketing_web",
  "developer_portal",
  "presentation_studio",
]);

export const ReasonCodeSchema = z.enum([
  "success",
  "redacted_security_boundary",
  "portal_mismatch",
  "department_excluded",
  "token_budget_exceeded",
  "no_eligible_specs",
]);

export const ContextSliceRequestSchema = z
  .object({
    task_input: z.string(),
    override_horizon: TaskHorizonSchema.nullable(),
    department: DepartmentSchema,
    portal: PortalSchema,
    affected_artifacts: z.array(z.string()),
  })
  .strict();

export const ContextSliceDecisionSchema = z
  .object({
    horizon: TaskHorizonSchema,
    department: DepartmentSchema,
    portal: PortalSchema,
    allowed_spec_keys: z.array(CanonicalSpecKeySchema),
    denied_spec_keys: z.array(z.tuple([CanonicalSpecKeySchema, ReasonCodeSchema])),
    token_ceiling: U32Schema,
    primary_reason: ReasonCodeSchema,
  })
  .strict();

export const SpecSliceSchema = z
  .object({
    spec_key: CanonicalSpecKeySchema,
    title: z.string(),
    content: z.string(),
    estimated_tokens: U32Schema,
    priority_rank: U32Schema,
  })
  .strict();

export const AuthorizedContextSliceSchema = z
  .object({
    project_id: z.string().min(1),
    decision: ContextSliceDecisionSchema,
    slices: z.array(SpecSliceSchema),
    total_estimated_tokens: U32Schema,
    atomic_complete: z.boolean(),
  })
  .strict();

export const PriorityTierSchema = z.enum([
  "P0_INVARIANT",
  "P1_TASK_CORE",
  "P2_OPTIONAL_UI",
  "P3_SUPPLEMENTARY",
]);

export const PromptSectionSchema = z
  .object({
    id: z.string(),
    priority: PriorityTierSchema,
    title: z.string(),
    content: z.string(),
  })
  .strict();

export const GovernedBudgetResultSchema = z
  .object({
    sections: z.array(PromptSectionSchema),
    total_tokens: U32Schema,
    token_ceiling: U32Schema,
    pruned_section_ids: z.array(z.string()),
    compressed_section_ids: z.array(z.string()),
  })
  .strict();

export const CompilationInputSchema = z
  .object({
    project_id: z.string().min(1),
    system_instruction: z.string(),
    project_constraints: z.array(z.string()),
    context_slice: AuthorizedContextSliceSchema.nullable(),
    task_instructions: z.string(),
    optional_components: z.array(z.string()),
  })
  .strict();

export const CompiledPromptPayloadSchema = z
  .object({
    compiled_prompt: z.string(),
    token_count: U32Schema,
    token_ceiling: U32Schema,
    sha256_digest: z.string().regex(/^[a-fA-F0-9]{64}$/),
    pruned_sections: z.array(z.string()),
    compressed_sections: z.array(z.string()),
  })
  .strict();

export const ModelProfileTierSchema = z.enum([
  "frontier_reasoning",
  "fast_coding",
  "compact_motion",
]);

export const ReasoningEffortSchema = z.enum(["high", "medium", "low"]);

export const ToolComplexitySchema = z.enum([
  "full_nested_schema",
  "flat_or_standard_schema",
  "micro_tools",
]);

export const DegradationReasonSchema = z.enum([
  "syntax_error_retry",
  "tool_complexity_overflow",
  "disallowed_profile_constraint",
  "token_ceiling_exceeded",
]);

export const ModelToolDefinitionSchema = z
  .object({
    name: z.string().min(1),
    description: z.string(),
    parameters: z.record(z.string(), z.unknown()),
    is_micro_tool: z.boolean(),
  })
  .strict();

export const AdaptationRequestSchema = z
  .object({
    project_id: z.string().min(1),
    requested_profile: ModelProfileTierSchema,
    raw_system_prompt: z.string(),
    tools: z.array(ModelToolDefinitionSchema),
    disallowed_profiles: z.array(ModelProfileTierSchema),
    syntax_error_reported: z.boolean(),
    max_budget_override: SafeJsIntegerSchema.min(0).nullable(),
  })
  .strict();

export const AdaptationResponseSchema = z
  .object({
    active_profile: ModelProfileTierSchema,
    reasoning_effort: ReasoningEffortSchema,
    system_prompt: z.string(),
    tools: z.array(ModelToolDefinitionSchema),
    token_budget: U32Schema,
    estimated_tokens: U32Schema,
    degraded: z.boolean(),
    degradation_reason: DegradationReasonSchema.nullable(),
    sha256_digest: z.string().regex(/^[a-fA-F0-9]{64}$/),
  })
  .strict();
