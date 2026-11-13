// @story 05.2
import { guardedRoute } from "../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const proposalId = ctx.params?.proposalId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/proposals/${encodeURIComponent(proposalId)}`
    );
  },
  { authMode: "strict-loopback" }
);
