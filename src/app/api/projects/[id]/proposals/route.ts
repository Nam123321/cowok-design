// @story 05.2
import { guardedRoute } from "@/lib/api-guard";
import { forwardFromGuardedContext, forwardToDaemonFlow } from "@/lib/daemon-client";
import { CreateProposalSchema } from "@/lib/validations/change-proposal";

export const GET = guardedRoute(
  (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const daemonPath = `/api/v1/projects/${encodeURIComponent(projectId)}/proposals`;
    const url = new URL(req.url, "http://127.0.0.1");
    const nodeId = url.searchParams.get("nodeId")?.trim() || "";
    if (!nodeId) {
      return forwardToDaemonFlow({
        method: req.method,
        daemonPath,
        rawQuery: "",
        inboundHeaders: req.headers,
        principal: {
          tenantId: ctx.tenantId || "local-default",
          userId: ctx.userId || "local-operator",
          role: ctx.role || "viewer",
        },
        correlationId: ctx.correlationId,
      });
    }
    return forwardToDaemonFlow({
      method: req.method,
      daemonPath,
      rawQuery: `nodeId=${encodeURIComponent(nodeId)}`,
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

export const POST = guardedRoute(
  (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    return forwardFromGuardedContext(req, ctx, `/api/v1/projects/${encodeURIComponent(projectId)}/proposals`, body);
  },
  { authMode: "strict-loopback", body: CreateProposalSchema }
);
