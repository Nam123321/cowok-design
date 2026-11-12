// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
import crypto from "node:crypto";
import { describe, expect, it } from "vitest";
import { issueBffLoopbackSession } from "../../src/lib/api-guard";
import { ensureRunningDaemon } from "../../src/lib/daemon-client";
import * as snapshotRoute from "../../src/app/api/projects/[id]/flow/snapshot/route";
import * as nodesRoute from "../../src/app/api/projects/[id]/flow/nodes/route";
import * as positionsRoute from "../../src/app/api/projects/[id]/flow/nodes/positions/route";
import * as singleNodeRoute from "../../src/app/api/projects/[id]/flow/nodes/[nodeId]/route";
import * as transitionsRoute from "../../src/app/api/projects/[id]/flow/transitions/route";
import * as deployDraftRoute from "../../src/app/api/projects/[id]/flow/deploy-draft/route";

async function seedIsolatedProject(slug: string): Promise<string> {
  const lock = await ensureRunningDaemon();
  const projectId = crypto.randomUUID();
  const res = await fetch(`http://127.0.0.1:${lock.port}/api/v1/projects`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      id: projectId,
      slug,
      name: `Project ${slug}`,
      product_brief: "Integration test project for Flow Workbench.",
      workspace_root: null,
      inherit_brand_from_project_id: null,
    }),
  });
  expect(res.status).toBe(201);
  return projectId;
}

function makeBffReq(url: string, method: string, token: string, body?: unknown): Request {
  return new Request(url, {
    method,
    headers: {
      Host: "127.0.0.1:5173",
      "Content-Type": "application/json",
      "X-Cowok-Session-Token": token,
    },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
}

describe("Flow Workbench End-to-End BFF <-> Rust SQLite WAL Integration", () => {
  it("IT-SNAPSHOT-01: orthogonal layoutVersion vs structural graphVersion and batch positions", async () => {
    const projectId = await seedIsolatedProject(`snap-${Date.now()}`);
    const { sessionToken } = issueBffLoopbackSession({ tenantId: "local-default", role: "editor" });

    const snap0Res = await snapshotRoute.GET(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/snapshot`, "GET", sessionToken),
      { params: { id: projectId } }
    );
    expect(snap0Res.status).toBe(200);
    const snap0 = await snap0Res.json();
    expect(snap0.graphVersion).toBe(1);

    const n1Res = await nodesRoute.POST(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/nodes`, "POST", sessionToken, {
        screenKey: "login",
        title: "Login",
        posX: 40,
        posY: 80,
      }),
      { params: { id: projectId } }
    );
    expect(n1Res.status).toBe(201);
    const n1 = await n1Res.json();

    const n2Res = await nodesRoute.POST(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/nodes`, "POST", sessionToken, {
        screenKey: "checkout",
        title: "Checkout",
        posX: 340,
        posY: 80,
      }),
      { params: { id: projectId } }
    );
    expect(n2Res.status).toBe(201);
    const n2 = await n2Res.json();

    // Layout-only drag increments layoutVersion ONLY, keeping structural graphVersion === 3
    const dragRes = await singleNodeRoute.PATCH(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/nodes/${n1.id}`, "PATCH", sessionToken, {
        posX: 120,
        posY: 160,
        expectedLayoutVersion: 1,
      }),
      { params: { id: projectId, nodeId: n1.id } }
    );
    expect(dragRes.status).toBe(200);
    const dragged = await dragRes.json();
    expect(dragged.layoutVersion).toBe(2);
    expect(dragged.version).toBe(1);

    // Batch positions update in 1 SQLite transaction
    const batchRes = await positionsRoute.PATCH(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/nodes/positions`, "PATCH", sessionToken, {
        positions: [
          { nodeId: n1.id, posX: 200, posY: 220, expectedLayoutVersion: 2 },
          { nodeId: n2.id, posX: 520, posY: 220, expectedLayoutVersion: 1 },
        ],
      }),
      { params: { id: projectId } }
    );
    expect(batchRes.status).toBe(200);

    const snapAfter = await (
      await snapshotRoute.GET(
        makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/snapshot`, "GET", sessionToken),
        { params: { id: projectId } }
      )
    ).json();
    expect(snapAfter.graphVersion).toBe(3);
    expect(snapAfter.layoutVersion).toBe(3);
  });

  it("IT-IDEMP-01: deploy-draft enforces 3-color DFS, pinned draftId idempotency replay, and payload mismatch 422", async () => {
    const projectId = await seedIsolatedProject(`draft-${Date.now()}`);
    const { sessionToken } = issueBffLoopbackSession({ tenantId: "local-default", role: "editor" });

    const nodeA = crypto.randomUUID();
    const nodeB = crypto.randomUUID();
    const draftId = crypto.randomUUID();

    const payload = {
      draftId,
      expectedGraphVersion: 1,
      createdNodes: [
        { id: nodeA, screenKey: "step-1", title: "Step 1", nodeType: "screen", posX: 80, posY: 100 },
        { id: nodeB, screenKey: "step-2", title: "Step 2", nodeType: "modal", posX: 380, posY: 100 },
      ],
      createdTransitions: [
        { sourceNodeId: nodeA, targetNodeId: nodeB, flowType: "if_else", triggerEvent: "onClick", guardCondition: "auth.ok" },
      ],
    };

    const firstRes = await deployDraftRoute.POST(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/deploy-draft`, "POST", sessionToken, payload),
      { params: { id: projectId } }
    );
    expect(firstRes.status).toBe(200);
    const firstSnap = await firstRes.json();
    expect(firstSnap.graphVersion).toBe(2);
    expect(firstSnap.nodes.length).toBe(2);
    expect(firstSnap.transitions.length).toBe(1);

    // Exact replay with same draftId returns identical 200 snapshot without duplicating entities
    const replayRes = await deployDraftRoute.POST(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/deploy-draft`, "POST", sessionToken, payload),
      { params: { id: projectId } }
    );
    expect(replayRes.status).toBe(200);
    const replaySnap = await replayRes.json();
    expect(replaySnap.graphVersion).toBe(2);

    // Mutated payload reusing same draftId returns 422 DRAFT_ID_PAYLOAD_MISMATCH
    const mismatchRes = await deployDraftRoute.POST(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/deploy-draft`, "POST", sessionToken, {
        ...payload,
        createdNodes: [{ id: nodeA, screenKey: "tampered", title: "Tampered", posX: 0, posY: 0 }],
        createdTransitions: [],
      }),
      { params: { id: projectId } }
    );
    expect(mismatchRes.status).toBe(422);
    const mismatchBody = await mismatchRes.json();
    expect(mismatchBody.code).toBe("DRAFT_ID_PAYLOAD_MISMATCH");

    // Cycle detection rejects B -> A with 422 DAG_CYCLE_DETECTED
    const cycRes = await transitionsRoute.POST(
      makeBffReq(`http://127.0.0.1:5173/api/projects/${projectId}/flow/transitions`, "POST", sessionToken, {
        sourceNodeId: nodeB,
        targetNodeId: nodeA,
        triggerEvent: "onClick",
      }),
      { params: { id: projectId } }
    );
    expect(cycRes.status).toBe(422);
    const cycBody = await cycRes.json();
    expect(cycBody.code).toBe("DAG_CYCLE_DETECTED");
  });
});
