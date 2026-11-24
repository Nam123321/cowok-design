// @story 08.7
// @implements AC-01 (BFF Proxy for GET /api/v1/skills/catalog)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  AiHarnessErrorEnvelopeSchema,
  SkillCatalogEnvelopeSchema,
} from "@/lib/validations/ai-harness";

const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };
const HTTP_SERVICE_UNAVAILABLE = 503;

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const params = new URLSearchParams();
    const q = url.searchParams.get("q")?.trim();
    const projectId = url.searchParams.get("projectId")?.trim();
    if (q) params.set("q", q);
    if (projectId) params.set("projectId", projectId);

    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/skills/catalog",
      rawQuery: params.toString() || undefined,
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
      const ok = SkillCatalogEnvelopeSchema.safeParse(parsed);
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
          message: "Daemon /skills/catalog contract violation",
          requestId,
        },
      }),
      { status: HTTP_SERVICE_UNAVAILABLE, headers: JSON_HEADERS }
    );
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
