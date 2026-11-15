// @story 04.3
// @implements AC-01
import { describe, expect, it } from "vitest";
import { KeyedParallelCoordinator } from "../store/keyed-parallel-coordinator";

describe("KeyedParallelCoordinator (Story 04.3 - AC-01)", () => {
  it("should execute jobs on distinct nodes in parallel up to maxConcurrent", () => {
    const coordinator = new KeyedParallelCoordinator(3);
    const firstJob = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Build form" });
    const secondJob = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-2", prompt: "Build list" });
    const thirdJob = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-3", prompt: "Build modal" });
    expect([firstJob.status, secondJob.status, thirdJob.status]).toEqual(["running", "running", "running"]);

    const summary = coordinator.getSummary();
    expect(summary.runningCount).toBe(3);
    expect(summary.queuedCount).toBe(0);
    expect(summary.nodeStatusMap["node-1"].status).toBe("running");
    expect(summary.nodeStatusMap["node-2"].status).toBe("running");
    expect(summary.nodeStatusMap["node-3"].status).toBe("running");
  });

  it("should strictly enqueue second job on the same node in FIFO order", () => {
    const coordinator = new KeyedParallelCoordinator(4);
    const primaryJob = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Step 1" });
    const queuedJob = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Step 2" });

    expect(primaryJob.status).toBe("running");
    expect(primaryJob.queuePosition).toBe(0);

    expect(queuedJob.status).toBe("queued");
    expect(queuedJob.queuePosition).toBe(1);

    const summary = coordinator.getSummary();
    expect(summary.runningCount).toBe(1);
    expect(summary.queuedCount).toBe(1);

    // Complete primaryJob -> queuedJob automatically transitions to running
    coordinator.completeJob(primaryJob.id);
    const afterSummary = coordinator.getSummary();
    expect(afterSummary.runningCount).toBe(1);
    expect(afterSummary.queuedCount).toBe(0);
    expect(afterSummary.nodeStatusMap["node-1"].jobId).toBe(queuedJob.id);
  });

  it("should support mid-turn steer, pause, resume, and cancel", () => {
    const coordinator = new KeyedParallelCoordinator(2);
    const activeJob = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Generate table" });

    // Steer mid-turn
    const steered = coordinator.steerJob(activeJob.id, "Use 8 columns instead of 4");
    expect(steered).toBe(true);

    // Pause and Resume
    coordinator.pauseJob(activeJob.id);
    expect(coordinator.getNodeStatus("node-1")?.status).toBe("paused");

    coordinator.resumeJob(activeJob.id);
    expect(coordinator.getNodeStatus("node-1")?.status).toBe("running");

    // Cancel
    coordinator.cancelJob(activeJob.id);
    expect(coordinator.getSummary().runningCount).toBe(0);
  });

  it("should isolate queues by compositeScopeKey and keep unrelated scopes intact during partial reconcile", () => {
    const coordinator = new KeyedParallelCoordinator(4);
    const s1Job = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-shared", prompt: "S1" });
    const s2Job = coordinator.enqueueJob({ projectId: "p1", sessionId: "s2", nodeId: "node-shared", prompt: "S2" });

    expect(coordinator.getNodeStatus("node-shared", { projectId: "p1", sessionId: "s1" })?.jobId).toBe(s1Job.id);
    expect(coordinator.getNodeStatus("node-shared", { projectId: "p1", sessionId: "s2" })?.jobId).toBe(s2Job.id);

    // Unconfirmed steer does not drift fencingToken
    const beforeToken = s1Job.fencingToken;
    coordinator.steerJob(s1Job.id, "Keep calm");
    expect(coordinator.getNodeStatus("node-shared", { projectId: "p1", sessionId: "s1" })?.fencingToken).toBe(beforeToken);

    // Partial reconcile for s1 must not wipe s2
    coordinator.reconcileFromDaemon(
      [
        {
          id: s1Job.id,
          projectId: "p1",
          sessionId: "s1",
          nodeId: "node-shared",
          compositeScopeKey: "p1::s1::node-shared",
          daemonNonce: "nonce-a",
          status: "PAUSED",
          fencingToken: beforeToken + 1,
          priority: 10,
          staleAfterTimestamp: null,
          createdAt: 100,
          updatedAt: 200,
        },
      ],
      11
    );

    expect(coordinator.getNodeStatus("node-shared", { projectId: "p1", sessionId: "s1" })?.status).toBe("paused");
    expect(coordinator.getNodeStatus("node-shared", { projectId: "p1", sessionId: "s2" })?.jobId).toBe(s2Job.id);
  });
});
