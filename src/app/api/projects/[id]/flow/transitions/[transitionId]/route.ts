// @story 04.1
// @implements AC-03, AC-04, AC-06
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { DeleteTransitionQuerySchema, UpdateFlowTransitionPayloadSchema } from "@/lib/validations/flow-workbench";

export const PATCH = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const transitionId = ctx.params?.transitionId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/transitions/${encodeURIComponent(transitionId)}`,
      body
    );
  },
  { authMode: "strict-loopback", body: UpdateFlowTransitionPayloadSchema }
);

export const DELETE = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const transitionId = ctx.params?.transitionId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/transitions/${encodeURIComponent(transitionId)}`
    );
  },
  { authMode: "strict-loopback", query: DeleteTransitionQuerySchema }
);
