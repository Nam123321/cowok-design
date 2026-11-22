// @story 04.2
// @implements AC-04
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { FsNativePickInputSchema } from "@/lib/validations/harness-daemon";

export const POST = guardedRoute(
  async (req, ctx, { body }) => {
    return forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/fs/pick-native",
      body: body ?? {},
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: ctx.correlationId,
    });
  },
  { authMode: "strict-loopback", roles: ["editor", "admin"], body: FsNativePickInputSchema }
);
