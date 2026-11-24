// @story 10.1
// @implements AC-02, AC-03, AC-04 (BFF Proxy for POST /api/v1/orchestrator/live-patch with O_NOFOLLOW/512KB UTF-8/429 Contract Guard)
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  firstSsotZodField,
  readBoundedRequestBytes,
  resolveRequestCorrelationId,
} from "@/lib/ssot-bff-proxy";
import {
  MAX_LIVE_SPEC_BYTES,
  OrchestratorErrorCodeSchema,
  OrchestratorErrorResponseSchema,
  type OrchestratorLivePatchRequest,
  OrchestratorLivePatchRequestSchema,
  OrchestratorLivePatchResponseSchema,
  utf8ByteLength,
} from "@/lib/validations/orchestrator";

const MAX_ORCHESTRATOR_BODY_BYTES = 655_360;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_UNPROCESSABLE = 422;
const HTTP_TOO_MANY_REQUESTS = 429;
const HTTP_BAD_GATEWAY = 502;
const JSON_HEADERS = { "Content-Type": "application/json; charset=utf-8" };

function jsonErr(message: string, requestId: string): Response {
  return new Response(
    JSON.stringify({ error: { code: "INTERNAL_ERROR", message, requestId } }),
    { status: HTTP_BAD_GATEWAY, headers: JSON_HEADERS }
  );
}

function buildInputErr(params: {
  status: number;
  code: "INVALID_REQUEST" | "LIVE_SPEC_PAYLOAD_TOO_LARGE";
  message: string;
  field?: string;
  details?: Record<string, unknown>;
  requestId: string;
}): Response {
  return new Response(
    JSON.stringify({
      error: {
        code: params.code,
        message: params.message,
        ...(params.field ? { field: params.field } : {}),
        requestId: params.requestId,
        ...(params.details ? { details: params.details } : {}),
      },
    }),
    { status: params.status, headers: JSON_HEADERS }
  );
}

function checkLiveSpecPayloadCeiling(parsedJson: unknown, requestId: string): Response | null {
  if (typeof parsedJson !== "object" || parsedJson === null) return null;
  const rawPayload = (parsedJson as { liveSpecPayload?: unknown }).liveSpecPayload;
  if (typeof rawPayload !== "string") return null;
  const actualBytes = utf8ByteLength(rawPayload);
  if (actualBytes <= MAX_LIVE_SPEC_BYTES) return null;
  return buildInputErr({
    status: HTTP_PAYLOAD_TOO_LARGE,
    code: "LIVE_SPEC_PAYLOAD_TOO_LARGE",
    message: `Live spec payload ${actualBytes} bytes exceeds ${MAX_LIVE_SPEC_BYTES} byte ceiling`,
    field: "liveSpecPayload",
    details: { actualBytes, maxBytes: MAX_LIVE_SPEC_BYTES },
    requestId,
  });
}

async function decodeBoundedJsonBody(
  req: Request,
  requestId: string
): Promise<{ ok: true; value: unknown } | { ok: false; response: Response }> {
  const rawBuf = await readBoundedRequestBytes(req, MAX_ORCHESTRATOR_BODY_BYTES);
  if (rawBuf === "TOO_LARGE") {
    return {
      ok: false,
      response: buildInputErr({
        status: HTTP_PAYLOAD_TOO_LARGE,
        code: "LIVE_SPEC_PAYLOAD_TOO_LARGE",
        message: "Live patch request body exceeds ceiling",
        field: "body",
        requestId,
      }),
    };
  }
  if (rawBuf === "EMPTY") {
    return {
      ok: false,
      response: buildInputErr({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "Request body is empty",
        field: "body",
        requestId,
      }),
    };
  }
  try {
    return { ok: true, value: JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(rawBuf)) };
  } catch {
    return {
      ok: false,
      response: buildInputErr({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: "Malformed UTF-8 JSON body",
        field: "body",
        requestId,
      }),
    };
  }
}

async function parseLivePatchRequestBody(
  req: Request,
  requestId: string
): Promise<{ ok: true; data: OrchestratorLivePatchRequest } | { ok: false; response: Response }> {
  const decoded = await decodeBoundedJsonBody(req, requestId);
  if (!decoded.ok) return decoded;
  const ceilingErr = checkLiveSpecPayloadCeiling(decoded.value, requestId);
  if (ceilingErr) return { ok: false, response: ceilingErr };
  const checked = OrchestratorLivePatchRequestSchema.safeParse(decoded.value);
  if (!checked.success) {
    return {
      ok: false,
      response: buildInputErr({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_REQUEST",
        message: checked.error.issues[0]?.message || "Request validation failed",
        field: firstSsotZodField(checked.error),
        requestId,
      }),
    };
  }
  return { ok: true, data: checked.data };
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
  const parsedCode = OrchestratorErrorCodeSchema.safeParse(rec.code);
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

async function mapDaemonLivePatchResponse(
  daemonResp: Response,
  requestId: string
): Promise<Response> {
  let jsonBody: unknown;
  try {
    jsonBody = JSON.parse(await daemonResp.text());
  } catch {
    return jsonErr("Daemon returned non-JSON live-patch payload", requestId);
  }
  if (daemonResp.ok) {
    const okCheck = OrchestratorLivePatchResponseSchema.safeParse(jsonBody);
    return okCheck.success
      ? new Response(JSON.stringify(okCheck.data), {
          status: daemonResp.status,
          headers: JSON_HEADERS,
        })
      : jsonErr("Orchestrator live-patch response contract violation blocked by BFF", requestId);
  }
  const errCheck = OrchestratorErrorResponseSchema.safeParse(jsonBody);
  if (errCheck.success) {
    const headers = new Headers(JSON_HEADERS);
    const retryAfter = daemonResp.headers.get("retry-after");
    if (
      (daemonResp.status === HTTP_TOO_MANY_REQUESTS ||
        errCheck.data.error.code === "ACTOR_BACKPRESSURE") &&
      retryAfter
    ) {
      headers.set("Retry-After", retryAfter);
    }
    return new Response(JSON.stringify(errCheck.data), {
      status: daemonResp.status,
      headers,
    });
  }
  const flatAuthErr = mapFlatDaemonAuthError(daemonResp.status, jsonBody, requestId);
  if (flatAuthErr) return flatAuthErr;
  return jsonErr("Daemon live-patch error contract violation", requestId);
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const requestId = resolveRequestCorrelationId(ctx);
    const parsed = await parseLivePatchRequestBody(req, requestId);
    if (!parsed.ok) return parsed.response;

    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/orchestrator/live-patch",
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
    return mapDaemonLivePatchResponse(daemonResp, requestId);
  },
  {
    authMode: "strict-loopback",
    roles: ["editor", "admin"],
    rawBinary: true,
    body: OrchestratorLivePatchRequestSchema,
  }
);
