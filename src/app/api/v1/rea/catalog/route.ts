// @story 09.1
// @implements AC-01, AC-02 (GET /api/v1/rea/catalog BFF proxy for Real Awesome-RE Visual App FTS5 Top-5 Search)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { firstSsotZodField, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  ReaCatalogQuerySchema,
  ReaCatalogResponseSchema,
  ReaErrorEnvelopeSchema,
} from "@/lib/validations/rea";

const HTTP_UNPROCESSABLE = 422;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), { status, headers: JSON_HEADERS });
}

function mapFlatCatalogAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (status !== HTTP_UNAUTHORIZED && status !== HTTP_FORBIDDEN) return null;
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const message = typeof obj.message === "string" ? obj.message : "Unauthorized catalog request";
  const code = status === HTTP_UNAUTHORIZED ? "UNAUTHORIZED" : "AUTH_FORBIDDEN";
  return jsonResponse({ error: { code, message, requestId } }, status);
}

async function validateCatalogDaemonResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = ReaCatalogResponseSchema.safeParse(parsed);
    if (daemonResp.ok && okData.success) return jsonResponse(okData.data, daemonResp.status);
    const errData = ReaErrorEnvelopeSchema.safeParse(parsed);
    if (!daemonResp.ok && errData.success) return jsonResponse(errData.data, daemonResp.status);
    const flatErr = !daemonResp.ok ? mapFlatCatalogAuthErr(daemonResp.status, parsed, requestId) : null;
    if (flatErr) return flatErr;
  } catch {
    // Fail closed on non-JSON or contract mismatch
  }
  return jsonResponse(
    { error: { code: "INTERNAL_ERROR", message: "Daemon /rea/catalog contract violation", requestId } },
    HTTP_SERVICE_UNAVAILABLE
  );
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const url = new URL(req.url, "http://127.0.0.1");
    const parsedQuery = ReaCatalogQuerySchema.safeParse(Object.fromEntries(url.searchParams.entries()));
    if (!parsedQuery.success) {
      return jsonResponse(
        {
          error: {
            code: "INVALID_REQUEST",
            message: parsedQuery.error.issues[0]?.message || "Invalid /rea/catalog query",
            field: firstSsotZodField(parsedQuery.error),
            requestId,
          },
        },
        HTTP_UNPROCESSABLE
      );
    }
    const qs = new URLSearchParams({
      q: parsedQuery.data.q,
      limit: String(parsedQuery.data.limit),
    }).toString();
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/rea/catalog",
      rawQuery: qs,
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
    return validateCatalogDaemonResp(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
  }
);
