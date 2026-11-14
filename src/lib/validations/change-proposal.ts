import { z } from "zod";

export const MIN_TITLE_LENGTH = 3;
export const MAX_TITLE_LENGTH = 200;
export const MAX_DESCRIPTION_LENGTH = 2000;

export const FileDiffItemSchema = z.object({
  path: z.string().min(1),
  status: z.enum(["added", "modified", "deleted"]),
  additions: z.number().int().min(0),
  deletions: z.number().int().min(0),
  patch: z.string(),
});

export const ValidationCheckResultSchema = z.object({
  checkType: z.enum(["ast_lint", "typecheck", "build", "fixture_verify"]),
  status: z.enum(["passed", "failed", "skipped"]),
  message: z.string(),
  timestamp: z.string(),
});

export const CreateProposalSchema = z.object({
  id: z.string().uuid().optional(),
  intentId: z.string().uuid().optional(),
  title: z.string().min(MIN_TITLE_LENGTH).max(MAX_TITLE_LENGTH),
  description: z.string().max(MAX_DESCRIPTION_LENGTH).default(""),
  files: z.array(FileDiffItemSchema).min(1, "Proposal must contain at least one modified file"),
});

export const ApplyProposalSchema = z.object({
  force: z.boolean().optional().default(false),
});

export const ChangeProposalResponseSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().min(1),
  projectId: z.string().min(1),
  intentId: z.string().uuid().optional(),
  title: z.string(),
  description: z.string(),
  files: z.array(FileDiffItemSchema),
  validationStatus: z.enum(["pending", "passed", "failed"]),
  validationChecks: z.array(ValidationCheckResultSchema),
  status: z.enum(["draft", "validating", "ready", "applied", "rejected"]),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type FileDiffItem = z.infer<typeof FileDiffItemSchema>;
export type ValidationCheckResult = z.infer<typeof ValidationCheckResultSchema>;
export type CreateProposalInput = z.infer<typeof CreateProposalSchema>;
export type ApplyProposalInput = z.input<typeof ApplyProposalSchema>;
export type ChangeProposalResponse = z.infer<typeof ChangeProposalResponseSchema>;

export interface RustChangeProposalV1 {
  id: string;
  tenant_id: string;
  project_id: string;
  intent_id: string | null;
  title: string;
  description: string;
  validation_status: string;
  status: string;
  created_at: string;
  updated_at: string;
}

export function toRustChangeProposalV1(proposal: ChangeProposalResponse): RustChangeProposalV1 {
  return {
    id: proposal.id,
    tenant_id: proposal.tenantId,
    project_id: proposal.projectId,
    intent_id: proposal.intentId || null,
    title: proposal.title,
    description: proposal.description,
    validation_status: proposal.validationStatus,
    status: proposal.status,
    created_at: proposal.createdAt,
    updated_at: proposal.updatedAt,
  };
}
