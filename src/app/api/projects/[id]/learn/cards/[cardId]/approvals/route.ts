// @story 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { LearnCardDecisionRequestSchema } from "@/lib/validations/plugins-learn-cron";

export const POST = guardedRoute(
  (req, ctx, { body }) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(ctx.params?.id || "")}/learn/cards/${encodeURIComponent(ctx.params?.cardId || "")}/approvals`,
      body
    ),
  { authMode: "strict-loopback", body: LearnCardDecisionRequestSchema }
);
