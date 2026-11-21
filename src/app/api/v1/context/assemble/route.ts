// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";
import { guardedRoute } from "@/lib/api-guard";
import { forwardToDaemonFlow } from "@/lib/daemon-client";
import {
  type ContextAssembleRequest,
  ContextAssembleRequestSchema,
  ContextAssembleSuccessSchema,
} from "@/lib/validations/context";
import {
  CONTEXT_LIMITS,
  type ContextApiError,
  type ContextApiErrorCode,
  ContextApiErrorSchema,
  isUuidV4,
} from "@/lib/validations/context-errors";

const HTTP_UNPROCESSABLE = 422;
const HTTP_SERVICE_UNAVAILABLE = 503;

export interface ContextErrorResponseParams {
  status: number;
  code: ContextApiErrorCode;
  message: string;
  requestId: string;
  field?: string;
  retryable?: boolean;
}

export function buildErrorResponse(params: ContextErrorResponseParams): Response {
  const payload: ContextApiError = {
    ok: false,
    error: {
      code: params.code,
      message: params.message,
      ...(params.field ? { field: params.field } : {}),
      retryable: params.retryable ?? false,
    },
    requestId: params.requestId,
  };
  return new Response(JSON.stringify(payload), {
    status: params.status,
    headers: { "Content-Type": "application/json; charset=utf-8" },
  });
}

export function firstZodField(err: z.ZodError): string | undefined {
  const issue = err.issues[0];
  if (!issue) return undefined;
  if (issue.code === "unrecognized_keys" && issue.keys.length > 0) {
    return issue.keys[0];
  }
  return issue.path.length > 0 ? String(issue.path[0]) : undefined;
}

export async function validateDaemonJsonResponse<T>(
  daemonResp: Response,
  successSchema: z.ZodType<T, z.ZodTypeDef, unknown>,
  requestId: string
): Promise<Response> {
  const rawText = await daemonResp.text();
  let jsonBody: unknown;
  try {
    jsonBody = JSON.parse(rawText);
  } catch {
    return buildErrorResponse({
      status: HTTP_SERVICE_UNAVAILABLE,
      code: "DAEMON_CONTRACT_VIOLATION",
      message: "contextMeter.errors.daemonContractViolation",
      requestId,
      retryable: true,
    });
  }
  const okCheck = successSchema.safeParse(jsonBody);
  if (daemonResp.ok && okCheck.success) {
    return new Response(JSON.stringify(okCheck.data), {
      status: daemonResp.status,
      headers: { "Content-Type": "application/json; charset=utf-8" },
    });
  }
  const errCheck = ContextApiErrorSchema.safeParse(jsonBody);
  if (!daemonResp.ok && errCheck.success) {
    const outHeaders = new Headers({ "Content-Type": "application/json; charset=utf-8" });
    const retryAfter = daemonResp.headers.get("retry-after");
    if (errCheck.data.error.code === "TELEMETRY_WRITE_BUSY" && retryAfter) {
      outHeaders.set("Retry-After", retryAfter);
    }
    return new Response(JSON.stringify(errCheck.data), {
      status: daemonResp.status,
      headers: outHeaders,
    });
  }
  return buildErrorResponse({
    status: HTTP_SERVICE_UNAVAILABLE,
    code: "DAEMON_CONTRACT_VIOLATION",
    message: "contextMeter.errors.daemonContractViolation",
    requestId,
    retryable: true,
  });
}

function makeInvalidRequestResponse(reqId: string, field?: string): { ok: false; response: Response } {
  return {
    ok: false,
    response: buildErrorResponse({
      status: HTTP_UNPROCESSABLE,
      code: "INVALID_REQUEST",
      message: "contextMeter.errors.invalidRequest",
      requestId: reqId,
      field,
    }),
  };
}

async function parseAssembleRequest(
  req: Request,
  reqId: string
): Promise<{ ok: true; data: ContextAssembleRequest } | { ok: false; response: Response }> {
  const rawBuf = new Uint8Array(await req.arrayBuffer());
  if (rawBuf.byteLength === 0 || rawBuf.byteLength > CONTEXT_LIMITS.MAX_CONTEXT_BODY_BYTES) {
    return makeInvalidRequestResponse(reqId);
  }
  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(rawBuf));
  } catch {
    return makeInvalidRequestResponse(reqId);
  }
  const validated = ContextAssembleRequestSchema.safeParse(parsedJson);
  if (!validated.success) {
    return makeInvalidRequestResponse(reqId, firstZodField(validated.error));
  }
  return { ok: true, data: validated.data };
}

export const POST = guardedRoute(
  async (req, ctx) => {
    const reqId =
      ctx.correlationId && isUuidV4(ctx.correlationId)
        ? ctx.correlationId
        : globalThis.crypto.randomUUID();
    const idemHeader = req.headers.get("idempotency-key") || "";
    if (!isUuidV4(idemHeader)) {
      return buildErrorResponse({
        status: HTTP_UNPROCESSABLE,
        code: "INVALID_IDEMPOTENCY_KEY",
        message: "contextMeter.errors.invalidIdempotencyKey",
        requestId: reqId,
        field: "Idempotency-Key",
      });
    }
    const parsed = await parseAssembleRequest(req, reqId);
    if (!parsed.ok) return parsed.response;
    const daemonResp = await forwardToDaemonFlow({
      method: "POST",
      daemonPath: "/api/v1/context/assemble",
      body: parsed.data,
      extraHeaders: { "Idempotency-Key": idemHeader },
      inboundHeaders: req.headers,
      principal: {
        tenantId: ctx.tenantId || "local-default",
        userId: ctx.userId || "local-operator",
        role: ctx.role || "editor",
      },
      correlationId: reqId,
      bffSessionToken: ctx.bffSessionToken,
      isHumanWorkbench: ctx.isHumanWorkbench,
      humanSessionToken: ctx.humanSessionToken,
      signal: req.signal,
    });
    return validateDaemonJsonResponse(daemonResp, ContextAssembleSuccessSchema, reqId);
  },
  { authMode: "strict-loopback", rawBinary: true, body: ContextAssembleRequestSchema }
);
