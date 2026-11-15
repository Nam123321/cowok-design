// @story 04.4
// @implements AC-01, AC-02 (Scope-Safe Daemon Queue Reconciliation Helpers)
import type { NodeExecutionJobDto } from "@/lib/validations/flow-execution";
import {
  DAEMON_TO_LOCAL_STATUS,
  mapDaemonDtoToJob,
  toCompositeScopeKey,
  toNodeStatusSnapshot,
  type NodeExecutionJob,
  type NodeStatusSnapshot,
} from "./keyed-parallel-types";

export interface ScopeFilter {
  projectId?: string;
  sessionId?: string;
}

export interface QueueStateBag {
  nodeQueues: Map<string, NodeExecutionJob[]>;
  activeJobs: Map<string, NodeExecutionJob>;
  fencingCounter: number;
}

export function lookupNodeStatusInQueues(
  nodeQueues: Map<string, NodeExecutionJob[]>,
  nodeId: string,
  scope?: ScopeFilter
): NodeStatusSnapshot | null {
  const direct = nodeQueues.get(nodeId);
  if (direct && direct.length > 0) return toNodeStatusSnapshot(direct[0]);
  if (scope?.projectId && scope?.sessionId) {
    const q = nodeQueues.get(toCompositeScopeKey(scope.projectId, scope.sessionId, nodeId));
    return q && q.length > 0 ? toNodeStatusSnapshot(q[0]) : null;
  }
  for (const queue of nodeQueues.values()) {
    if (queue.length === 0) continue;
    if (scope?.projectId && queue[0].projectId !== scope.projectId) continue;
    if (scope?.sessionId && queue[0].sessionId !== scope.sessionId) continue;
    if (queue[0].nodeId === nodeId) return toNodeStatusSnapshot(queue[0]);
  }
  return null;
}

export function findJobInQueues(bag: QueueStateBag, jobId: string): NodeExecutionJob | undefined {
  const active = bag.activeJobs.get(jobId);
  if (active) return active;
  for (const q of bag.nodeQueues.values()) {
    const hit = q.find((j) => j.id === jobId);
    if (hit) return hit;
  }
  return undefined;
}

function snapshotExistingJobs(nodeQueues: Map<string, NodeExecutionJob[]>): Map<string, NodeExecutionJob> {
  const out = new Map<string, NodeExecutionJob>();
  for (const q of nodeQueues.values()) {
    for (const j of q) out.set(j.id, j);
  }
  return out;
}

function pruneMatchingScopes(bag: QueueStateBag, touchedScopes: Set<string>, scopeFilter?: ScopeFilter): void {
  for (const [scopeKey, queue] of Array.from(bag.nodeQueues.entries())) {
    const sample = queue[0];
    const matchesFilter = scopeFilter?.projectId
      ? Boolean(sample && sample.projectId === scopeFilter.projectId && (!scopeFilter.sessionId || sample.sessionId === scopeFilter.sessionId))
      : touchedScopes.has(scopeKey);
    if (!matchesFilter) continue;
    for (const j of queue) bag.activeJobs.delete(j.id);
    bag.nodeQueues.delete(scopeKey);
  }
}

export function reconcileFromDaemonQueues(
  bag: QueueStateBag,
  dtos: NodeExecutionJobDto[],
  projectSeq?: number,
  scopeFilter?: ScopeFilter
): void {
  const prevById = snapshotExistingJobs(bag.nodeQueues);
  const touchedScopes = new Set(dtos.map((d) => d.compositeScopeKey || toCompositeScopeKey(d.projectId, d.sessionId, d.nodeId)));
  pruneMatchingScopes(bag, touchedScopes, scopeFilter);
  let queuedIdx = 0;
  for (const dto of dtos) {
    const status = DAEMON_TO_LOCAL_STATUS[dto.status] ?? "queued";
    if (status === "completed" || status === "error" || status === "cancelled") continue;
    bag.fencingCounter = Math.max(bag.fencingCounter, dto.fencingToken);
    const scopeKey = dto.compositeScopeKey || toCompositeScopeKey(dto.projectId, dto.sessionId, dto.nodeId);
    const q = bag.nodeQueues.get(scopeKey) ?? [];
    const job = mapDaemonDtoToJob(dto, status, status === "queued" ? ++queuedIdx : 0, projectSeq);
    const prior = prevById.get(dto.id);
    if (prior) {
      job.prompt = prior.prompt;
      job.steerHistory = prior.steerHistory;
    }
    q.push(job);
    bag.nodeQueues.set(scopeKey, q);
    if (status === "running" || status === "paused") bag.activeJobs.set(job.id, job);
  }
}

export function reconcileSingleJobInQueues(bag: QueueStateBag, dto: NodeExecutionJobDto, projectSeq?: number): void {
  const scopeKey = dto.compositeScopeKey || toCompositeScopeKey(dto.projectId, dto.sessionId, dto.nodeId);
  const prev = findJobInQueues(bag, dto.id);
  const status = DAEMON_TO_LOCAL_STATUS[dto.status] ?? "queued";
  bag.activeJobs.delete(dto.id);
  const q = (bag.nodeQueues.get(scopeKey) ?? []).filter((j) => j.id !== dto.id);
  if (status !== "completed" && status !== "error" && status !== "cancelled") {
    const job = mapDaemonDtoToJob(dto, status, status === "queued" ? q.length + 1 : 0, projectSeq ?? prev?.projectSeq);
    if (prev) {
      job.prompt = prev.prompt;
      job.steerHistory = prev.steerHistory;
    }
    q.push(job);
    if (status === "running" || status === "paused") bag.activeJobs.set(job.id, job);
  }
  if (q.length === 0) bag.nodeQueues.delete(scopeKey);
  else bag.nodeQueues.set(scopeKey, q);
  bag.fencingCounter = Math.max(bag.fencingCounter, dto.fencingToken);
}
