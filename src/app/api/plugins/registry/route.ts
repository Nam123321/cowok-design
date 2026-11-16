// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";

export const GET = guardedRoute(
  async (req, ctx) =>
    forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/plugins",
      rawQuery: new URL(req.url, "http://127.0.0.1").search.slice(1),
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    }),
  { authMode: "strict-loopback" }
);
