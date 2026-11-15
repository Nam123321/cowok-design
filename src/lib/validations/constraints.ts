// @story 03.2
import { z } from "zod";
import flagshipIconManifestRaw from "./flagship-icon-manifest.json";

export const FLAGSHIP_MANIFEST_SENTINEL = "builtin://flagship";
export const MAX_MANIFEST_ICONS = 2048;
export const MAX_IDENTIFIER_LEN = 64;

export const IconManifestEntrySchema = z
  .object({
    name: z.string().min(1).max(MAX_IDENTIFIER_LEN),
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
    count: z.number().int().nonnegative().max(MAX_MANIFEST_ICONS),
    icons: z.array(IconManifestEntrySchema).max(MAX_MANIFEST_ICONS),
  })
  .refine((m) => m.count === m.icons.length, {
    message: "Icon manifest count must match icons length",
  });

export const ValidateIconInputSchema = z
  .object({
    icon: z.string().max(MAX_IDENTIFIER_LEN),
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
    allowed_font_families: z.array(z.string().min(1)),
    allowed_font_weights: z.array(z.number().int().positive()),
    allowed_font_sizes: z.array(z.string().min(1)),
    min_wcag_contrast: z.number().positive(),
  })
  .strict();

export const ValidateTypographyInputSchema = z
  .object({
    font_family: z.string().min(1),
    font_weight: z.number().int().positive(),
    font_size: z.string().min(1).nullable().optional(),
    contrast_ratio: z.number().nonnegative(),
  })
  .strict();

export const TypographyValidationReportSchema = z
  .object({
    is_valid: z.boolean(),
    errors: z.array(z.string()),
  })
  .strict();

export const ComponentRegistryEntrySchema = z
  .object({
    name: z.string().min(1).max(MAX_IDENTIFIER_LEN),
    source_spec: z.string().nullable().optional(),
    allowed_props: z.array(z.string()).default([]),
  })
  .strict();

export const ComponentRegistryContractSchema = z
  .object({
    components: z.array(ComponentRegistryEntrySchema).default([]),
    forbid_unregistered: z.boolean().default(false),
  })
  .strict();

export const ValidateComponentInputSchema = z
  .object({
    component: z.string().max(MAX_IDENTIFIER_LEN),
  })
  .strict();

export const ComponentValidationReportSchema = z
  .object({
    requested_component: z.string(),
    is_valid: z.boolean(),
    suggestions: z.array(z.string()),
  })
  .strict();

export const ProjectDesignContractsSchema = z
  .object({
    icon_manifest_path: z.string().min(1),
    allowed_icon_set: z.array(z.string()).default([]),
    typography_contract: TypographyContractSchema,
    component_registry: ComponentRegistryContractSchema,
  })
  .strict();

export const ConstraintViolationSchema = z
  .object({
    rule_code: z.string(),
    severity: z.string(),
    message: z.string(),
    line: z.number().int().nonnegative().nullable().optional(),
    column: z.number().int().nonnegative().nullable().optional(),
  })
  .strict();

export const LintSourceInputSchema = z
  .object({
    source: z.string().min(1),
  })
  .strict();

export const LintSourceResponseSchema = z
  .object({
    violations: z.array(ConstraintViolationSchema),
    passed: z.boolean(),
  })
  .strict();

export type IconManifestEntry = z.infer<typeof IconManifestEntrySchema>;
export type IconManifest = z.infer<typeof IconManifestSchema>;
export type ValidateIconInput = z.infer<typeof ValidateIconInputSchema>;
export type IconValidationReport = z.infer<typeof IconValidationReportSchema>;
export type TypographyContract = z.infer<typeof TypographyContractSchema>;
export type ValidateTypographyInput = z.infer<typeof ValidateTypographyInputSchema>;
export type TypographyValidationReport = z.infer<typeof TypographyValidationReportSchema>;
export type ComponentRegistryEntry = z.infer<typeof ComponentRegistryEntrySchema>;
export type ComponentRegistryContract = z.infer<typeof ComponentRegistryContractSchema>;
export type ValidateComponentInput = z.infer<typeof ValidateComponentInputSchema>;
export type ComponentValidationReport = z.infer<typeof ComponentValidationReportSchema>;
export type ProjectDesignContracts = z.infer<typeof ProjectDesignContractsSchema>;
export type ConstraintViolation = z.infer<typeof ConstraintViolationSchema>;
export type LintSourceInput = z.infer<typeof LintSourceInputSchema>;
export type LintSourceResponse = z.infer<typeof LintSourceResponseSchema>;

export const DEFAULT_SEEDED_ICON_MANIFEST: IconManifest = IconManifestSchema.parse(
  flagshipIconManifestRaw,
);
