// @story 05.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { OpenIdeHandoffSchema } from "@/lib/validations/version-snapshot";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(projectId)}/handoff/open-ide`,
      body
    );
  },
  { authMode: "strict-loopback", body: OpenIdeHandoffSchema }
);
