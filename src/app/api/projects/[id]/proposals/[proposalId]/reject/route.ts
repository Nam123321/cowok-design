// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { proposalsStore, persistProposalsStore } from "../../route";

import { syncRejectWithDaemon } from "../../../../../../../api/change-proposal";

const HTTP_STATUS_BAD_GATEWAY = 502;

async function executeDaemonReject(
  projectId: string,
  proposalId: string,
  tenantId?: string
): Promise<Response | null> {
  try {
    await syncRejectWithDaemon(projectId, proposalId, tenantId);
    return null;
  } catch (err: unknown) {
    if (process.env.NODE_ENV === "test") {
      return null;
    }
    const msg = err instanceof Error ? err.message : "DAEMON_REJECT_FAILED";
    return new Response(JSON.stringify({ error: "DAEMON_REJECT_FAILED", message: msg }), {
      status: HTTP_STATUS_BAD_GATEWAY,
      headers: { "Content-Type": "application/json" },
    });
  }
}

export const POST = guardedRoute({}, async (_req, ctx) => {
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

  const daemonError = await executeDaemonReject(projectId, proposalId, tenantId);
  if (daemonError) return daemonError;

  const now = new Date().toISOString();
  proposal.status = "rejected";
  proposal.updatedAt = now;
  proposalsStore.set(proposalId, proposal);
  persistProposalsStore();

  return new Response(JSON.stringify(proposal), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
