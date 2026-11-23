// @story 09.1
// @implements AC-04 (POST /api/v1/rea/export-proof BFF proxy -> Honest Figma Proof Exporter)
import { guardedRoute, type GuardedContext } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  type ReaErrorCode,
  ReaErrorEnvelopeSchema,
  ReaExportProofRequestSchema,
  ReaExportProofResponseSchema,
} from "@/lib/validations/rea";

const MAX_EXPORT_BODY_BYTES = 32_768;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_SERVICE_UNAVAILABLE = 503;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function buildProofError(
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

function mapFlatProofAuthErr(status: number, body: unknown, requestId: string): Response | null {
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
  return buildProofError(status, code, msg, requestId);
}

async function validateProofDaemonResp(daemonResp: Response, requestId: string): Promise<Response> {
  try {
    const parsed: unknown = JSON.parse(await daemonResp.text());
    const okData = ReaExportProofResponseSchema.safeParse(parsed);
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
      ? mapFlatProofAuthErr(daemonResp.status, parsed, requestId)
      : null;
    if (flatErr) return flatErr;
  } catch {
    // Fall through to fail-closed contract error
  }
  return buildProofError(
    HTTP_SERVICE_UNAVAILABLE,
    "INTERNAL_ERROR",
    "Daemon /rea/export-proof contract violation",
    requestId
  );
}

async function forwardExportProof(
  req: Request,
  ctx: GuardedContext,
  body: unknown,
  requestId: string
): Promise<Response> {
  const daemonResp = await forwardToDaemonFlow({
    method: "POST",
    daemonPath: "/api/v1/rea/export-proof",
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
  return validateProofDaemonResp(daemonResp, requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      ReaExportProofRequestSchema,
      requestId,
      MAX_EXPORT_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    return forwardExportProof(req, ctx, parsed.data, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: ReaExportProofRequestSchema,
  }
);
