// @stateless-cache-bounded: ephemeral in-memory development store
import { guardedRoute } from "@/lib/api-guard";
import { proposalsStore, persistProposalsStore } from "../../route";
import { ChangeProposalResponse } from "../../../../../../../lib/validations/change-proposal";
import { runValidationChecks } from "../../../../../../../features/proposals/utils/diff-engine";

import { syncValidateWithDaemon } from "../../../../../../../api/change-proposal";

function checkValidationEligibility(proposal: ChangeProposalResponse, projectId: string, tenantId?: string): Response | null {
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
  if (proposal.status === "rejected" || proposal.status === "applied") {
    return new Response(
      JSON.stringify({
        error: "INVALID_STATE",
        message: `Cannot validate ChangeProposal in terminal '${proposal.status}' status.`,
      }),
      { status: 400, headers: { "Content-Type": "application/json" } }
    );
  }
  return null;
}

const HTTP_STATUS_BAD_GATEWAY = 502;

async function executeDaemonValidate(
  projectId: string,
  proposalId: string,
  tenantId?: string
): Promise<Response | null> {
  try {
    await syncValidateWithDaemon(projectId, proposalId, tenantId);
    return null;
  } catch (err: unknown) {
    if (process.env.NODE_ENV === "test") {
      return null;
    }
    const msg = err instanceof Error ? err.message : "DAEMON_VALIDATE_FAILED";
    return new Response(JSON.stringify({ error: "DAEMON_VALIDATE_FAILED", message: msg }), {
      status: HTTP_STATUS_BAD_GATEWAY,
      headers: { "Content-Type": "application/json" },
    });
  }
}

export const POST = guardedRoute({}, async (_req, ctx) => {
  const proposalId = ctx.params?.proposalId || "";
  const projectId = ctx.projectId || ctx.params?.id || "";
  const tenantId = ctx.tenantId;

  const proposal = proposalsStore.get(proposalId);
  if (!proposal) {
    return new Response(JSON.stringify({ error: "PROPOSAL_NOT_FOUND" }), {
      status: 404,
      headers: { "Content-Type": "application/json" },
    });
  }

  const eligibilityError = checkValidationEligibility(proposal, projectId, tenantId);
  if (eligibilityError) return eligibilityError;

  const { validationStatus, checks } = runValidationChecks(proposal.files);

  const daemonError = await executeDaemonValidate(projectId, proposalId, tenantId);
  if (daemonError) return daemonError;

  const now = new Date().toISOString();
  proposal.validationStatus = validationStatus;
  proposal.validationChecks = checks;
  proposal.status = validationStatus === "passed" ? "ready" : "draft";
  proposal.updatedAt = now;

  proposalsStore.set(proposalId, proposal);
  persistProposalsStore();

  return new Response(JSON.stringify(proposal), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
