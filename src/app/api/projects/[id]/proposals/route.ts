// @story 05.2
import { guardedRoute } from "../../../../../lib/api-guard";
import { forwardFromGuardedContext } from "../../../../../lib/daemon-client";
import { CreateProposalSchema } from "../../../../../lib/validations/change-proposal";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${projectId}/proposals`);
  },
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${projectId}/proposals`, body);
  },
  { authMode: "strict-loopback", body: CreateProposalSchema }
);
