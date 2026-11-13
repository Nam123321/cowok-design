// @story 05.1
import { guardedRoute } from "../../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../../lib/daemon-client";
import { CompileIntentSchema } from "../../../../../../lib/validations/sdi";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${projectId}/sdi/intent`);
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${projectId}/sdi/intent`, body);
  },
  { authMode: "strict-loopback", body: CompileIntentSchema }
);
