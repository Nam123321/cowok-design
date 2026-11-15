import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { RestoreProjectInputSchema } from "@/lib/validations/harness-daemon";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/restore`,
      body
    );
  },
  { authMode: "strict-loopback", body: RestoreProjectInputSchema }
);
