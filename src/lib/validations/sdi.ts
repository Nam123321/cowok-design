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
  previewPayload: z.record(z.union([z.string(), z.number(), z.boolean(), z.null()])).optional(),
});

export const CompileIntentSchema = z.object({
  rawPrompt: z.string().min(MIN_PROMPT_LENGTH).max(MAX_PROMPT_LENGTH),
  sourceAnchorCommentId: z.string().optional(),
  targetNodeId: z.string().optional(),
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
  targetScope: z.object({
    nodeIds: z.array(z.string()),
    screenKeys: z.array(z.string()),
    components: z.array(z.string()),
  }),
  ambiguityScore: z.number().min(0).max(1),
  riskScore: z.number().min(0).max(1),
  clarificationVariants: z.array(ClarificationVariantSchema).max(MAX_VARIANTS_COUNT).optional(),
  status: z.enum(["clarifying", "ready", "executed"]),
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

export function toRustDesignIntentV1(intent: DesignIntentResponse): RustDesignIntentV1 {
  return {
    id: intent.id,
    project_id: intent.projectId,
    visual_style: "clean-modern",
    color_palette: ["#3B82F6", "#10B981", "#F59E0B"],
    typography: ["Inter", "sans-serif"],
    key_principles: [
      `Intent: ${intent.intentType}`,
      `Prompt: ${intent.rawPrompt}`,
      `Ambiguity: ${intent.ambiguityScore}`,
    ],
  };
}
