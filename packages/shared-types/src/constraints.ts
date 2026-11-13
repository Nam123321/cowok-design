import { z } from "zod";
import { SafeJsIntegerSchema } from "./primitives.js";

export const ConstraintSeveritySchema = z.enum(["INFO", "WARN", "BLOCKER"]);

export const ActiveConstraintSchema = z
  .object({
    rule_code: z.string().min(1),
    severity: z.string().min(1),
    config_json: z.string(),
  })
  .strict();

export const ConstraintViolationSchema = z
  .object({
    rule_code: z.string().min(1),
    severity: z.string().min(1),
    message: z.string(),
    line: z.number().int().positive().nullable().optional(),
    column: z.number().int().positive().nullable().optional(),
  })
  .strict();

export const IconManifestEntrySchema = z
  .object({
    name: z.string().min(1),
    tier: z.string().default(""),
    file: z.string().default(""),
    source: z.string().default(""),
    tags: z.array(z.string()).default([]),
  })
  .strict();

export const IconManifestSchema = z
  .object({
    name: z.string().min(1),
    status: z.string().nullable().optional(),
    count: SafeJsIntegerSchema.min(0),
    icons: z.array(IconManifestEntrySchema).default([]),
  })
  .strict();

export const IconValidationReportSchema = z
  .object({
    requested_icon: z.string(),
    is_valid: z.boolean(),
    suggestions: z.array(z.string()),
  })
  .strict();

export const TypographyContractSchema = z
  .object({
    allowed_font_families: z.array(z.string()),
    allowed_font_weights: z.array(SafeJsIntegerSchema.min(1)),
    allowed_font_sizes: z.array(z.string()),
    min_wcag_contrast: z.number().positive(),
  })
  .strict();

export const ComponentRegistryEntrySchema = z
  .object({
    name: z.string().min(1),
    source_spec: z.string().nullable(),
    allowed_props: z.array(z.string()),
  })
  .strict();

export const ComponentRegistryContractSchema = z
  .object({
    components: z.array(ComponentRegistryEntrySchema),
    forbid_unregistered: z.boolean(),
  })
  .strict();

export const ComponentValidationReportSchema = z
  .object({
    requested_component: z.string(),
    is_valid: z.boolean(),
    suggestions: z.array(z.string()),
  })
  .strict();

export const TypographyValidationReportSchema = z
  .object({
    is_valid: z.boolean(),
    errors: z.array(z.string()),
  })
  .strict();

export const ProjectDesignContractsSchema = z
  .object({
    icon_manifest_path: z.string(),
    allowed_icon_set: z.array(z.string()),
    typography_contract: TypographyContractSchema,
    component_registry: ComponentRegistryContractSchema,
  })
  .strict();

export const PatchKindSchema = z.enum([
  "text_content",
  "metric_value",
  "css_variable",
  "attribute_value",
]);

export const SurgicalPatchRequestSchema = z
  .object({
    ff_id: z.string().min(1),
    patch_kind: PatchKindSchema,
    new_value: z.string(),
    source: z.string(),
    target_attribute: z.string().nullable().optional(),
  })
  .strict();

export const SurgicalPatchResponseSchema = z
  .object({
    ff_id: z.string().min(1),
    success: z.boolean(),
    original_text: z.string().nullable().optional(),
    patched_source: z.string(),
    execution_time_micros: SafeJsIntegerSchema.min(0),
  })
  .strict();

export const AstInjectionRequestSchema = z
  .object({
    source: z.string(),
    component_name: z.string().nullable().optional(),
  })
  .strict();

export const AstInjectionResponseSchema = z
  .object({
    total_elements_tagged: SafeJsIntegerSchema.min(0),
    tagged_source: z.string(),
    generated_ids: z.array(z.string()),
  })
  .strict();
