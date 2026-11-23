// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (BFF Proxy for POST /api/v1/decision/route with 409 STALE_FILE_OCC & 0% Float Leak Guard)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  parseJsonBodyStrict,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  DecisionErrorCodeSchema,
  DecisionErrorResponseSchema,
  DecisionRouteRequestSchema,
  DecisionRouteResponseSchema,
} from "@/lib/validations/decision";

const MAX_DECISION_BODY_BYTES = 262_144;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function jsonErr(message: string, requestId: string): Response {
  return new Response(
    JSON.stringify({ error: { code: "INTERNAL_ERROR", message, requestId } }),
    { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
  );
}

function mapFlatDaemonAuthError(
  status: number,
  jsonBody: unknown,
  requestId: string
): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  if (typeof jsonBody !== "object" || jsonBody === null) return null;
  const rec = jsonBody as { code?: unknown; message?: unknown };
  if (typeof rec.code !== "string" || typeof rec.message !== "string") return null;
  const parsedCode = DecisionErrorCodeSchema.safeParse(rec.code);
  let fallbackCode: "UNAUTHORIZED" | "FORBIDDEN" | "PAYLOAD_TOO_LARGE" = "PAYLOAD_TOO_LARGE";
  if (status === HTTP_UNAUTHORIZED) {
    fallbackCode = "UNAUTHORIZED";
  } else if (status === HTTP_FORBIDDEN) {
    fallbackCode = "FORBIDDEN";
  }
  const code = parsedCode.success ? parsedCode.data : fallbackCode;
  return new Response(
    JSON.stringify({ error: { code, message: rec.message, requestId } }),
    { status, headers: JSON_HEADERS }
  );
}

async function mapDaemonDecisionResponse(daemonResp: Response, requestId: string): Promise<Response> {
  let jsonBody: unknown;
  try {
    jsonBody = JSON.parse(await daemonResp.text());
  } catch {
    return jsonErr("Daemon returned non-JSON decision payload", requestId);
  }
  if (daemonResp.ok) {
    const okCheck = DecisionRouteResponseSchema.safeParse(jsonBody);
    return okCheck.success
      ? new Response(JSON.stringify(okCheck.data), { status: daemonResp.status, headers: JSON_HEADERS })
      : jsonErr("VerdictShield response contract violation blocked by BFF", requestId);
  }
  const errCheck = DecisionErrorResponseSchema.safeParse(jsonBody);
  if (errCheck.success) {
    return new Response(JSON.stringify(errCheck.data), {
      status: daemonResp.status,
      headers: JSON_HEADERS,
    });
  }
  const flatAuthErr = mapFlatDaemonAuthError(daemonResp.status, jsonBody, requestId);
  if (flatAuthErr) return flatAuthErr;
  return jsonErr("Daemon decision error contract violation", requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      DecisionRouteRequestSchema,
      requestId,
      MAX_DECISION_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;

    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/decision/route",
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
    return mapDaemonDecisionResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: DecisionRouteRequestSchema,
  }
);
