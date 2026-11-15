// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";

export const DELETE = guardedRoute(
  async (req, ctx) => {
    const serverId = ctx.params?.serverId || "";
    return forwardToDaemonFlow({
      method: "DELETE",
      daemonPath: `/api/v1/mcp-servers/${encodeURIComponent(serverId)}`,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback" }
);
