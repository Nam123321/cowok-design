// @story 05.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ForkTurnSnapshotSchema } from "@/lib/validations/version-snapshot";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const turnSnapshotId = ctx.params?.turnSnapshotId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/snapshots/${encodeURIComponent(turnSnapshotId)}/fork`,
      body
    );
  },
  { authMode: "strict-loopback", body: ForkTurnSnapshotSchema }
);
