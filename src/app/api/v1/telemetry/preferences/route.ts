// @story 14.1
// @implements AC-04 (BFF Proxy for GET/PUT /api/v1/telemetry/preferences)
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type SupportErrorCode,
  SupportErrorEnvelopeSchema,
  TelemetryPreferencesResponseSchema,
  UpdateTelemetryPreferencesRequestSchema,
} from "@/lib/validations/support";

const MAX_TELEMETRY_PREFS_BYTES = 4_096;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildPrefsError(
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
  if (status !== HTTP_UNAUTHORIZED && status !== HTTP_FORBIDDEN) return null;
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  const code: SupportErrorCode = status === HTTP_UNAUTHORIZED ? "UNAUTHORIZED" : "AUTH_FORBIDDEN";
  return buildPrefsError(status, code, msg, requestId);
}

async function validatePrefsResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = TelemetryPreferencesResponseSchema.safeParse(parsed);
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
  return buildPrefsError(
    HTTP_SERVICE_UNAVAILABLE,
    "INTERNAL_ERROR",
    "Daemon /telemetry/preferences contract violation",
    requestId
  );
}

interface ForwardTelemetryPrefsOptions {
  method: "GET" | "PUT";
  req: Request;
  ctx: GuardedContext;
  requestId: string;
  body?: unknown;
}

async function forwardTelemetryPrefs(opts: ForwardTelemetryPrefsOptions): Promise<Response> {
  const { method, req, ctx, requestId, body } = opts;
  const daemonResp = await forwardToDaemonFlow({
    method,
    daemonPath: "/api/v1/telemetry/preferences",
    body,
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
  return validatePrefsResp(daemonResp, requestId);
}

export const GET = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    return forwardTelemetryPrefs({ method: "GET", req, ctx, requestId });
  },
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
);

export const PUT = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      UpdateTelemetryPreferencesRequestSchema,
      requestId,
      MAX_TELEMETRY_PREFS_BYTES
    );
    if (!parsed.ok) return parsed.response;
    return forwardTelemetryPrefs({ method: "PUT", req, ctx, requestId, body: parsed.data });
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: UpdateTelemetryPreferencesRequestSchema,
  }
);
