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

function findDuplicateQueryKey(params: URLSearchParams): string | null {
  const seen = new Set<string>();
  for (const key of params.keys()) {
    if (seen.has(key)) return key;
    seen.add(key);
  }
  return null;
}

function buildCanonicalComponentsQuery(data: {
  projectId: string;
  portalId: string;
  cursor?: string;
  limit: number;
}): string {
  const canonical = new URLSearchParams({
    projectId: data.projectId,
    portalId: data.portalId,
    limit: String(data.limit),
  });
  if (data.cursor) canonical.set("cursor", data.cursor);
  return canonical.toString();
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const dupKey = findDuplicateQueryKey(url.searchParams);
    if (dupKey) {
      return buildSsotErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "Duplicate query parameter is not permitted",
        requestId,
        field: dupKey,
      });
    }
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
      rawQuery: buildCanonicalComponentsQuery(parsed.data),
      req,
      ctx,
      requestId,
      successSchema: SsotComponentsResponseSchema,
    });
  },
  { authMode: "strict-loopback" }
);
