// @story 14.1
// @implements AC-03, AC-04 (BFF Proxy for POST /api/v1/telemetry/events)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type SupportErrorCode,
  SupportErrorEnvelopeSchema,
  TelemetryEventRequestSchema,
  TelemetryEventResponseSchema,
} from "@/lib/validations/support";

const MAX_TELEMETRY_EVENT_BYTES = 65_536;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildTelemetryError(
  status: number,
  code: SupportErrorCode,
  message: string,
  requestId: string
): Response {
  return new Response(
    JSON.stringify({ error: { code, message, requestId } }),
    { status, headers: JSON_HEADERS }
  );
}

function mapFlatAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  let code: SupportErrorCode = "INVALID_REQUEST";
  if (status === HTTP_UNAUTHORIZED) code = "UNAUTHORIZED";
  else if (status === HTTP_FORBIDDEN) code = "AUTH_FORBIDDEN";
  else if (status === HTTP_PAYLOAD_TOO_LARGE) code = "PAYLOAD_TOO_LARGE";
  return buildTelemetryError(status, code, msg, requestId);
}

async function validateTelemetryEventResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = TelemetryEventResponseSchema.safeParse(parsed);
    if (daemonResp.ok && okData.success) {
      return new Response(JSON.stringify(okData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const errData = SupportErrorEnvelopeSchema.safeParse(parsed);
    if (!daemonResp.ok && errData.success) {
      return new Response(JSON.stringify(errData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const flatErr = !daemonResp.ok ? mapFlatAuthErr(daemonResp.status, parsed, requestId) : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed contract error
  }
  return buildTelemetryError(
    HTTP_SERVICE_UNAVAILABLE,
    "INTERNAL_ERROR",
    "Daemon /telemetry/events contract violation",
    requestId
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      TelemetryEventRequestSchema,
      requestId,
      MAX_TELEMETRY_EVENT_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/telemetry/events",
      body: parsed.data,
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
    return validateTelemetryEventResp(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: TelemetryEventRequestSchema,
  }
);
