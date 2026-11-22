// @story 02.4
// @implements AC-01, AC-02
import { guardedRoute } from "@/lib/api-guard";
import {
  parseJsonBodyStrict,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  SsotExportRequestSchema,
  SsotExportResponseSchema,
} from "@/lib/validations/ssot-registry";

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(req, SsotExportRequestSchema, requestId);
    if (!parsed.ok) return parsed.response;

    return proxyAndValidateSsotDaemon({
      method: "POST",
      daemonPath: "/api/v1/ssot/export",
      body: parsed.data,
      req,
      ctx,
      requestId,
      successSchema: SsotExportResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    rawBinary: true,
    body: SsotExportRequestSchema,
    roles: ["editor", "admin"],
  }
);
