// @story 04.2, 01.3
// @implements AC-01, AC-03, AC-05
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { CreateProjectInputSchema } from "@/lib/validations/harness-daemon";

export const GET = guardedRoute(
  async (req, ctx) => {
    return forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/projects",
      rawQuery: new URL(req.url).search.slice(1),
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
  async (req, ctx, { body }) => {
    return forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/projects",
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
  { authMode: "strict-loopback", body: CreateProjectInputSchema }
);
