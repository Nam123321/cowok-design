import { z } from "zod";

export const MAX_COMMENT_LENGTH = 2000;

export const AnchorStatusSchema = z.enum(["active", "stale", "resolved"]);
export type AnchorStatus = z.infer<typeof AnchorStatusSchema>;

export const CreateElementCommentSchema = z.object({
  elementFfId: z.string().min(1),
  author: z.string().min(1),
  content: z.string().min(1).max(MAX_COMMENT_LENGTH),
  domHash: z.string().min(1),
});
export type CreateElementCommentInput = z.infer<typeof CreateElementCommentSchema>;

export const UpdateElementCommentSchema = z.object({
  content: z.string().min(1).max(MAX_COMMENT_LENGTH).optional(),
  anchorStatus: AnchorStatusSchema.optional(),
  elementFfId: z.string().optional(),
  domHash: z.string().optional(),
});
export type UpdateElementCommentInput = z.infer<typeof UpdateElementCommentSchema>;

export const ElementCommentResponseSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string(),
  nodeId: z.string(),
  elementFfId: z.string(),
  author: z.string(),
  content: z.string(),
  domHash: z.string(),
  anchorStatus: AnchorStatusSchema,
  createdAt: z.string(),
  updatedAt: z.string(),
});
export type ElementCommentResponse = z.infer<typeof ElementCommentResponseSchema>;

export const StaleVerificationReportSchema = z.object({
  commentId: z.string(),
  elementFfId: z.string(),
  expectedDomHash: z.string(),
  actualDomHash: z.string().nullable(),
  isStale: z.boolean(),
  reason: z.enum(["ELEMENT_MISSING", "HASH_MISMATCH", "SYNCHRONIZED"]),
});
export type StaleVerificationReport = z.infer<typeof StaleVerificationReportSchema>;
