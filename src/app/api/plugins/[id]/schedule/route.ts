// @story 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { UpsertCronScheduleRequestSchema } from "@/lib/validations/plugins-learn-cron";

export const GET = guardedRoute(
  (req, ctx) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/plugins/${encodeURIComponent(ctx.params?.id || "")}/schedule`
    ),
  { authMode: "strict-loopback" }
);

export const PUT = guardedRoute(
  (req, ctx, { body }) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/plugins/${encodeURIComponent(ctx.params?.id || "")}/schedule`,
      body
    ),
  { authMode: "strict-loopback", body: UpsertCronScheduleRequestSchema }
);
