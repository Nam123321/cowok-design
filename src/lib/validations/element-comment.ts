// @story 04.3, 04.5
// @implements AC-03 (ElementComment CAS Schemas & STALE_MAPPING Contract)
import { z } from "zod";

export const MAX_COMMENT_LENGTH = 2000;

export const AnchorStatusSchema = z.enum(["active", "stale", "resolved"]);
export type AnchorStatus = z.infer<typeof AnchorStatusSchema>;

export const AnchorKindSchema = z.enum([
  "html_element",
  "marquee_box",
  "node_card",
  "text_quote",
]);
export type AnchorKind = z.infer<typeof AnchorKindSchema>;

export const CreateElementCommentSchema = z.object({
  elementFfId: z.string().min(1),
  author: z.string().min(1),
  content: z.string().min(1).max(MAX_COMMENT_LENGTH),
  domHash: z.string().min(1),
  artifactSha256: z.string().default(""),
  anchorKind: AnchorKindSchema.default("html_element"),
  expectedArtifactSha256: z.string().optional(),
  expectedDomHash: z.string().optional(),
});
export type CreateElementCommentInput = z.input<typeof CreateElementCommentSchema>;

export const UpdateElementCommentSchema = z.object({
  content: z.string().min(1).max(MAX_COMMENT_LENGTH).optional(),
  anchorStatus: AnchorStatusSchema.optional(),
  elementFfId: z.string().optional(),
  domHash: z.string().optional(),
  artifactSha256: z.string().optional(),
  anchorKind: AnchorKindSchema.optional(),
  expectedArtifactSha256: z.string().optional(),
  expectedDomHash: z.string().optional(),
  reanchor: z.boolean().optional(),
});
export type UpdateElementCommentInput = z.input<typeof UpdateElementCommentSchema>;

export const ElementCommentResponseSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string(),
  nodeId: z.string(),
  elementFfId: z.string(),
  author: z.string(),
  content: z.string(),
  domHash: z.string(),
  artifactSha256: z.string().default(""),
  anchorKind: AnchorKindSchema.default("html_element"),
  anchorStatus: AnchorStatusSchema,
  createdAt: z.string(),
  updatedAt: z.string(),
});
export type ElementCommentResponse = z.infer<typeof ElementCommentResponseSchema>;

export const StaleMappingErrorPayloadSchema = z.object({
  code: z.string().optional(),
  error: z.literal("STALE_MAPPING").optional(),
  message: z.string(),
  currentArtifactSha256: z.string().optional(),
  currentDomHash: z.string().optional(),
});
export type StaleMappingErrorPayload = z.infer<typeof StaleMappingErrorPayloadSchema>;

export const StaleVerificationReportSchema = z.object({
  commentId: z.string(),
  elementFfId: z.string(),
  expectedDomHash: z.string(),
  actualDomHash: z.string().nullable(),
  isStale: z.boolean(),
  reason: z.enum(["ELEMENT_MISSING", "HASH_MISMATCH", "SYNCHRONIZED"]),
});
export type StaleVerificationReport = z.infer<typeof StaleVerificationReportSchema>;
