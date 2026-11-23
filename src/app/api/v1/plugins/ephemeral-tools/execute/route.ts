// @story 08.5
// @implements AC-02, AC-05 (POST /api/v1/plugins/ephemeral-tools/execute STRICT HUMAN_ONLY BFF proxy)
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type DeRuntimeErrorCode,
  DeRuntimeErrorEnvelopeSchema,
  ExecuteEphemeralToolRequestSchema,
  ExecuteEphemeralToolResponseSchema,
} from "@/lib/validations/de-runtime";

const MAX_JAIL_BODY_BYTES = 131_072;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildHumanOnlyError(requestId: string): Response {
  return new Response(
    JSON.stringify({
      error: {
        code: "HUMAN_APPROVAL_REQUIRED",
        message: "Ephemeral tool execution strictly requires HumanWorkbenchSession (HUMAN_ONLY)",
        requestId,
      },
    }),
    { status: HTTP_FORBIDDEN, headers: JSON_HEADERS }
  );
}

function mapFlatJailAuthErr(status: number, body: unknown, requestId: string): Response | null {
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
  return new Response(JSON.stringify({ error: { code, message: msg, requestId } }), {
    status,
    headers: JSON_HEADERS,
  });
}

async function validateJailDaemonResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = ExecuteEphemeralToolResponseSchema.safeParse(parsed);
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
      ? mapFlatJailAuthErr(daemonResp.status, parsed, requestId)
      : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed contract error
  }
  return new Response(
    JSON.stringify({
      error: {
        code: "INTERNAL_ERROR",
        message: "Daemon ephemeral tool jail contract violation",
        requestId,
      },
    }),
    { status: HTTP_SERVICE_UNAVAILABLE, headers: JSON_HEADERS }
  );
}

async function forwardEphemeralExecute(
  req: Request,
  ctx: GuardedContext,
  body: unknown,
  requestId: string
): Promise<Response> {
  const daemonResp = await forwardToDaemonFlow({
    method: "POST",
    daemonPath: "/api/v1/plugins/ephemeral-tools/execute",
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
  return validateJailDaemonResp(daemonResp, requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    if (!ctx.isHumanWorkbench) return buildHumanOnlyError(requestId);
    const parsed = await parseJsonBodyStrict(
      req,
      ExecuteEphemeralToolRequestSchema,
      requestId,
      MAX_JAIL_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    return forwardEphemeralExecute(req, ctx, parsed.data, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    requireHumanAction: true,
    rawBinary: true,
    body: ExecuteEphemeralToolRequestSchema,
  }
);
