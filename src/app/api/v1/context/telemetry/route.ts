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

const HTTP_UNPROCESSABLE = 422;

function validateTelemetrySearchParams(
  url: URL,
  reqId: string
): { ok: true; rawQuery: string } | { ok: false; response: Response } {
  const limitRaw = url.searchParams.get("limit");
  if (limitRaw !== null && !/^\d+$/.test(limitRaw)) {
    return {
      ok: false,
      response: buildErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "contextMeter.errors.invalidRequest",
        requestId: reqId,
        field: "limit",
      }),
    };
  }
  const rawParams = Object.fromEntries(url.searchParams.entries());
  const parsed = ContextTelemetryQuerySchema.safeParse(rawParams);
  if (!parsed.success) {
    return {
      ok: false,
      response: buildErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "contextMeter.errors.invalidRequest",
        requestId: reqId,
        field: firstZodField(parsed.error),
      }),
    };
  }
  return { ok: true, rawQuery: url.search.slice(1) };
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const reqId =
      ctx.correlationId && isUuidV4(ctx.correlationId)
        ? ctx.correlationId
        : globalThis.crypto.randomUUID();
    const url = new URL(req.url, "http://127.0.0.1");
    const checked = validateTelemetrySearchParams(url, reqId);
    if (!checked.ok) return checked.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "GET",
      daemonPath: "/api/v1/context/telemetry",
      rawQuery: checked.rawQuery,
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
