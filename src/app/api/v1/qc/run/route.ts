// @story 07.1
// @implements AC-01, AC-02, AC-03 (Guarded BFF Proxy for POST /api/v1/qc/run)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import { parseJsonBodyStrict, resolveRequestCorrelationId } from "@/lib/ssot-bff-proxy";
import {
  QcRunRequestSchema,
  QcRunResponseSchema,
  VisualQcErrorResponseSchema,
} from "@/lib/validations/visual-qc";

const MAX_BODY_BYTES = 1_048_576;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

async function validateQcRunResponse(
  daemonResp: Response,
  requestId: string
): Promise<Response> {
  let body: unknown;
  try {
    body = JSON.parse(await daemonResp.text());
  } catch {
    return new Response(
      JSON.stringify({
        error: { code: "INTERNAL_ERROR", message: "Non-JSON daemon response", requestId },
      }),
      { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
    );
  }
  if (daemonResp.ok) {
    const parsed = QcRunResponseSchema.safeParse(body);
    if (!parsed.success) {
      return new Response(
        JSON.stringify({
          error: { code: "INTERNAL_ERROR", message: "Visual QC run response mismatch", requestId },
        }),
        { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
      );
    }
    return new Response(JSON.stringify(parsed.data), {
      status: daemonResp.status,
      headers: JSON_HEADERS,
    });
  }
  const errParsed = VisualQcErrorResponseSchema.safeParse(body);
  return new Response(
    JSON.stringify(
      errParsed.success
        ? errParsed.data
        : {
            error: {
              code: "INTERNAL_ERROR",
              message: "Visual QC run upstream error",
              requestId,
            },
          }
    ),
    { status: daemonResp.status, headers: JSON_HEADERS }
  );
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseJsonBodyStrict(
      req,
      QcRunRequestSchema,
      requestId,
      MAX_BODY_BYTES
    );
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/qc/run",
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
    return validateQcRunResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: QcRunRequestSchema,
  }
);
