// @story 09.1
// @implements AC-02, AC-03, AC-04 (POST /api/v1/rea/ingest BFF proxy with Firewall & SSRF/Zip-Bomb guards)
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type ReaErrorCode,
  ReaErrorEnvelopeSchema,
  ReaIngestRequestSchema,
  ReaIngestResponseSchema,
} from "@/lib/validations/rea";

const MAX_INGEST_BODY_BYTES = 2_097_152;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildReaError(status: number, code: ReaErrorCode, message: string, requestId: string): Response {
  return new Response(
    JSON.stringify({ error: { code, message, requestId } }),
    { status, headers: JSON_HEADERS }
  );
}

function mapFlatReaAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  let code: ReaErrorCode = "INVALID_REQUEST";
  if (status === HTTP_UNAUTHORIZED) code = "UNAUTHORIZED";
  else if (status === HTTP_FORBIDDEN) code = "AUTH_FORBIDDEN";
  else if (status === HTTP_PAYLOAD_TOO_LARGE) code = "PAYLOAD_TOO_LARGE";
  return buildReaError(status, code, msg, requestId);
}

async function validateIngestDaemonResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = ReaIngestResponseSchema.safeParse(parsed);
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
      ? mapFlatReaAuthErr(daemonResp.status, parsed, requestId)
      : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed contract error
  }
  return buildReaError(
    HTTP_SERVICE_UNAVAILABLE,
    "INTERNAL_ERROR",
    "Daemon /rea/ingest contract violation",
    requestId
  );
}

async function forwardReaIngest(
  req: Request,
  ctx: GuardedContext,
  body: unknown,
  requestId: string
): Promise<Response> {
  const daemonResp = await forwardToDaemonFlow({
    method: "POST",
    daemonPath: "/api/v1/rea/ingest",
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
  return validateIngestDaemonResp(daemonResp, requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      ReaIngestRequestSchema,
      requestId,
      MAX_INGEST_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    return forwardReaIngest(req, ctx, parsed.data, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: ReaIngestRequestSchema,
  }
);
