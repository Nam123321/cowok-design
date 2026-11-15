// @story 04.1
// @implements AC-03, AC-04, AC-06
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { DeleteFlowNodeQuerySchema, UpdateFlowNodePayloadSchema } from "@/lib/validations/flow-workbench";

export const PATCH = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}`,
      body
    );
  },
  { authMode: "strict-loopback", body: UpdateFlowNodePayloadSchema }
);

export const DELETE = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}`
    );
  },
  { authMode: "strict-loopback", query: DeleteFlowNodeQuerySchema }
);
