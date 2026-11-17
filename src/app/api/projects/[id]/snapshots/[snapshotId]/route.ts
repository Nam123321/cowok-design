// @story 05.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const snapshotId = ctx.params?.snapshotId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/snapshots/${encodeURIComponent(snapshotId)}`
    );
  },
  { authMode: "strict-loopback" }
);
