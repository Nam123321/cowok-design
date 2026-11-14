// @story 04.3, 04.4
// @implements AC-01, AC-02 (Types & Summary Helpers for KeyedParallelCoordinator)
import type { NodeExecutionJobDto } from "@/lib/validations/flow-execution";

export type NodeJobStatus =
  | "queued"
  | "running"
  | "paused"
  | "interrupted"
  | "completed"
  | "error"
  | "cancelled";

export interface NodeExecutionJob {
  id: string;
  projectId: string;
  sessionId: string;
  nodeId: string;
  compositeScopeKey: string;
  daemonNonce?: string;
  prompt: string;
  status: NodeJobStatus;
  queuePosition: number;
  fencingToken: number;
  projectSeq?: number;
  createdAt: number;
  startedAt?: number;
  completedAt?: number;
  errorMessage?: string;
  steerHistory: Array<{ timestamp: number; instruction: string }>;
}

export interface NodeStatusSnapshot {
  status: NodeJobStatus;
  queuePosition: number;
  jobId: string;
  fencingToken: number;
  projectSeq?: number;
}

export interface NodeQueueSummary {
  runningCount: number;
  pausedCount: number;
  interruptedCount: number;
  queuedCount: number;
  totalActive: number;
  nodeStatusMap: Record<string, NodeStatusSnapshot>;
}

export type QueueListener = (summary: NodeQueueSummary) => void;

export const DAEMON_TO_LOCAL_STATUS: Record<string, NodeJobStatus> = {
  QUEUED: "queued",
  RUNNING: "running",
  PAUSED: "paused",
  INTERRUPTED: "interrupted",
  COMPLETED: "completed",
  FAILED: "error",
  CANCELLED: "cancelled",
};

export function toNodeStatusSnapshot(head: NodeExecutionJob): NodeStatusSnapshot {
  return {
    status: head.status,
    queuePosition: head.status === "queued" ? Math.max(1, head.queuePosition) : 0,
    jobId: head.id,
    fencingToken: head.fencingToken,
    projectSeq: head.projectSeq,
  };
}

export function buildQueueSummary(nodeQueues: Map<string, NodeExecutionJob[]>): NodeQueueSummary {
  const counts: Record<"running" | "paused" | "interrupted" | "queued", number> = {
    running: 0,
    paused: 0,
    interrupted: 0,
    queued: 0,
  };
  const nodeStatusMap: Record<string, NodeStatusSnapshot> = {};

  for (const [nodeId, queue] of nodeQueues.entries()) {
    if (queue.length === 0) continue;
    const head = queue[0];
    if (head.status in counts) {
      counts[head.status as keyof typeof counts] += 1;
    }
    nodeStatusMap[nodeId] = toNodeStatusSnapshot(head);
    counts.queued += Math.max(0, queue.length - 1);
  }

  return {
    runningCount: counts.running,
    pausedCount: counts.paused,
    interruptedCount: counts.interrupted,
    queuedCount: counts.queued,
    totalActive: counts.running + counts.paused + counts.queued,
    nodeStatusMap,
  };
}

export function mapDaemonDtoToJob(dto: NodeExecutionJobDto, status: NodeJobStatus, queuePosition: number, projectSeq?: number): NodeExecutionJob {
  return {
    id: dto.id,
    projectId: dto.projectId,
    sessionId: dto.sessionId,
    nodeId: dto.nodeId,
    compositeScopeKey: dto.compositeScopeKey,
    daemonNonce: dto.daemonNonce,
    prompt: "",
    status,
    queuePosition,
    fencingToken: dto.fencingToken,
    projectSeq,
    createdAt: dto.createdAt,
    steerHistory: [],
  };
}
