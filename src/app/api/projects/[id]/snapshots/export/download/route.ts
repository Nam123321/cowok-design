// @story 05.3
import { guardedRoute } from "../../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../../lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/snapshots/export/download`
    );
  },
  { authMode: "strict-loopback" }
);
