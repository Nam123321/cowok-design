// @story 01.3
// @implements AC-01, AC-02, AC-03
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";

export const GET = guardedRoute(
  async (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "GET",
      daemonPath: `/api/v1/projects/${projectId}/specs`,
      rawQuery: new URL(req.url).search.slice(1),
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback" }
);
