// @story 04.3, 04.5
// @implements Clean-Room EV-002 (Synced CLI & BYOS Provider Usage / 5h & Weekly Quota Probe)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";

export const GET = guardedRoute(
  (req, ctx) =>
    forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/doctor/usage",
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
