// @story 05.1
import { guardedRoute } from "../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../lib/daemon-client";
import { ClarificationSelectSchema } from "../../../../../../lib/validations/sdi";

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${projectId}/sdi/clarification`,
      body
    );
  },
  { authMode: "strict-loopback", body: ClarificationSelectSchema }
);
