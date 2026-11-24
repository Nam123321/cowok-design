// @story 10.1
// @implements AC-04 (BFF Proxy for GET /api/v1/orchestrator/assets — Unified Asset Library Provenance Backlinks)
import { guardedRoute } from "@/lib/api-guard";
import {
  firstSsotZodField,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  type OrchestratorAssetsQuery,
  OrchestratorAssetsQuerySchema,
  OrchestratorAssetsResponseSchema,
} from "@/lib/validations/orchestrator";

const HTTP_UNPROCESSABLE = 422;

function buildCanonicalAssetsQuery(data: OrchestratorAssetsQuery): string {
  const canonical = new URLSearchParams({
    projectId: data.projectId,
  });
  if (data.sessionId) {
    canonical.set("sessionId", data.sessionId);
  }
  return canonical.toString();
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const parsedQuery = OrchestratorAssetsQuerySchema.safeParse(
      Object.fromEntries(url.searchParams.entries())
    );
    if (!parsedQuery.success) {
      return new Response(
        JSON.stringify({
          error: {
            code: "INVALID_REQUEST",
            message: parsedQuery.error.issues[0]?.message ?? "Invalid assets query parameters",
            field: firstSsotZodField(parsedQuery.error) ?? "query",
            requestId,
          },
        }),
        {
          status: HTTP_UNPROCESSABLE,
          headers: { "Content-Type": "application/json", "X-Correlation-Id": requestId },
        }
      );
    }

    return proxyAndValidateSsotDaemon({
      method: "GET",
      daemonPath: "/api/v1/orchestrator/assets",
      rawQuery: buildCanonicalAssetsQuery(parsedQuery.data),
      req,
      ctx,
      requestId,
      successSchema: OrchestratorAssetsResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);
