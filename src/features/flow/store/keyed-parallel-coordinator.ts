// @story 04.3, 04.4
// @implements AC-01, AC-02 (Persistent KeyedParallelCoordinator with Fencing Reconciliation)
import type { NodeExecutionJobDto } from "@/lib/validations/flow-execution";
import {
  DAEMON_TO_LOCAL_STATUS,
  buildQueueSummary,
  mapDaemonDtoToJob,
  toNodeStatusSnapshot,
  type NodeExecutionJob,
  type NodeJobStatus,
  type NodeQueueSummary,
  type NodeStatusSnapshot,
  type QueueListener,
} from "./keyed-parallel-types";

export type {
  NodeExecutionJob,
  NodeJobStatus,
  NodeQueueSummary,
  NodeStatusSnapshot,
  QueueListener,
};

export class KeyedParallelCoordinator {
  private maxConcurrent: number;
  private nodeQueues = new Map<string, NodeExecutionJob[]>();
  private activeJobs = new Map<string, NodeExecutionJob>();
  private fencingCounter = 0;
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
    return buildQueueSummary(this.nodeQueues);
  }

  public getNodeStatus(nodeId: string): NodeStatusSnapshot | null {
    const queue = this.nodeQueues.get(nodeId);
    if (!queue || queue.length === 0) return null;
    return toNodeStatusSnapshot(queue[0]);
  }

  public enqueueJob(params: { projectId: string; sessionId: string; nodeId: string; prompt: string }): NodeExecutionJob {
    this.fencingCounter += 1;
    const suffix = typeof globalThis.crypto?.randomUUID === "function"
      ? globalThis.crypto.randomUUID().slice(0, 8)
      : String(this.fencingCounter).padStart(8, "0");
    const job: NodeExecutionJob = {
      id: `job-${Date.now()}-${suffix}`,
      projectId: params.projectId,
      sessionId: params.sessionId,
      nodeId: params.nodeId,
      compositeScopeKey: `${params.projectId}::${params.sessionId}::${params.nodeId}`,
      prompt: params.prompt,
      status: "queued",
      queuePosition: this.nodeQueues.get(params.nodeId)?.length ?? 0,
      fencingToken: this.fencingCounter,
      createdAt: Date.now(),
      steerHistory: [],
    };
    const queue = this.nodeQueues.get(params.nodeId) ?? [];
    queue.push(job);
    this.nodeQueues.set(params.nodeId, queue);
    this.processNext();
    this.notify();
    return job;
  }

  private processNext(): void {
    const occupied = Array.from(this.activeJobs.values()).filter((j) => j.status === "running" || j.status === "paused").length;
    if (occupied >= this.maxConcurrent) return;
    for (const queue of this.nodeQueues.values()) {
      if (queue.length === 0) continue;
      const head = queue[0];
      if (head.status === "queued") {
        head.status = "running";
        head.startedAt = head.startedAt ?? Date.now();
        head.queuePosition = 0;
        this.activeJobs.set(head.id, head);
        this.processNext();
        break;
      }
    }
  }

  public reconcileFromDaemon(dtos: NodeExecutionJobDto[], projectSeq?: number): void {
    this.activeJobs.clear();
    this.nodeQueues.clear();
    let queuedIdx = 0;
    for (const dto of dtos) {
      const status = DAEMON_TO_LOCAL_STATUS[dto.status] ?? "queued";
      if (status === "completed" || status === "error" || status === "cancelled") continue;
      this.fencingCounter = Math.max(this.fencingCounter, dto.fencingToken);
      const q = this.nodeQueues.get(dto.nodeId) ?? [];
      const job = mapDaemonDtoToJob(dto, status, status === "queued" ? ++queuedIdx : 0, projectSeq);
      q.push(job);
      this.nodeQueues.set(dto.nodeId, q);
      if (status === "running" || status === "paused") this.activeJobs.set(job.id, job);
    }
    this.notify();
  }

  public updateProjectSeq(projectSeq: number): void {
    for (const queue of this.nodeQueues.values()) {
      for (const job of queue) job.projectSeq = Math.max(job.projectSeq ?? 0, projectSeq);
    }
    this.notify();
  }

  public steerJob(jobId: string, instruction: string, nextToken?: number): boolean {
    const job = this.findJob(jobId);
    if (!job || (job.status !== "running" && job.status !== "paused" && job.status !== "queued")) return false;
    job.fencingToken = nextToken ?? ++this.fencingCounter;
    job.steerHistory.push({ timestamp: Date.now(), instruction });
    this.notify();
    return true;
  }

  public pauseJob(jobId: string, nextToken?: number): boolean {
    const job = this.findJob(jobId);
    if (!job || job.status !== "running") return false;
    job.status = "paused";
    job.fencingToken = nextToken ?? ++this.fencingCounter;
    this.notify();
    return true;
  }

  public resumeJob(jobId: string, nextToken?: number): boolean {
    const job = this.findJob(jobId);
    if (!job || (job.status !== "paused" && job.status !== "interrupted")) return false;
    job.status = "running";
    job.fencingToken = nextToken ?? ++this.fencingCounter;
    this.activeJobs.set(job.id, job);
    this.notify();
    return true;
  }

  public completeJob(jobId: string, _outputSha256?: string): void { this.removeFromQueue(jobId, "completed"); }
  public cancelJob(jobId: string, reason = "USER_CANCELLED"): void { this.removeFromQueue(jobId, "cancelled", reason); }
  public failJob(jobId: string, reason = "EXECUTION_FAILED"): void { this.removeFromQueue(jobId, "error", reason); }

  private findJob(jobId: string): NodeExecutionJob | undefined {
    const active = this.activeJobs.get(jobId);
    if (active) return active;
    for (const q of this.nodeQueues.values()) {
      const hit = q.find((j) => j.id === jobId);
      if (hit) return hit;
    }
    return undefined;
  }

  private removeFromQueue(jobId: string, status: "completed" | "cancelled" | "error", reason?: string): void {
    const job = this.findJob(jobId);
    if (!job) return;
    this.activeJobs.delete(jobId);
    job.status = status;
    job.completedAt = Date.now();
    if (reason) job.errorMessage = reason;
    const queue = this.nodeQueues.get(job.nodeId);
    if (queue) {
      const idx = queue.findIndex((j) => j.id === jobId);
      if (idx !== -1) queue.splice(idx, 1);
      queue.forEach((j, i) => { j.queuePosition = i; });
      if (queue.length === 0) this.nodeQueues.delete(job.nodeId);
    }
    this.processNext();
    this.notify();
  }

  public getActiveJobs(): NodeExecutionJob[] {
    return Array.from(this.nodeQueues.values()).flat();
  }

  public clearAll(): void {
    this.activeJobs.clear();
    this.nodeQueues.clear();
    this.notify();
  }
}

export const globalKeyedParallelCoordinator = new KeyedParallelCoordinator(4);
