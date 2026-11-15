// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { UpsertMcpServerInputSchema } from "@/lib/validations/workbench-settings";

export const GET = guardedRoute(
  async (req, ctx) =>
    forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/mcp-servers",
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: ctx.correlationId,
    }),
  { authMode: "strict-loopback" }
);

export const POST = guardedRoute(
  async (req, ctx, { body }) =>
    forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/mcp-servers",
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    }),
  { body: UpsertMcpServerInputSchema, authMode: "strict-loopback" }
);
