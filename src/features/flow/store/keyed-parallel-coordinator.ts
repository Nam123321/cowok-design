// @story 04.3
// @implements AC-01 (KeyedParallelCoordinator Engine)
export type NodeJobStatus =
  | "queued"
  | "running"
  | "paused"
  | "completed"
  | "error"
  | "cancelled";

export interface NodeExecutionJob {
  id: string;
  projectId: string;
  sessionId: string;
  nodeId: string;
  compositeScopeKey: string; // `${projectId}::${sessionId}::${nodeId}`
  prompt: string;
  status: NodeJobStatus;
  queuePosition: number; // 0 if running/idle, 1..N if queued
  fencingToken: number;
  progressPercent?: number;
  outputArtifactSha256?: string;
  createdAt: number;
  startedAt?: number;
  completedAt?: number;
  errorMessage?: string;
  steerHistory: Array<{ timestamp: number; instruction: string }>;
}

export interface NodeQueueSummary {
  runningCount: number;
  queuedCount: number;
  totalActive: number;
  nodeStatusMap: Record<
    string,
    { status: NodeJobStatus; queuePosition: number; jobId: string }
  >;
}

export type QueueListener = (summary: NodeQueueSummary) => void;

export class KeyedParallelCoordinator {
  private maxConcurrent: number;
  // NodeId -> FIFO queue of jobs waiting or active for this node
  private nodeQueues: Map<string, NodeExecutionJob[]> = new Map();
  // Active running/paused jobs keyed by jobId
  private activeJobs: Map<string, NodeExecutionJob> = new Map();
  // Monotonic fencing token counter
  private fencingCounter: number = 0;
  // Subscribed listeners
  private listeners: Set<QueueListener> = new Set();

  constructor(maxConcurrent = 4) {
    this.maxConcurrent = maxConcurrent;
  }

  public subscribe(listener: QueueListener): () => void {
    this.listeners.add(listener);
    listener(this.getSummary());
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notify(): void {
    const summary = this.getSummary();
    for (const listener of this.listeners) {
      try {
        listener(summary);
      } catch {
        // Suppress listener errors
      }
    }
  }

  public getSummary(): NodeQueueSummary {
    let runningCount = 0;
    let queuedCount = 0;
    const nodeStatusMap: Record<
      string,
      { status: NodeJobStatus; queuePosition: number; jobId: string }
    > = {};

    for (const [nodeId, queue] of this.nodeQueues.entries()) {
      if (queue.length === 0) continue;
      const head = queue[0];
      if (head.status === "running" || head.status === "paused") {
        runningCount++;
        nodeStatusMap[nodeId] = {
          status: head.status,
          queuePosition: 0,
          jobId: head.id,
        };
      }
      for (let i = 1; i < queue.length; i++) {
        queuedCount++;
      }
      // If head is queued (waiting for concurrent slot)
      if (head.status === "queued") {
        queuedCount++;
        nodeStatusMap[nodeId] = {
          status: "queued",
          queuePosition: 1,
          jobId: head.id,
        };
      }
    }

    return {
      runningCount,
      queuedCount,
      totalActive: runningCount + queuedCount,
      nodeStatusMap,
    };
  }

  public getNodeStatus(nodeId: string): { status: NodeJobStatus; queuePosition: number; jobId?: string } | null {
    const queue = this.nodeQueues.get(nodeId);
    if (!queue || queue.length === 0) {
      return null;
    }
    const head = queue[0];
    return {
      status: head.status,
      queuePosition: head.queuePosition,
      jobId: head.id,
    };
  }

  public enqueueJob(params: {
    projectId: string;
    sessionId: string;
    nodeId: string;
    prompt: string;
  }): NodeExecutionJob {
    this.fencingCounter++;
    const compositeScopeKey = `${params.projectId}::${params.sessionId}::${params.nodeId}`;
    const randomSuffix =
      typeof globalThis.crypto?.randomUUID === "function"
        ? globalThis.crypto.randomUUID().slice(0, 8)
        : String(this.fencingCounter).padStart(8, "0");
    const jobId = `job-${Date.now()}-${randomSuffix}`;

    let queue = this.nodeQueues.get(params.nodeId);
    if (!queue) {
      queue = [];
      this.nodeQueues.set(params.nodeId, queue);
    }

    const queuePosition = queue.length; // 0 if empty (will run), >0 if enqueued
    const job: NodeExecutionJob = {
      id: jobId,
      projectId: params.projectId,
      sessionId: params.sessionId,
      nodeId: params.nodeId,
      compositeScopeKey,
      prompt: params.prompt,
      status: queue.length === 0 ? "queued" : "queued",
      queuePosition,
      fencingToken: this.fencingCounter,
      createdAt: Date.now(),
      steerHistory: [],
    };

    queue.push(job);
    this.processNext();
    this.notify();
    return job;
  }

  private processNext(): void {
    const currentRunning = Array.from(this.activeJobs.values()).filter(
      (j) => j.status === "running"
    ).length;

    if (currentRunning >= this.maxConcurrent) {
      return;
    }

    for (const [, queue] of this.nodeQueues.entries()) {
      if (queue.length === 0) continue;
      const head = queue[0];
      if (head.status === "queued") {
        head.status = "running";
        head.startedAt = Date.now();
        head.queuePosition = 0;
        this.activeJobs.set(head.id, head);
        this.processNext(); // Greedily schedule next distinct nodes
        break;
      }
    }
  }

  public steerJob(jobId: string, instruction: string): boolean {
    const job = this.activeJobs.get(jobId);
    if (!job || job.status !== "running") return false;
    job.steerHistory.push({ timestamp: Date.now(), instruction });
    this.notify();
    return true;
  }

  public pauseJob(jobId: string): boolean {
    const job = this.activeJobs.get(jobId);
    if (!job || job.status !== "running") return false;
    job.status = "paused";
    this.notify();
    return true;
  }

  public resumeJob(jobId: string): boolean {
    const job = this.activeJobs.get(jobId);
    if (!job || job.status !== "paused") return false;
    job.status = "running";
    this.notify();
    return true;
  }

  public completeJob(jobId: string, outputSha256?: string): void {
    const job = this.activeJobs.get(jobId);
    if (!job) return;

    job.status = "completed";
    job.completedAt = Date.now();
    job.outputArtifactSha256 = outputSha256;
    this.activeJobs.delete(jobId);

    const queue = this.nodeQueues.get(job.nodeId);
    if (queue) {
      const idx = queue.findIndex((j) => j.id === jobId);
      if (idx !== -1) {
        queue.splice(idx, 1);
      }
      // Re-index remaining jobs in queue
      queue.forEach((j, i) => {
        j.queuePosition = i;
      });
      if (queue.length === 0) {
        this.nodeQueues.delete(job.nodeId);
      }
    }

    this.processNext();
    this.notify();
  }

  public cancelJob(jobId: string, reason = "USER_CANCELLED"): void {
    let targetJob: NodeExecutionJob | undefined = this.activeJobs.get(jobId);
    if (targetJob) {
      this.activeJobs.delete(jobId);
    } else {
      for (const queue of this.nodeQueues.values()) {
        const found = queue.find((j) => j.id === jobId);
        if (found) {
          targetJob = found;
          break;
        }
      }
    }

    if (!targetJob) return;

    targetJob.status = "cancelled";
    targetJob.completedAt = Date.now();
    targetJob.errorMessage = reason;

    const queue = this.nodeQueues.get(targetJob.nodeId);
    if (queue) {
      const idx = queue.findIndex((j) => j.id === jobId);
      if (idx !== -1) {
        queue.splice(idx, 1);
      }
      queue.forEach((j, i) => {
        j.queuePosition = i;
      });
      if (queue.length === 0) {
        this.nodeQueues.delete(targetJob.nodeId);
      }
    }

    this.processNext();
    this.notify();
  }

  public getActiveJobs(): NodeExecutionJob[] {
    const list: NodeExecutionJob[] = [];
    for (const queue of this.nodeQueues.values()) {
      for (const job of queue) {
        list.push(job);
      }
    }
    return list;
  }

  public clearAll(): void {
    this.activeJobs.clear();
    this.nodeQueues.clear();
    this.notify();
  }
}

export const globalKeyedParallelCoordinator = new KeyedParallelCoordinator(4);
