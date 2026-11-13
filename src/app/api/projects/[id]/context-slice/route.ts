// @story 01.3, 02.1
// @implements AC-01, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { ContextSliceInputSchema } from "@/lib/validations/harness-daemon";

export const POST = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "POST",
      daemonPath: `/api/v1/projects/${projectId}/context-slice`,
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", body: ContextSliceInputSchema }
);
