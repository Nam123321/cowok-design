// @story 04.2, 01.3
// @implements AC-01, AC-03, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { UpdateChatSessionInputSchema } from "@/lib/validations/harness-daemon";

export const GET = guardedRoute(
  async (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const sessionId = ctx.params?.sessionId || "";
    return forwardToDaemonFlow({
      method: "GET",
      daemonPath: `/api/v1/projects/${projectId}/sessions/${sessionId}`,
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

export const PUT = guardedRoute(
  async (req, ctx, { body }) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const sessionId = ctx.params?.sessionId || "";
    return forwardToDaemonFlow({
      method: "PUT",
      daemonPath: `/api/v1/projects/${projectId}/sessions/${sessionId}`,
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
  { authMode: "strict-loopback", body: UpdateChatSessionInputSchema }
);

export const DELETE = guardedRoute(
  async (req, ctx) => {
    const projectId = ctx.projectId || ctx.params?.id || "";
    const sessionId = ctx.params?.sessionId || "";
    return forwardToDaemonFlow({
      method: "DELETE",
      daemonPath: `/api/v1/projects/${projectId}/sessions/${sessionId}`,
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
