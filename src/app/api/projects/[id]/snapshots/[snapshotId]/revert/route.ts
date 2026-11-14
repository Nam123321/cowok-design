// @story 05.3
import { guardedRoute } from "../../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../../lib/daemon-client";
import { RevertSnapshotSchema } from "../../../../../../../lib/validations/version-snapshot";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const snapshotId = ctx.params?.snapshotId || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/snapshots/${encodeURIComponent(snapshotId)}/revert`,
      body
    );
  },
  { authMode: "strict-loopback", body: RevertSnapshotSchema }
);
