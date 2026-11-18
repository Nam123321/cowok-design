import { z } from "zod";

export const MIN_PROMPT_LENGTH = 1;
export const MAX_PROMPT_LENGTH = 4000;
export const AMBIGUITY_THRESHOLD = 0.45;
export const MAX_VARIANTS_COUNT = 3;

export const ClarificationVariantSchema = z.object({
  id: z.string().min(1),
  title: z.string().min(1),
  description: z.string().min(1),
  pros: z.array(z.string()),
  cons: z.array(z.string()),
  previewPayload: z
    .record(z.union([z.string(), z.number(), z.boolean(), z.null(), z.array(z.string())]))
    .optional(),
});

export const GapQuestionOptionSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  description: z.string().optional(),
  previewThumbnail: z.string().optional(),
});
export type GapQuestionOption = z.infer<typeof GapQuestionOptionSchema>;

export const GapQuestionSchema = z.object({
  id: z.string().min(1),
  question: z.string().min(1),
  category: z.string().min(1),
  options: z.array(GapQuestionOptionSchema).min(1),
  defaultOptionId: z.string().optional(),
});
export type GapQuestion = z.infer<typeof GapQuestionSchema>;

export const CompileIntentSchema = z.object({
  rawPrompt: z.string().min(MIN_PROMPT_LENGTH).max(MAX_PROMPT_LENGTH),
  sourceAnchorCommentId: z.string().optional(),
  targetNodeId: z.string().optional(),
  executeImmediately: z.boolean().optional(),
  idempotencyKey: z.string().optional(),
});

export const ClarificationSelectSchema = z.object({
  intentId: z.string().uuid(),
  selectedVariantId: z.string().min(1),
});

export const DesignIntentResponseSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().min(1),
  projectId: z.string().min(1),
  rawPrompt: z.string(),
  intentType: z.enum(["create_screen", "update_flow", "edit_slide", "extract_rea", "general_refactor"]),
  routingMode: z.enum(["direct_edit", "full_plan", "clarification_required"]).default("direct_edit").optional(),
  targetScope: z.object({
    nodeIds: z.array(z.string()),
    screenKeys: z.array(z.string()),
    components: z.array(z.string()),
  }),
  ambiguityScore: z.number().min(0).max(1),
  riskScore: z.number().min(0).max(1),
  clarificationVariants: z.array(ClarificationVariantSchema).max(MAX_VARIANTS_COUNT).optional(),
  selectedVariantId: z.string().nullable().optional(),
  gapQuestions: z.array(GapQuestionSchema).optional(),
  status: z.enum(["clarifying", "ready", "executed"]),
  revision: z.number().int().min(1).default(1).optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type ClarificationVariant = z.infer<typeof ClarificationVariantSchema>;
export type CompileIntentInput = z.infer<typeof CompileIntentSchema>;
export type ClarificationSelectInput = z.infer<typeof ClarificationSelectSchema>;
export type DesignIntentResponse = z.infer<typeof DesignIntentResponseSchema>;

export interface RustDesignIntentV1 {
  id: string;
  project_id: string;
  visual_style: string;
  color_palette: string[];
  typography: string[];
  key_principles: string[];
}

export function toRustDesignIntentV1(
  intent: DesignIntentResponse,
  tokens?: {
    visualStyle?: string;
    colorPalette?: string[];
    typography?: string[];
  }
): RustDesignIntentV1 {
  return {
    id: intent.id,
    project_id: intent.projectId,
    visual_style: tokens?.visualStyle ?? "design-token-governed",
    color_palette: tokens?.colorPalette?.length
      ? tokens.colorPalette
      : ["var(--accent-primary)", "var(--accent-success)", "var(--accent-warning)"],
    typography: tokens?.typography?.length ? tokens.typography : ["system-ui", "sans-serif"],
    key_principles: [
      `Intent: ${intent.intentType}`,
      `Prompt: ${intent.rawPrompt}`,
      `Ambiguity: ${intent.ambiguityScore}`,
    ],
  };
}
