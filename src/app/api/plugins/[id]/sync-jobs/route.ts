// @story 08.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";
import { TriggerSyncRequestSchema } from "@/lib/validations/plugins";

export const POST = guardedRoute(
  (req, ctx, { body }) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/plugins/${encodeURIComponent(ctx.params?.id || "")}/sync-jobs`,
      body
    ),
  { authMode: "strict-loopback", body: TriggerSyncRequestSchema }
);
