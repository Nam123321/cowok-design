// @story 11.1
// @implements AC-02, AC-04 (BFF Proxy for GET /api/v1/bot-fleet/providers/status)
import { guardedRoute } from "@/lib/api-guard";
import {
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import { BotFleetProvidersStatusResponseSchema } from "@/lib/validations/bot-fleet";

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    return proxyAndValidateSsotDaemon({
      method: "GET",
      daemonPath: "/api/v1/bot-fleet/providers/status",
      req,
      ctx,
      requestId,
      successSchema: BotFleetProvidersStatusResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);
