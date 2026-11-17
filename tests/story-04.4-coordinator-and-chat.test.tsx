// @vitest-environment jsdom
// @story 04.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { setGlobalLocale } from "../src/hooks/useTranslation";
import { FlowExecutionClient } from "../src/api/flow-execution";
import { DagNodeHeader } from "../src/features/flow/components/DagNodeHeaderAndPorts";
import { ContextualNodeFloatingChat } from "../src/features/flow/components/ContextualNodeFloatingChat";
import {
  globalKeyedParallelCoordinator,
  KeyedParallelCoordinator,
} from "../src/features/flow/store/keyed-parallel-coordinator";
import { sharedChatStore } from "../src/features/flow/store/shared-chat-sync";
import { dispatchNodeExecutionToDaemon } from "../src/features/flow/hooks/dispatchNodeExecution";
import { apiFlowExecution } from "../src/api/flow-execution";
import { apiSdiEngine } from "../src/api/sdi-engine";
import type { WorkbenchNode } from "../src/features/flow/store/flow-workbench-store";
import { AgentChatPanel } from "../src/pages/workbench/components/AgentChatPanel";
import { SidebarSessionRow } from "../src/pages/workbench/components/SidebarSessionRow";
import manifestJson from "../src/plugins/cli-worker-subprocess-adapter/manifest.json";

const makeNode = (id: string, screenKey: string, title: string): WorkbenchNode => ({
  id,
  screenKey,
  title,
  nodeType: "screen",
  status: "draft",
  posX: 100,
  posY: 120,
  activeSubState: "DEFAULT",
  fixtureRef: "DEFAULT",
  note: "",
  version: 1,
});

describe("Story 04.4 Coordinator, Floating Chat v2, Left-Open Scope Chips & 3-Level Sidebar", () => {
  beforeEach(() => {
    setGlobalLocale("en-US");
    globalKeyedParallelCoordinator.clearAll();
    sharedChatStore.clearAll();
  });

  afterEach(() => {
    cleanup();
    setGlobalLocale("vi-VN");
  });

  it("enforces global running + paused <= 4 cap and reconciles daemon fencing tokens + interrupted states", () => {
    const coordinator = new KeyedParallelCoordinator(4);
    const j1 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "n1", prompt: "A" });
    coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "n2", prompt: "B" });
    coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "n3", prompt: "C" });
    coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "n4", prompt: "D" });
    const j5 = coordinator.enqueueJob({ projectId: "p1", sessionId: "s1", nodeId: "n5", prompt: "E" });

    expect(j5.status).toBe("queued");
    // Pausing j1 still occupies a slot (running + paused == 4), so j5 remains queued
    coordinator.pauseJob(j1.id);
    expect(coordinator.getSummary().pausedCount).toBe(1);
    expect(coordinator.getNodeStatus("n5")?.status).toBe("queued");

    // Cancelling j1 frees slot -> j5 promotes to running
    coordinator.cancelJob(j1.id);
    expect(coordinator.getNodeStatus("n5")?.status).toBe("running");

    // Reconcile daemon DTOs including INTERRUPTED & fencing token
    coordinator.reconcileFromDaemon(
      [
        {
          id: "job-daemon-9",
          projectId: "p1",
          sessionId: "s1",
          nodeId: "n9",
          compositeScopeKey: "p1::s1::n9",
          daemonNonce: "nonce-xyz",
          status: "INTERRUPTED",
          fencingToken: 12,
          priority: 10,
          staleAfterTimestamp: null,
          createdAt: 1000,
          updatedAt: 2000,
        },
      ],
      44
    );
    expect(coordinator.getSummary().interruptedCount).toBe(1);
    expect(coordinator.getNodeStatus("n9")).toMatchObject({
      status: "interrupted",
      fencingToken: 12,
      projectSeq: 44,
    });
  });

  it("tracks monotonic projectSeq and sends Last-Event-ID + afterSeq on SSE reconnect", async () => {
    const capturedHeaders: Record<string, string>[] = [];
    const capturedUrls: string[] = [];
    let callCount = 0;

    const fakeFetch = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes("/api/auth/local-session")) {
        return new Response(JSON.stringify({ sessionToken: "tok-local" }), { status: 200 });
      }
      callCount += 1;
      capturedUrls.push(url);
      capturedHeaders.push((init?.headers as Record<string, string>) || {});

      const seq = callCount === 1 ? 7 : 8;
      const frame = `id: ${seq}\nevent: execution_events\ndata: ${JSON.stringify([
        {
          id: `ev-${seq}`,
          jobId: "job-1",
          projectId: "11111111-1111-4111-8111-111111111111",
          nodeId: "n1",
          eventKind: "chunk",
          payloadText: `msg-${seq}`,
          seqNo: seq,
          projectSeq: seq,
          compositeScopeKey: "p::s::n1",
          jobStatus: "RUNNING",
          createdAt: "2026-11-15T00:00:00Z",
        },
      ])}\n\n`;
      const stream = new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(frame));
          controller.close();
        },
      });
      return new Response(stream, { status: 200 });
    });

    const originalFetch = globalThis.fetch;
    globalThis.fetch = fakeFetch as unknown as typeof fetch;
    try {
      const client = new FlowExecutionClient({ fetchImpl: fakeFetch as unknown as typeof fetch });
      const seen: number[] = [];
      await client.streamJobEvents("11111111-1111-4111-8111-111111111111", {
        onEvents: (evs) => evs.forEach((e) => seen.push(e.projectSeq ?? e.seqNo)),
      });
      expect(client.getLastProjectSeq("11111111-1111-4111-8111-111111111111")).toBe(7);

      await client.streamJobEvents("11111111-1111-4111-8111-111111111111", {
        onEvents: (evs) => evs.forEach((e) => seen.push(e.projectSeq ?? e.seqNo)),
      });
      expect(seen).toEqual([7, 8]);
      expect(capturedUrls[1]).toContain("afterSeq=7");
      expect(capturedHeaders[1]["Last-Event-ID"]).toBe("7");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("renders Floating Chat v2 360x480..900x850 clamp, 42px Pill, and Queue HUD badges", () => {
    const nodeA = makeNode("node-a", "SCR-A", "Checkout");
    const { container } = render(
      <div>
        <DagNodeHeader
          node={nodeA}
          isDraft={false}
          onStartRename={vi.fn()}
          executionStatus={{ status: "interrupted", queuePosition: 0 }}
        />
        <ContextualNodeFloatingChat
          projectId="proj-non-uuid"
          sessionId="sess-1"
          selectedNodes={[nodeA]}
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
        />
      </div>
    );

    expect(screen.getByText("INTERRUPTED")).toBeDefined();
    const pill = screen.getByTestId("fc-compact-pill");
    expect(pill.style.height).toBe("42px");

    fireEvent.click(screen.getByTestId("floating-chat-open-trigger"));
    const win = screen.getByTestId("contextual-node-floating-chat");
    expect(win.style.minWidth).toBe("360px");
    expect(win.style.maxWidth).toBe("900px");
    expect(win.style.minHeight).toBe("480px");
    expect(win.style.maxHeight).toContain("850px");
    expect(container.querySelector(".dag-node-interrupted-badge")).not.toBeNull();
  });

  it("renders Left-Open Scope Chips, 3-level V7 node_chat_sessions, and Hybrid Plugin manifest", () => {
    const nodeA = makeNode("node-scr-01", "SCR-01", "Login");
    const onDeselect = vi.fn();
    const onSelectSub = vi.fn();

    render(
      <div>
        <AgentChatPanel
          projectId="proj-local"
          activeSessionId="sess-1"
          selectedNodes={[nodeA]}
          onDeselectNode={onDeselect}
          isShortened={false}
          onToggleShorten={vi.fn()}
          width={320}
        />
        <SidebarSessionRow
          session={{
            id: "sess-1",
            title: "Main Flow",
            updatedAt: "10:00",
            subSessions: [
              {
                nodeId: "node-scr-01",
                nodeTitle: "Login",
                screenKey: "SCR-01",
                status: "paused",
                messageCount: 3,
              },
            ],
          }}
          keyPrefix="proj"
          activeSessionId="sess-1"
          projectId="proj-local"
          nodes={[nodeA]}
          onSelectSession={vi.fn()}
          onSelectNodeSubSession={onSelectSub}
        />
      </div>
    );

    expect(screen.getByTestId("left-chat-scope-chips")).toBeDefined();
    const subRow = screen.getByTestId("subsession-row-node-scr-01");
    expect(subRow.getAttribute("title")).toBe("Composite Scope: proj-local::sess-1::node-scr-01");
    fireEvent.click(subRow);
    expect(onSelectSub).toHaveBeenCalledWith("proj-local::sess-1::node-scr-01", "proj-local", "sess-1", "node-scr-01");
    expect(manifestJson.id).toBe("cli-worker-subprocess-adapter");
  });

  it("synchronizes messages across Left-Open and Floating Chat and never posts premature COMPLETED", async () => {
    const nodeA = makeNode("node-a", "SCR-A", "Checkout");
    render(
      <div>
        <AgentChatPanel
          projectId="proj-sync"
          activeSessionId="sess-sync"
          selectedNodes={[nodeA]}
          isShortened={false}
          onToggleShorten={vi.fn()}
          width={320}
        />
        <ContextualNodeFloatingChat
          projectId="proj-sync"
          sessionId="sess-sync"
          selectedNodes={[nodeA]}
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
          defaultExpanded
        />
      </div>
    );

    sharedChatStore.appendMessage("proj-sync::sess-sync", {
      id: "sync-msg-1",
      sender: "human",
      text: "Cross-surface prompt synced",
      timestamp: "10:05",
      createdAtMs: 1000,
      nodeId: "node-a",
    });
    expect((await screen.findAllByText("Cross-surface prompt synced")).length).toBeGreaterThanOrEqual(2);

    const postEventSpy = vi.spyOn(apiFlowExecution, "postJobEvent").mockResolvedValue({
      id: "ev-x", jobId: "j-x", projectId: "11111111-1111-4111-8111-111111111111", nodeId: "node-a",
      eventKind: "chunk", payloadText: "ok", seqNo: 1, projectSeq: 1, compositeScopeKey: "p::s::node-a",
      jobStatus: "RUNNING", createdAt: "2026-11-15T00:00:00Z",
    });
    vi.spyOn(apiFlowExecution, "enqueueJobs").mockResolvedValue({
      jobs: [{
        id: "job-running-1", projectId: "11111111-1111-4111-8111-111111111111", sessionId: "sess-sync",
        nodeId: "node-a", compositeScopeKey: "11111111-1111-4111-8111-111111111111::sess-sync::node-a",
        daemonNonce: "n1", status: "RUNNING", fencingToken: 1, priority: 10, staleAfterTimestamp: null,
        createdAt: 1000, updatedAt: 1000,
      }],
      activeSlotCount: 1,
    });
    vi.spyOn(apiSdiEngine, "compileIntent").mockResolvedValue({
      id: "intent-1", tenantId: "local-default", projectId: "11111111-1111-4111-8111-111111111111",
      rawPrompt: "Build screen", intentType: "create_screen", status: "ready", ambiguityScore: 0.1, riskScore: 0.1,
      targetScope: { nodeIds: ["node-a"], screenKeys: ["SCR-A"], components: [] },
      createdAt: "2026-11-15T00:00:00Z", updatedAt: "2026-11-15T00:00:00Z",
    });
    vi.spyOn(apiFlowExecution, "streamJobEvents").mockResolvedValue();

    await dispatchNodeExecutionToDaemon({
      projectId: "11111111-1111-4111-8111-111111111111",
      sessionId: "sess-sync",
      primaryNode: nodeA,
      selectedNodeIds: ["node-a"],
      prompt: "Build screen",
      coordinator: {
        ...globalKeyedParallelCoordinator,
        summary: globalKeyedParallelCoordinator.getSummary(),
        syncFromDaemon: vi.fn(async () => ({ sessions: [], jobs: [], activeSlotCount: 0, maxConcurrent: 4 })),
        reconcileSingleJob: vi.fn(),
        enqueueJob: globalKeyedParallelCoordinator.enqueueJob.bind(globalKeyedParallelCoordinator),
        steerJob: globalKeyedParallelCoordinator.steerJob.bind(globalKeyedParallelCoordinator),
        pauseJob: globalKeyedParallelCoordinator.pauseJob.bind(globalKeyedParallelCoordinator),
        resumeJob: globalKeyedParallelCoordinator.resumeJob.bind(globalKeyedParallelCoordinator),
        cancelJob: globalKeyedParallelCoordinator.cancelJob.bind(globalKeyedParallelCoordinator),
        failJob: globalKeyedParallelCoordinator.failJob.bind(globalKeyedParallelCoordinator),
        completeJob: globalKeyedParallelCoordinator.completeJob.bind(globalKeyedParallelCoordinator),
        getNodeStatus: globalKeyedParallelCoordinator.getNodeStatus.bind(globalKeyedParallelCoordinator),
        getActiveJobs: globalKeyedParallelCoordinator.getActiveJobs.bind(globalKeyedParallelCoordinator),
      },
      onAppendMsg: vi.fn(),
      t: (_k, f) => f,
      formatNumber: (n) => String(n),
    });

    expect(postEventSpy).not.toHaveBeenCalled();
    expect(globalKeyedParallelCoordinator.getNodeStatus("node-a")?.status).toBe("running");
  });
});
