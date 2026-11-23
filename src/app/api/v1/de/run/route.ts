// @story 08.5
// @implements AC-01 (POST /api/v1/de/run BFF proxy for 7 Built-in DE Tools)
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type DeRuntimeErrorCode,
  DeRuntimeErrorEnvelopeSchema,
  RunBuiltInDeRequestSchema,
  RunBuiltInDeResponseSchema,
} from "@/lib/validations/de-runtime";

const MAX_DE_BODY_BYTES = 589_824;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildDeError(params: {
  status: number;
  code: DeRuntimeErrorCode;
  message: string;
  requestId: string;
}): Response {
  return new Response(
    JSON.stringify({
      error: { code: params.code, message: params.message, requestId: params.requestId },
    }),
    { status: params.status, headers: JSON_HEADERS }
  );
}

function mapFlatDaemonAuthErr(status: number, body: unknown, requestId: string): Response | null {
  if (
    status !== HTTP_UNAUTHORIZED &&
    status !== HTTP_FORBIDDEN &&
    status !== HTTP_PAYLOAD_TOO_LARGE
  ) {
    return null;
  }
  const obj = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {};
  const msg = typeof obj.message === "string" ? obj.message : "Request rejected by daemon guard";
  const rawCode = typeof obj.code === "string" ? obj.code : "";
  let code: DeRuntimeErrorCode = "INVALID_REQUEST";
  if (rawCode === "HUMAN_APPROVAL_REQUIRED") code = "HUMAN_APPROVAL_REQUIRED";
  else if (status === HTTP_UNAUTHORIZED) code = "UNAUTHORIZED";
  else if (status === HTTP_FORBIDDEN) code = "AUTH_FORBIDDEN";
  else if (status === HTTP_PAYLOAD_TOO_LARGE) code = "PAYLOAD_TOO_LARGE";
  return buildDeError({ status, code, message: msg, requestId });
}

async function validateDeRunDaemonResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = RunBuiltInDeResponseSchema.safeParse(parsed);
    if (daemonResp.ok && okData.success) {
      return new Response(JSON.stringify(okData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const errData = DeRuntimeErrorEnvelopeSchema.safeParse(parsed);
    if (!daemonResp.ok && errData.success) {
      return new Response(JSON.stringify(errData.data), {
        status: daemonResp.status,
        headers: JSON_HEADERS,
      });
    }
    const flatErr = !daemonResp.ok
      ? mapFlatDaemonAuthErr(daemonResp.status, parsed, requestId)
      : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed error envelope
  }
  return buildDeError({
    status: HTTP_SERVICE_UNAVAILABLE,
    code: "INTERNAL_ERROR",
    message: "Daemon DE runtime contract violation",
    requestId,
  });
}

async function forwardDeRun(
  req: Request,
  ctx: GuardedContext,
  body: unknown,
  requestId: string
): Promise<Response> {
  const daemonResp = await forwardToDaemonFlow({
    method: "POST",
    daemonPath: "/api/v1/de/run",
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
  return validateDeRunDaemonResp(daemonResp, requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      RunBuiltInDeRequestSchema,
      requestId,
      MAX_DE_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    return forwardDeRun(req, ctx, parsed.data, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["viewer", "editor", "admin"],
    readOnlyPost: true,
    rawBinary: true,
    body: RunBuiltInDeRequestSchema,
  }
);
