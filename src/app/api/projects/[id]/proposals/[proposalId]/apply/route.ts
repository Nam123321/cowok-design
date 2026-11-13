// @story 05.2
import { guardedRoute } from "../../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../../lib/daemon-client";
import { ApplyProposalSchema } from "../../../../../../../lib/validations/change-proposal";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const proposalId = ctx.params?.proposalId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/proposals/${encodeURIComponent(proposalId)}/apply`,
      body
    );
  },
  { authMode: "strict-loopback", body: ApplyProposalSchema }
);
