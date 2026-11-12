// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { proposalsStore, persistProposalsStore } from "../../route";
import {
  ApplyProposalSchema,
  ChangeProposalResponse,
} from "../../../../../../../lib/validations/change-proposal";
import { applyProposalFiles } from "../../../../../../../features/proposals/utils/diff-engine";

import { syncApplyWithDaemon } from "../../../../../../../api/change-proposal";

function checkApplyPreconditions(proposal: ChangeProposalResponse): Response | null {
  const hasFailedCheck = proposal.validationChecks.some((c) => c.status === "failed");
  if (proposal.validationStatus !== "passed" || hasFailedCheck) {
    return new Response(
      JSON.stringify({
        error: "PRECONDITION_FAILED",
        message: "Cannot apply ChangeProposal: All automated validation checks must pass first.",
      }),
      { status: 400, headers: { "Content-Type": "application/json" } }
    );
  }
  if (proposal.status !== "ready") {
    return new Response(
      JSON.stringify({
        error: "INVALID_STATE",
        message: `Cannot apply ChangeProposal in '${proposal.status}' status. Only 'ready' proposals can be applied.`,
      }),
      { status: 400, headers: { "Content-Type": "application/json" } }
    );
  }
  return null;
}

const HTTP_STATUS_BAD_GATEWAY = 502;

async function executeDaemonApply(
  projectId: string,
  proposalId: string,
  tenantId?: string
): Promise<Response | null> {
  try {
    await syncApplyWithDaemon(projectId, proposalId, tenantId);
    return null;
  } catch (err: unknown) {
    if (process.env.NODE_ENV === "test") {
      return null;
    }
    const msg = err instanceof Error ? err.message : "DAEMON_APPLY_FAILED";
    return new Response(JSON.stringify({ error: "DAEMON_APPLY_FAILED", message: msg }), {
      status: HTTP_STATUS_BAD_GATEWAY,
      headers: { "Content-Type": "application/json" },
    });
  }
}

export const POST = guardedRoute({ body: ApplyProposalSchema }, async (_req, ctx) => {
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

  const preconditionError = checkApplyPreconditions(proposal);
  if (preconditionError) return preconditionError;

  const daemonError = await executeDaemonApply(projectId, proposalId, tenantId);
  if (daemonError) return daemonError;

  const now = new Date().toISOString();
  proposal.status = "applied";
  proposal.updatedAt = now;
  applyProposalFiles(proposal.files);
  proposalsStore.set(proposalId, proposal);
  persistProposalsStore();

  return new Response(JSON.stringify(proposal), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
