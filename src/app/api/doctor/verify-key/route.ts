// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { VerifyProviderKeyInputSchema } from "@/lib/validations/workbench-settings";

export const POST = guardedRoute(
  async (req, ctx, { body }) =>
    forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/doctor/verify-key",
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    }),
  { body: VerifyProviderKeyInputSchema, authMode: "strict-loopback" }
);
