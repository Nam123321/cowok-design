// @story 08.7
// @implements AC-03 (BFF Proxy for GET /api/v1/models/registry)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  AiHarnessErrorEnvelopeSchema,
  ModelsRegistryEnvelopeSchema,
} from "@/lib/validations/ai-harness";

const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };
const HTTP_SERVICE_UNAVAILABLE = 503;

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const providerId = url.searchParams.get("providerId")?.trim();
    const rawQuery = providerId ? new URLSearchParams({ providerId }).toString() : undefined;

    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/models/registry",
      rawQuery,
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: requestId,
      bffSessionToken: ctx.bffSessionToken,
      isHumanWorkbench: ctx.isHumanWorkbench,
      humanSessionToken: ctx.humanSessionToken,
      signal: req.signal,
    });

    try {
      const parsed: unknown = JSON.parse(await daemonResp.text());
      const ok = ModelsRegistryEnvelopeSchema.safeParse(parsed);
      if (daemonResp.ok && ok.success) {
        return new Response(JSON.stringify(ok.data), {
          status: daemonResp.status,
          headers: JSON_HEADERS,
        });
      }
      const err = AiHarnessErrorEnvelopeSchema.safeParse(parsed);
      if (!daemonResp.ok && err.success) {
        return new Response(JSON.stringify(err.data), {
          status: daemonResp.status,
          headers: JSON_HEADERS,
        });
      }
    } catch {
      // Fail closed below
    }
    return new Response(
      JSON.stringify({
        error: {
          code: "INTERNAL_ERROR",
          message: "Daemon /models/registry contract violation",
          requestId,
        },
      }),
      { status: HTTP_SERVICE_UNAVAILABLE, headers: JSON_HEADERS }
    );
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
