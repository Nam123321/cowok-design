// @story 02.4
// @implements AC-01, AC-02
import { guardedRoute } from "@/lib/api-guard";
import {
  buildSsotErrorResponse,
  firstSsotZodField,
  proxyAndValidateSsotDaemon,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  SsotComponentsQuerySchema,
  SsotComponentsResponseSchema,
} from "@/lib/validations/ssot-registry";

const HTTP_UNPROCESSABLE = 422;

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const limitRaw = url.searchParams.get("limit");
    if (limitRaw !== null && !/^\d+$/.test(limitRaw)) {
      return buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "limit must be a positive integer",
        requestId,
        field: "limit",
      });
    }

    const parsed = SsotComponentsQuerySchema.safeParse(Object.fromEntries(url.searchParams.entries()));
    if (!parsed.success) {
      return buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: parsed.error.issues[0]?.message || "Invalid query parameters",
        requestId,
        field: firstSsotZodField(parsed.error),
      });
    }

    return proxyAndValidateSsotDaemon({
      method: "GET",
      daemonPath: "/api/v1/ssot/components",
      rawQuery: url.search.slice(1),
      req,
      ctx,
      requestId,
      successSchema: SsotComponentsResponseSchema,
    });
  },
  { authMode: "strict-loopback" }
);
