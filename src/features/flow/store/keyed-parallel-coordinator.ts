// @story 04.3, 04.4
// @implements AC-01, AC-02 (Persistent KeyedParallelCoordinator with Scope-Safe Reconciliation)
import type { NodeExecutionJobDto } from "@/lib/validations/flow-execution";
import {
  buildQueueSummary,
  toCompositeScopeKey,
  type NodeExecutionJob,
  type NodeJobStatus,
  type NodeQueueSummary,
  type NodeStatusSnapshot,
  type QueueListener,
} from "./keyed-parallel-types";
import {
  findJobInQueues,
  lookupNodeStatusInQueues,
  reconcileFromDaemonQueues,
  reconcileSingleJobInQueues,
  type QueueStateBag,
  type ScopeFilter,
} from "./keyed-parallel-reconcile";

export type {
  NodeExecutionJob,
  NodeJobStatus,
  NodeQueueSummary,
  NodeStatusSnapshot,
  QueueListener,
  ScopeFilter,
};

export class KeyedParallelCoordinator {
  private maxConcurrent: number;
  private bag: QueueStateBag = { nodeQueues: new Map(), activeJobs: new Map(), fencingCounter: 0 };
  private listeners = new Set<QueueListener>();

  constructor(maxConcurrent = 4) {
    this.maxConcurrent = maxConcurrent;
  }

  public subscribe(listener: QueueListener): () => void {
    this.listeners.add(listener);
    listener(this.getSummary());
    return () => { this.listeners.delete(listener); };
  }

  private notify(): void {
    const summary = this.getSummary();
    for (const listener of this.listeners) {
      try { listener(summary); } catch { /* ignore listener errors */ }
    }
  }

  public getSummary(): NodeQueueSummary {
    return buildQueueSummary(this.bag.nodeQueues);
  }

  public getNodeStatus(nodeId: string, scope?: ScopeFilter): NodeStatusSnapshot | null {
    return lookupNodeStatusInQueues(this.bag.nodeQueues, nodeId, scope);
  }

  public enqueueJob(params: { projectId: string; sessionId: string; nodeId: string; prompt: string }): NodeExecutionJob {
    this.bag.fencingCounter += 1;
    const scopeKey = toCompositeScopeKey(params.projectId, params.sessionId, params.nodeId);
    const suffix = typeof globalThis.crypto?.randomUUID === "function"
      ? globalThis.crypto.randomUUID().slice(0, 8)
      : String(this.bag.fencingCounter).padStart(8, "0");
    const existing = this.bag.nodeQueues.get(scopeKey) ?? [];
    const job: NodeExecutionJob = {
      id: `job-${Date.now()}-${suffix}`,
      projectId: params.projectId, sessionId: params.sessionId, nodeId: params.nodeId,
      compositeScopeKey: scopeKey, prompt: params.prompt, status: "queued",
      queuePosition: existing.length, fencingToken: this.bag.fencingCounter, createdAt: Date.now(), steerHistory: [],
    };
    existing.push(job);
    this.bag.nodeQueues.set(scopeKey, existing);
    this.processNext();
    this.notify();
    return job;
  }

  private processNext(): void {
    const occupied = Array.from(this.bag.activeJobs.values()).filter((j) => j.status === "running" || j.status === "paused").length;
    if (occupied >= this.maxConcurrent) return;
    for (const queue of this.bag.nodeQueues.values()) {
      if (queue.length === 0) continue;
      const head = queue[0];
      if (head.status === "queued") {
        head.status = "running";
        head.startedAt = head.startedAt ?? Date.now();
        head.queuePosition = 0;
        this.bag.activeJobs.set(head.id, head);
        this.processNext();
        break;
      }
    }
  }

  public reconcileFromDaemon(dtos: NodeExecutionJobDto[], projectSeq?: number, scopeFilter?: ScopeFilter): void {
    reconcileFromDaemonQueues(this.bag, dtos, projectSeq, scopeFilter);
    this.notify();
  }

  public reconcileSingleJob(dto: NodeExecutionJobDto, projectSeq?: number): void {
    reconcileSingleJobInQueues(this.bag, dto, projectSeq);
    this.notify();
  }

  public updateProjectSeq(projectSeq: number): void {
    for (const queue of this.bag.nodeQueues.values()) {
      for (const job of queue) job.projectSeq = Math.max(job.projectSeq ?? 0, projectSeq);
    }
    this.notify();
  }

  public steerJob(jobId: string, instruction: string, nextToken?: number): boolean {
    const job = findJobInQueues(this.bag, jobId);
    if (!job || (job.status !== "running" && job.status !== "paused" && job.status !== "queued")) return false;
    if (nextToken !== undefined) {
      job.fencingToken = nextToken;
      this.bag.fencingCounter = Math.max(this.bag.fencingCounter, nextToken);
    }
    job.steerHistory.push({ timestamp: Date.now(), instruction });
    this.notify();
    return true;
  }

  public pauseJob(jobId: string, nextToken?: number): boolean {
    const job = findJobInQueues(this.bag, jobId);
    if (!job || job.status !== "running") return false;
    job.status = "paused";
    if (nextToken !== undefined) {
      job.fencingToken = nextToken;
      this.bag.fencingCounter = Math.max(this.bag.fencingCounter, nextToken);
    }
    this.notify();
    return true;
  }

  public resumeJob(jobId: string, nextToken?: number): boolean {
    const job = findJobInQueues(this.bag, jobId);
    if (!job || (job.status !== "paused" && job.status !== "interrupted")) return false;
    job.status = "running";
    if (nextToken !== undefined) {
      job.fencingToken = nextToken;
      this.bag.fencingCounter = Math.max(this.bag.fencingCounter, nextToken);
    }
    this.bag.activeJobs.set(job.id, job);
    this.notify();
    return true;
  }

  public completeJob(jobId: string, _outputSha256?: string): void { this.removeFromQueue(jobId, "completed"); }
  public cancelJob(jobId: string, reason = "USER_CANCELLED"): void { this.removeFromQueue(jobId, "cancelled", reason); }
  public failJob(jobId: string, reason = "EXECUTION_FAILED"): void { this.removeFromQueue(jobId, "error", reason); }

  private removeFromQueue(jobId: string, status: "completed" | "cancelled" | "error", reason?: string): void {
    const job = findJobInQueues(this.bag, jobId);
    if (!job) return;
    this.bag.activeJobs.delete(jobId);
    job.status = status;
    job.completedAt = Date.now();
    if (reason) job.errorMessage = reason;
    const scopeKey = job.compositeScopeKey || toCompositeScopeKey(job.projectId, job.sessionId, job.nodeId);
    const queue = this.bag.nodeQueues.get(scopeKey);
    if (queue) {
      const idx = queue.findIndex((j) => j.id === jobId);
      if (idx !== -1) queue.splice(idx, 1);
      queue.forEach((j, i) => { j.queuePosition = i; });
      if (queue.length === 0) this.bag.nodeQueues.delete(scopeKey);
    }
    this.processNext();
    this.notify();
  }

  public getActiveJobs(): NodeExecutionJob[] {
    return Array.from(this.bag.nodeQueues.values()).flat();
  }

  public clearAll(): void {
    this.bag.activeJobs.clear();
    this.bag.nodeQueues.clear();
    this.notify();
  }
}

export const globalKeyedParallelCoordinator = new KeyedParallelCoordinator(4);
