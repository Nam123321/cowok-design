// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { proposalsStore } from "../route";

export const GET = guardedRoute({}, async (_req, ctx) => {
  const proposalId = ctx.params?.proposalId || "";
  const tenantId = ctx.tenantId;

  const proposal = proposalsStore.get(proposalId);
  if (!proposal) {
    return new Response(JSON.stringify({ error: "PROPOSAL_NOT_FOUND" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  const projectId = ctx.projectId || ctx.params?.id || "";
  if (projectId && proposal.projectId !== projectId) {
    return new Response(JSON.stringify({ error: "PROJECT_MISMATCH" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  if (tenantId && proposal.tenantId !== tenantId) {
    return new Response(JSON.stringify({ error: "TENANT_MISMATCH" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  return new Response(JSON.stringify(proposal), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
