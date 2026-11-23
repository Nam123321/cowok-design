// @story 09.2
// @implements AC-04 (BFF Proxy for GET /api/v1/decision/audit)
import { guardedRoute } from "@/lib/api-guard";
import {
  firstSsotZodField,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  type DecisionAuditQuery,
  DecisionAuditQuerySchema,
  DecisionAuditResponseSchema,
} from "@/lib/validations/decision";

const HTTP_UNPROCESSABLE = 422;

function buildCanonicalAuditQuery(data: DecisionAuditQuery): string {
  const canonical = new URLSearchParams({
    projectId: data.projectId,
    limit: String(data.limit),
  });
  if (data.stage) {
    canonical.set("stage", data.stage);
  }
  return canonical.toString();
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const parsedQuery = DecisionAuditQuerySchema.safeParse(
      Object.fromEntries(url.searchParams.entries())
    );
    if (!parsedQuery.success) {
      return new Response(
        JSON.stringify({
          error: {
            code: "INVALID_REQUEST",
            message: parsedQuery.error.issues[0]?.message ?? "Invalid audit query parameters",
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
      daemonPath: "/api/v1/decision/audit",
      rawQuery: buildCanonicalAuditQuery(parsedQuery.data),
      req,
      ctx,
      requestId,
      successSchema: DecisionAuditResponseSchema,
    });
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);
