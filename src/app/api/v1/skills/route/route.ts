// @story 02.4
// @implements AC-03, AC-05
import { guardedRoute } from "@/lib/api-guard";
import {
  parseJsonBodyStrict,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  SkillRouteRequestSchema,
  SkillRouteResponseSchema,
} from "@/lib/validations/ssot-registry";

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, SkillRouteRequestSchema, requestId);
    if (!parsed.ok) return parsed.response;

    return proxyAndValidateSsotDaemon({
      method: "POST",
      daemonPath: "/api/v1/skills/route",
      body: parsed.data,
      req,
      ctx,
      requestId,
      successSchema: SkillRouteResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    rawBinary: true,
    body: SkillRouteRequestSchema,
    readOnlyPost: true,
  }
);
