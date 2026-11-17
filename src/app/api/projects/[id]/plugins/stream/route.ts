// @story 08.4
// @implements AC-02, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/projects/${encodeURIComponent(String(ctx.params?.id || ""))}/plugins/stream`
    ),
  { authMode: "strict-loopback" }
);
