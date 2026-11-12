// @story 04.3
// @implements AC-01
import { describe, expect, it } from "vitest";
import { KeyedParallelCoordinator } from "../store/keyed-parallel-coordinator";

describe("KeyedParallelCoordinator (Story 04.3 - AC-01)", () => {
  it("should execute jobs on distinct nodes in parallel up to maxConcurrent", () => {
    const coordinator = new KeyedParallelCoordinator(3);
    const j1 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Build form" });
    const j2 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-2", prompt: "Build list" });
    const j3 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-3", prompt: "Build modal" });

    const summary = coordinator.getSummary();
    expect(summary.runningCount).toBe(3);
    expect(summary.queuedCount).toBe(0);
    expect(summary.nodeStatusMap["node-1"].status).toBe("running");
    expect(summary.nodeStatusMap["node-2"].status).toBe("running");
    expect(summary.nodeStatusMap["node-3"].status).toBe("running");
  });

  it("should strictly enqueue second job on the same node in FIFO order", () => {
    const coordinator = new KeyedParallelCoordinator(4);
    const j1 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Step 1" });
    const j2 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Step 2" });

    expect(j1.status).toBe("running");
    expect(j1.queuePosition).toBe(0);

    expect(j2.status).toBe("queued");
    expect(j2.queuePosition).toBe(1);

    const summary = coordinator.getSummary();
    expect(summary.runningCount).toBe(1);
    expect(summary.queuedCount).toBe(1);

    // Complete j1 -> j2 automatically transitions to running
    coordinator.completeJob(j1.id);
    const afterSummary = coordinator.getSummary();
    expect(afterSummary.runningCount).toBe(1);
    expect(afterSummary.queuedCount).toBe(0);
    expect(afterSummary.nodeStatusMap["node-1"].jobId).toBe(j2.id);
  });

  it("should support mid-turn steer, pause, resume, and cancel", () => {
    const coordinator = new KeyedParallelCoordinator(2);
    const j1 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "node-1", prompt: "Generate table" });

    // Steer mid-turn
    const steered = coordinator.steerJob(j1.id, "Use 8 columns instead of 4");
    expect(steered).toBe(true);

    // Pause and Resume
    coordinator.pauseJob(j1.id);
    expect(coordinator.getNodeStatus("node-1")?.status).toBe("paused");

    coordinator.resumeJob(j1.id);
    expect(coordinator.getNodeStatus("node-1")?.status).toBe("running");

    // Cancel
    coordinator.cancelJob(j1.id);
    expect(coordinator.getSummary().runningCount).toBe(0);
  });
});
