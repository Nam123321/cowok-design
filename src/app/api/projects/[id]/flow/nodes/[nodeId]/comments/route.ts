// @story 04.3
// @implements AC-01, AC-02, AC-03
import { guardedRoute } from "../../../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../../../lib/daemon-client";
import { CreateElementCommentSchema } from "../../../../../../../../lib/validations/element-comment";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/flow/nodes/${encodeURIComponent(nodeId)}/comments`
    );
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/flow/nodes/${encodeURIComponent(nodeId)}/comments`,
      body
    );
  },
  { authMode: "strict-loopback", body: CreateElementCommentSchema }
);
