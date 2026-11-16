// @story 05.2
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ComputeVisualDiffSchema } from "@/lib/validations/change-proposal";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const proposalId = ctx.params?.proposalId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/diff`,
      body
    );
  },
  { authMode: "strict-loopback", body: ComputeVisualDiffSchema }
);
