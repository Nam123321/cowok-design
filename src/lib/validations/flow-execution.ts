// @story 04.4
// @implements AC-01, AC-02, AC-04 (Strict Zod Schemas for Flow Coordinator & Node Sessions)
import { z } from "zod";
import { ExecutionEventDtoSchema } from "./workbench-settings";

export const NodeExecutionJobStatusSchema = z.enum([
  "QUEUED",
  "RUNNING",
  "PAUSED",
  "INTERRUPTED",
  "COMPLETED",
  "FAILED",
  "CANCELLED",
]);

export const NodeExecutionJobDtoSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    sessionId: z.string().min(1),
    nodeId: z.string().min(1),
    compositeScopeKey: z.string(),
    daemonNonce: z.string(),
    status: NodeExecutionJobStatusSchema,
    fencingToken: z.number().int().nonnegative(),
    priority: z.number().int(),
    staleAfterTimestamp: z.number().int().nullable().optional(),
    createdAt: z.number().int(),
    updatedAt: z.number().int(),
  })
  .strict();

export const NodeChatSessionDtoSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    chatSessionId: z.string().min(1),
    nodeId: z.string().min(1),
    compositeScopeKey: z.string(),
    title: z.string(),
    status: z.string(),
    unreadCount: z.number().int().nonnegative(),
    lastMessageId: z.string().nullable().optional(),
    updatedAt: z.number().int(),
  })
  .strict();

export const EnqueueNodeJobInputSchema = z
  .object({
    sessionId: z.string().min(1),
    nodeIds: z.array(z.string().min(1)).min(1),
    prompt: z.string().min(1),
    mode: z.enum(["parallel", "queue"]).default("parallel"),
  })
  .strict();

export const SteerNodeJobInputSchema = z
  .object({
    jobId: z.string().min(1),
    expectedFencingToken: z.number().int().nonnegative(),
    instruction: z.string().min(1),
    pgidReapRequested: z.boolean().default(true),
  })
  .strict();

export const TransitionNodeJobInputSchema = z
  .object({
    jobId: z.string().min(1),
    expectedFencingToken: z.number().int().nonnegative(),
  })
  .strict();

export const MarkNodeSessionReadInputSchema = z
  .object({
    compositeScopeKey: z.string().min(1).optional(),
    sessionId: z.string().min(1).optional(),
    nodeId: z.string().min(1).optional(),
  })
  .strict();

export const MarkNodeSessionReadResponseSchema = z
  .object({
    ok: z.boolean(),
    compositeScopeKey: z.string(),
    clearedRows: z.number().int().nonnegative(),
  })
  .strict();

export const PostJobEventV2InputSchema = z
  .object({
    jobId: z.string().min(1),
    nodeId: z.string().min(1),
    sessionId: z.string().min(1).optional(),
    eventKind: z.string().min(1),
    payloadText: z.string().default(""),
    jobStatus: NodeExecutionJobStatusSchema.optional(),
    expectedFencingToken: z.number().int().nonnegative().optional(),
  })
  .strict();

export const EnqueueNodeJobsResponseSchema = z
  .object({
    jobs: z.array(NodeExecutionJobDtoSchema),
    activeSlotCount: z.number().int().nonnegative(),
  })
  .strict();

export const NodeSessionsResponseSchema = z
  .object({
    sessions: z.array(NodeChatSessionDtoSchema),
    jobs: z.array(NodeExecutionJobDtoSchema),
    activeSlotCount: z.number().int().nonnegative(),
    maxConcurrent: z.number().int().positive(),
  })
  .strict();

export type NodeExecutionJobDto = z.infer<typeof NodeExecutionJobDtoSchema>;
export type NodeChatSessionDto = z.infer<typeof NodeChatSessionDtoSchema>;
export type EnqueueNodeJobInput = z.infer<typeof EnqueueNodeJobInputSchema>;
export type SteerNodeJobInput = z.infer<typeof SteerNodeJobInputSchema>;
export type TransitionNodeJobInput = z.infer<typeof TransitionNodeJobInputSchema>;
export type PostJobEventV2Input = z.infer<typeof PostJobEventV2InputSchema>;
export type EnqueueNodeJobsResponse = z.infer<typeof EnqueueNodeJobsResponseSchema>;
export type NodeSessionsResponse = z.infer<typeof NodeSessionsResponseSchema>;
export type MarkNodeSessionReadInput = z.infer<typeof MarkNodeSessionReadInputSchema>;
export type MarkNodeSessionReadResponse = z.infer<typeof MarkNodeSessionReadResponseSchema>;
export { ExecutionEventDtoSchema };
