// @story 04.1
// @implements AC-03, AC-04, AC-06
import { guardedRoute } from "../../../../../../../lib/api-guard";
import { forwardToDaemonFlow } from "../../../../../../../lib/daemon-client";
import { BatchUpdateNodePositionsSchema } from "../../../../../../../lib/validations/flow-workbench";

export const PATCH = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "PATCH",
      daemonPath: `/api/v1/projects/${projectId}/flow/nodes/positions`,
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
  { authMode: "strict-loopback", body: BatchUpdateNodePositionsSchema }
);
