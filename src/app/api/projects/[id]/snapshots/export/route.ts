// @story 05.3
import { guardedRoute } from "../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../lib/daemon-client";
import { ExportZipSchema } from "../../../../../../lib/validations/version-snapshot";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/snapshots/export`,
      body
    );
  },
  { authMode: "strict-loopback", body: ExportZipSchema }
);
