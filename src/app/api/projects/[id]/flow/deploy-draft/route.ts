// @story 04.1
// @implements AC-03, AC-04, AC-06, AC-08
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { DeployFlowDraftPayloadSchema } from "@/lib/validations/flow-workbench";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/deploy-draft`,
      body
    );
  },
  { authMode: "strict-loopback", body: DeployFlowDraftPayloadSchema }
);
