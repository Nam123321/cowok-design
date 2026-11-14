// @story 04.4
// @implements AC-01, AC-02, AC-04 (Shared BFF Forwarder for Flow Job & Session Routes)
import { type ZodSchema } from "zod";
import { guardedRoute } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";

export function makeFlowProjectPostRoute<T>(suffix: string, schema: ZodSchema<T>) {
  return guardedRoute(
    async (req, ctx, { body }) => {
      const projectId = ctx.projectId || ctx.params?.id || "";
      return forwardToDaemonFlow({
        method: "POST",
        daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/${suffix}`,
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
    { authMode: "strict-loopback", body: schema }
  );
}

export function makeFlowProjectGetRoute(suffix: string) {
  return guardedRoute(
    async (req, ctx) => {
      const projectId = ctx.projectId || ctx.params?.id || "";
      return forwardToDaemonFlow({
        method: "GET",
        daemonPath: `/api/v1/projects/${encodeURIComponent(projectId)}/${suffix}`,
        inboundHeaders: req.headers,
        rawQuery: req.url.includes("?") ? req.url.slice(req.url.indexOf("?") + 1) : "",
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
}
