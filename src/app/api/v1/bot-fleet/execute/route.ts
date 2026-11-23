// @story 11.1
// @implements AC-01, AC-02, AC-03 (BFF Proxy for POST /api/v1/bot-fleet/execute)
import { guardedRoute } from "@/lib/api-guard";
import {
  parseJsonBodyStrict,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  BotFleetExecuteRequestSchema,
  BotFleetExecuteResponseSchema,
} from "@/lib/validations/bot-fleet";

const MAX_EXECUTE_BODY_BYTES = 262_144;

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      BotFleetExecuteRequestSchema,
      requestId,
      MAX_EXECUTE_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;

    return proxyAndValidateSsotDaemon({
      method: "POST",
      daemonPath: "/api/v1/bot-fleet/execute",
      body: parsed.data,
      req,
      ctx,
      requestId,
      successSchema: BotFleetExecuteResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: BotFleetExecuteRequestSchema,
  }
);
