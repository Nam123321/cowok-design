// @story 04.3
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { VaultConfigInputSchema } from "@/lib/validations/workbench-settings";

export const PUT = guardedRoute(
  async (req, ctx, { body }) =>
    forwardToDaemonFlow({
      method: "PUT",
      daemonPath: "/api/v1/settings/vault",
      body,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    }),
  { body: VaultConfigInputSchema, authMode: "strict-loopback" }
);
