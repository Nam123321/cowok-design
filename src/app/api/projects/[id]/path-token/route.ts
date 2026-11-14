// @story 04.2, 01.3
// @implements AC-01, AC-03, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { IssuePathTokenInputSchema } from "@/lib/validations/harness-daemon";

export const POST = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "POST",
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/path-token`,
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", body: IssuePathTokenInputSchema }
);
