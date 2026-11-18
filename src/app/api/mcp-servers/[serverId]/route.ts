// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext } from "@/lib/daemon-client";

export const DELETE = guardedRoute(
  (req, ctx) =>
    forwardFromGuardedContext(
      req,
      ctx,
      `/api/v1/mcp-servers/${encodeURIComponent(ctx.params?.serverId || "")}`
    ),
  { authMode: "strict-loopback" }
);
