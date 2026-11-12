// @story 04.1
// @implements AC-03, AC-04, AC-06
import { guardedRoute } from "../../../../../../lib/api-guard";
import { forwardToDaemonFlow } from "../../../../../../lib/daemon-client";

export const GET = guardedRoute(
  async (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "GET",
      daemonPath: `/api/v1/projects/${projectId}/flow/snapshot`,
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
