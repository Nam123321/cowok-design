// @story 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { ProposeLearnCardRequestSchema } from "@/lib/validations/plugins-learn-cron";

export const GET = guardedRoute(
  (req, ctx) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(ctx.params?.id || "")}/learn/cards`
    ),
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  (req, ctx, { body }) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(ctx.params?.id || "")}/learn/cards`,
      body
    ),
  { authMode: "strict-loopback", body: ProposeLearnCardRequestSchema }
);
