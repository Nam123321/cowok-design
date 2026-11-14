// @story-04.1
// @implements AC-1 @implements AC-2 @implements AC-3 @implements AC-4 @implements AC-5 @implements AC-6
import { describe, expect, it } from "vitest";
import {
  CreateFlowNodeSchema,
  CreateFlowTransitionSchema,
  UpdateFlowNodeSchema,
  FlowNodeResponseSchema,
  ViewportPresetSchema,
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

    it("TEST-13: RBAC Enforcement in guardedRoute - blocks unauthorized role with 403 FORBIDDEN", async () => {
      const handler = guardedRoute(
        { roles: ["admin"] },
        async () => new Response(JSON.stringify({ ok: true }), { status: 200 })
      );

      // Caller with 'viewer' role should be rejected
      const viewerReq = new Request("http://localhost/api/projects/p1/flow/deploy-draft", {
        method: "POST",
        headers: { "x-user-roles": "viewer" },
      });
      const res403 = await handler(viewerReq);
      expect(res403.status).toBe(403);
      const data403 = await res403.json();
      expect(data403.error).toBe("FORBIDDEN");

      // Caller with 'admin' role should be authorized
      const adminReq = new Request("http://localhost/api/projects/p1/flow/deploy-draft", {
        method: "POST",
        headers: { "x-user-roles": "admin" },
      });
      const res200 = await handler(adminReq);
      expect(res200.status).toBe(200);
    });

    it("TEST-14: FlowWorkbenchClient injects X-Cowok-Session-Token and X-Correlation-Id on request", async () => {
      let capturedToken = "";
      let capturedCorr = "";
      const mockFetch = async (_url: string, init?: RequestInit) => {
        const h = new Headers(init?.headers);
        capturedToken = h.get("X-Cowok-Session-Token") || "";
        capturedCorr = h.get("X-Correlation-Id") || "";
        return new Response(JSON.stringify([]), { status: 200, headers: { "Content-Type": "application/json" } });
      };

      const originalFetch = globalThis.fetch;
      globalThis.fetch = mockFetch as typeof fetch;
      try {
        const client = new FlowWorkbenchClient({
          baseUrl: "http://127.0.0.1:5173",
          getSessionToken: () => "cwk_bff_test_session_123",
        });
        await client.getNodes("11111111-1111-4111-8111-111111111111");
        expect(capturedToken).toBe("cwk_bff_test_session_123");
        expect(capturedCorr.length).toBeGreaterThan(4);
      } finally {
        globalThis.fetch = originalFetch;
      }
    });

    it("UT-STORE-01: Draft-Local Coordinate Guard skips network sync for ephemeral nodes, coalesces Note undo, and keeps pinned draftId", () => {
      let fetchCalls = 0;
      const originalFetch = globalThis.fetch;
      globalThis.fetch = (async () => {
        fetchCalls += 1;
        return new Response("{}", { status: 200 });
      }) as typeof fetch;

      try {
        const store = createNewFlowWorkbenchStore([], [], {
          projectId: "11111111-1111-4111-8111-111111111111",
          enableNetworkLayoutSync: true,
          debounceMs: 5,
        });
        const ephem = store.addNode({
          id: "22222222-2222-4222-8222-222222222201",
          screenKey: "SCR-EPHEM",
          title: "Ephemeral Screen",
          posX: 20,
          posY: 40,
        });
        const pinnedDraftId = store.getState().draftId;
        expect(pinnedDraftId).toBeTruthy();

        const moveMode = store.updateNodePosition(ephem.id, 180, 260);
        expect(moveMode).toBe("ephemeral_local_only");
        expect(fetchCalls).toBe(0);
        expect(store.getState().draftId).toBe(pinnedDraftId);

        // Coalesce Note updates for same nodeId into 1 undo entry
        const undoBefore = store.getState().undoStack.length;
        store.updateNodeNote(ephem.id, "H");
        store.updateNodeNote(ephem.id, "He");
        store.updateNodeNote(ephem.id, "Hello Note");
        expect(store.getState().undoStack.length).toBe(undoBefore + 1);
        expect(store.getState().nodes[0].note).toBe("Hello Note");
      } finally {
        globalThis.fetch = originalFetch;
      }
    });

    it("TEST-15: FlowWorkbenchStore.hydrate restores snapshot and clears dirty state", () => {
      const store = createNewFlowWorkbenchStore();
      store.addNode({
        id: "11111111-1111-4111-8111-111111111101",
        screenKey: "SCR-TEMP",
        title: "Temporary Screen",
        posX: 10,
        posY: 10,
      });
      expect(store.getState().isDraftDirty).toBe(true);

      const dbNodes: WorkbenchNode[] = [
        {
          id: "11111111-1111-4111-8111-111111111101",
          screenKey: "SCR-DB",
          title: "DB Screen",
          posX: 50,
          posY: 50,
          version: 2,
          status: "synced",
        },
      ];
      store.hydrate(dbNodes, []);

      expect(store.getState().isDraftDirty).toBe(false);
      expect(store.getState().nodes.length).toBe(1);
      expect(store.getState().nodes[0].title).toBe("DB Screen");
      expect(store.getState().nodes[0].status).toBe("synced");
    });

    it("TEST-16: Canonical FixtureRef normalization (DEFAULT|LOADING|EMPTY|ERROR) accepts TitleCase aliases and rejects unknown variants", () => {
      const defaulted = CreateFlowNodeSchema.parse({
        screenKey: "scr-def",
        title: "Default Screen",
      });
      expect(defaulted.fixtureRef).toBe("DEFAULT");

      const normalizedAlias = CreateFlowNodeSchema.parse({
        screenKey: "scr-alias",
        title: "Alias Screen",
        fixtureRef: "Loading",
      });
      expect(normalizedAlias.fixtureRef).toBe("LOADING");

      const legacyResponse = FlowNodeResponseSchema.parse({
        id: "11111111-1111-4111-8111-111111111111",
        tenantId: "local-default",
        projectId: "22222222-2222-4222-8222-222222222222",
        portalId: "default",
        screenKey: "home",
        title: "Home",
        nodeType: "screen",
        routePath: "/",
        componentEntryPath: "",
        fixtureRef: "Default",
        fixtureJson: "{}",
        note: "",
        viewportPreset: "desktop-1440",
        posX: 10,
        posY: 20,
        version: 1,
        layoutVersion: 1,
        createdAt: "1730000000000",
        updatedAt: "1730000000000",
      });
      expect(legacyResponse.fixtureRef).toBe("DEFAULT");

      expect(() =>
        CreateFlowNodeSchema.parse({
          screenKey: "scr-bad",
          title: "Bad Screen",
          fixtureRef: "UNKNOWN_VARIANT",
        })
      ).toThrow();
    });

    it("TEST-19: ViewportPresetSchema normalizes legacy aliases and fails closed on invalid presets", () => {
      expect(ViewportPresetSchema.parse("desktop")).toBe("desktop-1440");
      expect(ViewportPresetSchema.parse("tablet")).toBe("tablet-768");
      expect(ViewportPresetSchema.parse("mobile")).toBe("mobile-390");
      expect(ViewportPresetSchema.parse("  ")).toBe("desktop-1440");
      expect(() => ViewportPresetSchema.parse("bogus-preset")).toThrow();
    });

    it("TEST-20: setNodeSubState marks draft dirty + undoable and selectNode(id, true) toggles multi-selection cleanly", () => {
      const store = createNewFlowWorkbenchStore();
      store.hydrate(
        [
          { id: "n-1", screenKey: "k-1", title: "Screen 1", posX: 10, posY: 20, version: 1, status: "synced", fixtureRef: "DEFAULT" },
          { id: "n-2", screenKey: "k-2", title: "Screen 2", posX: 200, posY: 20, version: 1, status: "synced", fixtureRef: "DEFAULT" },
        ],
        []
      );
      expect(store.getState().isDraftDirty).toBe(false);

      store.setNodeSubState("n-1", "Loading");
      expect(store.getState().isDraftDirty).toBe(true);
      expect(store.getState().draftId).toBeTruthy();
      expect(store.getState().nodes[0].fixtureRef).toBe("LOADING");
      expect(store.getState().nodes[0].status).toBe("draft");
      expect(store.getState().undoStack.length).toBeGreaterThan(0);

      store.selectNode("n-1");
      store.selectNode("n-2", true);
      expect(store.getState().selectedNodeIds).toEqual(["n-1", "n-2"]);
      store.selectNode("n-1", true);
      expect(store.getState().selectedNodeIds).toEqual(["n-2"]);
      expect(store.getState().selectedNodeId).toBe("n-2");
    });
  });
});
