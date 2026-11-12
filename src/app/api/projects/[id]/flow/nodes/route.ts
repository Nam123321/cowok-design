// @story 04.1
// @implements AC-03, AC-04, AC-06
import { guardedRoute } from "../../../../../../lib/api-guard";
import { forwardToDaemonFlow } from "../../../../../../lib/daemon-client";
import { CreateFlowNodePayloadSchema } from "../../../../../../lib/validations/flow-workbench";

export const GET = guardedRoute(
  async (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "GET",
      daemonPath: `/api/v1/projects/${projectId}/flow/nodes`,
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

export const POST = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardToDaemonFlow({
      method: "POST",
      daemonPath: `/api/v1/projects/${projectId}/flow/nodes`,
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
  { authMode: "strict-loopback", body: CreateFlowNodePayloadSchema }
);
