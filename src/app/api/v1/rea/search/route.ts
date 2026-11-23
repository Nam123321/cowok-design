// @story 09.1
// @implements AC-01 (GET /api/v1/rea/search BFF proxy -> Two-Store Tier-1A / Tier-1B FTS5 search)
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type ReaErrorCode,
  ReaErrorEnvelopeSchema,
  ReaSearchQuerySchema,
  ReaSearchResponseSchema,
} from "@/lib/validations/rea";

const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_UNPROCESSABLE = 422;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildSearchError(
  status: number,
  code: ReaErrorCode,
  message: string,
  requestId: string
): Response {
  return new Response(
    JSON.stringify({ error: { code, message, requestId } }),
    { status, headers: JSON_HEADERS }
  );
}

function mapFlatSearchAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (status !== HTTP_UNAUTHORIZED && status !== HTTP_FORBIDDEN) return null;
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  const code: ReaErrorCode = status === HTTP_UNAUTHORIZED ? "UNAUTHORIZED" : "AUTH_FORBIDDEN";
  return buildSearchError(status, code, msg, requestId);
}

async function validateSearchDaemonResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = ReaSearchResponseSchema.safeParse(parsed);
    if (daemonResp.ok && okData.success) {
      return new Response(JSON.stringify(okData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const errData = ReaErrorEnvelopeSchema.safeParse(parsed);
    if (!daemonResp.ok && errData.success) {
      return new Response(JSON.stringify(errData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const flatErr = !daemonResp.ok
      ? mapFlatSearchAuthErr(daemonResp.status, parsed, requestId)
      : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed error envelope
  }
  return buildSearchError(
    HTTP_SERVICE_UNAVAILABLE,
    "INTERNAL_ERROR",
    "Daemon /rea/search contract violation",
    requestId
  );
}

async function forwardReaSearch(
  req: Request,
  ctx: GuardedContext,
  rawQuery: string,
  requestId: string
): Promise<Response> {
  const daemonResp = await forwardToDaemonFlow({
    method: "GET",
    daemonPath: "/api/v1/rea/search",
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
  return validateSearchDaemonResp(daemonResp, requestId);
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const parsed = ReaSearchQuerySchema.safeParse({
      q: url.searchParams.get("q") ?? "",
      limit: url.searchParams.get("limit") ?? undefined,
      tier: url.searchParams.get("tier") ?? undefined,
    });
    if (!parsed.success) {
      return buildSearchError(
        HTTP_UNPROCESSABLE,
        "INVALID_REQUEST",
        "Invalid /rea/search query parameters",
        requestId
      );
    }
    const qs = new URLSearchParams({
      q: parsed.data.q,
      limit: String(parsed.data.limit),
      tier: parsed.data.tier,
    }).toString();
    return forwardReaSearch(req, ctx, qs, requestId);
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);
