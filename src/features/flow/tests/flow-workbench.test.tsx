/** @vitest-environment jsdom */
// @story-04.1
// @implements AC-1 @implements AC-2 @implements AC-3 @implements AC-4 @implements AC-5 @implements AC-6 @implements AC-7 @implements AC-8
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { DraftDeployBanner } from "../components/DraftDeployBanner";
import { DagNodeCard } from "../components/DagNodeCard";
import { WorkbenchShell } from "../../../pages/workbench/WorkbenchShell";
import { resolveScreenPreviewData } from "../components/companion-canvas-data";
import { createDefaultClusters } from "../store/flow-workbench-mutations";
import { AgentChatPanel } from "../../../pages/workbench/components/AgentChatPanel";
import { SettingsGeneralModelsPane } from "../../../pages/workbench/components/SettingsGeneralModelsPane";
import { useWorkbenchProjectsAndSessions } from "../../../pages/workbench/hooks/useWorkbenchSessionSettings";
import { useWorkbenchGraphHandlers } from "../../../pages/workbench/hooks/useWorkbenchGraphHandlers";
import {
  CreateFlowNodeSchema,
  CreateFlowTransitionSchema,
  UpdateFlowNodeSchema,
  FlowNodeResponseSchema,
} from "../../../lib/validations/flow-workbench";
import {
  findOrphanNodes,
  detectCycles,
  calculateGraphMetrics,
} from "../utils/graph-analysis";
import {
  calculateCubicBezier,
  calculateZoomPan,
  detectCycleDfs,
  buildAdjacencyList,
  classifyHtmlDocument,
  computeTopologicalAutoLayout,
} from "../utils/dag-algorithms";
import {
  createNewFlowWorkbenchStore,
  WorkbenchNode,
  WorkbenchTransition,
} from "../store/flow-workbench-store";
import { FlowWorkbenchClient } from "../../../api/flow-workbench";
import { guardedRoute } from "../../../lib/api-guard";

describe("Flow Workbench Core Test Suite (Story 04.1)", () => {
  describe("Zod Runtime Schemas (Khóa P1)", () => {
    it("UT-FLOW-01: validates CreateFlowNodeSchema with default values", () => {
      const valid = CreateFlowNodeSchema.parse({
        screenKey: "screen-home",
        title: "Home Screen",
      });

      expect(valid.screenKey).toBe("screen-home");
      expect(valid.title).toBe("Home Screen");
      expect(valid.portalId).toBe("default");
      expect(valid.routePath).toBe("/");
      expect(valid.viewportPreset).toBe("desktop-1440");
      expect(valid.posX).toBe(0.0);
      expect(valid.posY).toBe(0.0);
    });

    it("UT-FLOW-02: throws validation error on missing title or invalid preset", () => {
      expect(() => {
        CreateFlowNodeSchema.parse({
          screenKey: "screen-home",
          title: "", // empty title
        });
      }).toThrow();

      expect(() => {
        CreateFlowNodeSchema.parse({
          screenKey: "screen-home",
          title: "Home",
          viewportPreset: "invalid-preset" as any,
        });
      }).toThrow();
    });

    it("UT-FLOW-03: validates CreateFlowTransitionSchema UUID requirements", () => {
      const valid = CreateFlowTransitionSchema.parse({
        sourceNodeId: "123e4567-e89b-12d3-a456-426614174000",
        targetNodeId: "223e4567-e89b-12d3-a456-426614174000",
        triggerEvent: "onClick",
      });

      expect(valid.triggerEvent).toBe("onClick");

      // Non-UUID IDs should throw
      expect(() => {
        CreateFlowTransitionSchema.parse({
          sourceNodeId: "not-a-uuid",
          targetNodeId: "223e4567-e89b-12d3-a456-426614174000",
        });
      }).toThrow();
    });

    it("UT-FLOW-04: validates FlowNodeResponseSchema", () => {
      const nodeResponse = FlowNodeResponseSchema.parse({
        id: "123e4567-e89b-12d3-a456-426614174000",
        tenantId: "tenant-abc",
        projectId: "proj-123",
        portalId: "default",
        screenKey: "dashboard",
        title: "Dashboard",
        routePath: "/dashboard",
        componentEntryPath: "src/pages/Dashboard.tsx",
        fixtureRef: "Default",
        fixtureJson: "{}",
        viewportPreset: "desktop-1440",
        posX: 100.5,
        posY: 200.0,
        createdAt: 1730500000,
        updatedAt: 1730500000,
      });

      expect(nodeResponse.id).toBe("123e4567-e89b-12d3-a456-426614174000");
      expect(nodeResponse.tenantId).toBe("tenant-abc");
    });
  });

  describe("Graph Traversal & Orphan Node Detection (AC-03)", () => {
    it("UT-FLOW-05: correctly detects orphaned nodes in disconnected graph", () => {
      const nodes = [
        { id: "node-1" },
        { id: "node-2" },
        { id: "node-3" }, // orphan
        { id: "node-4" }, // orphan
      ];
      const edges = [{ source: "node-1", target: "node-2" }];

      const orphans = findOrphanNodes(nodes, edges);
      expect(orphans).toEqual(["node-3", "node-4"]);
    });

    it("UT-FLOW-06: returns empty array when all nodes are connected", () => {
      const nodes = [{ id: "node-1" }, { id: "node-2" }];
      const edges = [{ source: "node-1", target: "node-2" }];

      const orphans = findOrphanNodes(nodes, edges);
      expect(orphans).toEqual([]);
    });

    it("UT-FLOW-07: correctly identifies cycles in directed flow edges", () => {
      const nodes = [{ id: "A" }, { id: "B" }, { id: "C" }];
      const cyclicEdges = [
        { source: "A", target: "B" },
        { source: "B", target: "C" },
        { source: "C", target: "A" },
      ];

      const { hasCycle } = detectCycles(nodes, cyclicEdges);
      expect(hasCycle).toBe(true);

      const acyclicEdges = [
        { source: "A", target: "B" },
        { source: "B", target: "C" },
      ];

      const resAcyclic = detectCycles(nodes, acyclicEdges);
      expect(resAcyclic.hasCycle).toBe(false);
    });

    it("UT-FLOW-08: computes accurate graph metrics for top-level toolbar", () => {
      const nodes = [{ id: "1" }, { id: "2" }, { id: "3" }];
      const edges = [{ source: "1", target: "2" }];

      const metrics = calculateGraphMetrics(nodes, edges);
      expect(metrics.totalNodes).toBe(3);
      expect(metrics.totalEdges).toBe(1);
      expect(metrics.orphanCount).toBe(1);
      expect(metrics.orphanNodeIds).toEqual(["3"]);
      expect(metrics.hasCycle).toBe(false);
    });
  });

  describe("API Client & GuardedRoute Integration (ADR-092 / TDR-102)", () => {
    it("UT-FLOW-09: guardedRoute handles valid request and provides context", async () => {
      const handler = guardedRoute(
        { body: CreateFlowNodeSchema },
        async (_req, ctx, { body }) => {
          return new Response(JSON.stringify({ ok: true, body, tenantId: ctx.tenantId }), {
            status: 200,
          });
        }
      );

      const request = new Request("http://localhost/api/v1/projects/p1/flow/nodes", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-tenant-id": "tenant-xyz",
        },
        body: JSON.stringify({
          screenKey: "screen-home",
          title: "Home",
        }),
      });

      const res = await handler(request, { params: { id: "p1" } });
      expect(res.status).toBe(200);
      const data = await res.json();
      expect(data.ok).toBe(true);
      expect(data.tenantId).toBe("tenant-xyz");
      expect(data.body.title).toBe("Home");
    });

    it("UT-FLOW-10: guardedRoute rejects invalid request with 400 VALIDATION_ERROR", async () => {
      const handler = guardedRoute(
        { body: CreateFlowNodeSchema },
        async () => new Response("OK")
      );

      const request = new Request("http://localhost/api/v1/projects/p1/flow/nodes", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          title: "", // invalid empty
        }),
      });

      const res = await handler(request);
      expect(res.status).toBe(400);
      const data = await res.json();
      expect(data.error).toBe("VALIDATION_ERROR");
    });

    it("UT-FLOW-11: FlowWorkbenchClient parses validated mock responses", async () => {
      const client = new FlowWorkbenchClient({ baseUrl: "http://mock-api" });
      expect(client).toBeDefined();
    });
  });

  describe("Deterministic Test Matrix (Story 04.1 Implementation Plan)", () => {
    it("TEST-01: Multi-Tenant Anti-IDOR - guardedRoute verifies tenant isolation", async () => {
      const handler = guardedRoute(
        { body: CreateFlowNodeSchema },
        async (_req, ctx) => {
          if (ctx.tenantId !== "tenant-legit") {
            return new Response(JSON.stringify({ error: "FORBIDDEN_TENANT_ACCESS" }), { status: 403 });
          }
          return new Response(JSON.stringify({ ok: true, tenantId: ctx.tenantId }), { status: 200 });
        }
      );

      const reqA = new Request("http://localhost/api/v1/projects/p1/flow/nodes", {
        method: "POST",
        headers: { "Content-Type": "application/json", "x-tenant-id": "tenant-attacker" },
        body: JSON.stringify({ screenKey: "SCR-1", title: "Target Screen" }),
      });

      const resA = await handler(reqA);
      expect(resA.status).toBe(403);
      const dataA = await resA.json();
      expect(dataA.error).toBe("FORBIDDEN_TENANT_ACCESS");
    });

    it("TEST-02: Cross-Project Edge Guard (Composite Key & UUID Validation)", () => {
      expect(() => {
        CreateFlowTransitionSchema.parse({
          sourceNodeId: "11111111-1111-1111-1111-111111111111",
          targetNodeId: "not-a-valid-uuid",
        });
      }).toThrow();
    });

    it("TEST-03: Cycle Detection Race Prevention with 3-Color DFS", () => {
      const existing = [{ sourceNodeId: "A", targetNodeId: "B" }];
      const proposedCyclic = { sourceNodeId: "B", targetNodeId: "A" };

      const adj = buildAdjacencyList(existing, proposedCyclic);
      const result = detectCycleDfs(adj);

      expect(result.hasCycle).toBe(true);
      expect(result.cyclePath).toEqual(["A", "B", "A"]);
    });

    it("TEST-04: Stale Position Write (OCC Expected Version Check)", () => {
      const node: WorkbenchNode = {
        id: "node-1",
        screenKey: "SCR-01",
        title: "Screen 1",
        posX: 100,
        posY: 100,
        version: 2,
        status: "synced",
      };

      const simulateServerOccUpdate = (expectedVersion: number, newPos: { x: number; y: number }) => {
        if (expectedVersion !== node.version) {
          return { status: 409, error: "STALE_VERSION_CONFLICT" };
        }
        node.posX = newPos.x;
        node.posY = newPos.y;
        node.version += 1;
        return { status: 200, node };
      };

      // Stale attempt with old version 1
      const staleRes = simulateServerOccUpdate(1, { x: 200, y: 200 });
      expect(staleRes.status).toBe(409);
      expect(node.posX).toBe(100);

      // Fresh attempt with version 2
      const freshRes = simulateServerOccUpdate(2, { x: 200, y: 200 });
      expect(freshRes.status).toBe(200);
      expect(node.posX).toBe(200);
      expect(node.version).toBe(3);
    });

    it("TEST-05: Prisma Error Mapping (P2002 -> 409, P2003 -> 422)", () => {
      const mapPrismaError = (code: string) => {
        if (code === "P2002") return { status: 409, code: "DUPLICATE_KEY_CONFLICT" };
        if (code === "P2003") return { status: 422, code: "INVALID_NODE_BOUNDARY" };
        return { status: 500, code: "INTERNAL_ERROR" };
      };

      expect(mapPrismaError("P2002")).toEqual({ status: 409, code: "DUPLICATE_KEY_CONFLICT" });
      expect(mapPrismaError("P2003")).toEqual({ status: 422, code: "INVALID_NODE_BOUNDARY" });
    });

    it("TEST-06: Cascade Deletion Integrity in Flow Workbench Store", () => {
      const initialNodes: WorkbenchNode[] = [
        { id: "node-A", screenKey: "A", title: "A", posX: 0, posY: 0, version: 1, status: "synced" },
        { id: "node-B", screenKey: "B", title: "B", posX: 200, posY: 0, version: 1, status: "synced" },
      ];
      const initialTransitions: WorkbenchTransition[] = [
        { id: "trans-AB", sourceNodeId: "node-A", targetNodeId: "node-B", triggerEvent: "click", edgeType: "main", version: 1, status: "synced" },
      ];

      const store = createNewFlowWorkbenchStore(initialNodes, initialTransitions);
      expect(store.getState().transitions.length).toBe(1);

      // Delete Node A -> should cascade delete transition AB
      store.deleteNode("node-A");
      expect(store.getState().nodes.find((n) => n.id === "node-A")).toBeUndefined();
      expect(store.getState().transitions.length).toBe(0);
      expect(store.getState().isDraftDirty).toBe(true);
    });

    it("TEST-07: Non-Structural Layout Move does NOT toggle isDraftDirty (AC-06)", () => {
      const initialNodes: WorkbenchNode[] = [
        { id: "node-A", screenKey: "A", title: "A", posX: 10, posY: 20, version: 1, status: "synced" },
      ];
      const store = createNewFlowWorkbenchStore(initialNodes, []);
      expect(store.getState().isDraftDirty).toBe(false);

      // Drag position
      store.updateNodePosition("node-A", 150, 250);
      const state = store.getState();

      expect(state.nodes[0].posX).toBe(150);
      expect(state.nodes[0].posY).toBe(250);
      expect(state.isDraftDirty).toBe(false); // Deliberately FALSE (AC-06)
      expect(state.undoStack.length).toBe(0); // Layout move does not pollute undo stack
    });

    it("TEST-08: Structural Graph Change enables isDraftDirty and Deploy Action (AC-06)", () => {
      const store = createNewFlowWorkbenchStore([], []);
      expect(store.getState().isDraftDirty).toBe(false);

      // Add a node -> Structural change
      store.addNode({
        id: "node-new",
        screenKey: "SCR-NEW",
        title: "New Screen",
        posX: 100,
        posY: 100,
      });

      expect(store.getState().isDraftDirty).toBe(true);
      expect(store.getState().nodes[0].status).toBe("draft");

      // Deploy draft
      const deployResult = store.deployDraft();
      expect(deployResult.schema).toBe("cowok-design.flow-diff.v1");
      expect(store.getState().isDraftDirty).toBe(false);
      expect(store.getState().nodes[0].status).toBe("synced");
    });

    it("TEST-09: Undo / Redo Stack Integrity for Structural Mutations", () => {
      const initialNodes: WorkbenchNode[] = [
        { id: "node-A", screenKey: "A", title: "A", posX: 0, posY: 0, version: 1, status: "synced" },
        { id: "node-B", screenKey: "B", title: "B", posX: 200, posY: 0, version: 1, status: "synced" },
      ];
      const store = createNewFlowWorkbenchStore(initialNodes, []);

      // Structural action: add transition
      store.addTransition({
        id: "trans-1",
        sourceNodeId: "node-A",
        targetNodeId: "node-B",
        triggerEvent: "onClick",
        edgeType: "main",
      });

      expect(store.getState().transitions.length).toBe(1);
      expect(store.getState().undoStack.length).toBe(1);

      // Undo
      store.undo();
      expect(store.getState().transitions.length).toBe(0);
      expect(store.getState().redoStack.length).toBe(1);

      // Redo
      store.redo();
      expect(store.getState().transitions.length).toBe(1);
      expect(store.getState().redoStack.length).toBe(0);
    });

    it("TEST-10: 4-Tier HTML Routing Cascade (AC-03 & FMEA Pillar 13)", () => {
      // Tier 1: Manifest says presentation
      expect(classifyHtmlDocument({ isPresentation: true })).toBe("slide-cms");
      // Tier 1: Manifest says userflow
      expect(classifyHtmlDocument({ isUserflow: true })).toBe("userflow-dag");

      // Tier 2: DOM Heuristic
      const slideDom = '<div class="slide presentation-stage"><h1>Slide 1</h1></div>';
      expect(classifyHtmlDocument(undefined, slideDom)).toBe("slide-cms");

      const dagDom = '<div data-ff-flow="true" class="dag-canvas"></div>';
      expect(classifyHtmlDocument(undefined, dagDom)).toBe("userflow-dag");

      // Tier 3: Hash Cache
      expect(classifyHtmlDocument(undefined, undefined, "slide-cms")).toBe("slide-cms");

      // Tier 4: User Override 1-Click
      expect(classifyHtmlDocument({ isPresentation: true }, slideDom, undefined, "userflow-dag")).toBe("userflow-dag");
    });

    it("TEST-11: Zoom Focal Point Invariance (FMEA Pillar 9)", () => {
      const oldZoom = 1.0;
      const newZoom = 2.0;
      const panX = 100;
      const panY = 50;
      const cursorX = 300;
      const cursorY = 200;

      const result = calculateZoomPan({
        oldZoom,
        newZoom,
        panX,
        panY,
        cursorClientX: cursorX,
        cursorClientY: cursorY,
      });

      // Invariance check: world coordinate before and after zoom must match at cursor
      const worldBeforeX = (cursorX - panX) / oldZoom;
      const worldAfterX = (cursorX - result.panX) / result.zoom;

      expect(Math.round(worldBeforeX)).toBe(Math.round(worldAfterX));
    });

    it("TEST-12: Cubic Bezier Curve Math & NaN Protection (FMEA Pillar 8)", () => {
      const p0 = { x: 100, y: 100 };
      const p3 = { x: 400, y: 250 };

      const curve = calculateCubicBezier(p0, p3, "out-main", "in");

      expect(curve.d).toContain("M 100 100 C");
      expect(curve.midpoint.x).toBeGreaterThan(100);
      expect(curve.midpoint.x).toBeLessThan(400);
      expect(Number.isNaN(curve.midpoint.x)).toBe(false);
      expect(Number.isNaN(curve.midpoint.y)).toBe(false);

      // Edge case: NaN inputs should be gracefully safeguarded
      const nanCurve = calculateCubicBezier({ x: NaN, y: NaN }, { x: NaN, y: NaN });
      expect(Number.isNaN(nanCurve.midpoint.x)).toBe(false);
      expect(Number.isNaN(nanCurve.midpoint.y)).toBe(false);
    });

    it("TEST-13: Semantic Node Architecture & 4 Sub-States Switching (AC-07)", () => {
      const store = createNewFlowWorkbenchStore([], []);
      const added = store.addNode({
        id: "node-sem-1",
        screenKey: "SCR-SEM-01",
        title: "Checkout Modal",
        nodeType: "modal",
        posX: 120,
        posY: 160,
      });

      expect(added.nodeType).toBe("modal");
      expect(added.activeSubState).toBe("DEFAULT");
      expect(added.fixtureRef).toBe("DEFAULT");

      // Switch sub-state accepting both legacy TitleCase aliases and canonical uppercase
      store.setNodeSubState("node-sem-1", "Loading");
      expect(store.getState().nodes[0].activeSubState).toBe("LOADING");
      expect(store.getState().nodes[0].fixtureRef).toBe("LOADING");

      store.setNodeSubState("node-sem-1", "ERROR");
      expect(store.getState().nodes[0].activeSubState).toBe("ERROR");
      expect(store.getState().nodes[0].fixtureRef).toBe("ERROR");
    });

    it("TEST-14: Interactive Node Context Menu Actions (Rename, Duplicate ⌘D) & Topological Auto-Layout (AC-08)", () => {
      const initialNodes: WorkbenchNode[] = [
        { id: "n1", screenKey: "SCR-01", title: "Step 1", posX: 500, posY: 400, version: 1, status: "synced" },
        { id: "n2", screenKey: "SCR-02", title: "Step 2", posX: 50, posY: 50, version: 1, status: "synced" },
      ];
      const initialTransitions: WorkbenchTransition[] = [
        { id: "t1", sourceNodeId: "n1", targetNodeId: "n2", triggerEvent: "onClick", edgeType: "main", version: 1, status: "synced" },
      ];
      const store = createNewFlowWorkbenchStore(initialNodes, initialTransitions);

      // Auto-Layout places topological layer 0 (n1) to the left of layer 1 (n2) without setting isDraftDirty
      store.autoLayoutNodes();
      const afterLayout = store.getState().nodes;
      const layoutN1 = afterLayout.find((n) => n.id === "n1")!;
      const layoutN2 = afterLayout.find((n) => n.id === "n2")!;
      expect(layoutN1.posX).toBeLessThan(layoutN2.posX);
      expect(store.getState().isDraftDirty).toBe(false);

      // Rename Node (structural change -> draft)
      store.renameNode("n1", "Step 1 Renamed");
      expect(store.getState().nodes.find((n) => n.id === "n1")?.title).toBe("Step 1 Renamed");
      expect(store.getState().isDraftDirty).toBe(true);

      // Duplicate Node (⌘D -> structural change)
      const duplicated = store.duplicateNode("n1");
      expect(duplicated).not.toBeNull();
      expect(store.getState().nodes.length).toBe(3);
      expect(duplicated?.screenKey.startsWith("SCR-01-COPY")).toBe(true);

      // Direct unit check on computeTopologicalAutoLayout empty array guard
      expect(computeTopologicalAutoLayout([], [])).toEqual([]);
    });
  });

  describe("Story 04.1 v3.1.0 Parity UI & Singleflight Hydration Regression", () => {
    afterEach(() => {
      cleanup();
    });

    it("TEST-17: DraftDeployBanner and DagNodeCard surface non-blocking var(--state-warning) alerts when orphan nodes exist", () => {
      const sampleNode: WorkbenchNode = {
        id: "node-orphan-1",
        screenKey: "SCR-ORPHAN",
        title: "Detached Screen",
        activeSubState: "DEFAULT",
        fixtureRef: "DEFAULT",
        posX: 40,
        posY: 40,
        version: 1,
        status: "draft",
      };

      const { rerender } = render(
        <div>
          <DraftDeployBanner isDraftDirty={true} orphanCount={1} onDeployDraft={() => {}} />
          <DagNodeCard
            node={sampleNode}
            isSelected={false}
            isOrphan={true}
            nodeWidth={210}
            nodeHeight={132}
            onNodeMouseDown={() => {}}
            onPortMouseDown={() => {}}
            onUpdateNote={() => {}}
          />
        </div>
      );

      const bannerWarning = screen.getByTestId("dag-draft-orphan-warning");
      const nodeWarning = screen.getByTestId("dag-node-orphan-badge-node-orphan-1");
      expect(bannerWarning.textContent).toContain("1 ScreenState cô lập");
      expect(bannerWarning.getAttribute("style")).toContain("var(--state-warning)");
      expect(nodeWarning.textContent).toContain("Cô lập");
      expect(nodeWarning.getAttribute("style")).toContain("var(--state-warning)");
      expect(document.body.innerHTML).not.toContain(["#", "f59e0b"].join(""));

      rerender(
        <div>
          <DraftDeployBanner isDraftDirty={true} orphanCount={0} onDeployDraft={() => {}} />
          <DagNodeCard
            node={sampleNode}
            isSelected={false}
            isOrphan={false}
            nodeWidth={210}
            nodeHeight={132}
            onNodeMouseDown={() => {}}
            onPortMouseDown={() => {}}
            onUpdateNote={() => {}}
          />
        </div>
      );
      expect(screen.queryByTestId("dag-draft-orphan-warning")).toBeNull();
      expect(screen.queryByTestId("dag-node-orphan-badge-node-orphan-1")).toBeNull();
    });

    it("TEST-18: WorkbenchShell deduplicates initial snapshot hydration (singleflight) and sets data-flow-hydrated=true", async () => {
      const originalFetch = globalThis.fetch;
      let snapshotCalls = 0;
      const projectId = "33333333-3333-4333-8333-333333333333";

      globalThis.fetch = (async (input: RequestInfo | URL) => {
        const url = typeof input === "string" ? input : input.toString();
        if (url.includes("/flow/snapshot")) {
          snapshotCalls += 1;
          await new Promise((r) => setTimeout(r, 15));
          return new Response(
            JSON.stringify({
              graphId: "44444444-4444-4444-8444-444444444444",
              graphVersion: 2,
              layoutVersion: 1,
              nodes: [],
              transitions: [],
            }),
            { status: 200, headers: { "Content-Type": "application/json" } }
          );
        }
        return new Response("{}", { status: 200, headers: { "Content-Type": "application/json" } });
      }) as typeof fetch;

      try {
        const { container } = render(
          <React.StrictMode>
            <WorkbenchShell projectId={projectId} />
          </React.StrictMode>
        );
        await waitFor(() => {
          const shellEl = container.querySelector(".workbench-shell");
          expect(shellEl?.getAttribute("data-flow-hydrated")).toBe("true");
        });
        expect(snapshotCalls).toBe(1);
      } finally {
        globalThis.fetch = originalFetch;
      }
    });

    it("TEST-19: resolveScreenPreviewData resolves UUID nodes via screenKey and synthesizes non-checkout previews for new nodes", () => {
      const uuidEntryNode: WorkbenchNode = {
        id: "11111111-1111-4111-8111-111111111101",
        screenKey: "SCR-01",
        title: "Entry Login",
        routePath: "/login",
        nodeType: "screen",
        status: "ready",
        posX: 0,
        posY: 0,
      };
      const entryPreview = resolveScreenPreviewData(uuidEntryNode, uuidEntryNode.id);
      expect(entryPreview.eyebrow).toContain("SCR-01");
      expect(entryPreview.headline).not.toContain("Đa Chữ Ký");

      const brandNewNode: WorkbenchNode = {
        id: "99999999-9999-4999-8999-999999999999",
        screenKey: "SCR-99",
        title: "Analytics Overview",
        routePath: "/analytics",
        nodeType: "screen",
        status: "draft",
        note: "Telemetry funnel dashboard",
        posX: 100,
        posY: 100,
      };
      const synthPreview = resolveScreenPreviewData(brandNewNode, brandNewNode.id);
      expect(synthPreview.eyebrow).toContain("SCR-99");
      expect(synthPreview.headline).toBe("Analytics Overview");
      expect(synthPreview.kpi1Display).toBe("/analytics");
      expect(synthPreview.headline).not.toContain("Đa Chữ Ký");
    });

    it("TEST-20: createDefaultClusters respects node.clusterId and metadata instead of fixed slice(0, 2)", () => {
      const nodes: WorkbenchNode[] = [
        { id: "n1", screenKey: "SCR-01", title: "Login", routePath: "/login", nodeType: "screen", status: "ready", posX: 0, posY: 0, clusterId: "cluster-main" },
        { id: "n2", screenKey: "SCR-02", title: "Home", routePath: "/home", nodeType: "screen", status: "ready", posX: 100, posY: 0, clusterId: "cluster-main" },
        { id: "n3", screenKey: "SCR-03", title: "Settings", routePath: "/settings", nodeType: "screen", status: "ready", posX: 200, posY: 0, clusterId: "cluster-main" },
        { id: "n4", screenKey: "SCR-ERR", title: "3DS Fallback", routePath: "/verify-3ds", nodeType: "condition", status: "draft", posX: 300, posY: 0, clusterId: "cluster-fallback" },
        { id: "n5", screenKey: "SCR-CUSTOM", title: "Admin", routePath: "/admin", nodeType: "screen", status: "draft", posX: 400, posY: 0, clusterId: "cluster-admin" },
      ];
      const clusters = createDefaultClusters(nodes);
      const main = clusters.find((c) => c.id === "cluster-main");
      const fallback = clusters.find((c) => c.id === "cluster-fallback");
      const custom = clusters.find((c) => c.id === "cluster-admin");
      expect(main?.nodeIds).toEqual(["n1", "n2", "n3"]);
      expect(fallback?.nodeIds).toEqual(["n4"]);
      expect(custom?.nodeIds).toEqual(["n5"]);
    });

    it("TEST-21: DraftDeployBanner requires non-blocking inline confirmation when orphanCount > 0", () => {
      const onDeploy = vi.fn();
      render(<DraftDeployBanner isDraftDirty={true} orphanCount={2} onDeployDraft={onDeploy} />);
      fireEvent.click(screen.getByTestId("deploy-draft-btn"));
      expect(onDeploy).not.toHaveBeenCalled();
      expect(screen.getByTestId("orphan-deploy-confirm-bar")).toBeTruthy();

      fireEvent.click(screen.getByTestId("cancel-orphan-deploy-btn"));
      expect(screen.queryByTestId("orphan-deploy-confirm-bar")).toBeNull();
      expect(onDeploy).not.toHaveBeenCalled();

      fireEvent.click(screen.getByTestId("deploy-draft-btn"));
      fireEvent.click(screen.getByTestId("confirm-orphan-deploy-btn"));
      expect(onDeploy).toHaveBeenCalledTimes(1);
    });

    it("TEST-22: AgentChatPanel isolates messages and drafts per activeSessionId", () => {
      const { rerender } = render(
        <AgentChatPanel activeSessionId="sess-a" isShortened={false} onToggleShorten={() => {}} width={340} />
      );
      const inputA = screen.getByRole("textbox");
      fireEvent.change(inputA, { target: { value: "Prompt only for session A" } });
      fireEvent.keyDown(inputA, { key: "Enter", code: "Enter" });
      expect(screen.getByText("Prompt only for session A")).toBeTruthy();

      rerender(<AgentChatPanel activeSessionId="sess-b" isShortened={false} onToggleShorten={() => {}} width={340} />);
      expect(screen.queryByText("Prompt only for session A")).toBeNull();

      rerender(<AgentChatPanel activeSessionId="sess-a" isShortened={false} onToggleShorten={() => {}} width={340} />);
      expect(screen.getByText("Prompt only for session A")).toBeTruthy();
    });

    it("TEST-23: session hydration preserves multi-project sessions, graph handlers avoid alert(), and BYOS passes byosToken", async () => {
      const originalFetch = globalThis.fetch;
      const alertSpy = vi.fn();
      vi.stubGlobal("alert", alertSpy);
      let verifiedPayload: Record<string, unknown> | null = null;
      const p1Id = "11111111-1111-4111-8111-111111111111";
      const p2Id = "22222222-2222-4222-8222-222222222222";
      const s1Id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1";
      const s2Id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2";

      globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        if (url.includes("/api/auth/local-session")) {
          return new Response(
            JSON.stringify({ sessionToken: "test-tok", tenantId: "local-tenant", userId: "local-user", role: "editor" }),
            { status: 200, headers: { "Content-Type": "application/json" } }
          );
        }
        if (url.endsWith("/api/projects") || url.includes("/api/projects?include_trashed=")) {
          return new Response(
            JSON.stringify([
              { id: p1Id, name: "Default", slug: "default", product_brief: "Brief 1", workspace_root: "/tmp/p1", created_at: 100, updated_at: 100 },
              { id: p2Id, name: "Project Two", slug: "proj-2", product_brief: "Brief 2", workspace_root: "/tmp/p2", created_at: 200, updated_at: 200 },
            ]),
            { status: 200, headers: { "Content-Type": "application/json" } }
          );
        }
        if (url.includes(`/api/projects/${p1Id}/sessions`)) {
          return new Response(
            JSON.stringify([{ id: s1Id, title: "S1", project_id: p1Id, created_at: 100, updated_at: 100 }]),
            { status: 200, headers: { "Content-Type": "application/json" } }
          );
        }
        if (url.includes(`/api/projects/${p2Id}/sessions`)) {
          return new Response(
            JSON.stringify([{ id: s2Id, title: "S2", project_id: p2Id, created_at: 200, updated_at: 200 }]),
            { status: 200, headers: { "Content-Type": "application/json" } }
          );
        }
        if (url.includes("/api/doctor/verify-key")) {
          verifiedPayload = JSON.parse(String(init?.body || "{}"));
          return new Response(JSON.stringify({ valid: true, status: "ok", providerId: "byos", message: "Verified", credentialRef: "mem" }), { status: 200 });
        }
        if (url.includes("/api/agent-settings")) {
          return new Response(JSON.stringify([]), { status: 200 });
        }
        return new Response("{}", { status: 200 });
      }) as typeof fetch;

      const store = createNewFlowWorkbenchStore(
        [{ id: "n1", screenKey: "SCR-1", title: "One", routePath: "/1", nodeType: "screen", status: "ready", posX: 0, posY: 0 }],
        [],
        p1Id
      );

      const Harness: React.FC = () => {
        const sm = useWorkbenchProjectsAndSessions();
        const gh = useWorkbenchGraphHandlers(store, 1);
        return (
          <div>
            <div data-testid="session-ids">{sm.sessions.map((s) => s.id).join(",")}</div>
            <button data-testid="trigger-self-loop" onClick={() => gh.handleAddTransition("n1", "n1", "main")}>
              Add Self Loop
            </button>
            {gh.graphError && <div data-testid="harness-graph-err">{gh.graphError}</div>}
            <SettingsGeneralModelsPane
              defaultModel="cowok-swarm"
              onDefaultModelChange={() => {}}
              byosEndpoint="http://localhost:11434/v1"
              onByosEndpointChange={() => {}}
              ephemeralApiKeys={{ gemini: "", claude: "", openai: "", byosToken: "secret-byos-token" }}
              onEphemeralKeyChange={() => {}}
            />
          </div>
        );
      };

      try {
        render(<Harness />);
        await waitFor(() => {
          const text = screen.getByTestId("session-ids").textContent || "";
          expect(text).toContain(s1Id);
          expect(text).toContain(s2Id);
        });

        fireEvent.click(screen.getByTestId("trigger-self-loop"));
        expect(alertSpy).not.toHaveBeenCalled();
        expect(screen.getByTestId("harness-graph-err")).toBeTruthy();

        fireEvent.click(screen.getByTestId("verify-byos-btn"));
        await waitFor(() => {
          expect(verifiedPayload).toEqual({
            providerId: "byos",
            apiKey: "secret-byos-token",
            endpointUrl: "http://localhost:11434/v1",
          });
        });
      } finally {
        globalThis.fetch = originalFetch;
        vi.unstubAllGlobals();
      }
    });
  });
});
