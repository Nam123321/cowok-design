import { z } from "zod";
import { SafeJsIntegerSchema, U32Schema } from "./primitives.js";

export const CANONICAL_SPEC_KEYS = [
  "01-product-overview.md",
  "02-database-spec.md",
  "03-ui-ux-spec.md",
  "04-architecture.md",
  "05-epics-and-stories.md",
  "06-cross-dependencies.md",
  "07-api-guidelines.md",
  "08-security-tenancy.md",
  "09-data-and-copy.md",
] as const;

export const CANONICAL_SPEC_KEYS_BY_NUMBER: Record<number, (typeof CANONICAL_SPEC_KEYS)[number]> = {
  1: "01-product-overview.md",
  2: "02-database-spec.md",
  3: "03-ui-ux-spec.md",
  4: "04-architecture.md",
  5: "05-epics-and-stories.md",
  6: "06-cross-dependencies.md",
  7: "07-api-guidelines.md",
  8: "08-security-tenancy.md",
  9: "09-data-and-copy.md",
};

export const CanonicalSpecKeySchema = z.enum(CANONICAL_SPEC_KEYS);

export const ProvenanceStatusSchema = z.enum(["CONFIRMED", "INFERRED", "MISSING"]);

export const ProjectSpecSchema = z
  .object({
    id: z.string().uuid(),
    project_id: z.string().uuid(),
    spec_key: CanonicalSpecKeySchema,
    status: ProvenanceStatusSchema,
    updated_at: SafeJsIntegerSchema,
  })
  .strict();

export const SpecRevisionSchema = z
  .object({
    id: z.string().uuid(),
    spec_id: z.string().uuid(),
    revision_number: U32Schema.refine((v) => v > 0, "revision_number must be > 0"),
    content_hash: z
      .string()
      .regex(/^(sha256-)?[a-fA-F0-9]{64}$/, "content_hash must be a valid 64-character lowercase hex SHA-256 string"),
    created_at: SafeJsIntegerSchema,
  })
  .strict();

export const SpecProvenanceSummarySchema = z
  .object({
    project_id: z.string().uuid(),
    confirmed_count: U32Schema,
    inferred_count: U32Schema,
    missing_count: U32Schema,
    overall_status: ProvenanceStatusSchema,
  })
  .strict();

export const ProjectSchema = z
  .object({
    id: z.string().uuid(),
    name: z.string().min(1),
    slug: z.string().min(1),
    created_at: SafeJsIntegerSchema,
    updated_at: SafeJsIntegerSchema,
  })
  .strict();

export const DesignIntentV1Schema = z
  .object({
    id: z.string().uuid(),
    project_id: z.string().uuid(),
    visual_style: z.string(),
    color_palette: z.array(z.string()),
    typography: z.array(z.string()),
    key_principles: z.array(z.string()),
  })
  .strict();

export const ConstraintProfileSchema = z
  .object({
    id: z.string().uuid(),
    project_id: z.string().uuid(),
    max_complexity_score: U32Schema,
    enforce_clean_code: z.boolean(),
    forbidden_patterns: z.array(z.string()),
    target_framework: z.string(),
  })
  .strict();

export const ScreenStateSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    route_path: z.string(),
    components: z.array(z.string()),
  })
  .strict();

export const TransitionSchema = z
  .object({
    id: z.string(),
    from_state: z.string(),
    to_state: z.string(),
    trigger_action: z.string(),
  })
  .strict();

export const FlowGraphSchema = z
  .object({
    id: z.string().uuid(),
    project_id: z.string().uuid(),
    states: z.array(ScreenStateSchema),
    transitions: z.array(TransitionSchema),
  })
  .strict();

export const PresentationDeckSchema = z
  .object({
    id: z.string().uuid(),
    project_id: z.string().uuid(),
    title: z.string(),
    slides_count: U32Schema,
    last_exported_at: SafeJsIntegerSchema.nullable().optional(),
  })
  .strict();

export const SkillPluginSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    version: z.string(),
    entry_point: z.string(),
    permissions: z.array(z.string()),
  })
  .strict();

export const BotJobSchema = z
  .object({
    id: z.string().uuid(),
    task_type: z.string(),
    payload_hash: z.string(),
    status: z.string(),
    enqueued_at: SafeJsIntegerSchema,
  })
  .strict();

export const GateReportSchema = z
  .object({
    gate_name: z.string(),
    passed: z.boolean(),
    checker_id: z.string(),
    timestamp: SafeJsIntegerSchema,
    evidence_hash: z.string(),
  })
  .strict();

export const ProjectConstraintProfileSchema = ConstraintProfileSchema;
export const FleetTaskSchema = BotJobSchema;
