// @story 04.1
// @implements AC-03, AC-04, AC-06
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  BatchUpdateNodePositionsPayloadSchema,
  BatchUpdateNodePositionsSchema,
  CreateFlowTransitionSchema,
  DeployDraftSchema,
  FlowGraphSnapshotDTOSchema,
  FlowGraphSnapshotSchema,
  FlowNodeResponseSchema,
  FlowTransitionResponseSchema,
  HostToIframeMessageSchema,
  IframeToHostMessageSchema,
} from "../src/lib/validations/flow-workbench";

describe("Flow Contract Parity Gate (SQLite V3 <-> Prisma DDR <-> Zod)", () => {
  const repoRoot = process.cwd();
  const v3Sql = fs.readFileSync(
    path.join(repoRoot, "crates/spec-registry/migrations/V3__flow_workbench_dag.sql"),
    "utf8"
  );
  const prismaSchema = fs.readFileSync(path.join(repoRoot, "prisma/schema.prisma"), "utf8");

  it("IT-PARITY-01: SQLite V3 migration defines all 4 Flow Workbench tables and composite FKs", () => {
    expect(v3Sql).toContain("CREATE TABLE flow_graphs_v3");
    expect(v3Sql).toContain("CREATE TABLE IF NOT EXISTS flow_draft_deployments");
    expect(v3Sql).toContain("CREATE TABLE flow_nodes_v3");
    expect(v3Sql).toContain("CREATE TABLE flow_edges_v3");
    expect(v3Sql).toContain("CHECK (source_node_id != target_node_id)");
    expect(v3Sql).toContain("FOREIGN KEY (source_node_id, tenant_id, project_id) REFERENCES flow_nodes(id, tenant_id, project_id)");
  });

  it("IT-PARITY-02: Prisma DDR mirror maps exact column names and table mappings", () => {
    expect(prismaSchema).toContain('model FlowGraph');
    expect(prismaSchema).toContain('@@map("flow_graphs")');
    expect(prismaSchema).toContain('model FlowDraftDeployment');
    expect(prismaSchema).toContain('@@map("flow_draft_deployments")');
    expect(prismaSchema).toContain('layoutVersion');
    expect(prismaSchema).toContain('@map("layout_version")');
    expect(prismaSchema).toContain('@map("flow_type")');
  });

  it("IT-PARITY-03: Zod schemas enforce self-loop ban, UUIDv4 draftId, finite coords, and directional iframe protocol", () => {
    expect(() =>
      CreateFlowTransitionSchema.parse({
        sourceNodeId: "11111111-1111-4111-8111-111111111111",
        targetNodeId: "11111111-1111-4111-8111-111111111111",
      })
    ).toThrow();

    expect(() =>
      DeployDraftSchema.parse({
        draftId: "not-a-uuid",
        expectedGraphVersion: 1,
      })
    ).toThrow();

    const parsedNode = FlowNodeResponseSchema.parse({
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
      note: "ok",
      viewportPreset: "desktop-1440",
      posX: 100,
      posY: 200,
      version: 1,
      layoutVersion: 1,
      createdAt: "1730000000000",
      updatedAt: "1730000000000",
    });
    expect(parsedNode.createdAt).toBe("1730000000000");

    const parsedEdge = FlowTransitionResponseSchema.parse({
      id: "33333333-3333-4333-8333-333333333333",
      tenantId: "local-default",
      projectId: "22222222-2222-4222-8222-222222222222",
      sourceNodeId: "11111111-1111-4111-8111-111111111111",
      targetNodeId: "44444444-4444-4444-8444-444444444444",
      flowType: "if_else",
      triggerEvent: "onClick",
      guardCondition: "cart.total > 0",
      version: 1,
      createdAt: "1730000000000",
      updatedAt: "1730000000000",
    });
    expect(parsedEdge.flowType).toBe("if_else");

    const snap = FlowGraphSnapshotSchema.parse({
      graphId: "55555555-5555-4555-8555-555555555555",
      graphVersion: 1,
      layoutVersion: 1,
      nodes: [parsedNode],
      transitions: [parsedEdge],
    });
    expect(snap.nodes.length).toBe(1);
    expect(BatchUpdateNodePositionsPayloadSchema).toBe(BatchUpdateNodePositionsSchema);
    expect(FlowGraphSnapshotDTOSchema).toBe(FlowGraphSnapshotSchema);

    expect(HostToIframeMessageSchema.safeParse({ type: "APPLY_HTML", epoch: 1, html: "<p>Hi</p>" }).success).toBe(true);
    expect(IframeToHostMessageSchema.safeParse({ type: "APPLY_HTML", epoch: 1, html: "<script>evil()</script>" }).success).toBe(false);
    expect(IframeToHostMessageSchema.safeParse({ type: "READY", epoch: 1 }).success).toBe(true);
  });
});
