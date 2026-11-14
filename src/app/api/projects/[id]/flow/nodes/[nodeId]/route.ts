// @story 04.1
// @implements AC-03, AC-04, AC-06
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { DeleteFlowNodeQuerySchema, UpdateFlowNodePayloadSchema } from "@/lib/validations/flow-workbench";

export const PATCH = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    return forwardToDaemonFlow({
      method: "PATCH",
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}`,
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
  { authMode: "strict-loopback", body: UpdateFlowNodePayloadSchema }
);

export const DELETE = guardedRoute(
  async (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const nodeId = ctx.params?.nodeId || "";
    const url = new URL(req.url, "http://127.0.0.1");
    return forwardToDaemonFlow({
      method: "DELETE",
      daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}`,
      rawQuery: url.search.replace(/^\?/, ""),
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", query: DeleteFlowNodeQuerySchema }
);
