/** @vitest-environment jsdom */
// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (Zod Contracts, DoS Bounds, SDK Runtime Parsing, Live Store Hydration, Native Edge SVG & 4-SubState Pills)
import React, { useEffect, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  FlowSynthesizeApiClient,
  FlowSynthesizeApiError,
  apiFlowSynthesize,
} from "@/api/flow-synthesize";
import { GET as handleGetSubstatesBff } from "@/app/api/v1/flows/synthesize/route";
import { issueBffLoopbackSession } from "@/lib/api-guard";
import * as daemonClient from "@/lib/daemon-client";
import {
  FlowSubstatesListResponseSchema,
  FlowSynthesizeRequestSchema,
  SynthesizeFlowResponseSchema,
  type SynthesizedFlowPlan,
} from "@/lib/validations/flow-synthesize";
import {
  type FlowWorkbenchStore,
  createNewFlowWorkbenchStore,
} from "../store/flow-workbench-store";
import {
  DagCanvas2D,
  mapSynthesizedPlanToWorkbenchGraph,
} from "../components/DagCanvas2D";

const PROJECT_ID = "55555555-5555-4555-8555-555555555555";
const PROJECT_B_ID = "66666666-6666-4666-8666-666666666666";

function buildSynthesizedPlanFixture(projectId = PROJECT_ID): SynthesizedFlowPlan {
  const primaryNodes = Array.from({ length: 8 }, (_, idx) => ({
    id: `10000000-0000-4000-8000-00000000000${idx + 1}`,
    screenKey: `cluster-screen-${idx + 1}`,
    title: `Happy Cluster ${idx + 1}`,
    routePath: `/cluster-${idx + 1}`,
    nodeType: "screen",
    flowTier: "PRIMARY" as const,
    clusterId: `cluster-${idx + 1}`,
    severityBand: idx === 7 ? ("CRITICAL" as const) : ("LOW" as const),
    mergedScreenKeys: [`screen-${idx + 1}`, `screen-extra-${idx + 1}`],
    posX: 120 + idx * 320,
    posY: 140,
    defaultFixtureId: `fix-default-${idx + 1}`,
  }));

  const edgeCaseNode = {
    id: "20000000-0000-4000-8000-000000000001",
    screenKey: "fmea-payment-fallback",
    title: "Payment Timeout Recovery",
    routePath: "/recovery/payment-timeout",
    nodeType: "condition",
    flowTier: "EDGE_CASE" as const,
    clusterId: "cluster-8",
    severityBand: "CRITICAL" as const,
    mergedScreenKeys: ["fmea-payment-fallback"],
    posX: 120,
    posY: 420,
    defaultFixtureId: "fix-default-edge-1",
  };

  const allNodes = [...primaryNodes, edgeCaseNode];
  const variants = ["DEFAULT", "LOADING", "EMPTY", "ERROR"] as const;
  const substates = allNodes.flatMap((n) =>
    variants.map((substate) => ({
      nodeId: n.id,
      projectId,
      substate,
      fixtureId: `fixture-${n.screenKey}-${substate.toLowerCase()}`,
      flowTier: n.flowTier,
      clusterId: n.clusterId,
      severityBand: n.severityBand,
      payloadJson: JSON.stringify({ state: substate, ready: substate === "DEFAULT" }),
      updatedAt: 1700000000000,
    }))
  );

  return {
    projectId,
    summary: {
      primaryClusterCount: 8,
      edgeCaseBranchCount: 1,
      substateCoveragePercent: 100,
      floatLeakCount: 0,
      strippedOrphanEdgeCount: 0,
      rejectedCycleEdgeCount: 0,
      preservedLoopbackEdgeCount: 1,
      decisionKind: "ExactMatch",
      highestSeverity: "CRITICAL",
      simulatedDimensions: [
        "onboarding-first-run",
        "core-happy-path",
        "search-and-filtering",
        "creation-and-editing",
        "checkout-and-commitment",
        "collaboration-and-handoff",
        "recovery-and-offline",
        "accessibility-and-empty-states",
      ],
    },
    nodes: allNodes,
    edges: [
      {
        id: "30000000-0000-4000-8000-000000000001",
        sourceNodeId: primaryNodes[0].id,
        targetNodeId: primaryNodes[1].id,
        flowType: "main",
        triggerEvent: "onProceed",
        guardCondition: null,
        flowTier: "PRIMARY",
        severityBand: "LOW",
      },
      {
        id: "30000000-0000-4000-8000-000000000002",
        sourceNodeId: primaryNodes[7].id,
        targetNodeId: edgeCaseNode.id,
        flowType: "if_not",
        triggerEvent: "onPaymentTimeout",
        guardCondition: "fmea_severity >= CRITICAL",
        flowTier: "EDGE_CASE",
        severityBand: "CRITICAL",
      },
      {
        id: "30000000-0000-4000-8000-000000000003",
        sourceNodeId: edgeCaseNode.id,
        targetNodeId: primaryNodes[7].id,
        flowType: "loopback",
        triggerEvent: "onUserRetry",
        guardCondition: "user_confirmed_retry == true",
        flowTier: "EDGE_CASE",
        severityBand: "CRITICAL",
      },
    ],
    substates,
  };
}

const ReactiveDagCanvasHarness: React.FC<{
  store: FlowWorkbenchStore;
  projectId: string;
  onSelectSubStateSpy?: (nodeId: string, subState: string) => void;
}> = ({ store, projectId, onSelectSubStateSpy }) => {
  const [snap, setSnap] = useState(() => store.getState());
  useEffect(() => store.subscribe(() => setSnap(store.getState())), [store]);
  return (
    <DagCanvas2D
      state={snap}
      projectId={projectId}
      onUpdatePosition={(id, x, y) => store.updateNodePosition(id, x, y)}
      onSelectNode={(id, multi) => store.selectNode(id, multi)}
      onSelectTransition={(id) => store.selectTransition(id)}
      onAddTransition={(s, t, type) => store.addTransition({ sourceNodeId: s, targetNodeId: t, triggerEvent: "onClick", edgeType: type || "main" })}
      onUpdateNodeNote={(id, note) => store.updateNodeNote(id, note)}
      onRenameNode={(id, title) => store.renameNode(id, title)}
      onDeleteNode={(id) => store.deleteNode(id)}
      onSelectSubState={(id, sub) => {
        onSelectSubStateSpy?.(id, sub);
        store.setNodeSubState(id, sub);
      }}
      onSynthesizedFlow={(plan) => {
        const mapped = mapSynthesizedPlanToWorkbenchGraph(plan);
        store.hydrate(mapped.nodes, mapped.transitions);
      }}
    />
  );
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Story 05.5 — 3-Tier Interactive Flow DAG Synthesizer & Lens", () => {
  it("UT-05.5-01: validates Zod schemas, DoS bounds, strict spoofing rejection & 0 float leaks", () => {
    const validReq = FlowSynthesizeRequestSchema.parse({
      projectId: PROJECT_ID,
      briefSummary: "12-screen fintech checkout",
    });
    expect(validReq.maxDefaultClusterNodes).toBe(8);
    expect(validReq.includeFmeaBranches).toBe(true);

    expect(() =>
      FlowSynthesizeRequestSchema.parse({
        projectId: PROJECT_ID,
        briefSummary: "Too many clusters",
        maxDefaultClusterNodes: 9,
      })
    ).toThrow();

    // Spoofable simulateDecisionTimeout MUST be rejected by .strict()
    expect(() =>
      FlowSynthesizeRequestSchema.parse({
        projectId: PROJECT_ID,
        briefSummary: "Spoof attempt",
        simulateDecisionTimeout: true,
      })
    ).toThrow();

    // DoS bounds: >64 screens, >256 candidateEdges, >4000 briefSummary
    expect(() =>
      FlowSynthesizeRequestSchema.parse({
        projectId: PROJECT_ID,
        briefSummary: "x".repeat(4001),
      })
    ).toThrow();
    expect(() =>
      FlowSynthesizeRequestSchema.parse({
        projectId: PROJECT_ID,
        briefSummary: "Too many screens",
        screens: Array.from({ length: 65 }, (_, i) => ({ screenKey: `s-${i}`, title: `S ${i}` })),
      })
    ).toThrow();
    expect(() =>
      FlowSynthesizeRequestSchema.parse({
        projectId: PROJECT_ID,
        briefSummary: "Too many edges",
        candidateEdges: Array.from({ length: 257 }, (_, i) => ({
          sourceNodeId: `a-${i}`,
          targetNodeId: `b-${i}`,
          flowType: "main" as const,
          triggerEvent: "onClick",
        })),
      })
    ).toThrow();

    const plan = buildSynthesizedPlanFixture();
    const okResp = SynthesizeFlowResponseSchema.parse({
      ok: true,
      data: plan,
      requestId: "req-055-1",
    });
    expect(okResp.data.summary.primaryClusterCount).toBeLessThanOrEqual(8);
    expect(okResp.data.summary.floatLeakCount).toBe(0);
    expect(okResp.data.summary.substateCoveragePercent).toBe(100);

    // VerdictShield rejects non-zero floatLeakCount
    expect(() =>
      SynthesizeFlowResponseSchema.parse({
        ok: true,
        data: {
          ...plan,
          summary: { ...plan.summary, floatLeakCount: 1 },
        },
        requestId: "req-055-bad",
      })
    ).toThrow();

    const listResp = FlowSubstatesListResponseSchema.parse({
      ok: true,
      data: {
        projectId: PROJECT_ID,
        items: plan.substates.map(({ payloadJson: _p, ...stored }) => stored),
        totalCount: plan.substates.length,
      },
      requestId: "req-055-list",
    });
    expect(listResp.data.totalCount).toBe(plan.nodes.length * 4);
  });

  it("UT-05.5-02: SDK validates listSubstates params and parses FlowSynthesizeErrorResponseSchema", async () => {
    const plan = buildSynthesizedPlanFixture();
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ ok: true, data: plan, requestId: "req-sdk-1" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              projectId: PROJECT_ID,
              items: plan.substates.map(({ payloadJson: _p, ...stored }) => stored),
              totalCount: plan.substates.length,
            },
            requestId: "req-sdk-2",
          }),
          { status: 200, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            error: {
              code: "VALIDATION_ERROR",
              message: "Invalid flowTier",
              field: "flowTier",
              requestId: "req-err-1",
            },
          }),
          { status: 422, headers: { "Content-Type": "application/json" } }
        )
      );
    const client = new FlowSynthesizeApiClient({
      baseUrl: "http://127.0.0.1:4175",
      getSessionToken: () => "cwk_sess_unit_055",
      fetchImpl: fetchMock as unknown as typeof fetch,
    });
    const resp = await client.synthesize({
      projectId: PROJECT_ID,
      briefSummary: "12-screen app synthesis",
    });
    expect(resp.data.summary.primaryClusterCount).toBe(8);
    expect(resp.data.summary.floatLeakCount).toBe(0);

    const listResp = await client.listSubstates({ projectId: PROJECT_ID, flowTier: "PRIMARY" });
    expect(listResp.data.totalCount).toBe(plan.nodes.length * 4);

    // Invalid params rejected client-side via FlowSubstatesQuerySchema.parse
    await expect(client.listSubstates({ projectId: "   " })).rejects.toThrow();

    // Error response parsed via FlowSynthesizeErrorResponseSchema.safeParse
    await expect(client.listSubstates({ projectId: PROJECT_ID })).rejects.toMatchObject({
      name: "FlowSynthesizeApiError",
      status: 422,
      code: "VALIDATION_ERROR",
      field: "flowTier",
    } satisfies Partial<FlowSynthesizeApiError>);
  });

  it("UT-05.5-03: DagCanvas2D hydrates live FlowWorkbenchStore, supports post-synthesis interactivity, native edge SVG & ordered 4-SubState pills", async () => {
    const plan = buildSynthesizedPlanFixture();
    vi.spyOn(apiFlowSynthesize, "listSubstates").mockResolvedValue({
      ok: true,
      data: { projectId: PROJECT_ID, items: [], totalCount: 0 },
      requestId: "req-init-empty",
    });
    const synthSpy = vi.spyOn(apiFlowSynthesize, "synthesize").mockResolvedValue({
      ok: true,
      data: plan,
      requestId: "req-ui-055",
    });
    const onSelectSubStateSpy = vi.fn();
    const store = createNewFlowWorkbenchStore([], [], { projectId: PROJECT_ID });

    render(
      <ReactiveDagCanvasHarness
        store={store}
        projectId={PROJECT_ID}
        onSelectSubStateSpy={onSelectSubStateSpy}
      />
    );

    const lensToggle = screen.getByTestId("show-edge-case-flows-toggle");
    expect(lensToggle.getAttribute("data-ff-id")).toBe("show-edge-case-flows-toggle");
    expect(lensToggle.getAttribute("aria-checked")).toBe("false");

    const clusterBadge = screen.getByTestId("dag-happy-path-cluster");
    expect(clusterBadge.getAttribute("data-ff-id")).toBe("dag-happy-path-cluster");

    // Trigger real 3-tier flow synthesis -> hydrates live store!
    fireEvent.click(screen.getByTestId("synthesize-3tier-flow-btn"));

    await waitFor(() => {
      expect(synthSpy).toHaveBeenCalledTimes(1);
      expect(screen.getByTestId("dag-verdict-shield-badge")).toBeTruthy();
      expect(store.getState().nodes.length).toBe(9);
    });

    // Default lens OFF: <= 8 PRIMARY nodes rendered, EDGE_CASE node & edges hidden
    expect(document.querySelectorAll("[data-flow-tier='PRIMARY'].dag-node-card").length).toBe(8);
    expect(screen.queryByTestId("dag-node-20000000-0000-4000-8000-000000000001")).toBeNull();
    expect(screen.queryByTestId("dag-edge-30000000-0000-4000-8000-000000000002")).toBeNull();

    // Toggle [Show Edge-Case Flows] ON -> EDGE_CASE node + native DagSvgEdges (NO duplicate overlay!)
    fireEvent.click(lensToggle);
    expect(lensToggle.getAttribute("aria-checked")).toBe("true");
    expect(screen.getByTestId("dag-node-20000000-0000-4000-8000-000000000001")).toBeTruthy();
    expect(screen.queryByTestId("dag-fmea-edges-overlay")).toBeNull();
    const fmeaEdge = screen.getByTestId("dag-edge-30000000-0000-4000-8000-000000000002");
    expect(fmeaEdge.getAttribute("data-flow-tier")).toBe("EDGE_CASE");
    const loopbackEdge = screen.getByTestId("dag-edge-30000000-0000-4000-8000-000000000003");
    expect(loopbackEdge.getAttribute("data-edge-type")).toBe("loopback");

    // Toggle OFF -> EDGE_CASE hides again
    fireEvent.click(lensToggle);
    expect(screen.queryByTestId("dag-node-20000000-0000-4000-8000-000000000001")).toBeNull();

    // Exact pill order: DEFAULT | LOADING | EMPTY | ERROR
    const firstNodeId = plan.nodes[0].id;
    const pillsGroup = screen.getByTestId(`dag-node-substates-${firstNodeId}`);
    expect(pillsGroup.getAttribute("data-ff-id")).toBe("dag-flow-node-substate-pills");
    const pillLabels = Array.from(pillsGroup.querySelectorAll("button")).map((b) => b.textContent);
    expect(pillLabels).toEqual(["DEFAULT", "LOADING", "EMPTY", "ERROR"]);

    const emptyPill = screen.getByTestId(`dag-substate-empty-${firstNodeId}`);
    expect(emptyPill.getAttribute("data-fixture-id")).toBe("fixture-cluster-screen-1-empty");
    fireEvent.click(emptyPill);
    expect(onSelectSubStateSpy).toHaveBeenCalledWith(firstNodeId, "EMPTY");
    expect(store.getState().nodes.find((n) => n.id === firstNodeId)?.activeSubState).toBe("EMPTY");
    expect(emptyPill.getAttribute("aria-pressed")).toBe("true");

    const errorPill = screen.getByTestId(`dag-substate-error-${firstNodeId}`);
    fireEvent.click(errorPill);
    expect(store.getState().nodes.find((n) => n.id === firstNodeId)?.activeSubState).toBe("ERROR");
    expect(errorPill.getAttribute("aria-pressed")).toBe("true");

    // Post-synthesis interactivity directly mutates live store & updates rendered canvas (no split-brain!)
    store.renameNode(firstNodeId, "Renamed Cluster One");
    await waitFor(() => {
      expect(screen.getByText("Renamed Cluster One")).toBeTruthy();
    });
    store.updateNodePosition(firstNodeId, 555, 222);
    await waitFor(() => {
      const card = screen.getByTestId(`dag-node-${firstNodeId}`);
      expect(card.style.left).toBe("555px");
      expect(card.style.top).toBe("222px");
    });
    store.deleteNode(plan.nodes[7].id);
    await waitFor(() => {
      expect(screen.queryByTestId(`dag-node-${plan.nodes[7].id}`)).toBeNull();
    });
  });

  it("UT-05.5-04: hydrates persisted substates on mount and resets stale metadata on projectId switch", async () => {
    const planA = buildSynthesizedPlanFixture(PROJECT_ID);
    const listSpy = vi
      .spyOn(apiFlowSynthesize, "listSubstates")
      .mockResolvedValueOnce({
        ok: true,
        data: {
          projectId: PROJECT_ID,
          items: planA.substates.map(({ payloadJson: _p, ...s }) => s),
          totalCount: planA.substates.length,
        },
        requestId: "req-hyd-a",
      })
      .mockResolvedValueOnce({
        ok: true,
        data: { projectId: PROJECT_B_ID, items: [], totalCount: 0 },
        requestId: "req-hyd-b",
      });

    const mapped = mapSynthesizedPlanToWorkbenchGraph(planA);
    const store = createNewFlowWorkbenchStore(mapped.nodes, mapped.transitions, {
      projectId: PROJECT_ID,
    });

    const { rerender } = render(
      <ReactiveDagCanvasHarness store={store} projectId={PROJECT_ID} />
    );

    const firstNodeId = planA.nodes[0].id;
    await waitFor(() => {
      expect(listSpy).toHaveBeenCalledWith({ projectId: PROJECT_ID }, expect.any(AbortSignal));
      expect(
        screen.getByTestId(`dag-substate-loading-${firstNodeId}`).getAttribute("data-fixture-id")
      ).toBe("fixture-cluster-screen-1-loading");
    });

    // Switch to Project B -> Project A fixture bindings & lens state MUST clear!
    rerender(<ReactiveDagCanvasHarness store={store} projectId={PROJECT_B_ID} />);
    await waitFor(() => {
      expect(listSpy).toHaveBeenCalledWith({ projectId: PROJECT_B_ID }, expect.any(AbortSignal));
      expect(
        screen.getByTestId(`dag-substate-loading-${firstNodeId}`).getAttribute("data-fixture-id")
      ).toBeNull();
    });
  });

  it("UT-05.5-05: BFF GET /api/v1/flows/synthesize passes through daemon VALIDATION_ERROR (400/422) instead of 503", async () => {
    const { sessionToken } = await issueBffLoopbackSession({
      tenantId: "tenant-a",
      role: "viewer",
    });
    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          error: {
            code: "VALIDATION_ERROR",
            message: "Invalid projectId format",
            field: "projectId",
            requestId: "req-bff-422",
          },
        }),
        { status: 422, headers: { "Content-Type": "application/json" } }
      )
    );

    const req = new Request(
      `http://127.0.0.1/api/v1/flows/synthesize?projectId=${PROJECT_ID}`,
      {
        method: "GET",
        headers: { "x-cowok-session-token": sessionToken },
      }
    );
    const res = await handleGetSubstatesBff(req);
    expect(res.status).toBe(422);
    const body = (await res.json()) as { error: { code: string; message: string } };
    expect(body.error.code).toBe("VALIDATION_ERROR");
  });
});
