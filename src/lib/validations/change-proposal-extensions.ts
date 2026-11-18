// @story 05.2
import { z } from "zod";

export const MAX_PROPOSAL_TRADEOFFS = 12;
export const MAX_VISUAL_DIFF_WIDTH = 1280;
export const MAX_VISUAL_DIFF_HEIGHT = 800;
export const MAX_ASK_PROMPT_LENGTH = 4000;

export const ProposalTradeoffDecisionEnum = z.enum([
  "pending",
  "agreed",
  "disagreed_instead",
  "asked",
  "skipped",
]);

export const InsteadPatchScalarSchema = z.union([
  z.string(),
  z.number(),
  z.boolean(),
  z.null(),
]);

export const ProposalTradeoffSchema = z.object({
  id: z.string().min(1),
  proposalId: z.string().min(1),
  nodeId: z.string().nullable().optional(),
  elementFfId: z.string().min(1),
  badgeIndex: z.number().int().min(1).max(MAX_PROPOSAL_TRADEOFFS),
  designNote: z.string(),
  insteadLabel: z.string(),
  insteadPatch: z
    .union([z.string(), z.record(InsteadPatchScalarSchema)])
    .nullable()
    .optional()
    .default(""),
  decision: ProposalTradeoffDecisionEnum,
  askPrompt: z.string().nullable().optional(),
  decidedAt: z.union([z.string(), z.number()]).nullable().optional(),
  createdAt: z.union([z.string(), z.number()]),
});

export const CreateProposalTradeoffSchema = z.object({
  nodeId: z.string().min(1).optional(),
  elementFfId: z.string().min(1),
  badgeIndex: z.number().int().min(1).max(MAX_PROPOSAL_TRADEOFFS),
  designNote: z.string().min(1),
  insteadLabel: z.string().min(1),
  insteadPatch: z.union([z.string(), z.record(InsteadPatchScalarSchema)]),
});

export const VisualDiffBboxSchema = z.object({
  x: z.number().int().min(0),
  y: z.number().int().min(0),
  width: z.number().int().min(0),
  height: z.number().int().min(0),
});

export const VisualDiffResultSchema = z.object({
  status: z.enum(["COMPUTED", "RASTERIZER_UNAVAILABLE"]),
  ssim: z.number().min(0).max(1),
  mismatchRatio: z.number().min(0).max(1),
  mismatchedPixels: z.number().int().min(0),
  totalPixels: z.number().int().min(0),
  width: z.number().int().min(0),
  height: z.number().int().min(0),
  changedBbox: VisualDiffBboxSchema.nullable().optional(),
  reason: z.string().nullable().optional(),
  structuralDiffSummary: z.array(z.string()).default([]),
});

export const ProposalGateReceiptSchema = z.object({
  id: z.string().min(1),
  proposalId: z.string().min(1),
  diffSha256: z.string().min(1),
  constraintsSha256: z.string().min(1),
  workspaceHeadSha256: z.string().min(1),
  exitCode: z.number().int(),
  receiptHmac: z.string().min(1),
  createdAt: z.union([z.string(), z.number()]),
});

export const ComputeVisualDiffSchema = z.object({
  beforeRgbaBase64: z.string().optional(),
  afterRgbaBase64: z.string().optional(),
  width: z.number().int().min(1).max(MAX_VISUAL_DIFF_WIDTH).optional(),
  height: z.number().int().min(1).max(MAX_VISUAL_DIFF_HEIGHT).optional(),
  threshold: z.number().min(0).max(1).optional(),
});

export const DecideTradeoffSchema = z.object({
  decision: z.enum(["agreed", "disagreed_instead", "asked", "skipped"]),
  askPrompt: z.string().max(MAX_ASK_PROMPT_LENGTH).optional(),
});

export type ProposalTradeoffDecision = z.infer<typeof ProposalTradeoffDecisionEnum>;
export type ProposalTradeoff = z.infer<typeof ProposalTradeoffSchema>;
export type CreateProposalTradeoffInput = z.infer<typeof CreateProposalTradeoffSchema>;
export type VisualDiffBbox = z.infer<typeof VisualDiffBboxSchema>;
export type VisualDiffResult = z.infer<typeof VisualDiffResultSchema>;
export type ProposalGateReceipt = z.infer<typeof ProposalGateReceiptSchema>;
export type ComputeVisualDiffInput = z.input<typeof ComputeVisualDiffSchema>;
export type DecideTradeoffInput = z.input<typeof DecideTradeoffSchema>;
