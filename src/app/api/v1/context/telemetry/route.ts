// @story 02.1
// @implements AC-02, AC-04
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  ContextTelemetryPageSchema,
  ContextTelemetryQuerySchema,
} from "@/lib/validations/context";
import { isUuidV4 } from "@/lib/validations/context-errors";
import {
  buildErrorResponse,
  firstZodField,
  validateDaemonJsonResponse,
} from "../assemble/route";

const HTTP_FORBIDDEN = 403;
const HTTP_UNPROCESSABLE = 422;

function findDuplicateParam(params: URLSearchParams): string | null {
  const seen = new Set<string>();
  for (const key of params.keys()) {
    if (seen.has(key)) return key;
    seen.add(key);
  }
  return null;
}

function makeInvalidTelemetryParam(reqId: string, field?: string): { ok: false; response: Response } {
  return {
    ok: false,
    response: buildErrorResponse({
      status: HTTP_UNPROCESSABLE,
      code: "INVALID_REQUEST",
      message: "contextMeter.errors.invalidRequest",
      requestId: reqId,
      field,
    }),
  };
}

function validateTelemetrySearchParams(
  url: URL,
  reqId: string
): { ok: true; rawQuery: string } | { ok: false; response: Response } {
  const dupKey = findDuplicateParam(url.searchParams);
  if (dupKey) return makeInvalidTelemetryParam(reqId, dupKey);
  const limitRaw = url.searchParams.get("limit");
  if (limitRaw !== null && !/^\d+$/.test(limitRaw)) {
    return makeInvalidTelemetryParam(reqId, "limit");
  }
  const parsed = ContextTelemetryQuerySchema.safeParse(Object.fromEntries(url.searchParams.entries()));
  if (!parsed.success) return makeInvalidTelemetryParam(reqId, firstZodField(parsed.error));
  const canonical = new URLSearchParams({
    projectId: parsed.data.projectId,
    limit: String(parsed.data.limit),
  });
  if (parsed.data.portalId) canonical.set("portalId", parsed.data.portalId);
  if (parsed.data.horizon) canonical.set("horizon", parsed.data.horizon);
  if (parsed.data.modelProfile) canonical.set("modelProfile", parsed.data.modelProfile);
  if (parsed.data.cursor) canonical.set("cursor", parsed.data.cursor);
  return { ok: true, rawQuery: canonical.toString() };
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const reqId =
      ctx.correlationId && isUuidV4(ctx.correlationId)
        ? ctx.correlationId
        : globalThis.crypto.randomUUID();
    const rawPortalHeader = req.headers.get("x-cowok-portal");
    if (rawPortalHeader !== null && rawPortalHeader.trim().length === 0) {
      return buildErrorResponse({
        status: HTTP_FORBIDDEN,
        code: "CROSS_PORTAL_LEAK_BLOCKED",
        message: "contextMeter.errors.crossPortalLeak",
        requestId: reqId,
        field: "portalId",
      });
    }
    const url = new URL(req.url, "http://127.0.0.1");
    const checked = validateTelemetrySearchParams(url, reqId);
    if (!checked.ok) return checked.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/context/telemetry",
      rawQuery: checked.rawQuery,
      ...(rawPortalHeader !== null ? { extraHeaders: { "X-Cowok-Portal": rawPortalHeader.trim() } } : {}),
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "viewer",
      },
      correlationId: reqId,
      bffSessionToken: ctx.bffSessionToken,
      isHumanWorkbench: ctx.isHumanWorkbench,
      humanSessionToken: ctx.humanSessionToken,
      signal: req.signal,
    });
    return validateDaemonJsonResponse(daemonResp, ContextTelemetryPageSchema, reqId);
  },
  { authMode: "strict-loopback" }
);
