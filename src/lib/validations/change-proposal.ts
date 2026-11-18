import { z } from "zod";
import {
  CreateProposalTradeoffSchema,
  MAX_PROPOSAL_TRADEOFFS,
  ProposalGateReceiptSchema,
  ProposalTradeoffSchema,
  VisualDiffResultSchema,
} from "./change-proposal-extensions";

export * from "./change-proposal-extensions";

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
  nodeId: z.string().min(1).optional(),
  batchId: z.string().min(1).optional(),
  nodeIds: z.array(z.string().min(1)).optional(),
  contentMode: z.enum(["full_file", "unified_diff", "ast_patch"]).optional(),
  baseFileHashes: z.record(z.string()).optional(),
  tradeoffs: z.array(CreateProposalTradeoffSchema).max(MAX_PROPOSAL_TRADEOFFS).optional(),
  title: z.string().min(MIN_TITLE_LENGTH).max(MAX_TITLE_LENGTH),
  description: z.string().max(MAX_DESCRIPTION_LENGTH).default(""),
  files: z.array(FileDiffItemSchema).min(1, "Proposal must contain at least one modified file"),
});

export const ApplyProposalSchema = z.object({
  force: z.boolean().optional().default(false),
});

export const ValidateProposalSchema = z.object({
  dryRun: z.boolean().optional().default(false),
  rebaseBaseHashes: z.boolean().optional().default(false),
});

export const RejectProposalSchema = z.object({
  reason: z.string().max(MAX_DESCRIPTION_LENGTH).optional(),
});

export const ChangeProposalResponseSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().min(1),
  projectId: z.string().min(1),
  intentId: z.string().uuid().nullable().optional(),
  nodeId: z.string().nullable().optional(),
  batchId: z.string().nullable().optional(),
  compositeScopeKey: z.string().optional(),
  contentMode: z.string().optional(),
  baseFileHashes: z.record(z.string()).optional(),
  nodeIds: z.array(z.string()).optional(),
  gateReceiptId: z.string().nullable().optional(),
  gateReceipt: ProposalGateReceiptSchema.nullable().optional(),
  visualDiff: VisualDiffResultSchema.nullable().optional(),
  tradeoffs: z.array(ProposalTradeoffSchema).optional(),
  title: z.string(),
  description: z.string(),
  files: z.array(FileDiffItemSchema),
  validationStatus: z.enum(["pending", "passed", "failed"]),
  validationChecks: z.array(ValidationCheckResultSchema),
  status: z.enum(["draft", "validating", "ready", "stale", "applied", "rejected"]),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type FileDiffItem = z.infer<typeof FileDiffItemSchema>;
export type ValidationCheckResult = z.infer<typeof ValidationCheckResultSchema>;
export type CreateProposalInput = z.input<typeof CreateProposalSchema>;
export type ApplyProposalInput = z.input<typeof ApplyProposalSchema>;
export type ValidateProposalInput = z.input<typeof ValidateProposalSchema>;
export type RejectProposalInput = z.infer<typeof RejectProposalSchema>;
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
